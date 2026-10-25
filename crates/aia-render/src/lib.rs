//! Renders the document with vello into a wgpu texture.
//!
//! Independent of the UI: the app registers the resulting texture wherever it needs it.

use aia_core::kurbo::{Affine, BezPath, Cap, Circle, Join, Line, Point, Rect, Stroke, Vec2};
use aia_core::{Color, Document, EdgeRef, Layer, LayerId, LayerKind, LayerTransform, Mesh, PointId, Shape, ShapeId};
use vello::peniko::{self, Fill};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions};

pub use vello::{Scene, wgpu};

/// Canvas view: which part of the document is visible and at what zoom.
/// Works in UI "points" (before applying pixel density).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Screen position of the document origin.
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

    /// Changes the zoom keeping the screen point `anchor` fixed.
    pub fn zoom_at(&mut self, anchor: Point, factor: f64) {
        let before = self.screen_to_doc(anchor);
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        self.offset = anchor.to_vec2() - before.to_vec2() * self.zoom;
    }

    /// Centers the document stage inside a viewport of `size` points.
    pub fn fit(&mut self, doc: &Document, size: Vec2) {
        let margin = 0.9;
        self.zoom = ((size.x / doc.width).min(size.y / doc.height) * margin).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        self.offset = (size - Vec2::new(doc.width, doc.height) * self.zoom) / 2.0;
    }
}

/// Things drawn on top of the document (or temporarily replacing parts of it) without being part of it.
#[derive(Default)]
pub struct Overlay<'a> {
    /// Temporary mesh of a layer (an edit in progress).
    pub edited_mesh: Option<(LayerId, &'a Mesh)>,
    /// Temporary transform of a layer.
    pub edited_transform: Option<(LayerId, LayerTransform)>,
    /// Points and curves of the layer being edited.
    pub wireframe: Option<Wireframe<'a>>,
    /// Transform box (scale/rotate), in document coordinates.
    pub transform_box: Option<Rect>,
    /// Selection marquee being dragged, in document coordinates.
    pub marquee: Option<Rect>,
    /// Origin of the active layer (crosshair), in document coordinates.
    pub origin: Option<Point>,
}

/// The (normally invisible) geometry of a vector layer, for editing.
pub struct Wireframe<'a> {
    pub layer: LayerId,
    pub selected_points: &'a [PointId],
    /// Show the curve handles at the selected points.
    pub handles: bool,
    /// Selected shapes: hatched fill and highlighted outline.
    pub selected_shapes: &'a [ShapeId],
    /// Highlighted edges (Create Shape preview, hovered edge).
    pub highlight: &'a [EdgeRef],
    /// Point that a dragged point will weld onto when dropped.
    pub weld_target: Option<PointId>,
}

/// Distance (in screen points) of the rotate handle above the box.
pub const ROTATE_HANDLE_OFFSET: f64 = 24.0;

/// Handles of the transform box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxHandle {
    /// 0 = top-left, 1 = top-right, 2 = bottom-right, 3 = bottom-left.
    Corner(u8),
    /// 0 = top, 1 = right, 2 = bottom, 3 = left.
    Edge(u8),
    Rotate,
}

/// Position of each handle for a box in screen space (points).
/// Used by both the renderer and the app's hit testing, so they always match.
pub fn box_handles(rect: Rect) -> [(BoxHandle, Point); 9] {
    let c = rect.center();
    [
        (BoxHandle::Corner(0), Point::new(rect.x0, rect.y0)),
        (BoxHandle::Corner(1), Point::new(rect.x1, rect.y0)),
        (BoxHandle::Corner(2), Point::new(rect.x1, rect.y1)),
        (BoxHandle::Corner(3), Point::new(rect.x0, rect.y1)),
        (BoxHandle::Edge(0), Point::new(c.x, rect.y0)),
        (BoxHandle::Edge(1), Point::new(rect.x1, c.y)),
        (BoxHandle::Edge(2), Point::new(c.x, rect.y1)),
        (BoxHandle::Edge(3), Point::new(rect.x0, c.y)),
        (BoxHandle::Rotate, Point::new(c.x, rect.y0 - ROTATE_HANDLE_OFFSET)),
    ]
}

const WORKSPACE_BG: peniko::Color = peniko::Color::from_rgb8(48, 48, 52);
const SELECTION: peniko::Color = peniko::Color::from_rgb8(40, 140, 255);
const WIRE: peniko::Color = peniko::Color::from_rgba8(70, 70, 90, 200);
const ORIGIN: peniko::Color = peniko::Color::from_rgb8(230, 60, 60);

fn color(c: Color, opacity: f64) -> peniko::Color {
    peniko::Color::from_rgba8(c.r, c.g, c.b, (c.a as f64 * opacity).round().clamp(0.0, 255.0) as u8)
}

fn draw_shape(scene: &mut Scene, mesh: &Mesh, shape: &Shape, transform: Affine, opacity: f64) {
    if let Some(fill) = shape.style.fill
        && let Some(path) = mesh.fill_path(shape)
    {
        // With more than one contour, the inner ones are holes.
        let rule = if shape.fill.len() > 1 { Fill::EvenOdd } else { Fill::NonZero };
        scene.fill(rule, transform, color(fill, opacity), None, &path);
    }
    if let Some(stroke) = shape.style.stroke {
        let style = Stroke::new(stroke.width).with_caps(Cap::Round).with_join(Join::Round);
        for path in mesh.outline_paths(shape) {
            scene.stroke(&style, transform, color(stroke.color, opacity), None, &path);
        }
    }
}

/// What the wireframe needs from its layer, resolved while walking the tree.
struct WireTarget<'a> {
    mesh: &'a Mesh,
    /// From layer space to screen pixels (already includes the view).
    to_screen: Affine,
}

fn draw_layers<'a>(
    scene: &mut Scene,
    layers: &'a [Layer],
    parent: Affine,
    opacity: f64,
    visible: bool,
    overlay: &Overlay<'a>,
    wire: &mut Option<WireTarget<'a>>,
) {
    for layer in layers {
        let local = match overlay.edited_transform {
            Some((id, t)) if id == layer.id => t.affine(layer.origin),
            _ => layer.affine(),
        };
        let transform = parent * local;
        let opacity = opacity * layer.opacity;
        let visible = visible && layer.visible;
        match &layer.kind {
            LayerKind::Vector { mesh } => {
                let mesh = match overlay.edited_mesh {
                    Some((id, edited)) if id == layer.id => edited,
                    _ => mesh,
                };
                if overlay.wireframe.as_ref().is_some_and(|w| w.layer == layer.id) {
                    *wire = Some(WireTarget { mesh, to_screen: transform });
                }
                if visible {
                    for shape in &mesh.shapes {
                        draw_shape(scene, mesh, shape, transform, opacity);
                    }
                }
            }
            LayerKind::Group { children } => draw_layers(scene, children, transform, opacity, visible, overlay, wire),
        }
    }
}

/// Builds the whole scene. `scale` converts UI points to physical pixels.
pub fn build_scene(scene: &mut Scene, doc: &Document, camera: &Camera, overlay: &Overlay, scale: f64) {
    scene.reset();
    let view = Affine::scale(scale) * camera.doc_to_screen();

    let stage = Rect::new(0.0, 0.0, doc.width, doc.height);
    scene.fill(Fill::NonZero, view, color(doc.background, 1.0), None, &stage);

    // Shapes outside the stage are visible too: useful for animating entrances.
    let mut wire = None;
    draw_layers(scene, &doc.layers, view, 1.0, true, overlay, &mut wire);

    if let (Some(w), Some(target)) = (&overlay.wireframe, wire) {
        draw_wireframe(scene, w, &target, scale);
    }

    let dashed = Stroke::new(scale).with_dashes(0.0, [4.0 * scale, 3.0 * scale]);
    if let Some(rect) = overlay.transform_box {
        draw_transform_box(scene, camera.doc_to_screen().transform_rect_bbox(rect), scale);
    }
    if let Some(rect) = overlay.marquee {
        let screen = view.transform_rect_bbox(rect);
        scene.fill(Fill::NonZero, Affine::IDENTITY, SELECTION.with_alpha(0.12), None, &screen);
        scene.stroke(&dashed, Affine::IDENTITY, SELECTION, None, &screen);
    }
    if let Some(origin) = overlay.origin {
        let p = view * origin;
        let r = 8.0 * scale;
        let line = Stroke::new(1.5 * scale);
        scene.stroke(&line, Affine::IDENTITY, ORIGIN, None, &Line::new(p - Vec2::new(r, 0.0), p + Vec2::new(r, 0.0)));
        scene.stroke(&line, Affine::IDENTITY, ORIGIN, None, &Line::new(p - Vec2::new(0.0, r), p + Vec2::new(0.0, r)));
        scene.stroke(&line, Affine::IDENTITY, ORIGIN, None, &Circle::new(p, r * 0.5));
    }
}

/// Curves, points and handles of the layer being edited, with constant on-screen width.
fn draw_wireframe(scene: &mut Scene, wire: &Wireframe, target: &WireTarget, scale: f64) {
    let mesh = target.mesh;
    let to_screen = target.to_screen;
    let thin = Stroke::new(scale);

    for curve in &mesh.curves {
        scene.stroke(&thin, Affine::IDENTITY, WIRE, None, &(to_screen * mesh.curve_path(curve)));
    }

    let thick = Stroke::new(3.0 * scale).with_caps(Cap::Round);
    for edge in wire.highlight {
        if let Some(seg) = mesh.edge_segment(*edge) {
            let mut path = BezPath::from_path_segments(std::iter::once(seg));
            path.apply_affine(to_screen);
            scene.stroke(&thick, Affine::IDENTITY, SELECTION, None, &path);
        }
    }

    for shape in wire.selected_shapes.iter().filter_map(|id| mesh.shape(*id)) {
        draw_shape_selection(scene, mesh, shape, to_screen, scale);
    }

    let half = 3.0 * scale;
    if wire.handles {
        for curve in &mesh.curves {
            for (i, node) in curve.nodes.iter().enumerate() {
                if !wire.selected_points.contains(&node.point) {
                    continue;
                }
                let Some(pos) = mesh.pos(node.point) else { continue };
                let (h_in, h_out) = mesh.handles(curve, i);
                let p = to_screen * pos;
                for h in [h_in, h_out] {
                    if h == Vec2::ZERO {
                        continue;
                    }
                    let hp = to_screen * (pos + h);
                    scene.stroke(&thin, Affine::IDENTITY, SELECTION, None, &Line::new(p, hp));
                    let dot = Circle::new(hp, half);
                    scene.fill(Fill::NonZero, Affine::IDENTITY, peniko::Color::WHITE, None, &dot);
                    scene.stroke(&thin, Affine::IDENTITY, SELECTION, None, &dot);
                }
            }
        }
    }

    if let Some(pos) = wire.weld_target.and_then(|id| mesh.pos(id)) {
        let ring = Circle::new(to_screen * pos, 3.0 * half);
        scene.stroke(&Stroke::new(2.0 * scale), Affine::IDENTITY, SELECTION, None, &ring);
    }

    for point in mesh.points() {
        let p = to_screen * point.pos;
        let square = Rect::from_center_size(p, (half * 2.0, half * 2.0));
        let selected = wire.selected_points.contains(&point.id);
        let fill = if selected { SELECTION } else { peniko::Color::WHITE };
        scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &square);
        scene.stroke(&thin, Affine::IDENTITY, if selected { SELECTION } else { WIRE }, None, &square);
    }
}

/// Selected shape, as in Moho: a hatch over its fill and its outline highlighted.
/// A shape without fill and stroke shows its area and a dashed outline, so it can be found.
fn draw_shape_selection(scene: &mut Scene, mesh: &Mesh, shape: &Shape, to_screen: Affine, scale: f64) {
    let invisible = shape.style.fill.is_none() && shape.style.stroke.is_none();
    if (shape.style.fill.is_some() || invisible)
        && let Some(path) = mesh.fill_path(shape)
    {
        let path = to_screen * path;
        let rule = if shape.fill.len() > 1 { Fill::EvenOdd } else { Fill::NonZero };
        let bounds = aia_core::kurbo::Shape::bounding_box(&path);
        scene.push_clip_layer(rule, Affine::IDENTITY, &path);
        scene.fill(Fill::NonZero, Affine::IDENTITY, SELECTION.with_alpha(0.15), None, &bounds);
        let hatch = Stroke::new(scale);
        let step = 6.0 * scale;
        let mut x = bounds.x0 - bounds.height();
        while x < bounds.x1 {
            let line = Line::new((x, bounds.y1), (x + bounds.height(), bounds.y0));
            scene.stroke(&hatch, Affine::IDENTITY, SELECTION.with_alpha(0.6), None, &line);
            x += step;
        }
        scene.pop_layer();
    }
    let outline = if shape.style.stroke.is_some() {
        Stroke::new(2.0 * scale).with_caps(Cap::Round).with_join(Join::Round)
    } else {
        Stroke::new(scale).with_dashes(0.0, [4.0 * scale, 3.0 * scale])
    };
    for path in mesh.outline_paths(shape) {
        scene.stroke(&outline, Affine::IDENTITY, SELECTION, None, &(to_screen * path));
    }
}

/// `rect` in screen points; `scale` turns it into pixels.
fn draw_transform_box(scene: &mut Scene, rect: Rect, scale: f64) {
    let to_px = Affine::scale(scale);
    let line = Stroke::new(scale);
    scene.stroke(&line, Affine::IDENTITY, SELECTION, None, &(to_px.transform_rect_bbox(rect)));
    let top = to_px * Point::new(rect.center().x, rect.y0);
    for (handle, p) in box_handles(rect) {
        let p = to_px * p;
        let half = 4.0 * scale;
        if handle == BoxHandle::Rotate {
            scene.stroke(&line, Affine::IDENTITY, SELECTION, None, &Line::new(top, p));
            let dot = Circle::new(p, half);
            scene.fill(Fill::NonZero, Affine::IDENTITY, peniko::Color::WHITE, None, &dot);
            scene.stroke(&line, Affine::IDENTITY, SELECTION, None, &dot);
        } else {
            let square = Rect::from_center_size(p, (half * 2.0, half * 2.0));
            scene.fill(Fill::NonZero, Affine::IDENTITY, peniko::Color::WHITE, None, &square);
            scene.stroke(&line, Affine::IDENTITY, SELECTION, None, &square);
        }
    }
}

/// The vello renderer plus the target texture, recreated when the size changes.
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
        Ok(Self { renderer, target: None })
    }

    /// Renders the scene. Returns the texture view and `true` if the texture is new
    /// (it must be registered again in the UI).
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
        let target = self.target.as_ref().expect("created above");
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
            // vello writes sRGB values into an Rgba8Unorm texture, which is exactly
            // what egui's shader expects.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { view, width, height }
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
