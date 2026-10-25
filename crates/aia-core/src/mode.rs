use serde::{Deserialize, Serialize};

/// The application's work modes:
///
/// - `Design`: build the character (vector drawing and, later, bones and weights).
///   Everything done here modifies the base.
/// - `Animation`: animate; changes create keyframes and never touch the base.
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
            Mode::Design => "Build the character: vector drawing, layers and bones",
            Mode::Animation => "Animate: keyframes and curves, without touching the base",
        }
    }
}

/// Specialized AI agents. Each one has its own conversation, tools, model and skills;
/// the document is the shared memory.
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
            Agent::Draw => "Creates and edits shapes, styles and layers",
            Agent::Rigging => "Builds the skeleton: bones, hierarchy and weights",
            Agent::Animation => "Animates: poses, keyframes and curves",
        }
    }

    /// The mode the agent can work in. In Animation only the animation agent works, so
    /// the AI cannot modify the base while animating either.
    pub fn mode(self) -> Mode {
        match self {
            Agent::Draw | Agent::Rigging => Mode::Design,
            Agent::Animation => Mode::Animation,
        }
    }

    pub fn available_in(self, mode: Mode) -> bool {
        self.mode() == mode
    }

    /// Suggested agent for a mode when there is no more specific hint
    /// (for example, the active tool).
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
