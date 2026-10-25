# Fuentes de conocimiento

Registro de dónde salió la información técnica usada en el proyecto.

## Skills

No hay skills instaladas sobre Rust, egui, wgpu o gráficos vectoriales (revisado el 2026-10-17).
Las skills disponibles en la sesión cubren documentos, artifacts, la API de Claude y la
configuración de Claude Code. **Para el chat de IA de los paneles** se usará la skill
`claude-api` (referencia de la API de Anthropic) cuando se implemente.

Si en el futuro se instalan skills de Rust/gráficos, anotarlas acá.

### Skill `claude-api` (consultada el 2026-10-18 para diseñar la IA por modo)
- Modelos y precios por millón de tokens (entrada/salida): Opus 5.5 $4/$20, Sonnet 5.5 $2/$10,
  Haiku 5.5 $0.10/$0.50; todos con 1M de contexto. Fable 5.1 $10/$50 (el más capaz).
- Caché de prompt: es por prefijo (herramientas → system → mensajes). **Cambiar herramientas o
  modelo a mitad de la sesión invalida el caché.** Para inyectar contexto nuevo (modo, selección,
  resumen de escena) sin romperlo: mensajes `role: "system"` a mitad de la conversación.
- Muchas herramientas: "tool search" carga solo los esquemas relevantes sin romper el caché.
- Herramientas con `strict: true` para que los argumentos respeten el esquema.
- Rust no tiene SDK oficial de Anthropic: la integración será HTTP directo (reqwest + streaming SSE).

## Registro de crates (crates.io)

- API `https://crates.io/api/v1/crates/<crate>` y `/<versión>/dependencies` para comprobar
  las últimas versiones y la compatibilidad de wgpu entre eframe, egui_dock y vello.

## Código fuente de los crates (fuente principal)

Leído directamente desde `~/.cargo/registry/src/` tras `cargo fetch`, porque las APIs
cambian entre versiones:

| Crate | Archivo | Qué se aprendió |
|---|---|---|
| vello 0.11 | `src/lib.rs`, `src/scene.rs` | `Renderer::new`, `RendererOptions`, `render_to_texture` (Rgba8Unorm + STORAGE_BINDING), `Scene::fill/stroke`, `AaSupport::area_only` |
| egui-wgpu 0.36 | `src/renderer.rs`, `src/lib.rs` | `register_native_texture`, `update_egui_texture_from_wgpu_texture`, `RenderState` |
| egui-wgpu 0.36 | `src/egui.wgsl` | El shader espera texturas no-sRGB → la salida de vello sirve directo |
| egui-wgpu 0.36 | `src/setup.rs` | `WgpuSetup::CreateNew`, `instance_descriptor.backends`, variables `WGPU_BACKEND` |
| eframe 0.36 | `src/epi.rs` | El trait `App` ahora usa `fn ui(&mut self, ui, frame)`; `CreationContext::wgpu_render_state` |
| egui 0.36 | `src/containers/panel.rs`, `menu.rs` | `Panel::top/left/right/bottom(...).show(ui)`, `CentralPanel::no_frame()`, `MenuBar::new().ui` |
| egui_dock 0.21 | `src/widgets/tab_viewer.rs`, `dock_state/` | `TabViewer` (requiere `id`), `split_below/right`, `DockArea::show_inside` |
| egui-phosphor 0.14 | `src/lib.rs`, `src/variants/` | `add_to_fonts(fonts, Variant::Regular)`, constantes `regular::CURSOR`, `BONE`, `FILM_STRIP`, etc. |
| eframe 0.36 | `src/epi.rs` (persistencia) | `get_value`/`set_value`/`APP_KEY`, `App::save` |
| egui 0.36 | `src/ui.rs`, `ui_builder.rs` | `scope_builder` + `UiBuilder::max_rect` para centrar los botones de modo |
| kurbo 0.13 | `src/bezpath.rs`, `src/simplify.rs` | Serde en `BezPath`/`Affine`; `simplify_bezpath` no permite ajustar el umbral de ángulo (descartado para el lápiz) |

## Documentación y algoritmos

- Docs oficiales: docs.rs de vello, egui, egui_dock y kurbo (referencia general).
- Enfoque de huesos sobre vectores (deformar puntos de control): modelo de Moho y Adobe Animate.
- Separación de modos Setup/Animate: modelo de Spine (Esoteric Software).
- Layout de barra de actividad + panel lateral + panel secundario: Visual Studio Code.
- Bug de `with_maximized` en Windows: diagnosticado comparando `GetClientRect`/`PrintWindow`
  con una captura de pantalla (Win32 vía PowerShell).
- Ramer-Douglas-Peucker y Catmull-Rom → Bézier: se evaluaron para el lápiz (descartado).

## Moho como referencia principal (2026-10-25)

Resumen completo en [REFERENCIA-MOHO.md](REFERENCIA-MOHO.md).
- Manuales oficiales de Moho 13.5 (usuario y tutoriales, PDF) descargados en `IA-DOCS/fuentes/moho/`
  junto con su texto en UTF-8 (extraído con `pdftotext -layout`). Esa carpeta no va a git (47 MB).
  Leídos: capítulos 3–8 (herramientas), 11 (tipos de capa), 13 (estilo), 16–17 (timeline, actions),
  20–22 (menús Dibujo/Huesos/Animación) y 27 (novedades 13.5).
- El manual de Moho 14 es una wiki que necesita JavaScript (https://manual.lostmarble.com/); no se pudo leer.
- API de scripting Lua (mohoscripting.com, no oficial): modelo de datos interno de Moho (`M_Mesh`,
  `M_Point`, `M_Curve`, `M_Shape`, `M_Style`, `M_Bone`, `M_Skeleton`, `MohoLayer`, `AnimChannel`,
  `InterpSetting`, constantes `MOHO`).

## Investigación de apps de referencia (2026-10-17)

Para debatir las opciones faltantes del modo Draw.

- **Moho (Lost Marble)**
  - Manual, herramientas de dibujo: https://www.lostmarble.com/moho/manual/draw_tools.html
    (Select/Translate/Scale/Rotate Points, Add Point, Delete Edge, Curvature, Magnet, Freehand,
    Rectangle, Oval, Arrow, Shear, Perspective, Bend, Noise; botones Peak/Smooth y Flip).
  - Manual, herramientas de relleno: https://www.lostmarble.com/moho/manual/fill_tools.html
    (Select/Create/Delete Shape, Line Width por punto, Hide Edge, Raise/Lower Shape).
    Moho separa geometría (puntos y curvas compartidos) de los rellenos/contornos.
  - Novedades de Moho 14: https://moho.lostmarble.com/blogs/news/moho-14-the-future-is-2d y
    https://mohoanimation.com/pages/features (Liquid Shapes: unir/restar/intersectar/mezclar;
    Freehand, Blob Brush y Eraser renovados; Multi-Stroke Fill; Connect and Create).
- **LoongBones** (animación esquelética 2D con asistente de IA; documentación también en dragonbones.cn)
  - Documentación del editor: https://www.loongbones.com/doc/en/editor/loongbones
    (huesos, slots, imágenes, mallas con pesos, shapes, máscaras, deformadores de malla,
    restricciones IK/transform/path/física, Dope Sheet, curvas, importación PSD).
  - Asistente de IA: https://www.loongbones.app/doc/editor/ui-ai (genera animaciones desde lenguaje natural).
- **DragonBones Pro 5.6** (antecesor): ficha en https://www.majorgeeks.com/files/details/dragonbones_pro.html
  (mallas, FFD, skinning, IK, editor de curvas, papel cebolla). Fuente de terceros.
