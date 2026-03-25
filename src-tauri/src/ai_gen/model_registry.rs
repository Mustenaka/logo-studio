/// Built-in model catalog for logo AI generation.
///
/// All repos listed here are verified publicly accessible (no HF token required
/// for the base weights). LoRA files that require a token are clearly noted.
///
/// File layout on disk (per model_id):
///   tokenizer/tokenizer.json
///   text_encoder/model.safetensors
///   unet/diffusion_pytorch_model.safetensors
///   vae/diffusion_pytorch_model.safetensors
///   lora.safetensors   (optional)

use serde::Serialize;

// ── Base model family ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelBase {
    /// Stable Diffusion 1.5 — lighter, faster on CPU, 512×512 native
    Sd15,
    /// Stable Diffusion XL — higher quality, 1024×1024 native (future)
    SdXl,
}

// ── LoRA specification ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoraSpec {
    /// HuggingFace repo hosting the LoRA file
    pub hf_repo: &'static str,
    /// Filename within that repo
    pub filename: &'static str,
    /// Blending scale (0.0 = ignore, 1.0 = full strength; 0.7–0.9 typical)
    pub scale: f32,
    /// Token to prepend to user prompt to activate the LoRA style
    pub trigger_word: Option<&'static str>,
    /// True if downloading this LoRA requires a HuggingFace access token
    pub requires_token: bool,
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
    /// True if the base model itself requires a HF access token to download
    pub requires_token: bool,
    pub lora: Option<LoraSpec>,
    /// Approximate total download size in MB
    pub size_mb: u32,
    /// Minimum system RAM recommended for CPU inference
    pub min_ram_mb: u32,
    pub default_steps_cpu: u32,
    pub default_steps_gpu: u32,
    pub default_guidance: f32,
    /// Maximum supported image dimension
    pub max_resolution: u32,
    /// Ready-to-use prompt shown in the UI placeholder
    pub example_prompt: &'static str,
}

// ── Catalog ───────────────────────────────────────────────────────────────────

pub fn catalog() -> &'static [ModelDef] {
    CATALOG
}

pub fn find(id: &str) -> Option<&'static ModelDef> {
    CATALOG.iter().find(|m| m.id == id)
}

static CATALOG: &[ModelDef] = &[
    // ── SD 1.5 base ───────────────────────────────────────────────────────────
    // Repo: stable-diffusion-v1-5/stable-diffusion-v1-5 (public, no token needed)
    // Replaces the defunct runwayml/stable-diffusion-v1-5 repo.
    ModelDef {
        id: "sd15-base",
        name: "Stable Diffusion 1.5",
        description: "通用 SD 1.5 基础模型，无 LoRA。适合各种风格的 logo 生成，CPU 友好。",
        base: ModelBase::Sd15,
        hf_repo: "stable-diffusion-v1-5/stable-diffusion-v1-5",
        requires_token: false,
        lora: None,
        size_mb: 4200,
        min_ram_mb: 5000,
        default_steps_cpu: 20,
        default_steps_gpu: 30,
        default_guidance: 7.5,
        max_resolution: 512,
        example_prompt: "minimalist logo, geometric shape, flat design, blue gradient, white background, vector style",
    },

    // ── SD 1.5 + LogoRedmond (SD 1.5 版本 LoRA) ───────────────────────────────
    // Base: same public SD 1.5 repo
    // LoRA: artificialguybr/LogoRedmond-LogoLoraForSDXL-V2 contains an SDXL LoRA;
    //       for SD 1.5 we use the trigger word pattern without LoRA until a
    //       verified SD 1.5 logo LoRA is confirmed. Replace lora field when ready.
    ModelDef {
        id: "sd15-dreamshaper",
        name: "DreamShaper 8",
        description: "DreamShaper 8 微调模型，生成质量更好，适合创意 logo 和插图风格。",
        base: ModelBase::Sd15,
        hf_repo: "Lykon/dreamshaper-8",
        requires_token: false,
        lora: None,
        size_mb: 4200,
        min_ram_mb: 5000,
        default_steps_cpu: 20,
        default_steps_gpu: 30,
        default_guidance: 8.0,
        max_resolution: 512,
        example_prompt: "professional logo design, minimalist icon, brand identity, flat vector, clean lines, white background",
    },

    // ── SDXL Turbo ────────────────────────────────────────────────────────────
    // Public, no token required. License: sai-nc-community (non-commercial free).
    // Note: SDXL uses dual CLIP encoders — pipeline support is work-in-progress.
    ModelDef {
        id: "sdxl-turbo",
        name: "SDXL Turbo",
        description: "高质量极速出图（4步），非商业免费。⚠ 双 CLIP 管线，当前为实验性支持。",
        base: ModelBase::SdXl,
        hf_repo: "stabilityai/sdxl-turbo",
        requires_token: false,
        lora: None,
        size_mb: 6800,
        min_ram_mb: 10000,
        default_steps_cpu: 4,
        default_steps_gpu: 4,
        default_guidance: 0.0,
        max_resolution: 512,
        example_prompt: "minimalist logo, abstract letter, modern gradient blue, clean white background",
    },

    // ── SDXL + LogoRedmond V2 ─────────────────────────────────────────────────
    // Base: stabilityai/sdxl-turbo (public)
    // LoRA: artificialguybr/LogoRedmond-LogoLoraForSDXL-V2 (public, 171 MB)
    //       Verified filename: LogoRedmondV2-Logo-LogoRedmAF.safetensors
    ModelDef {
        id: "sdxl-logo-redmond",
        name: "SDXL · Logo Redmond V2",
        description: "SDXL + LogoRedmond v2 LoRA，专为品牌 logo 训练，扁平矢量风格。",
        base: ModelBase::SdXl,
        hf_repo: "stabilityai/sdxl-turbo",
        requires_token: false,
        lora: Some(LoraSpec {
            hf_repo: "artificialguybr/LogoRedmond-LogoLoraForSDXL-V2",
            filename: "LogoRedmondV2-Logo-LogoRedmAF.safetensors",
            scale: 0.85,
            trigger_word: Some("LogoRedmond"),
            requires_token: false,
        }),
        size_mb: 7000,
        min_ram_mb: 10000,
        default_steps_cpu: 4,
        default_steps_gpu: 4,
        default_guidance: 0.0,
        max_resolution: 512,
        example_prompt: "LogoRedmond, minimalist tech startup logo, flat design, blue and white, white background",
    },
];
