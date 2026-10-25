//! AI foundation of AIA Studio.
//!
//! For now it only defines the shape of the settings: providers, models and skills
//! assigned to each agent. Chat and tool execution (high-level tools that translate
//! into [`aia_core::Command`]s) come later.

pub mod config;

pub use config::{AiSettings, AgentAiConfig, ModelRef, ProviderConfig, ProviderKind, SkillRef};
