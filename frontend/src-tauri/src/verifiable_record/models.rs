use crate::database::models::Participant;
use serde::{Deserialize, Serialize};

pub const VERIFIABLE_RECORD_SCHEMA_VERSION: u32 = 1;
const MAX_GENERATED_RECORD_BYTES: usize = 512_000;
const MAX_SUMMARY_BYTES: usize = 64_000;
const MAX_FINDINGS: usize = 24;
const MAX_LABEL_BYTES: usize = 256;
const MAX_DETAIL_BYTES: usize = 16_384;
const MAX_EVIDENCE_PER_FINDING: usize = 8;
const MAX_BLOCK_ID_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    Meeting,
    Interview,
    Content,
}

impl RecordType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Meeting => "meeting",
            Self::Interview => "interview",
            Self::Content => "content",
        }
    }

    pub fn parse(value: &str) -> Result<Self, VerifiableRecordError> {
        match value {
            "meeting" => Ok(Self::Meeting),
            "interview" => Ok(Self::Interview),
            "content" => Ok(Self::Content),
            _ => Err(VerifiableRecordError::InvalidOutput),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStatus {
    Pending,
    Processing,
    Completed,
    Unavailable,
    Failed,
    Stale,
}

impl GenerationStatus {
    pub fn parse(value: &str) -> Result<Self, VerifiableRecordError> {
        match value {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "unavailable" => Ok(Self::Unavailable),
            "failed" => Ok(Self::Failed),
            "stale" => Ok(Self::Stale),
            _ => Err(VerifiableRecordError::InvalidOutput),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReference {
    pub block_id: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub label: String,
    pub detail: String,
    pub evidence: Vec<EvidenceReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GeneratedRecord {
    Meeting {
        summary: String,
        decisions: Vec<Finding>,
        action_items: Vec<Finding>,
        key_points: Vec<Finding>,
    },
    Interview {
        summary: String,
        answers: Vec<Finding>,
        themes: Vec<Finding>,
        follow_ups: Vec<Finding>,
    },
    Content {
        summary: String,
        claims: Vec<Finding>,
        outline: Vec<Finding>,
        source_notes: Vec<Finding>,
    },
}

impl GeneratedRecord {
    pub fn summary(&self) -> &str {
        match self {
            Self::Meeting { summary, .. }
            | Self::Interview { summary, .. }
            | Self::Content { summary, .. } => summary,
        }
    }

    pub fn findings(&self) -> Vec<&Finding> {
        match self {
            Self::Meeting {
                decisions,
                action_items,
                key_points,
                ..
            } => decisions
                .iter()
                .chain(action_items)
                .chain(key_points)
                .collect(),
            Self::Interview {
                answers,
                themes,
                follow_ups,
                ..
            } => answers.iter().chain(themes).chain(follow_ups).collect(),
            Self::Content {
                claims,
                outline,
                source_notes,
                ..
            } => claims.iter().chain(outline).chain(source_notes).collect(),
        }
    }

    fn matches(&self, record_type: RecordType) -> bool {
        matches!(
            (self, record_type),
            (Self::Meeting { .. }, RecordType::Meeting)
                | (Self::Interview { .. }, RecordType::Interview)
                | (Self::Content { .. }, RecordType::Content)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordBlock {
    pub id: String,
    pub text: String,
    pub participant_id: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifiableRecord {
    pub schema_version: u32,
    pub meeting_id: String,
    pub title: String,
    pub record_type: RecordType,
    pub principal_transcript_revision: i64,
    pub participants: Vec<Participant>,
    pub transcript: Vec<RecordBlock>,
    pub generation_status: GenerationStatus,
    pub generated: Option<GeneratedRecord>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub record_type: RecordType,
    pub principal_transcript_revision: i64,
    pub participants: Vec<Participant>,
    pub transcript: Vec<RecordBlock>,
    pub prompt_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    UnsupportedOs,
    IneligibleDevice,
    AppleIntelligenceDisabled,
    ModelNotReady,
    UnsupportedLocale,
    HelperUnavailable,
}

impl UnavailableReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedOs => "unsupported_os",
            Self::IneligibleDevice => "ineligible_device",
            Self::AppleIntelligenceDisabled => "apple_intelligence_disabled",
            Self::ModelNotReady => "model_not_ready",
            Self::UnsupportedLocale => "unsupported_locale",
            Self::HelperUnavailable => "helper_unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    InvalidRequest,
    ContextSize,
    Cancelled,
    GenerationFailed,
    InvalidOutput,
    Timeout,
    HelperFailed,
}

impl FailureCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::ContextSize => "context_size",
            Self::Cancelled => "cancelled",
            Self::GenerationFailed => "generation_failed",
            Self::InvalidOutput => "invalid_output",
            Self::Timeout => "timeout",
            Self::HelperFailed => "helper_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum GenerationResponse {
    Completed {
        schema_version: u32,
        request_id: String,
        principal_transcript_revision: i64,
        result: GeneratedRecord,
    },
    Unavailable {
        schema_version: u32,
        request_id: String,
        reason: UnavailableReason,
    },
    Failed {
        schema_version: u32,
        request_id: String,
        code: FailureCode,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum VerifiableRecordError {
    #[error("local intelligence is unavailable")]
    Unavailable,
    #[error("generation output is invalid")]
    InvalidOutput,
    #[error("principal transcript changed")]
    StaleRevision,
    #[error("generation is already active")]
    AlreadyProcessing,
    #[error("generation timed out")]
    Timeout,
    #[error("generation was cancelled")]
    Cancelled,
    #[error("local helper failed")]
    HelperFailed,
    #[error("Session not found")]
    NotFound,
    #[error("record storage failed")]
    Storage,
}

pub fn validate_completed(
    request: &GenerationRequest,
    response: &GenerationResponse,
) -> Result<GeneratedRecord, VerifiableRecordError> {
    let GenerationResponse::Completed {
        schema_version,
        request_id,
        principal_transcript_revision,
        result,
    } = response
    else {
        return Err(VerifiableRecordError::InvalidOutput);
    };
    if *schema_version != VERIFIABLE_RECORD_SCHEMA_VERSION
        || request.schema_version != *schema_version
        || request.request_id != *request_id
    {
        return Err(VerifiableRecordError::InvalidOutput);
    }
    if request.principal_transcript_revision != *principal_transcript_revision {
        return Err(VerifiableRecordError::StaleRevision);
    }
    let findings = result.findings();
    let total_bytes = serde_json::to_vec(result)
        .map_err(|_| VerifiableRecordError::InvalidOutput)?
        .len();
    if !result.matches(request.record_type)
        || result.summary().trim().is_empty()
        || result.summary().len() > MAX_SUMMARY_BYTES
        || findings.is_empty()
        || findings.len() > MAX_FINDINGS
        || total_bytes > MAX_GENERATED_RECORD_BYTES
    {
        return Err(VerifiableRecordError::InvalidOutput);
    }
    for finding in findings {
        if finding.label.trim().is_empty()
            || finding.label.len() > MAX_LABEL_BYTES
            || finding.detail.trim().is_empty()
            || finding.detail.len() > MAX_DETAIL_BYTES
            || finding.evidence.is_empty()
            || finding.evidence.len() > MAX_EVIDENCE_PER_FINDING
        {
            return Err(VerifiableRecordError::InvalidOutput);
        }
        for evidence in &finding.evidence {
            if evidence.block_id.is_empty() || evidence.block_id.len() > MAX_BLOCK_ID_BYTES {
                return Err(VerifiableRecordError::InvalidOutput);
            }
            let Some(block) = request
                .transcript
                .iter()
                .find(|block| block.id == evidence.block_id)
            else {
                return Err(VerifiableRecordError::InvalidOutput);
            };
            if evidence.timestamp_ms < block.start_ms || evidence.timestamp_ms > block.end_ms {
                return Err(VerifiableRecordError::InvalidOutput);
            }
        }
    }
    Ok(result.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> GenerationRequest {
        GenerationRequest {
            schema_version: 1,
            request_id: "request-1".into(),
            record_type: RecordType::Meeting,
            principal_transcript_revision: 4,
            participants: vec![Participant {
                id: "p1".into(),
                display_name: "Speaker 1".into(),
                speaker_cluster_id: None,
            }],
            transcript: vec![RecordBlock {
                id: "b1".into(),
                text: "Principal text".into(),
                participant_id: "p1".into(),
                start_ms: 1000,
                end_ms: 2000,
            }],
            prompt_version: "meeting-v1".into(),
        }
    }

    fn response(evidence: EvidenceReference) -> GenerationResponse {
        GenerationResponse::Completed {
            schema_version: 1,
            request_id: "request-1".into(),
            principal_transcript_revision: 4,
            result: GeneratedRecord::Meeting {
                summary: "Grounded".into(),
                decisions: vec![Finding {
                    label: "Decision".into(),
                    detail: "Proceed".into(),
                    evidence: vec![evidence],
                }],
                action_items: vec![],
                key_points: vec![],
            },
        }
    }

    #[test]
    fn accepts_only_current_resolvable_playable_evidence() {
        assert!(validate_completed(
            &request(),
            &response(EvidenceReference {
                block_id: "b1".into(),
                timestamp_ms: 1500
            })
        )
        .is_ok());
    }

    #[test]
    fn rejects_missing_non_playable_and_stale_evidence() {
        for evidence in [
            EvidenceReference {
                block_id: "missing".into(),
                timestamp_ms: 1500,
            },
            EvidenceReference {
                block_id: "b1".into(),
                timestamp_ms: 999,
            },
            EvidenceReference {
                block_id: "b1".into(),
                timestamp_ms: 2001,
            },
        ] {
            assert_eq!(
                validate_completed(&request(), &response(evidence)),
                Err(VerifiableRecordError::InvalidOutput)
            );
        }
        let mut stale = response(EvidenceReference {
            block_id: "b1".into(),
            timestamp_ms: 1500,
        });
        if let GenerationResponse::Completed {
            principal_transcript_revision,
            ..
        } = &mut stale
        {
            *principal_transcript_revision = 3;
        }
        assert_eq!(
            validate_completed(&request(), &stale),
            Err(VerifiableRecordError::StaleRevision)
        );
    }

    #[test]
    fn rejects_fields_from_another_fixed_record_type() {
        let mut interview = request();
        interview.record_type = RecordType::Interview;
        assert_eq!(
            validate_completed(
                &interview,
                &response(EvidenceReference {
                    block_id: "b1".into(),
                    timestamp_ms: 1500
                })
            ),
            Err(VerifiableRecordError::InvalidOutput)
        );
    }

    #[test]
    fn rejects_excessive_generated_cardinality_and_string_bytes() {
        let evidence = EvidenceReference {
            block_id: "b1".into(),
            timestamp_ms: 1500,
        };
        let mut excessive = response(evidence.clone());
        if let GenerationResponse::Completed { result, .. } = &mut excessive {
            *result = GeneratedRecord::Meeting {
                summary: "x".repeat(100_000),
                decisions: (0..25)
                    .map(|_| Finding {
                        label: "Decision".into(),
                        detail: "Proceed".into(),
                        evidence: vec![evidence.clone()],
                    })
                    .collect(),
                action_items: vec![],
                key_points: vec![],
            };
        }
        assert_eq!(
            validate_completed(&request(), &excessive),
            Err(VerifiableRecordError::InvalidOutput)
        );
    }
}
