# Arquitectura

AIA Studio es nuestra versión de Moho con agentes de IA. El modelo de datos sigue el de Moho;
el diseño completo y las etapas están en [MODELO-V2.md](MODELO-V2.md).

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
  aia-core/    Documento estilo Moho, comandos, undo/redo, Mode, Agent, archivo de proyecto. Sin UI ni GPU.
  aia-render/  Documento + cámara + overlay → escena vello → textura wgpu.
  aia-ai/      IA: configuración, API keys, proveedores (Anthropic, compatible con OpenAI), chat en un hilo.
  aia-app/     Ejecutable `aia-studio`.
```

### aia-core por dentro
```
src/
  document.rs   Document, árbol de Layer (Vector | Group), LayerTransform + origen, búsquedas y transformaciones
  mesh.rs       Mesh: puntos (ordenados por id), curvas, formas; geometría (manijas Auto, segmentos, trazados) y hit-testing
  topology.rs   operaciones de estructura (insertar/borrar puntos, borrar arista, soldar, formas desde puntos, auto fill/stroke)
  style.rs      Color, StrokeStyle, Style
  command.rs    Command (capas, malla, formas, Batch) con su inverso
  editor.rs     Editor: documento + undo/redo, ids nuevos, comandos fusionables
  mode.rs       Mode (Design/Animation) y Agent (Draw/Rigging/Animation)
  project.rs    archivo .aia v4: { format_version, document, chats }
```

### aia-ai por dentro
```
src/
  config.rs     AiSettings: proveedores (id, tipo, URL, variable de la key) y modelo de cada agente
  keys.rs       API keys en el almacén seguro del sistema (keyring) o en una variable de entorno
  provider.rs   interfaz común: Request, Reply, Usage, Stop; stream() elige el adaptador
  anthropic.rs  Messages API por HTTP (ureq) con SSE; guarda los bloques de thinking para devolverlos
  openai.rs     Chat Completions compatibles con OpenAI, con SSE
  chat.rs       ChatJob: el pedido corre en un hilo y la UI lee los eventos cada frame
  prompts.rs    instrucciones fijas por agente (no cambian entre pedidos, así se cachean)
```
Reglas de la API de Claude (2026): versión `2026-10-21`; los bloques de *thinking* se devuelven sin modificar
(por eso `ChatMessage::raw`); no enviar `temperature`/`top_p`; no forzar `tool_choice`; manejar `refusal`.

### aia-app por dentro
```
src/
  main.rs         opciones de ventana y backend gráfico (DX12 en Windows)
  app.rs          nav de modos, layout de paneles, menús, atajos, archivos, persistencia
  canvas.rs       canvas compartido: render vello→egui, pan/zoom, auto-encuadre (sin herramientas)
  chat_panel.rs   panel de chat: selector de agentes, conversación, envío y respuesta en streaming
  ai_settings.rs  ventana AI Settings: proveedores, API keys y modelo de cada agente
  modes/
    mod.rs        íconos de modo y de agente, `Icon` (glifo Phosphor o dibujado), botón de herramienta
    design/       (DesignMode)
      mod.rs      estado del modo, panel Tools (grilla por sección), paleta inteligente, teclado, operaciones, vista previa
      icons.rs    íconos propios dibujados con trazos (estilo Moho, sin copiar sus imágenes)
      input.rs    mouse sobre el canvas por herramienta
      panels.rs   barra de opciones de la herramienta, panel Style, panel Layers (árbol)
      tools.rs    herramientas (nombres y atajos de Moho, ayuda para la barra de estado), gestos y matemática (con tests)
    animation.rs  placeholders: herramientas, opciones, inspector y línea de tiempo (AnimationMode)
```
Cada modo expone `tools_panel` (panel Tools), `options_bar` (barra de opciones) y sus paneles
(`style_panel`/`layers_panel` en Design, `inspector` en Animation); los que interpretan el mouse
exponen además `canvas_input`, `cursor` y `overlay`. `app.rs` decide según el modo activo. El canvas
solo hace pan/zoom y render: **el significado de un gesto depende del modo** (en Animation, mover = keyframe).

### Layout (como Moho, más el chat)
```
menú + botones de paneles      (Panel::top "menu_bar")
modos                          (Panel::top "mode_bar", Ctrl+M)
opciones de la herramienta     (Panel::top "options_bar")
Tools │ canvas │ Style/Layers │ AI Chat
      │ Messages / Timeline (Ctrl+J)
barra de estado: ayuda de la herramienta · zoom · agente
```
Tools = `Panel::left` (Ctrl+B). A la derecha se declaran primero el chat (queda en el extremo,
Ctrl+Alt+B) y después el inspector (Ctrl+Shift+B), que adentro tiene `Panel::top` (Style,
redimensionable) y `CentralPanel` (Layers).

### Idioma
La interfaz, los mensajes y los comentarios del código están en **inglés**; la documentación de
IA-DOCS y el README siguen en español. Los nombres de las herramientas son los de Moho.

Dependencias: `aia-app → aia-render → aia-core`, `aia-app → aia-ai → aia-core`.

## Principios de diseño

### 1. Todo cambio pasa por un `Command`
`aia_core::Command` es un enum serializable (`#[serde(tag = "type")]`). `Command::apply(doc)`
modifica el documento y **devuelve el comando inverso**; el `Editor` guarda los inversos
para deshacer/rehacer. `Batch` agrupa varios y se revierte entero si uno falla.

- Las operaciones que cambian la estructura de una malla (agregar/borrar puntos, soldar, borrar
  aristas) se calculan sobre una copia y se confirman con `ReplaceMesh` (el inverso es la malla anterior).
- Los cambios continuos (arrastrar un campo numérico, elegir un color, renombrar) usan
  `Editor::execute_merged(command, key)`: los cambios seguidos con la misma clave se deshacen de una
  vez. `Editor::end_merge` corta la fusión al soltar el mouse o perder el foco.

Esto es la base para la IA: las herramientas de alto nivel de cada agente se traducirán a `Batch`
de estos comandos. Lo que haga la IA queda en el historial.

### Agentes de IA
- `aia_core::Agent` (Draw, Rigging, Animation). Cada agente pertenece a un modo (`Agent::mode`)
  y solo trabaja en él: en Animation no hay agente con herramientas que modifiquen la base.
- Cada agente tiene herramientas fijas, así el caché de prompt se mantiene (cambiar herramientas o
  modelo a mitad de conversación lo invalida). El contexto variable (modo, selección, resumen de
  escena) irá como mensajes `system` dentro de la conversación.
- La IA verá la escena como capas con nombre y formas en SVG (ver MODELO-V2 §6).

### 2. Modelo del documento (estilo Moho)
```
Document { width, height, background, layers, next_id }
 └─ Layer { id, name, visible, locked, opacity, origin, transform: LayerTransform, kind }
     ├─ Vector { mesh: Mesh }
     │    Mesh { points (ordenados por id), curves, shapes }
     │     ├─ MeshPoint { id, pos, width }
     │     ├─ Curve { id, closed, nodes: [CurveNode { point, mode, handle_in, handle_out }] }
     │     │     mode = Auto { curvature } | Corner | Smooth | Symmetric
     │     └─ Shape { id, name, fill: [Loop = [EdgeRef]], outline: [EdgeRef], style }
     └─ Group { children: [Layer] }
```
- **Puntos compartidos:** las curvas referencian puntos por id; dos curvas que pasan por el mismo
  punto están soldadas. Mover un punto mueve todas sus curvas.
- **Geometría y apariencia separadas:** puntos y curvas no se renderizan; las formas (relleno y/o
  contorno) se arman con aristas `EdgeRef { curve, segment, reversed }`.
- **Manijas automáticas (Moho):** en modo `Auto` las manijas salen de los vecinos: tangente paralela
  a la recta entre vecinos × curvatura, repartida según la distancia a cada uno. Con 4 puntos y
  `DEFAULT_CURVATURE` (0.5523) se obtiene un círculo; curvatura 0 = pico.
- **Los índices de segmento cambian** al insertar o borrar puntos: `topology.rs` guarda las formas
  como secuencias de puntos antes de cada operación y reconstruye sus aristas después (puede pasar
  por puntos nuevos). Si un borde desaparece, el relleno se pierde, como en Moho.
- **Transformación en la capa:** `LayerTransform { translation, scale, rotation, flip_h, flip_v }`
  alrededor de `origin` (coordenadas de la capa). Mundo = padres × propia.
  `with_origin_moved` cambia el origen sin mover la capa (Set Origin).
- `Color` es RGBA8 sRGB sin premultiplicar. Ids únicos (`LayerId`, `PointId`, `CurveId`, `ShapeId`)
  asignados por el documento.

### 3. Estado de UI fuera del documento
Selección de puntos y formas, herramienta, capa activa y gestos en curso viven en `DesignMode`;
la cámara en `Canvas`; el modo y los paneles abiertos en `Layout` (persistido con eframe).
Nada de esto está en el documento. Durante un arrastre, la vista previa es una **copia de la malla**
(`Overlay::edited_mesh`) o una **transformación provisional de la capa** (`Overlay::edited_transform`);
al soltar se ejecuta un único comando.

**Paleta inteligente:** las herramientas de dibujo y relleno solo se habilitan con una capa vectorial
activa; en un grupo quedan las de capa.

### 4. Integración vello ↔ egui
`CanvasRenderer` renderiza en una textura `Rgba8Unorm` (STORAGE + TEXTURE_BINDING)
usando el mismo `Device` de eframe. Se registra en egui con
`register_native_texture` y se actualiza con `update_egui_texture_from_wgpu_texture`
si cambia el tamaño. El shader de egui espera texturas no-sRGB, que es lo que entrega vello.

El render recorre el árbol de capas acumulando transformación y opacidad. El wireframe (curvas,
puntos, manijas) de la capa activa se dibuja encima en espacio de pantalla.

### 5. Coordenadas
- Documento: unidades del escenario (1920×1080 por defecto).
- Capa: espacio de sus puntos; `Document::world_transform(id)` lleva de la capa al documento.
- `Camera { offset, zoom }`: documento → puntos de UI locales al canvas.
- `build_scene` recibe `scale = pixels_per_point` para renderizar en píxeles físicos.
- Wireframe, selección y cajas se dibujan en espacio de pantalla (grosor constante).
- La tolerancia de impacto se pasa a coordenadas de la capa dividiendo por la escala de su transformación.

### 6. Backend gráfico y ventana
En Windows se fuerza DX12 (`main.rs::preferred_backends`); `WGPU_BACKEND` lo sobreescribe.
La ventana se maximiza con `ViewportCommand::Maximized` en el primer frame (no con
`with_maximized`, que deja la superficie más grande que el área visible).

### 7. Atajos
Globales en `app.rs` (Ctrl+N/O/S, Ctrl+Z/Y, Ctrl+1/2, Ctrl+B, Ctrl+Shift+B, Ctrl+J, Ctrl+M, Ctrl+Alt+B); se consumen
primero los de más modificadores. Los de herramientas usan las letras de Moho (G, T, A, C, E, D, Q, U,
M, O) y los maneja cada modo, igual que Supr, Esc, Enter, flechas y Ctrl+A; se ignoran si hay un
campo de texto con foco. Todo atajo debe aparecer en el tooltip o en el menú correspondiente.

### 8. Pruebas en la app real
Para verificar cambios visuales se lanza la app y se la maneja con un script de PowerShell
(mouse con `SetCursorPos`/`mouse_event`, teclado con `keybd_event`, captura con `CopyFromScreen`).
El proceso del script tiene que llamar a `SetProcessDPIAware()`: si no, en pantallas escaladas las
coordenadas y la captura no coinciden con las de la app.
**Antes de simular mouse o teclado hay que confirmar con el usuario que no está usando la PC**: la
entrada simulada va a la ventana que esté al frente (pasó una vez con Moho abierto). Si el `.exe` está
en uso por una instancia del usuario, compilar con `--target-dir target/test-build` en vez de cerrarla.
