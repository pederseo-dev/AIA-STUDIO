use std::path::{Path, PathBuf};

use aia_ai::AiSettings;
use aia_core::{Document, Editor, PanelId};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};

use crate::panels::{self, drawing::DrawingPanel};

const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);

const FILE_FILTER: (&str, &[&str]) = ("AIA Studio", &["aia"]);

pub struct AiaApp {
    editor: Editor,
    dock: DockState<PanelId>,
    drawing: DrawingPanel,
    file: Option<PathBuf>,
    /// Base para los chats de IA por panel; todavía sin UI.
    #[allow(dead_code)]
    ai_settings: AiSettings,
    error: Option<String>,
    title: String,
    /// El usuario ya confirmó cerrar sin guardar.
    allow_close: bool,
}

impl AiaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut dock = DockState::new(vec![PanelId::Drawing]);
        let surface = dock.main_surface_mut();
        let [top, _timeline] = surface.split_below(NodeIndex::root(), 0.72, vec![PanelId::Timeline]);
        surface.split_right(top, 0.78, vec![PanelId::Bones]);

        Self {
            editor: Editor::default(),
            dock,
            drawing: DrawingPanel::new(cc.wgpu_render_state.clone()),
            file: None,
            ai_settings: AiSettings::default(),
            error: None,
            title: String::new(),
            allow_close: false,
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .file
            .as_deref()
            .and_then(Path::file_name)
            .map_or("Sin título".into(), |n| n.to_string_lossy().into_owned());
        let dirty = if self.editor.is_dirty() { "*" } else { "" };
        let title = format!("{name}{dirty} — AIA Studio");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    /// Pregunta qué hacer si hay cambios sin guardar. Devuelve `true` si se puede continuar.
    fn confirm_discard(&mut self) -> bool {
        if !self.editor.is_dirty() {
            return true;
        }
        let answer = rfd::MessageDialog::new()
            .set_title("AIA Studio")
            .set_description("Hay cambios sin guardar. ¿Guardar antes de continuar?")
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show();
        match answer {
            rfd::MessageDialogResult::Yes => self.save(false),
            rfd::MessageDialogResult::No => true,
            _ => false,
        }
    }

    fn new_document(&mut self) {
        if self.confirm_discard() {
            self.load_editor(Editor::default(), None);
        }
    }

    fn open(&mut self) {
        if !self.confirm_discard() {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter(FILE_FILTER.0, FILE_FILTER.1)
            .pick_file()
        else {
            return;
        };
        let result = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|json| Document::from_json(&json).map_err(|e| e.to_string()));
        match result {
            Ok(doc) => self.load_editor(Editor::new(doc), Some(path)),
            Err(e) => self.error = Some(format!("No se pudo abrir {}: {e}", path.display())),
        }
    }

    fn load_editor(&mut self, editor: Editor, file: Option<PathBuf>) {
        self.editor = editor;
        self.file = file;
        self.drawing.reset_view();
        self.error = None;
    }

    /// Devuelve `true` si se guardó.
    fn save(&mut self, save_as: bool) -> bool {
        let path = match (&self.file, save_as) {
            (Some(path), false) => path.clone(),
            _ => {
                let Some(mut path) = rfd::FileDialog::new()
                    .add_filter(FILE_FILTER.0, FILE_FILTER.1)
                    .set_file_name("animacion.aia")
                    .save_file()
                else {
                    return false;
                };
                if path.extension().is_none() {
                    path.set_extension("aia");
                }
                path
            }
        };
        let result = self
            .editor
            .doc()
            .to_json()
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.editor.mark_saved();
                self.file = Some(path);
                self.error = None;
                true
            }
            Err(e) => {
                self.error = Some(format!("No se pudo guardar {}: {e}", path.display()));
                false
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (undo, redo, new, open, save, save_as) = ctx.input_mut(|i| {
            // Primero los atajos con más modificadores, para que Ctrl+Z no "robe" Ctrl+Shift+Z.
            let redo = i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_ALT);
            let undo = i.consume_shortcut(&UNDO);
            let save_as = i.consume_shortcut(&SAVE_AS);
            let save = i.consume_shortcut(&SAVE);
            let new = i.consume_shortcut(&NEW);
            let open = i.consume_shortcut(&OPEN);
            (undo, redo, new, open, save, save_as)
        });
        if undo {
            self.editor.undo();
        }
        if redo {
            self.editor.redo();
        }
        if save_as {
            self.save(true);
        } else if save {
            self.save(false);
        }
        if new {
            self.new_document();
        }
        if open {
            self.open();
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.allow_close {
            return;
        }
        if self.confirm_discard() {
            self.allow_close = true;
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Archivo", |ui| {
                if ui.add(egui::Button::new("Nuevo").shortcut_text(ui.ctx().format_shortcut(&NEW))).clicked() {
                    self.new_document();
                }
                if ui.add(egui::Button::new("Abrir…").shortcut_text(ui.ctx().format_shortcut(&OPEN))).clicked() {
                    self.open();
                }
                ui.separator();
                if ui.add(egui::Button::new("Guardar").shortcut_text(ui.ctx().format_shortcut(&SAVE))).clicked() {
                    self.save(false);
                }
                if ui
                    .add(egui::Button::new("Guardar como…").shortcut_text(ui.ctx().format_shortcut(&SAVE_AS)))
                    .clicked()
                {
                    self.save(true);
                }
            });
            ui.menu_button("Editar", |ui| {
                let undo = egui::Button::new("Deshacer").shortcut_text(ui.ctx().format_shortcut(&UNDO));
                if ui.add_enabled(self.editor.can_undo(), undo).clicked() {
                    self.editor.undo();
                }
                let redo = egui::Button::new("Rehacer").shortcut_text(ui.ctx().format_shortcut(&REDO));
                if ui.add_enabled(self.editor.can_redo(), redo).clicked() {
                    self.editor.redo();
                }
            });
            ui.menu_button("Ver", |ui| {
                if ui.button("Encuadrar escenario").clicked() {
                    self.drawing.reset_view();
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("Zoom {:.0}%", self.drawing.zoom() * 100.0));
            ui.separator();
            ui.label(format!("Herramienta: {}", self.drawing.tool().name()));
            if let Some(error) = self.error.as_ref().or(self.drawing.message()) {
                ui.separator();
                ui.colored_label(ui.visuals().warn_fg_color, error);
            }
        });
    }
}

impl eframe::App for AiaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_close_request(&ctx);
        self.handle_shortcuts(&ctx);

        egui::Panel::top("menu_bar").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status_bar").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let mut tabs = Tabs {
                editor: &mut self.editor,
                drawing: &mut self.drawing,
            };
            DockArea::new(&mut self.dock)
                .style(egui_dock::Style::from_egui(ui.style().as_ref()))
                .show_inside(ui, &mut tabs);
        });

        self.update_title(&ctx);
    }
}

struct Tabs<'a> {
    editor: &'a mut Editor,
    drawing: &'a mut DrawingPanel,
}

impl TabViewer for Tabs<'_> {
    type Tab = PanelId;

    fn id(&mut self, tab: &mut PanelId) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut PanelId) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut PanelId) {
        match tab {
            PanelId::Drawing => self.drawing.ui(ui, self.editor),
            PanelId::Bones => panels::placeholder(ui, "Huesos", "Rigging 2D: jerarquía de huesos, IK y pesos."),
            PanelId::Timeline => panels::placeholder(ui, "Animación", "Línea de tiempo con keyframes y curvas."),
        }
    }

    fn is_closeable(&self, _tab: &PanelId) -> bool {
        false
    }
}
