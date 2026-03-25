/// Device detection: prefer GPU (CUDA / Metal) over CPU.
///
/// Priority: CUDA (NVIDIA) → Metal (Apple) → CPU
///
/// Compile-time features:
///   `cuda`  — enables NVIDIA GPU via CUDA Toolkit (Windows / Linux)
///   `metal` — enables Apple Metal GPU (macOS only)
///
/// Even when neither feature is compiled in, `detect_device` will check for
/// common GPU indicators at runtime and set `gpu_available_but_disabled = true`
/// so the frontend can hint the user to enable the appropriate feature.
use candle_core::Device;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// "cpu" | "cuda" | "metal"
    pub kind: String,
    /// Human-readable name shown in the UI badge
    pub name: String,
    /// Approximate VRAM in MB (None for CPU or when query is unsupported)
    pub vram_mb: Option<u64>,
    /// True when running on an accelerated device (GPU)
    pub is_accelerated: bool,
    /// True when a GPU was detected at runtime but the matching compile-time
    /// feature (`cuda` / `metal`) is not enabled in this build.
    /// The frontend uses this to show a "GPU available — enable GPU build" hint.
    pub gpu_available_but_disabled: bool,
}

/// Detect the best available compute device.
///
/// Returns the candle `Device` together with human-readable `DeviceInfo`.
pub fn detect_device() -> (Device, DeviceInfo) {
    // ── NVIDIA CUDA ───────────────────────────────────────────────────────────
    #[cfg(feature = "cuda")]
    {
        if candle_core::utils::cuda_is_available() {
            match Device::new_cuda(0) {
                Ok(dev) => {
                    eprintln!("[AI-GEN] Using CUDA device");
                    return (dev, DeviceInfo {
                        kind: "cuda".into(),
                        name: "CUDA GPU".into(),
                        vram_mb: None,
                        is_accelerated: true,
                        gpu_available_but_disabled: false,
                    });
                }
                Err(e) => {
                    eprintln!("[AI-GEN] CUDA available but init failed: {e} — falling back");
                }
            }
        }
    }

    // ── Apple Metal ───────────────────────────────────────────────────────────
    #[cfg(all(feature = "metal", target_os = "macos"))]
    {
        match Device::new_metal(0) {
            Ok(dev) => {
                eprintln!("[AI-GEN] Using Metal device");
                return (dev, DeviceInfo {
                    kind: "metal".into(),
                    name: "Apple Metal GPU".into(),
                    vram_mb: None,
                    is_accelerated: true,
                    gpu_available_but_disabled: false,
                });
            }
            Err(e) => {
                eprintln!("[AI-GEN] Metal init failed: {e} — falling back to CPU");
            }
        }
    }

    // ── CPU fallback ──────────────────────────────────────────────────────────
    let cpu_name = std::env::var("PROCESSOR_IDENTIFIER")
        .or_else(|_| std::env::var("PROCESSOR_BRAND_STRING"))
        .unwrap_or_else(|_| "CPU".into());

    let gpu_hint = gpu_present_but_feature_missing();

    eprintln!("[AI-GEN] Using CPU device (gpu_hint={})", gpu_hint);
    (Device::Cpu, DeviceInfo {
        kind: "cpu".into(),
        name: cpu_name,
        vram_mb: None,
        is_accelerated: false,
        gpu_available_but_disabled: gpu_hint,
    })
}

/// Returns `true` when a GPU is likely present on this machine but the
/// required compile-time feature (`cuda` / `metal`) is not enabled.
///
/// Uses only lightweight env-var / filesystem checks — no DLL loading.
fn gpu_present_but_feature_missing() -> bool {
    // ── CUDA not compiled in — check for NVIDIA GPU ───────────────────────
    #[cfg(not(feature = "cuda"))]
    {
        // CUDA installer sets CUDA_PATH / CUDA_HOME on Windows and Linux
        if std::env::var("CUDA_PATH").is_ok() || std::env::var("CUDA_HOME").is_ok() {
            return true;
        }

        // Common Windows CUDA runtime DLL location
        #[cfg(target_os = "windows")]
        if std::path::Path::new(r"C:\Windows\System32\nvcuda.dll").exists() {
            return true;
        }

        // Common Linux CUDA library path
        #[cfg(target_os = "linux")]
        if std::path::Path::new("/usr/local/cuda/lib64/libcudart.so").exists()
            || std::path::Path::new("/usr/lib/x86_64-linux-gnu/libcuda.so.1").exists()
        {
            return true;
        }
    }

    // ── Metal not compiled in — all modern macOS supports Metal ───────────
    #[cfg(all(not(feature = "metal"), target_os = "macos"))]
    {
        return true;
    }

    #[allow(unreachable_code)]
    false
}
