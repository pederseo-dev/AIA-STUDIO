//! Trazado vectorial editable: nodos con manijas y tipo de nodo.
//!
//! Es el formato propio del documento. Se convierte a [`BezPath`] de kurbo solo
//! para renderizar y hacer hit-testing. Los nodos explícitos son la base para que,
//! más adelante, los huesos deformen nodos con pesos.

use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveNearest, PathSeg, Point, Rect, Vec2};
use serde::{Deserialize, Serialize};

/// Constante para aproximar un cuarto de elipse con una cúbica.
const KAPPA: f64 = 0.552_284_749_830_793_4;

/// Cómo se relacionan las dos manijas de un nodo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Manijas independientes (o sin manijas): el trazado puede hacer un pico.
    #[default]
    Corner,
    /// Manijas alineadas en direcciones opuestas, con largos libres.
    Smooth,
    /// Manijas alineadas y del mismo largo.
    Symmetric,
}

impl NodeKind {
    pub const ALL: [NodeKind; 3] = [NodeKind::Corner, NodeKind::Smooth, NodeKind::Symmetric];

    pub fn name(self) -> &'static str {
        match self {
            NodeKind::Corner => "Esquina",
            NodeKind::Smooth => "Suave",
            NodeKind::Symmetric => "Simétrico",
        }
    }
}

/// Cuál de las dos manijas de un nodo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    In,
    Out,
}

/// Un nodo del trazado. Las manijas son relativas al punto: `Vec2::ZERO` = sin manija.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub point: Point,
    pub handle_in: Vec2,
    pub handle_out: Vec2,
    pub kind: NodeKind,
}

impl Node {
    pub fn corner(point: Point) -> Self {
        Self {
            point,
            handle_in: Vec2::ZERO,
            handle_out: Vec2::ZERO,
            kind: NodeKind::Corner,
        }
    }

    /// Nodo simétrico: la manija de entrada es el opuesto de la de salida.
    pub fn symmetric(point: Point, handle_out: Vec2) -> Self {
        Self {
            point,
            handle_in: -handle_out,
            handle_out,
            kind: NodeKind::Symmetric,
        }
    }

    pub fn handle(&self, handle: Handle) -> Vec2 {
        match handle {
            Handle::In => self.handle_in,
            Handle::Out => self.handle_out,
        }
    }

    /// Mueve una manija respetando el tipo de nodo.
    pub fn set_handle(&mut self, handle: Handle, value: Vec2) {
        let (moved, other) = match handle {
            Handle::In => (&mut self.handle_in, &mut self.handle_out),
            Handle::Out => (&mut self.handle_out, &mut self.handle_in),
        };
        *moved = value;
        match self.kind {
            NodeKind::Corner => {}
            NodeKind::Symmetric => *other = -value,
            NodeKind::Smooth => {
                if value.hypot() > f64::EPSILON {
                    *other = -value.normalize() * other.hypot();
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VectorPath {
    pub nodes: Vec<Node>,
    pub closed: bool,
}

impl VectorPath {
    pub fn rect(rect: Rect) -> Self {
        Self {
            nodes: [
                (rect.x0, rect.y0),
                (rect.x1, rect.y0),
                (rect.x1, rect.y1),
                (rect.x0, rect.y1),
            ]
            .into_iter()
            .map(|p| Node::corner(p.into()))
            .collect(),
            closed: true,
        }
    }

    /// Elipse con 4 nodos simétricos (arriba, derecha, abajo, izquierda).
    pub fn ellipse(rect: Rect) -> Self {
        let c = rect.center();
        let (rx, ry) = (rect.width() / 2.0, rect.height() / 2.0);
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        Self {
            nodes: vec![
                Node::symmetric(Point::new(c.x, c.y - ry), Vec2::new(kx, 0.0)),
                Node::symmetric(Point::new(c.x + rx, c.y), Vec2::new(0.0, ky)),
                Node::symmetric(Point::new(c.x, c.y + ry), Vec2::new(-kx, 0.0)),
                Node::symmetric(Point::new(c.x - rx, c.y), Vec2::new(0.0, -ky)),
            ],
            closed: true,
        }
    }

    /// Cantidad de segmentos (incluye el de cierre si el trazado está cerrado).
    pub fn segment_count(&self) -> usize {
        match self.nodes.len() {
            0 | 1 => 0,
            n if self.closed => n,
            n => n - 1,
        }
    }

    /// Segmento `i`, que va del nodo `i` al siguiente.
    pub fn segment(&self, i: usize) -> PathSeg {
        let a = &self.nodes[i];
        let b = &self.nodes[(i + 1) % self.nodes.len()];
        if a.handle_out == Vec2::ZERO && b.handle_in == Vec2::ZERO {
            PathSeg::Line(Line::new(a.point, b.point))
        } else {
            PathSeg::Cubic(CubicBez::new(
                a.point,
                a.point + a.handle_out,
                b.point + b.handle_in,
                b.point,
            ))
        }
    }

    pub fn to_bezpath(&self) -> BezPath {
        let mut path = BezPath::new();
        let Some(first) = self.nodes.first() else {
            return path;
        };
        path.move_to(first.point);
        for i in 0..self.segment_count() {
            match self.segment(i) {
                PathSeg::Line(line) => path.line_to(line.p1),
                PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
                PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
            }
        }
        if self.closed && self.nodes.len() > 1 {
            path.close_path();
        }
        path
    }

    /// Segmento más cercano a `point` dentro de `tolerance`: (índice, parámetro t).
    pub fn nearest_segment(&self, point: Point, tolerance: f64) -> Option<(usize, f64)> {
        (0..self.segment_count())
            .map(|i| (i, self.segment(i).nearest(point, 1e-3)))
            .filter(|(_, n)| n.distance_sq <= tolerance * tolerance)
            .min_by(|a, b| a.1.distance_sq.total_cmp(&b.1.distance_sq))
            .map(|(i, n)| (i, n.t))
    }

    /// Divide el segmento `i` en `t` agregando un nodo, sin cambiar la forma.
    /// Devuelve el índice del nodo nuevo.
    pub fn insert_node(&mut self, segment: usize, t: f64) -> usize {
        let next = (segment + 1) % self.nodes.len();
        let index = segment + 1;
        let node = match self.segment(segment) {
            PathSeg::Cubic(cubic) => {
                let a = cubic.subsegment(0.0..t);
                let b = cubic.subsegment(t..1.0);
                self.nodes[segment].handle_out = a.p1 - a.p0;
                self.nodes[next].handle_in = b.p2 - b.p3;
                Node {
                    point: a.p3,
                    handle_in: a.p2 - a.p3,
                    handle_out: b.p1 - b.p0,
                    kind: NodeKind::Smooth,
                }
            }
            seg => Node::corner(seg.eval(t)),
        };
        self.nodes.insert(index, node);
        index
    }

    /// Cambia el tipo de nodo ajustando sus manijas para que respeten el tipo.
    pub fn set_kind(&mut self, index: usize, kind: NodeKind) {
        let len = self.nodes.len();
        let prev = self.neighbor(index, len - 1);
        let next = self.neighbor(index, 1);
        let node = &mut self.nodes[index];
        node.kind = kind;
        if kind == NodeKind::Corner {
            return;
        }
        // Dirección de la tangente: la manija existente más larga o, si no hay,
        // la dirección entre los nodos vecinos.
        let (dir, len_in, len_out) = {
            let (li, lo) = (node.handle_in.hypot(), node.handle_out.hypot());
            let dir = if lo >= li && lo > f64::EPSILON {
                node.handle_out.normalize()
            } else if li > f64::EPSILON {
                -node.handle_in.normalize()
            } else {
                match (prev, next) {
                    (Some(p), Some(n)) if (n - p).hypot() > f64::EPSILON => (n - p).normalize(),
                    _ => Vec2::new(1.0, 0.0),
                }
            };
            let default_len = match (prev, next) {
                (Some(p), Some(n)) => (n - p).hypot() / 6.0,
                _ => 20.0,
            };
            let li = if li > f64::EPSILON { li } else { default_len };
            let lo = if lo > f64::EPSILON { lo } else { default_len };
            (dir, li, lo)
        };
        match kind {
            NodeKind::Smooth => {
                node.handle_out = dir * len_out;
                node.handle_in = -dir * len_in;
            }
            NodeKind::Symmetric => {
                let len = (len_in + len_out) / 2.0;
                node.handle_out = dir * len;
                node.handle_in = -dir * len;
            }
            NodeKind::Corner => unreachable!(),
        }
    }

    /// Posición del nodo vecino (`offset` = 1 siguiente, `len - 1` anterior), si existe.
    fn neighbor(&self, index: usize, offset: usize) -> Option<Point> {
        let len = self.nodes.len();
        if len < 2 {
            return None;
        }
        let j = (index + offset) % len;
        let wraps = (offset == 1 && index == len - 1) || (offset == len - 1 && index == 0);
        (self.closed || !wraps).then(|| self.nodes[j].point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape as _;

    #[test]
    fn rect_is_closed_with_four_lines() {
        let path = VectorPath::rect(Rect::new(0.0, 0.0, 10.0, 20.0));
        assert_eq!(path.segment_count(), 4);
        assert!((path.to_bezpath().area().abs() - 200.0).abs() < 1e-9);
    }

    #[test]
    fn ellipse_area_is_close_to_exact() {
        let path = VectorPath::ellipse(Rect::new(0.0, 0.0, 20.0, 10.0));
        let exact = std::f64::consts::PI * 10.0 * 5.0;
        assert!((path.to_bezpath().area().abs() - exact).abs() / exact < 0.001);
    }

    #[test]
    fn insert_node_keeps_the_curve() {
        let mut path = VectorPath::ellipse(Rect::new(0.0, 0.0, 20.0, 10.0));
        let before = path.segment(0).eval(0.3);
        let index = path.insert_node(0, 0.3);
        assert_eq!(index, 1);
        assert_eq!(path.nodes.len(), 5);
        assert!((path.nodes[1].point - before).hypot() < 1e-9);
        let area_before = VectorPath::ellipse(Rect::new(0.0, 0.0, 20.0, 10.0)).to_bezpath().area();
        assert!((path.to_bezpath().area() - area_before).abs() < 1e-6);
    }

    #[test]
    fn symmetric_handles_mirror() {
        let mut node = Node::symmetric(Point::ZERO, Vec2::new(5.0, 0.0));
        node.set_handle(Handle::In, Vec2::new(0.0, 3.0));
        assert_eq!(node.handle_out, Vec2::new(0.0, -3.0));
    }

    #[test]
    fn smooth_keeps_other_length() {
        let mut node = Node {
            kind: NodeKind::Smooth,
            ..Node::symmetric(Point::ZERO, Vec2::new(4.0, 0.0))
        };
        node.set_handle(Handle::Out, Vec2::new(0.0, 10.0));
        assert!((node.handle_in - Vec2::new(0.0, -4.0)).hypot() < 1e-9);
    }

    #[test]
    fn corner_to_symmetric_creates_handles() {
        let mut path = VectorPath::rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        path.set_kind(1, NodeKind::Symmetric);
        let node = path.nodes[1];
        assert!(node.handle_out.hypot() > 0.0);
        assert_eq!(node.handle_in, -node.handle_out);
    }

    #[test]
    fn open_path_has_one_less_segment() {
        let path = VectorPath {
            nodes: vec![Node::corner(Point::ZERO), Node::corner(Point::new(10.0, 0.0))],
            closed: false,
        };
        assert_eq!(path.segment_count(), 1);
        assert_eq!(path.nearest_segment(Point::new(5.0, 1.0), 2.0).map(|(i, _)| i), Some(0));
    }
}
