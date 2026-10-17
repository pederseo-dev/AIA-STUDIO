use kurbo::{Affine, BezPath, Point, Rect, Shape as _};
use serde::{Deserialize, Serialize};

/// Versión del formato de archivo `.aia`.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ShapeId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LayerId(pub u64);

/// Color sRGB de 8 bits por canal, sin premultiplicar.
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub fill: Option<Color>,
    pub stroke: Option<StrokeStyle>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fill: Some(Color::rgb(230, 120, 60)),
            stroke: Some(StrokeStyle {
                color: Color::rgb(30, 30, 30),
                width: 2.0,
            }),
        }
    }
}

/// Una forma vectorial: un path Bézier en coordenadas locales más su transformación.
///
/// La transformación queda separada del path para que, más adelante, los huesos
/// puedan deformar los puntos de control sin perder la geometría original.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub id: ShapeId,
    pub path: BezPath,
    pub transform: Affine,
    pub style: Style,
}

impl Shape {
    /// Caja envolvente en coordenadas del documento (incluye el grosor del trazo).
    pub fn bounds(&self) -> Rect {
        let rect = (self.transform * self.path.clone()).bounding_box();
        match self.style.stroke {
            Some(stroke) => rect.inflate(stroke.width / 2.0, stroke.width / 2.0),
            None => rect,
        }
    }

    /// `true` si el punto (en coordenadas del documento) toca la forma.
    /// `tolerance` amplía el área de impacto del trazo.
    pub fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        if !self.bounds().inflate(tolerance, tolerance).contains(point) {
            return false;
        }
        let local = self.transform.inverse() * point;
        if self.style.fill.is_some() && self.path.contains(local) {
            return true;
        }
        let half_width = self.style.stroke.map_or(0.0, |s| s.width / 2.0);
        let reach = half_width + tolerance;
        self.path.segments().any(|seg| {
            use kurbo::ParamCurveNearest;
            seg.nearest(local, 0.1).distance_sq <= reach * reach
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    /// Ordenadas de abajo hacia arriba.
    pub shapes: Vec<Shape>,
}

impl Layer {
    pub fn new(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            locked: false,
            shapes: Vec::new(),
        }
    }

    pub fn is_editable(&self) -> bool {
        self.visible && !self.locked
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub format_version: u32,
    /// Tamaño del escenario (lo que se exporta), en unidades del documento.
    pub width: f64,
    pub height: f64,
    pub background: Color,
    /// Ordenadas de abajo hacia arriba.
    pub layers: Vec<Layer>,
    next_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        let mut doc = Self {
            format_version: FORMAT_VERSION,
            width: 1920.0,
            height: 1080.0,
            background: Color::WHITE,
            layers: Vec::new(),
            next_id: 1,
        };
        let id = doc.new_layer_id();
        doc.layers.push(Layer::new(id, "Capa 1"));
        doc
    }
}

impl Document {
    pub fn new_shape_id(&mut self) -> ShapeId {
        ShapeId(self.alloc_id())
    }

    pub fn new_layer_id(&mut self) -> LayerId {
        LayerId(self.alloc_id())
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn shape(&self, id: ShapeId) -> Option<&Shape> {
        self.layers
            .iter()
            .flat_map(|l| &l.shapes)
            .find(|s| s.id == id)
    }

    pub fn shape_mut(&mut self, id: ShapeId) -> Option<&mut Shape> {
        self.layers
            .iter_mut()
            .flat_map(|l| &mut l.shapes)
            .find(|s| s.id == id)
    }

    /// Capa e índice dentro de la capa donde vive una forma.
    pub fn locate_shape(&self, id: ShapeId) -> Option<(LayerId, usize)> {
        self.layers.iter().find_map(|l| {
            l.shapes
                .iter()
                .position(|s| s.id == id)
                .map(|i| (l.id, i))
        })
    }

    /// La forma editable más arriba que toca el punto.
    pub fn hit_test(&self, point: Point, tolerance: f64) -> Option<ShapeId> {
        self.layers
            .iter()
            .rev()
            .filter(|l| l.is_editable())
            .flat_map(|l| l.shapes.iter().rev())
            .find(|s| s.hit_test(point, tolerance))
            .map(|s| s.id)
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}
