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

## 2026-10-18 — Modelo "nodos por forma" confirmado (vs. puntos compartidos de Moho)
Se debatió el modelo de Moho (puntos/curvas compartidos + rellenos aparte). Se eligió seguir
con un modelo tipo SVG (cada forma con sus nodos) porque es el que mejor maneja la IA:
los LLM conocen SVG, cada comando es autocontenido y se puede referir a las piezas por nombre.
La experiencia de herramientas de Moho se puede sumar encima (curvatura, imán, grosor por nodo,
deformadores). Las articulaciones se resolverán con huesos y pesos. Se agregó `Shape::name`.

## 2026-10-18 — Plan del modo Draw por fases
Investigación de Moho, LoongBones y DragonBones (ver FUENTES). Fase A (edición básica) hecha;
Fase B (calidad: paleta, trazo, gradientes, grosor variable, booleanas, grilla, texto) y
Fase C (grupos, pivote, importar SVG/imágenes, deformadores) pendientes.
Advertencias: grosor variable y booleanas son costosos (vello no hace trazos de grosor
variable; kurbo no trae booleanas).

## 2026-10-18 — Dos modos (Design/Animation) y agentes de IA con selector
Reemplaza "tres modos con un chat cada uno". Dibujo y huesos se alternan todo el tiempo, así
que van juntos en Design (como Rive y Spine); Animation nunca toca la base. La IA se separa en
agentes (Draw, Rigging, Animation) dentro de un único panel de chat con selector:
- Cada agente: conversación, herramientas, modelo y skills propios (caché estable, pocas tools).
- En Animation solo está disponible el agente Animation (Draw/Rigging deshabilitados).
- Selección automática según modo y herramienta, con override manual hasta que cambie la sugerencia.
- Historial de chat por proyecto, dentro del `.aia` (formato v3). Sin migración (regla 7).
- Pedidos entre agentes: botón de derivación; un orquestador queda para más adelante.

## 2026-10-18 — Portapapeles como texto JSON
egui solo emite el evento de pegar si el portapapeles del sistema tiene texto, así que las formas
se copian como JSON (`{"aia_studio_shapes": [...]}`). Permite pegar entre ventanas.

## 2026-10-18 — Atajos de orden con RePág/AvPág/Inicio/Fin
`Ctrl+[` / `Ctrl+]` no funcionan bien en teclados en español; se usan las teclas de Inkscape.

## 2026-10-25 — Moho como referencia principal
El usuario usó mucho Moho y lo considera el punto justo entre principiante y profesional: las
funcionalidades más importantes se toman de ahí. Se descargaron los manuales oficiales de Moho 13.5 y se
resumieron en [REFERENCIA-MOHO.md](REFERENCIA-MOHO.md).

## 2026-10-25 — Modelo estilo Moho (revierte "nodos por forma" del 2026-10-18)
AIA Studio será **nuestra propia versión de Moho** con chat de IA y agentes especializados por área.
Se adopta el modelo de Moho: árbol de capas con transformación y origen, mallas de **puntos compartidos**
(soldadura) + curvas + formas que referencian aristas, grosor por punto, y huesos que deforman puntos.
Motivo: casi todo lo distintivo de Moho (soldadura, bordes compartidos, Hide Edge, grosor por punto,
binding y Smart Bones) depende de que lo animado sean los puntos.
La facilidad para la IA (motivo de la decisión anterior) se resuelve con una **capa de traducción**:
la IA ve la escena como capas con nombre y formas en SVG, y usa herramientas de alto nivel por nombre
que se traducen a `Batch` de comandos. Se arranca limpio (el usuario autorizó borrar lo necesario);
sin migración de archivos (regla 7). Diseño en [MODELO-V2.md](MODELO-V2.md), pendiente de aprobación.

## 2026-10-25 — Interfaz y código en inglés
Se estandariza en inglés todo lo que se ve en la app y los comentarios del código; la documentación
(IA-DOCS, README) sigue en español para el usuario. Las herramientas usan los nombres de Moho.

## 2026-10-25 — Layout de Moho con el chat al costado
Reemplaza el layout "estilo VS Code" con panel lateral único. Como Moho: panel Tools a la izquierda en
grilla por secciones, barra de opciones de la herramienta arriba del canvas, Style y Layers a la derecha,
ayuda de la herramienta en la barra de estado. El chat de IA queda como columna aparte en el extremo
derecho (la diferencia con Moho). Se mantienen la barra de modos y los botones para ocultar paneles.

## 2026-10-25 — Style edita la selección con cualquier herramienta
Antes solo con Select Shape, y el usuario no encontraba cómo quitar el relleno o el trazo. Ahora, si hay
formas seleccionadas, Style las edita (las recién dibujadas quedan seleccionadas) y el cambio también pasa
a ser el valor por defecto, como en Moho. Curvature (C) se adelantó de la Etapa 2 por pedido del usuario.

## 2026-10-25 — Íconos de herramientas propios, inspirados en Moho
El usuario pidió los íconos de Moho para ubicarse. No se copian (son de Lost Marble/Smith Micro, con
copyright): se dibujan versiones propias con trazos en `modes/design/icons.rs` (nítidas a cualquier escala,
toman el color del tema). Empezó por las 6 herramientas principales de Draw.

## 2026-10-17 — Flujo de git
Se trabaja directo en `main` hasta tener una base estable. El usuario dirige y es autor de
los commits; Claude figura como `Co-Authored-By`.
