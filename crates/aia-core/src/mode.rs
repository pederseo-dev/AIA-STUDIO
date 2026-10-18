use serde::{Deserialize, Serialize};

/// Los modos de trabajo de la aplicación:
///
/// - `Design`: construir el personaje (dibujo vectorial y, más adelante, huesos y pesos).
///   Todo lo que se hace modifica la base.
/// - `Animation`: animar; los cambios crean keyframes y nunca tocan la base.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Design,
    Animation,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Design, Mode::Animation];

    pub fn title(self) -> &'static str {
        match self {
            Mode::Design => "Design",
            Mode::Animation => "Animation",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Mode::Design => "Construir el personaje: dibujo vectorial, capas y huesos",
            Mode::Animation => "Animar: keyframes y curvas, sin tocar la base",
        }
    }
}

/// Agentes de IA especializados. Cada uno tiene su propia conversación,
/// herramientas, modelo y skills; el documento es la memoria compartida.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Agent {
    Draw,
    Rigging,
    Animation,
}

impl Agent {
    pub const ALL: [Agent; 3] = [Agent::Draw, Agent::Rigging, Agent::Animation];

    pub fn title(self) -> &'static str {
        match self {
            Agent::Draw => "Draw",
            Agent::Rigging => "Rigging",
            Agent::Animation => "Animation",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Agent::Draw => "Crea y edita formas, estilos y capas",
            Agent::Rigging => "Arma el esqueleto: huesos, jerarquía y pesos",
            Agent::Animation => "Anima: poses, keyframes y curvas",
        }
    }

    /// El modo en el que el agente puede trabajar. En Animation solo trabaja el
    /// agente de animación, así la IA tampoco puede modificar la base mientras se anima.
    pub fn mode(self) -> Mode {
        match self {
            Agent::Draw | Agent::Rigging => Mode::Design,
            Agent::Animation => Mode::Animation,
        }
    }

    pub fn available_in(self, mode: Mode) -> bool {
        self.mode() == mode
    }

    /// Agente sugerido para un modo cuando no hay una pista más específica
    /// (por ejemplo, la herramienta activa).
    pub fn default_for(mode: Mode) -> Agent {
        match mode {
            Mode::Design => Agent::Draw,
            Mode::Animation => Agent::Animation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_has_an_available_default_agent() {
        for mode in Mode::ALL {
            assert!(Agent::default_for(mode).available_in(mode));
        }
    }

    #[test]
    fn only_animation_agent_works_in_animation_mode() {
        let agents: Vec<_> = Agent::ALL.into_iter().filter(|a| a.available_in(Mode::Animation)).collect();
        assert_eq!(agents, vec![Agent::Animation]);
    }
}
