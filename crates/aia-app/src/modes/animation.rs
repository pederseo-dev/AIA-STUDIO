use eframe::egui;

/// Modo Animation: poses, keyframes y curvas. Nunca modifica la base.
/// Todavía sin herramientas.
#[derive(Default)]
pub struct AnimationMode;

impl AnimationMode {
    pub fn tools_ui(&mut self, _ui: &mut egui::Ui) {}

    pub fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        super::coming_soon(ui, "Propiedades del keyframe y curvas de easing.");
    }

    /// Línea de tiempo: ocupa el panel inferior en este modo.
    pub fn timeline_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.strong("Línea de tiempo");
        ui.separator();
        super::coming_soon(ui, "Keyframes por capa y forma, reproducción y scrubbing.");
    }
}
