use aia_core::Mode;
use eframe::egui;
use egui_phosphor::regular as icons;

/// Panel derecho reservado para el chat de IA del modo activo.
pub fn ui(ui: &mut egui::Ui, mode: Mode) {
    ui.add_space(6.0);
    ui.heading(format!("{} Chat IA · {}", icons::CHAT_CIRCLE_DOTS, mode.title()));
    ui.separator();
    ui.label(format!("Asistente específico del modo {}.", mode.title()));
    ui.weak("Próximamente: modelo y skills configurables por modo, y acciones sobre el documento.");
}
