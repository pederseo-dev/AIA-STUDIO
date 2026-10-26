//! AI layer of AIA Studio: settings, API keys, model providers behind a common interface,
//! and chat requests on a worker thread.
//!
//! Next: the scene view for the AI and each agent's high-level tools, which translate into
//! [`aia_core::Command`]s.

mod anthropic;
pub mod chat;
pub mod config;
pub mod keys;
mod openai;
pub mod prompts;
pub mod provider;

pub use chat::{ChatEvent, ChatJob};
pub use config::{AgentAiConfig, AiSettings, DEFAULT_ANTHROPIC_MODEL, ModelRef, Preset, ProviderConfig, ProviderKind, SkillRef};
pub use keys::KeySource;
pub use provider::{Reply, Stop, Usage};
