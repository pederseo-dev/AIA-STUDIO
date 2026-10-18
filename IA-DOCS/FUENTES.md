# Fuentes de conocimiento

Registro de dónde salió la información técnica usada en el proyecto.

## Skills

No hay skills instaladas sobre Rust, egui, wgpu o gráficos vectoriales (revisado el 2026-10-17).
Las skills disponibles en la sesión cubren documentos, artifacts, la API de Claude y la
configuración de Claude Code. **Para el chat de IA de los paneles** se usará la skill
`claude-api` (referencia de la API de Anthropic) cuando se implemente.

Si en el futuro se instalan skills de Rust/gráficos, anotarlas acá.

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
