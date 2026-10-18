# Arquitectura

## Stack

| Capa | Tecnología | Versión |
|---|---|---|
| Lenguaje | Rust (edición 2024) | rustc 1.99 |
| Ventana + UI | eframe / egui | 0.36 |
| Íconos | egui-phosphor (Phosphor Regular) | 0.14 |
| GPU | wgpu (compartido por egui y vello) | 30 |
| Render vectorial | vello | 0.11 |
| Geometría | kurbo (con `serde`) | 0.13 |
| Serialización | serde / serde_json | 1.x |
| Diálogos de archivo | rfd | 0.17 |

**Restricción de versiones:** egui-phosphor 0.14 requiere egui 0.36 (no 0.37). vello 0.11 y
egui-wgpu 0.36 usan wgpu 30. Antes de actualizar cualquiera, verificar que todos
sigan compartiendo la misma versión de wgpu (`cargo tree -i wgpu`).

## Crates del workspace

```
crates/
  aia-core/    Documento, comandos, undo/redo, Mode, Agent, archivo de proyecto (chats). Sin UI ni GPU.
  aia-render/  Documento + cámara + overlay → escena vello → textura wgpu.
  aia-ai/      Configuración de IA por agente (solo tipos por ahora).
  aia-app/     Ejecutable `aia-studio`.
```

### aia-app por dentro
```
src/
  main.rs         opciones de ventana y backend gráfico (DX12 en Windows)
  app.rs          nav de modos, layout de paneles, menús, atajos, archivos, persistencia
  canvas.rs       canvas compartido: render vello→egui, pan/zoom, auto-encuadre (sin herramientas)
  chat_panel.rs   panel de chat: selector de agentes (automático + manual) y conversación del agente
  modes/
    mod.rs        íconos de modo y de agente, botón de herramienta con tooltip+atajo
    design/       (DesignMode)
      mod.rs      estado del modo, teclado, operaciones (orden, voltear, flechas, portapapeles), overlay
      input.rs    mouse sobre el canvas por herramienta (Selección, Editar nodos, Pluma, Rect/Elipse)
      sidebar.rs  panel lateral (Pluma/Nodo, Transformar, Estilo, Capas con formas)
      tools.rs    herramientas, gestos y matemática de transformación (con tests)
      clipboard.rs  formas ↔ texto JSON del portapapeles
    animation.rs  placeholder + área de línea de tiempo (AnimationMode)
```
Cada modo expone `tools_ui` (barra de íconos) y `sidebar_ui` (panel lateral); los que
interpretan el mouse exponen además `canvas_input`, `cursor` y `overlay`. `app.rs` decide
según el modo activo. El canvas solo hace pan/zoom y render: **el significado de un gesto
depende del modo** (en Animation, mover = keyframe).

Dependencias: `aia-app → aia-render → aia-core`, `aia-app → aia-ai → aia-core`.

## Principios de diseño

### 1. Todo cambio pasa por un `Command`
`aia_core::Command` es un enum serializable (`#[serde(tag = "type")]`). `Command::apply(doc)`
modifica el documento y **devuelve el comando inverso**; el `Editor` guarda los inversos
para deshacer/rehacer. `Batch` agrupa varios y se revierte entero si uno falla.

Esto es la base para la IA: los mismos comandos que usa la UI se expondrán como
herramientas (tool use) de cada agente. Lo que haga la IA queda en el historial.

### Agentes de IA
- `aia_core::Agent` (Draw, Rigging, Animation). Cada agente pertenece a un modo (`Agent::mode`)
  y solo trabaja en él: en Animation no hay agente con herramientas que modifiquen la base.
- Cada agente tiene herramientas fijas, así el caché de prompt se mantiene (cambiar herramientas o
  modelo a mitad de conversación lo invalida). El contexto variable (modo, selección, resumen de
  escena) irá como mensajes `system` dentro de la conversación.
- `aia_core::project`: el `.aia` guarda `{ format_version, document, chats }`. `Chats` no pasa por
  undo/redo.

### 2. Modelo del documento
```
Document { width, height, background, layers, next_id }
 └─ Layer { id, name, visible, locked, shapes }   (orden: abajo → arriba)
     └─ Shape { id, name, path: VectorPath, transform: Affine, style }
         ├─ VectorPath { nodes: Vec<Node>, closed }
         │   └─ Node { point, handle_in: Vec2, handle_out: Vec2, kind: Corner|Smooth|Symmetric }
         └─ Style { fill: Option<Color>, stroke: Option<StrokeStyle> }
```
- `VectorPath` es el formato propio (`aia-core/src/path.rs`); se convierte a `kurbo::BezPath`
  solo para render y hit-testing. Las manijas son relativas al nodo (`Vec2::ZERO` = sin manija),
  así mover un nodo arrastra sus manijas. `Node::set_handle` aplica la restricción del tipo.
- Los nodos explícitos son la base para que los huesos deformen nodos con pesos.
- La transformación está separada del path para que los huesos puedan deformar
  los puntos de control sin perder la geometría original.
- `Color` es RGBA8 sRGB sin premultiplicar (propio, no depende de peniko).
- Ids únicos (`ShapeId`, `LayerId`) asignados por el documento.

### 3. Estado de UI fuera del documento
Selección, herramienta activa, capa activa y gestos en curso viven en `DesignMode`; la
cámara en `Canvas`; el modo y los paneles abiertos en `Layout` (persistido con eframe).
Nada de esto está en el documento. Al arrastrar, el movimiento se muestra como
`Overlay` provisional y recién al soltar se ejecuta un único `TransformShapes`.

### 4. Integración vello ↔ egui
`CanvasRenderer` renderiza en una textura `Rgba8Unorm` (STORAGE + TEXTURE_BINDING)
usando el mismo `Device` de eframe. Se registra en egui con
`register_native_texture` y se actualiza con `update_egui_texture_from_wgpu_texture`
si cambia el tamaño. El shader de egui espera texturas no-sRGB, que es lo que entrega vello.

### 5. Coordenadas
- Documento: unidades del escenario (1920×1080 por defecto).
- `Camera { offset, zoom }`: documento → puntos de UI locales al canvas.
- `build_scene` recibe `scale = pixels_per_point` para renderizar en píxeles físicos.
- La selección se dibuja en espacio de pantalla (grosor constante).

### 6. Backend gráfico y ventana
En Windows se fuerza DX12 (`main.rs::preferred_backends`); `WGPU_BACKEND` lo sobreescribe.
La ventana se maximiza con `ViewportCommand::Maximized` en el primer frame (no con
`with_maximized`, que deja la superficie más grande que el área visible).

### 7. Atajos
Globales en `app.rs` (Ctrl+N/O/S, Ctrl+Z/Y, Ctrl+1/2/3, Ctrl+B, Ctrl+Alt+B); se consumen
primero los de más modificadores. Los de herramientas (V/R/E, Supr, Esc) los maneja cada
modo y se ignoran si hay un campo de texto con foco. Todo atajo debe aparecer en el
tooltip o en el menú correspondiente.
