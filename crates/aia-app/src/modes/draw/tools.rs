use aia_core::kurbo::{BezPath, Ellipse, Point, Rect, Shape as _};
use eframe::egui::Key;
use egui_phosphor::regular as icons;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rectangle,
    Ellipse,
}

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::Select, Tool::Rectangle, Tool::Ellipse];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Selección",
            Tool::Rectangle => "Rectángulo",
            Tool::Ellipse => "Elipse",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tool::Select => icons::CURSOR,
            Tool::Rectangle => icons::RECTANGLE,
            Tool::Ellipse => icons::CIRCLE,
        }
    }

    pub fn shortcut(self) -> Key {
        match self {
            Tool::Select => Key::V,
            Tool::Rectangle => Key::R,
            Tool::Ellipse => Key::E,
        }
    }

    /// Texto del atajo para tooltips, con la ayuda de los modificadores.
    pub fn shortcut_hint(self) -> &'static str {
        match self {
            Tool::Select => "V · Shift+clic suma a la selección",
            Tool::Rectangle => "R · Shift = cuadrado",
            Tool::Ellipse => "E · Shift = círculo",
        }
    }

    /// Las herramientas que crean formas necesitan una capa editable.
    pub fn creates_shapes(self) -> bool {
        !matches!(self, Tool::Select)
    }
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

pub fn box_path(tool: Tool, rect: Rect) -> BezPath {
    match tool {
        Tool::Ellipse => Ellipse::from_rect(rect).to_path(0.1),
        _ => rect.to_path(0.1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_drag_keeps_direction() {
        let rect = drag_rect(Point::new(10.0, 10.0), Point::new(4.0, 30.0), true);
        assert_eq!(rect, Rect::new(-10.0, 10.0, 10.0, 30.0));
    }
}
