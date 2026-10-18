//! Los modos de trabajo. Cada uno aporta sus herramientas (barra de íconos),
//! su panel lateral y su forma de interpretar el mouse sobre el canvas.

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

/// Botón cuadrado de la barra de herramientas, con nombre y atajo en el tooltip.
pub fn tool_button(ui: &mut egui::Ui, selected: bool, icon: &str, name: &str, shortcut: &str) -> egui::Response {
    let button = egui::Button::selectable(selected, egui::RichText::new(icon).size(20.0))
        .min_size(egui::vec2(34.0, 34.0));
    ui.add(button).on_hover_text(format!("{name}  ({shortcut})"))
}

/// Texto de "próximamente" para modos o secciones sin implementar.
pub fn coming_soon(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.weak(text);
    ui.weak("Próximamente");
}
