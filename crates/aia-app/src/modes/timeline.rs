use eframe::egui;

/// Modo Timeline: animación con keyframes. Todavía sin herramientas.
#[derive(Default)]
pub struct TimelineMode;

impl TimelineMode {
    pub fn tools_ui(&mut self, _ui: &mut egui::Ui) {}

    pub fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        super::coming_soon(ui, "Propiedades del keyframe y curvas de easing.");
    }

    /// Área de la línea de tiempo, debajo del canvas.
    pub fn timeline_ui(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.heading("Línea de tiempo");
            super::coming_soon(ui, "Keyframes por capa y forma, reproducción y scrubbing.");
        });
    }
}
