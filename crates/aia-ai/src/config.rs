use aia_core::PanelId;
use serde::{Deserialize, Serialize};

/// Configuración global de IA del usuario. Se editará desde el panel de
/// configuración de IA (pendiente de diseño).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiSettings {
    pub providers: Vec<ProviderConfig>,
    /// Una entrada por panel; cada panel tiene su propio chat.
    pub panels: Vec<PanelAiConfig>,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            panels: PanelId::ALL.into_iter().map(PanelAiConfig::new).collect(),
        }
    }
}

impl AiSettings {
    pub fn panel(&self, panel: PanelId) -> Option<&PanelAiConfig> {
        self.panels.iter().find(|p| p.panel == panel)
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

/// Una skill (instrucciones y conocimiento especializado) activable por panel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRef {
    pub id: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelAiConfig {
    pub panel: PanelId,
    /// `None` = el chat de este panel está desactivado.
    pub model: Option<ModelRef>,
    pub skills: Vec<SkillRef>,
    /// Instrucciones extra del usuario para este panel.
    pub system_prompt: Option<String>,
}

impl PanelAiConfig {
    pub fn new(panel: PanelId) -> Self {
        Self {
            panel,
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
    fn default_has_one_config_per_panel() {
        let settings = AiSettings::default();
        for panel in PanelId::ALL {
            assert!(settings.panel(panel).is_some());
        }
    }
}
