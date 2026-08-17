use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

const APPROVED_AARCH64_APPLE_DARWIN_SHA256: &str =
    "77d2c853f431318d55ec02676d9b2f185ebfdddb9f7677a251fbe453affe025a";

pub fn ensure_ffmpeg_binary() {
    let target = std::env::var("TARGET")
        .or_else(|_| std::env::var("HOST"))
        .expect("FFmpeg prerequisite failed: build_target_unavailable");
    let expected_digest = expected_ffmpeg_sha256(&target)
        .expect("FFmpeg prerequisite failed: unsupported_build_target");
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR")
            .expect("FFmpeg prerequisite failed: manifest_directory_unavailable"),
    );
    let destination = manifest_dir
        .join("binaries")
        .join(format!("ffmpeg-{target}"));
    let source = std::env::var_os("GCRDINGS_FFMPEG_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| destination.clone());

    println!("cargo:rerun-if-env-changed=GCRDINGS_FFMPEG_SOURCE");
    println!("cargo:rerun-if-changed={}", source.display());

    verify_local_ffmpeg(&source, expected_digest)
        .expect("FFmpeg prerequisite failed: local_binary_missing_or_invalid");
    if source != destination {
        stage_verified_binary(&source, &destination, expected_digest)
            .expect("FFmpeg prerequisite failed: local_binary_staging_failed");
    }
}

fn expected_ffmpeg_sha256(target: &str) -> Option<&'static str> {
    match target {
        "aarch64-apple-darwin" => Some(APPROVED_AARCH64_APPLE_DARWIN_SHA256),
        _ => None,
    }
}

fn stage_verified_binary(
    source: &Path,
    destination: &Path,
    expected_digest: &str,
) -> Result<(), ()> {
    let parent = destination.parent().ok_or(())?;
    fs::create_dir_all(parent).map_err(|_| ())?;
    let temporary = parent.join(format!(".ffmpeg-stage-{}.tmp", std::process::id()));
    let result = (|| {
        fs::copy(source, &temporary).map_err(|_| ())?;
        File::open(&temporary)
            .and_then(|file| file.sync_all())
            .map_err(|_| ())?;
        verify_local_ffmpeg(&temporary, expected_digest)?;
        fs::rename(&temporary, destination).map_err(|_| ())?;
        verify_local_ffmpeg(destination, expected_digest)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn verify_local_ffmpeg(path: &Path, expected_digest: &str) -> Result<(), ()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.file_type().is_file() || !is_executable(&metadata) {
        return Err(());
    }
    if sha256(path)? != expected_digest {
        return Err(());
    }
    let output = std::process::Command::new(path)
        .arg("-version")
        .output()
        .map_err(|_| ())?;
    if !output.status.success() || !output.stdout.starts_with(b"ffmpeg version ") {
        return Err(());
    }
    Ok(())
}

fn sha256(path: &Path) -> Result<String, ()> {
    let mut file = File::open(path).map_err(|_| ())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| ())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}
