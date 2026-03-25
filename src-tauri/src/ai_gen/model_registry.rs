/// Built-in model catalog for logo AI generation.
///
/// Each entry describes:
///   - which HuggingFace base model to download (SD 1.5 or SDXL)
///   - an optional LoRA weight file that specialises the model for logos
///   - sensible defaults for CPU vs GPU inference
///
/// The `id` field is the stable key used everywhere (filesystem, Tauri events,
/// frontend state). Never change an existing id once shipped.

use serde::Serialize;

// ── Base model family ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelBase {
    /// Stable Diffusion 1.5 — lighter, faster on CPU, 512×512
    Sd15,
    /// Stable Diffusion XL — higher quality, needs ≥8 GB RAM
    SdXl,
}

// ── LoRA specification ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoraSpec {
    /// HuggingFace repo that hosts the LoRA file, e.g. "artificialguybr/LogoRedmondV2"
    pub hf_repo: &'static str,
    /// Filename within that repo, e.g. "LogoRedmondV2-LogoRedmondV2.safetensors"
    pub filename: &'static str,
    /// Blending scale: 0.0 = ignore LoRA, 1.0 = full strength (0.7–0.9 typical)
    pub scale: f32,
    /// Token that activates the LoRA's style — prepend to user prompt when set
    pub trigger_word: Option<&'static str>,
}

// ── Model definition ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub base: ModelBase,
    /// HuggingFace repo for the base model weights
    pub hf_repo: &'static str,
    pub lora: Option<LoraSpec>,
    /// Approximate download size in MB (base + LoRA if applicable)
    pub size_mb: u32,
    /// Minimum system RAM required for inference (not VRAM — CPU mode)
    pub min_ram_mb: u32,
    pub default_steps_cpu: u32,
    pub default_steps_gpu: u32,
    pub default_guidance: f32,
    /// Maximum supported image dimension (SD 1.5 → 512, SDXL → 1024)
    pub max_resolution: u32,
    /// Ready-to-use example prompt shown in the UI placeholder
    pub example_prompt: &'static str,
}

// ── Catalog ───────────────────────────────────────────────────────────────────

/// All built-in models, in display order.
pub fn catalog() -> &'static [ModelDef] {
    CATALOG
}

/// Look up a model by its stable id.
pub fn find(id: &str) -> Option<&'static ModelDef> {
    CATALOG.iter().find(|m| m.id == id)
}

static CATALOG: &[ModelDef] = &[
    // ── SD 1.5 + LogoRedmond v2 ───────────────────────────────────────────────
    ModelDef {
        id: "sd15-logo-redmond",
        name: "Logo Redmond",
        description: "扁平矢量风格，适合科技/品牌 logo。LogoRedmond v2 微调，支持触发词。",
        base: ModelBase::Sd15,
        hf_repo: "runwayml/stable-diffusion-v1-5",
        lora: Some(LoraSpec {
            hf_repo: "artificialguybr/LogoRedmondV2",
            filename: "LogoRedmondV2-LogoRedmondV2.safetensors",
            scale: 0.85,
            trigger_word: Some("LogoRedmond"),
        }),
        size_mb: 4200,
        min_ram_mb: 5000,
        default_steps_cpu: 20,
        default_steps_gpu: 30,
        default_guidance: 7.5,
        max_resolution: 512,
        example_prompt: "LogoRedmond, minimalist tech startup logo, flat design, blue and white, white background",
    },
    // ── SD 1.5 + Logo Diffusion ───────────────────────────────────────────────
    ModelDef {
        id: "sd15-logo-diffusion",
        name: "Logo Diffusion",
        description: "多样化风格 logo，专业 logo 数据集微调，适合多种行业。",
        base: ModelBase::Sd15,
        hf_repo: "runwayml/stable-diffusion-v1-5",
        lora: Some(LoraSpec {
            hf_repo: "Sela/logo-diffusion",
            filename: "logo-diffusion.safetensors",
            scale: 0.9,
            trigger_word: Some("logo design"),
        }),
        size_mb: 4100,
        min_ram_mb: 5000,
        default_steps_cpu: 20,
        default_steps_gpu: 30,
        default_guidance: 8.0,
        max_resolution: 512,
        example_prompt: "logo design, mountain outdoor brand, green color scheme, vector style, white background",
    },
    // ── SDXL Turbo ────────────────────────────────────────────────────────────
    ModelDef {
        id: "sdxl-turbo",
        name: "SDXL Turbo",
        description: "高质量极速出图（4步），推荐用于高分辨率 logo 和快速预览。",
        base: ModelBase::SdXl,
        hf_repo: "stabilityai/sdxl-turbo",
        lora: None,
        size_mb: 6800,
        min_ram_mb: 8000,
        default_steps_cpu: 4,
        default_steps_gpu: 4,
        // SDXL Turbo is a distilled model — guidance > 0 degrades quality
        default_guidance: 0.0,
        max_resolution: 512,
        example_prompt: "minimalist logo, letter A, modern gradient blue, clean white background, vector style",
    },
];
