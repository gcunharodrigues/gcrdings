use super::models::{GenerationRequest, GenerationResponse, VerifiableRecordError};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

const MAX_HELPER_REQUEST_BYTES: usize = 8_000_000;
const MAX_HELPER_STDOUT_BYTES: usize = 1_000_000;

pub struct AppleFoundation {
    helper_path: PathBuf,
    timeout: Duration,
}

impl AppleFoundation {
    pub fn packaged() -> Result<Self, VerifiableRecordError> {
        let executable =
            std::env::current_exe().map_err(|_| VerifiableRecordError::HelperFailed)?;
        let directory = executable
            .parent()
            .ok_or(VerifiableRecordError::HelperFailed)?;
        let target = option_env!("TARGET").unwrap_or("aarch64-apple-darwin");
        let candidates = [
            directory.join("foundation-helper"),
            directory.join(format!("foundation-helper-{target}")),
        ];
        let helper_path = candidates
            .into_iter()
            .find(|path| path.is_file())
            .ok_or(VerifiableRecordError::Unavailable)?;
        Ok(Self {
            helper_path,
            timeout: Duration::from_secs(120),
        })
    }

    #[cfg(test)]
    fn fixture(path: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            helper_path: path.into(),
            timeout,
        }
    }

    pub async fn run(
        &self,
        request: &GenerationRequest,
        cancellation: CancellationToken,
    ) -> Result<GenerationResponse, VerifiableRecordError> {
        let mut child = Command::new(&self.helper_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| VerifiableRecordError::HelperFailed)?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or(VerifiableRecordError::HelperFailed)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(VerifiableRecordError::HelperFailed)?;
        let mut payload =
            serde_json::to_vec(request).map_err(|_| VerifiableRecordError::InvalidOutput)?;
        payload.push(b'\n');
        if payload.len() > MAX_HELPER_REQUEST_BYTES {
            return Err(VerifiableRecordError::InvalidOutput);
        }
        let exchange = async {
            let write = async {
                stdin
                    .write_all(&payload)
                    .await
                    .map_err(|_| VerifiableRecordError::HelperFailed)?;
                stdin
                    .shutdown()
                    .await
                    .map_err(|_| VerifiableRecordError::HelperFailed)
            };
            let read = async {
                let mut bytes = Vec::new();
                stdout
                    .take((MAX_HELPER_STDOUT_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await
                    .map_err(|_| VerifiableRecordError::HelperFailed)?;
                Ok::<_, VerifiableRecordError>(bytes)
            };
            let (_, stdout) = tokio::try_join!(write, read)?;
            if stdout.len() > MAX_HELPER_STDOUT_BYTES {
                child
                    .kill()
                    .await
                    .map_err(|_| VerifiableRecordError::HelperFailed)?;
                return Err(VerifiableRecordError::HelperFailed);
            }
            let status = child
                .wait()
                .await
                .map_err(|_| VerifiableRecordError::HelperFailed)?;
            Ok::<_, VerifiableRecordError>((status, stdout))
        };
        let (status, stdout) = tokio::select! {
            _ = cancellation.cancelled() => return Err(VerifiableRecordError::Cancelled),
            result = tokio::time::timeout(self.timeout, exchange) => {
                result.map_err(|_| VerifiableRecordError::Timeout)??
            }
        };
        if !status.success() {
            return Err(VerifiableRecordError::HelperFailed);
        }
        parse_single_response(request, &stdout)
    }
}

pub fn parse_single_response(
    request: &GenerationRequest,
    stdout: &[u8],
) -> Result<GenerationResponse, VerifiableRecordError> {
    if stdout.len() > MAX_HELPER_STDOUT_BYTES {
        return Err(VerifiableRecordError::HelperFailed);
    }
    let text = std::str::from_utf8(stdout).map_err(|_| VerifiableRecordError::HelperFailed)?;
    if !text.ends_with('\n') {
        return Err(VerifiableRecordError::HelperFailed);
    }
    let mut lines = text.lines();
    let line = lines
        .next()
        .filter(|line| !line.trim().is_empty())
        .ok_or(VerifiableRecordError::HelperFailed)?;
    if lines.next().is_some() {
        return Err(VerifiableRecordError::HelperFailed);
    }
    let response: GenerationResponse =
        serde_json::from_str(line).map_err(|_| VerifiableRecordError::HelperFailed)?;
    let (schema, request_id) = match &response {
        GenerationResponse::Completed {
            schema_version,
            request_id,
            ..
        }
        | GenerationResponse::Unavailable {
            schema_version,
            request_id,
            ..
        }
        | GenerationResponse::Failed {
            schema_version,
            request_id,
            ..
        } => (*schema_version, request_id),
    };
    if schema != request.schema_version || request_id != &request.request_id {
        return Err(VerifiableRecordError::HelperFailed);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verifiable_record::models::{RecordBlock, RecordType};

    fn request() -> GenerationRequest {
        GenerationRequest {
            schema_version: 1,
            request_id: "r1".into(),
            record_type: RecordType::Content,
            principal_transcript_revision: 1,
            participants: vec![],
            transcript: vec![RecordBlock {
                id: "b1".into(),
                text: "Text".into(),
                participant_id: "p1".into(),
                start_ms: 0,
                end_ms: 1000,
            }],
            prompt_version: "content-v1".into(),
        }
    }

    #[test]
    fn accepts_exactly_one_matching_ndjson_response() {
        let line = b"{\"status\":\"unavailable\",\"schema_version\":1,\"request_id\":\"r1\",\"reason\":\"model_not_ready\"}\n";
        assert!(matches!(
            parse_single_response(&request(), line),
            Ok(GenerationResponse::Unavailable { .. })
        ));
    }

    #[test]
    fn rejects_malformed_extra_and_mismatched_protocol_output() {
        for output in [b"not-json\n".as_slice(), b"{}\n{}\n".as_slice(), b"{\"status\":\"unavailable\",\"schema_version\":1,\"request_id\":\"wrong\",\"reason\":\"disabled\"}\n".as_slice(), b"{\"status\":\"unavailable\",\"schema_version\":1,\"request_id\":\"r1\",\"reason\":\"disabled\",\"private\":true}\n".as_slice()] {
            assert_eq!(parse_single_response(&request(), output), Err(VerifiableRecordError::HelperFailed));
        }
    }

    #[test]
    fn rejects_oversized_nested_unknown_and_unrecognized_typed_output() {
        let oversized = serde_json::json!({
            "status": "completed",
            "schema_version": 1,
            "request_id": "r1",
            "principal_transcript_revision": 1,
            "result": {
                "record_type": "content",
                "summary": "x".repeat(1_100_000),
                "claims": [{
                    "label": "Claim",
                    "detail": "Grounded",
                    "evidence": [{"block_id": "b1", "timestamp_ms": 500}]
                }],
                "outline": [],
                "source_notes": []
            }
        });
        let mut oversized = serde_json::to_vec(&oversized).unwrap();
        oversized.push(b'\n');
        assert_eq!(
            parse_single_response(&request(), &oversized),
            Err(VerifiableRecordError::HelperFailed)
        );

        let nested_unknown = b"{\"status\":\"completed\",\"schema_version\":1,\"request_id\":\"r1\",\"principal_transcript_revision\":1,\"result\":{\"record_type\":\"content\",\"summary\":\"Grounded\",\"claims\":[{\"label\":\"Claim\",\"detail\":\"Grounded\",\"evidence\":[{\"block_id\":\"b1\",\"timestamp_ms\":500,\"private\":\"/secret/path\"}]}],\"outline\":[],\"source_notes\":[]}}\n";
        assert_eq!(
            parse_single_response(&request(), nested_unknown),
            Err(VerifiableRecordError::HelperFailed)
        );

        for output in [
            b"{\"status\":\"unavailable\",\"schema_version\":1,\"request_id\":\"r1\",\"reason\":\"/private/model-state\"}\n".as_slice(),
            b"{\"status\":\"failed\",\"schema_version\":1,\"request_id\":\"r1\",\"code\":\"prompt: private transcript\"}\n".as_slice(),
        ] {
            assert_eq!(
                parse_single_response(&request(), output),
                Err(VerifiableRecordError::HelperFailed)
            );
        }
    }

    #[tokio::test]
    async fn timeout_and_cancellation_are_typed() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let mut executable = tempfile::NamedTempFile::new().unwrap();
        writeln!(executable, "#!/bin/sh\nsleep 5").unwrap();
        let mut permissions = executable.as_file().metadata().unwrap().permissions();
        permissions.set_mode(0o755);
        executable.as_file().set_permissions(permissions).unwrap();
        let helper = AppleFoundation::fixture(executable.path(), Duration::from_millis(10));
        assert_eq!(
            helper.run(&request(), CancellationToken::new()).await,
            Err(VerifiableRecordError::Timeout)
        );
        let token = CancellationToken::new();
        token.cancel();
        assert_eq!(
            AppleFoundation::fixture(executable.path(), Duration::from_secs(10))
                .run(&request(), token)
                .await,
            Err(VerifiableRecordError::Cancelled)
        );
    }

    #[tokio::test]
    async fn timeout_bounds_stdin_delivery_to_a_non_reading_helper() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let mut executable = tempfile::NamedTempFile::new().unwrap();
        writeln!(executable, "#!/bin/sh\nsleep 1").unwrap();
        let mut permissions = executable.as_file().metadata().unwrap().permissions();
        permissions.set_mode(0o755);
        executable.as_file().set_permissions(permissions).unwrap();
        let mut large_request = request();
        large_request.transcript[0].text = "x".repeat(1_000_000);

        assert_eq!(
            AppleFoundation::fixture(executable.path(), Duration::from_millis(10))
                .run(&large_request, CancellationToken::new())
                .await,
            Err(VerifiableRecordError::Timeout)
        );
    }

    #[tokio::test]
    async fn helper_death_is_typed() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let mut executable = tempfile::NamedTempFile::new().unwrap();
        writeln!(executable, "#!/bin/sh\nexit 9").unwrap();
        let mut permissions = executable.as_file().metadata().unwrap().permissions();
        permissions.set_mode(0o755);
        executable.as_file().set_permissions(permissions).unwrap();
        assert_eq!(
            AppleFoundation::fixture(executable.path(), Duration::from_secs(1))
                .run(&request(), CancellationToken::new())
                .await,
            Err(VerifiableRecordError::HelperFailed)
        );
    }
}
