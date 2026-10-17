# Arquitectura

## Stack

| Capa | Tecnología | Versión |
|---|---|---|
| Lenguaje | Rust (edición 2024) | rustc 1.99 |
| Ventana + UI | eframe / egui | 0.36 |
| Paneles acoplables | egui_dock | 0.21 |
| GPU | wgpu (compartido por egui y vello) | 30 |
| Render vectorial | vello | 0.11 |
| Geometría | kurbo (con `serde`) | 0.13 |
| Serialización | serde / serde_json | 1.x |
| Diálogos de archivo | rfd | 0.17 |

**Restricción de versiones:** egui_dock 0.21 requiere egui 0.36 (no 0.37). vello 0.11 y
egui-wgpu 0.36 usan wgpu 30. Antes de actualizar cualquiera, verificar que todos
sigan compartiendo la misma versión de wgpu (`cargo tree -i wgpu`).

## Crates del workspace

```
crates/
  aia-core/    Documento, comandos, undo/redo, PanelId. Sin UI ni GPU.
  aia-render/  Documento + cámara + overlay → escena vello → textura wgpu.
  aia-ai/      Configuración de IA por panel (solo tipos por ahora).
  aia-app/     Ejecutable `aia-studio`: ventana, menú, dock, paneles, herramientas.
```

Dependencias: `aia-app → aia-render → aia-core`, `aia-app → aia-ai → aia-core`.

## Principios de diseño

### 1. Todo cambio pasa por un `Command`
`aia_core::Command` es un enum serializable (`#[serde(tag = "type")]`). `Command::apply(doc)`
modifica el documento y **devuelve el comando inverso**; el `Editor` guarda los inversos
para deshacer/rehacer. `Batch` agrupa varios y se revierte entero si uno falla.

Esto es la base para la IA: los mismos comandos que usa la UI se expondrán como
herramientas (tool use) del chat de cada panel. Lo que haga la IA queda en el historial.

### 2. Modelo del documento
```
Document { width, height, background, layers, next_id }
 └─ Layer { id, name, visible, locked, shapes }   (orden: abajo → arriba)
     └─ Shape { id, path: BezPath, transform: Affine, style }
         └─ Style { fill: Option<Color>, stroke: Option<StrokeStyle> }
```
- La transformación está separada del path para que los huesos puedan deformar
  los puntos de control sin perder la geometría original.
- `Color` es RGBA8 sRGB sin premultiplicar (propio, no depende de peniko).
- Ids únicos (`ShapeId`, `LayerId`) asignados por el documento.

### 3. Estado de UI fuera del documento
Selección, herramienta activa, capa activa, cámara y gestos en curso viven en
`DrawingPanel`, no en el documento. Al arrastrar, el movimiento se muestra como
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

### 6. Backend gráfico
En Windows se fuerza DX12 (`main.rs::preferred_backends`); `WGPU_BACKEND` lo sobreescribe.
