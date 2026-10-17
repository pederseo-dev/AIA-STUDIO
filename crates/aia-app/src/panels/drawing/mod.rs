mod canvas;
mod tools;

use aia_core::kurbo::{Affine, Point, Vec2};
use aia_core::{Color, Command, Editor, Layer, LayerId, Shape, ShapeId, StrokeStyle, Style};
use aia_render::{Camera, Overlay, build_scene};
use eframe::egui::{self, Sense};
use eframe::egui_wgpu::RenderState;

use canvas::CanvasGpu;
pub use tools::Tool;
use tools::Gesture;

/// Radio de impacto del mouse, en puntos de pantalla.
const HIT_TOLERANCE: f64 = 4.0;

pub struct DrawingPanel {
    gpu: Result<CanvasGpu, String>,
    camera: Camera,
    /// Encuadrar el escenario en el próximo frame (al abrir o crear un documento).
    fit_pending: bool,
    tool: Tool,
    /// Estilo para las formas nuevas.
    style: Style,
    selection: Vec<ShapeId>,
    active_layer: Option<LayerId>,
    gesture: Gesture,
    message: Option<String>,
}

impl DrawingPanel {
    pub fn new(render_state: Option<RenderState>) -> Self {
        let gpu = render_state
            .ok_or_else(|| "No hay GPU disponible (wgpu)".to_string())
            .and_then(CanvasGpu::new);
        if let Err(e) = &gpu {
            log::error!("No se pudo iniciar el canvas: {e}");
        }
        Self {
            gpu,
            camera: Camera::default(),
            fit_pending: true,
            tool: Tool::Select,
            style: Style::default(),
            selection: Vec::new(),
            active_layer: None,
            gesture: Gesture::None,
            message: None,
        }
    }

    pub fn zoom(&self) -> f64 {
        self.camera.zoom
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn message(&self) -> Option<&String> {
        self.message.as_ref()
    }

    /// Vuelve a encuadrar y olvida el estado ligado al documento anterior.
    pub fn reset_view(&mut self) {
        self.fit_pending = true;
        self.selection.clear();
        self.active_layer = None;
        self.gesture = Gesture::None;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        self.sync_with_document(editor);

        egui::Panel::left("drawing_tools")
            .resizable(false)
            .exact_size(44.0)
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::right("drawing_properties")
            .default_size(230.0)
            .size_range(180.0..=400.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.style_section(ui, editor);
                    ui.separator();
                    self.layers_section(ui, editor);
                });
            });
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas(ui, editor));
    }

    /// Descarta referencias a formas/capas que ya no existen (por ejemplo, tras deshacer).
    fn sync_with_document(&mut self, editor: &Editor) {
        let doc = editor.doc();
        self.selection.retain(|id| doc.shape(*id).is_some());
        if self.active_layer.and_then(|id| doc.layer(id)).is_none() {
            self.active_layer = doc.layers.last().map(|l| l.id);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            for tool in Tool::ALL {
                let button = egui::Button::selectable(self.tool == tool, egui::RichText::new(tool.icon()).size(18.0))
                    .min_size(egui::vec2(32.0, 32.0));
                let response = ui
                    .add(button)
                    .on_hover_text(format!("{} ({:?})", tool.name(), tool.shortcut()));
                if response.clicked() {
                    self.set_tool(tool);
                }
            }
        });
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.gesture = Gesture::None;
    }

    fn style_section(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        ui.heading("Estilo");

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
            ui.heading("Capas");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_remove = editor.doc().layers.len() > 1 && self.active_layer.is_some();
                if ui
                    .add_enabled(can_remove, egui::Button::new("−"))
                    .on_hover_text("Borrar capa")
                    .clicked()
                    && let Some(id) = self.active_layer
                {
                    self.execute(editor, Command::RemoveLayer { id });
                }
                if ui.button("+").on_hover_text("Nueva capa").clicked() {
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
            });
        });

        // Se listan de arriba hacia abajo, como en cualquier editor.
        let mut pending = None;
        for layer in editor.doc().layers.iter().rev() {
            ui.horizontal(|ui| {
                let mut visible = layer.visible;
                if ui.checkbox(&mut visible, "").on_hover_text("Visible").changed() {
                    pending = Some(Command::SetLayerVisible { id: layer.id, visible });
                }
                let lock = if layer.locked { "🔒" } else { "🔓" };
                if ui
                    .add(egui::Button::new(lock).frame(false))
                    .on_hover_text("Bloquear")
                    .clicked()
                {
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

    fn canvas(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        if self.fit_pending {
            self.camera.fit(editor.doc(), Vec2::new(rect.width() as f64, rect.height() as f64));
            self.fit_pending = false;
        }

        self.handle_keys(ui, editor);
        self.handle_view_input(ui, &response, rect);
        self.handle_tool_input(ui, &response, rect, editor);

        let panning = ui.input(|i| i.key_down(egui::Key::Space));
        let cursor = if panning || response.dragged_by(egui::PointerButton::Middle) {
            egui::CursorIcon::Grab
        } else if self.tool.creates_shapes() {
            egui::CursorIcon::Crosshair
        } else {
            egui::CursorIcon::Default
        };
        response.on_hover_cursor(cursor);

        self.paint(ui, rect, editor);
    }

    fn handle_keys(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
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

    fn handle_view_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, rect: egui::Rect) {
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        if response.dragged_by(egui::PointerButton::Middle) || (space && response.dragged_by(egui::PointerButton::Primary)) {
            let delta = response.drag_delta();
            self.camera.offset += Vec2::new(delta.x as f64, delta.y as f64);
        }

        if let Some(pos) = response.hover_pos() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta().y, i.zoom_delta()));
            let factor = zoom as f64 * (scroll as f64 * 0.002).exp();
            if factor != 1.0 {
                self.camera.zoom_at(to_local(pos, rect), factor);
            }
        }
    }

    fn handle_tool_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, rect: egui::Rect, editor: &mut Editor) {
        if ui.input(|i| i.key_down(egui::Key::Space)) {
            return;
        }
        let shift = ui.input(|i| i.modifiers.shift);
        let camera = self.camera;
        let to_doc = |pos: egui::Pos2| camera.screen_to_doc(to_local(pos, rect));

        if response.clicked() && self.tool == Tool::Select
            && let Some(pos) = response.interact_pointer_pos()
        {
            let hit = editor.doc().hit_test(to_doc(pos), HIT_TOLERANCE / self.camera.zoom);
            self.click_select(hit, shift, editor);
        }

        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            let start = to_doc(origin);
            self.begin_gesture(start, shift, editor);
        }

        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            if let Gesture::Move { current, .. } | Gesture::Box { current, .. } = &mut self.gesture {
                *current = to_doc(pos);
            }
        }

        if response.drag_stopped() {
            self.finish_gesture(shift, editor);
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

    fn begin_gesture(&mut self, start: Point, shift: bool, editor: &Editor) {
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
            Tool::Select => {
                match editor.doc().hit_test(start, HIT_TOLERANCE / self.camera.zoom) {
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
                }
            }
            Tool::Rectangle | Tool::Ellipse => Gesture::Box { start, current: start },
        };
    }

    fn finish_gesture(&mut self, shift: bool, editor: &mut Editor) {
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
                let min_size = 2.0 / self.camera.zoom;
                if rect.width() >= min_size && rect.height() >= min_size {
                    let path = tools::box_path(self.tool, rect);
                    self.insert_shape(editor, path, self.style);
                }
            }
        }
    }

    fn insert_shape(&mut self, editor: &mut Editor, path: aia_core::kurbo::BezPath, style: Style) {
        let Some(layer) = self.active_layer else { return };
        let shape = Shape {
            id: editor.new_shape_id(),
            path,
            transform: Affine::IDENTITY,
            style,
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

    fn preview_shape(&self, shift: bool) -> Option<Shape> {
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

    fn paint(&mut self, ui: &mut egui::Ui, rect: egui::Rect, editor: &Editor) {
        let shift = ui.input(|i| i.modifiers.shift);
        let preview = self.preview_shape(shift);
        let gpu = match &mut self.gpu {
            Ok(gpu) => gpu,
            Err(e) => {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("Canvas no disponible: {e}"),
                    egui::FontId::proportional(14.0),
                    ui.visuals().warn_fg_color,
                );
                return;
            }
        };

        let moving = match &self.gesture {
            Gesture::Move { start, current } => Some(Affine::translate(*current - *start)),
            _ => None,
        };
        let overlay = Overlay {
            moving: moving.map(|t| (self.selection.as_slice(), t)),
            preview: preview.as_ref(),
            selection: &self.selection,
        };

        let ppp = ui.ctx().pixels_per_point();
        build_scene(&mut gpu.scene, editor.doc(), &self.camera, &overlay, ppp as f64);
        let width = (rect.width() * ppp).round() as u32;
        let height = (rect.height() * ppp).round() as u32;
        match gpu.render(width, height) {
            Ok(texture) => {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(texture, rect, uv, egui::Color32::WHITE);
            }
            Err(e) => self.message = Some(format!("Error de render: {e}")),
        }
    }
}

fn to_local(pos: egui::Pos2, rect: egui::Rect) -> Point {
    Point::new((pos.x - rect.min.x) as f64, (pos.y - rect.min.y) as f64)
}

fn color_button(ui: &mut egui::Ui, color: &mut Color) {
    let mut rgba = color.to_array();
    if ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed() {
        *color = Color::from_array(rgba);
    }
}
