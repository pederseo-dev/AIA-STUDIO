use std::path::{Path, PathBuf};

use aia_ai::AiSettings;
use aia_core::{Document, Editor, Mode};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use egui_phosphor::regular as icons;
use serde::{Deserialize, Serialize};

use crate::canvas::Canvas;
use crate::chat_panel;
use crate::modes::{self, bones::BonesMode, draw::DrawMode, timeline::TimelineMode};

const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
const TOGGLE_SIDEBAR: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);
const TOGGLE_CHAT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::B);

fn mode_shortcut(mode: Mode) -> KeyboardShortcut {
    let key = match mode {
        Mode::Draw => Key::Num1,
        Mode::Bones => Key::Num2,
        Mode::Timeline => Key::Num3,
    };
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}

const FILE_FILTER: (&str, &[&str]) = ("AIA Studio", &["aia"]);

/// Estado de la interfaz que se recuerda entre sesiones.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Layout {
    mode: Mode,
    sidebar_open: bool,
    chat_open: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            mode: Mode::Draw,
            sidebar_open: true,
            chat_open: true,
        }
    }
}

pub struct AiaApp {
    editor: Editor,
    layout: Layout,
    canvas: Canvas,
    draw: DrawMode,
    bones: BonesMode,
    timeline: TimelineMode,
    file: Option<PathBuf>,
    /// Base para los chats de IA por modo; todavía sin UI.
    #[allow(dead_code)]
    ai_settings: AiSettings,
    error: Option<String>,
    title: String,
    /// El usuario ya confirmó cerrar sin guardar.
    allow_close: bool,
    /// Maximizar en el primer frame. Hacerlo al crear la ventana deja la
    /// superficie de render más grande que el área visible en Windows.
    maximize_pending: bool,
}

impl AiaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        let stored: Option<Layout> = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY));
        // En la primera ejecución se maximiza; después eframe recuerda el tamaño de la ventana.
        let maximize_pending = stored.is_none();
        let layout = stored.unwrap_or_default();

        Self {
            editor: Editor::default(),
            layout,
            canvas: Canvas::new(cc.wgpu_render_state.clone()),
            draw: DrawMode::default(),
            bones: BonesMode,
            timeline: TimelineMode,
            file: None,
            ai_settings: AiSettings::default(),
            error: None,
            title: String::new(),
            allow_close: false,
            maximize_pending,
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        if mode != self.layout.mode {
            self.draw.cancel_gesture();
            self.layout.mode = mode;
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
        self.canvas.reset_view();
        self.draw.reset();
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
        struct Pressed {
            undo: bool,
            redo: bool,
            new: bool,
            open: bool,
            save: bool,
            save_as: bool,
            sidebar: bool,
            chat: bool,
            mode: Option<Mode>,
        }
        let pressed = ctx.input_mut(|i| {
            // Primero los atajos con más modificadores, para que Ctrl+Z no "robe" Ctrl+Shift+Z.
            let redo = i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_ALT);
            let save_as = i.consume_shortcut(&SAVE_AS);
            let chat = i.consume_shortcut(&TOGGLE_CHAT);
            Pressed {
                redo,
                save_as,
                chat,
                undo: i.consume_shortcut(&UNDO),
                save: i.consume_shortcut(&SAVE),
                new: i.consume_shortcut(&NEW),
                open: i.consume_shortcut(&OPEN),
                sidebar: i.consume_shortcut(&TOGGLE_SIDEBAR),
                mode: Mode::ALL.into_iter().find(|m| i.consume_shortcut(&mode_shortcut(*m))),
            }
        });
        if pressed.undo {
            self.editor.undo();
        }
        if pressed.redo {
            self.editor.redo();
        }
        if pressed.save_as {
            self.save(true);
        } else if pressed.save {
            self.save(false);
        }
        if pressed.new {
            self.new_document();
        }
        if pressed.open {
            self.open();
        }
        if pressed.sidebar {
            self.layout.sidebar_open ^= true;
        }
        if pressed.chat {
            self.layout.chat_open ^= true;
        }
        if let Some(mode) = pressed.mode {
            self.set_mode(mode);
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

    /// Barra superior: menús a la izquierda y los modos al centro.
    fn nav_bar(&mut self, ui: &mut egui::Ui) {
        let full = ui.max_rect();
        let menu = egui::MenuBar::new().ui(ui, |ui| self.menus(ui)).response.rect;

        let width = 340.0;
        let center = egui::pos2(full.center().x, menu.center().y);
        let rect = egui::Rect::from_center_size(center, egui::vec2(width, menu.height().max(26.0)));
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                for mode in Mode::ALL {
                    let text = format!("{} {}", modes::icon(mode), mode.title());
                    let button = egui::Button::selectable(self.layout.mode == mode, text).min_size(egui::vec2(104.0, 24.0));
                    let tip = format!("{}  ({})", mode.description(), ui.ctx().format_shortcut(&mode_shortcut(mode)));
                    if ui.add(button).on_hover_text(tip).clicked() {
                        self.set_mode(mode);
                    }
                }
            },
        );
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Archivo", |ui| {
            if ui.add(menu_item(ui, "Nuevo", &NEW)).clicked() {
                self.new_document();
            }
            if ui.add(menu_item(ui, "Abrir…", &OPEN)).clicked() {
                self.open();
            }
            ui.separator();
            if ui.add(menu_item(ui, "Guardar", &SAVE)).clicked() {
                self.save(false);
            }
            if ui.add(menu_item(ui, "Guardar como…", &SAVE_AS)).clicked() {
                self.save(true);
            }
        });
        ui.menu_button("Editar", |ui| {
            if ui.add_enabled(self.editor.can_undo(), menu_item(ui, "Deshacer", &UNDO)).clicked() {
                self.editor.undo();
            }
            if ui.add_enabled(self.editor.can_redo(), menu_item(ui, "Rehacer", &REDO)).clicked() {
                self.editor.redo();
            }
        });
        ui.menu_button("Ver", |ui| {
            if ui.add(menu_item(ui, "Panel lateral", &TOGGLE_SIDEBAR)).clicked() {
                self.layout.sidebar_open ^= true;
            }
            if ui.add(menu_item(ui, "Chat IA", &TOGGLE_CHAT)).clicked() {
                self.layout.chat_open ^= true;
            }
            ui.separator();
            if ui.button("Encuadrar escenario").clicked() {
                self.canvas.reset_view();
            }
        });
    }

    /// Barra de íconos a la izquierda: herramientas del modo y botones de paneles.
    fn activity_bar(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            match self.layout.mode {
                Mode::Draw => self.draw.tools_ui(ui),
                Mode::Bones => self.bones.tools_ui(ui),
                Mode::Timeline => self.timeline.tools_ui(ui),
            }
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            let shortcut = ui.ctx().format_shortcut(&TOGGLE_CHAT);
            if modes::tool_button(ui, self.layout.chat_open, icons::CHAT_CIRCLE_DOTS, "Chat IA", &shortcut).clicked() {
                self.layout.chat_open ^= true;
            }
            let shortcut = ui.ctx().format_shortcut(&TOGGLE_SIDEBAR);
            if modes::tool_button(ui, self.layout.sidebar_open, icons::SIDEBAR_SIMPLE, "Panel lateral", &shortcut).clicked() {
                self.layout.sidebar_open ^= true;
            }
        });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.strong(format!("{} {}", modes::icon(self.layout.mode), self.layout.mode.title()));
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| match self.layout.mode {
            Mode::Draw => self.draw.sidebar_ui(ui, &mut self.editor),
            Mode::Bones => self.bones.sidebar_ui(ui),
            Mode::Timeline => self.timeline.sidebar_ui(ui),
        });
    }

    fn canvas_area(&mut self, ui: &mut egui::Ui) {
        let mode = self.layout.mode;
        if mode == Mode::Draw {
            self.draw.handle_keys(ui, &mut self.editor);
        }
        let Some((view, response)) = self.canvas.begin(ui, self.editor.doc()) else {
            return;
        };

        let shift = ui.input(|i| i.modifiers.shift);
        let cursor = match mode {
            Mode::Draw => {
                self.draw.canvas_input(ui, &response, &view, &mut self.editor);
                self.draw.cursor(&view)
            }
            // Los demás modos todavía no interpretan el mouse: solo pan y zoom.
            Mode::Bones | Mode::Timeline if view.panning => egui::CursorIcon::Grab,
            Mode::Bones | Mode::Timeline => egui::CursorIcon::Default,
        };
        response.on_hover_cursor(cursor);

        let preview = (mode == Mode::Draw).then(|| self.draw.preview_shape(shift)).flatten();
        let overlay = match mode {
            Mode::Draw => self.draw.overlay(preview.as_ref()),
            Mode::Bones | Mode::Timeline => Default::default(),
        };
        if let Err(e) = self.canvas.paint(ui, view.rect, self.editor.doc(), &overlay) {
            self.error = Some(e);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("{} {}", modes::icon(self.layout.mode), self.layout.mode.title()));
            ui.separator();
            ui.label(format!("Zoom {:.0}%", self.canvas.zoom() * 100.0));
            if self.layout.mode == Mode::Draw {
                ui.separator();
                ui.label(format!("Herramienta: {}", self.draw.tool().name()));
            }
            if let Some(error) = self.error.as_ref().or(self.draw.message()) {
                ui.separator();
                ui.colored_label(ui.visuals().warn_fg_color, error);
            }
        });
    }
}

fn menu_item<'a>(ui: &egui::Ui, text: &'a str, shortcut: &KeyboardShortcut) -> egui::Button<'a> {
    egui::Button::new(text).shortcut_text(ui.ctx().format_shortcut(shortcut))
}

impl eframe::App for AiaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if std::mem::take(&mut self.maximize_pending) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        self.handle_close_request(&ctx);
        self.handle_shortcuts(&ctx);
        self.draw.sync_with_document(&self.editor);

        // El orden importa: los paneles laterales se reservan antes que el canvas central.
        egui::Panel::top("nav_bar").show(ui, |ui| self.nav_bar(ui));
        egui::Panel::bottom("status_bar").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("activity_bar")
            .resizable(false)
            .exact_size(48.0)
            .show(ui, |ui| self.activity_bar(ui));
        if self.layout.sidebar_open {
            egui::Panel::left("sidebar")
                .default_size(250.0)
                .size_range(180.0..=420.0)
                .show(ui, |ui| self.sidebar(ui));
        }
        if self.layout.chat_open {
            egui::Panel::right("chat")
                .default_size(300.0)
                .size_range(220.0..=520.0)
                .show(ui, |ui| chat_panel::ui(ui, self.layout.mode));
        }
        if self.layout.mode == Mode::Timeline {
            egui::Panel::bottom("timeline")
                .default_size(200.0)
                .size_range(120.0..=500.0)
                .show(ui, |ui| self.timeline.timeline_ui(ui));
        }
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas_area(ui));

        self.update_title(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.layout);
    }
}
