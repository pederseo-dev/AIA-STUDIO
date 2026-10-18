use std::path::{Path, PathBuf};

use aia_ai::AiSettings;
use aia_core::{Chats, Editor, Mode, project};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use serde::{Deserialize, Serialize};

use crate::canvas::Canvas;
use crate::chat_panel::ChatPanel;
use crate::modes::{self, animation::AnimationMode, design::DesignMode};

const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
const TOGGLE_SIDEBAR: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);
const TOGGLE_BOTTOM: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::J);
const TOGGLE_CHAT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::B);
const TOGGLE_MODES: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::M);

fn mode_shortcut(mode: Mode) -> KeyboardShortcut {
    let key = match mode {
        Mode::Design => Key::Num1,
        Mode::Animation => Key::Num2,
    };
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}

const FILE_FILTER: (&str, &[&str]) = ("AIA Studio", &["aia"]);
/// Máximo de mensajes que guarda el panel inferior.
const LOG_LIMIT: usize = 200;

/// Estado de la interfaz que se recuerda entre sesiones.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Layout {
    mode: Mode,
    mode_bar_open: bool,
    sidebar_open: bool,
    bottom_open: bool,
    chat_open: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            mode: Mode::Design,
            mode_bar_open: true,
            sidebar_open: true,
            bottom_open: true,
            chat_open: true,
        }
    }
}

/// Paneles que se pueden ocultar desde los botones de la esquina superior derecha.
#[derive(Clone, Copy)]
enum PanelSide {
    /// La barra de modos (arriba).
    Top,
    Left,
    Bottom,
    Right,
}

pub struct AiaApp {
    editor: Editor,
    layout: Layout,
    canvas: Canvas,
    design: DesignMode,
    animation: AnimationMode,
    chat: ChatPanel,
    /// Conversaciones con cada agente; se guardan dentro del proyecto `.aia`.
    chats: Chats,
    file: Option<PathBuf>,
    /// Modelo y skills de cada agente; todavía sin UI.
    #[allow(dead_code)]
    ai_settings: AiSettings,
    /// Mensajes para el usuario (errores y avisos). El último se ve en la barra de estado.
    log: Vec<String>,
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
            design: DesignMode::default(),
            animation: AnimationMode,
            chat: ChatPanel::default(),
            chats: Chats::default(),
            file: None,
            ai_settings: AiSettings::default(),
            log: Vec::new(),
            title: String::new(),
            allow_close: false,
            maximize_pending,
        }
    }

    fn notify(&mut self, message: impl Into<String>) {
        self.log.push(message.into());
        if self.log.len() > LOG_LIMIT {
            self.log.remove(0);
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        if mode != self.layout.mode {
            self.design.leave(&mut self.editor);
            self.layout.mode = mode;
        }
    }

    fn toggle(&mut self, side: PanelSide) {
        let open = match side {
            PanelSide::Top => &mut self.layout.mode_bar_open,
            PanelSide::Left => &mut self.layout.sidebar_open,
            PanelSide::Bottom => &mut self.layout.bottom_open,
            PanelSide::Right => &mut self.layout.chat_open,
        };
        *open ^= true;
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
            self.load_project(Editor::default(), Chats::default(), None);
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
            .and_then(|json| project::from_json(&json).map_err(|e| e.to_string()));
        match result {
            Ok((doc, chats)) => self.load_project(Editor::new(doc), chats, Some(path)),
            Err(e) => self.notify(format!("No se pudo abrir {}: {e}", path.display())),
        }
    }

    fn load_project(&mut self, editor: Editor, chats: Chats, file: Option<PathBuf>) {
        self.editor = editor;
        self.chats = chats;
        self.file = file;
        self.canvas.reset_view();
        self.design.reset();
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
        let result = project::to_json(self.editor.doc(), &self.chats)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.editor.mark_saved();
                self.notify(format!("Guardado en {}", path.display()));
                self.file = Some(path);
                true
            }
            Err(e) => {
                self.notify(format!("No se pudo guardar {}: {e}", path.display()));
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
            panel: Option<PanelSide>,
            mode: Option<Mode>,
        }
        let pressed = ctx.input_mut(|i| {
            // Primero los atajos con más modificadores, para que Ctrl+Z no "robe" Ctrl+Shift+Z.
            let redo = i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_ALT);
            let save_as = i.consume_shortcut(&SAVE_AS);
            let chat = i.consume_shortcut(&TOGGLE_CHAT);
            let panel = if chat {
                Some(PanelSide::Right)
            } else if i.consume_shortcut(&TOGGLE_MODES) {
                Some(PanelSide::Top)
            } else if i.consume_shortcut(&TOGGLE_SIDEBAR) {
                Some(PanelSide::Left)
            } else if i.consume_shortcut(&TOGGLE_BOTTOM) {
                Some(PanelSide::Bottom)
            } else {
                None
            };
            Pressed {
                redo,
                save_as,
                panel,
                undo: i.consume_shortcut(&UNDO),
                save: i.consume_shortcut(&SAVE),
                new: i.consume_shortcut(&NEW),
                open: i.consume_shortcut(&OPEN),
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
        if let Some(side) = pressed.panel {
            self.toggle(side);
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

    /// Primera barra: menús a la izquierda y botones de paneles a la derecha.
    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            self.menus(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // De derecha a izquierda: chat, inferior, lateral (igual que VS Code).
                let toggles = [
                    (PanelSide::Right, self.layout.chat_open, "Chat IA", &TOGGLE_CHAT),
                    (PanelSide::Bottom, self.layout.bottom_open, "Panel inferior", &TOGGLE_BOTTOM),
                    (PanelSide::Left, self.layout.sidebar_open, "Panel lateral", &TOGGLE_SIDEBAR),
                    (PanelSide::Top, self.layout.mode_bar_open, "Barra de modos", &TOGGLE_MODES),
                ];
                for (side, open, name, shortcut) in toggles {
                    let verb = if open { "Ocultar" } else { "Mostrar" };
                    let tip = format!("{verb} {}  ({})", name.to_lowercase(), ui.ctx().format_shortcut(shortcut));
                    if panel_toggle(ui, side, open).on_hover_text(tip).clicked() {
                        self.toggle(side);
                    }
                }
            });
        });
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
            if ui.add(menu_item(ui, "Barra de modos", &TOGGLE_MODES)).clicked() {
                self.toggle(PanelSide::Top);
            }
            if ui.add(menu_item(ui, "Panel lateral", &TOGGLE_SIDEBAR)).clicked() {
                self.toggle(PanelSide::Left);
            }
            if ui.add(menu_item(ui, "Panel inferior", &TOGGLE_BOTTOM)).clicked() {
                self.toggle(PanelSide::Bottom);
            }
            if ui.add(menu_item(ui, "Chat IA", &TOGGLE_CHAT)).clicked() {
                self.toggle(PanelSide::Right);
            }
            ui.separator();
            if ui.button("Encuadrar escenario").clicked() {
                self.canvas.reset_view();
            }
        });
    }

    /// Segunda barra: los modos de trabajo, centrados.
    fn mode_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let button_size = egui::vec2(140.0, 32.0);
            let spacing = ui.spacing().item_spacing.x;
            let total = button_size.x * Mode::ALL.len() as f32 + spacing * (Mode::ALL.len() - 1) as f32;
            ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
            for mode in Mode::ALL {
                let text = egui::RichText::new(format!("{}  {}", modes::icon(mode), mode.title())).size(16.0);
                let button = egui::Button::selectable(self.layout.mode == mode, text).min_size(button_size);
                let tip = format!("{}  ({})", mode.description(), ui.ctx().format_shortcut(&mode_shortcut(mode)));
                if ui.add(button).on_hover_text(tip).clicked() {
                    self.set_mode(mode);
                }
            }
        });
        ui.add_space(4.0);
    }

    /// Barra de íconos a la izquierda: herramientas del modo.
    fn activity_bar(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            match self.layout.mode {
                Mode::Design => self.design.tools_ui(ui, &mut self.editor),
                Mode::Animation => self.animation.tools_ui(ui),
            }
        });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.strong(format!("{} {}", modes::icon(self.layout.mode), self.layout.mode.title()));
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| match self.layout.mode {
            Mode::Design => self.design.sidebar_ui(ui, &mut self.editor),
            Mode::Animation => self.animation.sidebar_ui(ui),
        });
    }

    /// Panel inferior general: la línea de tiempo en Animation, los mensajes en Design.
    fn bottom_panel(&mut self, ui: &mut egui::Ui) {
        if self.layout.mode == Mode::Animation {
            self.animation.timeline_ui(ui);
            return;
        }
        ui.add_space(4.0);
        ui.strong("Mensajes");
        ui.separator();
        egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            if self.log.is_empty() {
                ui.weak("Sin mensajes.");
            }
            for message in &self.log {
                ui.label(message);
            }
        });
    }

    fn canvas_area(&mut self, ui: &mut egui::Ui) {
        let mode = self.layout.mode;
        if mode == Mode::Design {
            self.design.handle_keys(ui, &mut self.editor);
        }
        let Some((view, response)) = self.canvas.begin(ui, self.editor.doc()) else {
            return;
        };

        let cursor = match mode {
            Mode::Design => {
                self.design.canvas_input(ui, &response, &view, &mut self.editor);
                self.design.cursor(&view, &self.editor, response.hover_pos())
            }
            // Animation todavía no interpreta el mouse: solo pan y zoom.
            Mode::Animation if view.panning => egui::CursorIcon::Grab,
            Mode::Animation => egui::CursorIcon::Default,
        };
        response.on_hover_cursor(cursor);

        let data = self.design.prepare_overlay(ui, &view);
        let overlay = match mode {
            Mode::Design => self.design.overlay(&data, &self.editor),
            Mode::Animation => Default::default(),
        };
        if let Err(e) = self.canvas.paint(ui, view.rect, self.editor.doc(), &overlay) {
            self.notify(e);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("{} {}", modes::icon(self.layout.mode), self.layout.mode.title()));
            ui.separator();
            ui.label(format!("Zoom {:.0}%", self.canvas.zoom() * 100.0));
            if self.layout.mode == Mode::Design {
                ui.separator();
                ui.label(format!("Herramienta: {}", self.design.tool().name()));
            }
            ui.separator();
            ui.label(format!("{} Agente: {}", modes::agent_icon(self.chat.agent()), self.chat.agent().title()));
            if let Some(message) = self.log.last() {
                ui.separator();
                ui.weak(message);
            }
        });
    }
}

fn menu_item<'a>(ui: &egui::Ui, text: &'a str, shortcut: &KeyboardShortcut) -> egui::Button<'a> {
    egui::Button::new(text).shortcut_text(ui.ctx().format_shortcut(shortcut))
}

/// Botón con el dibujo de la ventana y el panel resaltado (como los de VS Code):
/// el panel se ve relleno cuando está abierto y solo con borde cuando está cerrado.
fn panel_toggle(ui: &mut egui::Ui, side: PanelSide, open: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(26.0, 22.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        if response.hovered() {
            ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
        }
        let color = visuals.fg_stroke.color;
        let frame = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 13.0));
        let stroke = egui::Stroke::new(1.2, color);
        ui.painter().rect_stroke(frame, 2.0, stroke, egui::StrokeKind::Inside);
        let panel = match side {
            PanelSide::Top => egui::Rect::from_min_max(frame.min, egui::pos2(frame.max.x, frame.min.y + 4.0)),
            PanelSide::Left => egui::Rect::from_min_max(frame.min, egui::pos2(frame.min.x + 5.0, frame.max.y)),
            PanelSide::Right => egui::Rect::from_min_max(egui::pos2(frame.max.x - 5.0, frame.min.y), frame.max),
            PanelSide::Bottom => egui::Rect::from_min_max(egui::pos2(frame.min.x, frame.max.y - 5.0), frame.max),
        };
        if open {
            ui.painter().rect_filled(panel, 1.0, color);
        } else {
            ui.painter().rect_stroke(panel, 1.0, stroke, egui::StrokeKind::Inside);
        }
    }
    response
}

impl eframe::App for AiaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if std::mem::take(&mut self.maximize_pending) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        self.handle_close_request(&ctx);
        self.handle_shortcuts(&ctx);
        self.design.sync_with_document(&self.editor);
        for message in self.design.take_messages() {
            self.notify(message);
        }
        let suggestion = match self.layout.mode {
            Mode::Design => self.design.suggested_agent(),
            Mode::Animation => aia_core::Agent::default_for(Mode::Animation),
        };
        self.chat.follow_suggestion(self.layout.mode, suggestion);

        // El orden importa: los paneles laterales se reservan antes que el canvas central.
        egui::Panel::top("menu_bar").show(ui, |ui| self.menu_bar(ui));
        if self.layout.mode_bar_open {
            egui::Panel::top("mode_bar").show(ui, |ui| self.mode_bar(ui));
        }
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
                .show(ui, |ui| self.chat.ui(ui, self.layout.mode, &self.chats));
        }
        if self.layout.bottom_open {
            egui::Panel::bottom("bottom_panel")
                .default_size(180.0)
                .size_range(80.0..=500.0)
                .show(ui, |ui| self.bottom_panel(ui));
        }
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas_area(ui));

        self.update_title(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.layout);
    }
}
