//! Render del documento con vello sobre una textura wgpu.
//!
//! No depende de la UI: la app registra la textura resultante donde la necesite.

use aia_core::kurbo::{Affine, Point, Rect, Stroke, Vec2};
use aia_core::{Color, Document, Shape, ShapeId};
use vello::peniko::{self, Fill};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions};

pub use vello::{Scene, wgpu};

/// Vista del canvas: qué parte del documento se ve y con qué zoom.
/// Trabaja en "puntos" de la UI (antes de aplicar la densidad de píxeles).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Posición en pantalla del origen del documento.
    pub offset: Vec2,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl Camera {
    pub const MIN_ZOOM: f64 = 0.05;
    pub const MAX_ZOOM: f64 = 64.0;

    pub fn doc_to_screen(&self) -> Affine {
        Affine::translate(self.offset) * Affine::scale(self.zoom)
    }

    pub fn screen_to_doc(&self, p: Point) -> Point {
        ((p.to_vec2() - self.offset) / self.zoom).to_point()
    }

    /// Ajusta el zoom manteniendo fijo el punto de pantalla `anchor`.
    pub fn zoom_at(&mut self, anchor: Point, factor: f64) {
        let before = self.screen_to_doc(anchor);
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        self.offset = anchor.to_vec2() - before.to_vec2() * self.zoom;
    }

    /// Centra el escenario del documento dentro de un viewport de `size` puntos.
    pub fn fit(&mut self, doc: &Document, size: Vec2) {
        let margin = 0.9;
        self.zoom = ((size.x / doc.width).min(size.y / doc.height) * margin)
            .clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        self.offset = (size - Vec2::new(doc.width, doc.height) * self.zoom) / 2.0;
    }
}

/// Cosas que se dibujan encima del documento sin ser parte de él.
#[derive(Default)]
pub struct Overlay<'a> {
    /// Transformación provisional (por ejemplo, mientras se arrastra) de algunas formas.
    pub moving: Option<(&'a [ShapeId], Affine)>,
    /// Forma que se está creando con una herramienta.
    pub preview: Option<&'a Shape>,
    /// Formas seleccionadas: se dibuja su caja envolvente.
    pub selection: &'a [ShapeId],
}

const WORKSPACE_BG: peniko::Color = peniko::Color::from_rgb8(48, 48, 52);
const SELECTION: peniko::Color = peniko::Color::from_rgb8(40, 140, 255);

fn color(c: Color) -> peniko::Color {
    peniko::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

fn draw_shape(scene: &mut Scene, shape: &Shape, transform: Affine) {
    let transform = transform * shape.transform;
    if let Some(fill) = shape.style.fill {
        scene.fill(Fill::NonZero, transform, color(fill), None, &shape.path);
    }
    if let Some(stroke) = shape.style.stroke {
        let style = Stroke::new(stroke.width);
        scene.stroke(&style, transform, color(stroke.color), None, &shape.path);
    }
}

/// Construye la escena completa. `scale` convierte puntos de UI a píxeles físicos.
pub fn build_scene(scene: &mut Scene, doc: &Document, camera: &Camera, overlay: &Overlay, scale: f64) {
    scene.reset();
    let view = Affine::scale(scale) * camera.doc_to_screen();

    let stage = Rect::new(0.0, 0.0, doc.width, doc.height);
    scene.fill(Fill::NonZero, view, color(doc.background), None, &stage);

    // Las formas fuera del escenario también se ven: es útil para animar entradas.
    for layer in doc.layers.iter().filter(|l| l.visible) {
        for shape in &layer.shapes {
            let extra = match overlay.moving {
                Some((ids, transform)) if ids.contains(&shape.id) => transform,
                _ => Affine::IDENTITY,
            };
            draw_shape(scene, shape, view * extra);
        }
    }

    if let Some(preview) = overlay.preview {
        draw_shape(scene, preview, view);
    }

    // La selección se dibuja en espacio de pantalla para que el grosor no dependa del zoom.
    let selection_stroke = Stroke::new(scale).with_dashes(0.0, [4.0 * scale, 3.0 * scale]);
    for id in overlay.selection {
        let Some(shape) = doc.shape(*id) else { continue };
        let mut bounds = shape.bounds();
        if let Some((ids, transform)) = overlay.moving
            && ids.contains(id)
        {
            bounds = transform.transform_rect_bbox(bounds);
        }
        let screen = view.transform_rect_bbox(bounds).inflate(2.0 * scale, 2.0 * scale);
        scene.stroke(&selection_stroke, Affine::IDENTITY, SELECTION, None, &screen);
    }
}

/// Renderer de vello más la textura de destino, que se recrea al cambiar de tamaño.
pub struct CanvasRenderer {
    renderer: Renderer,
    target: Option<Target>,
}

struct Target {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl CanvasRenderer {
    pub fn new(device: &wgpu::Device) -> Result<Self, vello::Error> {
        let renderer = Renderer::new(
            device,
            RendererOptions {
                antialiasing_support: AaSupport::area_only(),
                ..Default::default()
            },
        )?;
        Ok(Self {
            renderer,
            target: None,
        })
    }

    /// Renderiza la escena. Devuelve la vista de la textura y `true` si la
    /// textura es nueva (hay que volver a registrarla en la UI).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        width: u32,
        height: u32,
    ) -> Result<(&wgpu::TextureView, bool), vello::Error> {
        let width = width.max(1);
        let height = height.max(1);
        let recreated = !matches!(&self.target, Some(t) if t.width == width && t.height == height);
        if recreated {
            self.target = Some(Target::new(device, width, height));
        }
        let target = self.target.as_ref().expect("creado arriba");
        self.renderer.render_to_texture(
            device,
            queue,
            scene,
            &target.view,
            &RenderParams {
                base_color: WORKSPACE_BG,
                width,
                height,
                antialiasing_method: AaConfig::Area,
            },
        )?;
        Ok((&target.view, recreated))
    }
}

impl Target {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aia-canvas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // vello escribe valores sRGB en una textura Rgba8Unorm, que es justo
            // lo que espera el shader de egui.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            view,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_anchor_fixed() {
        let mut camera = Camera {
            offset: Vec2::new(30.0, 40.0),
            zoom: 1.5,
        };
        let anchor = Point::new(200.0, 150.0);
        let before = camera.screen_to_doc(anchor);
        camera.zoom_at(anchor, 2.0);
        let after = camera.screen_to_doc(anchor);
        assert!((before - after).hypot() < 1e-9);
    }
}
