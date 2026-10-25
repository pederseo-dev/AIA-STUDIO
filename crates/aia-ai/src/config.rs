use aia_core::Agent;
use serde::{Deserialize, Serialize};

/// The user's global AI settings. Will be edited from the AI settings panel
/// (design pending).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiSettings {
    pub providers: Vec<ProviderConfig>,
    /// One entry per agent; each agent has its own model and skills.
    pub agents: Vec<AgentAiConfig>,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            agents: Agent::ALL.into_iter().map(AgentAiConfig::new).collect(),
        }
    }
}

impl AiSettings {
    pub fn agent(&self, agent: Agent) -> Option<&AgentAiConfig> {
        self.agents.iter().find(|a| a.agent == agent)
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    /// Any API compatible with the OpenAI format (includes local servers).
    OpenAiCompatible,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Identifier chosen by the user, for example "claude" or "local".
    pub id: String,
    pub kind: ProviderKind,
    /// `None` uses the provider's default URL.
    pub base_url: Option<String>,
    /// Name of the environment variable holding the API key. The key is never stored
    /// in the settings file.
    pub api_key_env: Option<String>,
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
    fn default_has_one_config_per_agent() {
        let settings = AiSettings::default();
        for agent in Agent::ALL {
            assert!(settings.agent(agent).is_some());
        }
    }
}
