//! Common interface to the model providers. The rest of the app does not know whether it
//! talks to Claude, OpenAI or a local server.

use std::time::Duration;

use aia_core::{ChatMessage, RawContent};

use crate::config::{ProviderConfig, ProviderKind};
use crate::{anthropic, openai};

/// One request to a model.
pub struct Request<'a> {
    pub model: &'a str,
    /// Fixed instructions for the agent (kept identical between requests so it can be cached).
    pub system: &'a str,
    pub messages: &'a [ChatMessage],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// Why the model stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    Done,
    /// The answer hit the token limit and is cut.
    MaxTokens,
    /// The model declined the request (with the provider's explanation, if any).
    Refusal(Option<String>),
    Other(String),
}

/// A finished answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub text: String,
    /// Provider content to send back in later requests (for example Claude's thinking blocks).
    pub raw: Option<RawContent>,
    pub usage: Usage,
    pub stop: Stop,
}

/// Attempts after the first one when the provider is rate limited or overloaded.
const RETRIES: u32 = 3;
/// Longest wait between attempts.
const MAX_WAIT: Duration = Duration::from_secs(60);

/// An error from a provider. `retry` is set when trying again later may work
/// (rate limits, overload), with the wait the provider asked for, if any.
#[derive(Debug)]
pub struct ApiError {
    pub message: String,
    pub retry: Option<Option<Duration>>,
}

impl From<String> for ApiError {
    fn from(message: String) -> Self {
        Self { message, retry: None }
    }
}

/// Sends `request` and streams the text through `on_text` as it arrives. When the provider
/// is rate limited it waits and retries, telling why through `on_status` (free tiers cut
/// quickly). Blocks until the answer is complete; call it from a worker thread.
pub fn stream(
    provider: &ProviderConfig,
    key: Option<&str>,
    request: &Request,
    on_text: &mut dyn FnMut(&str),
    on_status: &mut dyn FnMut(&str),
) -> Result<Reply, String> {
    let mut attempt = 0;
    loop {
        let result = match provider.kind {
            ProviderKind::Anthropic => match key {
                Some(key) => anthropic::stream(provider, key, request, on_text),
                None => Err(ApiError::from("Missing API key".to_string())),
            },
            ProviderKind::OpenAiCompatible => openai::stream(provider, key, request, on_text),
        };
        match result {
            Ok(reply) => return Ok(reply),
            Err(ApiError { retry: Some(wait), message }) if attempt < RETRIES => {
                let wait = wait.unwrap_or(Duration::from_secs(5 << attempt)).min(MAX_WAIT);
                on_status(&format!("{message} — retrying in {} s…", wait.as_secs()));
                std::thread::sleep(wait);
                attempt += 1;
            }
            Err(error) => return Err(error.message),
        }
    }
}

/// Models the provider offers (`GET /models`), sorted. Free model lists change often, so
/// the settings ask the provider instead of keeping a fixed list.
pub fn list_models(provider: &ProviderConfig, key: Option<&str>) -> Result<Vec<String>, String> {
    let base = provider.base_url().trim_end_matches('/');
    let call = match provider.kind {
        ProviderKind::Anthropic => http()
            .get(format!("{base}/v1/models?limit=1000"))
            .header("x-api-key", key.ok_or("Missing API key")?)
            .header("anthropic-version", anthropic::API_VERSION),
        ProviderKind::OpenAiCompatible => {
            let call = http().get(format!("{base}/models"));
            match key {
                Some(key) => call.header("authorization", format!("Bearer {key}")),
                None => call,
            }
        }
    };
    let response = call.call().map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let body = response.into_body().read_to_string().map_err(|e| e.to_string())?;
    if status >= 400 {
        return Err(error_message(status, &body));
    }
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let mut models: Vec<String> = json["data"]
        .as_array()
        .map(|data| data.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    models.sort();
    Ok(models)
}

/// HTTP client shared by the adapters: errors come back as responses so their message can be read.
pub(crate) fn http() -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).build().into()
}

/// Builds the error for a failed response (`{"error": {"message": ...}}` in both APIs).
/// 429 (rate limit), 503 and 529 (overloaded) can be retried, after `retry-after` seconds if given.
pub(crate) fn response_error(status: u16, retry_after: Option<u64>, body: &str) -> ApiError {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(300).collect());
    let retryable = matches!(status, 429 | 503 | 529);
    ApiError {
        message: format!("HTTP {status}: {message}"),
        retry: retryable.then(|| retry_after.map(Duration::from_secs)),
    }
}

fn error_message(status: u16, body: &str) -> String {
    response_error(status, None, body).message
}

/// The `retry-after` header in seconds, if present.
pub(crate) fn retry_after(response: &ureq::http::Response<ureq::Body>) -> Option<u64> {
    response.headers().get("retry-after")?.to_str().ok()?.trim().parse().ok()
}
