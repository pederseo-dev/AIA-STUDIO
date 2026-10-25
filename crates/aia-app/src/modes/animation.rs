use eframe::egui;

/// Animation mode: poses, keyframes and curves. Never modifies the base.
/// No tools yet.
#[derive(Default)]
pub struct AnimationMode;

impl AnimationMode {
    pub fn tools_panel(&mut self, ui: &mut egui::Ui) {
        ui.weak("No tools yet");
    }

    pub fn options_bar(&mut self, ui: &mut egui::Ui) {
        ui.weak("Animation tools: coming soon");
    }

    pub fn inspector(&mut self, ui: &mut egui::Ui) {
        super::coming_soon(ui, "Keyframe properties and easing curves.");
    }

    /// Timeline: fills the bottom panel in this mode.
    pub fn timeline(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.strong("Timeline");
        ui.separator();
        super::coming_soon(ui, "Keyframes per layer and shape, playback and scrubbing.");
    }
}
