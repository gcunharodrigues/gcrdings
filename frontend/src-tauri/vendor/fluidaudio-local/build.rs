mod build_support;

use build_support::{
    require_local_adhoc_source, stage_canonical_archive, validate_canonical_archive,
    EXPECTED_FLUIDAUDIO_SHA256,
};
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=swift/");
    println!("cargo:rerun-if-changed=Package.swift");
    println!("cargo:rerun-if-changed=Package.resolved");
    println!("cargo:rerun-if-env-changed=GCRDINGS_LOCAL_ADHOC");
    println!("cargo:rerun-if-env-changed=GCRDINGS_FLUIDAUDIO_STATIC_LIBRARY");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is required"));
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required"));
    let swift_build_dir = out_dir.join("swift-build");
    std::fs::create_dir_all(&swift_build_dir).expect("failed to create Swift build directory");

    if std::env::var_os("GCRDINGS_LOCAL_ADHOC").is_some() {
        let source =
            require_local_adhoc_source(std::env::var_os("GCRDINGS_FLUIDAUDIO_STATIC_LIBRARY"))
                .unwrap_or_else(|error| panic!("{error}"));
        let release_dir = swift_build_dir.join("release");
        std::fs::create_dir_all(&release_dir).expect("failed to create canonical link directory");
        let staged_archive = release_dir.join("libFluidAudioLocalBridge.a");
        match std::fs::symlink_metadata(&staged_archive) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                panic!("canonical FluidAudio staging path is not a file")
            }
            Ok(_) => std::fs::remove_file(&staged_archive)
                .expect("failed to replace canonical FluidAudio input"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => panic!("failed to inspect canonical FluidAudio staging path"),
        }
        let private_stage = out_dir.join(format!(".canonical-fluidaudio-{}.a", std::process::id()));
        stage_canonical_archive(&source, &private_stage).unwrap_or_else(|error| panic!("{error}"));
        validate_canonical_archive(&private_stage, EXPECTED_FLUIDAUDIO_SHA256)
            .unwrap_or_else(|error| panic!("{error}"));
        std::fs::rename(&private_stage, &staged_archive)
            .expect("failed to publish staged canonical FluidAudio input");
        validate_canonical_archive(&staged_archive, EXPECTED_FLUIDAUDIO_SHA256)
            .unwrap_or_else(|error| panic!("{error}"));
    } else {
        let status = Command::new("swift")
            .args([
                "build",
                "-c",
                "release",
                "--build-path",
                swift_build_dir
                    .to_str()
                    .expect("Swift build path must be UTF-8"),
            ])
            .current_dir(manifest_dir)
            .status()
            .expect("failed to run swift build");
        assert!(status.success(), "Swift bridge build failed");
    }

    println!(
        "cargo:rustc-link-search=native={}",
        swift_build_dir.join("release").display()
    );
    println!("cargo:rustc-link-lib=static=FluidAudioLocalBridge");
    for framework in [
        "Foundation",
        "AVFoundation",
        "CoreML",
        "Accelerate",
        "Metal",
        "MetalPerformanceShaders",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!("cargo:rustc-link-lib=dylib=swiftCore");
    println!("cargo:rustc-link-lib=c++");
}
