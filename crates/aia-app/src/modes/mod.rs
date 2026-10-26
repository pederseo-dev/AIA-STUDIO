//! Work modes. Each one provides its tools, its panels and its own way of
//! interpreting the mouse on the canvas.

pub mod animation;
pub mod design;

use aia_core::{Agent, Mode};
use eframe::egui;
use egui_phosphor::regular as icons;

pub fn icon(mode: Mode) -> &'static str {
    match mode {
        Mode::Design => icons::COMPASS_TOOL,
        Mode::Animation => icons::FILM_STRIP,
    }
}

pub fn agent_icon(agent: Agent) -> &'static str {
    match agent {
        Agent::Draw => icons::PEN_NIB,
        Agent::Rigging => icons::BONE,
        Agent::Animation => icons::PERSON_SIMPLE_RUN,
    }
}

/// A tool icon: a Phosphor glyph or one drawn with strokes.
#[derive(Clone, Copy)]
pub enum Icon {
    Glyph(&'static str),
    Drawn(fn(&egui::Painter, egui::Rect, egui::Color32)),
}

impl Icon {
    /// Shows the icon at `size` points, in the current text color.
    pub fn show(self, ui: &mut egui::Ui, size: f32) -> egui::Response {
        match self {
            Icon::Glyph(glyph) => ui.label(egui::RichText::new(glyph).size(size)),
            Icon::Drawn(paint) => {
                let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                paint(ui.painter(), rect, ui.visuals().text_color());
                response
            }
        }
    }
}

/// Side of a tool button, in points (small, as in Moho).
pub const TOOL_BUTTON: f32 = 22.0;

/// Square button of the tools panel (the caller adds the tooltip).
pub fn tool_button(ui: &mut egui::Ui, selected: bool, icon: Icon) -> egui::Response {
    let size = egui::vec2(TOOL_BUTTON, TOOL_BUTTON);
    let button = match icon {
        Icon::Glyph(glyph) => egui::Button::selectable(selected, egui::RichText::new(glyph).size(14.0)),
        Icon::Drawn(_) => egui::Button::selectable(selected, ""),
    };
    let response = ui.add(button.min_size(size).frame_when_inactive(false));
    if let Icon::Drawn(paint) = icon {
        let color = ui.style().interact_selectable(&response, selected).fg_stroke.color;
        paint(ui.painter(), egui::Rect::from_center_size(response.rect.center(), egui::vec2(14.0, 14.0)), color);
    }
    response
}

/// "Coming soon" text for modes or sections not implemented yet.
pub fn coming_soon(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.weak(text);
    ui.weak("Coming soon");
}
