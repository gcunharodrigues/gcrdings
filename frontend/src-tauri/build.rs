#[path = "build/ffmpeg.rs"]
mod ffmpeg;

fn main() {
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-path", "HEAD"])
        .output()
    {
        if output.status.success() {
            if let Ok(head) = String::from_utf8(output.stdout) {
                println!("cargo:rerun-if-changed={}", head.trim());
            }
        }
    }
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .or_else(|| std::env::var("GCRDINGS_BUILD_COMMIT").ok())
        .expect("a production build must identify its source commit");
    assert!(
        commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "the production source commit must be a full hexadecimal SHA"
    );
    println!("cargo:rustc-env=GCRDINGS_BUILD_COMMIT={commit}");

    // GPU Acceleration Detection and Build Guidance
    detect_and_report_gpu_capabilities();

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

        // Let the enhanced_macos crate handle its own Swift compilation
        // The swift-rs crate build will be handled in the enhanced_macos crate's build.rs
    }

    // Download and bundle FFmpeg binary at build-time
    ffmpeg::ensure_ffmpeg_binary();

    tauri_build::build()
}

/// Detects GPU acceleration capabilities and provides build guidance
fn detect_and_report_gpu_capabilities() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    println!("cargo:warning=🚀 Building gcrdings for: {}", target_os);

    match target_os.as_str() {
        "macos" => {
            println!("cargo:warning=✅ macOS: Metal GPU acceleration ENABLED by default");
            #[cfg(feature = "coreml")]
            println!("cargo:warning=✅ CoreML acceleration ENABLED");
        }
        "windows" => {
            if cfg!(feature = "cuda") {
                println!("cargo:warning=✅ Windows: CUDA GPU acceleration ENABLED");
            } else if cfg!(feature = "vulkan") {
                println!("cargo:warning=✅ Windows: Vulkan GPU acceleration ENABLED");
            } else if cfg!(feature = "openblas") {
                println!("cargo:warning=✅ Windows: OpenBLAS CPU optimization ENABLED");
            } else {
                println!(
                    "cargo:warning=⚠️  Windows: Using CPU-only mode (no GPU or BLAS acceleration)"
                );
                println!("cargo:warning=💡 For NVIDIA GPU: cargo build --release --features cuda");
                println!(
                    "cargo:warning=💡 For AMD/Intel GPU: cargo build --release --features vulkan"
                );
                println!("cargo:warning=💡 For CPU optimization: cargo build --release --features openblas");

                // Try to detect NVIDIA GPU
                if which::which("nvidia-smi").is_ok() {
                    println!("cargo:warning=🎯 NVIDIA GPU detected! Consider rebuilding with --features cuda");
                }
            }
        }
        "linux" => {
            if cfg!(feature = "cuda") {
                println!("cargo:warning=✅ Linux: CUDA GPU acceleration ENABLED");
            } else if cfg!(feature = "vulkan") {
                println!("cargo:warning=✅ Linux: Vulkan GPU acceleration ENABLED");
            } else if cfg!(feature = "hipblas") {
                println!("cargo:warning=✅ Linux: AMD ROCm (HIP) acceleration ENABLED");
            } else if cfg!(feature = "openblas") {
                println!("cargo:warning=✅ Linux: OpenBLAS CPU optimization ENABLED");
            } else {
                println!(
                    "cargo:warning=⚠️  Linux: Using CPU-only mode (no GPU or BLAS acceleration)"
                );
                println!("cargo:warning=💡 For NVIDIA GPU: cargo build --release --features cuda");
                println!("cargo:warning=💡 For AMD GPU: cargo build --release --features hipblas");
                println!(
                    "cargo:warning=💡 For other GPUs: cargo build --release --features vulkan"
                );
                println!("cargo:warning=💡 For CPU optimization: cargo build --release --features openblas");

                // Try to detect NVIDIA GPU
                if which::which("nvidia-smi").is_ok() {
                    println!("cargo:warning=🎯 NVIDIA GPU detected! Consider rebuilding with --features cuda");
                }

                // Try to detect AMD GPU
                if which::which("rocm-smi").is_ok() {
                    println!("cargo:warning=🎯 AMD GPU detected! Consider rebuilding with --features hipblas");
                }
            }
        }
        _ => {
            println!("cargo:warning=ℹ️  Unknown platform: {}", target_os);
        }
    }

    // Performance guidance
    if !cfg!(feature = "cuda")
        && !cfg!(feature = "vulkan")
        && !cfg!(feature = "hipblas")
        && !cfg!(feature = "openblas")
        && target_os != "macos"
    {
        println!("cargo:warning=📊 Performance: CPU-only builds are significantly slower than GPU/BLAS builds");
        println!("cargo:warning=📚 See README.md for GPU/BLAS setup instructions");
    }
}
