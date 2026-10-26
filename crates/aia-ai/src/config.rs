use aia_core::Agent;
use serde::{Deserialize, Serialize};

/// Model used by default: a mid-tier one, to keep usage moderate.
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-5-5";

/// The user's global AI settings (saved with the app, never in the project).
/// API keys are not stored here: see [`crate::keys`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiSettings {
    pub providers: Vec<ProviderConfig>,
    /// One entry per agent; each agent has its own model and skills.
    pub agents: Vec<AgentAiConfig>,
}

impl Default for AiSettings {
    fn default() -> Self {
        let anthropic = ProviderConfig::new("anthropic", ProviderKind::Anthropic);
        let model = ModelRef {
            provider_id: anthropic.id.clone(),
            model: DEFAULT_ANTHROPIC_MODEL.into(),
        };
        Self {
            providers: vec![anthropic],
            agents: Agent::ALL
                .into_iter()
                .map(|agent| AgentAiConfig {
                    model: Some(model.clone()),
                    ..AgentAiConfig::new(agent)
                })
                .collect(),
        }
    }
}

impl AiSettings {
    pub fn agent(&self, agent: Agent) -> Option<&AgentAiConfig> {
        self.agents.iter().find(|a| a.agent == agent)
    }

    pub fn agent_mut(&mut self, agent: Agent) -> &mut AgentAiConfig {
        if let Some(i) = self.agents.iter().position(|a| a.agent == agent) {
            return &mut self.agents[i];
        }
        self.agents.push(AgentAiConfig::new(agent));
        self.agents.last_mut().expect("just pushed")
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// Provider and model of an agent, if it has one configured.
    pub fn agent_model(&self, agent: Agent) -> Option<(&ProviderConfig, &str)> {
        let model = self.agent(agent)?.model.as_ref()?;
        Some((self.provider(&model.provider_id)?, model.model.as_str()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    /// Any API compatible with the OpenAI format: OpenAI, Gemini, OpenRouter, Groq,
    /// DeepSeek, and local servers such as Ollama or LM Studio.
    OpenAiCompatible,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 2] = [ProviderKind::Anthropic, ProviderKind::OpenAiCompatible];

    pub fn title(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "Anthropic (Claude)",
            ProviderKind::OpenAiCompatible => "OpenAI-compatible",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "https://api.anthropic.com",
            ProviderKind::OpenAiCompatible => "https://api.openai.com/v1",
        }
    }

    /// Environment variable read when no key is in the system credential store.
    pub fn default_key_env(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "ANTHROPIC_API_KEY",
            ProviderKind::OpenAiCompatible => "OPENAI_API_KEY",
        }
    }

    /// Models offered in the settings (the user can type any other).
    pub fn suggested_models(self) -> &'static [&'static str] {
        match self {
            ProviderKind::Anthropic => &["claude-sonnet-5-5", "claude-opus-5-5", "claude-haiku-5-5"],
            ProviderKind::OpenAiCompatible => &[],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Identifier chosen by the user, for example "anthropic" or "local". It also names the
    /// key in the system credential store.
    pub id: String,
    pub kind: ProviderKind,
    /// `None` uses the provider's default URL.
    pub base_url: Option<String>,
    /// Environment variable with the API key, used when the credential store has none.
    /// `None` uses the kind's default (for example `ANTHROPIC_API_KEY`).
    pub api_key_env: Option<String>,
}

impl ProviderConfig {
    pub fn new(id: impl Into<String>, kind: ProviderKind) -> Self {
        Self {
            id: id.into(),
            kind,
            base_url: None,
            api_key_env: None,
        }
    }

    pub fn base_url(&self) -> &str {
        self.base_url.as_deref().unwrap_or(self.kind.default_base_url())
    }

    pub fn key_env(&self) -> &str {
        self.api_key_env.as_deref().unwrap_or(self.kind.default_key_env())
    }
}

/// A ready-made provider for the settings: URL and key variable already filled in.
pub struct Preset {
    pub name: &'static str,
    pub id: &'static str,
    pub kind: ProviderKind,
    pub base_url: &'static str,
    /// `None` = no key needed (local servers).
    pub key_env: Option<&'static str>,
    /// Free or local, with a short note about its limits.
    pub free: Option<&'static str>,
}

impl Preset {
    pub const ALL: [Preset; 8] = [
        Preset {
            name: "Anthropic (Claude)",
            id: "anthropic",
            kind: ProviderKind::Anthropic,
            base_url: "https://api.anthropic.com",
            key_env: Some("ANTHROPIC_API_KEY"),
            free: None,
        },
        Preset {
            name: "NVIDIA Build (NIM)",
            id: "nvidia",
            kind: ProviderKind::OpenAiCompatible,
            base_url: "https://integrate.api.nvidia.com/v1",
            key_env: Some("NVIDIA_API_KEY"),
            free: Some("free key at build.nvidia.com: DeepSeek, Qwen, Kimi, Llama, Nemotron… (rate limited)"),
        },
        Preset {
            name: "OpenRouter",
            id: "openrouter",
            kind: ProviderKind::OpenAiCompatible,
            base_url: "https://openrouter.ai/api/v1",
            key_env: Some("OPENROUTER_API_KEY"),
            free: Some("models ending in \":free\" cost nothing (about 20 requests/min, 50/day)"),
        },
        Preset {
            name: "Google Gemini",
            id: "gemini",
            kind: ProviderKind::OpenAiCompatible,
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
            key_env: Some("GEMINI_API_KEY"),
            free: Some("free tier with the Flash models (key at aistudio.google.com)"),
        },
        Preset {
            name: "Groq",
            id: "groq",
            kind: ProviderKind::OpenAiCompatible,
            base_url: "https://api.groq.com/openai/v1",
            key_env: Some("GROQ_API_KEY"),
            free: Some("free tier with open models, very fast (rate limited)"),
        },
        Preset {
            name: "OpenAI",
            id: "openai",
            kind: ProviderKind::OpenAiCompatible,
            base_url: "https://api.openai.com/v1",
            key_env: Some("OPENAI_API_KEY"),
            free: None,
        },
        Preset {
            name: "Ollama (local)",
            id: "ollama",
            kind: ProviderKind::OpenAiCompatible,
            base_url: "http://localhost:11434/v1",
            key_env: None,
            free: Some("runs on this computer: no internet or limits, but needs a good GPU"),
        },
        Preset {
            name: "LM Studio (local)",
            id: "lmstudio",
            kind: ProviderKind::OpenAiCompatible,
            base_url: "http://localhost:1234/v1",
            key_env: None,
            free: Some("runs on this computer: no internet or limits, but needs a good GPU"),
        },
    ];

    pub fn config(&self) -> ProviderConfig {
        ProviderConfig {
            id: self.id.into(),
            kind: self.kind,
            base_url: Some(self.base_url.into()),
            api_key_env: self.key_env.map(Into::into),
        }
    }

    /// The preset a configured provider was made from (by id), for its notes.
    pub fn find(id: &str) -> Option<&'static Preset> {
        Preset::ALL.iter().find(|p| p.id == id)
    }
}

/// A specific model from a configured provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub provider_id: String,
    pub model: String,
}

/// A skill (instructions and specialized knowledge) that can be enabled per agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRef {
    pub id: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAiConfig {
    pub agent: Agent,
    /// `None` = the agent is disabled.
    pub model: Option<ModelRef>,
    pub skills: Vec<SkillRef>,
    /// Extra user instructions for this agent.
    pub system_prompt: Option<String>,
}

impl AgentAiConfig {
    pub fn new(agent: Agent) -> Self {
        Self {
            agent,
            model: None,
            skills: Vec::new(),
            system_prompt: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_gives_every_agent_the_mid_tier_model() {
        let settings = AiSettings::default();
        for agent in Agent::ALL {
            let (provider, model) = settings.agent_model(agent).unwrap();
            assert_eq!(provider.kind, ProviderKind::Anthropic);
            assert_eq!(model, DEFAULT_ANTHROPIC_MODEL);
        }
    }
}
