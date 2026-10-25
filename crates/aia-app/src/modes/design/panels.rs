//! Design mode panels: the tool options bar (above the canvas, as in Moho), Style and Layers.

use aia_core::kurbo::{Affine, Point};
use aia_core::{Color, Command, Document, Editor, HandleMode, Layer, LayerId, LayerTransform, Style};
use eframe::egui;
use egui_phosphor::regular as icons;

use super::DesignMode;
use super::tools::{ShapeKind, Tool};

/// Something requested in the layers panel; applied after drawing it.
enum LayerAction {
    Select(LayerId),
    New { group: bool },
    Delete(LayerId),
    Raise(LayerId),
    Lower(LayerId),
    Group(LayerId),
    Ungroup(LayerId),
    MoveInto(LayerId, Option<LayerId>),
    Rename(LayerId, String),
    Run(Command),
}

/// Tracks whether a continuous edit (dragging or typing a value) has just finished.
#[derive(Default)]
struct Release(bool);

impl Release {
    fn track(&mut self, response: &egui::Response) {
        self.0 |= response.drag_stopped() || response.lost_focus();
    }
}

/// A labelled numeric field for the options bar.
fn number(ui: &mut egui::Ui, label: &str, value: &mut f64, speed: f64, suffix: &str, release: &mut Release) -> bool {
    ui.label(label);
    let response = ui.add(
        egui::DragValue::new(value)
            .speed(speed)
            .max_decimals(2)
            .suffix(suffix)
            .min_decimals(if suffix.is_empty() { 0 } else { 1 }),
    );
    release.track(&response);
    response.changed()
}

impl DesignMode {
    // ---- Options bar ----------------------------------------------------------------------

    /// Horizontal bar with the active tool's options, above the canvas.
    pub fn options_bar(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        ui.horizontal(|ui| {
            self.tool.icon().show(ui, 16.0);
            ui.label(egui::RichText::new(self.tool.name()).strong());
            ui.separator();
            match self.tool {
                Tool::SelectPoints => {
                    if ui.button("Select All").on_hover_text("Ctrl+A").clicked()
                        && let Some(mesh) = self.active_mesh(editor)
                    {
                        self.points = mesh.points().iter().map(|p| p.id).collect();
                    }
                }
                Tool::TransformPoints => {
                    self.points_position(ui, editor);
                    ui.separator();
                    self.weld_options(ui);
                    self.handles_option(ui);
                    ui.separator();
                    let enabled = self.points.len() >= 2;
                    if ui.add_enabled(enabled, egui::Button::new("Flip H")).on_hover_text("Flip Horizontally").clicked() {
                        self.flip_points(editor, true);
                    }
                    if ui.add_enabled(enabled, egui::Button::new("Flip V")).on_hover_text("Flip Vertically").clicked() {
                        self.flip_points(editor, false);
                    }
                }
                Tool::Curvature => self.handles_option(ui),
                Tool::AddPoint => {
                    self.weld_options(ui);
                    ui.checkbox(&mut self.sharp_corners, "Sharp Corners");
                    self.handles_option(ui);
                }
                Tool::DrawShape => {
                    for kind in ShapeKind::ALL {
                        ui.selectable_value(&mut self.shape_kind, kind, kind.name());
                    }
                    ui.separator();
                    ui.checkbox(&mut self.auto_fill, "Auto Fill");
                    ui.checkbox(&mut self.auto_stroke, "Auto Stroke");
                }
                Tool::DeleteEdge => {
                    ui.weak("Click a curve segment");
                }
                Tool::SelectShape => self.shape_order(ui, editor),
                Tool::CreateShape => {
                    let enabled = self.points.len() >= 2;
                    if ui.add_enabled(enabled, egui::Button::new("Create Shape")).on_hover_text("Enter").clicked() {
                        self.create_shape(editor);
                    }
                }
                Tool::TransformLayer => self.layer_transform_fields(ui, editor),
                Tool::SetOrigin => self.origin_fields(ui, editor),
            }
            if self.tool.uses_points() && !self.points.is_empty() {
                ui.separator();
                self.points_options(ui, editor);
            }
        });
    }

    /// Auto Weld, Auto Fill and Auto Stroke, as in Moho's point tools.
    fn weld_options(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.auto_weld, "Auto Weld")
            .on_hover_text("Dropping an endpoint onto another point welds them (<enter> while dragging welds any point)");
        ui.checkbox(&mut self.auto_fill, "Auto Fill");
        ui.checkbox(&mut self.auto_stroke, "Auto Stroke");
    }

    fn handles_option(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.show_handles, "Show Bezier Handles")
            .on_hover_text("Drag a handle to reshape the curve; <alt> = corner");
    }

    /// Position of the selected points (center of their bounding box).
    fn points_position(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let Some(bounds) = self.points_bounds(editor) else {
            ui.weak("No points selected");
            return;
        };
        let center = bounds.center();
        let mut target = center;
        let mut release = Release::default();
        let changed_x = number(ui, "Position  X:", &mut target.x, 1.0, "", &mut release);
        let changed_y = number(ui, "Y:", &mut target.y, 1.0, "", &mut release);
        if changed_x || changed_y {
            let key = format!("points_position:{:?}", self.points);
            self.transform_points(editor, Affine::translate(target - center), Some(key));
        }
        if release.0 {
            editor.end_merge();
        }
    }

    /// Curvature and deletion of the selected points.
    fn points_options(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let count = self.points.len();
        ui.label(if count == 1 { "1 point".into() } else { format!("{count} points") });
        let current = self
            .active_mesh(editor)
            .and_then(|m| m.curves.iter().flat_map(|c| &c.nodes).find(|n| self.points.contains(&n.point)))
            .map(|n| n.mode);
        if let Some(HandleMode::Auto { curvature }) = current {
            let mut value = curvature;
            let response = ui
                .add(egui::DragValue::new(&mut value).range(0.0..=super::MAX_CURVATURE).speed(0.01).prefix("Curvature: "))
                .on_hover_text("0 = peak");
            if response.changed() {
                self.set_curvature(editor, HandleMode::Auto { curvature: value }, true);
            }
            if response.drag_stopped() || response.lost_focus() {
                editor.end_merge();
            }
        } else if current.is_some() {
            ui.weak("Manual handles");
        }
        if ui.button("Peak").on_hover_text("Sharp corner (curvature 0)").clicked() {
            self.set_curvature(editor, HandleMode::PEAK, false);
        }
        if ui.button("Smooth").on_hover_text("Default curvature (like a circle)").clicked() {
            self.set_curvature(editor, HandleMode::SMOOTH, false);
        }
        if ui.button("Reset").on_hover_text("Return manual Bezier handles to their automatic shape").clicked() {
            self.reset_handles(editor);
        }
        if ui.button(icons::TRASH).on_hover_text("Delete points (Del)").clicked() {
            self.delete_points(editor);
        }
    }

    /// Stacking order and deletion of the selected shape.
    fn shape_order(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let (Some(layer), Some(mesh)) = (self.active_layer, self.active_mesh(editor)) else { return };
        let Some(&id) = self.shapes.first() else {
            ui.weak("Click a shape to select it");
            return;
        };
        let (Some(index), last) = (mesh.shape_index(id), mesh.shapes.len().saturating_sub(1)) else { return };
        let mut target = None;
        for (icon, tip, to, enabled) in [
            (icons::ARROW_LINE_UP, "Raise to Front", last, index < last),
            (icons::ARROW_UP, "Raise Shape", index + 1, index < last),
            (icons::ARROW_DOWN, "Lower Shape", index.saturating_sub(1), index > 0),
            (icons::ARROW_LINE_DOWN, "Lower to Back", 0, index > 0),
        ] {
            if ui.add_enabled(enabled, egui::Button::new(icon)).on_hover_text(tip).clicked() {
                target = Some(to);
            }
        }
        if let Some(index) = target {
            self.execute(editor, Command::MoveShape { layer, id, index });
        }
        if ui.button(icons::TRASH).on_hover_text("Delete shapes (Del)").clicked() {
            self.delete_shapes(editor);
        }
    }

    /// Position, scale and angle of the active layer, each with its Reset (as in Moho).
    fn layer_transform_fields(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let Some(id) = self.active_layer else { return };
        let Some(layer) = editor.doc().layer(id) else { return };
        let original = layer.transform;
        let defaults = LayerTransform::default();
        let mut t = original;
        let mut release = Release::default();

        number(ui, "Position  X:", &mut t.translation.x, 1.0, "", &mut release);
        number(ui, "Y:", &mut t.translation.y, 1.0, "", &mut release);
        if ui.button("Reset").clicked() {
            t.translation = defaults.translation;
            release.0 = true;
        }
        ui.separator();
        let mut scale = t.scale * 100.0;
        number(ui, "Scale  X:", &mut scale.x, 1.0, "%", &mut release);
        number(ui, "Y:", &mut scale.y, 1.0, "%", &mut release);
        t.scale = scale / 100.0;
        if ui.button("Reset").clicked() {
            t.scale = defaults.scale;
            release.0 = true;
        }
        ui.separator();
        let mut degrees = t.rotation.to_degrees();
        if number(ui, "Angle:", &mut degrees, 0.5, "°", &mut release) {
            t.rotation = degrees.to_radians();
        }
        if ui.button("Reset").clicked() {
            t.rotation = defaults.rotation;
            release.0 = true;
        }
        ui.separator();
        release.0 |= ui.toggle_value(&mut t.flip_h, icons::FLIP_HORIZONTAL).on_hover_text("Flip Horizontally").changed();
        release.0 |= ui.toggle_value(&mut t.flip_v, icons::FLIP_VERTICAL).on_hover_text("Flip Vertically").changed();

        if t != original {
            self.execute_with(editor, Command::SetLayerTransform { id, transform: t }, Some(format!("transform:{id:?}")));
        }
        if release.0 {
            editor.end_merge();
        }
    }

    /// The layer origin; changing it does not move the layer.
    fn origin_fields(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let Some(id) = self.active_layer else { return };
        let Some(layer) = editor.doc().layer(id) else { return };
        let (old, transform) = (layer.origin, layer.transform);
        let mut origin = old;
        let mut release = Release::default();
        number(ui, "Origin  X:", &mut origin.x, 1.0, "", &mut release);
        number(ui, "Y:", &mut origin.y, 1.0, "", &mut release);
        if ui.button("Reset").clicked() {
            origin = Point::ZERO;
            release.0 = true;
        }
        if origin != old {
            let transform = transform.with_origin_moved(old, origin);
            let commands = vec![Command::SetLayerOrigin { id, origin }, Command::SetLayerTransform { id, transform }];
            self.execute_with(editor, Command::Batch { commands }, Some(format!("origin:{id:?}")));
        }
        if release.0 {
            editor.end_merge();
        }
    }

    // ---- Style ----------------------------------------------------------------------------

    /// With shapes selected it edits those shapes (with any tool); otherwise the defaults for
    /// new shapes, like Moho's Style window. Changes are live and undone at once.
    pub fn style_panel(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let layer = self.active_layer;
        let first = self
            .shapes
            .first()
            .and_then(|id| self.active_mesh(editor)?.shape(*id))
            .map(|s| (s.id, s.style, s.name.clone()));

        ui.horizontal(|ui| {
            let mode = match &first {
                None => "DEFAULTS (for new shapes)".to_string(),
                Some((_, _, name)) if self.shapes.len() == 1 => format!("SHAPE: {name}"),
                Some(_) => format!("{} SHAPES", self.shapes.len()),
            };
            ui.label(egui::RichText::new(mode).small().strong());
            if first.is_some() {
                let deselect = ui.small_button("Deselect").on_hover_text("Edit the defaults for new shapes (Esc)");
                if deselect.clicked() {
                    self.shapes.clear();
                }
            }
        });

        egui::Grid::new("style_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            if let (Some((id, _, name)), Some(layer)) = (&first, layer)
                && self.shapes.len() == 1
            {
                ui.label("Name");
                let mut name = name.clone();
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0));
                if r.changed() && !name.trim().is_empty() {
                    let command = Command::RenameShape { layer, id: *id, name: name.clone() };
                    self.execute_with(editor, command, Some(format!("name:{id:?}")));
                }
                if r.lost_focus() {
                    editor.end_merge();
                }
                ui.end_row();
            }

            let mut style = first.as_ref().map_or(self.style, |(_, style, _)| *style);
            let before = style;
            let released = style_rows(ui, &mut style);
            if style != before {
                match (first.is_some(), layer) {
                    (true, Some(layer)) => self.apply_style(editor, layer, before, style),
                    _ => self.style = style,
                }
            }
            if released {
                editor.end_merge();
            }
        });
    }

    /// Applies to the selected shapes only the part of the style that changed (fill or stroke).
    /// The change also becomes the default for new shapes, as in Moho.
    fn apply_style(&mut self, editor: &mut Editor, layer: LayerId, before: Style, after: Style) {
        if after.fill != before.fill {
            self.style.fill = after.fill;
        }
        if after.stroke != before.stroke {
            self.style.stroke = after.stroke;
        }
        let Some(mesh) = self.active_mesh(editor) else { return };
        let commands: Vec<Command> = self
            .shapes
            .iter()
            .filter_map(|id| mesh.shape(*id))
            .map(|s| {
                let mut style = s.style;
                if after.fill != before.fill {
                    style.fill = after.fill;
                }
                if after.stroke != before.stroke {
                    style.stroke = after.stroke;
                }
                Command::SetShapeStyle { layer, id: s.id, style }
            })
            .collect();
        let key = format!("style:{:?}", self.shapes);
        self.execute_with(editor, Command::Batch { commands }, Some(key));
    }

    // ---- Layers ---------------------------------------------------------------------------

    pub fn layers_panel(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let mut actions = Vec::new();
        let active = self.active_layer;
        let located = active.and_then(|id| editor.doc().locate_layer(id));
        let siblings = located.and_then(|(p, _)| editor.doc().siblings(p)).map_or(0, <[Layer]>::len);
        ui.horizontal(|ui| {
            if ui.button(icons::FILE_PLUS).on_hover_text("New Vector Layer").clicked() {
                actions.push(LayerAction::New { group: false });
            }
            if ui.button(icons::FOLDER_PLUS).on_hover_text("New Group").clicked() {
                actions.push(LayerAction::New { group: true });
            }
            if let Some(id) = active {
                // The document needs at least one top-level layer.
                let can_delete = !(located.is_some_and(|(p, _)| p.is_none()) && editor.doc().layers.len() == 1);
                if ui.add_enabled(can_delete, egui::Button::new(icons::TRASH)).on_hover_text("Delete Layer").clicked() {
                    actions.push(LayerAction::Delete(id));
                }
                let index = located.map_or(0, |(_, i)| i);
                if ui.add_enabled(index + 1 < siblings, egui::Button::new(icons::ARROW_UP)).on_hover_text("Raise Layer").clicked() {
                    actions.push(LayerAction::Raise(id));
                }
                if ui.add_enabled(index > 0, egui::Button::new(icons::ARROW_DOWN)).on_hover_text("Lower Layer").clicked() {
                    actions.push(LayerAction::Lower(id));
                }
            }
        });
        ui.separator();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let doc = editor.doc();
            self.layer_rows(ui, doc, &doc.layers, 0, &mut actions);
        });
        for action in actions {
            self.apply_layer_action(editor, action);
        }
    }

    /// Tree rows, top to bottom (as in Moho).
    fn layer_rows(&mut self, ui: &mut egui::Ui, doc: &Document, layers: &[Layer], depth: usize, actions: &mut Vec<LayerAction>) {
        for layer in layers.iter().rev() {
            ui.horizontal(|ui| {
                let (eye, tip) = if layer.visible { (icons::EYE, "Hide") } else { (icons::EYE_SLASH, "Show") };
                if ui.add(egui::Button::new(eye).frame(false)).on_hover_text(tip).clicked() {
                    actions.push(LayerAction::Run(Command::SetLayerVisible {
                        id: layer.id,
                        visible: !layer.visible,
                    }));
                }
                let (lock, tip) = if layer.locked {
                    (icons::LOCK_SIMPLE, "Unlock")
                } else {
                    (icons::LOCK_SIMPLE_OPEN, "Lock")
                };
                if ui.add(egui::Button::new(lock).frame(false)).on_hover_text(tip).clicked() {
                    actions.push(LayerAction::Run(Command::SetLayerLocked {
                        id: layer.id,
                        locked: !layer.locked,
                    }));
                }
                ui.add_space(depth as f32 * 14.0);
                if layer.is_group() {
                    let collapsed = self.collapsed.contains(&layer.id);
                    let caret = if collapsed { icons::CARET_RIGHT } else { icons::CARET_DOWN };
                    if ui.add(egui::Button::new(caret).frame(false)).clicked() && !self.collapsed.remove(&layer.id) {
                        self.collapsed.insert(layer.id);
                    }
                } else {
                    ui.add_space(18.0);
                }
                ui.label(if layer.is_group() { icons::FOLDER } else { icons::VECTOR_TWO });

                if let Some((id, name)) = &mut self.renaming
                    && *id == layer.id
                {
                    if let Some(done) = rename_field(ui, name) {
                        if done && !name.trim().is_empty() {
                            actions.push(LayerAction::Run(Command::RenameLayer {
                                id: layer.id,
                                name: name.trim().to_string(),
                            }));
                        }
                        self.renaming = None;
                    }
                    return;
                }

                let response = ui.selectable_label(self.active_layer == Some(layer.id), &layer.name);
                if response.double_clicked() {
                    actions.push(LayerAction::Rename(layer.id, layer.name.clone()));
                } else if response.clicked() {
                    actions.push(LayerAction::Select(layer.id));
                }
                response.context_menu(|ui| self.layer_menu(ui, doc, layer, actions));
            });
            if let Some(children) = layer.children()
                && !self.collapsed.contains(&layer.id)
            {
                self.layer_rows(ui, doc, children, depth + 1, actions);
            }
        }
    }

    fn layer_menu(&self, ui: &mut egui::Ui, doc: &Document, layer: &Layer, actions: &mut Vec<LayerAction>) {
        if ui.button("Rename").clicked() {
            actions.push(LayerAction::Rename(layer.id, layer.name.clone()));
            ui.close();
        }
        if ui.button("Group").on_hover_text("Put the layer inside a new group").clicked() {
            actions.push(LayerAction::Group(layer.id));
            ui.close();
        }
        let parent = doc.locate_layer(layer.id).and_then(|(p, _)| p);
        if ui.add_enabled(parent.is_some(), egui::Button::new("Ungroup")).on_hover_text("Take the layer out of its group").clicked() {
            actions.push(LayerAction::Ungroup(layer.id));
            ui.close();
        }
        ui.menu_button("Move To", |ui| {
            if ui.add_enabled(parent.is_some(), egui::Button::new("Top Level")).clicked() {
                actions.push(LayerAction::MoveInto(layer.id, None));
                ui.close();
            }
            for group in doc.all_layers().into_iter().filter(|g| g.is_group() && !doc.is_inside(g.id, layer.id)) {
                let here = parent == Some(group.id);
                if ui.add_enabled(!here, egui::Button::new(&group.name)).clicked() {
                    actions.push(LayerAction::MoveInto(layer.id, Some(group.id)));
                    ui.close();
                }
            }
        });
        let can_delete = !(parent.is_none() && doc.layers.len() == 1);
        if ui.add_enabled(can_delete, egui::Button::new("Delete")).clicked() {
            actions.push(LayerAction::Delete(layer.id));
            ui.close();
        }
    }

    fn apply_layer_action(&mut self, editor: &mut Editor, action: LayerAction) {
        let doc = editor.doc();
        match action {
            LayerAction::Select(id) => self.set_active_layer(id),
            LayerAction::Rename(id, name) => self.renaming = Some((id, name)),
            LayerAction::Run(command) => self.execute(editor, command),
            LayerAction::New { group } => {
                // Above the active layer, among its siblings.
                let (parent, index) = self
                    .active_layer
                    .and_then(|id| doc.locate_layer(id))
                    .map_or((None, None), |(p, i)| (p, Some(i + 1)));
                let number = doc.all_layers().len() + 1;
                let id = editor.new_layer_id();
                let layer = if group {
                    Layer::group(id, format!("Group {number}"))
                } else {
                    Layer::vector(id, format!("Layer {number}"))
                };
                self.execute(editor, Command::InsertLayer { parent, index, layer });
                self.set_active_layer(id);
            }
            LayerAction::Delete(id) => self.execute(editor, Command::RemoveLayer { id }),
            LayerAction::Raise(id) | LayerAction::Lower(id) => {
                let up = matches!(action, LayerAction::Raise(_));
                let Some((parent, index)) = doc.locate_layer(id) else { return };
                let index = if up { index + 1 } else { index.saturating_sub(1) };
                self.execute(editor, Command::MoveLayer { id, parent, index });
            }
            LayerAction::Group(id) => {
                let Some((parent, index)) = doc.locate_layer(id) else { return };
                let group = editor.new_layer_id();
                let number = editor.doc().all_layers().len() + 1;
                let commands = vec![
                    Command::InsertLayer {
                        parent,
                        index: Some(index),
                        layer: Layer::group(group, format!("Group {number}")),
                    },
                    Command::MoveLayer {
                        id,
                        parent: Some(group),
                        index: 0,
                    },
                ];
                self.execute(editor, Command::Batch { commands });
            }
            LayerAction::Ungroup(id) => {
                let Some((Some(parent), _)) = doc.locate_layer(id) else { return };
                let Some((grandparent, index)) = doc.locate_layer(parent) else { return };
                self.execute(
                    editor,
                    Command::MoveLayer {
                        id,
                        parent: grandparent,
                        index: index + 1,
                    },
                );
            }
            LayerAction::MoveInto(id, target) => {
                let Some((current, _)) = doc.locate_layer(id) else { return };
                let Some(len) = doc.siblings(target).map(<[Layer]>::len) else { return };
                // On top of the target (in the same parent it no longer counts itself).
                let index = len - usize::from(current == target);
                self.execute(editor, Command::MoveLayer { id, parent: target, index });
            }
        }
    }
}

/// Fill, stroke and width rows of the style grid. Returns `true` when a continuous
/// interaction ends (to stop merging undo steps).
fn style_rows(ui: &mut egui::Ui, style: &mut Style) -> bool {
    let mut released = false;
    let mut has_fill = style.fill.is_some();
    if ui.checkbox(&mut has_fill, "Fill").changed() {
        style.fill = has_fill.then_some(Style::DEFAULT_FILL);
        released = true;
    }
    match &mut style.fill {
        Some(fill) => color_button(ui, fill),
        None => {
            ui.label("");
        }
    }
    ui.end_row();

    let mut has_stroke = style.stroke.is_some();
    if ui.checkbox(&mut has_stroke, "Stroke").changed() {
        style.stroke = has_stroke.then_some(Style::DEFAULT_STROKE);
        released = true;
    }
    match &mut style.stroke {
        Some(stroke) => color_button(ui, &mut stroke.color),
        None => {
            ui.label("");
        }
    }
    ui.end_row();

    if let Some(stroke) = &mut style.stroke {
        ui.label("Width");
        let r = ui.add(egui::DragValue::new(&mut stroke.width).range(0.25..=200.0).speed(0.25).suffix(" px"));
        released |= r.drag_stopped() || r.lost_focus();
        ui.end_row();
    }
    released
}

/// Text field for renaming. Returns `Some(true)` on confirm (Enter or focus lost),
/// `Some(false)` on cancel (Esc) and `None` while editing.
fn rename_field(ui: &mut egui::Ui, name: &mut String) -> Option<bool> {
    let response = ui.add(egui::TextEdit::singleline(name).desired_width(120.0));
    if !response.has_focus() && !response.lost_focus() {
        response.request_focus();
    }
    if response.lost_focus() {
        let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
        return Some(!cancelled);
    }
    None
}

fn color_button(ui: &mut egui::Ui, color: &mut Color) {
    let mut rgba = color.to_array();
    if ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed() {
        *color = Color::from_array(rgba);
    }
}
