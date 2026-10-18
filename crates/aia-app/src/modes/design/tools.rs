use aia_core::kurbo::{Affine, Point, Rect, Vec2};
use aia_core::{Handle, ShapeId, VectorPath};
use aia_render::BoxHandle;
use eframe::egui::Key;
use egui_phosphor::regular as icons;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    EditNodes,
    Pen,
    Rectangle,
    Ellipse,
}

impl Tool {
    pub const ALL: [Tool; 5] = [Tool::Select, Tool::EditNodes, Tool::Pen, Tool::Rectangle, Tool::Ellipse];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Selección",
            Tool::EditNodes => "Editar nodos",
            Tool::Pen => "Pluma",
            Tool::Rectangle => "Rectángulo",
            Tool::Ellipse => "Elipse",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tool::Select => icons::CURSOR,
            Tool::EditNodes => icons::BEZIER_CURVE,
            Tool::Pen => icons::PEN,
            Tool::Rectangle => icons::RECTANGLE,
            Tool::Ellipse => icons::CIRCLE,
        }
    }

    pub fn shortcut(self) -> Key {
        match self {
            Tool::Select => Key::V,
            Tool::EditNodes => Key::A,
            Tool::Pen => Key::P,
            Tool::Rectangle => Key::R,
            Tool::Ellipse => Key::E,
        }
    }

    /// Texto del atajo para tooltips, con la ayuda de los modificadores.
    pub fn shortcut_hint(self) -> &'static str {
        match self {
            Tool::Select => "V · Shift+clic suma · arrastrar en vacío = marco · manijas: escalar/rotar",
            Tool::EditNodes => "A · Shift+clic suma nodos · marco en vacío · clic en el trazado agrega nodo",
            Tool::Pen => "P · clic = esquina · arrastrar = curva · Enter termina",
            Tool::Rectangle => "R · Shift = cuadrado · Alt = desde el centro",
            Tool::Ellipse => "E · Shift = círculo · Alt = desde el centro",
        }
    }

    /// Las herramientas que crean formas necesitan una capa editable.
    pub fn creates_shapes(self) -> bool {
        matches!(self, Tool::Pen | Tool::Rectangle | Tool::Ellipse)
    }

    /// Nombre base para las formas que crea la herramienta.
    pub fn shape_name(self) -> &'static str {
        match self {
            Tool::Rectangle => "Rectángulo",
            Tool::Ellipse => "Elipse",
            _ => "Trazado",
        }
    }
}

/// Parte de un trazado que se está arrastrando con Editar nodos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeTarget {
    Node(usize),
    Handle(usize, Handle),
}

/// Lo que está haciendo el usuario con el mouse dentro del canvas.
/// Los puntos están en coordenadas del documento salvo que se indique otra cosa.
#[derive(Default)]
pub enum Gesture {
    #[default]
    None,
    Move {
        start: Point,
        current: Point,
    },
    /// Rectángulo o elipse en creación.
    Box {
        start: Point,
        current: Point,
    },
    /// Marco de selección de formas (Selección) o de nodos (Editar nodos).
    Marquee {
        start: Point,
        current: Point,
    },
    /// Escalar o rotar la selección con una manija de la caja de transformación.
    Transform {
        handle: BoxHandle,
        bounds: Rect,
        start: Point,
        current: Point,
    },
    /// Arrastre de nodos o de una manija. Puntos en coordenadas locales de la forma.
    EditPath {
        shape: ShapeId,
        target: NodeTarget,
        original: VectorPath,
        path: VectorPath,
        start: Point,
    },
}

/// Trazado en construcción con la pluma, en coordenadas del documento.
pub struct PenPath {
    pub path: VectorPath,
    /// Se está arrastrando para dar curva al último nodo (o al primero, al cerrar).
    pub dragging: Option<usize>,
}

/// Rectángulo entre dos esquinas. `square` fuerza la misma medida en ambos ejes;
/// `from_center` usa `start` como centro en vez de esquina.
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

pub fn box_path(tool: Tool, rect: Rect) -> VectorPath {
    match tool {
        Tool::Ellipse => VectorPath::ellipse(rect),
        _ => VectorPath::rect(rect),
    }
}

/// Restringe `p` a múltiplos de 45° alrededor de `origin`.
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

/// Escala `bounds` alrededor de `anchor` con factores (sx, sy).
fn scale_about(anchor: Point, sx: f64, sy: f64) -> Affine {
    Affine::translate(anchor.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-anchor.to_vec2())
}

/// Factor de escala para que `from` (medido desde `anchor`) pase a `to`. Si la caja
/// no tiene tamaño en ese eje, no se escala.
fn factor(anchor: f64, from: f64, to: f64) -> f64 {
    let size = from - anchor;
    if size.abs() < 1e-9 { 1.0 } else { (to - anchor) / size }
}

/// Transformación de la selección al arrastrar una manija de la caja.
/// `proportional` (Shift) mantiene la proporción o rota de a 15°;
/// `from_center` (Alt) escala desde el centro.
pub fn box_transform(
    handle: BoxHandle,
    bounds: Rect,
    start: Point,
    current: Point,
    proportional: bool,
    from_center: bool,
) -> Affine {
    let center = bounds.center();
    let delta = current - start;
    match handle {
        BoxHandle::Rotate => {
            let mut angle = (current - center).atan2() - (start - center).atan2();
            if proportional {
                let step = 15f64.to_radians();
                angle = (angle / step).round() * step;
            }
            Affine::translate(center.to_vec2()) * Affine::rotate(angle) * Affine::translate(-center.to_vec2())
        }
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

/// Transformación que lleva la caja `from` a la caja `to` (escala + traslación).
pub fn rect_to_rect(from: Rect, to: Rect) -> Affine {
    let sx = if from.width().abs() < 1e-9 { 1.0 } else { to.width() / from.width() };
    let sy = if from.height().abs() < 1e-9 { 1.0 } else { to.height() / from.height() };
    Affine::translate(to.origin().to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-from.origin().to_vec2())
}

/// Rotación (en grados) de una transformación sin sesgo.
pub fn rotation_degrees(transform: Affine) -> f64 {
    let [a, b, ..] = transform.as_coeffs();
    b.atan2(a).to_degrees()
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
        let p = snap_45(Point::ZERO, Point::new(10.0, 1.0));
        assert!((p.y).abs() < 1e-9 && (p.x - 101f64.sqrt()).abs() < 1e-9);
        let p = snap_45(Point::ZERO, Point::new(10.0, 9.0));
        assert!((p.x - p.y).abs() < 1e-9);
    }

    #[test]
    fn corner_scale_keeps_opposite_corner() {
        let bounds = Rect::new(0.0, 0.0, 10.0, 10.0);
        let t = box_transform(BoxHandle::Corner(2), bounds, Point::new(10.0, 10.0), Point::new(20.0, 15.0), false, false);
        assert!(close(t * Point::ZERO, Point::ZERO));
        assert!(close(t * Point::new(10.0, 10.0), Point::new(20.0, 15.0)));
    }

    #[test]
    fn proportional_corner_scale_is_uniform() {
        let bounds = Rect::new(0.0, 0.0, 10.0, 10.0);
        let t = box_transform(BoxHandle::Corner(2), bounds, Point::new(10.0, 10.0), Point::new(30.0, 12.0), true, false);
        assert!(close(t * Point::new(10.0, 10.0), Point::new(30.0, 30.0)));
    }

    #[test]
    fn edge_scale_only_one_axis() {
        let bounds = Rect::new(0.0, 0.0, 10.0, 10.0);
        let t = box_transform(BoxHandle::Edge(1), bounds, Point::new(10.0, 5.0), Point::new(20.0, 50.0), false, false);
        assert!(close(t * Point::new(10.0, 10.0), Point::new(20.0, 10.0)));
    }

    #[test]
    fn rotation_snaps_to_15_degrees() {
        let bounds = Rect::new(-1.0, -1.0, 1.0, 1.0);
        let t = box_transform(BoxHandle::Rotate, bounds, Point::new(0.0, -10.0), Point::new(4.0, -10.0), true, false);
        let angle = rotation_degrees(t);
        assert!((angle / 15.0 - (angle / 15.0).round()).abs() < 1e-9);
    }

    #[test]
    fn rect_to_rect_maps_corners() {
        let t = rect_to_rect(Rect::new(0.0, 0.0, 10.0, 20.0), Rect::new(5.0, 5.0, 25.0, 15.0));
        assert!(close(t * Point::ZERO, Point::new(5.0, 5.0)));
        assert!(close(t * Point::new(10.0, 20.0), Point::new(25.0, 15.0)));
    }
}
