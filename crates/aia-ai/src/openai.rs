//! APIs compatible with OpenAI's Chat Completions (`POST {base}/chat/completions`), with
//! streaming: OpenAI, Gemini, OpenRouter, Groq, DeepSeek, Ollama, LM Studio…

use std::io::{BufRead, BufReader, Read};

use aia_core::ChatRole;
use serde_json::{Value, json};

use crate::config::ProviderConfig;
use crate::provider::{ApiError, Reply, Request, Stop, Usage, http, response_error, retry_after};

fn body(request: &Request) -> Value {
    let mut messages = Vec::new();
    if !request.system.is_empty() {
        messages.push(json!({ "role": "system", "content": request.system }));
    }
    for m in request.messages.iter().filter(|m| !m.text.is_empty()) {
        let role = match m.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        };
        messages.push(json!({ "role": role, "content": m.text }));
    }
    json!({
        "model": request.model,
        "stream": true,
        "stream_options": { "include_usage": true },
        "messages": messages,
    })
}

/// `key` is optional: local servers usually do not need one.
pub fn stream(provider: &ProviderConfig, key: Option<&str>, request: &Request, on_text: &mut dyn FnMut(&str)) -> Result<Reply, ApiError> {
    let url = format!("{}/chat/completions", provider.base_url().trim_end_matches('/'));
    let mut call = http().post(&url).header("content-type", "application/json");
    if let Some(key) = key {
        call = call.header("authorization", format!("Bearer {key}"));
    }
    let response = call.send_json(body(request)).map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let wait = retry_after(&response);
    let mut reader = response.into_body().into_reader();
    if status >= 400 {
        let mut text = String::new();
        reader.read_to_string(&mut text).map_err(|e| e.to_string())?;
        return Err(response_error(status, wait, &text));
    }

    let mut text = String::new();
    let mut usage = Usage::default();
    let mut stop = Stop::Done;
    for line in BufReader::new(reader).lines() {
        let line = line.map_err(|e| e.to_string())?;
        let Some(data) = line.strip_prefix("data:").map(str::trim) else { continue };
        if data == "[DONE]" {
            break;
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else { continue };
        if let Some(message) = chunk["error"]["message"].as_str() {
            return Err(message.to_string().into());
        }
        let choice = &chunk["choices"][0];
        if let Some(delta) = choice["delta"]["content"].as_str() {
            text.push_str(delta);
            on_text(delta);
        }
        match choice["finish_reason"].as_str() {
            Some("length") => stop = Stop::MaxTokens,
            Some("content_filter") => stop = Stop::Refusal(None),
            _ => {}
        }
        if let Some(u) = chunk["usage"].as_object() {
            usage.input_tokens = u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
            usage.output_tokens = u.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0);
        }
    }
    Ok(Reply {
        text,
        raw: None,
        usage,
        stop,
    })
}
