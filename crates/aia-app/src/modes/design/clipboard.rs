//! Copiar y pegar formas usando el portapapeles del sistema como texto JSON.
//!
//! egui solo entrega el evento de pegar si el portapapeles tiene texto, así que las
//! formas viajan como JSON. Esto además permite pegar entre ventanas de la app.

use aia_core::Shape;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ClipboardData {
    aia_studio_shapes: Vec<Shape>,
}

pub fn to_text(shapes: Vec<Shape>) -> String {
    serde_json::to_string(&ClipboardData {
        aia_studio_shapes: shapes,
    })
    .expect("las formas siempre se pueden serializar")
}

/// `None` si el texto no son formas de AIA Studio.
pub fn from_text(text: &str) -> Option<Vec<Shape>> {
    serde_json::from_str::<ClipboardData>(text)
        .ok()
        .map(|data| data.aia_studio_shapes)
        .filter(|shapes| !shapes.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aia_core::kurbo::{Affine, Rect};
    use aia_core::{ShapeId, Style, VectorPath};

    #[test]
    fn roundtrip_and_rejects_other_text() {
        let shape = Shape {
            id: ShapeId(7),
            name: "cabeza".into(),
            path: VectorPath::ellipse(Rect::new(0.0, 0.0, 10.0, 10.0)),
            transform: Affine::IDENTITY,
            style: Style::default(),
        };
        let text = to_text(vec![shape.clone()]);
        assert_eq!(from_text(&text), Some(vec![shape]));
        assert_eq!(from_text("hola"), None);
    }
}
