//! Chat panel (right): a single panel with a selector of specialized agents.
//!
//! Each agent has its own conversation. The selector follows the mode and the active
//! tool automatically; picking an agent by hand keeps it until the suggestion changes
//! (another mode or another tool). Answers stream in from a worker thread.

use aia_ai::{AiSettings, ChatEvent, ChatJob, Stop, Usage, keys, prompts};
use aia_core::{Agent, ChatMessage, ChatRole, Chats, Mode};
use eframe::egui;
use egui_phosphor::regular as icons;

use crate::modes;

/// An answer being received.
struct Pending {
    agent: Agent,
    job: ChatJob,
    text: String,
    /// What is happening while no text arrives (for example, waiting for a rate limit).
    status: Option<String>,
}

pub struct ChatPanel {
    agent: Agent,
    /// Last applied suggestion; when it changes, the selector jumps to the new one.
    last_suggestion: Option<Agent>,
    draft: String,
    pending: Option<Pending>,
    /// Error of the last request (not part of the conversation).
    error: Option<String>,
    /// Tokens used by the last answer.
    last_usage: Option<Usage>,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            agent: Agent::default_for(Mode::default()),
            last_suggestion: None,
            draft: String::new(),
            pending: None,
            error: None,
            last_usage: None,
        }
    }
}

impl ChatPanel {
    pub fn agent(&self) -> Agent {
        self.agent
    }

    /// `true` while an answer is arriving.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Automatic selection: called every frame with the active mode's suggestion.
    pub fn follow_suggestion(&mut self, mode: Mode, suggestion: Agent) {
        if self.last_suggestion != Some(suggestion) || !self.agent.available_in(mode) {
            self.agent = suggestion;
            self.last_suggestion = Some(suggestion);
        }
    }

    /// Receives what arrived from the worker thread. Returns `true` if the conversation changed.
    pub fn poll(&mut self, chats: &mut Chats) -> bool {
        let Some(pending) = &mut self.pending else { return false };
        let mut finished = None;
        for event in pending.job.poll() {
            match event {
                ChatEvent::Text(text) => {
                    pending.text.push_str(&text);
                    pending.status = None;
                }
                ChatEvent::Status(status) => pending.status = Some(status),
                ChatEvent::Finished(reply) => finished = Some(Ok(reply)),
                ChatEvent::Failed(error) => finished = Some(Err(error)),
            }
        }
        let Some(result) = finished else { return false };
        let agent = pending.agent;
        self.pending = None;
        match result {
            Ok(reply) => {
                self.last_usage = Some(reply.usage);
                self.error = match &reply.stop {
                    Stop::Done => None,
                    Stop::MaxTokens => Some("The answer was cut: it reached the token limit.".into()),
                    Stop::Refusal(why) => Some(format!("The model declined the request. {}", why.as_deref().unwrap_or(""))),
                    Stop::Other(reason) => Some(format!("The answer stopped: {reason}")),
                };
                chats.push(
                    agent,
                    ChatMessage {
                        role: ChatRole::Assistant,
                        text: reply.text,
                        raw: reply.raw,
                    },
                );
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    /// Returns `true` when the user asked to open the AI settings.
    pub fn ui(&mut self, ui: &mut egui::Ui, mode: Mode, chats: &mut Chats, settings: &AiSettings) -> bool {
        let mut open_settings = false;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.strong(format!("{} AI Chat", icons::CHAT_CIRCLE_DOTS));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                open_settings = ui.small_button(icons::GEAR).on_hover_text("AI Settings").clicked();
                let can_clear = !chats.messages(self.agent).is_empty() && !self.busy();
                if ui
                    .add_enabled(can_clear, egui::Button::new(icons::TRASH).small())
                    .on_hover_text("Clear this agent's conversation")
                    .clicked()
                {
                    chats.clear(self.agent);
                    self.error = None;
                }
            });
        });
        ui.add_space(4.0);
        self.agent_selector(ui, mode);
        let model = settings.agent_model(self.agent);
        ui.weak(match model {
            Some((_, model)) => format!("{} · {model}", self.agent.description()),
            None => self.agent.description().to_string(),
        });
        ui.separator();

        let input_height = 90.0;
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - input_height).max(0.0))
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| self.conversation(ui, chats));

        ui.separator();
        let ready = model.is_some() && !self.busy();
        let response = ui.add_enabled(
            ready,
            egui::TextEdit::multiline(&mut self.draft)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text(format!("Ask {} something… (Enter to send, Shift+Enter for a new line)", self.agent.title())),
        );
        let response = if model.is_none() {
            response.on_disabled_hover_text("Choose a model for this agent in AI Settings")
        } else {
            response
        };
        let enter = response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
        ui.horizontal(|ui| {
            let send = ui.add_enabled(ready && !self.draft.trim().is_empty(), egui::Button::new(format!("{} Send", icons::PAPER_PLANE_RIGHT)));
            if send.clicked() || enter {
                self.send(ui.ctx(), chats, settings);
            }
            if let Some(usage) = self.last_usage {
                ui.weak(format!(
                    "{} in · {} out · {} cached",
                    usage.input_tokens, usage.output_tokens, usage.cache_read_tokens
                ))
                .on_hover_text("Tokens of the last answer (input, output, read from the cache)");
            }
        });
        open_settings
    }

    fn conversation(&self, ui: &mut egui::Ui, chats: &Chats) {
        let messages = chats.messages(self.agent);
        if messages.is_empty() && self.pending.is_none() {
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
        if let Some(pending) = self.pending.as_ref().filter(|p| p.agent == self.agent) {
            ui.label(egui::RichText::new(self.agent.title()).strong());
            if !pending.text.is_empty() {
                ui.label(&pending.text);
            }
            if pending.text.is_empty() || pending.status.is_some() {
                ui.horizontal_wrapped(|ui| {
                    ui.spinner();
                    ui.weak(pending.status.as_deref().unwrap_or("Thinking…"));
                });
            }
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, format!("{} {error}", icons::WARNING));
        }
    }

    fn send(&mut self, ctx: &egui::Context, chats: &mut Chats, settings: &AiSettings) {
        let text = self.draft.trim().to_string();
        let Some((provider, model)) = settings.agent_model(self.agent) else { return };
        let key = keys::get(provider);
        // Local servers need no key; the rest do.
        if key.is_none() && (provider.kind == aia_ai::ProviderKind::Anthropic || provider.api_key_env.is_some()) {
            self.error = Some(format!("Add the API key of {} in AI Settings.", provider.id));
            return;
        }
        chats.push(self.agent, ChatMessage::user(text));
        self.draft.clear();
        self.error = None;
        let extra = settings.agent(self.agent).and_then(|a| a.system_prompt.as_deref());
        let ctx = ctx.clone();
        let job = ChatJob::start(
            provider.clone(),
            key,
            model.to_string(),
            prompts::system_prompt(self.agent, extra),
            chats.messages(self.agent).to_vec(),
            move || ctx.request_repaint(),
        );
        self.pending = Some(Pending {
            agent: self.agent,
            job,
            text: String::new(),
            status: None,
        });
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
