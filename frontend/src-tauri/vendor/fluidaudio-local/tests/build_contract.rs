#[path = "../build_support.rs"]
mod build_support;

use build_support::{
    require_local_adhoc_source, stage_canonical_archive, validate_archive_identity,
    validate_canonical_archive, CanonicalArchiveError, EXPECTED_FLUIDAUDIO_SHA256,
};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must follow the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gcrdings-fluidaudio-contract-{}-{nonce}-{}",
            std::process::id(),
            SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("scratch directory should be created");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn valid_symbols() -> &'static str {
    "000 T _fluidaudio_local_create\n\
     000 T _fluidaudio_local_destroy\n\
     000 T _fluidaudio_local_initialize_diarization\n\
     000 T _fluidaudio_local_diarize_file\n\
     000 T _fluidaudio_local_free_result\n"
}

#[test]
fn local_adhoc_never_falls_back_when_the_input_is_absent() {
    assert_eq!(
        require_local_adhoc_source(None),
        Err(CanonicalArchiveError::MissingSetting)
    );
    assert_eq!(
        require_local_adhoc_source(Some(OsString::new())),
        Err(CanonicalArchiveError::MissingSetting)
    );
}

#[test]
fn missing_input_fails_closed() {
    let scratch = Scratch::new();
    assert_eq!(
        validate_canonical_archive(&scratch.0.join("missing.a"), &"0".repeat(64)),
        Err(CanonicalArchiveError::Missing)
    );
}

#[test]
fn tampered_input_fails_before_native_inspection() {
    let scratch = Scratch::new();
    let archive = scratch.0.join("libFluidAudioLocalBridge.a");
    fs::write(&archive, b"tampered").expect("fixture should be written");
    assert_eq!(
        validate_canonical_archive(&archive, EXPECTED_FLUIDAUDIO_SHA256),
        Err(CanonicalArchiveError::HashMismatch)
    );
}

#[test]
fn symlink_input_is_rejected() {
    let scratch = Scratch::new();
    let target = scratch.0.join("target.a");
    let alias = scratch.0.join("alias.a");
    fs::write(&target, b"archive").expect("fixture should be written");
    symlink(&target, &alias).expect("fixture symlink should be created");
    assert_eq!(
        validate_canonical_archive(&alias, &"0".repeat(64)),
        Err(CanonicalArchiveError::Aliased)
    );
}

#[test]
fn wrong_architecture_is_rejected() {
    assert_eq!(
        validate_archive_identity("x86_64\n", valid_symbols()),
        Err(CanonicalArchiveError::WrongArchitecture)
    );
}

#[test]
fn missing_bridge_symbol_is_rejected() {
    assert_eq!(
        validate_archive_identity("arm64\n", "000 T _fluidaudio_local_create\n"),
        Err(CanonicalArchiveError::MissingSymbol)
    );
}

#[test]
fn exact_arm64_bridge_contract_is_accepted() {
    assert_eq!(
        validate_archive_identity("arm64\n", valid_symbols()),
        Ok(())
    );
}

#[test]
fn stages_the_exact_source_exclusively_before_validation() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source.a");
    let destination = scratch.0.join("private").join("canonical.a");
    fs::write(&source, b"first canonical bytes").expect("source fixture");

    stage_canonical_archive(&source, &destination).expect("first exclusive stage");
    fs::write(&source, b"mutated after staging").expect("source mutation");

    assert_eq!(
        fs::read(&destination).expect("staged bytes"),
        b"first canonical bytes"
    );
    assert!(stage_canonical_archive(&source, &destination).is_err());
}
