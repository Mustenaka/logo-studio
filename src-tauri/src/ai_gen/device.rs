/// Device detection: prefer GPU (CUDA) over CPU.
///
/// At runtime the backend probes for CUDA availability. If the `cuda` cargo
/// feature is enabled and a CUDA-capable device is found, inference runs on
/// the GPU; otherwise it falls back to multi-threaded CPU.
use candle_core::Device;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// "cpu" | "cuda"
    pub kind: String,
    /// Human-readable name for the UI
    pub name: String,
    /// Approximate VRAM in MB (None for CPU or if query unsupported)
    pub vram_mb: Option<u64>,
    /// Estimated speed hint so the frontend can suggest sensible step counts
    pub is_accelerated: bool,
}

/// Detect the best available device and return it together with human-readable info.
///
/// Priority: CUDA (GPU) → CPU
pub fn detect_device() -> (Device, DeviceInfo) {
    // ── CUDA ────────────────────────────────────────────────────────────────
    #[cfg(feature = "cuda")]
    {
        if candle_core::utils::cuda_is_available() {
            match Device::new_cuda(0) {
                Ok(dev) => {
                    let info = DeviceInfo {
                        kind: "cuda".into(),
                        name: "CUDA GPU".into(),
                        vram_mb: None,
                        is_accelerated: true,
                    };
                    eprintln!("[AI-GEN] Using CUDA device");
                    return (dev, info);
                }
                Err(e) => {
                    eprintln!("[AI-GEN] CUDA available but init failed: {e} — falling back to CPU");
                }
            }
        }
    }

    // ── CPU fallback ─────────────────────────────────────────────────────────
    let cpu_name = std::env::var("PROCESSOR_IDENTIFIER")
        .or_else(|_| std::env::var("PROCESSOR_BRAND_STRING"))
        .unwrap_or_else(|_| {
            // On Linux/macOS we could read /proc/cpuinfo but keep it simple
            "CPU".into()
        });

    let info = DeviceInfo {
        kind: "cpu".into(),
        name: cpu_name,
        vram_mb: None,
        is_accelerated: false,
    };
    eprintln!("[AI-GEN] Using CPU device");
    (Device::Cpu, info)
}
