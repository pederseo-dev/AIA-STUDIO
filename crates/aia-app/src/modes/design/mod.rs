mod icons;
mod input;
mod panels;
mod tools;

use std::collections::HashSet;

use aia_core::kurbo::{Affine, Point, Rect, Vec2};
use aia_core::{
    Agent, Command, CurveId, DEFAULT_CURVATURE, EdgeRef, Editor, HandleMode, LayerId, LayerTransform, Mesh, PointId, ShapeId, Style,
};
use aia_render::{Overlay, Wireframe};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::canvas::CanvasView;
pub use tools::Tool;
use tools::{Gesture, ShapeKind, ToolGroup};

/// Mouse hit radius, in screen points.
const HIT_TOLERANCE: f64 = 5.0;
/// Upper limit of the curvature (also used by the curvature field).
const MAX_CURVATURE: f64 = 2.0;
/// Tool buttons per row in the tools panel.
const TOOLS_PER_ROW: usize = 4;

const SELECT_ALL: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::A);

/// Design mode: build the character (vector drawing; later, bones).
/// Everything done here modifies the base.
pub struct DesignMode {
    tool: Tool,
    /// What the Draw Shape tool draws.
    shape_kind: ShapeKind,
    auto_fill: bool,
    auto_stroke: bool,
    /// Dropping a dragged endpoint onto another point welds them (Transform Points, Add Point).
    auto_weld: bool,
    /// <enter> was pressed while dragging: weld on release even without Auto Weld (as in Moho).
    weld_requested: bool,
    /// Add Point creates corners instead of smooth curves.
    sharp_corners: bool,
    show_handles: bool,
    /// Style for new shapes.
    style: Style,
    active_layer: Option<LayerId>,
    /// Selected points of the active layer.
    points: Vec<PointId>,
    /// Selected shapes of the active layer.
    shapes: Vec<ShapeId>,
    gesture: Gesture,
    messages: Vec<String>,
    /// Layer being renamed in the layers panel.
    renaming: Option<(LayerId, String)>,
    /// Groups collapsed in the layers panel.
    collapsed: HashSet<LayerId>,
}

impl Default for DesignMode {
    fn default() -> Self {
        Self {
            tool: Tool::DrawShape,
            shape_kind: ShapeKind::Rectangle,
            auto_fill: true,
            auto_stroke: true,
            auto_weld: true,
            weld_requested: false,
            sharp_corners: false,
            show_handles: true,
            style: Style::default(),
            active_layer: None,
            points: Vec::new(),
            shapes: Vec::new(),
            gesture: Gesture::None,
            messages: Vec::new(),
            renaming: None,
            collapsed: HashSet::new(),
        }
    }
}

/// The active layer as seen from the document.
struct Space {
    layer: LayerId,
    /// From layer space to the document.
    to_doc: Affine,
    /// From the document to layer space.
    to_layer: Affine,
    /// Hit radius in document coordinates.
    doc_tolerance: f64,
    /// Hit radius in layer coordinates.
    tolerance: f64,
}

/// Ids for a new rectangle or oval.
struct BoxIds {
    points: [PointId; 4],
    curve: CurveId,
    shape: ShapeId,
}

impl BoxIds {
    /// Ids that cannot clash with the document's, for the preview.
    fn preview() -> Self {
        Self {
            points: [0, 1, 2, 3].map(|i| PointId(u64::MAX - i)),
            curve: CurveId(u64::MAX - 4),
            shape: ShapeId(u64::MAX - 5),
        }
    }
}

/// What is drawn on top of the document this frame (owned data).
#[derive(Default)]
pub struct DesignOverlay {
    edited_mesh: Option<Mesh>,
    edited_transform: Option<LayerTransform>,
    transform_box: Option<Rect>,
    marquee: Option<Rect>,
    origin: Option<Point>,
    highlight: Vec<EdgeRef>,
    weld_target: Option<PointId>,
}

impl DesignMode {
    /// AI agent that fits the active tool. Once bone tools exist, they will suggest Rigging.
    pub fn suggested_agent(&self) -> Agent {
        Agent::Draw
    }

    /// New messages for the user (errors, warnings).
    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages)
    }

    /// Forgets the state tied to the previous document.
    pub fn reset(&mut self) {
        self.active_layer = None;
        self.points.clear();
        self.shapes.clear();
        self.gesture = Gesture::None;
        self.renaming = None;
    }

    /// Called when leaving the mode: drops whatever was in progress.
    pub fn leave(&mut self, _editor: &mut Editor) {
        self.gesture = Gesture::None;
    }

    /// Drops references to layers, points or shapes that no longer exist (for example, after undo).
    pub fn sync_with_document(&mut self, editor: &Editor) {
        let doc = editor.doc();
        if self.active_layer.and_then(|id| doc.layer(id)).is_none() {
            self.active_layer = doc.layers.last().map(|l| l.id);
            self.gesture = Gesture::None;
        }
        match self.active_mesh(editor) {
            Some(mesh) => {
                self.points.retain(|id| mesh.point(*id).is_some());
                self.shapes.retain(|id| mesh.shape(*id).is_some());
            }
            None => {
                self.points.clear();
                self.shapes.clear();
                // Smart palette: on a group only the layer tools remain.
                if self.tool.needs_vector() {
                    self.set_tool(Tool::TransformLayer);
                }
            }
        }
    }

    fn active_mesh<'a>(&self, editor: &'a Editor) -> Option<&'a Mesh> {
        editor.doc().mesh(self.active_layer?)
    }

    fn set_active_layer(&mut self, id: LayerId) {
        if self.active_layer != Some(id) {
            self.active_layer = Some(id);
            self.points.clear();
            self.shapes.clear();
            self.gesture = Gesture::None;
        }
    }

    fn space(&self, editor: &Editor, view: &CanvasView) -> Option<Space> {
        let layer = self.active_layer?;
        let to_doc = editor.doc().world_transform(layer)?;
        let scale = to_doc.determinant().abs().sqrt().max(1e-9);
        let doc_tolerance = HIT_TOLERANCE / view.camera.zoom;
        Some(Space {
            layer,
            to_doc,
            to_layer: to_doc.inverse(),
            doc_tolerance,
            tolerance: doc_tolerance / scale,
        })
    }

    /// How to use the active tool, for the status bar.
    pub fn tool_hint(&self) -> &'static str {
        self.tool.hint()
    }

    /// Tools panel: one titled section per group with a grid of buttons, as in Moho.
    pub fn tools_panel(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let vector = self.active_mesh(editor).is_some();
        for (i, group) in ToolGroup::ALL.into_iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            ui.label(egui::RichText::new(group.title()).small().strong());
            let tools: Vec<Tool> = Tool::ALL.into_iter().filter(|t| t.group() == group).collect();
            for row in tools.chunks(TOOLS_PER_ROW) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for &tool in row {
                        let enabled = vector || !tool.needs_vector();
                        let response = ui
                            .add_enabled_ui(enabled, |ui| super::tool_button(ui, self.tool == tool, tool.icon()))
                            .inner;
                        let tip = format!("{}  ({})", tool.name(), ui.ctx().format_shortcut(&KeyboardShortcut::new(Modifiers::NONE, tool.shortcut())));
                        let response = response.on_hover_text(&tip).on_disabled_hover_text(format!("{tip}: vector layers only"));
                        if response.clicked() {
                            self.set_tool(tool);
                        }
                    }
                });
            }
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.gesture = Gesture::None;
    }

    fn execute(&mut self, editor: &mut Editor, command: Command) {
        if let Err(e) = editor.execute(command) {
            self.messages.push(e.to_string());
        }
    }

    /// Runs the command merged with the previous one if `merge` repeats its key.
    fn execute_with(&mut self, editor: &mut Editor, command: Command, merge: Option<String>) {
        let result = match merge {
            Some(key) => editor.execute_merged(command, key),
            None => editor.execute(command),
        };
        if let Err(e) = result {
            self.messages.push(e.to_string());
        }
    }

    /// `true` if the active layer (and its parents) is visible and unlocked; otherwise warns.
    fn check_editable(&mut self, editor: &Editor) -> bool {
        let editable = self.active_layer.is_some_and(|id| editor.doc().is_editable(id));
        if !editable {
            self.messages.push("The active layer is hidden or locked".into());
        }
        editable
    }

    fn commit_mesh(&mut self, editor: &mut Editor, mesh: Mesh) {
        let Some(layer) = self.active_layer else { return };
        if editor.doc().mesh(layer) != Some(&mesh) {
            self.execute(editor, Command::ReplaceMesh { layer, mesh });
        }
    }

    // ---- Keyboard -------------------------------------------------------------------------

    /// Mode shortcuts. Ignored while a text field has focus.
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
            select_all: bool,
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
            Keys {
                select_all: i.consume_shortcut(&SELECT_ALL),
                tool: plain.then(|| Tool::ALL.into_iter().find(|t| i.key_pressed(t.shortcut()))).flatten(),
                delete: i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                escape: i.key_pressed(Key::Escape),
                enter: i.key_pressed(Key::Enter),
                nudge,
            }
        });

        if let Some(tool) = keys.tool
            && (self.active_mesh(editor).is_some() || !tool.needs_vector())
        {
            self.set_tool(tool);
        }
        if keys.escape {
            if matches!(self.gesture, Gesture::None) {
                self.points.clear();
                self.shapes.clear();
            }
            self.gesture = Gesture::None;
        }
        if keys.enter {
            match self.gesture {
                Gesture::Points { .. } | Gesture::AddPoint { .. } => self.weld_requested = true,
                _ if self.tool == Tool::CreateShape => self.create_shape(editor),
                _ => {}
            }
        }
        if keys.select_all
            && let Some(mesh) = self.active_mesh(editor)
        {
            if self.tool == Tool::SelectShape {
                self.shapes = mesh.shapes.iter().map(|s| s.id).collect();
            } else {
                self.points = mesh.points().iter().map(|p| p.id).collect();
            }
        }
        if keys.delete {
            if self.tool == Tool::SelectShape {
                self.delete_shapes(editor);
            } else if self.tool.uses_points() {
                self.delete_points(editor);
            }
        }
        if keys.nudge != Vec2::ZERO {
            match self.tool.group() {
                ToolGroup::Layer => self.nudge_layer(editor, keys.nudge),
                _ if self.tool.uses_points() => self.transform_points(editor, Affine::translate(keys.nudge), None),
                _ => {}
            }
        }
    }

    // ---- Operations -----------------------------------------------------------------------

    /// Bounding box of the selected points in document coordinates.
    fn points_bounds(&self, editor: &Editor) -> Option<Rect> {
        let mesh = self.active_mesh(editor)?;
        let to_doc = editor.doc().world_transform(self.active_layer?)?;
        self.points
            .iter()
            .filter_map(|id| mesh.pos(*id))
            .map(|p| Rect::from_points(to_doc * p, to_doc * p))
            .reduce(|a, b| a.union(b))
    }

    /// The active mesh with the selected points transformed by `transform` (in the document).
    fn transformed_mesh(&self, editor: &Editor, transform: Affine) -> Option<Mesh> {
        let mut mesh = self.active_mesh(editor)?.clone();
        let to_doc = editor.doc().world_transform(self.active_layer?)?;
        let local = to_doc.inverse() * transform * to_doc;
        for id in &self.points {
            if let Some(point) = mesh.point_mut(*id) {
                point.pos = local * point.pos;
            }
        }
        Some(mesh)
    }

    fn transform_points(&mut self, editor: &mut Editor, transform: Affine, merge: Option<String>) {
        if self.points.is_empty() || transform == Affine::IDENTITY || !self.check_editable(editor) {
            return;
        }
        let (Some(layer), Some(mesh)) = (self.active_layer, self.transformed_mesh(editor, transform)) else { return };
        let moves = self.points.iter().filter_map(|id| mesh.pos(*id).map(|p| (*id, p))).collect();
        self.execute_with(editor, Command::MovePoints { layer, moves }, merge);
    }

    fn delete_points(&mut self, editor: &mut Editor) {
        if self.points.is_empty() || !self.check_editable(editor) {
            return;
        }
        let Some(mut mesh) = self.active_mesh(editor).cloned() else { return };
        mesh.delete_points(&self.points.drain(..).collect());
        self.commit_mesh(editor, mesh);
    }

    fn delete_shapes(&mut self, editor: &mut Editor) {
        let Some(layer) = self.active_layer else { return };
        if self.shapes.is_empty() || !self.check_editable(editor) {
            return;
        }
        let commands = self.shapes.drain(..).map(|id| Command::RemoveShape { layer, id }).collect();
        self.execute(editor, Command::Batch { commands });
    }

    /// Creates a shape from the edges between the selected points (Create Shape).
    fn create_shape(&mut self, editor: &mut Editor) {
        if !self.check_editable(editor) {
            return;
        }
        let Some(mesh) = self.active_mesh(editor) else { return };
        let Some((fill, outline)) = mesh.shape_from_points(&self.points.iter().copied().collect()) else {
            self.messages.push("Select points joined by curves to create a shape".into());
            return;
        };
        let Some(layer) = self.active_layer else { return };
        let id = editor.new_shape_id();
        let style = Style {
            fill: self.style.fill.filter(|_| !fill.is_empty()),
            stroke: self.style.stroke,
        };
        let shape = aia_core::Shape {
            id,
            name: format!("Shape {}", id.0),
            fill,
            outline,
            style,
        };
        self.execute(editor, Command::InsertShape { layer, index: None, shape });
        self.shapes = vec![id];
    }

    /// Curvature of the curves at the selected points (Peak = 0, Smooth = a circle's).
    fn set_curvature(&mut self, editor: &mut Editor, mode: HandleMode, merge: bool) {
        self.change_handle_modes(editor, merge, |_| mode);
    }

    /// Reset: manual handles at the selected points become automatic again (default shape).
    fn reset_handles(&mut self, editor: &mut Editor) {
        self.change_handle_modes(editor, false, |mode| match mode {
            HandleMode::Auto { .. } => mode,
            _ => HandleMode::SMOOTH,
        });
    }

    fn change_handle_modes(&mut self, editor: &mut Editor, merge: bool, change: impl Fn(HandleMode) -> HandleMode) {
        let (Some(layer), Some(mesh)) = (self.active_layer, self.active_mesh(editor)) else { return };
        let commands: Vec<Command> = mesh
            .curves
            .iter()
            .filter(|c| c.nodes.iter().any(|n| self.points.contains(&n.point)))
            .map(|c| {
                let mut curve = c.clone();
                for node in curve.nodes.iter_mut().filter(|n| self.points.contains(&n.point)) {
                    node.mode = change(node.mode);
                }
                Command::SetCurve { layer, curve }
            })
            .filter(|command| match command {
                Command::SetCurve { curve, .. } => mesh.curve(curve.id) != Some(curve),
                _ => true,
            })
            .collect();
        if commands.is_empty() || !self.check_editable(editor) {
            return;
        }
        let key = merge.then(|| format!("curvature:{layer:?}:{:?}", self.points));
        self.execute_with(editor, Command::Batch { commands }, key);
    }

    /// The active mesh with the curvature at the selected points changed by `delta` (Curvature tool).
    /// Manual handles become automatic, starting from the default curvature.
    fn curved_mesh(&self, editor: &Editor, delta: f64) -> Option<Mesh> {
        let mut mesh = self.active_mesh(editor)?.clone();
        for node in mesh.curves.iter_mut().flat_map(|c| &mut c.nodes).filter(|n| self.points.contains(&n.point)) {
            let base = match node.mode {
                HandleMode::Auto { curvature } => curvature,
                _ => DEFAULT_CURVATURE,
            };
            node.mode = HandleMode::Auto {
                curvature: (base + delta).clamp(0.0, MAX_CURVATURE),
            };
        }
        Some(mesh)
    }

    /// Point that `moving` would weld onto if dropped now, as in Moho: with Auto Weld only an
    /// endpoint of a curve welds; after <enter>, any point. `exclude` is never a target.
    fn weld_target(&self, mesh: &Mesh, moving: PointId, exclude: Option<PointId>, tolerance: f64) -> Option<PointId> {
        let endpoint = mesh.curves.iter().any(|c| c.is_endpoint(moving));
        if !(self.weld_requested || (self.auto_weld && endpoint)) {
            return None;
        }
        let pos = mesh.pos(moving)?;
        mesh.points()
            .iter()
            .filter(|p| p.id != moving && Some(p.id) != exclude)
            .map(|p| (p.id, p.pos.distance(pos)))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// Welds `moving` onto `target`. Curves closed by the weld get Auto Fill / Auto Stroke.
    fn weld_points(&self, editor: &mut Editor, mesh: &mut Mesh, moving: PointId, target: PointId) -> bool {
        let closed_before: Vec<CurveId> = mesh.curves.iter().filter(|c| c.closed).map(|c| c.id).collect();
        if !mesh.weld(moving, target) {
            return false;
        }
        let closed_now: Vec<CurveId> = mesh
            .curves
            .iter()
            .filter(|c| c.closed && !closed_before.contains(&c.id) && c.nodes.iter().any(|n| n.point == target))
            .map(|c| c.id)
            .collect();
        for curve in closed_now {
            let shape = editor.new_shape_id();
            self.auto_shape(mesh, curve, shape);
        }
        true
    }

    /// Mirrors the selected points around their center (Flip Horizontally / Vertically).
    fn flip_points(&mut self, editor: &mut Editor, horizontal: bool) {
        let Some(c) = self.points_bounds(editor).map(|b| b.center().to_vec2()) else { return };
        let (sx, sy) = if horizontal { (-1.0, 1.0) } else { (1.0, -1.0) };
        let flip = Affine::translate(c) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-c);
        self.transform_points(editor, flip, None);
    }

    fn nudge_layer(&mut self, editor: &mut Editor, delta: Vec2) {
        let Some(id) = self.active_layer else { return };
        let (Some(layer), Some(parent)) = (editor.doc().layer(id), editor.doc().parent_transform(id)) else { return };
        let mut transform = layer.transform;
        let to_parent = parent.inverse();
        transform.translation += (to_parent * Point::new(delta.x, delta.y)) - (to_parent * Point::ZERO);
        if self.check_editable(editor) {
            self.execute(editor, Command::SetLayerTransform { id, transform });
        }
    }

    /// Adds a rectangle or oval (in document coordinates) to `mesh`.
    fn add_box(&self, mesh: &mut Mesh, space: &Space, rect: Rect, ids: &BoxIds) {
        let c = rect.center();
        let corners = match self.shape_kind {
            ShapeKind::Rectangle => [(rect.x0, rect.y0), (rect.x1, rect.y0), (rect.x1, rect.y1), (rect.x0, rect.y1)],
            ShapeKind::Oval => [(c.x, rect.y0), (rect.x1, c.y), (c.x, rect.y1), (rect.x0, c.y)],
        };
        let points: Vec<(PointId, Point)> = ids
            .points
            .iter()
            .zip(corners)
            .map(|(id, p)| (*id, space.to_layer * Point::from(p)))
            .collect();
        let mode = match self.shape_kind {
            ShapeKind::Rectangle => HandleMode::PEAK,
            ShapeKind::Oval => HandleMode::SMOOTH,
        };
        mesh.add_closed_curve(ids.curve, &points, mode);
        self.auto_shape(mesh, ids.curve, ids.shape);
    }

    /// Automatic fill and outline according to the tool options.
    fn auto_shape(&self, mesh: &mut Mesh, curve: CurveId, shape: ShapeId) {
        let fill = self.style.fill.filter(|_| self.auto_fill);
        let stroke = self.style.stroke.filter(|_| self.auto_stroke);
        mesh.auto_shape(curve, fill, stroke, shape, format!("Shape {}", shape.0));
    }

    /// Bounding box of a layer's content (including its children) in document coordinates.
    fn layer_bounds(editor: &Editor, id: LayerId) -> Option<Rect> {
        let doc = editor.doc();
        let mut bounds: Option<Rect> = None;
        for layer in doc.all_layers().into_iter().filter(|l| doc.is_inside(l.id, id)) {
            let (Some(mesh), Some(to_doc)) = (layer.mesh(), doc.world_transform(layer.id)) else { continue };
            let rects = mesh
                .points()
                .iter()
                .map(|p| Rect::from_points(to_doc * p.pos, to_doc * p.pos))
                .chain(mesh.shapes.iter().filter_map(|s| mesh.shape_bounds(s)).map(|b| to_doc.transform_rect_bbox(b)));
            for r in rects {
                bounds = Some(bounds.map_or(r, |b| b.union(r)));
            }
        }
        bounds
    }

    // ---- Overlay --------------------------------------------------------------------------

    /// Prepares what is drawn on top of the document this frame.
    pub fn prepare_overlay(&self, ui: &egui::Ui, view: &CanvasView, editor: &Editor) -> DesignOverlay {
        let mut overlay = DesignOverlay::default();
        let Some(space) = self.space(editor, view) else { return overlay };
        let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|p| view.rect.contains(*p));

        match &self.gesture {
            Gesture::Marquee { start, current } => overlay.marquee = Some(Rect::from_points(*start, *current)),
            Gesture::Points {
                drag,
                bounds,
                start,
                current,
            } => {
                let transform = tools::box_transform(*drag, *bounds, *start, *current, shift, alt);
                overlay.edited_mesh = self.transformed_mesh(editor, transform);
                if matches!(drag, tools::Drag::Move)
                    && let ([moving], Some(mesh)) = (self.points.as_slice(), &overlay.edited_mesh)
                {
                    overlay.weld_target = self.weld_target(mesh, *moving, None, space.tolerance);
                }
                overlay.transform_box = (self.points.len() >= 2).then(|| transform.transform_rect_bbox(*bounds));
            }
            Gesture::BezierHandle { mesh, .. } => overlay.edited_mesh = Some(mesh.clone()),
            Gesture::AddPoint { mesh, point, anchor, .. } => {
                overlay.weld_target = self.weld_target(mesh, *point, anchor.map(|(id, _)| id), space.tolerance);
                overlay.edited_mesh = Some(mesh.clone());
            }
            Gesture::Curvature { start, current } => {
                overlay.edited_mesh = self.curved_mesh(editor, tools::curvature_delta(*start, *current, view.camera.zoom));
            }
            Gesture::DrawShape { start, current } => {
                if let Some(mut mesh) = self.active_mesh(editor).cloned() {
                    self.add_box(&mut mesh, &space, tools::drag_rect(*start, *current, shift, alt), &BoxIds::preview());
                    overlay.edited_mesh = Some(mesh);
                }
            }
            Gesture::Layer {
                drag,
                original,
                origin,
                start,
                current,
            } => {
                let to_parent = editor.doc().parent_transform(space.layer).unwrap_or_default().inverse();
                overlay.edited_transform = Some(tools::layer_drag(*drag, *original, *origin, to_parent, *start, *current, shift));
            }
            Gesture::None => {}
        }

        match self.tool {
            Tool::TransformPoints if matches!(self.gesture, Gesture::None) && self.points.len() >= 2 => {
                overlay.transform_box = self.points_bounds(editor);
            }
            Tool::TransformLayer | Tool::SetOrigin => {
                let layer = editor.doc().layer(space.layer);
                let parent = editor.doc().parent_transform(space.layer).unwrap_or_default();
                // With a temporary transform, the box and the origin move with it.
                let delta = match (layer, overlay.edited_transform) {
                    (Some(l), Some(t)) => parent * t.affine(l.origin) * (parent * l.affine()).inverse(),
                    _ => Affine::IDENTITY,
                };
                overlay.origin = layer.map(|l| delta * (space.to_doc * l.origin));
                if self.tool == Tool::TransformLayer {
                    overlay.transform_box = Self::layer_bounds(editor, space.layer).map(|b| delta.transform_rect_bbox(b));
                }
            }
            Tool::CreateShape => {
                if let Some((_, outline)) = self
                    .active_mesh(editor)
                    .and_then(|m| m.shape_from_points(&self.points.iter().copied().collect()))
                {
                    overlay.highlight = outline;
                }
            }
            Tool::DeleteEdge => {
                if let (Some(mesh), Some(hover)) = (self.active_mesh(editor), hover)
                    && let Some((curve, segment, _)) = mesh.nearest_segment(space.to_layer * view.to_doc(hover), space.tolerance)
                {
                    overlay.highlight = vec![EdgeRef::forward(curve, segment)];
                }
            }
            _ => {}
        }
        overlay
    }

    pub fn overlay<'a>(&'a self, data: &'a DesignOverlay, editor: &'a Editor) -> Overlay<'a> {
        let wireframe = self
            .active_layer
            .filter(|id| self.tool.needs_vector() && editor.doc().mesh(*id).is_some())
            .map(|layer| Wireframe {
                layer,
                selected_points: &self.points,
                handles: self.show_handles && matches!(self.tool, Tool::TransformPoints | Tool::Curvature),
                selected_shapes: &self.shapes,
                highlight: &data.highlight,
                weld_target: data.weld_target,
            });
        Overlay {
            edited_mesh: self.active_layer.zip(data.edited_mesh.as_ref()),
            edited_transform: self.active_layer.zip(data.edited_transform),
            wireframe,
            transform_box: data.transform_box,
            marquee: data.marquee,
            origin: data.origin,
        }
    }
}
