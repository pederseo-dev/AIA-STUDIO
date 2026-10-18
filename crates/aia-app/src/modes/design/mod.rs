mod clipboard;
mod input;
mod sidebar;
mod tools;

use std::collections::HashSet;

use aia_core::kurbo::{Affine, BezPath, Rect, Vec2};
use aia_core::{Agent, Command, Editor, Layer, LayerId, Shape, ShapeId, Style, VectorPath};
use aia_render::{NodeOverlay, Overlay};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::canvas::CanvasView;
pub use tools::Tool;
use tools::{Gesture, PenPath};

/// Radio de impacto del mouse, en puntos de pantalla.
const HIT_TOLERANCE: f64 = 5.0;
/// Desplazamiento de cada pegado/duplicado, en unidades del documento.
const PASTE_OFFSET: f64 = 10.0;

const DUPLICATE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);

/// Cambios de orden de apilado dentro de la capa.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    Up,
    Down,
    Front,
    Back,
}

/// Nombre que se está editando en el panel de capas.
enum Renaming {
    Layer(LayerId, String),
    Shape(ShapeId, String),
}

/// Modo Design: construir el personaje (dibujo vectorial; más adelante, huesos).
/// Todo lo que se hace acá modifica la base.
pub struct DesignMode {
    tool: Tool,
    /// Estilo para las formas nuevas.
    style: Style,
    selection: Vec<ShapeId>,
    active_layer: Option<LayerId>,
    gesture: Gesture,
    pen: Option<PenPath>,
    /// Nodos seleccionados de la forma que se edita (la última seleccionada).
    nodes: Vec<usize>,
    messages: Vec<String>,
    renaming: Option<Renaming>,
    /// Capas con la lista de formas plegada en el panel.
    collapsed_layers: HashSet<LayerId>,
    /// Cuántas veces se pegó lo mismo (cada pegado se desplaza un poco más).
    paste_count: u32,
    /// Transformación provisional mientras se arrastra un campo numérico.
    numeric_preview: Option<Affine>,
    /// Mantener la proporción al cambiar ancho/alto en los campos numéricos.
    keep_ratio: bool,
}

impl Default for DesignMode {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            style: Style::default(),
            selection: Vec::new(),
            active_layer: None,
            gesture: Gesture::None,
            pen: None,
            nodes: Vec::new(),
            messages: Vec::new(),
            renaming: None,
            collapsed_layers: HashSet::new(),
            paste_count: 0,
            numeric_preview: None,
            keep_ratio: true,
        }
    }
}

/// Lo que se dibuja encima del documento en este frame (datos con dueño).
#[derive(Default)]
pub struct DesignOverlay {
    preview: Option<Shape>,
    guide: Option<BezPath>,
    /// Transformación provisional de la selección (arrastre, manijas o campos numéricos).
    moving: Option<Affine>,
    marquee: Option<Rect>,
}

impl DesignMode {
    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Agente de IA que conviene según la herramienta activa. Cuando exista la
    /// herramienta Hueso, sugerirá Rigging.
    pub fn suggested_agent(&self) -> Agent {
        Agent::Draw
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
        self.nodes.clear();
        self.renaming = None;
        self.numeric_preview = None;
    }

    /// Se llama al salir del modo: termina lo que esté a medias.
    pub fn leave(&mut self, editor: &mut Editor) {
        self.gesture = Gesture::None;
        self.numeric_preview = None;
        self.finish_pen(editor);
    }

    /// Descarta referencias a formas/capas/nodos que ya no existen (por ejemplo, tras deshacer).
    pub fn sync_with_document(&mut self, editor: &Editor) {
        let doc = editor.doc();
        self.selection.retain(|id| doc.shape(*id).is_some());
        if self.active_layer.and_then(|id| doc.layer(id)).is_none() {
            self.active_layer = doc.layers.last().map(|l| l.id);
        }
        let count = self.edit_target(editor).map_or(0, |s| s.path.nodes.len());
        self.nodes.retain(|i| *i < count);
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
        self.nodes.clear();
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

    fn insert_shape(&mut self, editor: &mut Editor, path: VectorPath, base_name: &str) {
        let Some(layer) = self.active_layer else { return };
        let id = editor.new_shape_id();
        let shape = Shape {
            id,
            name: format!("{base_name} {}", id.0),
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

    fn finish_pen(&mut self, editor: &mut Editor) {
        let Some(pen) = self.pen.take() else { return };
        if pen.path.nodes.len() >= 2 {
            self.insert_shape(editor, pen.path, Tool::Pen.shape_name());
        }
    }

    // ---- Teclado --------------------------------------------------------------------------

    /// Atajos propios del modo. Se ignoran si hay un campo de texto con foco.
    pub fn handle_keys(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        struct Keys {
            tool: Option<Tool>,
            delete: bool,
            escape: bool,
            enter: bool,
            nudge: Vec2,
            order: Option<Order>,
            duplicate: bool,
            copy: bool,
            cut: bool,
            paste: Option<String>,
        }
        let keys = ui.input_mut(|i| {
            let plain = !i.modifiers.any();
            let step = if i.modifiers.shift { 10.0 } else { 1.0 };
            let mut nudge = Vec2::ZERO;
            for (key, dir) in [
                (Key::ArrowLeft, Vec2::new(-1.0, 0.0)),
                (Key::ArrowRight, Vec2::new(1.0, 0.0)),
                (Key::ArrowUp, Vec2::new(0.0, -1.0)),
                (Key::ArrowDown, Vec2::new(0.0, 1.0)),
            ] {
                if i.key_pressed(key) {
                    nudge += dir * step;
                }
            }
            let order = [
                (Key::PageUp, Order::Up),
                (Key::PageDown, Order::Down),
                (Key::Home, Order::Front),
                (Key::End, Order::Back),
            ]
            .into_iter()
            .find(|(key, _)| plain && i.key_pressed(*key))
            .map(|(_, order)| order);
            let mut keys = Keys {
                tool: if plain {
                    Tool::ALL.into_iter().find(|t| i.key_pressed(t.shortcut()))
                } else {
                    None
                },
                delete: i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                escape: i.key_pressed(Key::Escape),
                enter: i.key_pressed(Key::Enter),
                nudge,
                order,
                duplicate: i.consume_shortcut(&DUPLICATE),
                copy: false,
                cut: false,
                paste: None,
            };
            for event in &i.events {
                match event {
                    egui::Event::Copy => keys.copy = true,
                    egui::Event::Cut => keys.cut = true,
                    egui::Event::Paste(text) => keys.paste = Some(text.clone()),
                    _ => {}
                }
            }
            keys
        });

        if let Some(tool) = keys.tool {
            self.set_tool(tool, editor);
        }
        if keys.enter || (keys.escape && self.pen.is_some()) {
            self.finish_pen(editor);
        } else if keys.escape {
            self.gesture = Gesture::None;
            if self.nodes.is_empty() {
                self.selection.clear();
            } else {
                self.nodes.clear();
            }
        }
        if keys.delete {
            self.delete_selected(editor);
        }
        if keys.nudge != Vec2::ZERO {
            self.nudge(editor, keys.nudge);
        }
        if let Some(order) = keys.order {
            self.reorder(editor, order);
        }
        if keys.copy || keys.cut {
            self.copy(ui.ctx(), editor);
        }
        if keys.cut {
            self.delete_selected(editor);
        }
        if let Some(text) = keys.paste {
            self.paste(editor, &text);
        }
        if keys.duplicate {
            self.duplicate(editor);
        }
    }

    // ---- Operaciones sobre la selección ---------------------------------------------------

    /// Caja envolvente de la geometría de la selección, en coordenadas del documento.
    fn selection_bounds(&self, editor: &Editor) -> Option<Rect> {
        self.selection
            .iter()
            .filter_map(|id| editor.doc().shape(*id))
            .map(Shape::geometry_bounds)
            .reduce(|a, b| a.union(b))
    }

    fn transform_selection(&mut self, editor: &mut Editor, transform: Affine) {
        if self.selection.is_empty() || transform == Affine::IDENTITY {
            return;
        }
        let ids = self.selection.clone();
        self.execute(editor, Command::TransformShapes { ids, transform });
    }

    fn flip(&mut self, editor: &mut Editor, horizontal: bool) {
        let Some(bounds) = self.selection_bounds(editor) else { return };
        let c = bounds.center().to_vec2();
        let (sx, sy) = if horizontal { (-1.0, 1.0) } else { (1.0, -1.0) };
        let transform = Affine::translate(c) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-c);
        self.transform_selection(editor, transform);
    }

    /// Flechas: mueven los nodos seleccionados (Editar nodos) o las formas.
    fn nudge(&mut self, editor: &mut Editor, delta: Vec2) {
        if self.tool == Tool::EditNodes
            && !self.nodes.is_empty()
            && let Some(shape) = self.edit_target(editor)
        {
            let (id, transform) = (shape.id, shape.transform);
            let mut path = shape.path.clone();
            let inverse = transform.inverse();
            for &i in &self.nodes {
                let node = &mut path.nodes[i];
                node.point = inverse * (transform * node.point + delta);
            }
            self.execute(editor, Command::SetShapePath { id, path });
            return;
        }
        self.transform_selection(editor, Affine::translate(delta));
    }

    fn delete_selected(&mut self, editor: &mut Editor) {
        if self.tool == Tool::EditNodes
            && !self.nodes.is_empty()
            && let Some(shape) = self.edit_target(editor)
        {
            let remaining = shape.path.nodes.len() - self.nodes.len();
            if remaining >= 2 {
                let id = shape.id;
                let path = VectorPath {
                    nodes: (shape.path.nodes.iter().enumerate())
                        .filter(|(i, _)| !self.nodes.contains(i))
                        .map(|(_, n)| *n)
                        .collect(),
                    closed: shape.path.closed && remaining > 2,
                };
                self.nodes.clear();
                self.execute(editor, Command::SetShapePath { id, path });
            } else {
                self.messages.push("Un trazado necesita al menos dos nodos".into());
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

    /// Cambia el orden de apilado de las formas seleccionadas dentro de su capa.
    fn reorder(&mut self, editor: &mut Editor, order: Order) {
        let mut commands = Vec::new();
        for layer in &editor.doc().layers {
            let current: Vec<ShapeId> = layer.shapes.iter().map(|s| s.id).collect();
            let selected = |id: &ShapeId| self.selection.contains(id);
            if !current.iter().any(selected) {
                continue;
            }
            let mut target = current.clone();
            match order {
                Order::Front => {
                    target.sort_by_key(|id| selected(id));
                }
                Order::Back => {
                    target.sort_by_key(|id| !selected(id));
                }
                Order::Up => {
                    for i in (0..target.len().saturating_sub(1)).rev() {
                        if selected(&target[i]) && !selected(&target[i + 1]) {
                            target.swap(i, i + 1);
                        }
                    }
                }
                Order::Down => {
                    for i in 1..target.len() {
                        if selected(&target[i]) && !selected(&target[i - 1]) {
                            target.swap(i, i - 1);
                        }
                    }
                }
            }
            // Se simulan los movimientos para emitir solo los necesarios.
            let mut simulated = current;
            for (index, id) in target.iter().enumerate() {
                if simulated[index] != *id {
                    let from = simulated.iter().position(|s| s == id).expect("misma lista");
                    let moved = simulated.remove(from);
                    simulated.insert(index, moved);
                    commands.push(Command::MoveShape {
                        id: *id,
                        layer: layer.id,
                        index,
                    });
                }
            }
        }
        if !commands.is_empty() {
            self.execute(editor, Command::Batch { commands });
        }
    }

    /// Las formas seleccionadas en orden de apilado (de abajo hacia arriba).
    fn selected_shapes(&self, editor: &Editor) -> Vec<Shape> {
        editor
            .doc()
            .layers
            .iter()
            .flat_map(|l| &l.shapes)
            .filter(|s| self.selection.contains(&s.id))
            .cloned()
            .collect()
    }

    fn copy(&mut self, ctx: &egui::Context, editor: &Editor) {
        let shapes = self.selected_shapes(editor);
        if !shapes.is_empty() {
            ctx.copy_text(clipboard::to_text(shapes));
            self.paste_count = 0;
        }
    }

    /// Inserta copias de `shapes` en la capa activa con ids nuevos, desplazadas y seleccionadas.
    fn insert_copies(&mut self, editor: &mut Editor, shapes: Vec<Shape>, offset: Vec2) {
        if !self.active_layer_editable(editor) {
            return;
        }
        let Some(layer) = self.active_layer else { return };
        let mut new_ids = Vec::new();
        let commands = shapes
            .into_iter()
            .map(|mut shape| {
                shape.id = editor.new_shape_id();
                shape.transform = Affine::translate(offset) * shape.transform;
                new_ids.push(shape.id);
                Command::InsertShape {
                    layer,
                    index: None,
                    shape,
                }
            })
            .collect();
        self.execute(editor, Command::Batch { commands });
        self.selection = new_ids;
        self.nodes.clear();
    }

    fn paste(&mut self, editor: &mut Editor, text: &str) {
        let Some(shapes) = clipboard::from_text(text) else { return };
        self.paste_count += 1;
        let offset = Vec2::splat(PASTE_OFFSET * self.paste_count as f64);
        self.insert_copies(editor, shapes, offset);
    }

    fn duplicate(&mut self, editor: &mut Editor) {
        let shapes = self.selected_shapes(editor);
        if !shapes.is_empty() {
            self.insert_copies(editor, shapes, Vec2::splat(PASTE_OFFSET));
        }
    }

    // ---- Overlay --------------------------------------------------------------------------

    /// Prepara lo que se dibuja encima del documento en este frame.
    pub fn prepare_overlay(&self, ui: &egui::Ui, view: &CanvasView) -> DesignOverlay {
        let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|p| view.rect.contains(*p));
        let mut overlay = DesignOverlay {
            moving: self.numeric_preview,
            ..Default::default()
        };

        match &self.gesture {
            Gesture::Box { start, current } => {
                let rect = tools::drag_rect(*start, *current, shift, alt);
                overlay.preview = Some(self.preview(tools::box_path(self.tool, rect)));
            }
            Gesture::Move { start, current } => overlay.moving = Some(Affine::translate(*current - *start)),
            Gesture::Transform {
                handle,
                bounds,
                start,
                current,
            } => overlay.moving = Some(tools::box_transform(*handle, *bounds, *start, *current, shift, alt)),
            Gesture::Marquee { start, current } => overlay.marquee = Some(Rect::from_points(*start, *current)),
            Gesture::None | Gesture::EditPath { .. } => {}
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
            name: String::new(),
            path,
            transform: Affine::IDENTITY,
            style: self.style,
        }
    }

    pub fn overlay<'a>(&'a self, data: &'a DesignOverlay, editor: &'a Editor) -> Overlay<'a> {
        let edited = match &self.gesture {
            Gesture::EditPath { shape, path, .. } => Some((*shape, path)),
            _ => None,
        };

        let nodes = if let Some(pen) = &self.pen {
            Some(NodeOverlay {
                path: &pen.path,
                transform: Affine::IDENTITY,
                selected: &[],
            })
        } else if self.tool == Tool::EditNodes {
            self.edit_target(editor).map(|shape| NodeOverlay {
                path: edited.map_or(&shape.path, |(_, p)| p),
                transform: shape.transform,
                selected: &self.nodes,
            })
        } else {
            None
        };

        let transform_box = (self.tool == Tool::Select)
            .then(|| self.selection_bounds(editor))
            .flatten()
            .map(|b| data.moving.map_or(b, |m| m.transform_rect_bbox(b)));

        Overlay {
            moving: data.moving.map(|m| (self.selection.as_slice(), m)),
            edited,
            preview: data.preview.as_ref(),
            // Con Editar nodos se ven los nodos en vez de la caja de selección.
            selection: if self.tool == Tool::EditNodes { &[] } else { &self.selection },
            guide: data.guide.as_ref(),
            nodes,
            transform_box,
            marquee: data.marquee,
        }
    }
}
