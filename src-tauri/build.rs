fn main() {
    tauri_build::build();
    check_gpu_features();
}

/// Emit cargo warnings to guide developers toward the right GPU build.
///
/// - CUDA feature enabled  → validate CUDA Toolkit is installed
/// - Metal feature enabled → validate we are on macOS
/// - No GPU feature        → detect GPU presence and print the enabling command
fn check_gpu_features() {
    let cuda_on  = std::env::var("CARGO_FEATURE_CUDA").is_ok();
    let metal_on = std::env::var("CARGO_FEATURE_METAL").is_ok();

    if cuda_on {
        if cuda_toolkit_present() {
            println!("cargo:warning=[AI-GEN] ✓ CUDA feature enabled — building with NVIDIA GPU support");

            // On Windows, nvcc requires cl.exe (MSVC C++ compiler) in PATH or via
            // NVCC_CCBIN. Warn early if neither is set so the user gets a clear
            // message instead of a cryptic "Cannot find cl.exe" from nvcc.
            #[cfg(target_os = "windows")]
            {
                let ccbin_set = std::env::var("NVCC_CCBIN").is_ok();
                let cl_in_path = which_cl_exe();
                if !ccbin_set && !cl_in_path {
                    println!(
                        "cargo:warning=[AI-GEN] ⚠ NVCC_CCBIN is not set and cl.exe is not in PATH."
                    );
                    println!(
                        "cargo:warning=[AI-GEN]   nvcc will fail with 'Cannot find compiler cl.exe'."
                    );
                    println!(
                        "cargo:warning=[AI-GEN]   Fix: run  powershell -ExecutionPolicy Bypass -File setup-gpu-env.ps1"
                    );
                    println!(
                        "cargo:warning=[AI-GEN]   This writes NVCC_CCBIN into src-tauri/.cargo/config.toml (once only)."
                    );
                }
            }
        } else {
            println!(
                "cargo:warning=[AI-GEN] ⚠ cuda feature is ON but CUDA Toolkit was not found. \
                 Install from https://developer.nvidia.com/cuda-downloads \
                 or the build will fail at the CUDA linking step."
            );
        }
        return;
    }

    if metal_on {
        #[cfg(not(target_os = "macos"))]
        println!("cargo:warning=[AI-GEN] ⚠ metal feature is ON but target is not macOS — compile will likely fail.");
        #[cfg(target_os = "macos")]
        println!("cargo:warning=[AI-GEN] ✓ Metal feature enabled — building with Apple GPU support");
        return;
    }

    // ── No GPU feature: detect available GPU and suggest the right command ────
    if cuda_toolkit_present() {
        println!(
            "cargo:warning=[AI-GEN] ⚡ NVIDIA GPU / CUDA Toolkit detected but cuda feature is OFF. \
             To enable GPU acceleration run:  npm run tauri:dev:gpu  (dev)  \
             or  npm run tauri:build:gpu  (release)"
        );
    }

    #[cfg(target_os = "macos")]
    println!(
        "cargo:warning=[AI-GEN] ⚡ Apple Metal available but metal feature is OFF. \
         To enable GPU acceleration run:  npm run tauri:dev:metal"
    );
}

/// True when cl.exe can be found on PATH (Windows only).
#[cfg(target_os = "windows")]
fn which_cl_exe() -> bool {
    std::process::Command::new("where")
        .arg("cl.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// True when a CUDA Toolkit installation can be found at compile time.
fn cuda_toolkit_present() -> bool {
    // Environment variables set by CUDA Toolkit installer
    if std::env::var("CUDA_PATH").is_ok() || std::env::var("CUDA_HOME").is_ok() {
        return true;
    }
    // Windows CUDA runtime DLL (present even without CUDA_PATH set)
    #[cfg(target_os = "windows")]
    if std::path::Path::new(r"C:\Windows\System32\nvcuda.dll").exists() {
        return true;
    }
    // Linux CUDA library
    #[cfg(target_os = "linux")]
    if std::path::Path::new("/usr/local/cuda/lib64/libcudart.so").exists()
        || std::path::Path::new("/usr/lib/x86_64-linux-gnu/libcuda.so.1").exists()
    {
        return true;
    }
    false
}
