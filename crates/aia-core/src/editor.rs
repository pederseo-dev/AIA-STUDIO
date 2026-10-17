use crate::command::{Command, CommandError};
use crate::document::{Document, LayerId, ShapeId};

/// El documento abierto junto con su historial de undo/redo.
///
/// Es el único punto de entrada para modificar el documento.
#[derive(Default)]
pub struct Editor {
    doc: Document,
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,
    /// Aumenta con cada cambio (incluye undo/redo). Sirve para saber si hay
    /// cambios sin guardar o si hay que volver a renderizar.
    revision: u64,
    saved_revision: u64,
}

impl Editor {
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            ..Default::default()
        }
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn new_shape_id(&mut self) -> ShapeId {
        self.doc.new_shape_id()
    }

    pub fn new_layer_id(&mut self) -> LayerId {
        self.doc.new_layer_id()
    }

    pub fn execute(&mut self, command: Command) -> Result<(), CommandError> {
        let inverse = command.apply(&mut self.doc)?;
        self.undo_stack.push(inverse);
        self.redo_stack.clear();
        self.revision += 1;
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(command) = self.undo_stack.pop() {
            let redo = command.apply(&mut self.doc).expect("el inverso siempre aplica");
            self.redo_stack.push(redo);
            self.revision += 1;
        }
    }

    pub fn redo(&mut self) {
        if let Some(command) = self.redo_stack.pop() {
            let undo = command.apply(&mut self.doc).expect("el inverso siempre aplica");
            self.undo_stack.push(undo);
            self.revision += 1;
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn mark_saved(&mut self) {
        self.saved_revision = self.revision;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Layer;

    #[test]
    fn undo_redo_cycle() {
        let mut editor = Editor::default();
        let id = editor.new_layer_id();
        editor
            .execute(Command::InsertLayer {
                index: None,
                layer: Layer::new(id, "Capa 2"),
            })
            .unwrap();
        assert_eq!(editor.doc().layers.len(), 2);
        assert!(editor.is_dirty());

        editor.undo();
        assert_eq!(editor.doc().layers.len(), 1);
        editor.redo();
        assert_eq!(editor.doc().layers.len(), 2);
        assert!(!editor.can_redo());
    }
}
