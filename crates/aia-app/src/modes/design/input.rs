//! Interpretación del mouse sobre el canvas para cada herramienta del modo Draw.

use aia_core::kurbo::{Point, Rect, Vec2};
use aia_core::{Command, Editor, Handle, Node, NodeKind, Shape, ShapeId, VectorPath};
use aia_render::{BoxHandle, box_handles};
use eframe::egui;

use super::tools::{self, Gesture, NodeTarget, PenPath, Tool};
use super::{DesignMode, HIT_TOLERANCE};
use crate::canvas::CanvasView;

/// Arrastre mínimo (en puntos de pantalla) para que la pluma cree una curva.
const PEN_DRAG_THRESHOLD: f64 = 2.0;
/// Radio de impacto de las manijas de la caja de transformación, en puntos de pantalla.
const BOX_HANDLE_RADIUS: f64 = 7.0;

impl DesignMode {
    pub fn cursor(&self, view: &CanvasView, editor: &Editor, hover: Option<egui::Pos2>) -> egui::CursorIcon {
        if view.panning {
            return egui::CursorIcon::Grab;
        }
        if self.tool == Tool::Select
            && let Some(pos) = hover
            && let Some((handle, _)) = self.hit_box_handle(view, editor, pos)
        {
            return match handle {
                BoxHandle::Corner(0 | 2) => egui::CursorIcon::ResizeNwSe,
                BoxHandle::Corner(_) => egui::CursorIcon::ResizeNeSw,
                BoxHandle::Edge(0 | 2) => egui::CursorIcon::ResizeVertical,
                BoxHandle::Edge(_) => egui::CursorIcon::ResizeHorizontal,
                BoxHandle::Rotate => egui::CursorIcon::Grab,
            };
        }
        if self.tool.creates_shapes() {
            egui::CursorIcon::Crosshair
        } else {
            egui::CursorIcon::Default
        }
    }

    pub fn canvas_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, view: &CanvasView, editor: &mut Editor) {
        if view.panning {
            return;
        }
        match self.tool {
            Tool::Select => self.select_input(ui, response, view, editor),
            Tool::EditNodes => self.edit_input(ui, response, view, editor),
            Tool::Pen => self.pen_input(ui, response, view, editor),
            Tool::Rectangle | Tool::Ellipse => self.box_input(ui, response, view, editor),
        }
    }

    // ---- Selección ------------------------------------------------------------------------

    /// Manija de la caja de transformación bajo `pos`, con la caja en coordenadas del documento.
    fn hit_box_handle(&self, view: &CanvasView, editor: &Editor, pos: egui::Pos2) -> Option<(BoxHandle, Rect)> {
        let bounds = self.selection_bounds(editor)?;
        let screen = view.camera.doc_to_screen().transform_rect_bbox(bounds);
        let local = view.to_local(pos);
        box_handles(screen)
            .into_iter()
            .find(|(_, p)| p.distance(local) <= BOX_HANDLE_RADIUS)
            .map(|(handle, _)| (handle, bounds))
    }

    fn select_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, view: &CanvasView, editor: &mut Editor) {
        let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
        let tolerance = HIT_TOLERANCE / view.camera.zoom;

        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let hit = editor.doc().hit_test(view.to_doc(pos), tolerance);
            self.click_select(hit, shift, editor);
        }

        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            let start = view.to_doc(origin);
            self.gesture = if let Some((handle, bounds)) = self.hit_box_handle(view, editor, origin) {
                Gesture::Transform {
                    handle,
                    bounds,
                    start,
                    current: start,
                }
            } else if let Some(id) = editor.doc().hit_test(start, tolerance) {
                if !self.selection.contains(&id) {
                    self.click_select(Some(id), shift, editor);
                }
                Gesture::Move { start, current: start }
            } else {
                if !shift {
                    self.selection.clear();
                }
                Gesture::Marquee { start, current: start }
            };
        }

        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
            && let Gesture::Move { current, .. } | Gesture::Transform { current, .. } | Gesture::Marquee { current, .. } =
                &mut self.gesture
        {
            *current = view.to_doc(pos);
        }

        if response.drag_stopped() {
            match std::mem::take(&mut self.gesture) {
                Gesture::Move { start, current } => {
                    self.transform_selection(editor, aia_core::kurbo::Affine::translate(current - start));
                }
                Gesture::Transform {
                    handle,
                    bounds,
                    start,
                    current,
                } => {
                    let transform = tools::box_transform(handle, bounds, start, current, shift, alt);
                    self.transform_selection(editor, transform);
                }
                Gesture::Marquee { start, current } => {
                    let rect = Rect::from_points(start, current);
                    let hits = shapes_in_rect(editor, rect);
                    for id in hits {
                        if !self.selection.contains(&id) {
                            self.selection.push(id);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn click_select(&mut self, hit: Option<ShapeId>, shift: bool, editor: &Editor) {
        match hit {
            Some(id) if shift => {
                if let Some(i) = self.selection.iter().position(|s| *s == id) {
                    self.selection.remove(i);
                } else {
                    self.selection.push(id);
                }
            }
            Some(id) => self.selection = vec![id],
            None if !shift => self.selection.clear(),
            None => {}
        }
        self.nodes.clear();
        self.load_style_from_selection(editor);
    }

    /// El panel de estilo muestra el estilo de la última forma elegida.
    pub(super) fn load_style_from_selection(&mut self, editor: &Editor) {
        if let Some(shape) = self.selection.last().and_then(|id| editor.doc().shape(*id)) {
            self.style = shape.style;
        }
    }

    // ---- Rectángulo y elipse --------------------------------------------------------------

    fn box_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, view: &CanvasView, editor: &mut Editor) {
        let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));

        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
            && self.active_layer_editable(editor)
        {
            let start = view.to_doc(origin);
            self.gesture = Gesture::Box { start, current: start };
        }

        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
            && let Gesture::Box { current, .. } = &mut self.gesture
        {
            *current = view.to_doc(pos);
        }

        if response.drag_stopped()
            && let Gesture::Box { start, current } = std::mem::take(&mut self.gesture)
        {
            let rect = tools::drag_rect(start, current, shift, alt);
            let min_size = 2.0 / view.camera.zoom;
            if rect.width() >= min_size && rect.height() >= min_size {
                self.insert_shape(editor, tools::box_path(self.tool, rect), self.tool.shape_name());
            }
        }
    }

    // ---- Pluma ----------------------------------------------------------------------------

    fn pen_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, view: &CanvasView, editor: &mut Editor) {
        let (pressed, down, double, pos, alt, shift) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.button_double_clicked(egui::PointerButton::Primary),
                i.pointer.interact_pos(),
                i.modifiers.alt,
                i.modifiers.shift,
            )
        });
        let Some(pos) = pos else { return };
        let mut p = view.to_doc(pos);
        let tolerance = HIT_TOLERANCE / view.camera.zoom;

        if pressed && response.contains_pointer() {
            if self.pen.is_none() && !self.active_layer_editable(editor) {
                return;
            }
            let pen = self.pen.get_or_insert_with(|| PenPath {
                path: VectorPath::default(),
                dragging: None,
            });
            let nodes = &mut pen.path.nodes;
            if nodes.len() >= 2 && nodes[0].point.distance(p) <= tolerance {
                pen.path.closed = true;
                pen.dragging = Some(0);
            } else {
                if shift && let Some(last) = nodes.last() {
                    p = tools::snap_45(last.point, p);
                }
                nodes.push(Node::corner(p));
                pen.dragging = Some(nodes.len() - 1);
            }
        }

        if down
            && let Some(pen) = &mut self.pen
            && let Some(index) = pen.dragging
        {
            let node = &mut pen.path.nodes[index];
            let drag = p - node.point;
            if drag.hypot() * view.camera.zoom >= PEN_DRAG_THRESHOLD {
                if alt {
                    // Rompe la simetría: solo cambia la manija de salida.
                    node.kind = NodeKind::Corner;
                    node.handle_out = drag;
                } else {
                    *node = Node::symmetric(node.point, drag);
                }
            }
        }

        // Se mira el estado del botón (no el evento de soltar), que es más confiable:
        // si un evento de soltar se pierde, el arrastre igual termina.
        if !down
            && let Some(pen) = &mut self.pen
            && pen.dragging.is_some()
        {
            pen.dragging = None;
            if pen.path.closed {
                self.finish_pen(editor);
                return;
            }
        }

        if double && let Some(pen) = &mut self.pen {
            // El doble clic agregó dos nodos casi en el mismo lugar: se quita el último.
            let nodes = &mut pen.path.nodes;
            if nodes.len() >= 2 && nodes[nodes.len() - 1].point.distance(nodes[nodes.len() - 2].point) <= tolerance {
                nodes.pop();
            }
            self.finish_pen(editor);
        }
    }

    // ---- Editar nodos ---------------------------------------------------------------------

    fn edit_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, view: &CanvasView, editor: &mut Editor) {
        let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
        let tolerance = HIT_TOLERANCE / view.camera.zoom;

        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.edit_click(view.to_doc(pos), tolerance, shift, editor);
        }

        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            let start = view.to_doc(origin);
            let target = self
                .edit_target(editor)
                .and_then(|shape| self.hit_node(shape, start, tolerance).map(|t| (shape, t)));
            self.gesture = match target {
                Some((shape, target)) => {
                    if let NodeTarget::Node(i) = target
                        && !self.nodes.contains(&i)
                    {
                        if !shift {
                            self.nodes.clear();
                        }
                        self.nodes.push(i);
                    }
                    Gesture::EditPath {
                        shape: shape.id,
                        target,
                        original: shape.path.clone(),
                        path: shape.path.clone(),
                        start: shape.transform.inverse() * start,
                    }
                }
                None => {
                    if !shift {
                        self.nodes.clear();
                    }
                    Gesture::Marquee { start, current: start }
                }
            };
        }

        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            let doc_point = view.to_doc(pos);
            match &mut self.gesture {
                Gesture::Marquee { current, .. } => *current = doc_point,
                Gesture::EditPath {
                    shape,
                    target,
                    original,
                    path,
                    start,
                } => {
                    let Some(transform) = editor.doc().shape(*shape).map(|s| s.transform) else {
                        return;
                    };
                    let local = transform.inverse() * doc_point;
                    match *target {
                        NodeTarget::Node(_) => {
                            let delta = local - *start;
                            for &i in &self.nodes {
                                path.nodes[i].point = original.nodes[i].point + delta;
                            }
                        }
                        NodeTarget::Handle(i, handle) => {
                            let node = &mut path.nodes[i];
                            if alt {
                                node.kind = NodeKind::Corner;
                            }
                            node.set_handle(handle, local - node.point);
                        }
                    }
                }
                _ => {}
            }
        }

        if response.drag_stopped() {
            match std::mem::take(&mut self.gesture) {
                Gesture::EditPath { shape, path, .. } => {
                    if editor.doc().shape(shape).is_some_and(|s| s.path != path) {
                        self.execute(editor, Command::SetShapePath { id: shape, path });
                    }
                }
                Gesture::Marquee { start, current } => {
                    let rect = Rect::from_points(start, current);
                    if let Some(shape) = self.edit_target(editor) {
                        // Con una forma en edición, el marco selecciona sus nodos.
                        for (i, node) in shape.path.nodes.iter().enumerate() {
                            if rect.contains(shape.transform * node.point) && !self.nodes.contains(&i) {
                                self.nodes.push(i);
                            }
                        }
                    } else if let Some(id) = shapes_in_rect(editor, rect).last() {
                        self.selection = vec![*id];
                        self.load_style_from_selection(editor);
                    }
                }
                _ => {}
            }
        }
    }

    /// Clic con Editar nodos: elige nodos, agrega un nodo sobre el trazado o cambia de forma.
    fn edit_click(&mut self, point: Point, tolerance: f64, shift: bool, editor: &mut Editor) {
        if let Some(shape) = self.edit_target(editor) {
            match self.hit_node(shape, point, tolerance) {
                Some(NodeTarget::Node(i)) => {
                    if shift {
                        if let Some(pos) = self.nodes.iter().position(|n| *n == i) {
                            self.nodes.remove(pos);
                        } else {
                            self.nodes.push(i);
                        }
                    } else {
                        self.nodes = vec![i];
                    }
                    return;
                }
                Some(NodeTarget::Handle(..)) => return,
                None => {}
            }
            let local = shape.transform.inverse() * point;
            if let Some((segment, t)) = shape.path.nearest_segment(local, tolerance) {
                let mut path = shape.path.clone();
                let id = shape.id;
                let index = path.insert_node(segment, t);
                self.nodes = vec![index];
                self.execute(editor, Command::SetShapePath { id, path });
                return;
            }
        }
        let hit = editor.doc().hit_test(point, tolerance);
        if hit.is_none() && !self.nodes.is_empty() {
            // Primer clic en el vacío: suelta los nodos; el segundo, la forma.
            self.nodes.clear();
            return;
        }
        self.click_select(hit, false, editor);
    }

    /// Nodo o manija bajo el punto. Las manijas solo existen en los nodos seleccionados.
    fn hit_node(&self, shape: &Shape, point: Point, tolerance: f64) -> Option<NodeTarget> {
        let to_doc = shape.transform;
        for &index in &self.nodes {
            let Some(node) = shape.path.nodes.get(index) else { continue };
            for handle in [Handle::Out, Handle::In] {
                let offset = node.handle(handle);
                if offset != Vec2::ZERO && (to_doc * (node.point + offset)).distance(point) <= tolerance {
                    return Some(NodeTarget::Handle(index, handle));
                }
            }
        }
        shape
            .path
            .nodes
            .iter()
            .position(|n| (to_doc * n.point).distance(point) <= tolerance)
            .map(NodeTarget::Node)
    }
}

/// Formas de capas editables cuya caja toca el rectángulo (de abajo hacia arriba).
fn shapes_in_rect(editor: &Editor, rect: Rect) -> Vec<ShapeId> {
    editor
        .doc()
        .layers
        .iter()
        .filter(|l| l.is_editable())
        .flat_map(|l| &l.shapes)
        .filter(|s| s.bounds().overlaps(rect) || rect.contains_rect(s.bounds()))
        .map(|s| s.id)
        .collect()
}
