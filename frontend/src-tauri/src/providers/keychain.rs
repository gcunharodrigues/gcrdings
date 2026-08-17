use super::{CredentialStore, ProviderError, SecretString};
#[cfg(target_os = "macos")]
use security_framework::os::macos::keychain::SecKeychain;

const ITEM_NOT_FOUND: i32 = -25300;

pub const KEYCHAIN_SERVICE: &str = "com.gcrdings.provider";

pub struct MacKeychain {
    service: String,
    #[cfg(target_os = "macos")]
    keychain: Option<SecKeychain>,
}

impl Default for MacKeychain {
    fn default() -> Self {
        Self::new(KEYCHAIN_SERVICE)
    }
}

impl MacKeychain {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            #[cfg(target_os = "macos")]
            keychain: SecKeychain::default().ok(),
        }
    }

    #[cfg(all(test, target_os = "macos"))]
    fn with_keychain(service: impl Into<String>, keychain: SecKeychain) -> Self {
        Self {
            service: service.into(),
            keychain: Some(keychain),
        }
    }
}

impl CredentialStore for MacKeychain {
    fn save(&self, provider: &str, secret: &str) -> Result<(), ProviderError> {
        validate_account(provider)?;
        if secret.trim().is_empty() {
            return Err(ProviderError::InvalidConfiguration);
        }
        #[cfg(target_os = "macos")]
        {
            self.keychain
                .as_ref()
                .ok_or(ProviderError::Storage)?
                .set_generic_password(&self.service, provider, secret.as_bytes())
                .map_err(|_| ProviderError::Storage)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(ProviderError::Storage)
        }
    }

    fn read(&self, provider: &str) -> Result<Option<SecretString>, ProviderError> {
        validate_account(provider)?;
        #[cfg(target_os = "macos")]
        {
            match self
                .keychain
                .as_ref()
                .ok_or(ProviderError::Storage)?
                .find_generic_password(&self.service, provider)
            {
                Ok((secret, _)) => Ok(Some(SecretString::new(secret.as_ref()))),
                Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
                Err(_) => Err(ProviderError::Storage),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(ProviderError::Storage)
        }
    }

    fn remove(&self, provider: &str) -> Result<(), ProviderError> {
        validate_account(provider)?;
        #[cfg(target_os = "macos")]
        {
            match self
                .keychain
                .as_ref()
                .ok_or(ProviderError::Storage)?
                .find_generic_password(&self.service, provider)
            {
                Ok((_, item)) => {
                    item.delete();
                    Ok(())
                }
                Err(error) if error.code() == ITEM_NOT_FOUND => Ok(()),
                Err(_) => Err(ProviderError::Storage),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(ProviderError::Storage)
        }
    }
}

fn validate_account(account: &str) -> Result<(), ProviderError> {
    if account.is_empty()
        || account.len() > 128
        || !account
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
    {
        Err(ProviderError::InvalidConfiguration)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn synthetic_credential_can_be_saved_replaced_masked_and_removed_idempotently() {
        use security_framework::os::macos::keychain::CreateOptions;
        let directory = tempfile::tempdir().unwrap();
        let keychain = CreateOptions::new()
            .password("synthetic-wave6-keychain")
            .create(directory.path().join("provider-test.keychain"))
            .unwrap();
        let service = format!("com.gcrdings.test.{}", uuid::Uuid::new_v4());
        let provider = "synthetic-provider";
        let first = "synthetic-wave6-first";
        let replacement = "synthetic-wave6-replacement";
        let keychain = MacKeychain::with_keychain(service, keychain);

        let result = (|| {
            keychain.save(provider, first)?;
            let saved = keychain.read(provider)?.expect("saved credential");
            assert_eq!(saved.expose(), first.as_bytes());
            assert_eq!(format!("{saved:?}"), "SecretString([REDACTED])");

            keychain.save(provider, replacement)?;
            let replaced = keychain.read(provider)?.expect("replaced credential");
            assert_eq!(replaced.expose(), replacement.as_bytes());

            keychain.remove(provider)?;
            keychain.remove(provider)?;
            assert!(keychain.read(provider)?.is_none());
            Ok::<_, ProviderError>(())
        })();
        let _ = keychain.remove(provider);
        result.unwrap();
    }
}
