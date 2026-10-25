//! Chat panel (right): a single panel with a selector of specialized agents.
//!
//! Each agent has its own conversation. The selector follows the mode and the active
//! tool automatically; picking an agent by hand keeps it until the suggestion changes
//! (another mode or another tool).

use aia_core::{Agent, ChatRole, Chats, Mode};
use eframe::egui;
use egui_phosphor::regular as icons;

use crate::modes;

pub struct ChatPanel {
    agent: Agent,
    /// Last applied suggestion; when it changes, the selector jumps to the new one.
    last_suggestion: Option<Agent>,
    draft: String,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            agent: Agent::default_for(Mode::default()),
            last_suggestion: None,
            draft: String::new(),
        }
    }
}

impl ChatPanel {
    pub fn agent(&self) -> Agent {
        self.agent
    }

    /// Automatic selection: called every frame with the active mode's suggestion.
    pub fn follow_suggestion(&mut self, mode: Mode, suggestion: Agent) {
        if self.last_suggestion != Some(suggestion) || !self.agent.available_in(mode) {
            self.agent = suggestion;
            self.last_suggestion = Some(suggestion);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mode: Mode, chats: &Chats) {
        ui.add_space(6.0);
        ui.strong(format!("{} AI Chat", icons::CHAT_CIRCLE_DOTS));
        ui.add_space(4.0);
        self.agent_selector(ui, mode);
        ui.weak(self.agent.description());
        ui.separator();

        let input_height = 64.0;
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - input_height).max(0.0))
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let messages = chats.messages(self.agent);
                if messages.is_empty() {
                    ui.weak(format!("No conversation with the {} agent yet.", self.agent.title()));
                }
                for message in messages {
                    let who = match message.role {
                        ChatRole::User => "You",
                        ChatRole::Assistant => self.agent.title(),
                    };
                    ui.label(egui::RichText::new(who).strong());
                    ui.label(&message.text);
                    ui.add_space(6.0);
                }
            });

        ui.separator();
        // No provider configured yet: the input is visible but disabled.
        ui.add_enabled(
            false,
            egui::TextEdit::multiline(&mut self.draft)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text(format!("Ask {} something…", self.agent.title())),
        )
        .on_disabled_hover_text("Configure a model for this agent (AI settings: coming soon)");
    }

    fn agent_selector(&mut self, ui: &mut egui::Ui, mode: Mode) {
        ui.horizontal_wrapped(|ui| {
            for agent in Agent::ALL {
                let text = format!("{} {}", modes::agent_icon(agent), agent.title());
                let available = agent.available_in(mode);
                let response = ui
                    .add_enabled(available, egui::Button::selectable(self.agent == agent, text))
                    .on_hover_text(agent.description())
                    .on_disabled_hover_text(format!(
                        "Available in {} mode: switch modes to use it",
                        agent.mode().title()
                    ));
                if response.clicked() {
                    self.agent = agent;
                }
            }
        });
    }
}
