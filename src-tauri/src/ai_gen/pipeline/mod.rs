/// Stable Diffusion inference pipeline.
///
/// Supports SD 1.5 and SDXL Turbo using candle-transformers. Pipeline:
///   1. Tokenise prompt with CLIP BPE tokenizer
///   2. Encode to text embeddings (CLIP + CLIP2 for SDXL)
///   3. Initialise latent noise [1, 4, H/8, W/8]
///   4. DDIM / Euler-Ancestral denoising loop (UNet + CFG)
///   5. Optional Hires Fix: upscale latents → add noise → re-denoise
///   6. VAE decode latents → pixel image
///   7. Return PNG bytes
///
/// LoRA merging is marked as TODO — Phase 2 work.
mod scheduler;

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use candle_core::{safetensors as candle_safetensors, DType, Device, IndexOp, Module, Tensor};
use candle_transformers::models::stable_diffusion as sd;
use ndarray::Array2;
use rand::{Rng, SeedableRng};
use tokenizers::Tokenizer;

use crate::ai_gen::{
    downloader::ModelPaths,
    model_registry::{LoraSpec, ModelBase},
};
use scheduler::DdimScheduler;

// ── Sampler ───────────────────────────────────────────────────────────────────

/// Sampling algorithm used during the denoising loop.
#[derive(Debug, Clone, PartialEq)]
pub enum Sampler {
    /// Deterministic DDIM (eta=0). Fast, reproducible with fixed seed.
    Ddim,
    /// Euler Ancestral — stochastic noise each step. More varied outputs.
    EulerA,
    /// DPM++ 2M Karras — second-order multistep + Karras sigma spacing.
    /// Best quality-per-step ratio; recommended for ≥15 steps.
    DpmPP2MKarras,
}

impl Sampler {
    /// eta for DDIM/Euler A; DPM++ 2M uses its own step function.
    fn eta(&self) -> f64 {
        match self {
            Sampler::Ddim => 0.0,
            Sampler::EulerA => 1.0,
            Sampler::DpmPP2MKarras => 0.0,
        }
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Sampler::Ddim
    }
}

// ── Hires Fix ─────────────────────────────────────────────────────────────────

/// Parameters for the optional Hires Fix second pass.
///
/// Workflow:
///   1. Generate at base resolution (params.width × params.height)
///   2. Upscale latents to (target_width × target_height) via nearest-neighbor
///   3. Add noise at the level corresponding to `denoising_strength`
///   4. Re-denoise for `steps` steps using the same text embeddings
#[derive(Debug, Clone)]
pub struct HiresFixParams {
    /// Final image width after upscale (must be multiple of 8)
    pub width: usize,
    /// Final image height after upscale (must be multiple of 8)
    pub height: usize,
    /// How strongly to re-denoise (0.0=no change, 1.0=fully re-denoise).
    /// Typical values: 0.35–0.65
    pub denoising_strength: f64,
    /// Number of UNet steps for the hires pass
    pub steps: usize,
}

// ── Public interface ──────────────────────────────────────────────────────────

/// Parameters for a single generation request.
pub struct GenerateParams {
    pub model_base: ModelBase,
    pub prompt: String,
    pub negative_prompt: String,
    pub steps: usize,
    /// Classifier-free guidance scale (7–9 for SD 1.5; 0 for SDXL Turbo)
    pub guidance_scale: f64,
    /// Base generation width in pixels (must be multiple of 8)
    pub width: usize,
    /// Base generation height in pixels (must be multiple of 8)
    pub height: usize,
    pub seed: Option<u64>,
    pub lora: Option<LoraSpec>,
    /// Sampling algorithm
    pub sampler: Sampler,
    /// If Some, run a second high-resolution denoising pass after the first
    pub hires_fix: Option<HiresFixParams>,
}

/// Run the full SD pipeline and return raw PNG bytes.
///
/// `on_step(current_step, total_steps)` is called after each UNet step so
/// the caller can forward progress to the frontend.
pub fn run_pipeline(
    paths: &ModelPaths,
    params: &GenerateParams,
    device: &Device,
    on_status: impl Fn(&str),
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
    on_status("loadingTokenizer");
    eprintln!("[AI-GEN] Loading tokenizer");
    let tokenizer =
        Tokenizer::from_file(&paths.tokenizer).map_err(|e| format!("Load tokenizer: {e}"))?;

    let tokens = clip_tokenize(&tokenizer, &params.prompt)?;
    let unc_tokens = clip_tokenize(&tokenizer, &params.negative_prompt)?;

    let tokens_t = int_tensor(&tokens, device)?;
    let unc_t = int_tensor(&unc_tokens, device)?;

    // ── CLIP text encoder ─────────────────────────────────────────────────────
    on_status("loadingClip");
    eprintln!("[AI-GEN] Loading CLIP");
    let text_model =
        sd::build_clip_transformer(&sd_config.clip, &paths.clip_weights, device, dtype)
            .map_err(|e| format!("Build CLIP: {e}"))?;

    let cond_emb = text_model
        .forward(&tokens_t)
        .map_err(|e| format!("CLIP fwd: {e}"))?;
    let uncond_emb = text_model
        .forward(&unc_t)
        .map_err(|e| format!("CLIP uncond: {e}"))?;

    // ── SDXL: second text encoder (OpenCLIP ViT-bigG, 1280-dim) ──────────────
    // SDXL UNet cross-attention expects 2048-dim = 768 (CLIP1) + 1280 (CLIP2).
    // For SD 1.5 we skip this and use the 768-dim embeddings as-is.
    let (cond_final, uncond_final) = if matches!(params.model_base, ModelBase::SdXl) {
        on_status("loadingClip2");
        let clip2_cfg = sd_config.clip2.as_ref().ok_or_else(|| {
            "SDXL sd_config missing clip2 — candle version may be too old".to_string()
        })?;
        let clip2_path = paths.clip2_weights.as_ref().ok_or_else(|| {
            "SDXL paths missing clip2_weights — re-download the model".to_string()
        })?;

        eprintln!("[AI-GEN] Loading CLIP2 (OpenCLIP ViT-bigG, 1280-dim)");
        let text_model2 = sd::build_clip_transformer(clip2_cfg, clip2_path, device, dtype)
            .map_err(|e| format!("Build CLIP2: {e}"))?;

        let cond2 = text_model2
            .forward(&tokens_t)
            .map_err(|e| format!("CLIP2 fwd: {e}"))?;
        let uncond2 = text_model2
            .forward(&unc_t)
            .map_err(|e| format!("CLIP2 uncond: {e}"))?;

        // Concat along last dim: [1, 77, 768] ++ [1, 77, 1280] → [1, 77, 2048]
        let cond =
            Tensor::cat(&[&cond_emb, &cond2], 2).map_err(|e| format!("Cat CLIP emb: {e}"))?;
        let uncond = Tensor::cat(&[&uncond_emb, &uncond2], 2)
            .map_err(|e| format!("Cat CLIP uncond: {e}"))?;
        (cond, uncond)
    } else {
        (cond_emb, uncond_emb)
    };

    // [2, seq_len, embed_dim] — unconditional first (diffusers convention)
    let text_embeddings = Tensor::cat(&[&uncond_final, &cond_final], 0)
        .map_err(|e| format!("Cat embeddings: {e}"))?;

    // ── UNet ──────────────────────────────────────────────────────────────────
    on_status(if params.lora.is_some() {
        "mergingLora"
    } else {
        "loadingUnet"
    });
    eprintln!("[AI-GEN] Loading UNet");
    let unet_weights = resolve_unet_weights_path(paths, params)?;
    on_status("loadingUnet");
    let unet = sd_config
        .build_unet(&unet_weights, device, 4, false, dtype)
        .map_err(|e| format!("Build UNet: {e}"))?;

    // ── Initial latents ───────────────────────────────────────────────────────
    let latent_h = params.height / 8;
    let latent_w = params.width / 8;

    let mut rng = match params.seed {
        Some(s) => rand::rngs::StdRng::seed_from_u64(s),
        None => rand::rngs::StdRng::from_entropy(),
    };

    let mut latents = random_latents(latent_h, latent_w, device, &mut rng)?;

    // ── Base denoising loop ───────────────────────────────────────────────────
    let has_hires = params.hires_fix.is_some();
    let base_total = params.steps;
    let hires_total = params.hires_fix.as_ref().map_or(0, |h| h.steps);
    // Total steps reported to the caller = base + hires
    let report_total = base_total + hires_total;

    // Build scheduler — DPM++ 2M Karras uses its own constructor.
    let scheduler = match params.sampler {
        Sampler::DpmPP2MKarras => DdimScheduler::with_karras(params.steps),
        _ => DdimScheduler::with_eta(params.steps, params.sampler.eta()),
    };
    let timesteps = scheduler.timesteps().to_vec();

    eprintln!(
        "[AI-GEN] Denoising ({} steps, guidance={:.1}, sampler={:?}{})",
        base_total,
        params.guidance_scale,
        params.sampler,
        if has_hires { ", hires_fix=on" } else { "" }
    );

    // DPM++ 2M multistep state: (prev_denoised, prev_h)
    let mut dpm2m_state: Option<(Vec<f32>, f64)> = None;

    for (step_idx, &t) in timesteps.iter().enumerate() {
        let latent_input = Tensor::cat(&[&latents, &latents], 0)
            .map_err(|e| format!("Cat latents step {step_idx}: {e}"))?;

        let noise_pred = unet
            .forward(&latent_input, t as f64, &text_embeddings)
            .map_err(|e| format!("UNet step {step_idx}: {e}"))?;

        let noise_uncond = noise_pred.i(0..1).map_err(|e| format!("{e}"))?;
        let noise_cond = noise_pred.i(1..2).map_err(|e| format!("{e}"))?;

        let guided = if params.guidance_scale == 0.0 {
            noise_cond
        } else {
            let diff = (&noise_cond - &noise_uncond).map_err(|e| format!("Guidance diff: {e}"))?;
            let scaled =
                (diff * params.guidance_scale).map_err(|e| format!("Guidance scale: {e}"))?;
            (&noise_uncond + &scaled).map_err(|e| format!("Guidance add: {e}"))?
        };

        let latents_flat: Vec<f32> = latents
            .flatten_all()
            .and_then(|t| t.to_vec1::<f32>())
            .map_err(|e| format!("Flatten latents: {e}"))?;
        let guided_flat: Vec<f32> = guided
            .flatten_all()
            .and_then(|t| t.to_vec1::<f32>())
            .map_err(|e| format!("Flatten guided: {e}"))?;

        // Dispatch to the correct sampler step function
        let new_flat = match params.sampler {
            Sampler::DpmPP2MKarras => {
                let prev = dpm2m_state.as_ref().map(|(d, h)| (d.as_slice(), *h));
                let (new_latents, denoised, h) =
                    scheduler.step_dpm2m(&latents_flat, &guided_flat, step_idx, prev);
                dpm2m_state = Some((denoised, h));
                new_latents
            }
            _ => {
                let step_noise = if scheduler.eta > 0.0 {
                    Some(sample_noise_vec(latents_flat.len(), &mut rng))
                } else {
                    None
                };
                scheduler.step(&latents_flat, &guided_flat, step_idx, step_noise.as_deref())
            }
        };

        latents = Tensor::from_vec(new_flat, (1usize, 4usize, latent_h, latent_w), device)
            .map_err(|e| format!("Rebuild latents: {e}"))?;

        on_step(step_idx + 1, report_total);
        eprintln!("[AI-GEN] Step {}/{base_total}", step_idx + 1);
    }

    // ── Hires Fix pass ────────────────────────────────────────────────────────
    if let Some(hires) = &params.hires_fix {
        eprintln!(
            "[AI-GEN] Hires Fix: {}×{} → {}×{}, strength={:.2}, steps={}",
            params.width,
            params.height,
            hires.width,
            hires.height,
            hires.denoising_strength,
            hires.steps,
        );

        let hires_lh = hires.height / 8;
        let hires_lw = hires.width / 8;

        // 1. Upscale latents to target resolution (nearest-neighbor in latent space)
        let latents_up = latents
            .upsample_nearest2d(hires_lh, hires_lw)
            .map_err(|e| format!("Hires upsample: {e}"))?;

        // 2. Add noise corresponding to denoising_strength
        let start_t = DdimScheduler::start_timestep(hires.denoising_strength);
        let noise_vec = sample_noise_vec(4 * hires_lh * hires_lw, &mut rng);
        let latents_up_flat: Vec<f32> = latents_up
            .flatten_all()
            .and_then(|t| t.to_vec1::<f32>())
            .map_err(|e| format!("Flatten hires latents: {e}"))?;

        // Build a temporary full scheduler just for add_noise (needs full alphas_cumprod)
        let noise_sched = DdimScheduler::with_eta(hires.steps, params.sampler.eta());
        let noisy_flat = noise_sched.add_noise(&latents_up_flat, &noise_vec, start_t);

        let mut hires_latents =
            Tensor::from_vec(noisy_flat, (1usize, 4usize, hires_lh, hires_lw), device)
                .map_err(|e| format!("Rebuild hires latents: {e}"))?;

        // 3. Re-build UNet for hires resolution (SD config is resolution-aware)
        let hires_sd_config = match params.model_base {
            ModelBase::Sd15 => {
                sd::StableDiffusionConfig::v1_5(None, Some(hires.height), Some(hires.width))
            }
            ModelBase::SdXl => {
                sd::StableDiffusionConfig::sdxl_turbo(None, Some(hires.height), Some(hires.width))
            }
        };
        on_status("loadingUnet");
        let hires_unet = hires_sd_config
            .build_unet(&unet_weights, device, 4, false, dtype)
            .map_err(|e| format!("Build hires UNet: {e}"))?;

        // 4. Hires denoising loop
        let hires_scheduler =
            DdimScheduler::for_img2img(hires.steps, hires.denoising_strength, params.sampler.eta());
        let hires_timesteps = hires_scheduler.timesteps().to_vec();
        let hires_actual_steps = hires_timesteps.len();

        eprintln!("[AI-GEN] Hires denoising ({hires_actual_steps} steps)");

        for (step_idx, &t) in hires_timesteps.iter().enumerate() {
            let latent_input = Tensor::cat(&[&hires_latents, &hires_latents], 0)
                .map_err(|e| format!("Hires cat latents: {e}"))?;

            let noise_pred = hires_unet
                .forward(&latent_input, t as f64, &text_embeddings)
                .map_err(|e| format!("Hires UNet step {step_idx}: {e}"))?;

            let noise_uncond = noise_pred.i(0..1).map_err(|e| format!("{e}"))?;
            let noise_cond = noise_pred.i(1..2).map_err(|e| format!("{e}"))?;

            let guided = if params.guidance_scale == 0.0 {
                noise_cond
            } else {
                let diff = (&noise_cond - &noise_uncond)
                    .map_err(|e| format!("Hires guidance diff: {e}"))?;
                let scaled = (diff * params.guidance_scale)
                    .map_err(|e| format!("Hires guidance scale: {e}"))?;
                (&noise_uncond + &scaled).map_err(|e| format!("Hires guidance add: {e}"))?
            };

            let hires_flat: Vec<f32> = hires_latents
                .flatten_all()
                .and_then(|t| t.to_vec1::<f32>())
                .map_err(|e| format!("Flatten hires: {e}"))?;
            let guided_flat: Vec<f32> = guided
                .flatten_all()
                .and_then(|t| t.to_vec1::<f32>())
                .map_err(|e| format!("Flatten hires guided: {e}"))?;

            let step_noise = if hires_scheduler.eta > 0.0 {
                Some(sample_noise_vec(hires_flat.len(), &mut rng))
            } else {
                None
            };

            let new_flat =
                hires_scheduler.step(&hires_flat, &guided_flat, step_idx, step_noise.as_deref());

            hires_latents =
                Tensor::from_vec(new_flat, (1usize, 4usize, hires_lh, hires_lw), device)
                    .map_err(|e| format!("Rebuild hires latents: {e}"))?;

            on_step(base_total + step_idx + 1, report_total);
            eprintln!("[AI-GEN] Hires step {}/{hires_actual_steps}", step_idx + 1);
        }

        latents = hires_latents;
    }

    // ── VAE decode ────────────────────────────────────────────────────────────
    let (final_h, final_w) = if let Some(hires) = &params.hires_fix {
        (hires.height, hires.width)
    } else {
        (params.height, params.width)
    };

    let vae_sd_config = match params.model_base {
        ModelBase::Sd15 => sd::StableDiffusionConfig::v1_5(None, Some(final_h), Some(final_w)),
        ModelBase::SdXl => {
            sd::StableDiffusionConfig::sdxl_turbo(None, Some(final_h), Some(final_w))
        }
    };

    on_status("decodingVae");
    eprintln!("[AI-GEN] Decoding with VAE ({}×{})", final_w, final_h);
    let vae = vae_sd_config
        .build_vae(&paths.vae_weights, device, dtype)
        .map_err(|e| format!("Build VAE: {e}"))?;

    // Scale latents before VAE decode.
    // SD 1.5 uses 0.18215; SDXL uses 0.13025 — different VAE training scales.
    let vae_scale = match params.model_base {
        ModelBase::Sd15 => 0.18215_f64,
        ModelBase::SdXl => 0.13025_f64,
    };
    let latents_scaled = (latents / vae_scale).map_err(|e| format!("Scale latents: {e}"))?;

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
    Tensor::from_vec(ids, (1usize, 77usize), device).map_err(|e| format!("Token tensor: {e}"))
}

/// Initialize random latent noise [1, 4, h, w].
fn random_latents(
    h: usize,
    w: usize,
    device: &Device,
    rng: &mut impl Rng,
) -> Result<Tensor, String> {
    let n = 4 * h * w;
    let noise: Vec<f32> = (0..n).map(|_| sample_normal(rng)).collect();
    Tensor::from_vec(noise, (1usize, 4usize, h, w), device)
        .map_err(|e| format!("Init latents: {e}"))
}

/// Generate a noise vector of length `n`.
fn sample_noise_vec(n: usize, rng: &mut impl Rng) -> Vec<f32> {
    (0..n).map(|_| sample_normal(rng)).collect()
}

/// Box–Muller normal sample (mean=0, std=1).
fn sample_normal(rng: &mut impl Rng) -> f32 {
    let u1: f32 = rng.gen::<f32>().max(1e-10);
    let u2: f32 = rng.gen();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
}

fn resolve_unet_weights_path(
    paths: &ModelPaths,
    params: &GenerateParams,
) -> Result<PathBuf, String> {
    let Some(lora) = params.lora.as_ref() else {
        return Ok(paths.unet_weights.clone());
    };

    if !matches!(params.model_base, ModelBase::SdXl) {
        return Err("LoRA merging is currently implemented only for SDXL models".into());
    }

    let lora_path = paths.lora_weights.as_ref().ok_or_else(|| {
        "Model expects a LoRA file but no lora_weights path was provided".to_string()
    })?;

    ensure_merged_unet_weights(&paths.unet_weights, lora_path, lora)
}

fn ensure_merged_unet_weights(
    base_unet_path: &Path,
    lora_path: &Path,
    lora: &LoraSpec,
) -> Result<PathBuf, String> {
    let cache_path = merged_unet_cache_path(base_unet_path, lora);
    if merged_unet_cache_is_fresh(&cache_path, base_unet_path, lora_path)? {
        eprintln!(
            "[AI-GEN] Using cached merged UNet weights: {}",
            cache_path.display()
        );
        return Ok(cache_path);
    }

    eprintln!(
        "[AI-GEN] Merging LoRA '{}' into UNet weights",
        lora.filename
    );
    merge_sdxl_unet_lora_to_file(base_unet_path, lora_path, lora.scale, &cache_path)?;
    Ok(cache_path)
}

fn merged_unet_cache_path(base_unet_path: &Path, lora: &LoraSpec) -> PathBuf {
    let parent = base_unet_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = Path::new(lora.filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("lora");
    let safe_stem: String = stem
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();
    let scale_tag = format!("{:.3}", lora.scale).replace('.', "_");
    parent.join(format!(
        "diffusion_pytorch_model.merged-{safe_stem}-s{scale_tag}.safetensors"
    ))
}

fn merged_unet_cache_is_fresh(
    cache_path: &Path,
    base_unet_path: &Path,
    lora_path: &Path,
) -> Result<bool, String> {
    if !cache_path.exists() {
        return Ok(false);
    }

    let cache_time = modified_time(cache_path)?;
    Ok(cache_time >= modified_time(base_unet_path)? && cache_time >= modified_time(lora_path)?)
}

fn modified_time(path: &Path) -> Result<SystemTime, String> {
    fs::metadata(path)
        .map_err(|e| format!("Read metadata for {}: {e}", path.display()))?
        .modified()
        .map_err(|e| format!("Read modified time for {}: {e}", path.display()))
}

fn merge_sdxl_unet_lora_to_file(
    base_unet_path: &Path,
    lora_path: &Path,
    lora_scale: f32,
    output_path: &Path,
) -> Result<(), String> {
    let merge_start = std::time::Instant::now();
    let cpu = Device::Cpu;
    let mut merged = candle_safetensors::load(base_unet_path, &cpu)
        .map_err(|e| format!("Load base UNet weights: {e}"))?;
    let lora_weights = unsafe {
        candle_core::safetensors::MmapedSafetensors::new(lora_path)
            .map_err(|e| format!("Open LoRA safetensors: {e}"))?
    };

    let mut applied = 0usize;
    let mut skipped = 0usize;

    let module_names = kohya_lora_module_names(&lora_weights);
    let total_modules = module_names.len();
    for (module_idx, module_name) in module_names.into_iter().enumerate() {
        let Some(target_prefix) = sdxl_kohya_lora_module_to_candle_prefix(&module_name) else {
            skipped += 1;
            continue;
        };
        let target_weight_name = format!("{target_prefix}.weight");
        let Some(base_weight) = merged.get(&target_weight_name).cloned() else {
            skipped += 1;
            continue;
        };

        let down = lora_weights
            .load(&format!("{module_name}.lora_down.weight"), &cpu)
            .map_err(|e| format!("Load LoRA down weight for {module_name}: {e}"))?
            .to_dtype(DType::F32)
            .map_err(|e| format!("Cast LoRA down weight for {module_name}: {e}"))?;
        let up = lora_weights
            .load(&format!("{module_name}.lora_up.weight"), &cpu)
            .map_err(|e| format!("Load LoRA up weight for {module_name}: {e}"))?
            .to_dtype(DType::F32)
            .map_err(|e| format!("Cast LoRA up weight for {module_name}: {e}"))?;
        let alpha = read_lora_alpha(&lora_weights, &module_name, down.dims()[0] as f32)?;
        let delta = compute_lora_delta(&up, &down, alpha, lora_scale)?;

        if delta.shape() != base_weight.shape() {
            return Err(format!(
                "Merged LoRA shape mismatch for {target_weight_name}: delta {:?}, base {:?}",
                delta.shape().dims(),
                base_weight.shape().dims()
            ));
        }

        let merged_weight = (base_weight
            .to_dtype(DType::F32)
            .map_err(|e| format!("Cast base weight for {target_weight_name}: {e}"))?
            + delta)
            .map_err(|e| format!("Add LoRA delta for {target_weight_name}: {e}"))?
            .to_dtype(base_weight.dtype())
            .map_err(|e| format!("Restore dtype for {target_weight_name}: {e}"))?;
        merged.insert(target_weight_name, merged_weight);
        applied += 1;

        if applied == 1 || applied % 25 == 0 || module_idx + 1 == total_modules {
            eprintln!(
                "[AI-GEN] LoRA merge progress: {}/{} modules applied",
                applied, total_modules
            );
        }
    }

    if applied == 0 {
        return Err(format!(
            "No SDXL UNet LoRA modules could be applied from {}",
            lora_path.display()
        ));
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Create merged weights directory {}: {e}", parent.display()))?;
    }
    eprintln!(
        "[AI-GEN] Writing merged UNet weights to disk: {}",
        output_path.display()
    );
    candle_safetensors::save(&merged, output_path)
        .map_err(|e| format!("Save merged UNet weights to {}: {e}", output_path.display()))?;

    eprintln!(
        "[AI-GEN] LoRA merge complete in {:.1}s: applied {applied} modules, skipped {skipped}",
        merge_start.elapsed().as_secs_f32()
    );
    Ok(())
}

fn kohya_lora_module_names(
    lora_weights: &candle_core::safetensors::MmapedSafetensors,
) -> Vec<String> {
    let mut names = BTreeSet::new();
    for (name, _) in lora_weights.tensors() {
        if let Some(module_name) = name.strip_suffix(".lora_down.weight") {
            names.insert(module_name.to_string());
        }
    }
    names.into_iter().collect()
}

fn read_lora_alpha(
    lora_weights: &candle_core::safetensors::MmapedSafetensors,
    module_name: &str,
    fallback: f32,
) -> Result<f32, String> {
    let alpha_key = format!("{module_name}.alpha");
    match lora_weights.load(&alpha_key, &Device::Cpu) {
        Ok(alpha) => {
            let values = alpha
                .to_dtype(DType::F32)
                .map_err(|e| format!("Cast alpha for {module_name}: {e}"))?
                .flatten_all()
                .map_err(|e| format!("Flatten alpha for {module_name}: {e}"))?
                .to_vec1::<f32>()
                .map_err(|e| format!("Read alpha for {module_name}: {e}"))?;
            Ok(*values.first().unwrap_or(&fallback))
        }
        Err(_) => Ok(fallback),
    }
}

fn compute_lora_delta(
    up: &Tensor,
    down: &Tensor,
    alpha: f32,
    lora_scale: f32,
) -> Result<Tensor, String> {
    let rank = down.dims().first().copied().unwrap_or(0);
    if rank == 0 {
        return Err("LoRA rank must be greater than zero".into());
    }
    let effective_scale = (alpha / rank as f32) * lora_scale;

    match down.dims() {
        [_, _] => compute_linear_lora_delta(up, down, effective_scale),
        [_, _, _, _] => compute_conv_lora_delta(up, down, effective_scale),
        dims => Err(format!("Unsupported LoRA tensor rank: {dims:?}")),
    }
}

fn compute_linear_lora_delta(up: &Tensor, down: &Tensor, scale: f32) -> Result<Tensor, String> {
    let up_shape = up.dims();
    let down_shape = down.dims();
    if up_shape.len() != 2 || down_shape.len() != 2 {
        return Err("Linear LoRA tensors must both be rank-2".into());
    }
    let out_dim = up_shape[0];
    let rank = up_shape[1];
    let in_dim = down_shape[1];
    if down_shape[0] != rank {
        return Err(format!(
            "Linear LoRA rank mismatch: up {:?}, down {:?}",
            up_shape, down_shape
        ));
    }

    let up_values = up
        .flatten_all()
        .map_err(|e| format!("Flatten LoRA up tensor: {e}"))?
        .to_vec1::<f32>()
        .map_err(|e| format!("Read LoRA up tensor: {e}"))?;
    let down_values = down
        .flatten_all()
        .map_err(|e| format!("Flatten LoRA down tensor: {e}"))?
        .to_vec1::<f32>()
        .map_err(|e| format!("Read LoRA down tensor: {e}"))?;

    let up_mat = Array2::from_shape_vec((out_dim, rank), up_values)
        .map_err(|e| format!("Build linear LoRA up matrix: {e}"))?;
    let down_mat = Array2::from_shape_vec((rank, in_dim), down_values)
        .map_err(|e| format!("Build linear LoRA down matrix: {e}"))?;
    let merged = array_into_raw_vec(up_mat.dot(&down_mat) * scale);

    Tensor::from_vec(merged, (out_dim, in_dim), &Device::Cpu)
        .map_err(|e| format!("Build merged linear LoRA tensor: {e}"))
}

fn compute_conv_lora_delta(up: &Tensor, down: &Tensor, scale: f32) -> Result<Tensor, String> {
    let up_shape = up.dims();
    let down_shape = down.dims();
    if up_shape.len() != 4 || down_shape.len() != 4 {
        return Err("Conv LoRA tensors must both be rank-4".into());
    }

    let out_channels = up_shape[0];
    let rank = up_shape[1];
    let rank_down = down_shape[0];
    let in_channels = down_shape[1];
    if rank != rank_down {
        return Err(format!(
            "Conv LoRA rank mismatch: up {:?}, down {:?}",
            up_shape, down_shape
        ));
    }

    let up_kernel_h = up_shape[2];
    let up_kernel_w = up_shape[3];
    let down_kernel_h = down_shape[2];
    let down_kernel_w = down_shape[3];

    if (up_kernel_h, up_kernel_w) != (1, 1) && (down_kernel_h, down_kernel_w) != (1, 1) {
        return Err(format!(
            "Unsupported conv LoRA kernels: up {:?}, down {:?}",
            up_shape, down_shape
        ));
    }

    let up_values = up
        .flatten_all()
        .map_err(|e| format!("Flatten conv LoRA up tensor: {e}"))?
        .to_vec1::<f32>()
        .map_err(|e| format!("Read conv LoRA up tensor: {e}"))?;
    let down_values = down
        .flatten_all()
        .map_err(|e| format!("Flatten conv LoRA down tensor: {e}"))?
        .to_vec1::<f32>()
        .map_err(|e| format!("Read conv LoRA down tensor: {e}"))?;

    let merged = if (up_kernel_h, up_kernel_w) == (1, 1) {
        let down_mat = Array2::from_shape_vec(
            (rank, in_channels * down_kernel_h * down_kernel_w),
            down_values,
        )
        .map_err(|e| format!("Build conv LoRA down matrix: {e}"))?;
        let up_mat = Array2::from_shape_vec((out_channels, rank), up_values)
            .map_err(|e| format!("Build conv LoRA up matrix: {e}"))?;
        let merged = array_into_raw_vec(up_mat.dot(&down_mat) * scale);
        (
            merged,
            out_channels,
            in_channels,
            down_kernel_h,
            down_kernel_w,
        )
    } else {
        let up_mat = Array2::from_shape_vec(
            (out_channels * up_kernel_h * up_kernel_w, rank),
            reorder_up_conv_values(&up_values, out_channels, rank, up_kernel_h, up_kernel_w),
        )
        .map_err(|e| format!("Build conv LoRA up matrix: {e}"))?;
        let down_mat = Array2::from_shape_vec((rank, in_channels), down_values)
            .map_err(|e| format!("Build conv LoRA down matrix: {e}"))?;
        let merged_2d = up_mat.dot(&down_mat) * scale;
        (
            reorder_merged_conv_values(
                array_into_raw_vec(merged_2d),
                out_channels,
                in_channels,
                up_kernel_h,
                up_kernel_w,
            ),
            out_channels,
            in_channels,
            up_kernel_h,
            up_kernel_w,
        )
    };

    Tensor::from_vec(
        merged.0,
        (merged.1, merged.2, merged.3, merged.4),
        &Device::Cpu,
    )
    .map_err(|e| format!("Build merged conv LoRA tensor: {e}"))
}

fn reorder_up_conv_values(
    values: &[f32],
    out_channels: usize,
    rank: usize,
    kernel_h: usize,
    kernel_w: usize,
) -> Vec<f32> {
    let mut reordered = vec![0f32; values.len()];
    for out_idx in 0..out_channels {
        for kh in 0..kernel_h {
            for kw in 0..kernel_w {
                for rank_idx in 0..rank {
                    let src = ((out_idx * rank + rank_idx) * kernel_h + kh) * kernel_w + kw;
                    let dst = ((out_idx * kernel_h + kh) * kernel_w + kw) * rank + rank_idx;
                    reordered[dst] = values[src];
                }
            }
        }
    }
    reordered
}

fn reorder_merged_conv_values(
    values: Vec<f32>,
    out_channels: usize,
    in_channels: usize,
    kernel_h: usize,
    kernel_w: usize,
) -> Vec<f32> {
    let mut reordered = vec![0f32; values.len()];
    for out_idx in 0..out_channels {
        for kh in 0..kernel_h {
            for kw in 0..kernel_w {
                for in_idx in 0..in_channels {
                    let src = ((out_idx * kernel_h + kh) * kernel_w + kw) * in_channels + in_idx;
                    let dst = ((out_idx * in_channels + in_idx) * kernel_h + kh) * kernel_w + kw;
                    reordered[dst] = values[src];
                }
            }
        }
    }
    reordered
}

fn array_into_raw_vec<T, D>(array: ndarray::Array<T, D>) -> Vec<T>
where
    D: ndarray::Dimension,
{
    let (values, offset) = array.into_raw_vec_and_offset();
    debug_assert_eq!(offset, Some(0));
    values
}

fn sdxl_kohya_lora_module_to_candle_prefix(module_name: &str) -> Option<String> {
    if !module_name.starts_with("lora_unet_") {
        return None;
    }

    let search = module_name.trim_start_matches("lora_unet_");
    for (kohya_prefix, candle_prefix) in sdxl_unet_lora_prefix_map() {
        if let Some(suffix) = search.strip_prefix(&kohya_prefix) {
            let suffix = kohya_suffix_to_candle_path(suffix);
            return if suffix.is_empty() {
                Some(candle_prefix)
            } else {
                Some(format!("{candle_prefix}.{suffix}"))
            };
        }
    }
    None
}

fn kohya_suffix_to_candle_path(suffix: &str) -> String {
    let suffix = suffix.trim_start_matches('_');
    if suffix.is_empty() {
        return String::new();
    }

    let mut path = suffix.to_string();
    let replacements = [
        ("transformer_blocks_", "transformer_blocks."),
        ("_attn1_to_out_0", ".attn1.to_out.0"),
        ("_attn2_to_out_0", ".attn2.to_out.0"),
        ("_attn1_to_q", ".attn1.to_q"),
        ("_attn1_to_k", ".attn1.to_k"),
        ("_attn1_to_v", ".attn1.to_v"),
        ("_attn2_to_q", ".attn2.to_q"),
        ("_attn2_to_k", ".attn2.to_k"),
        ("_attn2_to_v", ".attn2.to_v"),
        ("_ff_net_0_proj", ".ff.net.0.proj"),
        ("_ff_net_2", ".ff.net.2"),
        ("_time_emb_proj", ".time_emb_proj"),
        ("_conv_shortcut", ".conv_shortcut"),
        ("_proj_in", ".proj_in"),
        ("_proj_out", ".proj_out"),
        ("_conv1", ".conv1"),
        ("_conv2", ".conv2"),
        ("_norm1", ".norm1"),
        ("_norm2", ".norm2"),
        ("_conv", ".conv"),
    ];
    for (from, to) in replacements {
        path = path.replace(from, to);
    }
    path
}

fn sdxl_unet_lora_prefix_map() -> Vec<(String, String)> {
    let mut map = Vec::new();

    for i in 0..3 {
        for j in 0..2 {
            let sd_down_res = format!("input_blocks_{}_0", 3 * i + j + 1);
            let hf_down_res = format!("down_blocks.{i}.resnets.{j}");
            append_resnet_mapping(&mut map, &sd_down_res, &hf_down_res);

            let sd_down_attn = format!("input_blocks_{}_1", 3 * i + j + 1);
            let hf_down_attn = format!("down_blocks.{i}.attentions.{j}");
            map.push((sd_down_attn, hf_down_attn));
        }

        for j in 0..3 {
            let sd_up_res = format!("output_blocks_{}_0", 3 * i + j);
            let hf_up_res = format!("up_blocks.{i}.resnets.{j}");
            append_resnet_mapping(&mut map, &sd_up_res, &hf_up_res);

            let sd_up_attn = format!("output_blocks_{}_1", 3 * i + j);
            let hf_up_attn = format!("up_blocks.{i}.attentions.{j}");
            map.push((sd_up_attn, hf_up_attn));
        }

        let sd_downsample = format!("input_blocks_{}_0_op", 3 * (i + 1));
        let hf_downsample = format!("down_blocks.{i}.downsamplers.0.conv");
        map.push((sd_downsample, hf_downsample));

        let sd_upsample = format!("output_blocks_{}_2", 3 * i + 2);
        let hf_upsample = format!("up_blocks.{i}.upsamplers.0");
        map.push((sd_upsample, hf_upsample));
    }

    map.push((
        "middle_block_1".to_string(),
        "mid_block.attentions.0".to_string(),
    ));
    for j in 0..2 {
        let sd_mid_res = format!("middle_block_{}", 2 * j);
        let hf_mid_res = format!("mid_block.resnets.{j}");
        append_resnet_mapping(&mut map, &sd_mid_res, &hf_mid_res);
    }

    for j in 0..2 {
        map.push((
            format!("time_embed_{}", j * 2),
            format!("time_embedding.linear_{}", j + 1),
        ));
        map.push((
            format!("label_emb_0_{}", j * 2),
            format!("add_embedding.linear_{}", j + 1),
        ));
    }

    map.push(("input_blocks_0_0".to_string(), "conv_in".to_string()));
    map.push(("out_0".to_string(), "conv_norm_out".to_string()));
    map.push(("out_2".to_string(), "conv_out".to_string()));

    map.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
    map
}

fn append_resnet_mapping(map: &mut Vec<(String, String)>, sd_prefix: &str, candle_prefix: &str) {
    let replacements = [
        ("in_layers_0", "norm1"),
        ("in_layers_2", "conv1"),
        ("out_layers_0", "norm2"),
        ("out_layers_3", "conv2"),
        ("emb_layers_1", "time_emb_proj"),
        ("skip_connection", "conv_shortcut"),
    ];

    for (sd_suffix, candle_suffix) in replacements {
        map.push((
            format!("{sd_prefix}_{sd_suffix}"),
            format!("{candle_prefix}.{candle_suffix}"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_sdxl_kohya_lora_keys_to_candle_unet_weights() {
        assert_eq!(
            sdxl_kohya_lora_module_to_candle_prefix(
                "lora_unet_input_blocks_4_1_transformer_blocks_0_attn1_to_k"
            )
            .as_deref(),
            Some("down_blocks.1.attentions.0.transformer_blocks.0.attn1.to_k")
        );
        assert_eq!(
            sdxl_kohya_lora_module_to_candle_prefix("lora_unet_middle_block_1_proj_out").as_deref(),
            Some("mid_block.attentions.0.proj_out")
        );
        assert_eq!(
            sdxl_kohya_lora_module_to_candle_prefix("lora_unet_output_blocks_2_2_conv").as_deref(),
            Some("up_blocks.0.upsamplers.0.conv")
        );
    }

    #[test]
    fn computes_linear_lora_delta_with_alpha_and_scale() {
        let cpu = Device::Cpu;
        let down = Tensor::from_vec(vec![1f32, 2f32, 3f32, 4f32], (2, 2), &cpu).unwrap();
        let up = Tensor::from_vec(vec![5f32, 6f32, 7f32, 8f32], (2, 2), &cpu).unwrap();

        let delta = compute_lora_delta(&up, &down, 4.0, 0.5).unwrap();
        let values = delta.flatten_all().unwrap().to_vec1::<f32>().unwrap();

        assert_eq!(values, vec![23.0, 34.0, 31.0, 46.0]);
    }

    #[test]
    fn computes_conv_lora_delta_for_1x1_up_and_3x3_down() {
        let cpu = Device::Cpu;
        let up = Tensor::from_vec(vec![2f32], (1, 1, 1, 1), &cpu).unwrap();
        let down =
            Tensor::from_vec((1..=9).map(|v| v as f32).collect(), (1, 1, 3, 3), &cpu).unwrap();

        let delta = compute_conv_lora_delta(&up, &down, 0.5).unwrap();
        let values = delta.flatten_all().unwrap().to_vec1::<f32>().unwrap();

        assert_eq!(values, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn computes_conv_lora_delta_for_3x3_up_and_1x1_down() {
        let cpu = Device::Cpu;
        let up = Tensor::from_vec((1..=9).map(|v| v as f32).collect(), (1, 1, 3, 3), &cpu).unwrap();
        let down = Tensor::from_vec(vec![2f32], (1, 1, 1, 1), &cpu).unwrap();

        let delta = compute_conv_lora_delta(&up, &down, 0.5).unwrap();
        let values = delta.flatten_all().unwrap().to_vec1::<f32>().unwrap();

        assert_eq!(values, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }
}
