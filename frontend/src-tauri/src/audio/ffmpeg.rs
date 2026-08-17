use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

const EXECUTABLE_NAME: &str = "ffmpeg";
const APPROVED_BUNDLED_FFMPEG_SHA256: &str =
    "77d2c853f431318d55ec02676d9b2f185ebfdddb9f7677a251fbe453affe025a";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FfmpegError {
    ExecutableLocationUnavailable,
    BundledBinaryMissing,
    BundledBinaryInvalid,
    BundledBinaryNotExecutable,
    BundledBinaryIntegrityFailed,
}

impl FfmpegError {
    pub fn code(self) -> &'static str {
        match self {
            Self::ExecutableLocationUnavailable => "ffmpeg_executable_location_unavailable",
            Self::BundledBinaryMissing => "ffmpeg_bundled_binary_missing",
            Self::BundledBinaryInvalid => "ffmpeg_bundled_binary_invalid",
            Self::BundledBinaryNotExecutable => "ffmpeg_bundled_binary_not_executable",
            Self::BundledBinaryIntegrityFailed => "ffmpeg_bundled_binary_integrity_failed",
        }
    }
}

impl fmt::Display for FfmpegError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for FfmpegError {}

pub struct VerifiedFfmpeg {
    file: File,
    path: PathBuf,
    #[cfg(unix)]
    staging_directory: tempfile::TempDir,
}

impl VerifiedFfmpeg {
    pub fn command(&self) -> Result<Command, FfmpegError> {
        #[cfg(unix)]
        {
            let held = self
                .file
                .metadata()
                .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
            let staged =
                fs::symlink_metadata(&self.path).map_err(|_| FfmpegError::BundledBinaryInvalid)?;
            if !staged.file_type().is_file()
                || held.dev() != staged.dev()
                || held.ino() != staged.ino()
            {
                return Err(FfmpegError::BundledBinaryInvalid);
            }
        }
        Ok(Command::new(&self.path))
    }
}

#[cfg(unix)]
impl Drop for VerifiedFfmpeg {
    fn drop(&mut self) {
        let _ = fs::set_permissions(
            self.staging_directory.path(),
            fs::Permissions::from_mode(0o700),
        );
    }
}

pub fn verified_ffmpeg_for_spawn() -> Result<VerifiedFfmpeg, FfmpegError> {
    find_bundled_ffmpeg_path()
}

pub fn find_ffmpeg_path() -> Result<VerifiedFfmpeg, FfmpegError> {
    verified_ffmpeg_for_spawn()
}

pub fn find_bundled_ffmpeg_path() -> Result<VerifiedFfmpeg, FfmpegError> {
    let executable =
        std::env::current_exe().map_err(|_| FfmpegError::ExecutableLocationUnavailable)?;
    verify_bundled_ffmpeg_for_spawn(&executable, APPROVED_BUNDLED_FFMPEG_SHA256)
}

pub fn resolve_bundled_ffmpeg_from_executable(
    application_executable: &Path,
    expected_sha256: &str,
) -> Result<PathBuf, FfmpegError> {
    let candidate = application_executable
        .parent()
        .ok_or(FfmpegError::ExecutableLocationUnavailable)?
        .join(EXECUTABLE_NAME);
    verify_bundled_ffmpeg_for_spawn(application_executable, expected_sha256)?;
    Ok(candidate)
}

pub fn verify_bundled_ffmpeg_for_spawn(
    application_executable: &Path,
    expected_sha256: &str,
) -> Result<VerifiedFfmpeg, FfmpegError> {
    let directory = application_executable
        .parent()
        .ok_or(FfmpegError::ExecutableLocationUnavailable)?;
    let candidate = directory.join(EXECUTABLE_NAME);
    let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            FfmpegError::BundledBinaryMissing
        } else {
            FfmpegError::BundledBinaryInvalid
        }
    })?;
    if !metadata.file_type().is_file() {
        return Err(FfmpegError::BundledBinaryInvalid);
    }
    if !is_executable(&metadata) {
        return Err(FfmpegError::BundledBinaryNotExecutable);
    }
    #[cfg(unix)]
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&candidate)
        .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
    #[cfg(not(unix))]
    let mut file = File::open(&candidate).map_err(|_| FfmpegError::BundledBinaryInvalid)?;
    let opened_metadata = file
        .metadata()
        .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
    if !opened_metadata.file_type().is_file() || !is_executable(&opened_metadata) {
        return Err(FfmpegError::BundledBinaryInvalid);
    }
    if sha256_file(&mut file).map_err(|_| FfmpegError::BundledBinaryInvalid)? != expected_sha256 {
        return Err(FfmpegError::BundledBinaryIntegrityFailed);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| FfmpegError::BundledBinaryInvalid)?;

    #[cfg(unix)]
    {
        let staging_directory = tempfile::Builder::new()
            .prefix("gcrdings-ffmpeg-")
            .tempdir()
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        fs::set_permissions(staging_directory.path(), fs::Permissions::from_mode(0o700))
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        let staged_path = staging_directory.path().join(EXECUTABLE_NAME);
        let mut staged_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o500)
            .open(&staged_path)
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        std::io::copy(&mut file, &mut staged_file)
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        staged_file
            .sync_all()
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        fs::set_permissions(&staged_path, fs::Permissions::from_mode(0o500))
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        staged_file
            .seek(SeekFrom::Start(0))
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        if sha256_file(&mut staged_file).map_err(|_| FfmpegError::BundledBinaryInvalid)?
            != expected_sha256
        {
            return Err(FfmpegError::BundledBinaryIntegrityFailed);
        }
        staged_file
            .seek(SeekFrom::Start(0))
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        let staged_metadata = staged_file
            .metadata()
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        let staged_path_metadata =
            fs::symlink_metadata(&staged_path).map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        if !is_executable(&staged_path_metadata)
            || staged_metadata.dev() != staged_path_metadata.dev()
            || staged_metadata.ino() != staged_path_metadata.ino()
        {
            return Err(FfmpegError::BundledBinaryInvalid);
        }
        File::open(staging_directory.path())
            .and_then(|directory| directory.sync_all())
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        fs::set_permissions(staging_directory.path(), fs::Permissions::from_mode(0o500))
            .map_err(|_| FfmpegError::BundledBinaryInvalid)?;
        Ok(VerifiedFfmpeg {
            file: staged_file,
            path: staged_path,
            staging_directory,
        })
    }

    #[cfg(not(unix))]
    Ok(VerifiedFfmpeg {
        file,
        path: candidate,
    })
}

fn sha256_file(file: &mut File) -> Result<String, std::io::Error> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("gcrdings");
        fs::write(&executable, b"application").expect("application fixture");
        (directory, executable)
    }

    fn write_executable(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).expect("FFmpeg fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .expect("executable permission");
        }
    }

    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn accepts_only_hash_verified_sibling_binary() {
        let (_directory, executable) = fixture();
        let ffmpeg = executable.with_file_name(EXECUTABLE_NAME);
        let bytes = b"approved-ffmpeg";
        write_executable(&ffmpeg, bytes);

        assert_eq!(
            resolve_bundled_ffmpeg_from_executable(&executable, &digest(bytes)),
            Ok(ffmpeg)
        );
    }

    #[test]
    fn missing_bundled_binary_has_no_network_fallback_and_returns_typed_sanitized_error() {
        let (_directory, executable) = fixture();

        let error = resolve_bundled_ffmpeg_from_executable(&executable, &digest(b"missing"))
            .expect_err("missing binary must fail");

        assert_eq!(error, FfmpegError::BundledBinaryMissing);
        assert_eq!(error.to_string(), "ffmpeg_bundled_binary_missing");
        assert!(!error
            .to_string()
            .contains(executable.to_string_lossy().as_ref()));
    }

    #[test]
    fn substituted_or_corrupt_binary_fails_integrity() {
        let (_directory, executable) = fixture();
        let ffmpeg = executable.with_file_name(EXECUTABLE_NAME);
        write_executable(&ffmpeg, b"substituted-ffmpeg");

        assert_eq!(
            resolve_bundled_ffmpeg_from_executable(&executable, &digest(b"approved-ffmpeg")),
            Err(FfmpegError::BundledBinaryIntegrityFailed)
        );
    }

    #[test]
    fn spawn_boundary_rejects_a_binary_swapped_after_prior_validation() {
        let (_directory, executable) = fixture();
        let ffmpeg = executable.with_file_name(EXECUTABLE_NAME);
        let approved = b"#!/bin/sh\nprintf 'approved\\n'\n";
        write_executable(&ffmpeg, approved);
        let verified = verify_bundled_ffmpeg_for_spawn(&executable, &digest(approved))
            .expect("original inode verified");
        fs::rename(&ffmpeg, executable.with_file_name("original-ffmpeg"))
            .expect("move verified inode");
        write_executable(&ffmpeg, b"#!/bin/sh\nprintf substituted\n");

        let output = verified
            .command()
            .expect("verified command")
            .output()
            .expect("spawn verified inode");
        assert_eq!(output.stdout, b"approved\n");
    }

    #[test]
    fn spawn_boundary_ignores_in_place_source_mutation_after_private_staging() {
        let (_directory, executable) = fixture();
        let ffmpeg = executable.with_file_name(EXECUTABLE_NAME);
        let approved = b"#!/bin/sh\nprintf 'approved\\n'\n";
        write_executable(&ffmpeg, approved);
        let verified = verify_bundled_ffmpeg_for_spawn(&executable, &digest(approved))
            .expect("private staged executable");
        write_executable(&ffmpeg, b"#!/bin/sh\nprintf 'substituted\\n'\n");

        let output = verified
            .command()
            .expect("verified command")
            .output()
            .expect("spawn private staged executable");
        assert_eq!(output.stdout, b"approved\n");
    }

    #[test]
    fn private_staged_bundled_macho_remains_executable() {
        let output = find_bundled_ffmpeg_path()
            .expect("bundled FFmpeg is verified")
            .command()
            .expect("private staged command")
            .arg("-version")
            .output()
            .expect("execute private staged Mach-O");

        assert!(output.status.success());
        assert!(output.stdout.starts_with(b"ffmpeg version"));
    }

    #[test]
    fn path_shadow_is_ignored_when_bundle_is_missing() {
        let (directory, executable) = fixture();
        let shadow = directory.path().join("path-shadow").join(EXECUTABLE_NAME);
        fs::create_dir_all(shadow.parent().expect("shadow parent")).expect("shadow directory");
        write_executable(&shadow, b"approved-ffmpeg");

        assert_eq!(
            resolve_bundled_ffmpeg_from_executable(&executable, &digest(b"approved-ffmpeg")),
            Err(FfmpegError::BundledBinaryMissing)
        );
    }

    #[test]
    fn resolution_does_not_mutate_home_or_shell_profiles() {
        let (directory, executable) = fixture();
        let profile = directory.path().join(format!(".{}", "zshrc"));
        fs::write(&profile, b"preserve-me\n").expect("profile fixture");
        let before = fs::read(&profile).expect("profile before");

        let _ = resolve_bundled_ffmpeg_from_executable(&executable, &digest(b"missing"));

        assert_eq!(fs::read(&profile).expect("profile after"), before);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_and_non_executable_candidates_are_rejected() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let (directory, executable) = fixture();
        let outside = directory.path().join("outside-ffmpeg");
        write_executable(&outside, b"approved-ffmpeg");
        let ffmpeg = executable.with_file_name(EXECUTABLE_NAME);
        symlink(&outside, &ffmpeg).expect("symlink fixture");
        assert_eq!(
            resolve_bundled_ffmpeg_from_executable(&executable, &digest(b"approved-ffmpeg")),
            Err(FfmpegError::BundledBinaryInvalid)
        );

        fs::remove_file(&ffmpeg).expect("remove symlink");
        fs::write(&ffmpeg, b"approved-ffmpeg").expect("non-executable fixture");
        fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o600))
            .expect("non-executable permission");
        assert_eq!(
            resolve_bundled_ffmpeg_from_executable(&executable, &digest(b"approved-ffmpeg")),
            Err(FfmpegError::BundledBinaryNotExecutable)
        );
    }
}
