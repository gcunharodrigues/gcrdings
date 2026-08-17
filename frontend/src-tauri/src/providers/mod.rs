pub mod commands;
pub mod keychain;
pub mod repository;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const AGENT_HANDOFF_TASK: &str = "agent_handoff";

pub(crate) fn credential_account(provider: &str, task: &str) -> String {
    format!("transfer:{task}:{provider}")
}

pub(crate) fn credential_generation_account(
    configuration: &ProviderConfiguration,
    generation: i64,
) -> String {
    use sha2::{Digest, Sha256};
    let binding = format!("{}\n{}", configuration.endpoint, configuration.model);
    let digest = format!("{:x}", Sha256::digest(binding.as_bytes()));
    format!(
        "transfer:{}:{}:g{}:{}",
        configuration.task,
        configuration.provider,
        generation,
        &digest[..16]
    )
}

pub(crate) fn summary_credential_account(provider: &str) -> String {
    format!("summary:{provider}")
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfiguration {
    pub provider: String,
    pub display_name: String,
    pub endpoint: String,
    pub model: String,
    pub task: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAuthorizationStatus {
    pub provider: String,
    pub task: String,
    pub endpoint: String,
    pub model: String,
    pub enabled: bool,
    pub credential_present: bool,
    pub credential_mask: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreview {
    pub transfer_id: String,
    pub preview_digest: String,
    pub provider: String,
    pub provider_display_name: String,
    pub session_title: String,
    pub purpose: String,
    pub task: String,
    pub data_types: Vec<String>,
    pub principal_transcript_revision: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTransferOutcome {
    pub transfer_id: String,
    pub provider: String,
    pub task: String,
    pub status: String,
    pub result: Option<serde_json::Value>,
    pub error_code: Option<String>,
}

#[derive(Debug, thiserror::Error, Serialize, PartialEq, Eq)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ProviderError {
    #[error("provider configuration is invalid")]
    InvalidConfiguration,
    #[error("provider credential is unavailable")]
    CredentialUnavailable,
    #[error("provider task is disabled")]
    TaskDisabled,
    #[error("provider preview is stale")]
    StalePreview,
    #[error("provider confirmation was already consumed")]
    AlreadyConsumed,
    #[error("provider request failed")]
    RequestFailed,
    #[error("provider may have accepted the request; retry is disabled")]
    OutcomeUnknown,
    #[error("provider response is invalid")]
    InvalidResponse,
    #[error("provider storage is unavailable")]
    Storage,
}

pub struct SecretString(Vec<u8>);

impl SecretString {
    pub fn new(value: impl AsRef<[u8]>) -> Self {
        Self(value.as_ref().to_vec())
    }

    pub(crate) fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretString([REDACTED])")
    }
}

pub trait CredentialStore: Send + Sync {
    fn save(&self, provider: &str, secret: &str) -> Result<(), ProviderError>;
    fn read(&self, provider: &str) -> Result<Option<SecretString>, ProviderError>;
    fn remove(&self, provider: &str) -> Result<(), ProviderError>;
}

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn test(
        &self,
        configuration: &ProviderConfiguration,
        credential: &SecretString,
    ) -> Result<(), ProviderError>;

    async fn send(
        &self,
        configuration: &ProviderConfiguration,
        credential: &SecretString,
        transfer_id: &str,
        payload: &str,
    ) -> Result<serde_json::Value, ProviderError>;
}
