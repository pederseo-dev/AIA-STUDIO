use aia_core::Agent;
use serde::{Deserialize, Serialize};

/// Configuración global de IA del usuario. Se editará desde el panel de
/// configuración de IA (pendiente de diseño).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiSettings {
    pub providers: Vec<ProviderConfig>,
    /// Una entrada por agente; cada agente tiene su propio modelo y skills.
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
    /// Cualquier API compatible con el formato de OpenAI (incluye servidores locales).
    OpenAiCompatible,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Identificador elegido por el usuario, por ejemplo "claude" o "local".
    pub id: String,
    pub kind: ProviderKind,
    /// `None` usa la URL por defecto del proveedor.
    pub base_url: Option<String>,
    /// Nombre de la variable de entorno con la API key. La key nunca se guarda
    /// en el archivo de configuración.
    pub api_key_env: Option<String>,
}

/// Un modelo concreto de un proveedor configurado.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub provider_id: String,
    pub model: String,
}

/// Una skill (instrucciones y conocimiento especializado) activable por agente.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRef {
    pub id: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAiConfig {
    pub agent: Agent,
    /// `None` = el agente está desactivado.
    pub model: Option<ModelRef>,
    pub skills: Vec<SkillRef>,
    /// Instrucciones extra del usuario para este agente.
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
