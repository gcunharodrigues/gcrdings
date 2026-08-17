use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const EXPECTED_FLUIDAUDIO_SHA256: &str =
    "e14173844a6c296995c9e8ca0fea574168ef2204d0c682a828f46f002892ed5d";

const REQUIRED_SYMBOLS: [&str; 5] = [
    "_fluidaudio_local_create",
    "_fluidaudio_local_destroy",
    "_fluidaudio_local_initialize_diarization",
    "_fluidaudio_local_diarize_file",
    "_fluidaudio_local_free_result",
];

#[derive(Debug, PartialEq, Eq)]
pub enum CanonicalArchiveError {
    MissingSetting,
    Missing,
    Aliased,
    NotRegular,
    HashUnavailable,
    HashMismatch,
    InspectionFailed,
    WrongArchitecture,
    MissingSymbol,
    StagingFailed,
}

impl fmt::Display for CanonicalArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::MissingSetting => "canonical FluidAudio input is not configured",
            Self::Missing => "canonical FluidAudio input is missing",
            Self::Aliased => "canonical FluidAudio input must not be a symlink",
            Self::NotRegular => "canonical FluidAudio input must be a regular file",
            Self::HashUnavailable => "canonical FluidAudio input hash is unavailable",
            Self::HashMismatch => "canonical FluidAudio input hash does not match",
            Self::InspectionFailed => "canonical FluidAudio input inspection failed",
            Self::WrongArchitecture => "canonical FluidAudio input architecture does not match",
            Self::MissingSymbol => "canonical FluidAudio input contract is incomplete",
            Self::StagingFailed => "canonical FluidAudio input could not be staged",
        };
        formatter.write_str(message)
    }
}

pub fn stage_canonical_archive(
    source: &Path,
    destination: &Path,
) -> Result<(), CanonicalArchiveError> {
    let source_metadata = fs::symlink_metadata(source).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CanonicalArchiveError::Missing
        } else {
            CanonicalArchiveError::StagingFailed
        }
    })?;
    if source_metadata.file_type().is_symlink() {
        return Err(CanonicalArchiveError::Aliased);
    }
    if !source_metadata.file_type().is_file() {
        return Err(CanonicalArchiveError::NotRegular);
    }
    let parent = destination
        .parent()
        .ok_or(CanonicalArchiveError::StagingFailed)?;
    fs::create_dir_all(parent).map_err(|_| CanonicalArchiveError::StagingFailed)?;
    let mut source_file =
        fs::File::open(source).map_err(|_| CanonicalArchiveError::StagingFailed)?;
    let mut destination_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| CanonicalArchiveError::StagingFailed)?;
    std::io::copy(&mut source_file, &mut destination_file)
        .map_err(|_| CanonicalArchiveError::StagingFailed)?;
    destination_file
        .flush()
        .and_then(|_| destination_file.sync_all())
        .map_err(|_| CanonicalArchiveError::StagingFailed)
}

pub fn require_local_adhoc_source(
    configured: Option<OsString>,
) -> Result<PathBuf, CanonicalArchiveError> {
    configured
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(CanonicalArchiveError::MissingSetting)
}

pub fn validate_canonical_archive(
    path: &Path,
    expected_sha256: &str,
) -> Result<(), CanonicalArchiveError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CanonicalArchiveError::Missing
        } else {
            CanonicalArchiveError::InspectionFailed
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(CanonicalArchiveError::Aliased);
    }
    if !metadata.file_type().is_file() {
        return Err(CanonicalArchiveError::NotRegular);
    }

    let actual_sha256 = sha256_file(path)?;
    if actual_sha256 != expected_sha256 {
        return Err(CanonicalArchiveError::HashMismatch);
    }

    let architectures = command_output("/usr/bin/lipo", &[OsString::from("-archs"), path.into()])?;
    let symbols = command_output("/usr/bin/nm", &[OsString::from("-gU"), path.into()])?;
    validate_archive_identity(&architectures, &symbols)
}

pub fn validate_archive_identity(
    architectures: &str,
    symbols: &str,
) -> Result<(), CanonicalArchiveError> {
    if architectures.trim() != "arm64" {
        return Err(CanonicalArchiveError::WrongArchitecture);
    }
    for required in REQUIRED_SYMBOLS {
        let present = symbols
            .lines()
            .filter_map(|line| line.split_whitespace().last())
            .any(|symbol| symbol == required);
        if !present {
            return Err(CanonicalArchiveError::MissingSymbol);
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, CanonicalArchiveError> {
    let output = Command::new("/usr/bin/shasum")
        .args([OsString::from("-a"), OsString::from("256"), path.into()])
        .output()
        .map_err(|_| CanonicalArchiveError::HashUnavailable)?;
    if !output.status.success() {
        return Err(CanonicalArchiveError::HashUnavailable);
    }
    let stdout =
        String::from_utf8(output.stdout).map_err(|_| CanonicalArchiveError::HashUnavailable)?;
    let digest = stdout
        .split_whitespace()
        .next()
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or(CanonicalArchiveError::HashUnavailable)?;
    Ok(digest.to_ascii_lowercase())
}

fn command_output(program: &str, arguments: &[OsString]) -> Result<String, CanonicalArchiveError> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|_| CanonicalArchiveError::InspectionFailed)?;
    if !output.status.success() {
        return Err(CanonicalArchiveError::InspectionFailed);
    }
    String::from_utf8(output.stdout).map_err(|_| CanonicalArchiveError::InspectionFailed)
}
