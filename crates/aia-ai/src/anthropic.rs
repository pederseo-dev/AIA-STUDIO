//! Claude through the Messages API (`POST /v1/messages`) over HTTP, with streaming (SSE).
//! There is no official Rust SDK, so this follows the raw HTTP documentation.

use std::io::{BufRead, BufReader, Read};

use aia_core::{ChatRole, RawContent};
use serde_json::{Value, json};

use crate::config::ProviderConfig;
use crate::provider::{ApiError, Reply, Request, Stop, Usage, http, response_error, retry_after};

pub(crate) const API_VERSION: &str = "2026-10-21";
const PROVIDER: &str = "anthropic";
/// Ceiling per answer (not a target): streaming allows a generous one.
const MAX_TOKENS: u32 = 64_000;
/// Models that support the server-side fallback when a safety classifier declines.
const FALLBACK_MODELS: [&str; 3] = ["claude-sonnet-5-5", "claude-opus-5-5", "claude-fable-5-1"];

fn messages(request: &Request) -> Vec<Value> {
    request
        .messages
        .iter()
        .filter(|m| !m.text.is_empty() || m.raw.is_some())
        .map(|m| {
            let role = match m.role {
                ChatRole::User => "user",
                ChatRole::Assistant => "assistant",
            };
            // Our own earlier answers go back exactly as they came (thinking blocks included).
            let content = match &m.raw {
                Some(raw) if raw.provider == PROVIDER => raw.content.clone(),
                _ => json!(m.text),
            };
            json!({ "role": role, "content": content })
        })
        .collect()
}

fn body(request: &Request) -> Value {
    let mut body = json!({
        "model": request.model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "messages": messages(request),
        // Chat-like use: low effort keeps answers quick and cheap.
        "output_config": { "effort": "low" },
        // Caches the stable prefix (system prompt and earlier messages).
        "cache_control": { "type": "ephemeral" },
    });
    if !request.system.is_empty() {
        body["system"] = json!(request.system);
    }
    if FALLBACK_MODELS.contains(&request.model) {
        body["fallbacks"] = json!("default");
    }
    body
}

pub fn stream(provider: &ProviderConfig, key: &str, request: &Request, on_text: &mut dyn FnMut(&str)) -> Result<Reply, ApiError> {
    let url = format!("{}/v1/messages", provider.base_url().trim_end_matches('/'));
    let mut call = http()
        .post(&url)
        .header("x-api-key", key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json");
    if FALLBACK_MODELS.contains(&request.model) {
        call = call.header("anthropic-beta", "server-side-fallback-2026-07-01");
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

    let mut answer = Answer::default();
    for line in BufReader::new(reader).lines() {
        let line = line.map_err(|e| e.to_string())?;
        let Some(data) = line.strip_prefix("data:") else { continue };
        let Ok(event) = serde_json::from_str::<Value>(data.trim()) else { continue };
        let overloaded = event["type"] == "error" && event["error"]["type"] == "overloaded_error";
        match answer.handle(&event, on_text) {
            Ok(true) => break,
            Ok(false) => {}
            // Overloaded before any text: it can be retried.
            Err(message) if overloaded && answer.blocks.is_empty() => {
                return Err(ApiError {
                    message,
                    retry: Some(None),
                });
            }
            Err(message) => return Err(message.into()),
        }
    }
    Ok(answer.finish(request.model))
}

/// The answer being assembled from the stream events.
#[derive(Default)]
struct Answer {
    /// Content blocks as the API sent them, completed by the deltas.
    blocks: Vec<Value>,
    /// Partial JSON of tool inputs, per block.
    tool_json: Vec<String>,
    usage: Usage,
    stop: Option<Stop>,
}

impl Answer {
    /// Applies one stream event. Returns `true` at the end of the message.
    fn handle(&mut self, event: &Value, on_text: &mut dyn FnMut(&str)) -> Result<bool, String> {
        let index = event["index"].as_u64().unwrap_or(0) as usize;
        match event["type"].as_str().unwrap_or_default() {
            "message_start" => {
                let usage = &event["message"]["usage"];
                self.usage.input_tokens = usage["input_tokens"].as_u64().unwrap_or(0);
                self.usage.cache_read_tokens = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                self.usage.cache_write_tokens = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
            }
            "content_block_start" => {
                if self.blocks.len() <= index {
                    self.blocks.resize(index + 1, Value::Null);
                    self.tool_json.resize(index + 1, String::new());
                }
                self.blocks[index] = event["content_block"].clone();
            }
            "content_block_delta" => {
                let Some(block) = self.blocks.get_mut(index) else { return Ok(false) };
                let delta = &event["delta"];
                match delta["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        let text = delta["text"].as_str().unwrap_or_default();
                        append(block, "text", text);
                        on_text(text);
                    }
                    "thinking_delta" => append(block, "thinking", delta["thinking"].as_str().unwrap_or_default()),
                    "signature_delta" => block["signature"] = delta["signature"].clone(),
                    "input_json_delta" => self.tool_json[index].push_str(delta["partial_json"].as_str().unwrap_or_default()),
                    _ => {}
                }
            }
            "content_block_stop" => {
                // Tool inputs arrive as JSON text; they are parsed once complete.
                if let Some(json) = self.tool_json.get(index).filter(|j| !j.is_empty()) {
                    self.blocks[index]["input"] = serde_json::from_str(json).unwrap_or(Value::Null);
                }
            }
            "message_delta" => {
                if let Some(output) = event["usage"]["output_tokens"].as_u64() {
                    self.usage.output_tokens = output;
                }
                self.stop = event["delta"]["stop_reason"].as_str().map(|reason| match reason {
                    "end_turn" | "stop_sequence" => Stop::Done,
                    "max_tokens" => Stop::MaxTokens,
                    "refusal" => Stop::Refusal(event["delta"]["stop_details"]["explanation"].as_str().map(str::to_string)),
                    other => Stop::Other(other.to_string()),
                });
            }
            "message_stop" => return Ok(true),
            "error" => {
                let message = event["error"]["message"].as_str().unwrap_or("unknown error");
                return Err(format!("Anthropic: {message}"));
            }
            _ => {}
        }
        Ok(false)
    }

    fn finish(self, model: &str) -> Reply {
        let text = self
            .blocks
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let blocks: Vec<Value> = self.blocks.into_iter().filter(|b| !b.is_null()).collect();
        Reply {
            text,
            raw: Some(RawContent {
                provider: PROVIDER.into(),
                model: model.into(),
                content: Value::Array(blocks),
            }),
            usage: self.usage,
            stop: self.stop.unwrap_or(Stop::Done),
        }
    }
}

fn append(block: &mut Value, field: &str, text: &str) {
    let mut current = block[field].as_str().unwrap_or_default().to_string();
    current.push_str(text);
    block[field] = Value::String(current);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_events_rebuild_text_and_thinking() {
        let events = [
            json!({"type": "message_start", "message": {"usage": {"input_tokens": 10, "cache_read_input_tokens": 4}}}),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "hmm"}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "sig"}}),
            json!({"type": "content_block_start", "index": 1, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "Hel"}}),
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "lo"}}),
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 7}}),
            json!({"type": "message_stop"}),
        ];
        let mut answer = Answer::default();
        let mut streamed = String::new();
        for event in &events {
            answer.handle(event, &mut |t| streamed.push_str(t)).unwrap();
        }
        let reply = answer.finish("claude-sonnet-5-5");
        assert_eq!(reply.text, "Hello");
        assert_eq!(streamed, "Hello");
        assert_eq!(reply.stop, Stop::Done);
        assert_eq!(reply.usage.output_tokens, 7);
        let raw = reply.raw.unwrap().content;
        // The thinking block keeps its signature, so it can be sent back unchanged.
        assert_eq!(raw[0]["signature"], "sig");
        assert_eq!(raw[0]["thinking"], "hmm");
    }
}
