//! Panel de chat (derecha): un solo panel con un selector de agentes especializados.
//!
//! Cada agente tiene su propia conversación. El selector sigue automáticamente al
//! modo y a la herramienta activa; elegir un agente a mano lo mantiene hasta que
//! cambie la sugerencia (otro modo u otra herramienta).

use aia_core::{Agent, ChatRole, Chats, Mode};
use eframe::egui;
use egui_phosphor::regular as icons;

use crate::modes;

pub struct ChatPanel {
    agent: Agent,
    /// Última sugerencia aplicada; si cambia, el selector salta a la nueva.
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

    /// Selección automática: se llama cada frame con la sugerencia del modo activo.
    pub fn follow_suggestion(&mut self, mode: Mode, suggestion: Agent) {
        if self.last_suggestion != Some(suggestion) || !self.agent.available_in(mode) {
            self.agent = suggestion;
            self.last_suggestion = Some(suggestion);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mode: Mode, chats: &Chats) {
        ui.add_space(6.0);
        ui.strong(format!("{} Chat IA", icons::CHAT_CIRCLE_DOTS));
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
                    ui.weak(format!("Todavía no hay conversación con el agente {}.", self.agent.title()));
                }
                for message in messages {
                    let who = match message.role {
                        ChatRole::User => "Vos",
                        ChatRole::Assistant => self.agent.title(),
                    };
                    ui.label(egui::RichText::new(who).strong());
                    ui.label(&message.text);
                    ui.add_space(6.0);
                }
            });

        ui.separator();
        // Sin proveedor configurado todavía: la entrada queda visible pero deshabilitada.
        ui.add_enabled(
            false,
            egui::TextEdit::multiline(&mut self.draft)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text(format!("Pedile algo a {}…", self.agent.title())),
        )
        .on_disabled_hover_text("Configurá un modelo para este agente (configuración de IA: próximamente)");
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
                        "Disponible en el modo {}: cambiá de modo para usarlo",
                        agent.mode().title()
                    ));
                if response.clicked() {
                    self.agent = agent;
                }
            }
        });
    }
}
