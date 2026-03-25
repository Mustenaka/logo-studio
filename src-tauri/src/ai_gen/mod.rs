/// AI image generation module — Stable Diffusion + LoRA via candle.
///
/// Structure:
///   device         — GPU/CPU detection
///   model_registry — built-in model catalog
///   downloader     — model file download with progress events
///   pipeline        — SD 1.5 / SDXL inference (CLIP → UNet → VAE)
pub mod device;
pub mod downloader;
pub mod model_registry;
pub mod pipeline;
