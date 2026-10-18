//! Canvas compartido por todos los modos: render con vello, pan y zoom.
//! La interpretación del mouse (herramientas) la hace cada modo.

use aia_core::Document;
use aia_core::kurbo::{Point, Vec2};
use aia_render::{Camera, CanvasRenderer, Overlay, Scene, build_scene, wgpu};
use eframe::egui::{self, Sense};
use eframe::egui_wgpu::RenderState;

/// Lo que un modo necesita saber del canvas en este frame.
pub struct CanvasView {
    pub rect: egui::Rect,
    pub camera: Camera,
    /// El usuario está moviendo la vista: las herramientas no deben actuar.
    pub panning: bool,
}

impl CanvasView {
    /// Posición de pantalla → coordenadas del documento.
    pub fn to_doc(&self, pos: egui::Pos2) -> Point {
        let local = Point::new((pos.x - self.rect.min.x) as f64, (pos.y - self.rect.min.y) as f64);
        self.camera.screen_to_doc(local)
    }
}

pub struct Canvas {
    gpu: Result<CanvasGpu, String>,
    camera: Camera,
    /// Mientras el usuario no mueva ni haga zoom, el escenario se mantiene
    /// encuadrado aunque cambie el tamaño del canvas (por ejemplo, al maximizar).
    auto_fit: bool,
    last_size: egui::Vec2,
}

impl Canvas {
    pub fn new(render_state: Option<RenderState>) -> Self {
        let gpu = render_state
            .ok_or_else(|| "No hay GPU disponible (wgpu)".to_string())
            .and_then(CanvasGpu::new);
        if let Err(e) = &gpu {
            log::error!("No se pudo iniciar el canvas: {e}");
        }
        Self {
            gpu,
            camera: Camera::default(),
            auto_fit: true,
            last_size: egui::Vec2::ZERO,
        }
    }

    pub fn zoom(&self) -> f64 {
        self.camera.zoom
    }

    pub fn reset_view(&mut self) {
        self.auto_fit = true;
        self.last_size = egui::Vec2::ZERO;
    }

    /// Reserva el área del canvas y aplica pan/zoom. Devuelve `None` si no hay espacio.
    pub fn begin(&mut self, ui: &mut egui::Ui, doc: &Document) -> Option<(CanvasView, egui::Response)> {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return None;
        }
        if self.auto_fit && rect.size() != self.last_size {
            self.camera.fit(doc, Vec2::new(rect.width() as f64, rect.height() as f64));
            self.last_size = rect.size();
        }

        let space = ui.input(|i| i.key_down(egui::Key::Space));
        let middle = response.dragged_by(egui::PointerButton::Middle);
        if middle || (space && response.dragged_by(egui::PointerButton::Primary)) {
            let delta = response.drag_delta();
            self.camera.offset += Vec2::new(delta.x as f64, delta.y as f64);
            self.auto_fit = false;
        }

        if let Some(pos) = response.hover_pos() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta().y, i.zoom_delta()));
            let factor = zoom as f64 * (scroll as f64 * 0.002).exp();
            if factor != 1.0 {
                let local = Point::new((pos.x - rect.min.x) as f64, (pos.y - rect.min.y) as f64);
                self.camera.zoom_at(local, factor);
                self.auto_fit = false;
            }
        }

        let view = CanvasView {
            rect,
            camera: self.camera,
            panning: space || middle,
        };
        Some((view, response))
    }

    pub fn paint(&mut self, ui: &mut egui::Ui, rect: egui::Rect, doc: &Document, overlay: &Overlay) -> Result<(), String> {
        let gpu = match &mut self.gpu {
            Ok(gpu) => gpu,
            Err(e) => {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("Canvas no disponible: {e}"),
                    egui::FontId::proportional(14.0),
                    ui.visuals().warn_fg_color,
                );
                return Ok(());
            }
        };
        let ppp = ui.ctx().pixels_per_point();
        build_scene(&mut gpu.scene, doc, &self.camera, overlay, ppp as f64);
        let width = (rect.width() * ppp).round() as u32;
        let height = (rect.height() * ppp).round() as u32;
        let texture = gpu.render(width, height)?;
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture, rect, uv, egui::Color32::WHITE);
        Ok(())
    }
}

/// Recursos de GPU del canvas: el renderer de vello y la textura registrada en egui.
struct CanvasGpu {
    render_state: RenderState,
    renderer: CanvasRenderer,
    scene: Scene,
    texture_id: Option<egui::TextureId>,
}

impl CanvasGpu {
    fn new(render_state: RenderState) -> Result<Self, String> {
        let renderer = CanvasRenderer::new(&render_state.device).map_err(|e| e.to_string())?;
        Ok(Self {
            render_state,
            renderer,
            scene: Scene::new(),
            texture_id: None,
        })
    }

    /// Renderiza `self.scene` y devuelve la textura lista para pintar con egui.
    fn render(&mut self, width: u32, height: u32) -> Result<egui::TextureId, String> {
        let RenderState {
            device,
            queue,
            renderer: egui_renderer,
            ..
        } = &self.render_state;
        let (view, recreated) = self
            .renderer
            .render(device, queue, &self.scene, width, height)
            .map_err(|e| format!("Error de render: {e}"))?;

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
