/// Stable Diffusion inference pipeline.
///
/// Supports SD 1.5 and SDXL Turbo using candle-transformers. Pipeline:
///   1. Tokenise prompt with CLIP BPE tokenizer
///   2. Encode to text embeddings (CLIP text encoder)
///   3. Initialise latent noise [1, 4, H/8, W/8]
///   4. DDIM denoising loop (UNet + classifier-free guidance)
///   5. VAE decode latents → pixel image
///   6. Return PNG bytes
///
/// LoRA merging is marked as TODO — Phase 2 work.

mod scheduler;

use candle_core::{DType, Device, IndexOp, Module, Tensor};
use candle_transformers::models::stable_diffusion as sd;
use rand::{Rng, SeedableRng};
use tokenizers::Tokenizer;

use crate::ai_gen::{
    downloader::ModelPaths,
    model_registry::{LoraSpec, ModelBase},
};
use scheduler::DdimScheduler;

// ── Public interface ──────────────────────────────────────────────────────────

/// Parameters for a single generation request.
pub struct GenerateParams {
    pub model_base: ModelBase,
    pub prompt: String,
    pub negative_prompt: String,
    pub steps: usize,
    /// Classifier-free guidance scale (7–9 for SD 1.5; 0 for SDXL Turbo)
    pub guidance_scale: f64,
    /// Must be a multiple of 8
    pub width: usize,
    pub height: usize,
    pub seed: Option<u64>,
    /// LoRA spec from the model definition (future use)
    pub lora: Option<LoraSpec>,
}

/// Run the full SD pipeline and return raw PNG bytes.
///
/// `on_step(current_step, total_steps)` is called after each UNet step so
/// the caller can forward progress to the frontend.
pub fn run_pipeline(
    paths: &ModelPaths,
    params: &GenerateParams,
    device: &Device,
    on_step: impl Fn(usize, usize),
) -> Result<Vec<u8>, String> {
    let dtype = DType::F32;

    // ── SD config ─────────────────────────────────────────────────────────────
    let sd_config = match params.model_base {
        ModelBase::Sd15 => {
            sd::StableDiffusionConfig::v1_5(None, Some(params.height), Some(params.width))
        }
        ModelBase::SdXl => {
            sd::StableDiffusionConfig::sdxl_turbo(None, Some(params.height), Some(params.width))
        }
    };

    // ── Tokenize ─────────────────────────────────────────────────────────────
    eprintln!("[AI-GEN] Loading tokenizer");
    let tokenizer = Tokenizer::from_file(&paths.tokenizer)
        .map_err(|e| format!("Load tokenizer: {e}"))?;

    let tokens     = clip_tokenize(&tokenizer, &params.prompt)?;
    let unc_tokens = clip_tokenize(&tokenizer, &params.negative_prompt)?;

    let tokens_t = int_tensor(&tokens, device)?;
    let unc_t    = int_tensor(&unc_tokens, device)?;

    // ── CLIP text encoder ─────────────────────────────────────────────────────
    eprintln!("[AI-GEN] Loading CLIP");
    let text_model =
        sd::build_clip_transformer(&sd_config.clip, &paths.clip_weights, device, dtype)
            .map_err(|e| format!("Build CLIP: {e}"))?;

    let cond_emb   = text_model.forward(&tokens_t).map_err(|e| format!("CLIP fwd: {e}"))?;
    let uncond_emb = text_model.forward(&unc_t).map_err(|e| format!("CLIP uncond: {e}"))?;

    // [2, seq_len, embed_dim] — unconditional first (diffusers convention)
    let text_embeddings = Tensor::cat(&[&uncond_emb, &cond_emb], 0)
        .map_err(|e| format!("Cat embeddings: {e}"))?;

    // ── UNet ──────────────────────────────────────────────────────────────────
    eprintln!("[AI-GEN] Loading UNet");
    // TODO (Phase 2): load lora weights and merge them before building UNet
    if params.lora.is_some() {
        eprintln!("[AI-GEN] Warning: LoRA merging not yet implemented — using base weights");
    }
    let unet = sd_config
        .build_unet(&paths.unet_weights, device, 4, false, dtype)
        .map_err(|e| format!("Build UNet: {e}"))?;

    // ── Initial latents ───────────────────────────────────────────────────────
    let latent_h = params.height / 8;
    let latent_w = params.width / 8;
    let latent_n = 4 * latent_h * latent_w;

    let mut rng = match params.seed {
        Some(s) => rand::rngs::StdRng::seed_from_u64(s),
        None    => rand::rngs::StdRng::from_entropy(),
    };
    let noise: Vec<f32> = (0..latent_n).map(|_| sample_normal(&mut rng)).collect();
    let mut latents =
        Tensor::from_vec(noise, (1usize, 4usize, latent_h, latent_w), device)
            .map_err(|e| format!("Init latents: {e}"))?;

    // ── DDIM loop ─────────────────────────────────────────────────────────────
    let scheduler = DdimScheduler::new(params.steps);
    let timesteps = scheduler.timesteps().to_vec();
    let total = timesteps.len();

    eprintln!("[AI-GEN] Denoising ({total} steps, guidance={:.1})", params.guidance_scale);

    for (step_idx, &t) in timesteps.iter().enumerate() {
        // Duplicate latents for classifier-free guidance: [2, 4, H, W]
        let latent_input = Tensor::cat(&[&latents, &latents], 0)
            .map_err(|e| format!("Cat latents step {step_idx}: {e}"))?;

        // UNet forward — predicts ε_θ(x_t, t, c)
        let noise_pred = unet
            .forward(&latent_input, t as f64, &text_embeddings)
            .map_err(|e| format!("UNet step {step_idx}: {e}"))?;

        // Split along batch dim: uncond [1,4,H,W] and cond [1,4,H,W]
        let noise_uncond = noise_pred.i(0..1).map_err(|e| format!("{e}"))?;
        let noise_cond   = noise_pred.i(1..2).map_err(|e| format!("{e}"))?;

        // ε_guided = ε_uncond + w·(ε_cond − ε_uncond)
        let guided = if params.guidance_scale == 0.0 {
            noise_cond
        } else {
            let diff = (&noise_cond - &noise_uncond)
                .map_err(|e| format!("Guidance diff: {e}"))?;
            let scaled = (diff * params.guidance_scale)
                .map_err(|e| format!("Guidance scale: {e}"))?;
            (&noise_uncond + &scaled)
                .map_err(|e| format!("Guidance add: {e}"))?
        };

        // Flatten to &[f32] for the scheduler, then reshape back
        let latents_flat: Vec<f32> = latents
            .flatten_all()
            .and_then(|t| t.to_vec1::<f32>())
            .map_err(|e| format!("Flatten latents: {e}"))?;
        let guided_flat: Vec<f32> = guided
            .flatten_all()
            .and_then(|t| t.to_vec1::<f32>())
            .map_err(|e| format!("Flatten guided: {e}"))?;

        let new_flat = scheduler.step(&latents_flat, &guided_flat, step_idx);

        latents =
            Tensor::from_vec(new_flat, (1usize, 4usize, latent_h, latent_w), device)
                .map_err(|e| format!("Rebuild latents: {e}"))?;

        on_step(step_idx + 1, total);
        eprintln!("[AI-GEN] Step {}/{total}", step_idx + 1);
    }

    // ── VAE decode ────────────────────────────────────────────────────────────
    eprintln!("[AI-GEN] Decoding with VAE");
    let vae = sd_config
        .build_vae(&paths.vae_weights, device, dtype)
        .map_err(|e| format!("Build VAE: {e}"))?;

    // Scale latents — SD convention is latent / 0.18215 before VAE decode
    let latents_scaled = (latents / 0.18215_f64)
        .map_err(|e| format!("Scale latents: {e}"))?;

    let decoded = vae
        .decode(&latents_scaled)
        .map_err(|e| format!("VAE decode: {e}"))?;

    // Map [−1, 1] → [0, 1] → u8
    let image_tensor = ((decoded
        .clamp(-1.0_f64, 1.0_f64)
        .map_err(|e| format!("Clamp: {e}"))?
        + 1.0_f64)
        .map_err(|e| format!("Shift: {e}"))?
        / 2.0_f64)
        .map_err(|e| format!("Normalise: {e}"))?;

    let image_u8 = (image_tensor * 255.0_f64)
        .map_err(|e| format!("Scale to u8: {e}"))?
        .to_dtype(DType::U8)
        .map_err(|e| format!("Cast to u8: {e}"))?;

    // NCHW → HWC → flat Vec<u8>
    let (_, _, h, w) = image_u8.dims4().map_err(|e| format!("dims4: {e}"))?;
    let pixels: Vec<u8> = image_u8
        .squeeze(0)
        .and_then(|t| t.permute((1, 2, 0)))
        .and_then(|t| t.flatten_all())
        .and_then(|t| t.to_vec1::<u8>())
        .map_err(|e| format!("Pixel extraction: {e}"))?;

    // Encode as PNG
    let img = image::RgbImage::from_raw(w as u32, h as u32, pixels)
        .ok_or("Failed to create RgbImage from decoded pixels")?;
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encode: {e}"))?;

    eprintln!("[AI-GEN] Done — {} bytes", buf.get_ref().len());
    Ok(buf.into_inner())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Tokenize `text` with CLIP BPE, pad/truncate to 77 tokens.
///
/// Token 49407 is both <|endoftext|> and the padding token in CLIP.
fn clip_tokenize(tokenizer: &Tokenizer, text: &str) -> Result<Vec<u32>, String> {
    let enc = tokenizer
        .encode(text, true)
        .map_err(|e| format!("Tokenize '{text}': {e}"))?;
    let mut ids: Vec<u32> = enc.get_ids().to_vec();
    ids.truncate(77);
    while ids.len() < 77 {
        ids.push(49407);
    }
    Ok(ids)
}

/// Build a [1, 77] i64 tensor from a token list.
fn int_tensor(tokens: &[u32], device: &Device) -> Result<Tensor, String> {
    let ids: Vec<i64> = tokens.iter().map(|&x| x as i64).collect();
    Tensor::from_vec(ids, (1usize, 77usize), device)
        .map_err(|e| format!("Token tensor: {e}"))
}

/// Box–Muller normal sample (mean=0, std=1).
fn sample_normal(rng: &mut impl Rng) -> f32 {
    let u1: f32 = rng.gen::<f32>().max(1e-10);
    let u2: f32 = rng.gen();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
}
