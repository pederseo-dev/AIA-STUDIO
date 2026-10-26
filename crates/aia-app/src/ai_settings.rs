//! AI Settings window: providers with their API key (kept in the system credential store)
//! and the model of each agent.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};

use aia_ai::{AiSettings, KeySource, ModelRef, Preset, ProviderConfig, ProviderKind, keys, provider};
use aia_core::Agent;
use eframe::egui;
use egui_phosphor::regular as icons;

/// Answer of a "Fetch models" request.
type ModelList = Result<Vec<String>, String>;

#[derive(Default)]
pub struct AiSettingsWindow {
    pub open: bool,
    /// Key being typed for each provider (never shown once saved).
    key_drafts: HashMap<String, String>,
    /// Where each provider's key comes from; read when the window opens (the credential store is slow to query every frame).
    sources: HashMap<String, KeySource>,
    new_provider: (String, String),
    status: Option<String>,
    /// Models offered by each provider, as fetched from it.
    models: HashMap<String, Vec<String>>,
    /// Model list being fetched: (provider id, answer).
    fetching: Option<(String, Receiver<ModelList>)>,
}

impl AiSettingsWindow {
    pub fn show(&mut self, ctx: &egui::Context, settings: &mut AiSettings) {
        if !self.open {
            self.sources.clear();
            return;
        }
        if self.sources.is_empty() {
            self.refresh(settings);
        }
        self.receive_models();
        let mut open = self.open;
        egui::Window::new(format!("{} AI Settings", icons::GEAR))
            .open(&mut open)
            .default_width(460.0)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.providers(ui, settings);
                    ui.add_space(8.0);
                    self.agents(ui, settings);
                    if let Some(status) = &self.status {
                        ui.add_space(6.0);
                        ui.weak(status);
                    }
                });
            });
        self.open = open;
    }

    fn receive_models(&mut self) {
        let Some((id, answer)) = &self.fetching else { return };
        let Ok(result) = answer.try_recv() else { return };
        match result {
            Ok(models) => {
                self.status = Some(format!("{}: {} models available.", id, models.len()));
                self.models.insert(id.clone(), models);
            }
            Err(e) => self.status = Some(format!("Could not list the models of {id}: {e}")),
        }
        self.fetching = None;
    }

    /// Asks the provider for its models on a worker thread.
    fn fetch_models(&mut self, ctx: &egui::Context, provider: &ProviderConfig) {
        let (sender, answer) = channel();
        self.fetching = Some((provider.id.clone(), answer));
        let (provider, key, ctx) = (provider.clone(), keys::get(provider), ctx.clone());
        std::thread::spawn(move || {
            let _ = sender.send(provider::list_models(&provider, key.as_deref()));
            ctx.request_repaint();
        });
    }

    fn refresh(&mut self, settings: &AiSettings) {
        self.sources = settings.providers.iter().map(|p| (p.id.clone(), keys::source(p))).collect();
    }

    fn providers(&mut self, ui: &mut egui::Ui, settings: &mut AiSettings) {
        ui.heading("Providers");
        ui.weak("API keys are saved in the system credential store, never in project or settings files.");
        let mut fetch = None;
        let mut remove = None;
        for (index, provider) in settings.providers.iter_mut().enumerate() {
            ui.add_space(4.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.strong(&provider.id);
                    ui.weak(Preset::find(&provider.id).map_or(provider.kind.title(), |p| p.name));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(icons::TRASH).on_hover_text("Remove provider").clicked() {
                            remove = Some(index);
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("URL:");
                    let mut url = provider.base_url.clone().unwrap_or_default();
                    let edit = egui::TextEdit::singleline(&mut url).hint_text(provider.kind.default_base_url()).desired_width(300.0);
                    if ui.add(edit).changed() {
                        provider.base_url = Some(url.trim().to_string()).filter(|u| !u.is_empty());
                    }
                });
                if let Some(note) = Preset::find(&provider.id).and_then(|p| p.free) {
                    ui.weak(format!("{} {note}", icons::GIFT));
                }
                self.key_row(ui, provider);
                ui.horizontal(|ui| {
                    let busy = self.fetching.is_some();
                    if ui.add_enabled(!busy, egui::Button::new(format!("{} Fetch models", icons::ARROWS_CLOCKWISE))).clicked() {
                        fetch = Some(provider.clone());
                    }
                    if self.fetching.as_ref().is_some_and(|(id, _)| *id == provider.id) {
                        ui.spinner();
                    } else if let Some(models) = self.models.get(&provider.id) {
                        ui.weak(format!("{} models", models.len()));
                    }
                });
            });
        }
        if let Some(provider) = fetch {
            self.fetch_models(ui.ctx(), &provider);
        }
        if let Some(index) = remove {
            let provider = settings.providers.remove(index);
            for agent in &mut settings.agents {
                if agent.model.as_ref().is_some_and(|m| m.provider_id == provider.id) {
                    agent.model = None;
                }
            }
        }

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Add:");
            let missing: Vec<&Preset> = Preset::ALL.iter().filter(|p| settings.provider(p.id).is_none()).collect();
            for preset in missing {
                let label = if preset.free.is_some() { format!("{} {}", icons::GIFT, preset.name) } else { preset.name.to_string() };
                if ui.small_button(label).on_hover_text(preset.free.unwrap_or("paid API")).clicked() {
                    let provider = preset.config();
                    self.sources.insert(provider.id.clone(), keys::source(&provider));
                    settings.providers.push(provider);
                }
            }
        });
        ui.weak(format!(
            "{} Free tiers may log your requests or use them for training; local servers keep everything on this computer.",
            icons::INFO
        ));
        ui.horizontal(|ui| {
            ui.label("Custom:");
            ui.add(egui::TextEdit::singleline(&mut self.new_provider.0).hint_text("id, e.g. openai or local").desired_width(130.0));
            ui.add(egui::TextEdit::singleline(&mut self.new_provider.1).hint_text("URL (empty = OpenAI)").desired_width(170.0));
            let id = self.new_provider.0.trim().to_string();
            let valid = !id.is_empty() && settings.provider(&id).is_none();
            if ui
                .add_enabled(valid, egui::Button::new(format!("{} Add", icons::PLUS)))
                .on_hover_text("OpenAI-compatible provider: OpenAI, Gemini, OpenRouter, Groq, DeepSeek, Ollama, LM Studio…")
                .clicked()
            {
                let mut provider = ProviderConfig::new(id, ProviderKind::OpenAiCompatible);
                provider.base_url = Some(self.new_provider.1.trim().to_string()).filter(|u| !u.is_empty());
                self.sources.insert(provider.id.clone(), keys::source(&provider));
                settings.providers.push(provider);
                self.new_provider = Default::default();
            }
        });
    }

    fn key_row(&mut self, ui: &mut egui::Ui, provider: &ProviderConfig) {
        let source = self.sources.get(&provider.id).cloned().unwrap_or(KeySource::Missing);
        ui.horizontal(|ui| {
            ui.label("Key:");
            match &source {
                KeySource::CredentialStore => ui.label(format!("{} saved", icons::CHECK_CIRCLE)),
                KeySource::Env(var) => ui.label(format!("{} from ${var}", icons::CHECK_CIRCLE)),
                KeySource::Missing if provider.kind == ProviderKind::OpenAiCompatible => ui.weak("none (fine for local servers)"),
                KeySource::Missing => ui.colored_label(ui.visuals().warn_fg_color, format!("{} missing", icons::WARNING)),
            };
            if source == KeySource::CredentialStore && ui.small_button("Remove").clicked() {
                self.status = Some(match keys::remove(&provider.id) {
                    Ok(()) => format!("Key of {} removed.", provider.id),
                    Err(e) => format!("Could not remove the key: {e}"),
                });
                self.sources.insert(provider.id.clone(), keys::source(provider));
            }
        });
        ui.horizontal(|ui| {
            let draft = self.key_drafts.entry(provider.id.clone()).or_default();
            ui.add(egui::TextEdit::singleline(draft).password(true).hint_text("paste a new API key").desired_width(300.0));
            if ui.add_enabled(!draft.trim().is_empty(), egui::Button::new("Save key")).clicked() {
                self.status = Some(match keys::store(&provider.id, draft) {
                    Ok(()) => format!("Key of {} saved in the credential store.", provider.id),
                    Err(e) => format!("Could not save the key: {e}"),
                });
                draft.clear();
                self.sources.insert(provider.id.clone(), keys::source(provider));
            }
        });
    }

    fn agents(&mut self, ui: &mut egui::Ui, settings: &mut AiSettings) {
        ui.heading("Agents");
        ui.weak("Mid-tier models by default, to keep usage moderate. Use \"Fetch models\" to pick from a provider's list.");
        let providers: Vec<(String, ProviderKind)> = settings.providers.iter().map(|p| (p.id.clone(), p.kind)).collect();
        egui::Grid::new("agent_models").num_columns(3).spacing([8.0, 6.0]).show(ui, |ui| {
            for agent in Agent::ALL {
                ui.label(agent.title());
                let config = settings.agent_mut(agent);
                let current = config.model.clone();
                let selected = current.as_ref().map_or("disabled".to_string(), |m| m.provider_id.clone());
                egui::ComboBox::from_id_salt(("agent_provider", agent))
                    .selected_text(selected)
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(current.is_none(), "disabled").clicked() {
                            config.model = None;
                        }
                        for (id, kind) in &providers {
                            let chosen = current.as_ref().is_some_and(|m| &m.provider_id == id);
                            if ui.selectable_label(chosen, id).clicked() && !chosen {
                                config.model = Some(ModelRef {
                                    provider_id: id.clone(),
                                    model: kind.suggested_models().first().copied().unwrap_or_default().to_string(),
                                });
                            }
                        }
                    });
                if let Some(model) = &mut config.model {
                    let kind = providers.iter().find(|(id, _)| *id == model.provider_id).map(|(_, k)| *k);
                    // Fetched models first; otherwise the suggested ones.
                    let fetched = self.models.get(&model.provider_id);
                    let suggested: Vec<&str> = match fetched {
                        Some(list) => list.iter().map(String::as_str).collect(),
                        None => kind.map_or(&[][..], ProviderKind::suggested_models).to_vec(),
                    };
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut model.model).hint_text("model").desired_width(200.0));
                        if !suggested.is_empty() {
                            ui.menu_button(icons::CARET_DOWN, |ui| {
                                ui.weak("Type in the field to filter");
                                let filter = model.model.to_lowercase();
                                let matching: Vec<&str> = suggested.iter().copied().filter(|n| n.to_lowercase().contains(&filter)).collect();
                                let shown = if matching.is_empty() { suggested.clone() } else { matching };
                                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                    for name in shown {
                                        if ui.button(name).clicked() {
                                            model.model = name.to_string();
                                            ui.close();
                                        }
                                    }
                                });
                            });
                        }
                    });
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
    }
}
