use serde::{Deserialize, Serialize};

/// Los paneles principales de la aplicación.
///
/// Se usa para la UI (pestañas acoplables) y para asociar a cada panel
/// su propia configuración de IA (modelo, skills, herramientas).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelId {
    Drawing,
    Bones,
    Timeline,
}

impl PanelId {
    pub const ALL: [PanelId; 3] = [PanelId::Drawing, PanelId::Bones, PanelId::Timeline];

    pub fn title(self) -> &'static str {
        match self {
            PanelId::Drawing => "Dibujo",
            PanelId::Bones => "Huesos",
            PanelId::Timeline => "Animación",
        }
    }
}
