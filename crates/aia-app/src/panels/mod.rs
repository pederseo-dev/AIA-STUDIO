pub mod drawing;

use eframe::egui;

/// Contenido provisional de un panel que todavía no está implementado.
pub fn placeholder(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.heading(title);
            ui.label(description);
            ui.weak("Próximamente");
        });
    });
}
