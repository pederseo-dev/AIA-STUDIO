//! Mouse input on the canvas for each Design mode tool.

use aia_core::kurbo::{Point, Rect};
use aia_core::{Command, CurveNode, Editor, Handle, HandleMode, MeshPoint, PointId};
use aia_render::{BoxHandle, box_handles};
use eframe::egui;

use super::tools::{self, Drag, Gesture, Tool};
use super::{BoxIds, DesignMode, Space};
use crate::canvas::CanvasView;

/// Hit radius of the transform box handles, in screen points.
const BOX_HANDLE_RADIUS: f64 = 7.0;
/// Minimum drag (in screen points) to create something.
const MIN_DRAG: f64 = 2.0;

/// Selection modifiers: Shift adds, Alt removes.
#[derive(Clone, Copy)]
struct Modifiers {
    shift: bool,
    alt: bool,
}

impl DesignMode {
    pub fn cursor(&self, view: &CanvasView, editor: &Editor, hover: Option<egui::Pos2>) -> egui::CursorIcon {
        if view.panning {
            return egui::CursorIcon::Grab;
        }
        if let Some(pos) = hover
            && let Some(handle) = self.hit_box_handle(view, editor, pos)
        {
            return match handle {
                BoxHandle::Corner(0 | 2) => egui::CursorIcon::ResizeNwSe,
                BoxHandle::Corner(_) => egui::CursorIcon::ResizeNeSw,
                BoxHandle::Edge(0 | 2) => egui::CursorIcon::ResizeVertical,
                BoxHandle::Edge(_) => egui::CursorIcon::ResizeHorizontal,
                BoxHandle::Rotate => egui::CursorIcon::Grab,
            };
        }
        match self.tool {
            Tool::AddPoint | Tool::DrawShape | Tool::SetOrigin => egui::CursorIcon::Crosshair,
            Tool::TransformPoints | Tool::TransformLayer => egui::CursorIcon::Move,
            Tool::Curvature => egui::CursorIcon::ResizeHorizontal,
            _ => egui::CursorIcon::Default,
        }
    }

    pub fn canvas_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, view: &CanvasView, editor: &mut Editor) {
        if view.panning {
            return;
        }
        let Some(space) = self.space(editor, view) else { return };
        let mods = ui.input(|i| Modifiers {
            shift: i.modifiers.shift,
            alt: i.modifiers.alt,
        });
        let press = ui.input(|i| i.pointer.press_origin()).map(|p| view.to_doc(p));
        let pointer = response.interact_pointer_pos().map(|p| view.to_doc(p));
        let started = response.drag_started_by(egui::PointerButton::Primary);
        let dragged = response.dragged_by(egui::PointerButton::Primary);

        // Update the current point of any gesture in progress.
        if dragged && let Some(current) = pointer {
            self.drag_to(current, &space, mods);
        }

        match self.tool {
            Tool::SelectPoints | Tool::CreateShape => {
                if response.clicked()
                    && let Some(p) = pointer
                {
                    self.click_points(editor, &space, p, mods);
                }
                if started && let Some(start) = press {
                    if !mods.shift && !mods.alt {
                        self.points.clear();
                    }
                    self.gesture = Gesture::Marquee { start, current: start };
                }
            }
            Tool::TransformPoints => {
                if started && let Some(start) = press {
                    self.start_points_drag(view, editor, &space, start);
                }
            }
            Tool::AddPoint => {
                if response.clicked()
                    && let Some(p) = pointer
                {
                    self.insert_point_on_curve(editor, &space, p);
                }
                if started && let Some(start) = press {
                    self.start_add_point(editor, &space, start);
                }
            }
            Tool::Curvature => {
                if response.clicked()
                    && let Some(p) = pointer
                {
                    self.click_points(editor, &space, p, mods);
                }
                if started && let Some(start) = press {
                    self.start_curvature(editor, &space, start);
                }
            }
            Tool::DrawShape => {
                if started
                    && let Some(start) = press
                    && self.check_editable(editor)
                {
                    self.gesture = Gesture::DrawShape { start, current: start };
                }
            }
            Tool::DeleteEdge => {
                if response.clicked()
                    && let Some(p) = pointer
                {
                    self.delete_edge_at(editor, &space, p);
                }
            }
            Tool::SelectShape => {
                if response.clicked()
                    && let Some(p) = pointer
                {
                    self.click_shape(editor, &space, p, mods);
                }
            }
            Tool::TransformLayer => {
                if started
                    && let Some(start) = press
                    && self.check_editable(editor)
                {
                    self.start_layer_drag(view, editor, &space, start);
                }
            }
            Tool::SetOrigin => {
                if response.clicked()
                    && let Some(p) = pointer
                {
                    self.set_origin_at(editor, &space, p);
                }
            }
        }

        if response.drag_stopped() {
            self.finish_gesture(editor, view, &space, mods);
        }
    }

    fn drag_to(&mut self, current: Point, space: &Space, mods: Modifiers) {
        match &mut self.gesture {
            Gesture::Marquee { current: c, .. }
            | Gesture::Points { current: c, .. }
            | Gesture::Curvature { current: c, .. }
            | Gesture::DrawShape { current: c, .. }
            | Gesture::Layer { current: c, .. } => *c = current,
            Gesture::BezierHandle { curve, node, handle, mesh } => {
                let local = space.to_layer * current;
                let Some(c) = mesh.curves.iter().position(|c| c.id == *curve) else { return };
                let Some(pos) = mesh.pos(mesh.curves[c].nodes[*node].point) else { return };
                let n = &mut mesh.curves[c].nodes[*node];
                if mods.alt {
                    // Alt breaks the alignment: the handle becomes a corner.
                    n.mode = HandleMode::Corner;
                }
                n.set_handle(*handle, local - pos);
            }
            Gesture::AddPoint { mesh, point, anchor, .. } => {
                let mut local = space.to_layer * current;
                if mods.shift && let Some((_, a)) = anchor {
                    local = tools::snap_45(*a, local);
                }
                if let Some(p) = mesh.point_mut(*point) {
                    p.pos = local;
                }
            }
            Gesture::None => {}
        }
    }

    fn finish_gesture(&mut self, editor: &mut Editor, view: &CanvasView, space: &Space, mods: Modifiers) {
        let gesture = std::mem::take(&mut self.gesture);
        self.finish(gesture, editor, view, space, mods);
        self.weld_requested = false;
    }

    fn finish(&mut self, gesture: Gesture, editor: &mut Editor, view: &CanvasView, space: &Space, mods: Modifiers) {
        match gesture {
            Gesture::Marquee { start, current } => {
                let rect = Rect::from_points(start, current);
                let Some(mesh) = self.active_mesh(editor) else { return };
                let hits: Vec<PointId> = mesh
                    .points()
                    .iter()
                    .filter(|p| rect.contains(space.to_doc * p.pos))
                    .map(|p| p.id)
                    .collect();
                self.apply_point_selection(hits, mods, true);
            }
            Gesture::Points {
                drag,
                bounds,
                start,
                current,
            } => {
                let transform = tools::box_transform(drag, bounds, start, current, mods.shift, mods.alt);
                // Dropping a single point onto another one welds them (Auto Weld or <enter>).
                if drag == Drag::Move
                    && let [moving] = self.points[..]
                    && let Some(mut mesh) = self.transformed_mesh(editor, transform)
                    && let Some(target) = self.weld_target(&mesh, moving, None, space.tolerance)
                    && self.check_editable(editor)
                    && self.weld_points(editor, &mut mesh, moving, target)
                {
                    self.commit_mesh(editor, mesh);
                    self.points = vec![target];
                    return;
                }
                self.transform_points(editor, transform, None);
            }
            Gesture::BezierHandle { curve, mesh, .. } => {
                if let (Some(layer), Some(curve)) = (self.active_layer, mesh.curve(curve).cloned()) {
                    self.execute(editor, Command::SetCurve { layer, curve });
                }
            }
            Gesture::Curvature { start, current } => {
                if let Some(mesh) = self.curved_mesh(editor, tools::curvature_delta(start, current, view.camera.zoom)) {
                    self.commit_mesh(editor, mesh);
                }
            }
            Gesture::AddPoint {
                mut mesh,
                point,
                curve,
                anchor,
            } => {
                let Some(pos) = mesh.pos(point) else { return };
                // A click without dragging from an end or in empty space creates nothing.
                if let Some((_, a)) = anchor
                    && (space.to_doc * pos).distance(space.to_doc * a) * view.camera.zoom < MIN_DRAG
                {
                    return;
                }
                // Auto Weld: dropping onto another point joins it.
                let target = self.weld_target(&mesh, point, anchor.map(|(id, _)| id), space.tolerance);
                let selected = match target {
                    Some(t) if self.weld_points(editor, &mut mesh, point, t) => t,
                    _ => point,
                };
                let shape = editor.new_shape_id();
                self.auto_shape(&mut mesh, curve, shape);
                let created = mesh.shape(shape).is_some();
                self.commit_mesh(editor, mesh);
                self.points = vec![selected];
                if created {
                    self.shapes = vec![shape];
                }
            }
            Gesture::DrawShape { start, current } => {
                let rect = tools::drag_rect(start, current, mods.shift, mods.alt);
                let min = MIN_DRAG / view.camera.zoom;
                if rect.width() < min || rect.height() < min {
                    return;
                }
                let Some(mut mesh) = self.active_mesh(editor).cloned() else { return };
                let ids = BoxIds {
                    points: [(); 4].map(|_| editor.new_point_id()),
                    curve: editor.new_curve_id(),
                    shape: editor.new_shape_id(),
                };
                self.add_box(&mut mesh, space, rect, &ids);
                let created = mesh.shape(ids.shape).is_some();
                self.commit_mesh(editor, mesh);
                self.points = ids.points.to_vec();
                self.shapes = if created { vec![ids.shape] } else { Vec::new() };
            }
            Gesture::Layer {
                drag,
                original,
                origin,
                start,
                current,
            } => {
                let to_parent = editor.doc().parent_transform(space.layer).unwrap_or_default().inverse();
                let transform = tools::layer_drag(drag, original, origin, to_parent, start, current, mods.shift);
                if transform != original {
                    self.execute(editor, Command::SetLayerTransform { id: space.layer, transform });
                }
            }
            Gesture::None => {}
        }
    }

    // ---- Point selection --------------------------------------------------------------

    /// Click: a point, the whole curve under the mouse, or all the points of a fill.
    fn click_points(&mut self, editor: &Editor, space: &Space, p: Point, mods: Modifiers) {
        let Some(mesh) = self.active_mesh(editor) else { return };
        let local = space.to_layer * p;
        let hits = if let Some(point) = mesh.nearest_point(local, space.tolerance) {
            vec![point]
        } else if let Some((curve, _, _)) = mesh.nearest_segment(local, space.tolerance) {
            mesh.curve(curve).map(|c| c.nodes.iter().map(|n| n.point).collect()).unwrap_or_default()
        } else if let Some(shape) = mesh.shape_at(local, space.tolerance).and_then(|id| mesh.shape(id)) {
            mesh.shape_points(shape)
        } else {
            Vec::new()
        };
        self.apply_point_selection(hits, mods, false);
    }

    /// Shift adds, Alt removes, no modifier replaces. `keep` = do not clear first (the marquee already did).
    fn apply_point_selection(&mut self, hits: Vec<PointId>, mods: Modifiers, keep: bool) {
        if mods.alt {
            self.points.retain(|p| !hits.contains(p));
            return;
        }
        if !mods.shift && !keep {
            self.points.clear();
        }
        for id in hits {
            if !self.points.contains(&id) {
                self.points.push(id);
            }
        }
    }

    // ---- Transform points ---------------------------------------------------------------

    /// Transform box handle (of the points or the layer) under `pos`.
    fn hit_box_handle(&self, view: &CanvasView, editor: &Editor, pos: egui::Pos2) -> Option<BoxHandle> {
        let bounds = match self.tool {
            Tool::TransformPoints if self.points.len() >= 2 => self.points_bounds(editor)?,
            Tool::TransformLayer => Self::layer_bounds(editor, self.active_layer?)?,
            _ => return None,
        };
        let screen = view.camera.doc_to_screen().transform_rect_bbox(bounds);
        let local = view.to_local(pos);
        box_handles(screen)
            .into_iter()
            .find(|(_, p)| p.distance(local) <= BOX_HANDLE_RADIUS)
            .map(|(handle, _)| handle)
    }

    /// Visible Bézier handle under `p`: (curve, node, which).
    fn hit_bezier_handle(&self, editor: &Editor, space: &Space, p: Point) -> Option<(usize, usize, Handle)> {
        let mesh = self.active_mesh(editor)?;
        for (ci, curve) in mesh.curves.iter().enumerate() {
            for (ni, node) in curve.nodes.iter().enumerate() {
                if !self.points.contains(&node.point) {
                    continue;
                }
                let pos = mesh.pos(node.point)?;
                let (h_in, h_out) = mesh.handles(curve, ni);
                for (handle, offset) in [(Handle::Out, h_out), (Handle::In, h_in)] {
                    if offset.hypot() > 0.0 && (space.to_doc * (pos + offset)).distance(p) <= space.doc_tolerance {
                        return Some((ci, ni, handle));
                    }
                }
            }
        }
        None
    }

    fn start_points_drag(&mut self, view: &CanvasView, editor: &Editor, space: &Space, start: Point) {
        if !self.check_editable(editor) {
            return;
        }
        let origin = view.camera.doc_to_screen() * start;
        let origin = egui::pos2(origin.x as f32 + view.rect.min.x, origin.y as f32 + view.rect.min.y);
        if let Some(handle) = self.hit_box_handle(view, editor, origin)
            && let Some(bounds) = self.points_bounds(editor)
        {
            self.gesture = Gesture::Points {
                drag: Drag::Handle(handle),
                bounds,
                start,
                current: start,
            };
            return;
        }
        if self.start_bezier_drag(editor, space, start) {
            return;
        }
        self.pick_points_to_move(editor, space, start);
        if let Some(bounds) = self.points_bounds(editor) {
            self.gesture = Gesture::Points {
                drag: Drag::Move,
                bounds,
                start,
                current: start,
            };
        }
    }

    /// What Transform Points moves: the point under the mouse; the whole shape when grabbed from
    /// inside its fill; otherwise the selection, or the nearest point with fewer than two selected
    /// (as in Moho).
    fn pick_points_to_move(&mut self, editor: &Editor, space: &Space, start: Point) {
        let Some(mesh) = self.active_mesh(editor) else { return };
        let local = space.to_layer * start;
        if let Some(point) = mesh.nearest_point(local, space.tolerance) {
            if !self.points.contains(&point) {
                self.points = vec![point];
            }
        } else if let Some(shape) = mesh.shape_at(local, space.tolerance).and_then(|id| mesh.shape(id)) {
            let points = mesh.shape_points(shape);
            if !points.iter().all(|p| self.points.contains(p)) {
                self.points = points;
            }
        } else if self.points.len() < 2 {
            self.select_nearest_point(editor, space, start);
        }
    }

    /// Starts dragging the visible Bezier handle under `start`, if there is one.
    fn start_bezier_drag(&mut self, editor: &Editor, space: &Space, start: Point) -> bool {
        if self.show_handles
            && let Some((ci, ni, handle)) = self.hit_bezier_handle(editor, space, start)
            && let Some(mut mesh) = self.active_mesh(editor).cloned()
        {
            // An auto handle becomes manual (smooth) keeping its current shape.
            let (h_in, h_out) = mesh.handles(&mesh.curves[ci], ni);
            let node = &mut mesh.curves[ci].nodes[ni];
            if let HandleMode::Auto { .. } = node.mode {
                node.mode = HandleMode::Smooth;
                node.handle_in = h_in;
                node.handle_out = h_out;
            }
            self.gesture = Gesture::BezierHandle {
                curve: mesh.curves[ci].id,
                node: ni,
                handle,
                mesh,
            };
            return true;
        }
        false
    }

    /// Selects only the point closest to `p` (at any distance).
    fn select_nearest_point(&mut self, editor: &Editor, space: &Space, p: Point) {
        let Some(mesh) = self.active_mesh(editor) else { return };
        let local = space.to_layer * p;
        let nearest = mesh
            .points()
            .iter()
            .min_by(|a, b| a.pos.distance(local).total_cmp(&b.pos.distance(local)))
            .map(|p| p.id);
        self.points = nearest.into_iter().collect();
    }

    // ---- Curvature ----------------------------------------------------------------------

    /// Dragging a visible Bezier handle changes its angle and length (as in Moho); anywhere else,
    /// the curvature. Without a selection, the point nearest the mouse is the one that changes.
    fn start_curvature(&mut self, editor: &Editor, space: &Space, start: Point) {
        if !self.check_editable(editor) || self.start_bezier_drag(editor, space, start) {
            return;
        }
        if self.points.is_empty() {
            self.select_nearest_point(editor, space, start);
        }
        if !self.points.is_empty() {
            self.gesture = Gesture::Curvature { start, current: start };
        }
    }

    // ---- Add point --------------------------------------------------------------------

    /// Click on a curve: inserts a point there.
    fn insert_point_on_curve(&mut self, editor: &mut Editor, space: &Space, p: Point) {
        let Some(mut mesh) = self.active_mesh(editor).cloned() else { return };
        let local = space.to_layer * p;
        if mesh.nearest_point(local, space.tolerance).is_some() {
            return;
        }
        let Some((curve, segment, t)) = mesh.nearest_segment(local, space.tolerance) else { return };
        if !self.check_editable(editor) {
            return;
        }
        let id = editor.new_point_id();
        if mesh.insert_point(curve, segment, t, id) {
            self.commit_mesh(editor, mesh);
            self.points = vec![id];
        }
    }

    fn start_add_point(&mut self, editor: &mut Editor, space: &Space, start: Point) {
        if !self.check_editable(editor) {
            return;
        }
        let Some(mut mesh) = self.active_mesh(editor).cloned() else { return };
        let local = space.to_layer * start;
        let mode = if self.sharp_corners { HandleMode::PEAK } else { HandleMode::SMOOTH };
        let new = editor.new_point_id();

        let (curve, anchor) = if let Some(existing) = mesh.nearest_point(local, space.tolerance) {
            let pos = mesh.pos(existing).unwrap_or(local);
            mesh.add_point(MeshPoint::new(new, pos));
            let node = CurveNode::new(new, mode);
            // From the end of an open curve: extends it. From any other point: a welded branch.
            let curve = if let Some(c) = mesh.curves.iter_mut().find(|c| c.is_endpoint(existing)) {
                if c.nodes.last().map(|n| n.point) == Some(existing) {
                    c.nodes.push(node);
                } else {
                    c.nodes.insert(0, node);
                }
                c.id
            } else {
                let id = editor.new_curve_id();
                mesh.curves.push(aia_core::Curve {
                    id,
                    closed: false,
                    nodes: vec![CurveNode::new(existing, mode), node],
                });
                id
            };
            (curve, Some((existing, pos)))
        } else if let Some((curve, segment, t)) = mesh.nearest_segment(local, space.tolerance) {
            // On a curve: insert the point and drag it.
            if !mesh.insert_point(curve, segment, t, new) {
                return;
            }
            (curve, None)
        } else {
            // In empty space: a new two-point curve.
            let first = editor.new_point_id();
            mesh.add_point(MeshPoint::new(first, local));
            mesh.add_point(MeshPoint::new(new, local));
            let id = editor.new_curve_id();
            mesh.curves.push(aia_core::Curve {
                id,
                closed: false,
                nodes: vec![CurveNode::new(first, mode), CurveNode::new(new, mode)],
            });
            (id, Some((first, local)))
        };
        self.points = vec![new];
        self.gesture = Gesture::AddPoint {
            mesh,
            point: new,
            curve,
            anchor,
        };
    }

    // ---- Delete edge, shapes, layer -------------------------------------------------------

    fn delete_edge_at(&mut self, editor: &mut Editor, space: &Space, p: Point) {
        let Some(mut mesh) = self.active_mesh(editor).cloned() else { return };
        let Some((curve, segment, _)) = mesh.nearest_segment(space.to_layer * p, space.tolerance) else { return };
        if !self.check_editable(editor) {
            return;
        }
        let new_curve = editor.new_curve_id();
        if mesh.delete_edge(curve, segment, new_curve) {
            self.commit_mesh(editor, mesh);
        }
    }

    fn click_shape(&mut self, editor: &Editor, space: &Space, p: Point, mods: Modifiers) {
        let Some(mesh) = self.active_mesh(editor) else { return };
        let hit = mesh.shape_at(space.to_layer * p, space.tolerance);
        match hit {
            Some(id) if mods.alt => self.shapes.retain(|s| *s != id),
            Some(id) if mods.shift => {
                if !self.shapes.contains(&id) {
                    self.shapes.push(id);
                }
            }
            Some(id) => self.shapes = vec![id],
            None if !mods.shift && !mods.alt => self.shapes.clear(),
            None => {}
        }
    }

    fn start_layer_drag(&mut self, view: &CanvasView, editor: &Editor, space: &Space, start: Point) {
        let Some(layer) = editor.doc().layer(space.layer) else { return };
        let screen = view.camera.doc_to_screen() * start;
        let pos = egui::pos2(screen.x as f32 + view.rect.min.x, screen.y as f32 + view.rect.min.y);
        let drag = self.hit_box_handle(view, editor, pos).map_or(Drag::Move, Drag::Handle);
        self.gesture = Gesture::Layer {
            drag,
            original: layer.transform,
            origin: space.to_doc * layer.origin,
            start,
            current: start,
        };
    }

    /// The new origin goes under the mouse; the layer does not move on screen.
    fn set_origin_at(&mut self, editor: &mut Editor, space: &Space, p: Point) {
        let Some(layer) = editor.doc().layer(space.layer) else { return };
        let origin = space.to_layer * p;
        let transform = layer.transform.with_origin_moved(layer.origin, origin);
        let id = space.layer;
        if self.check_editable(editor) {
            self.execute(
                editor,
                Command::Batch {
                    commands: vec![Command::SetLayerOrigin { id, origin }, Command::SetLayerTransform { id, transform }],
                },
            );
        }
    }
}
