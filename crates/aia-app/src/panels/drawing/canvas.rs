use aia_render::{CanvasRenderer, Scene, wgpu};
use eframe::egui;
use eframe::egui_wgpu::RenderState;

/// Recursos de GPU del canvas: el renderer de vello y la textura registrada en egui.
pub struct CanvasGpu {
    render_state: RenderState,
    renderer: CanvasRenderer,
    pub scene: Scene,
    texture_id: Option<egui::TextureId>,
}

impl CanvasGpu {
    pub fn new(render_state: RenderState) -> Result<Self, String> {
        let renderer = CanvasRenderer::new(&render_state.device).map_err(|e| e.to_string())?;
        Ok(Self {
            render_state,
            renderer,
            scene: Scene::new(),
            texture_id: None,
        })
    }

    /// Renderiza `self.scene` y devuelve la textura lista para pintar con egui.
    pub fn render(&mut self, width: u32, height: u32) -> Result<egui::TextureId, String> {
        let RenderState {
            device,
            queue,
            renderer: egui_renderer,
            ..
        } = &self.render_state;
        let (view, recreated) = self
            .renderer
            .render(device, queue, &self.scene, width, height)
            .map_err(|e| e.to_string())?;

        let mut egui_renderer = egui_renderer.write();
        let id = match self.texture_id {
            None => egui_renderer.register_native_texture(device, view, wgpu::FilterMode::Linear),
            Some(id) => {
                if recreated {
                    egui_renderer.update_egui_texture_from_wgpu_texture(
                        device,
                        view,
                        wgpu::FilterMode::Linear,
                        id,
                    );
                }
                id
            }
        };
        self.texture_id = Some(id);
        Ok(id)
    }
}
