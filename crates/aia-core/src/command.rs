use std::fmt;

use kurbo::Affine;
use serde::{Deserialize, Serialize};

use crate::document::{Document, Layer, LayerId, Shape, ShapeId, Style};
use crate::path::VectorPath;

/// Una operación atómica sobre el documento.
///
/// Los comandos son serializables a JSON: la misma lista que usa la UI se
/// expondrá como herramientas a la IA de cada panel.
///
/// Aplicar un comando devuelve su inverso, que es lo que guarda el historial
/// para deshacer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// Inserta una forma en una capa. `index: None` la pone arriba de todo.
    InsertShape {
        layer: LayerId,
        index: Option<usize>,
        shape: Shape,
    },
    RemoveShape {
        id: ShapeId,
    },
    /// Pre-multiplica la transformación de las formas (mover, escalar, rotar).
    TransformShapes {
        ids: Vec<ShapeId>,
        transform: Affine,
    },
    SetShapeStyle {
        id: ShapeId,
        style: Style,
    },
    /// Reemplaza el trazado (nodos) de una forma.
    SetShapePath {
        id: ShapeId,
        path: VectorPath,
    },
    /// Inserta una capa. `index: None` la pone arriba de todo.
    InsertLayer {
        index: Option<usize>,
        layer: Layer,
    },
    RemoveLayer {
        id: LayerId,
    },
    SetLayerVisible {
        id: LayerId,
        visible: bool,
    },
    SetLayerLocked {
        id: LayerId,
        locked: bool,
    },
    RenameLayer {
        id: LayerId,
        name: String,
    },
    /// Varios comandos que se aplican y se deshacen como uno solo.
    Batch {
        commands: Vec<Command>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum CommandError {
    LayerNotFound(LayerId),
    ShapeNotFound(ShapeId),
    IndexOutOfRange(usize),
    LastLayer,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::LayerNotFound(id) => write!(f, "no existe la capa {}", id.0),
            CommandError::ShapeNotFound(id) => write!(f, "no existe la forma {}", id.0),
            CommandError::IndexOutOfRange(i) => write!(f, "índice fuera de rango: {i}"),
            CommandError::LastLayer => write!(f, "el documento debe tener al menos una capa"),
        }
    }
}

impl std::error::Error for CommandError {}

impl Command {
    /// Aplica el comando y devuelve su inverso.
    ///
    /// Si falla, el documento queda sin cambios.
    pub fn apply(self, doc: &mut Document) -> Result<Command, CommandError> {
        match self {
            Command::InsertShape {
                layer,
                index,
                shape,
            } => {
                let layer_ref = doc
                    .layer_mut(layer)
                    .ok_or(CommandError::LayerNotFound(layer))?;
                let index = index.unwrap_or(layer_ref.shapes.len());
                if index > layer_ref.shapes.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let id = shape.id;
                layer_ref.shapes.insert(index, shape);
                Ok(Command::RemoveShape { id })
            }
            Command::RemoveShape { id } => {
                let (layer, index) = doc.locate_shape(id).ok_or(CommandError::ShapeNotFound(id))?;
                let shape = doc
                    .layer_mut(layer)
                    .expect("la capa existe")
                    .shapes
                    .remove(index);
                Ok(Command::InsertShape {
                    layer,
                    index: Some(index),
                    shape,
                })
            }
            Command::TransformShapes { ids, transform } => {
                if let Some(missing) = ids.iter().find(|id| doc.shape(**id).is_none()) {
                    return Err(CommandError::ShapeNotFound(*missing));
                }
                for id in &ids {
                    let shape = doc.shape_mut(*id).expect("verificado arriba");
                    shape.transform = transform * shape.transform;
                }
                Ok(Command::TransformShapes {
                    ids,
                    transform: transform.inverse(),
                })
            }
            Command::SetShapeStyle { id, style } => {
                let shape = doc.shape_mut(id).ok_or(CommandError::ShapeNotFound(id))?;
                let old = std::mem::replace(&mut shape.style, style);
                Ok(Command::SetShapeStyle { id, style: old })
            }
            Command::SetShapePath { id, path } => {
                let shape = doc.shape_mut(id).ok_or(CommandError::ShapeNotFound(id))?;
                let old = std::mem::replace(&mut shape.path, path);
                Ok(Command::SetShapePath { id, path: old })
            }
            Command::InsertLayer { index, layer } => {
                let index = index.unwrap_or(doc.layers.len());
                if index > doc.layers.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let id = layer.id;
                doc.layers.insert(index, layer);
                Ok(Command::RemoveLayer { id })
            }
            Command::RemoveLayer { id } => {
                let index = doc.layer_index(id).ok_or(CommandError::LayerNotFound(id))?;
                if doc.layers.len() == 1 {
                    return Err(CommandError::LastLayer);
                }
                let layer = doc.layers.remove(index);
                Ok(Command::InsertLayer {
                    index: Some(index),
                    layer,
                })
            }
            Command::SetLayerVisible { id, visible } => {
                let layer = doc.layer_mut(id).ok_or(CommandError::LayerNotFound(id))?;
                let old = std::mem::replace(&mut layer.visible, visible);
                Ok(Command::SetLayerVisible { id, visible: old })
            }
            Command::SetLayerLocked { id, locked } => {
                let layer = doc.layer_mut(id).ok_or(CommandError::LayerNotFound(id))?;
                let old = std::mem::replace(&mut layer.locked, locked);
                Ok(Command::SetLayerLocked { id, locked: old })
            }
            Command::RenameLayer { id, name } => {
                let layer = doc.layer_mut(id).ok_or(CommandError::LayerNotFound(id))?;
                let old = std::mem::replace(&mut layer.name, name);
                Ok(Command::RenameLayer { id, name: old })
            }
            Command::Batch { commands } => {
                let mut inverses = Vec::with_capacity(commands.len());
                for command in commands {
                    match command.apply(doc) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(err) => {
                            // Revierte lo ya aplicado para no dejar el documento a medias.
                            for inverse in inverses.into_iter().rev() {
                                inverse.apply(doc).expect("el inverso siempre aplica");
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
    use crate::document::Color;
    use kurbo::{Rect, Vec2};

    fn rect_shape(doc: &mut Document) -> Shape {
        Shape {
            id: doc.new_shape_id(),
            path: VectorPath::rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
            transform: Affine::IDENTITY,
            style: Style::default(),
        }
    }

    #[test]
    fn insert_and_undo_shape() {
        let mut doc = Document::default();
        let layer = doc.layers[0].id;
        let shape = rect_shape(&mut doc);
        let before = doc.clone();

        let inverse = Command::InsertShape {
            layer,
            index: None,
            shape,
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.layers[0].shapes.len(), 1);

        inverse.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
    }

    #[test]
    fn batch_rolls_back_on_error() {
        let mut doc = Document::default();
        let layer = doc.layers[0].id;
        let shape = rect_shape(&mut doc);
        let before = doc.clone();

        let result = Command::Batch {
            commands: vec![
                Command::InsertShape {
                    layer,
                    index: None,
                    shape,
                },
                Command::RemoveShape { id: ShapeId(999) },
            ],
        }
        .apply(&mut doc);

        assert_eq!(result, Err(CommandError::ShapeNotFound(ShapeId(999))));
        assert_eq!(doc, before);
    }

    #[test]
    fn transform_inverse_restores() {
        let mut doc = Document::default();
        let layer = doc.layers[0].id;
        let shape = rect_shape(&mut doc);
        let id = shape.id;
        Command::InsertShape {
            layer,
            index: None,
            shape,
        }
        .apply(&mut doc)
        .unwrap();

        let inverse = Command::TransformShapes {
            ids: vec![id],
            transform: Affine::translate(Vec2::new(5.0, 7.0)),
        }
        .apply(&mut doc)
        .unwrap();
        inverse.apply(&mut doc).unwrap();
        assert_eq!(doc.shape(id).unwrap().transform, Affine::IDENTITY);
    }

    #[test]
    fn cannot_remove_last_layer() {
        let mut doc = Document::default();
        let id = doc.layers[0].id;
        assert_eq!(
            Command::RemoveLayer { id }.apply(&mut doc),
            Err(CommandError::LastLayer)
        );
    }

    #[test]
    fn command_json_roundtrip() {
        let command = Command::SetShapeStyle {
            id: ShapeId(3),
            style: Style {
                fill: Some(Color::rgb(1, 2, 3)),
                stroke: None,
            },
        };
        let json = serde_json::to_string(&command).unwrap();
        assert!(json.contains("\"type\":\"set_shape_style\""));
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), command);
    }
}
