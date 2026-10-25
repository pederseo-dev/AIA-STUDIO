use crate::command::{Command, CommandError};
use crate::document::{Document, LayerId};
use crate::mesh::{CurveId, PointId, ShapeId};

/// The open document together with its undo/redo history.
///
/// It is the only entry point for modifying the document.
#[derive(Default)]
pub struct Editor {
    doc: Document,
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,
    /// Key of the last mergeable command (see [`Editor::execute_merged`]).
    merge_key: Option<String>,
    /// Increases with every change (including undo/redo). Tells whether there are
    /// unsaved changes or the view must be redrawn.
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

    pub fn new_layer_id(&mut self) -> LayerId {
        self.doc.new_layer_id()
    }

    pub fn new_point_id(&mut self) -> PointId {
        self.doc.new_point_id()
    }

    pub fn new_curve_id(&mut self) -> CurveId {
        self.doc.new_curve_id()
    }

    pub fn new_shape_id(&mut self) -> ShapeId {
        self.doc.new_shape_id()
    }

    pub fn execute(&mut self, command: Command) -> Result<(), CommandError> {
        let inverse = command.apply(&mut self.doc)?;
        self.undo_stack.push(inverse);
        self.redo_stack.clear();
        self.merge_key = None;
        self.revision += 1;
        Ok(())
    }

    /// Like [`Editor::execute`], but if the previous command had the same `key`, both
    /// become a single undo step. Meant for continuous changes (dragging a numeric field,
    /// picking a color): they are undone at once.
    pub fn execute_merged(&mut self, command: Command, key: String) -> Result<(), CommandError> {
        if self.merge_key.as_ref() != Some(&key) || self.undo_stack.is_empty() {
            self.execute(command)?;
            self.merge_key = Some(key);
            return Ok(());
        }
        // The stored inverse already restores the state before the first change.
        command.apply(&mut self.doc)?;
        self.redo_stack.clear();
        self.revision += 1;
        Ok(())
    }

    /// Ends merging: the next change will be a separate undo step.
    pub fn end_merge(&mut self) {
        self.merge_key = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(command) = self.undo_stack.pop() {
            let redo = command.apply(&mut self.doc).expect("the inverse always applies");
            self.redo_stack.push(redo);
            self.merge_key = None;
            self.revision += 1;
        }
    }

    pub fn redo(&mut self) {
        if let Some(command) = self.redo_stack.pop() {
            let undo = command.apply(&mut self.doc).expect("the inverse always applies");
            self.undo_stack.push(undo);
            self.merge_key = None;
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
                parent: None,
                index: None,
                layer: Layer::vector(id, "Layer 2"),
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

    #[test]
    fn merged_changes_undo_in_one_step() {
        let mut editor = Editor::default();
        let id = editor.doc().layers[0].id;
        for opacity in [0.9, 0.5, 0.2] {
            editor
                .execute_merged(Command::SetLayerOpacity { id, opacity }, "opacity".into())
                .unwrap();
        }
        assert_eq!(editor.doc().layers[0].opacity, 0.2);
        editor.undo();
        assert_eq!(editor.doc().layers[0].opacity, 1.0);
        assert!(!editor.can_undo());
    }
}
