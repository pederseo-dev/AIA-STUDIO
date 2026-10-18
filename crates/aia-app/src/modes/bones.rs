use eframe::egui;

/// Modo Bones: armado del esqueleto. Todavía sin herramientas.
#[derive(Default)]
pub struct BonesMode;

impl BonesMode {
    pub fn tools_ui(&mut self, _ui: &mut egui::Ui) {}

    pub fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        super::coming_soon(ui, "Jerarquía de huesos, IK y pesos.");
    }
}
