//! API keys: in the system credential store (Windows Credential Manager, macOS Keychain,
//! Linux Secret Service), or in an environment variable. Never in a settings or project file.

use crate::config::ProviderConfig;

/// Service name under which the keys are stored; the provider id is the account.
const SERVICE: &str = "AIA Studio";

/// Where a provider's key comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeySource {
    CredentialStore,
    Env(String),
    Missing,
}

fn entry(provider_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, provider_id).map_err(|e| e.to_string())
}

fn from_store(provider_id: &str) -> Option<String> {
    entry(provider_id).ok()?.get_password().ok().filter(|k| !k.is_empty())
}

fn from_env(provider: &ProviderConfig) -> Option<String> {
    std::env::var(provider.key_env()).ok().filter(|k| !k.is_empty())
}

/// The key to use: the credential store first, then the environment variable.
pub fn get(provider: &ProviderConfig) -> Option<String> {
    from_store(&provider.id).or_else(|| from_env(provider))
}

pub fn source(provider: &ProviderConfig) -> KeySource {
    if from_store(&provider.id).is_some() {
        KeySource::CredentialStore
    } else if from_env(provider).is_some() {
        KeySource::Env(provider.key_env().to_string())
    } else {
        KeySource::Missing
    }
}

pub fn store(provider_id: &str, key: &str) -> Result<(), String> {
    entry(provider_id)?.set_password(key.trim()).map_err(|e| e.to_string())
}

pub fn remove(provider_id: &str) -> Result<(), String> {
    match entry(provider_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
