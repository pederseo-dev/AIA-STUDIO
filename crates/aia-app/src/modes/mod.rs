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

/// Cursor over the canvas: a system cursor plus an optional badge next to it, as in Moho.
pub struct CanvasCursor {
    pub icon: egui::CursorIcon,
    pub badge: Option<Icon>,
}

/// Size of the cursor badge, in points.
const BADGE: f32 = 14.0;

/// Draws the badge below and to the right of the mouse, over everything, on a light square
/// so it reads on dark and light drawings.
pub fn paint_cursor_badge(ctx: &egui::Context, pointer: egui::Pos2, icon: Icon) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("cursor_badge")));
    let rect = egui::Rect::from_min_size(pointer + egui::vec2(12.0, 14.0), egui::vec2(BADGE, BADGE));
    painter.rect_filled(rect.expand(2.0), 3.0, egui::Color32::from_white_alpha(225));
    painter.rect_stroke(rect.expand(2.0), 3.0, egui::Stroke::new(1.0, egui::Color32::from_gray(120)), egui::StrokeKind::Inside);
    let ink = egui::Color32::from_gray(25);
    match icon {
        Icon::Drawn(paint) => paint(&painter, rect, ink),
        Icon::Glyph(glyph) => {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(BADGE - 1.0), ink);
        }
    }
}

/// "Coming soon" text for modes or sections not implemented yet.
pub fn coming_soon(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.weak(text);
    ui.weak("Coming soon");
}
