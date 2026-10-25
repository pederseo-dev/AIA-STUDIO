use std::fmt;

use kurbo::Point;
use serde::{Deserialize, Serialize};

use crate::document::{Document, Layer, LayerId, LayerTransform};
use crate::mesh::{Curve, CurveId, Mesh, PointId, Shape, ShapeId};
use crate::style::Style;

/// An atomic operation on the document.
///
/// Commands serialize to JSON: they are used by the UI tools and, through each agent's
/// high-level tools, by the AI.
///
/// Applying a command returns its inverse, which is what the history keeps for undo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    // ---- Layers ----
    /// Inserts a layer inside `parent` (`None` = top level). `index: None` = on top.
    InsertLayer {
        parent: Option<LayerId>,
        index: Option<usize>,
        layer: Layer,
    },
    RemoveLayer {
        id: LayerId,
    },
    /// Moves a layer elsewhere in the tree. `index` is the final position among its new siblings.
    MoveLayer {
        id: LayerId,
        parent: Option<LayerId>,
        index: usize,
    },
    RenameLayer {
        id: LayerId,
        name: String,
    },
    SetLayerVisible {
        id: LayerId,
        visible: bool,
    },
    SetLayerLocked {
        id: LayerId,
        locked: bool,
    },
    SetLayerOpacity {
        id: LayerId,
        opacity: f64,
    },
    SetLayerTransform {
        id: LayerId,
        transform: LayerTransform,
    },
    SetLayerOrigin {
        id: LayerId,
        origin: Point,
    },

    // ---- Mesh of a vector layer ----
    /// Replaces the whole mesh. Used by operations that change the structure (adding or
    /// deleting points, welding, deleting edges); the inverse is the previous mesh.
    ReplaceMesh {
        layer: LayerId,
        mesh: Mesh,
    },
    /// Moves points to absolute positions (in layer coordinates).
    MovePoints {
        layer: LayerId,
        moves: Vec<(PointId, Point)>,
    },
    /// Replaces a curve (nodes, handle modes and handles).
    SetCurve {
        layer: LayerId,
        curve: Curve,
    },
    /// Inserts a shape. `index: None` = on top.
    InsertShape {
        layer: LayerId,
        index: Option<usize>,
        shape: Shape,
    },
    RemoveShape {
        layer: LayerId,
        id: ShapeId,
    },
    SetShapeStyle {
        layer: LayerId,
        id: ShapeId,
        style: Style,
    },
    RenameShape {
        layer: LayerId,
        id: ShapeId,
        name: String,
    },
    /// Changes a shape's stacking order (0 = bottom).
    MoveShape {
        layer: LayerId,
        id: ShapeId,
        index: usize,
    },

    /// Several commands applied and undone as one.
    Batch {
        commands: Vec<Command>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum CommandError {
    LayerNotFound(LayerId),
    NotAGroup(LayerId),
    NotAVectorLayer(LayerId),
    /// Moving a group inside itself.
    LayerCycle(LayerId),
    LastLayer,
    PointNotFound(PointId),
    CurveNotFound(CurveId),
    ShapeNotFound(ShapeId),
    IndexOutOfRange(usize),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::LayerNotFound(id) => write!(f, "layer {} does not exist", id.0),
            CommandError::NotAGroup(id) => write!(f, "layer {} is not a group", id.0),
            CommandError::NotAVectorLayer(id) => write!(f, "layer {} is not a vector layer", id.0),
            CommandError::LayerCycle(id) => write!(f, "layer {} cannot be moved inside itself", id.0),
            CommandError::LastLayer => write!(f, "the document must have at least one layer"),
            CommandError::PointNotFound(id) => write!(f, "point {} does not exist", id.0),
            CommandError::CurveNotFound(id) => write!(f, "curve {} does not exist", id.0),
            CommandError::ShapeNotFound(id) => write!(f, "shape {} does not exist", id.0),
            CommandError::IndexOutOfRange(i) => write!(f, "index out of range: {i}"),
        }
    }
}

impl std::error::Error for CommandError {}

fn layer_mut(doc: &mut Document, id: LayerId) -> Result<&mut Layer, CommandError> {
    doc.layer_mut(id).ok_or(CommandError::LayerNotFound(id))
}

fn mesh_mut(doc: &mut Document, id: LayerId) -> Result<&mut Mesh, CommandError> {
    layer_mut(doc, id)?.mesh_mut().ok_or(CommandError::NotAVectorLayer(id))
}

fn shape_mut(doc: &mut Document, layer: LayerId, id: ShapeId) -> Result<&mut Shape, CommandError> {
    mesh_mut(doc, layer)?.shape_mut(id).ok_or(CommandError::ShapeNotFound(id))
}

impl Command {
    /// Applies the command and returns its inverse.
    ///
    /// On failure the document is left unchanged.
    pub fn apply(self, doc: &mut Document) -> Result<Command, CommandError> {
        match self {
            Command::InsertLayer { parent, index, layer } => {
                let not_group = CommandError::NotAGroup(parent.unwrap_or(layer.id));
                let siblings = doc.siblings_mut(parent).ok_or(not_group)?;
                let index = index.unwrap_or(siblings.len());
                if index > siblings.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let id = layer.id;
                siblings.insert(index, layer);
                Ok(Command::RemoveLayer { id })
            }
            Command::RemoveLayer { id } => {
                let (parent, index) = doc.locate_layer(id).ok_or(CommandError::LayerNotFound(id))?;
                if parent.is_none() && doc.layers.len() == 1 {
                    return Err(CommandError::LastLayer);
                }
                let layer = doc.siblings_mut(parent).expect("exists").remove(index);
                Ok(Command::InsertLayer {
                    parent,
                    index: Some(index),
                    layer,
                })
            }
            Command::MoveLayer { id, parent, index } => {
                let (old_parent, old_index) = doc.locate_layer(id).ok_or(CommandError::LayerNotFound(id))?;
                if let Some(p) = parent {
                    if doc.is_inside(p, id) {
                        return Err(CommandError::LayerCycle(id));
                    }
                    if !doc.layer(p).ok_or(CommandError::LayerNotFound(p))?.is_group() {
                        return Err(CommandError::NotAGroup(p));
                    }
                }
                // Target length as it will be after taking the layer out.
                let target_len = doc.siblings(parent).expect("is a group").len() - usize::from(parent == old_parent);
                if index > target_len {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let layer = doc.siblings_mut(old_parent).expect("exists").remove(old_index);
                doc.siblings_mut(parent).expect("is a group").insert(index, layer);
                Ok(Command::MoveLayer {
                    id,
                    parent: old_parent,
                    index: old_index,
                })
            }
            Command::RenameLayer { id, name } => {
                let old = std::mem::replace(&mut layer_mut(doc, id)?.name, name);
                Ok(Command::RenameLayer { id, name: old })
            }
            Command::SetLayerVisible { id, visible } => {
                let old = std::mem::replace(&mut layer_mut(doc, id)?.visible, visible);
                Ok(Command::SetLayerVisible { id, visible: old })
            }
            Command::SetLayerLocked { id, locked } => {
                let old = std::mem::replace(&mut layer_mut(doc, id)?.locked, locked);
                Ok(Command::SetLayerLocked { id, locked: old })
            }
            Command::SetLayerOpacity { id, opacity } => {
                let old = std::mem::replace(&mut layer_mut(doc, id)?.opacity, opacity);
                Ok(Command::SetLayerOpacity { id, opacity: old })
            }
            Command::SetLayerTransform { id, transform } => {
                let old = std::mem::replace(&mut layer_mut(doc, id)?.transform, transform);
                Ok(Command::SetLayerTransform { id, transform: old })
            }
            Command::SetLayerOrigin { id, origin } => {
                let old = std::mem::replace(&mut layer_mut(doc, id)?.origin, origin);
                Ok(Command::SetLayerOrigin { id, origin: old })
            }
            Command::ReplaceMesh { layer, mesh } => {
                let old = std::mem::replace(mesh_mut(doc, layer)?, mesh);
                Ok(Command::ReplaceMesh { layer, mesh: old })
            }
            Command::MovePoints { layer, moves } => {
                let mesh = mesh_mut(doc, layer)?;
                if let Some((missing, _)) = moves.iter().find(|(id, _)| mesh.point(*id).is_none()) {
                    return Err(CommandError::PointNotFound(*missing));
                }
                let old = moves
                    .into_iter()
                    .map(|(id, pos)| {
                        let point = mesh.point_mut(id).expect("checked above");
                        (id, std::mem::replace(&mut point.pos, pos))
                    })
                    .collect();
                Ok(Command::MovePoints { layer, moves: old })
            }
            Command::SetCurve { layer, curve } => {
                let mesh = mesh_mut(doc, layer)?;
                let id = curve.id;
                let slot = mesh.curve_mut(id).ok_or(CommandError::CurveNotFound(id))?;
                let old = std::mem::replace(slot, curve);
                Ok(Command::SetCurve { layer, curve: old })
            }
            Command::InsertShape { layer, index, shape } => {
                let mesh = mesh_mut(doc, layer)?;
                let index = index.unwrap_or(mesh.shapes.len());
                if index > mesh.shapes.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let id = shape.id;
                mesh.shapes.insert(index, shape);
                Ok(Command::RemoveShape { layer, id })
            }
            Command::RemoveShape { layer, id } => {
                let mesh = mesh_mut(doc, layer)?;
                let index = mesh.shape_index(id).ok_or(CommandError::ShapeNotFound(id))?;
                let shape = mesh.shapes.remove(index);
                Ok(Command::InsertShape {
                    layer,
                    index: Some(index),
                    shape,
                })
            }
            Command::SetShapeStyle { layer, id, style } => {
                let old = std::mem::replace(&mut shape_mut(doc, layer, id)?.style, style);
                Ok(Command::SetShapeStyle { layer, id, style: old })
            }
            Command::RenameShape { layer, id, name } => {
                let old = std::mem::replace(&mut shape_mut(doc, layer, id)?.name, name);
                Ok(Command::RenameShape { layer, id, name: old })
            }
            Command::MoveShape { layer, id, index } => {
                let mesh = mesh_mut(doc, layer)?;
                let from = mesh.shape_index(id).ok_or(CommandError::ShapeNotFound(id))?;
                if index >= mesh.shapes.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let shape = mesh.shapes.remove(from);
                mesh.shapes.insert(index, shape);
                Ok(Command::MoveShape { layer, id, index: from })
            }
            Command::Batch { commands } => {
                let mut inverses = Vec::with_capacity(commands.len());
                for command in commands {
                    match command.apply(doc) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(err) => {
                            // Revert what was applied so the document is not left half-changed.
                            for inverse in inverses.into_iter().rev() {
                                inverse.apply(doc).expect("the inverse always applies");
                            }
                            return Err(err);
                        }
                    }
                }
                inverses.reverse();
                Ok(Command::Batch { commands: inverses })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{HandleMode, MeshPoint};
    use kurbo::Vec2;

    fn doc_with_square() -> (Document, LayerId) {
        let mut doc = Document::default();
        let layer = doc.layers[0].id;
        let ids: Vec<PointId> = (0..4).map(|_| doc.new_point_id()).collect();
        let curve = doc.new_curve_id();
        let shape = doc.new_shape_id();
        let mesh = doc.mesh_mut(layer).unwrap();
        let pts: Vec<_> = ids.iter().zip([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]).map(|(id, p)| (*id, Point::from(p))).collect();
        mesh.add_closed_curve(curve, &pts, HandleMode::PEAK);
        mesh.auto_shape(curve, Some(Style::DEFAULT_FILL), None, shape, "square".into());
        (doc, layer)
    }

    #[test]
    fn move_points_and_undo() {
        let (mut doc, layer) = doc_with_square();
        let before = doc.clone();
        let id = doc.mesh(layer).unwrap().points()[0].id;
        let inverse = Command::MovePoints {
            layer,
            moves: vec![(id, Point::new(-5.0, -5.0))],
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.mesh(layer).unwrap().pos(id), Some(Point::new(-5.0, -5.0)));
        inverse.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
    }

    #[test]
    fn replace_mesh_and_undo() {
        let (mut doc, layer) = doc_with_square();
        let before = doc.clone();
        let mut mesh = doc.mesh(layer).unwrap().clone();
        mesh.add_point(MeshPoint::new(PointId(999), Point::ZERO));
        let inverse = Command::ReplaceMesh { layer, mesh }.apply(&mut doc).unwrap();
        assert_eq!(doc.mesh(layer).unwrap().points().len(), 5);
        inverse.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
    }

    #[test]
    fn batch_rolls_back_on_error() {
        let (mut doc, layer) = doc_with_square();
        let before = doc.clone();
        let result = Command::Batch {
            commands: vec![
                Command::RenameLayer {
                    id: layer,
                    name: "other".into(),
                },
                Command::RemoveShape { layer, id: ShapeId(999) },
            ],
        }
        .apply(&mut doc);
        assert_eq!(result, Err(CommandError::ShapeNotFound(ShapeId(999))));
        assert_eq!(doc, before);
    }

    #[test]
    fn move_layer_into_group_and_back() {
        let mut doc = Document::default();
        let vector = doc.layers[0].id;
        let group = doc.new_layer_id();
        Command::InsertLayer {
            parent: None,
            index: None,
            layer: Layer::group(group, "group"),
        }
        .apply(&mut doc)
        .unwrap();
        let before = doc.clone();
        let undo = Command::MoveLayer {
            id: vector,
            parent: Some(group),
            index: 0,
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.locate_layer(vector), Some((Some(group), 0)));
        // A group cannot go inside itself.
        assert_eq!(
            Command::MoveLayer {
                id: group,
                parent: Some(group),
                index: 0
            }
            .apply(&mut doc),
            Err(CommandError::LayerCycle(group))
        );
        undo.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
    }

    #[test]
    fn cannot_remove_last_layer() {
        let mut doc = Document::default();
        let id = doc.layers[0].id;
        assert_eq!(Command::RemoveLayer { id }.apply(&mut doc), Err(CommandError::LastLayer));
    }

    #[test]
    fn command_json_roundtrip() {
        let command = Command::SetLayerTransform {
            id: LayerId(3),
            transform: LayerTransform {
                translation: Vec2::new(1.0, 2.0),
                ..Default::default()
            },
        };
        let json = serde_json::to_string(&command).unwrap();
        assert!(json.contains("\"type\":\"set_layer_transform\""));
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), command);
    }
}
