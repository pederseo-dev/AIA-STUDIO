use serde::{Deserialize, Serialize};

/// Los modos de trabajo de la aplicación. Cada modo es específico:
///
/// - `Draw`: crear y editar la geometría base y su estilo.
/// - `Bones`: armar el esqueleto y vincularlo con las formas.
/// - `Timeline`: animar; los cambios crean keyframes y nunca tocan la geometría base.
///
/// Cada modo tiene además su propio chat de IA (modelo, skills, herramientas).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Draw,
    Bones,
    Timeline,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Draw, Mode::Bones, Mode::Timeline];

    pub fn title(self) -> &'static str {
        match self {
            Mode::Draw => "Draw",
            Mode::Bones => "Bones",
            Mode::Timeline => "Timeline",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Mode::Draw => "Dibujo vectorial: formas, estilo y capas",
            Mode::Bones => "Rigging 2D: jerarquía de huesos, IK y pesos",
            Mode::Timeline => "Animación: keyframes y curvas",
        }
    }
}
