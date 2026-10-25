//! Vector mesh of a layer (like Moho's `M_Mesh`): shared points, curves passing through
//! those points, and shapes (fill and outline) built from curve edges.
//!
//! Points and curves are the geometry and are not rendered: what you see are the shapes.
//! Two curves passing through the same point are welded.

use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveNearest, PathSeg, Point, Rect, Shape as _, Vec2};
use serde::{Deserialize, Serialize};

use crate::style::Style;

/// Default curvature of auto nodes: four points make a circle.
pub const DEFAULT_CURVATURE: f64 = 0.552_284_749_830_793_4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PointId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CurveId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ShapeId(pub u64);

/// A control point. It is what moves when editing and, later, what bones animate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshPoint {
    pub id: PointId,
    pub pos: Point,
    /// Line width multiplier where the stroke passes through the point (1 = the style's width).
    pub width: f64,
}

impl MeshPoint {
    pub fn new(id: PointId, pos: Point) -> Self {
        Self { id, pos, width: 1.0 }
    }
}

/// How a curve's handles at a point are computed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HandleMode {
    /// Handles computed from the neighboring points (like Moho). `curvature: 0` = peak.
    Auto { curvature: f64 },
    /// Independent manual handles.
    Corner,
    /// Aligned manual handles with free lengths.
    Smooth,
    /// Aligned manual handles of equal length.
    Symmetric,
}

impl HandleMode {
    pub const SMOOTH: HandleMode = HandleMode::Auto {
        curvature: DEFAULT_CURVATURE,
    };
    pub const PEAK: HandleMode = HandleMode::Auto { curvature: 0.0 };
}

/// Which of a node's two handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    In,
    Out,
}

/// A curve passing through a point. Handles belong to the curve (not the point) and are relative to it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurveNode {
    pub point: PointId,
    pub mode: HandleMode,
    /// Only used in the manual modes.
    pub handle_in: Vec2,
    pub handle_out: Vec2,
}

impl CurveNode {
    pub fn new(point: PointId, mode: HandleMode) -> Self {
        Self {
            point,
            mode,
            handle_in: Vec2::ZERO,
            handle_out: Vec2::ZERO,
        }
    }

    /// Moves a manual handle, respecting the mode.
    pub fn set_handle(&mut self, handle: Handle, value: Vec2) {
        let (moved, other) = match handle {
            Handle::In => (&mut self.handle_in, &mut self.handle_out),
            Handle::Out => (&mut self.handle_out, &mut self.handle_in),
        };
        *moved = value;
        match self.mode {
            HandleMode::Corner => {}
            HandleMode::Symmetric => *other = -value,
            HandleMode::Smooth | HandleMode::Auto { .. } => {
                if value.hypot() > f64::EPSILON {
                    *other = -value.normalize() * other.hypot();
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    pub id: CurveId,
    pub closed: bool,
    pub nodes: Vec<CurveNode>,
}

impl Curve {
    /// Number of segments (includes the closing one if the curve is closed).
    pub fn segment_count(&self) -> usize {
        match self.nodes.len() {
            0 | 1 => 0,
            n if self.closed => n,
            n => n - 1,
        }
    }

    /// Index of the next node (wraps around on closed curves).
    pub fn next(&self, i: usize) -> usize {
        (i + 1) % self.nodes.len()
    }

    pub fn node_index(&self, point: PointId) -> Option<usize> {
        self.nodes.iter().position(|n| n.point == point)
    }

    /// `true` if the point is the first or last node of an open curve.
    pub fn is_endpoint(&self, point: PointId) -> bool {
        !self.closed && (self.nodes.first().map(|n| n.point) == Some(point) || self.nodes.last().map(|n| n.point) == Some(point))
    }

    pub fn edges(&self) -> Vec<EdgeRef> {
        (0..self.segment_count()).map(|s| EdgeRef::forward(self.id, s)).collect()
    }
}

/// A curve segment, from node `segment` to the next one (or backwards if `reversed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeRef {
    pub curve: CurveId,
    pub segment: usize,
    #[serde(default)]
    pub reversed: bool,
}

impl EdgeRef {
    pub fn forward(curve: CurveId, segment: usize) -> Self {
        Self {
            curve,
            segment,
            reversed: false,
        }
    }
}

/// A closed contour: edges chained in order.
pub type Loop = Vec<EdgeRef>;

/// What is visible: a fill and/or an outline over the curves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub id: ShapeId,
    /// Display name ("head", "left_arm"): used by the style panel and the AI.
    pub name: String,
    /// Closed contours of the region to fill (more than one = holes). Empty = cannot be filled.
    pub fill: Vec<Loop>,
    /// Edges drawn with the stroke.
    pub outline: Vec<EdgeRef>,
    pub style: Style,
}

/// Points, curves and shapes of a vector layer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    /// Sorted by id for fast lookup.
    points: Vec<MeshPoint>,
    pub curves: Vec<Curve>,
    /// Stacking order: bottom to top.
    pub shapes: Vec<Shape>,
}

impl Mesh {
    // ---- Points -----------------------------------------------------------------------------

    pub fn points(&self) -> &[MeshPoint] {
        &self.points
    }

    pub fn point(&self, id: PointId) -> Option<&MeshPoint> {
        self.points.binary_search_by_key(&id, |p| p.id).ok().map(|i| &self.points[i])
    }

    pub fn point_mut(&mut self, id: PointId) -> Option<&mut MeshPoint> {
        self.points.binary_search_by_key(&id, |p| p.id).ok().map(|i| &mut self.points[i])
    }

    pub fn pos(&self, id: PointId) -> Option<Point> {
        self.point(id).map(|p| p.pos)
    }

    /// Adds a point (or replaces it if that id already exists).
    pub fn add_point(&mut self, point: MeshPoint) {
        match self.points.binary_search_by_key(&point.id, |p| p.id) {
            Ok(i) => self.points[i] = point,
            Err(i) => self.points.insert(i, point),
        }
    }

    pub(crate) fn remove_point(&mut self, id: PointId) {
        if let Ok(i) = self.points.binary_search_by_key(&id, |p| p.id) {
            self.points.remove(i);
        }
    }

    pub(crate) fn retain_points(&mut self, keep: impl FnMut(&MeshPoint) -> bool) {
        self.points.retain(keep);
    }

    /// Restores the id order (for example, after reading a file).
    pub(crate) fn sort_points(&mut self) {
        self.points.sort_by_key(|p| p.id);
    }

    // ---- Curves and shapes --------------------------------------------------------------------

    pub fn curve(&self, id: CurveId) -> Option<&Curve> {
        self.curves.iter().find(|c| c.id == id)
    }

    pub fn curve_mut(&mut self, id: CurveId) -> Option<&mut Curve> {
        self.curves.iter_mut().find(|c| c.id == id)
    }

    pub fn shape(&self, id: ShapeId) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }

    pub fn shape_mut(&mut self, id: ShapeId) -> Option<&mut Shape> {
        self.shapes.iter_mut().find(|s| s.id == id)
    }

    pub fn shape_index(&self, id: ShapeId) -> Option<usize> {
        self.shapes.iter().position(|s| s.id == id)
    }

    /// Curves passing through the point: (curve index, node index).
    pub fn curves_at(&self, point: PointId) -> Vec<(usize, usize)> {
        self.curves
            .iter()
            .enumerate()
            .flat_map(|(ci, c)| {
                c.nodes
                    .iter()
                    .enumerate()
                    .filter(move |(_, n)| n.point == point)
                    .map(move |(ni, _)| (ci, ni))
            })
            .collect()
    }

    /// The points touched by a shape's edges.
    pub fn shape_points(&self, shape: &Shape) -> Vec<PointId> {
        let mut points = Vec::new();
        for edge in shape.fill.iter().flatten().chain(&shape.outline) {
            if let Some((a, b)) = self.edge_points(*edge) {
                for p in [a, b] {
                    if !points.contains(&p) {
                        points.push(p);
                    }
                }
            }
        }
        points
    }

    /// Start and end points of an edge, in the direction it is traversed.
    pub fn edge_points(&self, edge: EdgeRef) -> Option<(PointId, PointId)> {
        let curve = self.curve(edge.curve)?;
        if edge.segment >= curve.segment_count() {
            return None;
        }
        let a = curve.nodes[edge.segment].point;
        let b = curve.nodes[curve.next(edge.segment)].point;
        Some(if edge.reversed { (b, a) } else { (a, b) })
    }

    // ---- Geometry --------------------------------------------------------------------------

    /// Effective handles (relative to the point) of node `i`, resolving the auto mode.
    pub fn handles(&self, curve: &Curve, i: usize) -> (Vec2, Vec2) {
        let node = &curve.nodes[i];
        let HandleMode::Auto { curvature } = node.mode else {
            return (node.handle_in, node.handle_out);
        };
        let n = curve.nodes.len();
        let interior = curve.closed || (i > 0 && i + 1 < n);
        if n < 3 || !interior || curvature == 0.0 {
            return (Vec2::ZERO, Vec2::ZERO);
        }
        let pos = |k: usize| self.pos(curve.nodes[k].point).unwrap_or(Point::ZERO);
        let (prev, here, next) = (pos((i + n - 1) % n), pos(i), pos((i + 1) % n));
        let (len_prev, len_next) = ((here - prev).hypot(), (next - here).hypot());
        let total = len_prev + len_next;
        if total < f64::EPSILON {
            return (Vec2::ZERO, Vec2::ZERO);
        }
        // Tangent parallel to the line between the neighbors, split by the distance to each one.
        let tangent = (next - prev) * curvature;
        (-tangent * (len_prev / total), tangent * (len_next / total))
    }

    /// Segment `i` of a curve, from node `i` to the next one.
    pub fn segment(&self, curve: &Curve, i: usize) -> PathSeg {
        let j = curve.next(i);
        let a = self.pos(curve.nodes[i].point).unwrap_or(Point::ZERO);
        let b = self.pos(curve.nodes[j].point).unwrap_or(Point::ZERO);
        let (_, out) = self.handles(curve, i);
        let (inn, _) = self.handles(curve, j);
        if out == Vec2::ZERO && inn == Vec2::ZERO {
            PathSeg::Line(Line::new(a, b))
        } else {
            PathSeg::Cubic(CubicBez::new(a, a + out, b + inn, b))
        }
    }

    pub fn edge_segment(&self, edge: EdgeRef) -> Option<PathSeg> {
        let curve = self.curve(edge.curve)?;
        if edge.segment >= curve.segment_count() {
            return None;
        }
        let seg = self.segment(curve, edge.segment);
        Some(if edge.reversed { seg.reverse() } else { seg })
    }

    pub fn curve_path(&self, curve: &Curve) -> BezPath {
        let mut path = BezPath::new();
        if let Some(first) = curve.nodes.first().and_then(|n| self.pos(n.point)) {
            path.move_to(first);
            for i in 0..curve.segment_count() {
                append(&mut path, self.segment(curve, i));
            }
            if curve.closed {
                path.close_path();
            }
        }
        path
    }

    /// Path of a shape's fill, or `None` if it cannot be filled.
    pub fn fill_path(&self, shape: &Shape) -> Option<BezPath> {
        let mut path = BezPath::new();
        for lp in &shape.fill {
            for (k, edge) in lp.iter().enumerate() {
                let Some(seg) = self.edge_segment(*edge) else { continue };
                if k == 0 {
                    path.move_to(seg.start());
                }
                append(&mut path, seg);
            }
            path.close_path();
        }
        (!shape.fill.is_empty()).then_some(path)
    }

    /// Outline paths: consecutive edges of the same curve go into a single path.
    pub fn outline_paths(&self, shape: &Shape) -> Vec<BezPath> {
        let mut paths = Vec::new();
        let mut done: Vec<CurveId> = Vec::new();
        for edge in &shape.outline {
            if done.contains(&edge.curve) {
                continue;
            }
            done.push(edge.curve);
            let Some(curve) = self.curve(edge.curve) else { continue };
            let count = curve.segment_count();
            let mut on = vec![false; count];
            for e in shape.outline.iter().filter(|e| e.curve == curve.id && e.segment < count) {
                on[e.segment] = true;
            }
            if curve.closed && on.iter().all(|&b| b) {
                paths.push(self.curve_path(curve));
                continue;
            }
            // Start right after a missing segment so runs are not split when wrapping around.
            let start = if curve.closed {
                on.iter().position(|&b| !b).map_or(0, |i| (i + 1) % count)
            } else {
                0
            };
            let mut current: Option<BezPath> = None;
            for k in 0..count {
                let s = (start + k) % count;
                if on[s] {
                    let seg = self.segment(curve, s);
                    let path = current.get_or_insert_with(|| {
                        let mut p = BezPath::new();
                        p.move_to(seg.start());
                        p
                    });
                    append(path, seg);
                } else if let Some(path) = current.take() {
                    paths.push(path);
                }
            }
            paths.extend(current);
        }
        paths
    }

    /// Bounding box of the shape in layer coordinates (includes half the stroke width).
    pub fn shape_bounds(&self, shape: &Shape) -> Option<Rect> {
        let mut bounds = self.fill_path(shape).map(|p| p.bounding_box());
        for path in self.outline_paths(shape) {
            let b = path.bounding_box();
            bounds = Some(bounds.map_or(b, |r| r.union(b)));
        }
        let half = shape.style.stroke.map_or(0.0, |s| s.width / 2.0);
        bounds.map(|b| b.inflate(half, half))
    }

    // ---- Hit testing (layer coordinates) ------------------------------------

    /// The nearest point within `tolerance`.
    pub fn nearest_point(&self, p: Point, tolerance: f64) -> Option<PointId> {
        self.points
            .iter()
            .map(|mp| (mp.id, mp.pos.distance_squared(p)))
            .filter(|(_, d)| *d <= tolerance * tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// The nearest segment within `tolerance`: (curve, segment, parameter t).
    pub fn nearest_segment(&self, p: Point, tolerance: f64) -> Option<(CurveId, usize, f64)> {
        self.curves
            .iter()
            .flat_map(|c| (0..c.segment_count()).map(move |s| (c, s)))
            .map(|(c, s)| (c.id, s, self.segment(c, s).nearest(p, 1e-3)))
            .filter(|(_, _, n)| n.distance_sq <= tolerance * tolerance)
            .min_by(|a, b| a.2.distance_sq.total_cmp(&b.2.distance_sq))
            .map(|(c, s, n)| (c, s, n.t))
    }

    /// The topmost visible shape touching the point.
    pub fn shape_at(&self, p: Point, tolerance: f64) -> Option<ShapeId> {
        self.shapes.iter().rev().find(|s| self.shape_hit(s, p, tolerance)).map(|s| s.id)
    }

    /// A shape without fill and stroke is invisible but still hit by its area and its outline,
    /// so it can be selected again.
    fn shape_hit(&self, shape: &Shape, p: Point, tolerance: f64) -> bool {
        let invisible = shape.style.fill.is_none() && shape.style.stroke.is_none();
        if (shape.style.fill.is_some() || invisible) && self.fill_path(shape).is_some_and(|path| path.contains(p)) {
            return true;
        }
        let width = match shape.style.stroke {
            Some(stroke) => stroke.width,
            None if invisible => 0.0,
            None => return false,
        };
        let reach = width / 2.0 + tolerance;
        shape
            .outline
            .iter()
            .filter_map(|e| self.edge_segment(*e))
            .any(|seg| seg.nearest(p, 1e-3).distance_sq <= reach * reach)
    }
}

fn append(path: &mut BezPath, seg: PathSeg) {
    match seg {
        PathSeg::Line(l) => path.line_to(l.p1),
        PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
        PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(r: f64) -> Mesh {
        let mut mesh = Mesh::default();
        let pts = [(0.0, -r), (r, 0.0), (0.0, r), (-r, 0.0)];
        let nodes = pts
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                mesh.add_point(MeshPoint::new(PointId(i as u64 + 1), Point::new(x, y)));
                CurveNode::new(PointId(i as u64 + 1), HandleMode::SMOOTH)
            })
            .collect();
        mesh.curves.push(Curve {
            id: CurveId(10),
            closed: true,
            nodes,
        });
        mesh
    }

    #[test]
    fn four_auto_points_make_a_circle() {
        let mesh = circle(10.0);
        let area = mesh.curve_path(&mesh.curves[0]).area().abs();
        let exact = std::f64::consts::PI * 100.0;
        assert!((area - exact).abs() / exact < 0.001, "area {area}");
    }

    #[test]
    fn peak_points_make_straight_lines() {
        let mut mesh = circle(10.0);
        for node in &mut mesh.curves[0].nodes {
            node.mode = HandleMode::PEAK;
        }
        // A rhombus with diagonals of 20: area 200.
        assert!((mesh.curve_path(&mesh.curves[0]).area().abs() - 200.0).abs() < 1e-9);
    }

    #[test]
    fn points_stay_sorted() {
        let mut mesh = Mesh::default();
        for id in [5, 2, 9, 1] {
            mesh.add_point(MeshPoint::new(PointId(id), Point::ZERO));
        }
        let ids: Vec<u64> = mesh.points().iter().map(|p| p.id.0).collect();
        assert_eq!(ids, vec![1, 2, 5, 9]);
        assert!(mesh.point(PointId(5)).is_some());
    }

    #[test]
    fn outline_of_partial_closed_curve_is_one_path() {
        let mut mesh = circle(10.0);
        let curve = mesh.curves[0].id;
        // Without segment 1 the stroke must be a single run 2 → 3 → 0 → 1.
        mesh.shapes.push(Shape {
            id: ShapeId(20),
            name: "arc".into(),
            fill: Vec::new(),
            outline: [0, 2, 3].map(|s| EdgeRef::forward(curve, s)).to_vec(),
            style: Style::default(),
        });
        assert_eq!(mesh.outline_paths(&mesh.shapes[0]).len(), 1);
    }
}
