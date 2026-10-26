//! Design mode tools (like Moho's), mouse gestures and transform math.

use aia_core::kurbo::{Affine, Point, Rect, Vec2};
use aia_core::{CurveId, Handle, LayerTransform, Mesh, PointId};
use aia_render::BoxHandle;
use eframe::egui::Key;
use egui_phosphor::regular as phosphor;

use super::icons;
use crate::modes::Icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    // Draw
    SelectPoints,
    TransformPoints,
    AddPoint,
    Curvature,
    DrawShape,
    DeleteEdge,
    // Fill
    SelectShape,
    CreateShape,
    // Layer
    TransformLayer,
    SetOrigin,
}

/// Sections of the tools panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolGroup {
    Draw,
    Fill,
    Layer,
}

impl ToolGroup {
    pub const ALL: [ToolGroup; 3] = [ToolGroup::Draw, ToolGroup::Fill, ToolGroup::Layer];

    pub fn title(self) -> &'static str {
        match self {
            ToolGroup::Draw => "Draw",
            ToolGroup::Fill => "Fill",
            ToolGroup::Layer => "Layer",
        }
    }
}

impl Tool {
    pub const ALL: [Tool; 10] = [
        Tool::SelectPoints,
        Tool::TransformPoints,
        Tool::AddPoint,
        Tool::Curvature,
        Tool::DrawShape,
        Tool::DeleteEdge,
        Tool::SelectShape,
        Tool::CreateShape,
        Tool::TransformLayer,
        Tool::SetOrigin,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::SelectPoints => "Select Points",
            Tool::TransformPoints => "Transform Points",
            Tool::AddPoint => "Add Point",
            Tool::Curvature => "Curvature",
            Tool::DrawShape => "Draw Shape",
            Tool::DeleteEdge => "Delete Edge",
            Tool::SelectShape => "Select Shape",
            Tool::CreateShape => "Create Shape",
            Tool::TransformLayer => "Transform Layer",
            Tool::SetOrigin => "Set Origin",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Tool::SelectPoints => Icon::Drawn(icons::select_points),
            Tool::TransformPoints => Icon::Drawn(icons::transform_points),
            Tool::AddPoint => Icon::Drawn(icons::add_point),
            Tool::Curvature => Icon::Drawn(icons::curvature),
            Tool::DrawShape => Icon::Drawn(icons::draw_shape),
            Tool::DeleteEdge => Icon::Drawn(icons::delete_edge),
            Tool::SelectShape => Icon::Glyph(phosphor::CURSOR_CLICK),
            Tool::CreateShape => Icon::Glyph(phosphor::SELECTION_PLUS),
            Tool::TransformLayer => Icon::Glyph(phosphor::ARROWS_OUT),
            Tool::SetOrigin => Icon::Glyph(phosphor::CROSSHAIR),
        }
    }

    /// Same shortcuts as Moho.
    pub fn shortcut(self) -> Key {
        match self {
            Tool::SelectPoints => Key::G,
            Tool::TransformPoints => Key::T,
            Tool::AddPoint => Key::A,
            Tool::Curvature => Key::C,
            Tool::DrawShape => Key::E,
            Tool::DeleteEdge => Key::D,
            Tool::SelectShape => Key::Q,
            Tool::CreateShape => Key::U,
            Tool::TransformLayer => Key::M,
            Tool::SetOrigin => Key::O,
        }
    }

    /// How to use the tool; shown in the status bar, like Moho.
    pub fn hint(self) -> &'static str {
        match self {
            Tool::SelectPoints => {
                "Select points (click a point, a curve or a fill; drag for a marquee; hold <shift> to add, <alt> to remove)"
            }
            Tool::TransformPoints => {
                "Move/Scale/Rotate selected points (drag between the two boxes to rotate around the pivot, drag the pivot to move it; press <enter> to weld, hold <shift> to constrain, <alt> to scale from the center)"
            }
            Tool::AddPoint => {
                "Add points (drag from empty space for a new curve, from an end to extend it, on a curve to insert; drop on a point to weld; <shift> for 45°)"
            }
            Tool::Curvature => {
                "Drag left to sharpen or right to round the curve at the selected points (or the nearest one); click to select a point"
            }
            Tool::DrawShape => "Draw a rectangle or oval (hold <shift> to constrain, <alt> to draw from the center)",
            Tool::DeleteEdge => "Click a curve segment to delete it (points are kept; fills using it are removed)",
            Tool::SelectShape => "Select shapes (hold <shift> to add, <alt> to remove)",
            Tool::CreateShape => "Select the points around a region, then press <enter> to create a shape",
            Tool::TransformLayer => {
                "Transform entire layer (drag to move; box handles scale and rotate around the origin; hold <shift> to constrain)"
            }
            Tool::SetOrigin => "Click to set the layer's origin (the layer does not move)",
        }
    }

    pub fn group(self) -> ToolGroup {
        match self {
            Tool::SelectPoints | Tool::TransformPoints | Tool::AddPoint | Tool::Curvature | Tool::DrawShape | Tool::DeleteEdge => {
                ToolGroup::Draw
            }
            Tool::SelectShape | Tool::CreateShape => ToolGroup::Fill,
            Tool::TransformLayer | Tool::SetOrigin => ToolGroup::Layer,
        }
    }

    /// Draw and fill tools only exist on vector layers.
    pub fn needs_vector(self) -> bool {
        self.group() != ToolGroup::Layer
    }

    /// Tools that work on the point selection.
    pub fn uses_points(self) -> bool {
        matches!(
            self,
            Tool::SelectPoints | Tool::TransformPoints | Tool::AddPoint | Tool::Curvature | Tool::CreateShape
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Oval,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 2] = [ShapeKind::Rectangle, ShapeKind::Oval];

    pub fn name(self) -> &'static str {
        match self {
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::Oval => "Oval",
        }
    }
}

/// What a transform drags: the whole content or a box handle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag {
    Move,
    Handle(BoxHandle),
}

/// What the user is doing with the mouse on the canvas.
/// Points are in document coordinates unless stated otherwise.
#[derive(Default)]
pub enum Gesture {
    #[default]
    None,
    /// Point selection marquee (also used by Create Shape).
    Marquee { start: Point, current: Point },
    /// Moving, scaling or rotating the selected points.
    Points {
        drag: Drag,
        bounds: Rect,
        start: Point,
        current: Point,
    },
    /// Dragging a Bézier handle; `mesh` is the working copy.
    BezierHandle {
        curve: CurveId,
        node: usize,
        handle: Handle,
        mesh: Mesh,
    },
    /// New point being dragged with Add Point; `mesh` is the working copy (it already
    /// contains the point) and `anchor` the point the segment starts from (id and layer position).
    AddPoint {
        mesh: Mesh,
        point: PointId,
        curve: CurveId,
        anchor: Option<(PointId, Point)>,
    },
    /// Curvature drag: the horizontal distance changes the curvature at the selected points.
    Curvature { start: Point, current: Point },
    /// Moving the rotation pivot of Transform Points.
    Pivot { current: Point },
    /// Rectangle or oval being drawn.
    DrawShape { start: Point, current: Point },
    /// Moving, scaling or rotating the active layer.
    Layer {
        drag: Drag,
        original: LayerTransform,
        /// The layer origin in document coordinates.
        origin: Point,
        start: Point,
        current: Point,
    },
}

/// Curvature change per screen point dragged with the Curvature tool (right = rounder).
const CURVATURE_PER_POINT: f64 = 0.005;

/// Curvature change for a horizontal drag from `start` to `current` (document coordinates).
pub fn curvature_delta(start: Point, current: Point, zoom: f64) -> f64 {
    (current.x - start.x) * zoom * CURVATURE_PER_POINT
}

/// Rectangle between two corners. `square` forces equal sides;
/// `from_center` uses `start` as the center instead of a corner.
pub fn drag_rect(start: Point, current: Point, square: bool, from_center: bool) -> Rect {
    let mut delta = current - start;
    if square {
        let side = delta.x.abs().max(delta.y.abs());
        delta.x = side.copysign(delta.x);
        delta.y = side.copysign(delta.y);
    }
    if from_center {
        Rect::from_points(start - delta, start + delta)
    } else {
        Rect::from_points(start, start + delta)
    }
}

/// Snaps `p` to multiples of 45° around `origin`.
pub fn snap_45(origin: Point, p: Point) -> Point {
    let d = p - origin;
    let step = std::f64::consts::FRAC_PI_4;
    let angle = (d.atan2() / step).round() * step;
    origin + Vec2::from_angle(angle) * d.hypot()
}

fn corner(bounds: Rect, i: u8) -> Point {
    match i {
        0 => Point::new(bounds.x0, bounds.y0),
        1 => Point::new(bounds.x1, bounds.y0),
        2 => Point::new(bounds.x1, bounds.y1),
        _ => Point::new(bounds.x0, bounds.y1),
    }
}

fn scale_about(anchor: Point, sx: f64, sy: f64) -> Affine {
    Affine::translate(anchor.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-anchor.to_vec2())
}

/// Scale and rotation of the selection around `pivot` (numeric fields of Transform Points).
pub fn scale_rotate_about(pivot: Point, scale: Vec2, angle: f64) -> Affine {
    Affine::translate(pivot.to_vec2()) * Affine::rotate(angle) * Affine::scale_non_uniform(scale.x, scale.y) * Affine::translate(-pivot.to_vec2())
}

fn rotate_about(center: Point, angle: f64) -> Affine {
    Affine::translate(center.to_vec2()) * Affine::rotate(angle) * Affine::translate(-center.to_vec2())
}

/// Scale factor that takes `from` (measured from `anchor`) to `to`. If the box has
/// no size along that axis, it is not scaled.
fn factor(anchor: f64, from: f64, to: f64) -> f64 {
    let size = from - anchor;
    if size.abs() < 1e-9 { 1.0 } else { (to - anchor) / size }
}

/// Rotation angle when dragging from `start` to `current` around `center`.
/// `snap` (Shift) rounds it to steps of `step_degrees`.
pub fn drag_angle(center: Point, start: Point, current: Point, snap: bool, step_degrees: f64) -> f64 {
    let angle = (current - center).atan2() - (start - center).atan2();
    if snap {
        let step = step_degrees.to_radians();
        (angle / step).round() * step
    } else {
        angle
    }
}

/// Transform (in document coordinates) of the selection while dragging.
/// `proportional` (Shift) keeps the aspect ratio or rotates in 45° steps;
/// `from_center` (Alt) scales from the center. Rotation is around `pivot`, as in Moho.
pub fn box_transform(drag: Drag, bounds: Rect, pivot: Point, start: Point, current: Point, proportional: bool, from_center: bool) -> Affine {
    let center = bounds.center();
    let delta = current - start;
    let handle = match drag {
        Drag::Move => return Affine::translate(delta),
        Drag::Handle(handle) => handle,
    };
    match handle {
        BoxHandle::Rotate => rotate_about(pivot, drag_angle(pivot, start, current, proportional, 45.0)),
        BoxHandle::Corner(i) => {
            let moving = corner(bounds, i);
            let anchor = if from_center { center } else { corner(bounds, (i + 2) % 4) };
            let target = moving + delta;
            let mut sx = factor(anchor.x, moving.x, target.x);
            let mut sy = factor(anchor.y, moving.y, target.y);
            if proportional {
                let s = if sx.abs() > sy.abs() { sx } else { sy };
                (sx, sy) = (s, s);
            }
            scale_about(anchor, sx, sy)
        }
        BoxHandle::Edge(i) => {
            let vertical = i % 2 == 0;
            let moving = match i {
                0 => Point::new(center.x, bounds.y0),
                1 => Point::new(bounds.x1, center.y),
                2 => Point::new(center.x, bounds.y1),
                _ => Point::new(bounds.x0, center.y),
            };
            let anchor = if from_center {
                center
            } else {
                match i {
                    0 => Point::new(center.x, bounds.y1),
                    1 => Point::new(bounds.x0, center.y),
                    2 => Point::new(center.x, bounds.y0),
                    _ => Point::new(bounds.x1, center.y),
                }
            };
            let target = moving + delta;
            let s = if vertical {
                factor(anchor.y, moving.y, target.y)
            } else {
                factor(anchor.x, moving.x, target.x)
            };
            match (vertical, proportional) {
                (_, true) => scale_about(anchor, s, s),
                (true, false) => scale_about(anchor, 1.0, s),
                (false, false) => scale_about(anchor, s, 1.0),
            }
        }
    }
}

/// New layer transform while dragging with Transform Layer.
///
/// `origin` is the layer origin in document coordinates and `to_parent` maps the document
/// to the parent's space (where the translation lives). Scaling and rotating always happen
/// around the origin, as in Moho. `shift`: free (non-proportional) scaling on corners;
/// 15° steps when rotating.
pub fn layer_drag(drag: Drag, original: LayerTransform, origin: Point, to_parent: Affine, start: Point, current: Point, shift: bool) -> LayerTransform {
    let mut t = original;
    match drag {
        Drag::Move => {
            t.translation += (to_parent * current) - (to_parent * start);
        }
        Drag::Handle(BoxHandle::Rotate) => {
            t.rotation += drag_angle(origin, start, current, shift, 15.0);
        }
        Drag::Handle(handle) => {
            let (from, to) = (start - origin, current - origin);
            let ratio = |a: f64, b: f64| if a.abs() < 1e-9 { 1.0 } else { b / a };
            let (sx, sy) = match handle {
                BoxHandle::Edge(1 | 3) => (ratio(from.x, to.x), 1.0),
                BoxHandle::Edge(_) => (1.0, ratio(from.y, to.y)),
                _ if shift => (ratio(from.x, to.x), ratio(from.y, to.y)),
                _ => {
                    let s = ratio(from.hypot(), to.hypot());
                    (s, s)
                }
            };
            t.scale = Vec2::new(original.scale.x * sx, original.scale.y * sy);
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        (a - b).hypot() < 1e-9
    }

    #[test]
    fn square_drag_keeps_direction() {
        let rect = drag_rect(Point::new(10.0, 10.0), Point::new(4.0, 30.0), true, false);
        assert_eq!(rect, Rect::new(-10.0, 10.0, 10.0, 30.0));
    }

    #[test]
    fn drag_from_center() {
        let rect = drag_rect(Point::new(10.0, 10.0), Point::new(15.0, 12.0), false, true);
        assert_eq!(rect, Rect::new(5.0, 8.0, 15.0, 12.0));
    }

    #[test]
    fn snap_goes_to_nearest_45() {
        let p = snap_45(Point::ZERO, Point::new(10.0, 9.0));
        assert!((p.x - p.y).abs() < 1e-9);
    }

    #[test]
    fn corner_scale_keeps_opposite_corner() {
        let bounds = Rect::new(0.0, 0.0, 10.0, 10.0);
        let drag = Drag::Handle(BoxHandle::Corner(2));
        let t = box_transform(drag, bounds, bounds.center(), Point::new(10.0, 10.0), Point::new(20.0, 15.0), false, false);
        assert!(close(t * Point::ZERO, Point::ZERO));
        assert!(close(t * Point::new(10.0, 10.0), Point::new(20.0, 15.0)));
    }

    #[test]
    fn edge_scale_only_one_axis() {
        let bounds = Rect::new(0.0, 0.0, 10.0, 10.0);
        let drag = Drag::Handle(BoxHandle::Edge(1));
        let t = box_transform(drag, bounds, bounds.center(), Point::new(10.0, 5.0), Point::new(20.0, 50.0), false, false);
        assert!(close(t * Point::new(10.0, 10.0), Point::new(20.0, 10.0)));
    }

    #[test]
    fn rotation_snaps_to_15_degrees() {
        let angle = drag_angle(Point::ZERO, Point::new(0.0, -10.0), Point::new(4.0, -10.0), true, 15.0);
        let steps = angle / 15f64.to_radians();
        assert!((steps - steps.round()).abs() < 1e-9);
    }

    #[test]
    fn rotation_is_around_the_pivot() {
        let bounds = Rect::new(0.0, 0.0, 10.0, 10.0);
        let pivot = Point::new(10.0, 10.0);
        let drag = Drag::Handle(BoxHandle::Rotate);
        let t = box_transform(drag, bounds, pivot, Point::new(20.0, 10.0), Point::new(10.0, 20.0), false, false);
        assert!(close(t * pivot, pivot));
        assert!(close(t * Point::new(0.0, 10.0), Point::new(10.0, 0.0)));
    }

    #[test]
    fn layer_scale_is_uniform_around_origin() {
        let origin = Point::new(100.0, 100.0);
        let t = layer_drag(
            Drag::Handle(BoxHandle::Corner(2)),
            LayerTransform::default(),
            origin,
            Affine::IDENTITY,
            Point::new(110.0, 100.0),
            Point::new(130.0, 100.0),
            false,
        );
        assert!((t.scale.x - 3.0).abs() < 1e-9 && (t.scale.y - 3.0).abs() < 1e-9);
    }

    #[test]
    fn layer_move_uses_parent_space() {
        // The parent is scaled 2x: moving 10 on screen is 5 in the parent.
        let to_parent = Affine::scale(2.0).inverse();
        let t = layer_drag(Drag::Move, LayerTransform::default(), Point::ZERO, to_parent, Point::ZERO, Point::new(10.0, 0.0), false);
        assert!((t.translation - Vec2::new(5.0, 0.0)).hypot() < 1e-9);
    }
}
