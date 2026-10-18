mod tools;

use aia_core::kurbo::{Affine, BezPath, Point, Vec2};
use aia_core::{Color, Command, Editor, Layer, LayerId, Shape, ShapeId, StrokeStyle, Style};
use aia_render::Overlay;
use eframe::egui;
use egui_phosphor::regular as icons;

use crate::canvas::CanvasView;
pub use tools::Tool;
use tools::Gesture;

/// Radio de impacto del mouse, en puntos de pantalla.
const HIT_TOLERANCE: f64 = 4.0;

/// Modo Draw: crear y editar la geometría base y su estilo.
pub struct DrawMode {
    tool: Tool,
    /// Estilo para las formas nuevas.
    style: Style,
    selection: Vec<ShapeId>,
    active_layer: Option<LayerId>,
    gesture: Gesture,
    message: Option<String>,
}

impl Default for DrawMode {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            style: Style::default(),
            selection: Vec::new(),
            active_layer: None,
            gesture: Gesture::None,
            message: None,
        }
    }
}

impl DrawMode {
    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn message(&self) -> Option<&String> {
        self.message.as_ref()
    }

    /// Olvida el estado ligado al documento anterior.
    pub fn reset(&mut self) {
        self.selection.clear();
        self.active_layer = None;
        self.gesture = Gesture::None;
        self.message = None;
    }

    /// Se llama al salir del modo: un gesto a medias no debe quedar colgado.
    pub fn cancel_gesture(&mut self) {
        self.gesture = Gesture::None;
    }

    /// Descarta referencias a formas/capas que ya no existen (por ejemplo, tras deshacer).
    pub fn sync_with_document(&mut self, editor: &Editor) {
        let doc = editor.doc();
        self.selection.retain(|id| doc.shape(*id).is_some());
        if self.active_layer.and_then(|id| doc.layer(id)).is_none() {
            self.active_layer = doc.layers.last().map(|l| l.id);
        }
    }

    pub fn tools_ui(&mut self, ui: &mut egui::Ui) {
        for tool in Tool::ALL {
            if super::tool_button(ui, self.tool == tool, tool.icon(), tool.name(), tool.shortcut_hint()).clicked() {
                self.set_tool(tool);
            }
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.gesture = Gesture::None;
    }

    pub fn sidebar_ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        egui::CollapsingHeader::new("Estilo")
            .default_open(true)
            .show(ui, |ui| self.style_section(ui, editor));
        egui::CollapsingHeader::new("Capas")
            .default_open(true)
            .show(ui, |ui| self.layers_section(ui, editor));
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
        match editor.execute(command) {
            Ok(()) => self.message = None,
            Err(e) => self.message = Some(e.to_string()),
        }
    }

    /// Atajos propios del modo (herramientas, borrar, cancelar).
    pub fn handle_keys(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let (tool, delete, escape) = ui.input(|i| {
            let tool = if i.modifiers.any() {
                None
            } else {
                Tool::ALL.into_iter().find(|t| i.key_pressed(t.shortcut()))
            };
            let delete = i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace);
            (tool, delete, i.key_pressed(egui::Key::Escape))
        });
        if let Some(tool) = tool {
            self.set_tool(tool);
        }
        if delete && !self.selection.is_empty() {
            let commands = self
                .selection
                .drain(..)
                .map(|id| Command::RemoveShape { id })
                .collect();
            self.execute(editor, Command::Batch { commands });
        }
        if escape {
            self.gesture = Gesture::None;
            self.selection.clear();
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
            self.begin_gesture(view.to_doc(origin), tolerance, shift, editor);
        }

        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
            && let Gesture::Move { current, .. } | Gesture::Box { current, .. } = &mut self.gesture
        {
            *current = view.to_doc(pos);
        }

        if response.drag_stopped() {
            self.finish_gesture(shift, view.camera.zoom, editor);
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
        // El panel de estilo muestra el estilo de la última forma elegida.
        if let Some(shape) = self.selection.last().and_then(|id| editor.doc().shape(*id)) {
            self.style = shape.style;
        }
    }

    fn begin_gesture(&mut self, start: Point, tolerance: f64, shift: bool, editor: &Editor) {
        if self.tool.creates_shapes() {
            let editable = self
                .active_layer
                .and_then(|id| editor.doc().layer(id))
                .is_some_and(Layer::is_editable);
            if !editable {
                self.message = Some("La capa activa está oculta o bloqueada".into());
                return;
            }
        }
        self.gesture = match self.tool {
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
            Tool::Rectangle | Tool::Ellipse => Gesture::Box { start, current: start },
        };
    }

    fn finish_gesture(&mut self, shift: bool, zoom: f64, editor: &mut Editor) {
        match std::mem::take(&mut self.gesture) {
            Gesture::None => {}
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
                let min_size = 2.0 / zoom;
                if rect.width() >= min_size && rect.height() >= min_size {
                    let path = tools::box_path(self.tool, rect);
                    self.insert_shape(editor, path);
                }
            }
        }
    }

    fn insert_shape(&mut self, editor: &mut Editor, path: BezPath) {
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

    /// La forma que se está creando, para dibujarla antes de confirmarla.
    pub fn preview_shape(&self, shift: bool) -> Option<Shape> {
        let Gesture::Box { start, current } = &self.gesture else {
            return None;
        };
        let rect = tools::drag_rect(*start, *current, shift);
        Some(Shape {
            id: ShapeId(0),
            path: tools::box_path(self.tool, rect),
            transform: Affine::IDENTITY,
            style: self.style,
        })
    }

    /// Lo que se dibuja encima del documento: selección, arrastre y forma en creación.
    pub fn overlay<'a>(&'a self, preview: Option<&'a Shape>) -> Overlay<'a> {
        let moving = match &self.gesture {
            Gesture::Move { start, current } => Some((self.selection.as_slice(), Affine::translate(*current - *start))),
            _ => None,
        };
        Overlay {
            moving,
            preview,
            selection: &self.selection,
        }
    }
}

fn color_button(ui: &mut egui::Ui, color: &mut Color) {
    let mut rgba = color.to_array();
    if ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed() {
        *color = Color::from_array(rgba);
    }
}
