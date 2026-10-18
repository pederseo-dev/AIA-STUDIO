use aia_core::kurbo::{Point, Rect, Vec2};
use aia_core::{Handle, ShapeId, VectorPath};
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
            Tool::Select => "V · Shift+clic suma a la selección",
            Tool::EditNodes => "A · clic en el trazado agrega un nodo · Alt+arrastrar manija = esquina",
            Tool::Pen => "P · clic = esquina · arrastrar = curva · Enter termina",
            Tool::Rectangle => "R · Shift = cuadrado",
            Tool::Ellipse => "E · Shift = círculo",
        }
    }

    /// Las herramientas que crean formas necesitan una capa editable.
    pub fn creates_shapes(self) -> bool {
        matches!(self, Tool::Pen | Tool::Rectangle | Tool::Ellipse)
    }
}

/// Parte de un trazado que se está arrastrando con Editar nodos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeTarget {
    Node(usize),
    Handle(usize, Handle),
}

/// Lo que está haciendo el usuario con el mouse dentro del canvas.
/// Los puntos están en coordenadas del documento.
#[derive(Default)]
pub enum Gesture {
    #[default]
    None,
    Move {
        start: Point,
        current: Point,
    },
    Box {
        start: Point,
        current: Point,
    },
    /// Arrastre de un nodo o manija. `path` es la copia de trabajo, en coordenadas locales.
    EditPath {
        shape: ShapeId,
        target: NodeTarget,
        path: VectorPath,
    },
}

/// Trazado en construcción con la pluma, en coordenadas del documento.
pub struct PenPath {
    pub path: VectorPath,
    /// Se está arrastrando para dar curva al último nodo (o al primero, al cerrar).
    pub dragging: Option<usize>,
}

/// Rectángulo entre dos esquinas; con `square` se fuerza la misma medida en ambos ejes.
pub fn drag_rect(start: Point, current: Point, square: bool) -> Rect {
    let mut delta = current - start;
    if square {
        let side = delta.x.abs().max(delta.y.abs());
        delta.x = side.copysign(delta.x);
        delta.y = side.copysign(delta.y);
    }
    Rect::from_points(start, start + delta)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_drag_keeps_direction() {
        let rect = drag_rect(Point::new(10.0, 10.0), Point::new(4.0, 30.0), true);
        assert_eq!(rect, Rect::new(-10.0, 10.0, 10.0, 30.0));
    }

    #[test]
    fn snap_goes_to_nearest_45() {
        let p = snap_45(Point::ZERO, Point::new(10.0, 1.0));
        assert!((p.y).abs() < 1e-9 && (p.x - 101f64.sqrt()).abs() < 1e-9);
        let p = snap_45(Point::ZERO, Point::new(10.0, 9.0));
        assert!((p.x - p.y).abs() < 1e-9);
    }
}
