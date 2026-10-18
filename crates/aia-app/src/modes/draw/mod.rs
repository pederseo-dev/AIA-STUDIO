mod tools;

use aia_core::kurbo::{Affine, BezPath, Point, Vec2};
use aia_core::{
    Color, Command, Editor, Handle, Layer, LayerId, Node, NodeKind, Shape, ShapeId, StrokeStyle, Style, VectorPath,
};
use aia_render::{NodeOverlay, Overlay};
use eframe::egui;
use egui_phosphor::regular as icons;

use crate::canvas::CanvasView;
pub use tools::Tool;
use tools::{Gesture, NodeTarget, PenPath};

/// Radio de impacto del mouse, en puntos de pantalla.
const HIT_TOLERANCE: f64 = 5.0;
/// Arrastre mínimo (en puntos de pantalla) para que la pluma cree una curva.
const PEN_DRAG_THRESHOLD: f64 = 2.0;

/// Modo Draw: crear y editar la geometría base y su estilo.
pub struct DrawMode {
    tool: Tool,
    /// Estilo para las formas nuevas.
    style: Style,
    selection: Vec<ShapeId>,
    active_layer: Option<LayerId>,
    gesture: Gesture,
    pen: Option<PenPath>,
    /// Nodo seleccionado de la forma que se edita (la última seleccionada).
    node: Option<usize>,
    messages: Vec<String>,
}

impl Default for DrawMode {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            style: Style::default(),
            selection: Vec::new(),
            active_layer: None,
            gesture: Gesture::None,
            pen: None,
            node: None,
            messages: Vec::new(),
        }
    }
}

/// Lo que se dibuja encima del documento en este frame (datos con dueño).
#[derive(Default)]
pub struct DrawOverlay {
    preview: Option<Shape>,
    guide: Option<BezPath>,
}

impl DrawMode {
    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Mensajes nuevos para mostrar al usuario (errores, avisos).
    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages)
    }

    /// Olvida el estado ligado al documento anterior.
    pub fn reset(&mut self) {
        self.selection.clear();
        self.active_layer = None;
        self.gesture = Gesture::None;
        self.pen = None;
        self.node = None;
    }

    /// Se llama al salir del modo: termina lo que esté a medias.
    pub fn leave(&mut self, editor: &mut Editor) {
        self.gesture = Gesture::None;
        self.finish_pen(editor);
    }

    /// Descarta referencias a formas/capas/nodos que ya no existen (por ejemplo, tras deshacer).
    pub fn sync_with_document(&mut self, editor: &Editor) {
        let doc = editor.doc();
        self.selection.retain(|id| doc.shape(*id).is_some());
        if self.active_layer.and_then(|id| doc.layer(id)).is_none() {
            self.active_layer = doc.layers.last().map(|l| l.id);
        }
        let nodes = self.edit_target(editor).map_or(0, |s| s.path.nodes.len());
        if self.node.is_some_and(|i| i >= nodes) {
            self.node = None;
        }
    }

    /// La forma cuyos nodos se editan: la última seleccionada, si su capa es editable.
    fn edit_target<'a>(&self, editor: &'a Editor) -> Option<&'a Shape> {
        let id = *self.selection.last()?;
        let (layer, _) = editor.doc().locate_shape(id)?;
        editor
            .doc()
            .layer(layer)
            .filter(|l| l.is_editable())
            .and_then(|_| editor.doc().shape(id))
    }

    pub fn tools_ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        for tool in Tool::ALL {
            if super::tool_button(ui, self.tool == tool, tool.icon(), tool.name(), tool.shortcut_hint()).clicked() {
                self.set_tool(tool, editor);
            }
        }
    }

    fn set_tool(&mut self, tool: Tool, editor: &mut Editor) {
        if tool != Tool::Pen {
            self.finish_pen(editor);
        }
        self.tool = tool;
        self.gesture = Gesture::None;
        self.node = None;
    }

    pub fn sidebar_ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        match self.tool {
            Tool::Pen => {
                egui::CollapsingHeader::new("Pluma").default_open(true).show(ui, |ui| {
                    ui.weak("Clic: nodo de esquina\nArrastrar: nodo con curva\nAlt al arrastrar: solo la manija de salida\nShift: ángulos de 45°\nClic en el primer nodo: cerrar\nEnter o Esc: terminar");
                });
            }
            Tool::EditNodes => {
                egui::CollapsingHeader::new("Nodo")
                    .default_open(true)
                    .show(ui, |ui| self.node_section(ui, editor));
            }
            _ => {}
        }
        egui::CollapsingHeader::new("Estilo")
            .default_open(true)
            .show(ui, |ui| self.style_section(ui, editor));
        egui::CollapsingHeader::new("Capas")
            .default_open(true)
            .show(ui, |ui| self.layers_section(ui, editor));
    }

    fn node_section(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let Some(shape) = self.edit_target(editor) else {
            ui.weak("Seleccioná una forma para editar sus nodos.");
            return;
        };
        let (id, mut path) = (shape.id, shape.path.clone());
        let mut changed = false;

        let mut closed = path.closed;
        if ui.checkbox(&mut closed, "Trazado cerrado").changed() && path.nodes.len() > 2 {
            path.closed = closed;
            changed = true;
        }

        if let Some(index) = self.node {
            ui.label(format!("Nodo {} de {}", index + 1, path.nodes.len()));
            ui.horizontal(|ui| {
                for kind in NodeKind::ALL {
                    if ui.selectable_label(path.nodes[index].kind == kind, kind.name()).clicked() {
                        path.set_kind(index, kind);
                        changed = true;
                    }
                }
            });
            let can_remove = path.nodes.len() > 2;
            if ui
                .add_enabled(can_remove, egui::Button::new(format!("{} Borrar nodo", icons::TRASH)))
                .on_hover_text("Supr")
                .clicked()
            {
                path.nodes.remove(index);
                self.node = None;
                changed = true;
            }
        } else {
            ui.weak("Clic en un nodo para seleccionarlo.\nClic sobre el trazado agrega un nodo.");
        }

        if changed {
            self.execute(editor, Command::SetShapePath { id, path });
        }
    }

    fn style_section(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        ui.horizontal(|ui| {
            let mut has_fill = self.style.fill.is_some();
            if ui.checkbox(&mut has_fill, "Relleno").changed() {
                self.style.fill = has_fill.then_some(Color::rgb(230, 120, 60));
            }
            if let Some(fill) = &mut self.style.fill {
                color_button(ui, fill);
            }
        });

        ui.horizontal(|ui| {
            let mut has_stroke = self.style.stroke.is_some();
            if ui.checkbox(&mut has_stroke, "Trazo").changed() {
                self.style.stroke = has_stroke.then_some(StrokeStyle {
                    color: Color::BLACK,
                    width: 2.0,
                });
            }
            if let Some(stroke) = &mut self.style.stroke {
                color_button(ui, &mut stroke.color);
            }
        });
        if let Some(stroke) = &mut self.style.stroke {
            ui.horizontal(|ui| {
                ui.label("Grosor");
                ui.add(egui::DragValue::new(&mut stroke.width).range(0.25..=200.0).speed(0.25));
            });
        }

        let apply = egui::Button::new("Aplicar a la selección");
        if ui.add_enabled(!self.selection.is_empty(), apply).clicked() {
            let commands = self
                .selection
                .iter()
                .map(|&id| Command::SetShapeStyle { id, style: self.style })
                .collect();
            self.execute(editor, Command::Batch { commands });
        }
    }

    fn layers_section(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        ui.horizontal(|ui| {
            if ui.button(icons::PLUS).on_hover_text("Nueva capa").clicked() {
                let id = editor.new_layer_id();
                let name = format!("Capa {}", editor.doc().layers.len() + 1);
                // Arriba de la capa activa.
                let index = self
                    .active_layer
                    .and_then(|id| editor.doc().layer_index(id))
                    .map(|i| i + 1);
                self.execute(
                    editor,
                    Command::InsertLayer {
                        index,
                        layer: Layer::new(id, name),
                    },
                );
                self.active_layer = Some(id);
            }
            let can_remove = editor.doc().layers.len() > 1 && self.active_layer.is_some();
            if ui
                .add_enabled(can_remove, egui::Button::new(icons::TRASH))
                .on_hover_text("Borrar capa activa")
                .clicked()
                && let Some(id) = self.active_layer
            {
                self.execute(editor, Command::RemoveLayer { id });
            }
        });

        // Se listan de arriba hacia abajo, como en cualquier editor.
        let mut pending = None;
        for layer in editor.doc().layers.iter().rev() {
            ui.horizontal(|ui| {
                let (eye, eye_tip) = if layer.visible {
                    (icons::EYE, "Ocultar")
                } else {
                    (icons::EYE_SLASH, "Mostrar")
                };
                if ui.add(egui::Button::new(eye).frame(false)).on_hover_text(eye_tip).clicked() {
                    pending = Some(Command::SetLayerVisible {
                        id: layer.id,
                        visible: !layer.visible,
                    });
                }
                let (lock, lock_tip) = if layer.locked {
                    (icons::LOCK_SIMPLE, "Desbloquear")
                } else {
                    (icons::LOCK_SIMPLE_OPEN, "Bloquear")
                };
                if ui.add(egui::Button::new(lock).frame(false)).on_hover_text(lock_tip).clicked() {
                    pending = Some(Command::SetLayerLocked {
                        id: layer.id,
                        locked: !layer.locked,
                    });
                }
                let active = self.active_layer == Some(layer.id);
                if ui.selectable_label(active, &layer.name).clicked() {
                    self.active_layer = Some(layer.id);
                }
            });
        }
        if let Some(command) = pending {
            self.execute(editor, command);
        }
    }

    fn execute(&mut self, editor: &mut Editor, command: Command) {
        if let Err(e) = editor.execute(command) {
            self.messages.push(e.to_string());
        }
    }

    fn active_layer_editable(&mut self, editor: &Editor) -> bool {
        let editable = self
            .active_layer
            .and_then(|id| editor.doc().layer(id))
            .is_some_and(Layer::is_editable);
        if !editable {
            self.messages.push("La capa activa está oculta o bloqueada".into());
        }
        editable
    }

    /// Atajos propios del modo (herramientas, borrar, terminar, cancelar).
    pub fn handle_keys(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let (tool, delete, escape, enter) = ui.input(|i| {
            let tool = if i.modifiers.any() {
                None
            } else {
                Tool::ALL.into_iter().find(|t| i.key_pressed(t.shortcut()))
            };
            let delete = i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace);
            (tool, delete, i.key_pressed(egui::Key::Escape), i.key_pressed(egui::Key::Enter))
        });
        if let Some(tool) = tool {
            self.set_tool(tool, editor);
        }
        if enter || (escape && self.pen.is_some()) {
            self.finish_pen(editor);
        } else if escape {
            self.gesture = Gesture::None;
            if self.node.take().is_none() {
                self.selection.clear();
            }
        }
        if delete {
            self.delete_selected(editor);
        }
    }

    fn delete_selected(&mut self, editor: &mut Editor) {
        if self.tool == Tool::EditNodes
            && let Some(index) = self.node
            && let Some(shape) = self.edit_target(editor)
        {
            if shape.path.nodes.len() > 2 {
                let mut path = shape.path.clone();
                path.nodes.remove(index);
                let id = shape.id;
                self.node = None;
                self.execute(editor, Command::SetShapePath { id, path });
            }
            return;
        }
        if !self.selection.is_empty() {
            let commands = self
                .selection
                .drain(..)
                .map(|id| Command::RemoveShape { id })
                .collect();
            self.execute(editor, Command::Batch { commands });
        }
    }

    pub fn cursor(&self, view: &CanvasView) -> egui::CursorIcon {
        if view.panning {
            egui::CursorIcon::Grab
        } else if self.tool.creates_shapes() {
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
            Tool::Pen => self.pen_input(ui, response, view, editor),
            Tool::EditNodes => self.edit_input(ui, response, view, editor),
            Tool::Select | Tool::Rectangle | Tool::Ellipse => self.shape_input(ui, response, view, editor),
        }
    }

    // ---- Selección, rectángulo y elipse -------------------------------------------------

    fn shape_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, view: &CanvasView, editor: &mut Editor) {
        let shift = ui.input(|i| i.modifiers.shift);
        let tolerance = HIT_TOLERANCE / view.camera.zoom;

        if response.clicked()
            && self.tool == Tool::Select
            && let Some(pos) = response.interact_pointer_pos()
        {
            let hit = editor.doc().hit_test(view.to_doc(pos), tolerance);
            self.click_select(hit, shift, editor);
        }

        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            let start = view.to_doc(origin);
            let tool = self.tool;
            self.gesture = match tool {
                Tool::Select => match editor.doc().hit_test(start, tolerance) {
                    Some(id) => {
                        if !self.selection.contains(&id) {
                            self.click_select(Some(id), shift, editor);
                        }
                        Gesture::Move { start, current: start }
                    }
                    None => {
                        if !shift {
                            self.selection.clear();
                        }
                        Gesture::None
                    }
                },
                _ if self.active_layer_editable(editor) => Gesture::Box { start, current: start },
                _ => Gesture::None,
            };
        }

        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
            && let Gesture::Move { current, .. } | Gesture::Box { current, .. } = &mut self.gesture
        {
            *current = view.to_doc(pos);
        }

        if response.drag_stopped() {
            match std::mem::take(&mut self.gesture) {
                Gesture::Move { start, current } => {
                    let delta = current - start;
                    if delta != Vec2::ZERO && !self.selection.is_empty() {
                        self.execute(
                            editor,
                            Command::TransformShapes {
                                ids: self.selection.clone(),
                                transform: Affine::translate(delta),
                            },
                        );
                    }
                }
                Gesture::Box { start, current } => {
                    let rect = tools::drag_rect(start, current, shift);
                    let min_size = 2.0 / view.camera.zoom;
                    if rect.width() >= min_size && rect.height() >= min_size {
                        self.insert_shape(editor, tools::box_path(self.tool, rect));
                    }
                }
                Gesture::None | Gesture::EditPath { .. } => {}
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
        self.node = None;
        // El panel de estilo muestra el estilo de la última forma elegida.
        if let Some(shape) = self.selection.last().and_then(|id| editor.doc().shape(*id)) {
            self.style = shape.style;
        }
    }

    fn insert_shape(&mut self, editor: &mut Editor, path: VectorPath) {
        let Some(layer) = self.active_layer else { return };
        let shape = Shape {
            id: editor.new_shape_id(),
            path,
            transform: Affine::IDENTITY,
            style: self.style,
        };
        self.execute(
            editor,
            Command::InsertShape {
                layer,
                index: None,
                shape,
            },
        );
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

    /// Convierte el trazado de la pluma en una forma (si tiene al menos dos nodos).
    fn finish_pen(&mut self, editor: &mut Editor) {
        let Some(pen) = self.pen.take() else { return };
        if pen.path.nodes.len() >= 2 {
            self.insert_shape(editor, pen.path);
        }
    }

    // ---- Editar nodos ---------------------------------------------------------------------

    fn edit_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, view: &CanvasView, editor: &mut Editor) {
        let alt = ui.input(|i| i.modifiers.alt);
        let tolerance = HIT_TOLERANCE / view.camera.zoom;

        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.edit_click(view.to_doc(pos), tolerance, editor);
        }

        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
            && let Some(shape) = self.edit_target(editor)
            && let Some(target) = self.hit_node(shape, view.to_doc(origin), tolerance)
        {
            let (NodeTarget::Node(index) | NodeTarget::Handle(index, _)) = target;
            self.node = Some(index);
            self.gesture = Gesture::EditPath {
                shape: shape.id,
                target,
                path: shape.path.clone(),
            };
        }

        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
            && let Gesture::EditPath { shape, target, path } = &mut self.gesture
            && let Some(transform) = editor.doc().shape(*shape).map(|s| s.transform)
        {
            let local = transform.inverse() * view.to_doc(pos);
            match *target {
                NodeTarget::Node(i) => path.nodes[i].point = local,
                NodeTarget::Handle(i, handle) => {
                    let node = &mut path.nodes[i];
                    if alt {
                        node.kind = NodeKind::Corner;
                    }
                    node.set_handle(handle, local - node.point);
                }
            }
        }

        if response.drag_stopped()
            && let Gesture::EditPath { shape, path, .. } = std::mem::take(&mut self.gesture)
            && editor.doc().shape(shape).is_some_and(|s| s.path != path)
        {
            self.execute(editor, Command::SetShapePath { id: shape, path });
        }
    }

    /// Clic con Editar nodos: elige nodo, agrega un nodo sobre el trazado o cambia de forma.
    fn edit_click(&mut self, point: Point, tolerance: f64, editor: &mut Editor) {
        if let Some(shape) = self.edit_target(editor) {
            if let Some(target) = self.hit_node(shape, point, tolerance) {
                let (NodeTarget::Node(index) | NodeTarget::Handle(index, _)) = target;
                self.node = Some(index);
                return;
            }
            let local = shape.transform.inverse() * point;
            if let Some((segment, t)) = shape.path.nearest_segment(local, tolerance) {
                let mut path = shape.path.clone();
                let id = shape.id;
                let index = path.insert_node(segment, t);
                self.node = Some(index);
                self.execute(editor, Command::SetShapePath { id, path });
                return;
            }
        }
        let hit = editor.doc().hit_test(point, tolerance);
        self.click_select(hit, false, editor);
    }

    /// Nodo o manija bajo el punto. Las manijas solo existen en el nodo seleccionado.
    fn hit_node(&self, shape: &Shape, point: Point, tolerance: f64) -> Option<NodeTarget> {
        let to_doc = shape.transform;
        if let Some(index) = self.node
            && let Some(node) = shape.path.nodes.get(index)
        {
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

    // ---- Overlay --------------------------------------------------------------------------

    /// Prepara lo que se dibuja encima del documento (forma en creación, guía de la pluma).
    pub fn prepare_overlay(&self, ui: &egui::Ui, view: &CanvasView) -> DrawOverlay {
        let shift = ui.input(|i| i.modifiers.shift);
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|p| view.rect.contains(*p));
        let mut overlay = DrawOverlay::default();

        if let Gesture::Box { start, current } = &self.gesture {
            let rect = tools::drag_rect(*start, *current, shift);
            overlay.preview = Some(self.preview(tools::box_path(self.tool, rect)));
        }

        if let Some(pen) = &self.pen {
            overlay.preview = Some(self.preview(pen.path.clone()));
            // Tramo elástico desde el último nodo hasta el mouse.
            if pen.dragging.is_none()
                && let (Some(last), Some(hover)) = (pen.path.nodes.last(), hover)
            {
                let mut target = view.to_doc(hover);
                if shift {
                    target = tools::snap_45(last.point, target);
                }
                let mut guide = BezPath::new();
                guide.move_to(last.point);
                if last.handle_out == Vec2::ZERO {
                    guide.line_to(target);
                } else {
                    guide.curve_to(last.point + last.handle_out, target, target);
                }
                overlay.guide = Some(guide);
            }
        }
        overlay
    }

    fn preview(&self, path: VectorPath) -> Shape {
        Shape {
            id: ShapeId(0),
            path,
            transform: Affine::IDENTITY,
            style: self.style,
        }
    }

    pub fn overlay<'a>(&'a self, data: &'a DrawOverlay, editor: &'a Editor) -> Overlay<'a> {
        let moving = match &self.gesture {
            Gesture::Move { start, current } => Some((self.selection.as_slice(), Affine::translate(*current - *start))),
            _ => None,
        };
        let edited = match &self.gesture {
            Gesture::EditPath { shape, path, .. } => Some((*shape, path)),
            _ => None,
        };

        let nodes = if let Some(pen) = &self.pen {
            Some(NodeOverlay {
                path: &pen.path,
                transform: Affine::IDENTITY,
                selected: pen.path.nodes.len().checked_sub(1),
            })
        } else if self.tool == Tool::EditNodes {
            self.edit_target(editor).map(|shape| NodeOverlay {
                path: edited.map_or(&shape.path, |(_, p)| p),
                transform: shape.transform,
                selected: self.node,
            })
        } else {
            None
        };

        Overlay {
            moving,
            edited,
            preview: data.preview.as_ref(),
            // Con Editar nodos se ven los nodos en vez de la caja de selección.
            selection: if self.tool == Tool::EditNodes { &[] } else { &self.selection },
            guide: data.guide.as_ref(),
            nodes,
        }
    }
}

fn color_button(ui: &mut egui::Ui, color: &mut Color) {
    let mut rgba = color.to_array();
    if ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed() {
        *color = Color::from_array(rgba);
    }
}
