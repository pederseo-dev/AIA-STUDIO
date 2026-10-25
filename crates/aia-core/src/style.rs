use serde::{Deserialize, Serialize};

/// 8-bit sRGB color, not premultiplied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn from_array([r, g, b, a]: [u8; 4]) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrokeStyle {
    pub color: Color,
    pub width: f64,
}

/// How a shape looks: fill and outline.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub fill: Option<Color>,
    pub stroke: Option<StrokeStyle>,
}

impl Style {
    pub const DEFAULT_FILL: Color = Color::rgb(230, 120, 60);
    pub const DEFAULT_STROKE: StrokeStyle = StrokeStyle {
        color: Color::rgb(30, 30, 30),
        width: 2.0,
    };
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fill: Some(Self::DEFAULT_FILL),
            stroke: Some(Self::DEFAULT_STROKE),
        }
    }
}
