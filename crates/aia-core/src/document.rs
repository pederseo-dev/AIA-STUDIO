//! The document: a tree of layers, as in Moho. Each layer has a transform around its
//! origin; vector layers hold a mesh of points, curves and shapes.

use kurbo::{Affine, Point, Vec2};
use serde::{Deserialize, Serialize};

use crate::mesh::{CurveId, Mesh, PointId, ShapeId};
use crate::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LayerId(pub u64);

/// A layer's transform relative to its parent.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerTransform {
    pub translation: Vec2,
    pub scale: Vec2,
    /// In radians.
    pub rotation: f64,
    pub flip_h: bool,
    pub flip_v: bool,
}

impl Default for LayerTransform {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
            scale: Vec2::new(1.0, 1.0),
            rotation: 0.0,
            flip_h: false,
            flip_v: false,
        }
    }
}

impl LayerTransform {
    /// From layer space to parent space: scales and rotates around `origin`, then translates.
    pub fn affine(&self, origin: Point) -> Affine {
        let o = origin.to_vec2();
        Affine::translate(self.translation + o) * self.linear() * Affine::translate(-o)
    }

    /// Rotation, scale and flip (no translation).
    pub fn linear(&self) -> Affine {
        let sx = if self.flip_h { -self.scale.x } else { self.scale.x };
        let sy = if self.flip_v { -self.scale.y } else { self.scale.y };
        Affine::rotate(self.rotation) * Affine::scale_non_uniform(sx, sy)
    }

    /// Changes the origin without moving the layer on screen (Set Origin tool).
    pub fn with_origin_moved(self, old: Point, new: Point) -> Self {
        let lin = self.linear();
        let drift = |o: Point| o.to_vec2() - (lin * o).to_vec2();
        Self {
            translation: self.translation + drift(old) - drift(new),
            ..self
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LayerKind {
    Vector { mesh: Mesh },
    /// Groups layers: whatever is done to the group applies to all of them.
    Group { children: Vec<Layer> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f64,
    /// Pivot for rotation and scale, in layer coordinates.
    pub origin: Point,
    pub transform: LayerTransform,
    pub kind: LayerKind,
}

impl Layer {
    pub fn vector(id: LayerId, name: impl Into<String>) -> Self {
        Self::new(id, name, LayerKind::Vector { mesh: Mesh::default() })
    }

    pub fn group(id: LayerId, name: impl Into<String>) -> Self {
        Self::new(id, name, LayerKind::Group { children: Vec::new() })
    }

    fn new(id: LayerId, name: impl Into<String>, kind: LayerKind) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            origin: Point::ZERO,
            transform: LayerTransform::default(),
            kind,
        }
    }

    /// From layer space to parent space.
    pub fn affine(&self) -> Affine {
        self.transform.affine(self.origin)
    }

    pub fn mesh(&self) -> Option<&Mesh> {
        match &self.kind {
            LayerKind::Vector { mesh } => Some(mesh),
            LayerKind::Group { .. } => None,
        }
    }

    pub fn mesh_mut(&mut self) -> Option<&mut Mesh> {
        match &mut self.kind {
            LayerKind::Vector { mesh } => Some(mesh),
            LayerKind::Group { .. } => None,
        }
    }

    pub fn children(&self) -> Option<&[Layer]> {
        match &self.kind {
            LayerKind::Group { children } => Some(children),
            LayerKind::Vector { .. } => None,
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<Layer>> {
        match &mut self.kind {
            LayerKind::Group { children } => Some(children),
            LayerKind::Vector { .. } => None,
        }
    }

    pub fn is_group(&self) -> bool {
        matches!(self.kind, LayerKind::Group { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Stage size (what gets exported), in document units.
    pub width: f64,
    pub height: f64,
    pub background: Color,
    /// Top-level layers, bottom to top.
    pub layers: Vec<Layer>,
    next_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        let mut doc = Self {
            width: 1920.0,
            height: 1080.0,
            background: Color::WHITE,
            layers: Vec::new(),
            next_id: 1,
        };
        let id = doc.new_layer_id();
        doc.layers.push(Layer::vector(id, "Layer 1"));
        doc
    }
}

impl Document {
    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn new_layer_id(&mut self) -> LayerId {
        LayerId(self.alloc_id())
    }

    pub fn new_point_id(&mut self) -> PointId {
        PointId(self.alloc_id())
    }

    pub fn new_curve_id(&mut self) -> CurveId {
        CurveId(self.alloc_id())
    }

    pub fn new_shape_id(&mut self) -> ShapeId {
        ShapeId(self.alloc_id())
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        find(&self.layers, id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        find_mut(&mut self.layers, id)
    }

    pub fn mesh(&self, id: LayerId) -> Option<&Mesh> {
        self.layer(id).and_then(Layer::mesh)
    }

    pub fn mesh_mut(&mut self, id: LayerId) -> Option<&mut Mesh> {
        self.layer_mut(id).and_then(Layer::mesh_mut)
    }

    /// Sibling layers inside `parent` (`None` = top level). `None` if the parent is not a group.
    pub fn siblings(&self, parent: Option<LayerId>) -> Option<&[Layer]> {
        match parent {
            None => Some(&self.layers),
            Some(id) => self.layer(id)?.children(),
        }
    }

    pub fn siblings_mut(&mut self, parent: Option<LayerId>) -> Option<&mut Vec<Layer>> {
        match parent {
            None => Some(&mut self.layers),
            Some(id) => self.layer_mut(id)?.children_mut(),
        }
    }

    /// Parent and index among its siblings.
    pub fn locate_layer(&self, id: LayerId) -> Option<(Option<LayerId>, usize)> {
        let path = self.layer_path(id)?;
        let parent = (path.len() >= 2).then(|| path[path.len() - 2].id);
        let index = self.siblings(parent)?.iter().position(|l| l.id == id)?;
        Some((parent, index))
    }

    /// The layers from the top level down to `id` (inclusive).
    pub fn layer_path(&self, id: LayerId) -> Option<Vec<&Layer>> {
        let mut path = Vec::new();
        path_to(&self.layers, id, &mut path).then_some(path)
    }

    /// From layer space to document space.
    pub fn world_transform(&self, id: LayerId) -> Option<Affine> {
        Some(self.layer_path(id)?.iter().fold(Affine::IDENTITY, |acc, l| acc * l.affine()))
    }

    /// From the layer's parent space to document space.
    pub fn parent_transform(&self, id: LayerId) -> Option<Affine> {
        let path = self.layer_path(id)?;
        Some(path[..path.len() - 1].iter().fold(Affine::IDENTITY, |acc, l| acc * l.affine()))
    }

    /// Visible and unlocked, both the layer and all its parents.
    pub fn is_editable(&self, id: LayerId) -> bool {
        self.layer_path(id).is_some_and(|path| path.iter().all(|l| l.visible && !l.locked))
    }

    /// `true` if `id` is inside `ancestor` (at any depth) or is the same layer.
    pub fn is_inside(&self, id: LayerId, ancestor: LayerId) -> bool {
        self.layer_path(id).is_some_and(|path| path.iter().any(|l| l.id == ancestor))
    }

    /// All layers in drawing order (bottom to top; children before whatever is above the group).
    pub fn all_layers(&self) -> Vec<&Layer> {
        fn walk<'a>(layers: &'a [Layer], out: &mut Vec<&'a Layer>) {
            for layer in layers {
                out.push(layer);
                if let Some(children) = layer.children() {
                    walk(children, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.layers, &mut out);
        out
    }

    pub(crate) fn sort_points(&mut self) {
        fn walk(layers: &mut [Layer]) {
            for layer in layers {
                match &mut layer.kind {
                    LayerKind::Vector { mesh } => mesh.sort_points(),
                    LayerKind::Group { children } => walk(children),
                }
            }
        }
        walk(&mut self.layers);
    }
}

fn find(layers: &[Layer], id: LayerId) -> Option<&Layer> {
    for layer in layers {
        if layer.id == id {
            return Some(layer);
        }
        if let Some(found) = layer.children().and_then(|c| find(c, id)) {
            return Some(found);
        }
    }
    None
}

fn find_mut(layers: &mut [Layer], id: LayerId) -> Option<&mut Layer> {
    for layer in layers {
        if layer.id == id {
            return Some(layer);
        }
        if let Some(found) = layer.children_mut().and_then(|c| find_mut(c, id)) {
            return Some(found);
        }
    }
    None
}

fn path_to<'a>(layers: &'a [Layer], id: LayerId, path: &mut Vec<&'a Layer>) -> bool {
    for layer in layers {
        path.push(layer);
        if layer.id == id || layer.children().is_some_and(|c| path_to(c, id, path)) {
            return true;
        }
        path.pop();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        (a - b).hypot() < 1e-9
    }

    #[test]
    fn transform_rotates_around_origin() {
        let t = LayerTransform {
            rotation: std::f64::consts::FRAC_PI_2,
            ..Default::default()
        };
        let origin = Point::new(10.0, 0.0);
        let a = t.affine(origin);
        assert!(close(a * origin, origin));
        assert!(close(a * Point::new(20.0, 0.0), Point::new(10.0, 10.0)));
    }

    #[test]
    fn moving_the_origin_keeps_the_layer_in_place() {
        let t = LayerTransform {
            rotation: 0.7,
            scale: Vec2::new(2.0, 0.5),
            translation: Vec2::new(3.0, 4.0),
            ..Default::default()
        };
        let (old, new) = (Point::new(1.0, 2.0), Point::new(-5.0, 8.0));
        let moved = t.with_origin_moved(old, new);
        let p = Point::new(7.0, -3.0);
        assert!(close(t.affine(old) * p, moved.affine(new) * p));
    }

    #[test]
    fn world_transform_composes_groups() {
        let mut doc = Document::default();
        let group_id = doc.new_layer_id();
        let child_id = doc.new_layer_id();
        let mut group = Layer::group(group_id, "group");
        group.transform.translation = Vec2::new(100.0, 0.0);
        let mut child = Layer::vector(child_id, "child");
        child.transform.translation = Vec2::new(0.0, 50.0);
        group.children_mut().unwrap().push(child);
        doc.layers.push(group);

        assert_eq!(doc.locate_layer(child_id), Some((Some(group_id), 0)));
        assert!(close(doc.world_transform(child_id).unwrap() * Point::ZERO, Point::new(100.0, 50.0)));
        assert!(doc.is_inside(child_id, group_id));
        doc.layer_mut(group_id).unwrap().locked = true;
        assert!(!doc.is_editable(child_id));
    }
}
