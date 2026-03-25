/// SAM2 ONNX inference engine.
///
/// Models expected in <exe_dir>/models/ (or SAM2_MODELS_DIR env var):
///   encoder.onnx  –  image encoder (Hiera ViT)
///   decoder.onnx  –  prompt-based mask decoder
///
/// Tensor names from sam2-onnx-cpp export:
///   Encoder  in : "image"              [1, 3, 1024, 1024]
///   Encoder out : "image_embeddings"   [1, 256, 64, 64]
///              + "high_res_features1"  [1, 32, 256, 256]
///              + "high_res_features2"  [1, 64, 128, 128]
///   Decoder  in : "image_embed"        [1, 256, 64, 64]
///              + "high_res_feats_0"    [1, 32, 256, 256]
///              + "high_res_feats_1"    [1, 64, 128, 128]
///              + "point_coords"        [1, N, 2]   float32
///              + "point_labels"        [1, N]      float32  (1=fg, 0=bg)
///   Decoder out : "mask_for_mem"       [1, M, 1024, 1024]  sigmoid
///              + "pred_mask"           [1, M, 256, 256]    logits (fallback)

use image::{DynamicImage, GenericImageView};
use ndarray::{Array2, Array3, Array4, ArrayD};
use ort::{session::Session, session::builder::GraphOptimizationLevel, value::Tensor};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// ── Tensor name constants ────────────────────────────────────────────────────

const ENC_IN_IMAGE: &str = "input"; // export/src/utils.py: input_names=["input"]
const ENC_OUT_EMBED: &str = "image_embeddings";
const ENC_OUT_HR1: &str = "high_res_features1";
const ENC_OUT_HR2: &str = "high_res_features2";

const DEC_IN_EMBED: &str = "image_embed";
const DEC_IN_HR0: &str = "high_res_feats_0";
const DEC_IN_HR1: &str = "high_res_feats_1";
const DEC_IN_COORDS: &str = "point_coords";
const DEC_IN_LABELS: &str = "point_labels";
const DEC_OUT_MASK_HIRES: &str = "mask_for_mem";
const DEC_OUT_MASK_LORES: &str = "pred_mask";

// ── ImageNet normalisation ───────────────────────────────────────────────────

const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const SAM2_SIZE: u32 = 1024;

// ── Internal types ───────────────────────────────────────────────────────────

#[allow(dead_code)]
struct ImageEncoding {
    embeddings: ArrayD<f32>,
    high_res_0: ArrayD<f32>,
    high_res_1: ArrayD<f32>,
    orig_width: u32,
    orig_height: u32,
}

struct EncoderCache {
    image_hash: u64,
    encoding: ImageEncoding,
}

// ── Global state (lazy-initialised) ─────────────────────────────────────────

static ENCODER: OnceLock<Mutex<Session>> = OnceLock::new();
static DECODER: OnceLock<Mutex<Session>> = OnceLock::new();
static CACHE: OnceLock<Mutex<Option<EncoderCache>>> = OnceLock::new();

// ── Model file name candidates ───────────────────────────────────────────────

const ENCODER_NAMES: &[&str] = &["encoder.onnx", "sam2_encoder.onnx", "image_encoder.onnx"];
const DECODER_NAMES: &[&str] = &["decoder.onnx", "sam2_decoder.onnx", "image_decoder.onnx"];

// ── Public API ───────────────────────────────────────────────────────────────

/// True if both model files exist on disk.
pub fn is_available() -> bool {
    let dir = models_dir();
    find_model(&dir, ENCODER_NAMES).is_some() && find_model(&dir, DECODER_NAMES).is_some()
}

/// Run SAM2 segmentation.
///
/// `points`: (canvas_x, canvas_y, label) in 0..1024 canvas space normalised to
/// the full original image. Returns GrayImage: 255 = foreground, 0 = background.
pub fn run_sam2(
    img: &DynamicImage,
    points: &[(f32, f32, i32)],
    threshold: f32,
) -> Result<image::GrayImage, String> {
    ensure_loaded()?;

    let (orig_w, orig_h) = img.dimensions();
    let img_hash = hash_image(img);

    // ── Encoder (cached per image) ──────────────────────────────────────────
    let needs_encode = {
        let cache = CACHE.get().unwrap().lock().map_err(|_| "Cache lock poisoned")?;
        cache.as_ref().map_or(true, |c| c.image_hash != img_hash)
    };

    if needs_encode {
        let array = preprocess_image(img);
        let enc_input = Tensor::<f32>::from_array(array)
            .map_err(|e| format!("Create encoder tensor: {e}"))?;

        // Inner block: `enc_out` borrows from `encoder` (MutexGuard).
        // Both are dropped at the end of this block, releasing the lock.
        let (embeddings, high_res_0, high_res_1) = {
            let mut encoder = ENCODER.get().unwrap().lock().map_err(|_| "Encoder lock poisoned")?;
            let enc_out = encoder
                .run(ort::inputs![ENC_IN_IMAGE => enc_input])
                .map_err(|e| format!("Encoder inference: {e}"))?;

            let emb = enc_out[ENC_OUT_EMBED]
                .try_extract_array::<f32>()
                .map_err(|e| format!("Extract {ENC_OUT_EMBED}: {e}"))?
                .to_owned();
            let hr0 = enc_out[ENC_OUT_HR1]
                .try_extract_array::<f32>()
                .map_err(|e| format!("Extract {ENC_OUT_HR1}: {e}"))?
                .to_owned();
            let hr1 = enc_out[ENC_OUT_HR2]
                .try_extract_array::<f32>()
                .map_err(|e| format!("Extract {ENC_OUT_HR2}: {e}"))?
                .to_owned();
            (emb, hr0, hr1)
        }; // encoder lock released here

        let mut cache = CACHE.get().unwrap().lock().map_err(|_| "Cache lock poisoned")?;
        *cache = Some(EncoderCache {
            image_hash: img_hash,
            encoding: ImageEncoding {
                embeddings,
                high_res_0,
                high_res_1,
                orig_width: orig_w,
                orig_height: orig_h,
            },
        });
    }

    // Clone cached tensors to release the lock before running the decoder.
    let (embeddings, high_res_0, high_res_1) = {
        let cache = CACHE.get().unwrap().lock().map_err(|_| "Cache lock poisoned")?;
        let enc = &cache.as_ref().unwrap().encoding;
        (enc.embeddings.clone(), enc.high_res_0.clone(), enc.high_res_1.clone())
    };

    // ── Decoder ─────────────────────────────────────────────────────────────
    let (point_coords, point_labels) = make_point_tensors(points, orig_w, orig_h);

    // `from_array` requires owned Array<T,D> – all variables here are owned.
    let embed_t  = Tensor::<f32>::from_array(embeddings).map_err(|e| format!("{e}"))?;
    let hr0_t    = Tensor::<f32>::from_array(high_res_0).map_err(|e| format!("{e}"))?;
    let hr1_t    = Tensor::<f32>::from_array(high_res_1).map_err(|e| format!("{e}"))?;
    let coords_t = Tensor::<f32>::from_array(point_coords).map_err(|e| format!("{e}"))?;
    let labels_t = Tensor::<f32>::from_array(point_labels).map_err(|e| format!("{e}"))?;

    // Inner block: `dec_out` borrows from `decoder`. Both dropped at block end.
    let (flat, mask_h, mask_w, threshold): (Vec<f32>, usize, usize, f32) = {
        let mut decoder = DECODER.get().unwrap().lock().map_err(|_| "Decoder lock poisoned")?;
        let dec_out = decoder
            .run(ort::inputs![
                DEC_IN_EMBED  => embed_t,
                DEC_IN_HR0    => hr0_t,
                DEC_IN_HR1    => hr1_t,
                DEC_IN_COORDS => coords_t,
                DEC_IN_LABELS => labels_t,
            ])
            .map_err(|e| format!("Decoder inference: {e}"))?;

        // For single-image segmentation, use pred_mask (raw logits, output index 2).
        // Python reference: `mask255 = (pred_low[0,0] > 0)` — threshold = 0.0.
        //
        // mask_for_mem (output index 1) is the sigmoid-scaled mask for the video
        // memory encoder.  It has different semantics and must NOT be used here.
        //
        // The user-facing `threshold` parameter (default 0.35) is converted to a
        // logit: logit_thresh = ln(t / (1-t)).
        //   t=0.50 → logit=0.000  (standard SAM threshold — balanced)
        //   t=0.35 → logit=-0.619 (more inclusive — keeps uncertain edge pixels)
        //   t=0.25 → logit=-1.099 (very inclusive — keeps shadows/gradients too)
        let logit_thresh = (threshold / (1.0 - threshold)).ln();

        match dec_out[DEC_OUT_MASK_LORES].try_extract_array::<f32>() {
            Ok(view) => {
                let sh = view.shape().to_vec();
                let h = sh[sh.len() - 2];
                let w = sh[sh.len() - 1];
                (view.iter().copied().collect(), h, w, logit_thresh)
            }
            Err(_) => {
                // Fallback to mask_for_mem (sigmoid) if pred_mask not available
                let view = dec_out[DEC_OUT_MASK_HIRES]
                    .try_extract_array::<f32>()
                    .map_err(|e| format!("Extract {DEC_OUT_MASK_HIRES}: {e}"))?;
                let sh = view.shape().to_vec();
                let h = sh[sh.len() - 2];
                let w = sh[sh.len() - 1];
                (view.iter().copied().collect(), h, w, threshold)
            }
        }
    }; // decoder lock released here

    // Build GrayImage from first mask (batch=0, mask_idx=0).
    // pred_mask shape is [1, M, H, W]; flat index [0,0,y,x] = y*W + x.
    let mut gray = image::GrayImage::new(mask_w as u32, mask_h as u32);
    for y in 0..mask_h {
        for x in 0..mask_w {
            let val = flat[y * mask_w + x];
            gray.put_pixel(x as u32, y as u32,
                image::Luma([if val > threshold { 255u8 } else { 0u8 }]));
        }
    }

    // pred_mask is 256×256 (quarter of the 1024 encoder resolution).
    // The valid region is proportional to how much of the 1024 canvas the original
    // image occupied (longest-side-resize → letterbox padding on the other axis).
    //
    // valid_w/h in 256-space = floor(orig_dim * (256 / 1024) * scale)
    //                        = floor(orig_dim * scale / 4)
    // where scale = 1024 / max(orig_w, orig_h).
    let scale = SAM2_SIZE as f32 / orig_w.max(orig_h) as f32;
    let valid_w = ((orig_w as f32 * scale) / (SAM2_SIZE as f32 / mask_w as f32)).round() as u32;
    let valid_h = ((orig_h as f32 * scale) / (SAM2_SIZE as f32 / mask_h as f32)).round() as u32;
    let crop_w = valid_w.min(mask_w as u32);
    let crop_h = valid_h.min(mask_h as u32);

    let final_mask = image::DynamicImage::ImageLuma8(gray)
        .crop_imm(0, 0, crop_w, crop_h)
        .resize_exact(orig_w, orig_h, image::imageops::FilterType::Lanczos3)
        .into_luma8();

    Ok(final_mask)
}

// ── Private helpers ──────────────────────────────────────────────────────────

fn models_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SAM2_MODELS_DIR") {
        return PathBuf::from(dir);
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    // 1. Production: <exe_dir>/models/
    let prod = exe_dir.join("models");
    if prod.exists() {
        return prod;
    }
    // 2. Dev (cargo run): target/debug/ → ../../models/ = src-tauri/models/
    let dev = exe_dir.join("../../models");
    if dev.exists() {
        return dev;
    }
    prod // fallback to production path even if missing (will produce clear error)
}

fn find_model(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists())
}

fn ensure_loaded() -> Result<(), String> {
    if ENCODER.get().is_some() {
        return Ok(());
    }

    let dir = models_dir();

    let enc_path = find_model(&dir, ENCODER_NAMES).ok_or_else(|| {
        format!(
            "SAM2 encoder not found in {}. Expected one of: {}",
            dir.display(),
            ENCODER_NAMES.join(", ")
        )
    })?;
    let dec_path = find_model(&dir, DECODER_NAMES).ok_or_else(|| {
        format!(
            "SAM2 decoder not found in {}. Expected one of: {}",
            dir.display(),
            DECODER_NAMES.join(", ")
        )
    })?;

    // Encoder: extended optimisations are safe (static shapes throughout).
    let encoder = Session::builder()
        .map_err(|e| format!("ORT builder: {e}"))?
        .with_optimization_level(GraphOptimizationLevel::Level2)
        .map_err(|e| format!("ORT enc opt: {e}"))?
        .commit_from_file(&enc_path)
        .map_err(|e| format!("Load encoder: {e}"))?;

    // Decoder: DISABLE ALL graph optimisations.
    // The FusedGemmTransposeFusion that ORT applies at Extended/All levels
    // bakes in the traced point-count (K=2) and breaks for any other count.
    // Python reference uses ORT_DISABLE_ALL + disable_gemm_fast_gelu_fusion.
    let decoder = Session::builder()
        .map_err(|e| format!("ORT builder: {e}"))?
        .with_optimization_level(GraphOptimizationLevel::Disable)
        .map_err(|e| format!("ORT dec opt: {e}"))?
        .with_config_entry("session.disable_gemm_fast_gelu_fusion", "1")
        .map_err(|e| format!("ORT dec cfg: {e}"))?
        .commit_from_file(&dec_path)
        .map_err(|e| format!("Load decoder: {e}"))?;

    let _ = ENCODER.set(Mutex::new(encoder));
    let _ = DECODER.set(Mutex::new(decoder));
    let _ = CACHE.set(Mutex::new(None));

    Ok(())
}

/// Resize longest side to 1024, pad remainder with zeros, normalise (ImageNet stats).
/// Returns owned NCHW Array4<f32> [1, 3, 1024, 1024].
fn preprocess_image(img: &DynamicImage) -> Array4<f32> {
    let (orig_w, orig_h) = img.dimensions();
    let scale = SAM2_SIZE as f32 / orig_w.max(orig_h) as f32;
    let new_w = (orig_w as f32 * scale).round() as u32;
    let new_h = (orig_h as f32 * scale).round() as u32;

    // Convert RGBA → RGB: replace transparent pixels with ImageNet mean colour
    // (neutral grey ≈ 0.485, 0.456, 0.406 in [0,1]) so SAM2 treats them as
    // uninteresting background rather than hard black.
    let rgba_resized = img
        .resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3)
        .to_rgba8();

    let neutral_r = (MEAN[0] * 255.0) as u8;
    let neutral_g = (MEAN[1] * 255.0) as u8;
    let neutral_b = (MEAN[2] * 255.0) as u8;

    let mut tensor = Array4::<f32>::zeros([1, 3, SAM2_SIZE as usize, SAM2_SIZE as usize]);
    for y in 0..new_h as usize {
        for x in 0..new_w as usize {
            let p = rgba_resized.get_pixel(x as u32, y as u32);
            let (r, g, b) = if p[3] < 30 {
                (neutral_r, neutral_g, neutral_b)
            } else {
                (p[0], p[1], p[2])
            };
            tensor[[0, 0, y, x]] = (r as f32 / 255.0 - MEAN[0]) / STD[0];
            tensor[[0, 1, y, x]] = (g as f32 / 255.0 - MEAN[1]) / STD[1];
            tensor[[0, 2, y, x]] = (b as f32 / 255.0 - MEAN[2]) / STD[2];
        }
    }
    tensor
}

/// Build point_coords [1, N, 2] (f32) and point_labels [1, N] (f32).
/// Canvas coords are 0..1024 normalised to the original image dimensions.
/// Note: export/src/utils.py exports point_labels as float32, not int64.
fn make_point_tensors(
    points: &[(f32, f32, i32)],
    orig_w: u32,
    orig_h: u32,
) -> (Array3<f32>, Array2<f32>) {
    let scale = SAM2_SIZE as f32 / orig_w.max(orig_h) as f32;
    let n = points.len();
    let mut coords = Array3::<f32>::zeros([1, n, 2]);
    let mut labels = Array2::<f32>::zeros([1, n]);

    for (i, &(cx, cy, label)) in points.iter().enumerate() {
        let orig_x = cx / SAM2_SIZE as f32 * orig_w as f32;
        let orig_y = cy / SAM2_SIZE as f32 * orig_h as f32;
        coords[[0, i, 0]] = orig_x * scale;
        coords[[0, i, 1]] = orig_y * scale;
        labels[[0, i]] = label as f32;
    }

    (coords, labels)
}

fn hash_image(img: &DynamicImage) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    img.as_bytes().hash(&mut h);
    h.finish()
}
