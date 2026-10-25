//! Operations that change a mesh's structure: adding points on curves, deleting points
//! or edges, welding, creating shapes from points.
//!
//! Shapes reference edges by segment index, which these operations change. So before each
//! change shapes are saved as point sequences and their edges are rebuilt afterwards. If a
//! border no longer exists (for example, its edge was deleted) the fill is lost, as in Moho.

use std::collections::{HashMap, HashSet};

use kurbo::{CubicBez, ParamCurve, PathSeg, Point, Vec2};

use crate::mesh::{Curve, CurveId, CurveNode, EdgeRef, HandleMode, Loop, Mesh, MeshPoint, PointId, Shape, ShapeId};
use crate::style::{Color, StrokeStyle, Style};

/// What happened to the points during an operation.
#[derive(Default)]
struct Change {
    removed: HashSet<PointId>,
    /// Welded points: old id → id that replaces it.
    renamed: HashMap<PointId, PointId>,
}

/// A shape described by points, independent of segment indices.
struct SavedShape {
    fill: Vec<Vec<PointId>>,
    outline: Vec<(PointId, PointId)>,
}

impl Mesh {
    /// Applies `edit` and rebuilds the shapes' edges. Shapes left with neither fill nor
    /// outline are deleted.
    fn retopologize(&mut self, edit: impl FnOnce(&mut Mesh) -> Change) {
        let old: HashSet<PointId> = self.points().iter().map(|p| p.id).collect();
        let saved: Vec<SavedShape> = self
            .shapes
            .iter()
            .map(|s| SavedShape {
                fill: s
                    .fill
                    .iter()
                    .map(|lp| lp.iter().filter_map(|e| self.edge_points(*e)).map(|(a, _)| a).collect())
                    .collect(),
                outline: s.outline.iter().filter_map(|e| self.edge_points(*e)).collect(),
            })
            .collect();

        let change = edit(self);

        let mut shapes = std::mem::take(&mut self.shapes);
        for (shape, saved) in shapes.iter_mut().zip(saved) {
            shape.fill = self.restore_fill(&saved.fill, &old, &change);
            shape.outline = self.restore_outline(saved.outline, &old, &change);
        }
        shapes.retain(|s| !s.fill.is_empty() || !s.outline.is_empty());
        self.shapes = shapes;
    }

    fn restore_fill(&self, cycles: &[Vec<PointId>], old: &HashSet<PointId>, change: &Change) -> Vec<Loop> {
        let mut loops = Vec::new();
        for cycle in cycles {
            let mut points: Vec<PointId> = cycle
                .iter()
                .map(|p| *change.renamed.get(p).unwrap_or(p))
                .filter(|p| !change.removed.contains(p))
                .collect();
            points.dedup();
            while points.len() > 1 && points.first() == points.last() {
                points.pop();
            }
            if points.len() < 2 {
                return Vec::new();
            }
            let mut used = HashSet::new();
            let mut edges = Vec::new();
            for k in 0..points.len() {
                let (a, b) = (points[k], points[(k + 1) % points.len()]);
                match self.find_edges(a, b, old, &used) {
                    Some(found) => {
                        used.extend(found.iter().map(|e| (e.curve, e.segment)));
                        edges.extend(found);
                    }
                    // A border is missing: the fill is no longer closed.
                    None => return Vec::new(),
                }
            }
            loops.push(edges);
        }
        loops
    }

    fn restore_outline(&self, pairs: Vec<(PointId, PointId)>, old: &HashSet<PointId>, change: &Change) -> Vec<EdgeRef> {
        let rename = |p: PointId| *change.renamed.get(&p).unwrap_or(&p);
        let mut pairs: Vec<(PointId, PointId)> = pairs
            .into_iter()
            .map(|(a, b)| (rename(a), rename(b)))
            .filter(|(a, b)| a != b)
            .collect();
        // When a point is deleted its two runs merge into one (the curve goes neighbor to neighbor).
        for &r in &change.removed {
            let touching: Vec<usize> = (0..pairs.len()).filter(|&i| pairs[i].0 == r || pairs[i].1 == r).collect();
            let other = |(a, b): (PointId, PointId)| if a == r { b } else { a };
            let merged = (touching.len() == 2).then(|| (other(pairs[touching[0]]), other(pairs[touching[1]])));
            for &i in touching.iter().rev() {
                pairs.remove(i);
            }
            if let Some((a, b)) = merged
                && a != b
            {
                pairs.push((a, b));
            }
        }
        let mut edges: Vec<EdgeRef> = Vec::new();
        for (a, b) in pairs {
            for edge in self.find_edges(a, b, old, &HashSet::new()).unwrap_or_default() {
                if !edges.iter().any(|e| e.curve == edge.curve && e.segment == edge.segment) {
                    edges.push(edge);
                }
            }
        }
        edges
    }

    /// Edges joining `a` to `b` along a curve. They may pass through new points (not in
    /// `old`), so a border that was split is still found.
    fn find_edges(&self, a: PointId, b: PointId, old: &HashSet<PointId>, avoid: &HashSet<(CurveId, usize)>) -> Option<Vec<EdgeRef>> {
        for curve in &self.curves {
            let n = curve.nodes.len();
            for start in (0..n).filter(|&i| curve.nodes[i].point == a) {
                for forward in [true, false] {
                    let mut edges = Vec::new();
                    let mut k = start;
                    loop {
                        let (segment, next) = if forward {
                            if !curve.closed && k + 1 >= n {
                                break;
                            }
                            (k, (k + 1) % n)
                        } else {
                            if !curve.closed && k == 0 {
                                break;
                            }
                            let prev = (k + n - 1) % n;
                            (prev, prev)
                        };
                        edges.push(EdgeRef {
                            curve: curve.id,
                            segment,
                            reversed: !forward,
                        });
                        let p = curve.nodes[next].point;
                        if p == b {
                            if !avoid.contains(&(curve.id, edges[0].segment)) {
                                return Some(edges);
                            }
                            break;
                        }
                        if old.contains(&p) || next == start {
                            break;
                        }
                        k = next;
                    }
                }
            }
        }
        None
    }

    /// Freezes the effective handles of an auto node (so it does not change when neighbors move).
    fn bake_node(&mut self, curve: usize, node: usize) {
        let (handle_in, handle_out) = self.handles(&self.curves[curve], node);
        let n = &mut self.curves[curve].nodes[node];
        if let HandleMode::Auto { .. } = n.mode {
            n.mode = if handle_in == Vec2::ZERO && handle_out == Vec2::ZERO {
                HandleMode::Corner
            } else {
                HandleMode::Smooth
            };
            n.handle_in = handle_in;
            n.handle_out = handle_out;
        }
    }

    // ---- Operations --------------------------------------------------------------------------

    /// Adds point `id` on segment `segment` of the curve, at parameter `t`.
    pub fn insert_point(&mut self, curve: CurveId, segment: usize, t: f64, id: PointId) -> bool {
        let Some(ci) = self.curves.iter().position(|c| c.id == curve) else { return false };
        if segment >= self.curves[ci].segment_count() {
            return false;
        }
        self.retopologize(|mesh| {
            let c = &mesh.curves[ci];
            let j = c.next(segment);
            let seg = mesh.segment(c, segment);
            let pos = seg.eval(t);
            let node = match (c.nodes[segment].mode, c.nodes[j].mode, seg) {
                // Between two auto nodes the new one is auto as well.
                (HandleMode::Auto { curvature }, HandleMode::Auto { .. }, _) => CurveNode::new(id, HandleMode::Auto { curvature }),
                (_, _, PathSeg::Line(_)) => CurveNode::new(id, HandleMode::Corner),
                (_, _, PathSeg::Cubic(cubic)) => {
                    mesh.bake_node(ci, segment);
                    mesh.bake_node(ci, j);
                    split_cubic(mesh, ci, segment, j, cubic, t, id)
                }
                (_, _, PathSeg::Quad(_)) => CurveNode::new(id, HandleMode::Corner),
            };
            mesh.add_point(MeshPoint::new(id, pos));
            mesh.curves[ci].nodes.insert(segment + 1, node);
            Change::default()
        });
        true
    }

    /// Deletes points. Curves passing through them continue from neighbor to neighbor.
    pub fn delete_points(&mut self, ids: &HashSet<PointId>) {
        self.retopologize(|mesh| {
            for curve in &mut mesh.curves {
                curve.nodes.retain(|n| !ids.contains(&n.point));
                if curve.nodes.len() < 3 {
                    curve.closed = false;
                }
            }
            mesh.curves.retain(|c| c.nodes.len() >= 2);
            mesh.retain_points(|p| !ids.contains(&p.id));
            Change {
                removed: ids.clone(),
                ..Default::default()
            }
        });
    }

    /// Deletes a segment without deleting its points. An open curve is split in two
    /// (the second one gets `new_curve`); a closed one is opened.
    pub fn delete_edge(&mut self, curve: CurveId, segment: usize, new_curve: CurveId) -> bool {
        let Some(ci) = self.curves.iter().position(|c| c.id == curve) else { return false };
        if segment >= self.curves[ci].segment_count() {
            return false;
        }
        self.retopologize(|mesh| {
            let j = mesh.curves[ci].next(segment);
            // The new ends keep the shape of what remains.
            mesh.bake_node(ci, segment);
            mesh.bake_node(ci, j);
            let c = &mut mesh.curves[ci];
            if c.closed {
                c.nodes.rotate_left(segment + 1);
                c.closed = false;
            } else {
                let right = c.nodes.split_off(segment + 1);
                if right.len() >= 2 {
                    mesh.curves.push(Curve {
                        id: new_curve,
                        closed: false,
                        nodes: right,
                    });
                }
            }
            mesh.curves.retain(|c| c.nodes.len() >= 2);
            Change::default()
        });
        true
    }

    /// Welds `moving` onto `target`: curves through `moving` now use `target`.
    /// If both ends of an open curve are joined, the curve closes.
    pub fn weld(&mut self, moving: PointId, target: PointId) -> bool {
        if moving == target || self.point(moving).is_none() || self.point(target).is_none() {
            return false;
        }
        self.retopologize(|mesh| {
            for curve in &mut mesh.curves {
                for node in &mut curve.nodes {
                    if node.point == moving {
                        node.point = target;
                    }
                }
                curve.nodes.dedup_by_key(|n| n.point);
                let n = curve.nodes.len();
                if n >= 3 && curve.nodes[0].point == curve.nodes[n - 1].point {
                    curve.nodes.pop();
                    curve.closed = true;
                }
                if curve.nodes.len() < 3 {
                    curve.closed = false;
                }
            }
            mesh.curves.retain(|c| c.nodes.len() >= 2);
            mesh.remove_point(moving);
            Change {
                renamed: HashMap::from([(moving, target)]),
                ..Default::default()
            }
        });
        true
    }

    /// Edges between points in `selected`, to create a shape (Create Shape tool).
    /// Returns (fill contours, outline). If the edges do not form closed cycles,
    /// there is no fill.
    pub fn shape_from_points(&self, selected: &HashSet<PointId>) -> Option<(Vec<Loop>, Vec<EdgeRef>)> {
        let mut edges: Vec<(EdgeRef, PointId, PointId)> = Vec::new();
        for curve in &self.curves {
            for s in 0..curve.segment_count() {
                let (a, b) = (curve.nodes[s].point, curve.nodes[curve.next(s)].point);
                if selected.contains(&a) && selected.contains(&b) {
                    edges.push((EdgeRef::forward(curve.id, s), a, b));
                }
            }
        }
        if edges.is_empty() {
            return None;
        }
        let outline: Vec<EdgeRef> = edges.iter().map(|e| e.0).collect();

        let mut degree: HashMap<PointId, usize> = HashMap::new();
        for (_, a, b) in &edges {
            *degree.entry(*a).or_default() += 1;
            *degree.entry(*b).or_default() += 1;
        }
        if degree.values().any(|&d| d != 2) {
            return Some((Vec::new(), outline));
        }

        let mut used = vec![false; edges.len()];
        let mut loops = Vec::new();
        for first in 0..edges.len() {
            if used[first] {
                continue;
            }
            used[first] = true;
            let (edge, start, mut current) = edges[first];
            let mut lp = vec![edge];
            while current != start {
                let Some(k) = (0..edges.len()).find(|&k| !used[k] && (edges[k].1 == current || edges[k].2 == current)) else {
                    return Some((Vec::new(), outline));
                };
                used[k] = true;
                let (e, a, b) = edges[k];
                if a == current {
                    lp.push(e);
                    current = b;
                } else {
                    lp.push(EdgeRef { reversed: true, ..e });
                    current = a;
                }
            }
            loops.push(lp);
        }
        Some((loops, outline))
    }

    /// Adds a closed curve with its points (rectangles and ovals).
    pub fn add_closed_curve(&mut self, id: CurveId, points: &[(PointId, Point)], mode: HandleMode) {
        for &(pid, pos) in points {
            self.add_point(MeshPoint::new(pid, pos));
        }
        self.curves.push(Curve {
            id,
            closed: true,
            nodes: points.iter().map(|&(pid, _)| CurveNode::new(pid, mode)).collect(),
        });
    }

    /// Automatic fill and outline for a curve (Auto Fill / Auto Stroke options).
    ///
    /// If a shape already uses only that curve, it is updated to cover the whole curve;
    /// otherwise `new_shape` is created. Returns the affected shape.
    pub fn auto_shape(
        &mut self,
        curve: CurveId,
        fill: Option<Color>,
        stroke: Option<StrokeStyle>,
        new_shape: ShapeId,
        name: String,
    ) -> Option<ShapeId> {
        let c = self.curve(curve)?;
        let (closed, edges) = (c.closed, c.edges());
        let only_this_curve = |s: &Shape| {
            let mut all = s.fill.iter().flatten().chain(&s.outline).peekable();
            all.peek().is_some() && all.all(|e| e.curve == curve)
        };
        if let Some(shape) = self.shapes.iter_mut().find(|s| only_this_curve(s)) {
            if !shape.outline.is_empty() {
                shape.outline = edges.clone();
            }
            if closed && shape.fill.is_empty() && fill.is_some() {
                shape.fill = vec![edges];
                shape.style.fill = shape.style.fill.or(fill);
            }
            return Some(shape.id);
        }
        let fill = fill.filter(|_| closed);
        if fill.is_none() && stroke.is_none() {
            return None;
        }
        self.shapes.push(Shape {
            id: new_shape,
            name,
            fill: if closed { vec![edges.clone()] } else { Vec::new() },
            outline: if stroke.is_some() { edges } else { Vec::new() },
            style: Style { fill, stroke },
        });
        Some(new_shape)
    }
}

/// Splits a manual cubic segment at `t` without changing its shape; returns the new node.
fn split_cubic(mesh: &mut Mesh, ci: usize, i: usize, j: usize, cubic: CubicBez, t: f64, id: PointId) -> CurveNode {
    let first = cubic.subsegment(0.0..t);
    let second = cubic.subsegment(t..1.0);
    let nodes = &mut mesh.curves[ci].nodes;
    nodes[i].handle_out = first.p1 - first.p0;
    nodes[j].handle_in = second.p2 - second.p3;
    CurveNode {
        point: id,
        mode: HandleMode::Smooth,
        handle_in: first.p2 - first.p3,
        handle_out: second.p1 - second.p0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape as _;

    /// Square 0..10 with fill and outline.
    fn square() -> Mesh {
        let mut mesh = Mesh::default();
        let pts = [(1, (0.0, 0.0)), (2, (10.0, 0.0)), (3, (10.0, 10.0)), (4, (0.0, 10.0))].map(|(i, p)| (PointId(i), Point::from(p)));
        mesh.add_closed_curve(CurveId(10), &pts, HandleMode::PEAK);
        mesh.auto_shape(CurveId(10), Some(Color::WHITE), Some(Style::DEFAULT_STROKE), ShapeId(20), "square".into());
        mesh
    }

    fn fill_area(mesh: &Mesh) -> f64 {
        mesh.fill_path(&mesh.shapes[0]).map_or(0.0, |p| p.area().abs())
    }

    #[test]
    fn auto_shape_fills_closed_curve() {
        let mesh = square();
        assert_eq!(mesh.shapes.len(), 1);
        assert!((fill_area(&mesh) - 100.0).abs() < 1e-9);
        assert_eq!(mesh.shapes[0].outline.len(), 4);
    }

    #[test]
    fn invisible_shape_can_still_be_hit() {
        let mut mesh = square();
        mesh.shapes[0].style = Style { fill: None, stroke: None };
        assert_eq!(mesh.shape_at(Point::new(5.0, 5.0), 0.5), Some(ShapeId(20)));
        assert_eq!(mesh.shape_at(Point::new(10.2, 5.0), 0.5), Some(ShapeId(20)));
    }

    #[test]
    fn inserting_a_point_keeps_the_fill() {
        let mut mesh = square();
        assert!(mesh.insert_point(CurveId(10), 0, 0.5, PointId(5)));
        assert_eq!(mesh.curves[0].nodes.len(), 5);
        assert_eq!(mesh.pos(PointId(5)), Some(Point::new(5.0, 0.0)));
        assert_eq!(mesh.shapes[0].fill[0].len(), 5);
        assert_eq!(mesh.shapes[0].outline.len(), 5);
        assert!((fill_area(&mesh) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn deleting_an_edge_removes_the_fill() {
        let mut mesh = square();
        assert!(mesh.delete_edge(CurveId(10), 1, CurveId(11)));
        assert!(!mesh.curves[0].closed);
        assert_eq!(mesh.points().len(), 4);
        // The fill is lost; the outline keeps the other three sides.
        assert!(mesh.shapes[0].fill.is_empty());
        assert_eq!(mesh.shapes[0].outline.len(), 3);
    }

    #[test]
    fn deleting_a_point_joins_its_neighbors() {
        let mut mesh = square();
        mesh.delete_points(&HashSet::from([PointId(3)]));
        assert_eq!(mesh.curves[0].nodes.len(), 3);
        // A triangle remains: area 50.
        assert!((fill_area(&mesh) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn welding_the_ends_closes_the_curve() {
        let mut mesh = Mesh::default();
        for (i, p) in [(1, (0.0, 0.0)), (2, (10.0, 0.0)), (3, (5.0, 8.0)), (4, (0.1, 0.1))] {
            mesh.add_point(MeshPoint::new(PointId(i), Point::from(p)));
        }
        mesh.curves.push(Curve {
            id: CurveId(10),
            closed: false,
            nodes: (1..=4).map(|i| CurveNode::new(PointId(i), HandleMode::PEAK)).collect(),
        });
        assert!(mesh.weld(PointId(4), PointId(1)));
        assert!(mesh.curves[0].closed);
        assert_eq!(mesh.curves[0].nodes.len(), 3);
        assert!(mesh.point(PointId(4)).is_none());
    }

    #[test]
    fn welding_joins_two_curves_at_a_shared_point() {
        let mut mesh = square();
        mesh.add_point(MeshPoint::new(PointId(5), Point::new(20.0, 20.0)));
        mesh.add_point(MeshPoint::new(PointId(6), Point::new(10.2, 10.1)));
        mesh.curves.push(Curve {
            id: CurveId(11),
            closed: false,
            nodes: vec![CurveNode::new(PointId(5), HandleMode::PEAK), CurveNode::new(PointId(6), HandleMode::PEAK)],
        });
        assert!(mesh.weld(PointId(6), PointId(3)));
        // Point 3 is now shared by both curves; the square's fill is intact.
        assert_eq!(mesh.curves_at(PointId(3)).len(), 2);
        assert!((fill_area(&mesh) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn shape_from_selected_points() {
        let mut mesh = square();
        mesh.shapes.clear();
        let all: HashSet<PointId> = (1..=4).map(PointId).collect();
        let (loops, outline) = mesh.shape_from_points(&all).unwrap();
        assert_eq!(loops.len(), 1);
        assert_eq!(outline.len(), 4);
        // With three points there is no cycle: outline only.
        let three: HashSet<PointId> = (1..=3).map(PointId).collect();
        let (loops, outline) = mesh.shape_from_points(&three).unwrap();
        assert!(loops.is_empty());
        assert_eq!(outline.len(), 2);
    }
}
