//! Panel lateral del modo Draw: herramienta, transformar, estilo y capas.

use aia_core::kurbo::Affine;
use aia_core::{Color, Command, Editor, Layer, NodeKind, ShapeId, StrokeStyle};
use eframe::egui;
use egui_phosphor::regular as icons;

use super::tools::{self, Tool};
use super::{DesignMode, Order, Renaming};

impl DesignMode {
    pub fn sidebar_ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        match self.tool {
            Tool::Pen => {
                egui::CollapsingHeader::new("Pluma").default_open(true).show(ui, |ui| {
                    ui.weak(
                        "Clic: nodo de esquina\nArrastrar: nodo con curva\nAlt al arrastrar: solo la manija de salida\n\
                         Shift: ángulos de 45°\nClic en el primer nodo: cerrar\nEnter, Esc o doble clic: terminar",
                    );
                });
            }
            Tool::EditNodes => {
                egui::CollapsingHeader::new("Nodo")
                    .default_open(true)
                    .show(ui, |ui| self.node_section(ui, editor));
            }
            _ => {}
        }
        if !self.selection.is_empty() {
            egui::CollapsingHeader::new("Transformar")
                .default_open(true)
                .show(ui, |ui| self.transform_section(ui, editor));
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

        if self.nodes.is_empty() {
            ui.weak("Clic en un nodo para seleccionarlo (Shift suma).\nArrastrá en el vacío para un marco.\nClic sobre el trazado agrega un nodo.");
        } else {
            let count = self.nodes.len();
            if count == 1 {
                ui.label(format!("Nodo {} de {}", self.nodes[0] + 1, path.nodes.len()));
            } else {
                ui.label(format!("{count} nodos seleccionados"));
            }
            // Un tipo se ve activo si todos los nodos seleccionados lo tienen.
            ui.horizontal(|ui| {
                for kind in NodeKind::ALL {
                    let all = self.nodes.iter().all(|&i| path.nodes[i].kind == kind);
                    if ui.selectable_label(all, kind.name()).clicked() {
                        for &i in &self.nodes {
                            path.set_kind(i, kind);
                        }
                        changed = true;
                    }
                }
            });
            let can_remove = path.nodes.len() - count >= 2;
            let label = if count == 1 { "Borrar nodo" } else { "Borrar nodos" };
            if ui
                .add_enabled(can_remove, egui::Button::new(format!("{} {label}", icons::TRASH)))
                .on_hover_text("Supr")
                .clicked()
            {
                path.nodes = (path.nodes.iter().enumerate())
                    .filter(|(i, _)| !self.nodes.contains(i))
                    .map(|(_, n)| *n)
                    .collect();
                path.closed &= path.nodes.len() > 2;
                self.nodes.clear();
                changed = true;
            }
        }

        if changed {
            self.execute(editor, Command::SetShapePath { id, path });
        }
    }

    /// Posición, tamaño y rotación de la selección, más voltear y orden.
    fn transform_section(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let Some(bounds) = self.selection_bounds(editor) else { return };
        // Lo que muestran los campos incluye la vista previa si se está arrastrando uno.
        let shown = self.numeric_preview.map_or(bounds, |t| t.transform_rect_bbox(bounds));
        let mut edited = shown;
        let mut interaction = NumericInteraction::default();

        egui::Grid::new("transform_grid").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
            let mut x = shown.x0;
            let mut y = shown.y0;
            ui.label("X");
            interaction.track(ui.add(egui::DragValue::new(&mut x).speed(1.0).max_decimals(1)));
            ui.label("Y");
            interaction.track(ui.add(egui::DragValue::new(&mut y).speed(1.0).max_decimals(1)));
            ui.end_row();
            edited = edited.with_origin((x, y));

            let mut w = shown.width();
            let mut h = shown.height();
            ui.label("An");
            let rw = ui.add(egui::DragValue::new(&mut w).speed(1.0).range(0.1..=f64::MAX).max_decimals(1));
            ui.label("Al");
            let rh = ui.add(egui::DragValue::new(&mut h).speed(1.0).range(0.1..=f64::MAX).max_decimals(1));
            ui.end_row();
            if self.keep_ratio && shown.width() > 0.0 && shown.height() > 0.0 {
                if rw.changed() {
                    h = w * shown.height() / shown.width();
                } else if rh.changed() {
                    w = h * shown.width() / shown.height();
                }
            }
            interaction.track(rw);
            interaction.track(rh);
            edited = edited.with_size((w, h));
        });

        // Relativa a lo que se muestra (que ya incluye la vista previa); se compone abajo.
        let mut transform = (edited != shown).then(|| tools::rect_to_rect(shown, edited));

        // Rotación: solo con una forma, que tiene un ángulo propio.
        if let [id] = self.selection[..]
            && let Some(shape) = editor.doc().shape(id)
        {
            let current = tools::rotation_degrees(self.numeric_preview.unwrap_or_default() * shape.transform);
            let mut angle = current;
            ui.horizontal(|ui| {
                ui.label("Rotación");
                interaction.track(ui.add(egui::DragValue::new(&mut angle).speed(0.5).suffix("°").max_decimals(1)));
            });
            if angle != current {
                let c = shown.center().to_vec2();
                let delta = (angle - current).to_radians();
                transform = Some(Affine::translate(c) * Affine::rotate(delta) * Affine::translate(-c));
            }
        }

        ui.checkbox(&mut self.keep_ratio, format!("{} Mantener proporción", icons::LINK));

        // Arrastrar un campo muestra una vista previa; soltar o escribir confirma el cambio.
        if let Some(t) = transform {
            let total = t * self.numeric_preview.unwrap_or_default();
            if interaction.dragging {
                self.numeric_preview = Some(total);
            } else {
                self.numeric_preview = None;
                self.transform_selection(editor, total);
            }
        } else if interaction.released
            && let Some(total) = self.numeric_preview.take()
        {
            self.transform_selection(editor, total);
        }

        ui.horizontal(|ui| {
            if ui.button(icons::FLIP_HORIZONTAL).on_hover_text("Voltear horizontal").clicked() {
                self.flip(editor, true);
            }
            if ui.button(icons::FLIP_VERTICAL).on_hover_text("Voltear vertical").clicked() {
                self.flip(editor, false);
            }
            ui.separator();
            for (icon, tip, order) in [
                (icons::ARROW_LINE_UP, "Al frente (Inicio)", Order::Front),
                (icons::ARROW_UP, "Subir (RePág)", Order::Up),
                (icons::ARROW_DOWN, "Bajar (AvPág)", Order::Down),
                (icons::ARROW_LINE_DOWN, "Al fondo (Fin)", Order::Back),
            ] {
                if ui.button(icon).on_hover_text(tip).clicked() {
                    self.reorder(editor, order);
                }
            }
        });
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
        let layer_count = editor.doc().layers.len();
        let active_index = self.active_layer.and_then(|id| editor.doc().layer_index(id));
        ui.horizontal(|ui| {
            if ui.button(icons::PLUS).on_hover_text("Nueva capa").clicked() {
                self.add_layer(editor);
            }
            let can_remove = layer_count > 1 && active_index.is_some();
            if ui
                .add_enabled(can_remove, egui::Button::new(icons::TRASH))
                .on_hover_text("Borrar capa activa")
                .clicked()
                && let Some(id) = self.active_layer
            {
                self.execute(editor, Command::RemoveLayer { id });
            }
            let can_up = active_index.is_some_and(|i| i + 1 < layer_count);
            if ui
                .add_enabled(can_up, egui::Button::new(icons::ARROW_UP))
                .on_hover_text("Subir capa")
                .clicked()
                && let (Some(id), Some(i)) = (self.active_layer, active_index)
            {
                self.execute(editor, Command::MoveLayer { id, index: i + 1 });
            }
            let can_down = active_index.is_some_and(|i| i > 0);
            if ui
                .add_enabled(can_down, egui::Button::new(icons::ARROW_DOWN))
                .on_hover_text("Bajar capa")
                .clicked()
                && let (Some(id), Some(i)) = (self.active_layer, active_index)
            {
                self.execute(editor, Command::MoveLayer { id, index: i - 1 });
            }
        });

        // Las acciones se juntan y se ejecutan después de dibujar (el documento se está leyendo).
        let mut pending: Vec<Command> = Vec::new();
        let mut select: Option<(ShapeId, bool)> = None;
        let shift = ui.input(|i| i.modifiers.shift);

        // Se listan de arriba hacia abajo, como en cualquier editor.
        for (layer_index, layer) in editor.doc().layers.iter().enumerate().rev() {
            self.layer_row(ui, layer, layer_index, layer_count, &mut pending);
            if self.collapsed_layers.contains(&layer.id) {
                continue;
            }
            for shape in layer.shapes.iter().rev() {
                ui.horizontal(|ui| {
                    ui.add_space(28.0);
                    if let Some(Renaming::Shape(id, name)) = &mut self.renaming
                        && *id == shape.id
                    {
                        if let Some(done) = rename_field(ui, name) {
                            if done && !name.trim().is_empty() {
                                pending.push(Command::RenameShape {
                                    id: shape.id,
                                    name: name.trim().to_string(),
                                });
                            }
                            self.renaming = None;
                        }
                        return;
                    }
                    let selected = self.selection.contains(&shape.id);
                    let response = ui.selectable_label(selected, &shape.name);
                    if response.double_clicked() {
                        self.renaming = Some(Renaming::Shape(shape.id, shape.name.clone()));
                    } else if response.clicked() {
                        select = Some((shape.id, shift));
                        self.active_layer = Some(layer.id);
                    }
                    response.context_menu(|ui| {
                        if ui.button("Renombrar").clicked() {
                            self.renaming = Some(Renaming::Shape(shape.id, shape.name.clone()));
                        }
                        if ui.button("Borrar").clicked() {
                            pending.push(Command::RemoveShape { id: shape.id });
                        }
                    });
                });
            }
        }

        if let Some((id, shift)) = select {
            self.click_select_from_list(id, shift, editor);
        }
        for command in pending {
            self.execute(editor, command);
        }
    }

    fn layer_row(
        &mut self,
        ui: &mut egui::Ui,
        layer: &Layer,
        layer_index: usize,
        layer_count: usize,
        pending: &mut Vec<Command>,
    ) {
        ui.horizontal(|ui| {
            let collapsed = self.collapsed_layers.contains(&layer.id);
            let caret = if collapsed { icons::CARET_RIGHT } else { icons::CARET_DOWN };
            if ui.add(egui::Button::new(caret).frame(false)).clicked() && !self.collapsed_layers.remove(&layer.id) {
                self.collapsed_layers.insert(layer.id);
            }
            let (eye, eye_tip) = if layer.visible {
                (icons::EYE, "Ocultar")
            } else {
                (icons::EYE_SLASH, "Mostrar")
            };
            if ui.add(egui::Button::new(eye).frame(false)).on_hover_text(eye_tip).clicked() {
                pending.push(Command::SetLayerVisible {
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
                pending.push(Command::SetLayerLocked {
                    id: layer.id,
                    locked: !layer.locked,
                });
            }

            if let Some(Renaming::Layer(id, name)) = &mut self.renaming
                && *id == layer.id
            {
                if let Some(done) = rename_field(ui, name) {
                    if done && !name.trim().is_empty() {
                        pending.push(Command::RenameLayer {
                            id: layer.id,
                            name: name.trim().to_string(),
                        });
                    }
                    self.renaming = None;
                }
                return;
            }

            let active = self.active_layer == Some(layer.id);
            let response = ui.selectable_label(active, egui::RichText::new(&layer.name).strong());
            if response.double_clicked() {
                self.renaming = Some(Renaming::Layer(layer.id, layer.name.clone()));
            } else if response.clicked() {
                self.active_layer = Some(layer.id);
            }
            response.context_menu(|ui| {
                if ui
                    .add_enabled(!self.selection.is_empty(), egui::Button::new("Mover selección a esta capa"))
                    .clicked()
                {
                    pending.extend(self.move_selection_to(layer));
                    ui.close();
                }
                if ui.button("Renombrar").clicked() {
                    self.renaming = Some(Renaming::Layer(layer.id, layer.name.clone()));
                    ui.close();
                }
                if ui.add_enabled(layer_index + 1 < layer_count, egui::Button::new("Subir")).clicked() {
                    pending.push(Command::MoveLayer {
                        id: layer.id,
                        index: layer_index + 1,
                    });
                    ui.close();
                }
                if ui.add_enabled(layer_index > 0, egui::Button::new("Bajar")).clicked() {
                    pending.push(Command::MoveLayer {
                        id: layer.id,
                        index: layer_index - 1,
                    });
                    ui.close();
                }
                if ui.add_enabled(layer_count > 1, egui::Button::new("Borrar")).clicked() {
                    pending.push(Command::RemoveLayer { id: layer.id });
                    ui.close();
                }
            });
        });
    }

    /// Mueve las formas seleccionadas arriba de todo en `layer`, manteniendo su orden.
    fn move_selection_to(&self, layer: &Layer) -> Vec<Command> {
        let moving: Vec<ShapeId> = self.selection.clone();
        let staying = layer.shapes.iter().filter(|s| !moving.contains(&s.id)).count();
        let commands: Vec<Command> = moving
            .iter()
            .enumerate()
            .map(|(offset, &id)| Command::MoveShape {
                id,
                layer: layer.id,
                index: staying + offset,
            })
            .collect();
        vec![Command::Batch { commands }]
    }

    fn add_layer(&mut self, editor: &mut Editor) {
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

    fn click_select_from_list(&mut self, id: ShapeId, shift: bool, editor: &Editor) {
        if shift {
            if let Some(i) = self.selection.iter().position(|s| *s == id) {
                self.selection.remove(i);
            } else {
                self.selection.push(id);
            }
        } else {
            self.selection = vec![id];
        }
        self.nodes.clear();
        self.load_style_from_selection(editor);
    }
}

/// Estado de los campos numéricos en este frame.
#[derive(Default)]
struct NumericInteraction {
    dragging: bool,
    released: bool,
}

impl NumericInteraction {
    fn track(&mut self, response: egui::Response) {
        self.dragging |= response.dragged();
        self.released |= response.drag_stopped();
    }
}

/// Campo de texto para renombrar. Devuelve `Some(true)` al confirmar (Enter o perder
/// el foco), `Some(false)` al cancelar (Esc) y `None` mientras se edita.
fn rename_field(ui: &mut egui::Ui, name: &mut String) -> Option<bool> {
    let response = ui.add(egui::TextEdit::singleline(name).desired_width(140.0));
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
