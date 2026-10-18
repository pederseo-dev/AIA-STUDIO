# Registro de decisiones

## 2026-10-17 — Rust nativo como stack
Se compararon Tauri + web, Electron, C++/Qt, Rust nativo y Godot. El usuario eligió
**Rust nativo** (egui + wgpu) por rendimiento y un solo lenguaje.
Riesgos aceptados: soporte de presión de tableta limitado en winit, estética de egui por defecto.

## 2026-10-17 — Tres paneles con chat de IA cada uno
Dibujo, Huesos y Animación. La IA actúa con el mismo sistema de comandos que el usuario.

## 2026-10-17 — Dibujo vectorial (no raster)
Encaja con huesos estilo cutout/Moho (se deforman puntos de control).

## 2026-10-17 — vello (y no lyon) como renderer
Elegido por la IA a pedido del usuario. vello renderiza curvas Bézier directo en GPU
con antialiasing analítico, gradientes y blending; kurbo comparte la geometría.
lyon queda como alternativa si vello da problemas de madurez.

## 2026-10-17 — Alcance v1 sin chat de IA
Solo se prepara la base: `Command` serializable, `PanelId` y `aia-ai::config`
(proveedores, modelos, skills por panel). El panel de configuración de IA se debatirá
antes de implementarlo.

## 2026-10-17 — Sin lápiz a mano alzada
El usuario no necesita animación tradicional ni estilo dibujado por ahora. Se quitó el lápiz.
La siguiente herramienta vectorial será la **Pluma** (nodos Bézier).

## 2026-10-17 — DX12 por defecto en Windows
La app crasheaba (segfault) al iniciar con Vulkan en Intel UHD Graphics
(driver 31.0.101.2135). Con DX12 funciona. Se fuerza DX12 en Windows salvo que
el usuario defina `WGPU_BACKEND`.

## 2026-10-17 — Versiones fijadas
eframe/egui 0.36 en vez de 0.37 porque egui_dock 0.21.1 todavía requiere egui ^0.36.
(Actualización: egui_dock se quitó, pero egui-phosphor 0.14 también requiere egui 0.36.)

## 2026-10-17 — Modos en vez de paneles acoplables
El usuario pidió un nav superior con Draw / Bones / Timeline y un layout tipo VS Code:
herramientas en barra de íconos a la izquierda (tooltip con nombre y atajo), configuración
en un panel lateral desplegable a la izquierda y el chat de IA a la derecha.
Cada modo es específico (como Setup/Animate en Spine). Se quitó egui_dock.
Canvas compartido entre modos; Timeline agrega la línea de tiempo abajo.

## 2026-10-17 — Un único historial de undo
Compartido por los tres modos. Más simple y predecible que historiales por modo.

## 2026-10-17 — Íconos Phosphor
egui-phosphor (variante Regular) para modos, herramientas y botones de capas.

## 2026-10-17 — Persistencia del layout
Se recuerda modo, paneles abiertos y tamaño de ventana/paneles (feature `persistence` de
eframe, guarda en `%APPDATA%/AIA Studio/data/app.ron`). El documento NO se persiste ahí.

## 2026-10-17 — Modelo de nodos propio (VectorPath) y formato .aia v2
`BezPath` de kurbo no guarda el tipo de nodo. Se pasó a `VectorPath { nodes, closed }` con
nodos de tipo Esquina/Suave/Simétrico. Motivo principal: los huesos van a deformar nodos con pesos.
Sin migración de archivos v1 (regla 7: en desarrollo no se conservan datos guardados).

## 2026-10-17 — Pluma y Editar nodos
Pluma estilo Illustrator. Editar nodos (A) se sumó en la misma etapa porque sin ella no se
pueden corregir curvas. Toda edición de nodos se confirma con un único `SetShapePath`
(la copia de trabajo vive en el gesto y se muestra con `Overlay::edited`).

## 2026-10-17 — Barras superiores y panel inferior general
Menús y modos en barras separadas (los modos centrados y más grandes). Botones para ocultar
paneles en la esquina superior derecha, dibujados como los de VS Code. El panel inferior es
general: mensajes en Draw/Bones y la línea de tiempo en Timeline (Ctrl+J).

## 2026-10-17 — Flujo de git
Se trabaja directo en `main` hasta tener una base estable. El usuario dirige y es autor de
los commits; Claude figura como `Co-Authored-By`.
