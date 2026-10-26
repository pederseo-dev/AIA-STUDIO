//! Tool icons drawn with strokes (our own, in the spirit of Moho's), so they stay sharp at
//! any scale and take the theme color. Each one draws on a 16×16 grid inside `rect`.

use eframe::egui::epaint::CubicBezierShape;
use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};

/// Maps a point of the 16×16 grid into `rect`.
struct Grid {
    rect: Rect,
    unit: f32,
}

impl Grid {
    fn new(rect: Rect) -> Self {
        Self { rect, unit: rect.width().min(rect.height()) / 16.0 }
    }

    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + vec2(x, y) * self.unit
    }

    fn stroke(&self, color: Color32) -> Stroke {
        Stroke::new(1.4 * self.unit, color)
    }

    /// Point of the mesh: a small filled square.
    fn point(&self, painter: &Painter, x: f32, y: f32, color: Color32) {
        let size = vec2(3.0, 3.0) * self.unit;
        painter.rect_filled(Rect::from_center_size(self.p(x, y), size), 0.0, color);
    }

    /// The arch used by the curve tools.
    fn arch(&self, painter: &Painter, color: Color32) {
        let points = [self.p(2.0, 14.0), self.p(2.5, 2.0), self.p(13.5, 2.0), self.p(14.0, 14.0)];
        painter.add(CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, self.stroke(color)));
    }
}

/// Dashed square with a point at each corner.
pub fn select_points(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    let corners = [g.p(2.0, 2.0), g.p(14.0, 2.0), g.p(14.0, 14.0), g.p(2.0, 14.0), g.p(2.0, 2.0)];
    painter.extend(Shape::dashed_line(&corners, Stroke::new(g.unit, color), 2.0 * g.unit, 1.5 * g.unit));
    for (x, y) in [(2.0, 2.0), (14.0, 2.0), (14.0, 14.0), (2.0, 14.0)] {
        g.point(painter, x, y, color);
    }
}

/// Cross of four arrows with a point in the center.
pub fn transform_points(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    let stroke = g.stroke(color);
    painter.line_segment([g.p(1.0, 8.0), g.p(15.0, 8.0)], stroke);
    painter.line_segment([g.p(8.0, 1.0), g.p(8.0, 15.0)], stroke);
    // Arrow heads: (tip, the two wing ends).
    for (tip, a, b) in [
        ((1.0, 8.0), (3.5, 5.5), (3.5, 10.5)),
        ((15.0, 8.0), (12.5, 5.5), (12.5, 10.5)),
        ((8.0, 1.0), (5.5, 3.5), (10.5, 3.5)),
        ((8.0, 15.0), (5.5, 12.5), (10.5, 12.5)),
    ] {
        let tip = g.p(tip.0, tip.1);
        painter.line_segment([tip, g.p(a.0, a.1)], stroke);
        painter.line_segment([tip, g.p(b.0, b.1)], stroke);
    }
    painter.circle_filled(g.p(8.0, 8.0), 1.8 * g.unit, color);
}

/// Arch with its points and a "+" at the top right.
pub fn add_point(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    g.arch(painter, color);
    g.point(painter, 2.0, 14.0, color);
    g.point(painter, 14.0, 14.0, color);
    let stroke = g.stroke(color);
    painter.line_segment([g.p(10.0, 6.0), g.p(16.0, 6.0)], stroke);
    painter.line_segment([g.p(13.0, 3.0), g.p(13.0, 9.0)], stroke);
}

/// Arch with a highlighted point at the top and its handle.
pub fn curvature(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    g.arch(painter, color);
    painter.line_segment([g.p(3.0, 5.0), g.p(13.0, 5.0)], Stroke::new(g.unit, color));
    for x in [3.0, 13.0] {
        painter.circle_stroke(g.p(x, 5.0), 1.3 * g.unit, Stroke::new(g.unit, color));
    }
    painter.circle_filled(g.p(8.0, 5.0), 2.0 * g.unit, color);
    g.point(painter, 2.0, 14.0, color);
    g.point(painter, 14.0, 14.0, color);
}

/// Overlapping square and circle.
pub fn draw_shape(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    let stroke = g.stroke(color);
    painter.rect_stroke(
        Rect::from_min_max(g.p(1.5, 1.5), g.p(10.0, 10.0)),
        0.0,
        stroke,
        eframe::egui::StrokeKind::Middle,
    );
    painter.circle_stroke(g.p(10.5, 10.5), 4.8 * g.unit, stroke);
}

/// Curve cut in the middle with an "×".
pub fn delete_edge(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    let stroke = g.stroke(color);
    let left = [g.p(1.0, 14.0), g.p(1.5, 8.0), g.p(3.0, 5.0), g.p(5.0, 4.0)];
    let right = [g.p(11.0, 4.0), g.p(13.0, 5.0), g.p(14.5, 8.0), g.p(15.0, 14.0)];
    for points in [left, right] {
        painter.add(CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, stroke));
    }
    g.point(painter, 1.0, 14.0, color);
    g.point(painter, 15.0, 14.0, color);
    painter.line_segment([g.p(6.0, 1.5), g.p(10.0, 5.5)], stroke);
    painter.line_segment([g.p(10.0, 1.5), g.p(6.0, 5.5)], stroke);
}

// ---- Cursor badges (small symbols drawn next to the mouse, as in Moho) --------------------------

/// Circular arrow: rotate.
pub fn rotate(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    let stroke = g.stroke(color);
    let center = g.p(8.0, 8.0);
    let radius = 5.5 * g.unit;
    // Three quarters of a circle, from the top going clockwise.
    let points: Vec<Pos2> = (0..=24)
        .map(|i| {
            let angle = -std::f32::consts::FRAC_PI_2 + i as f32 / 24.0 * 1.5 * std::f32::consts::PI;
            center + vec2(angle.cos(), angle.sin()) * radius
        })
        .collect();
    let tip = points[0];
    painter.add(Shape::line(points, stroke));
    painter.line_segment([tip, tip + vec2(3.0, -2.5) * g.unit], stroke);
    painter.line_segment([tip, tip + vec2(3.0, 2.5) * g.unit], stroke);
}

/// Diagonal double arrow: scale.
pub fn scale(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    let stroke = g.stroke(color);
    let (a, b) = (g.p(3.0, 13.0), g.p(13.0, 3.0));
    painter.line_segment([a, b], stroke);
    for (tip, dir) in [(a, vec2(1.0, -1.0)), (b, vec2(-1.0, 1.0))] {
        painter.line_segment([tip, tip + vec2(dir.x, 0.0) * 4.0 * g.unit], stroke);
        painter.line_segment([tip, tip + vec2(0.0, dir.y) * 4.0 * g.unit], stroke);
    }
}

/// Crosshair with a circle: the rotation pivot.
pub fn pivot(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    let stroke = g.stroke(color);
    painter.circle_stroke(g.p(8.0, 8.0), 3.5 * g.unit, stroke);
    painter.line_segment([g.p(1.0, 8.0), g.p(15.0, 8.0)], stroke);
    painter.line_segment([g.p(8.0, 1.0), g.p(8.0, 15.0)], stroke);
}

/// Two points meeting in a ring: weld.
pub fn weld(painter: &Painter, rect: Rect, color: Color32) {
    let g = Grid::new(rect);
    let stroke = g.stroke(color);
    painter.circle_stroke(g.p(8.0, 8.0), 4.0 * g.unit, stroke);
    painter.circle_filled(g.p(8.0, 8.0), 1.8 * g.unit, color);
    painter.line_segment([g.p(0.5, 8.0), g.p(4.0, 8.0)], stroke);
    painter.line_segment([g.p(12.0, 8.0), g.p(15.5, 8.0)], stroke);
}
