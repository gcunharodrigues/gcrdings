#[cfg(target_os = "macos")]
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MANIFEST_JSON: &str =
    include_str!("../../../../qualification/audio-corpus/model-manifest.json");
const ENGINE_REVISION: &str = "d302273d49ef4d8914b27f20d342be482e8810f1";
const MODEL_REVISION: &str = "1ed7a662fdc7109e36d822db793ee6eebdaf8594";
static MODEL_LOCK: tokio::sync::RwLock<()> = tokio::sync::RwLock::const_new(());
static MANIFEST: OnceLock<ModelManifest> = OnceLock::new();

#[derive(Clone, Debug, Deserialize)]
struct ModelManifest {
    schema_version: u8,
    engine: String,
    engine_version: String,
    engine_revision: String,
    repository: String,
    revision: String,
    license: String,
    directory: String,
    files: Vec<ModelFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct ModelFile {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiarizationModelState {
    Missing,
    Corrupted,
    Available,
    Unsupported,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiarizationModelStatus {
    pub state: DiarizationModelState,
    pub engine_version: String,
    pub revision: String,
    pub license: String,
    pub verified_files: usize,
    pub total_files: usize,
}

fn manifest() -> Result<&'static ModelManifest> {
    if let Some(manifest) = MANIFEST.get() {
        return Ok(manifest);
    }
    let parsed: ModelManifest = serde_json::from_str(MANIFEST_JSON)
        .context("embedded diarization model manifest is invalid")?;
    validate_manifest(&parsed)?;
    let _ = MANIFEST.set(parsed);
    MANIFEST
        .get()
        .ok_or_else(|| anyhow!("failed to initialize diarization model manifest"))
}

fn validate_manifest(manifest: &ModelManifest) -> Result<()> {
    if manifest.schema_version != 1
        || manifest.engine != "FluidAudio"
        || manifest.engine_version != "0.14.1"
        || manifest.engine_revision != ENGINE_REVISION
        || manifest.repository != "FluidInference/speaker-diarization-coreml"
        || manifest.revision != MODEL_REVISION
        || manifest.license != "CC-BY-4.0"
        || manifest.directory != "speaker-diarization"
        || manifest.files.is_empty()
    {
        return Err(anyhow!("unsupported diarization model manifest"));
    }
    for file in &manifest.files {
        let relative = Path::new(&file.path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            || !is_hex(&file.sha256, 64)
            || file.bytes == 0
        {
            return Err(anyhow!("unsafe diarization model manifest entry"));
        }
    }
    Ok(())
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn model_directory() -> Result<PathBuf> {
    let manifest = manifest()?;
    dirs::data_dir()
        .map(|path| path.join("FluidAudio/Models").join(&manifest.directory))
        .ok_or_else(|| anyhow!("application support directory is unavailable"))
}

fn file_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_directory(directory: &Path, manifest: &ModelManifest) -> DiarizationModelStatus {
    let mut verified_files = 0;
    let mut state = DiarizationModelState::Available;
    for expected in &manifest.files {
        let path = directory.join(&expected.path);
        match path.metadata() {
            Ok(metadata)
                if metadata.is_file()
                    && metadata.len() == expected.bytes
                    && file_hash(&path).is_ok_and(|hash| hash == expected.sha256) =>
            {
                verified_files += 1;
            }
            Ok(_) => state = DiarizationModelState::Corrupted,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if state != DiarizationModelState::Corrupted {
                    state = DiarizationModelState::Missing;
                }
            }
            Err(_) => state = DiarizationModelState::Corrupted,
        }
    }
    DiarizationModelStatus {
        state,
        engine_version: manifest.engine_version.clone(),
        revision: manifest.revision.clone(),
        license: manifest.license.clone(),
        verified_files,
        total_files: manifest.files.len(),
    }
}

pub fn verified_model_directory() -> Result<PathBuf> {
    let manifest = manifest()?;
    let directory = model_directory()?;
    let status = verify_directory(&directory, manifest);
    if status.state == DiarizationModelState::Available {
        Ok(directory)
    } else {
        Err(anyhow!(
            "speaker diarization model is not installed or failed integrity verification"
        ))
    }
}

pub async fn processing_guard() -> tokio::sync::RwLockReadGuard<'static, ()> {
    MODEL_LOCK.read().await
}

#[tauri::command]
pub async fn diarization_model_status() -> Result<DiarizationModelStatus, String> {
    #[cfg(not(target_os = "macos"))]
    {
        let manifest = manifest().map_err(|_| "Model status is unavailable".to_string())?;
        return Ok(DiarizationModelStatus {
            state: DiarizationModelState::Unsupported,
            engine_version: manifest.engine_version.clone(),
            revision: manifest.revision.clone(),
            license: manifest.license.clone(),
            verified_files: 0,
            total_files: manifest.files.len(),
        });
    }
    #[cfg(target_os = "macos")]
    {
        let manifest = manifest()
            .map_err(|_| "Diarization model status is unavailable".to_string())?
            .clone();
        let directory =
            model_directory().map_err(|_| "Diarization model status is unavailable".to_string())?;
        tokio::task::spawn_blocking(move || verify_directory(&directory, &manifest))
            .await
            .map_err(|_| "Diarization model status is unavailable".to_string())
    }
}

#[tauri::command]
pub async fn diarization_install_models() -> Result<DiarizationModelStatus, String> {
    #[cfg(not(target_os = "macos"))]
    return Err("Speaker diarization is available on macOS only".to_string());

    #[cfg(target_os = "macos")]
    {
        let _install_guard = MODEL_LOCK.write().await;
        let manifest = manifest()
            .map_err(|_| "Diarization model installation failed".to_string())?
            .clone();
        let target =
            model_directory().map_err(|_| "Diarization model installation failed".to_string())?;
        if verify_directory(&target, &manifest).state == DiarizationModelState::Available {
            return Ok(verify_directory(&target, &manifest));
        }
        install_models(&target, &manifest).await.map_err(|_| {
            log::error!("Diarization model installation failed");
            "Diarization model installation failed".to_string()
        })?;
        Ok(verify_directory(&target, &manifest))
    }
}

async fn install_models(target: &Path, manifest: &ModelManifest) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| anyhow!("invalid model destination"))?;
    tokio::fs::create_dir_all(parent).await?;
    let staging = tempfile::Builder::new()
        .prefix(".speaker-diarization.installing-")
        .tempdir_in(parent)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    for expected in &manifest.files {
        let url = format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            manifest.repository, manifest.revision, expected.path
        );
        let response = client.get(url).send().await?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > expected.bytes)
        {
            return Err(anyhow!("downloaded model file exceeds expected size"));
        }
        let expected_bytes = usize::try_from(expected.bytes)?;
        let mut contents = Vec::with_capacity(expected_bytes);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if contents.len().saturating_add(chunk.len()) > expected_bytes {
                return Err(anyhow!("downloaded model file exceeds expected size"));
            }
            contents.extend_from_slice(&chunk);
        }
        let actual_hash = format!("{:x}", Sha256::digest(&contents));
        if contents.len() as u64 != expected.bytes || actual_hash != expected.sha256 {
            return Err(anyhow!(
                "downloaded model file failed integrity verification"
            ));
        }
        let destination = staging.path().join(&expected.path);
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(destination, contents).await?;
    }

    if verify_directory(staging.path(), manifest).state != DiarizationModelState::Available {
        return Err(anyhow!("staged model failed integrity verification"));
    }

    activate_staging(staging.path(), target)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn activate_staging(staging: &Path, target: &Path) -> Result<()> {
    let target_exists = match std::fs::symlink_metadata(target) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if !target_exists {
        std::fs::rename(staging, target)?;
        return Ok(());
    }
    let staging_c = CString::new(staging.as_os_str().as_bytes())?;
    let target_c = CString::new(target.as_os_str().as_bytes())?;
    let result =
        unsafe { libc::renamex_np(staging_c.as_ptr(), target_c.as_ptr(), libc::RENAME_SWAP) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if std::fs::symlink_metadata(staging).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        let _ = std::fs::remove_file(staging);
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn activate_staging(staging: &Path, target: &Path) -> Result<()> {
    std::fs::rename(staging, target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_file_manifest(contents: &[u8]) -> ModelManifest {
        ModelManifest {
            schema_version: 1,
            engine: "FluidAudio".to_string(),
            engine_version: "0.14.1".to_string(),
            engine_revision: ENGINE_REVISION.to_string(),
            repository: "FluidInference/speaker-diarization-coreml".to_string(),
            revision: MODEL_REVISION.to_string(),
            license: "CC-BY-4.0".to_string(),
            directory: "speaker-diarization".to_string(),
            files: vec![ModelFile {
                path: "Model.mlmodelc/coremldata.bin".to_string(),
                sha256: format!("{:x}", Sha256::digest(contents)),
                bytes: contents.len() as u64,
            }],
        }
    }

    #[test]
    fn rejects_missing_and_wrong_model_files() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = one_file_manifest(b"verified");
        assert_eq!(
            verify_directory(directory.path(), &manifest).state,
            DiarizationModelState::Missing
        );

        let model = directory.path().join(&manifest.files[0].path);
        std::fs::create_dir_all(model.parent().unwrap()).unwrap();
        std::fs::write(&model, b"modified").unwrap();
        assert_eq!(
            verify_directory(directory.path(), &manifest).state,
            DiarizationModelState::Corrupted
        );
    }

    #[test]
    fn accepts_only_the_expected_model_hash() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = one_file_manifest(b"verified");
        let model = directory.path().join(&manifest.files[0].path);
        std::fs::create_dir_all(model.parent().unwrap()).unwrap();
        std::fs::write(model, b"verified").unwrap();
        let status = verify_directory(directory.path(), &manifest);
        assert_eq!(status.state, DiarizationModelState::Available);
        assert_eq!(status.verified_files, 1);
    }

    #[test]
    fn rejects_manifest_path_traversal() {
        let mut manifest = one_file_manifest(b"verified");
        manifest.files[0].path = "../outside.bin".to_string();
        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn rejects_unpinned_manifest_provenance() {
        let mut manifest = one_file_manifest(b"verified");
        assert!(validate_manifest(&manifest).is_ok());
        manifest.revision = "0000000000000000000000000000000000000000".to_string();
        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn local_bridge_has_no_model_download_path() {
        let bridge =
            include_str!("../../vendor/fluidaudio-local/swift/FluidAudioLocalBridge.swift");
        assert!(!bridge.contains("URLSession"));
        assert!(!bridge.contains("prepareModels"));
        assert!(!bridge.contains("DownloadUtils"));
        assert!(bridge.contains("configuration.postProcessing.exclusiveSegments = false"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn swaps_installed_models_atomically_and_preserves_target_on_failure() {
        let parent = tempfile::tempdir().unwrap();
        let target = parent.path().join("speaker-diarization");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("state"), b"old").unwrap();
        let staging = tempfile::tempdir_in(parent.path()).unwrap();
        std::fs::write(staging.path().join("state"), b"new").unwrap();

        activate_staging(staging.path(), &target).unwrap();
        assert_eq!(std::fs::read(target.join("state")).unwrap(), b"new");
        assert_eq!(std::fs::read(staging.path().join("state")).unwrap(), b"old");

        let missing = parent.path().join("missing");
        assert!(activate_staging(&missing, &target).is_err());
        assert_eq!(std::fs::read(target.join("state")).unwrap(), b"new");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn replaces_a_broken_target_symlink() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let target = parent.path().join("speaker-diarization");
        symlink(parent.path().join("missing"), &target).unwrap();
        let staging = tempfile::tempdir_in(parent.path()).unwrap();
        std::fs::write(staging.path().join("state"), b"new").unwrap();

        activate_staging(staging.path(), &target).unwrap();
        assert_eq!(std::fs::read(target.join("state")).unwrap(), b"new");
    }
}
