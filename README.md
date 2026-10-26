# AIA-STUDIO

# Artificial Intelligence Animation Studio

Aplicación de escritorio para animación 2D vectorial, hecha en Rust (egui + wgpu + vello).

## Requisitos

- [Rust](https://rustup.rs/) estable (`winget install Rustlang.Rustup`)
- En Windows: Visual Studio Build Tools con "Desarrollo de escritorio con C++"
- Una GPU con soporte para DirectX 12, Vulkan o Metal

## Comandos

```bash
# Ejecutar la aplicación (la primera compilación tarda varios minutos)
cargo run -p aia-app

# Ejecutar en modo release (más rápido, recomendado para probar rendimiento)
cargo run -p aia-app --release

# Correr los tests
cargo test --workspace

# Solo compilar, sin ejecutar
cargo build -p aia-app

# Ver logs de errores en consola (PowerShell)
$env:RUST_LOG="warn"; cargo run -p aia-app
```

> Si `cargo` no se reconoce, reiniciá la terminal (o VS Code) después de instalar Rust.

## Interfaz (en inglés, con el layout de Moho)

```
File Edit View                                     [modos][Tools][abajo][Style/Layers][Chat]
                     [Design]   [Animation]                        ← modos (Ctrl+1/2, Ctrl+M oculta)
Position X:[..] Y:[..] [Reset] Scale X:[..%] ... ← opciones de la herramienta activa
┌─Tools──────┐┌──────────────────────┐┌─Style────┐┌─AI Chat─┐
│Draw  ▣✥✎◇  ││                      ││Fill  ■   ││ Draw    │
│      ✂     ││        canvas        ││Stroke ■  ││ Rigging │
│Fill  ↖⊞    ││                      │├─Layers───┤│ ...     │
│Layer ⤢⊕    │├──────────────────────┤│👁 Layer 1│└─────────┘
└────────────┘│ Messages / Timeline  │└──────────┘
Ayuda de la herramienta (como Moho)                                  Zoom · Agente
```

- **Tools** (izquierda, `Ctrl+B`): herramientas en grilla por sección (Draw, Fill, Layer). Las de dibujo y relleno solo se activan con una capa vectorial.
- **Barra de opciones** (arriba del canvas): las opciones de la herramienta activa, con campos numéricos y `Reset`, como en Moho.
- **Style** y **Layers** (derecha, `Ctrl+Shift+B`): estilo de las formas nuevas o de las seleccionadas; árbol de capas.
- **AI Chat** (extremo derecho, `Ctrl+Alt+B`): selector de agentes — **Draw** y **Rigging** en Design, **Animation** en Animation.
- **Panel inferior** (`Ctrl+J`): mensajes en Design, línea de tiempo en Animation.
- **Barra de estado**: a la izquierda, cómo usar la herramienta activa; a la derecha, zoom y agente.
- Los botones de la esquina superior derecha muestran u ocultan cada panel. La app recuerda el modo, los paneles abiertos y el tamaño de la ventana.

## Uso del modo Design (al estilo de Moho)

Como en Moho, la **geometría** (puntos y curvas) es invisible al exportar: lo que se ve son las **formas**
(relleno y/o contorno) que se arman sobre ella. Dos curvas que pasan por el mismo punto están soldadas.

| Herramienta | Atajo | Cómo |
|---|---|---|
| Select Points | `G` | Clic en un punto, en una curva (toda la curva) o en un relleno (todos sus puntos); arrastrar = marco. `Shift` suma, `Alt` resta |
| Transform Points | `T` | Arrastrar en la franja entre la caja y el rectángulo exterior = rotar alrededor del **pivote** (mira con círculo; se arrastra para moverlo, Reset lo vuelve al centro). Arrastrar un punto = mueve ese punto; desde adentro de un relleno = mueve toda la forma; en otro lado = la selección (con menos de dos puntos, el más cercano). Soltar la punta de una curva sobre otro punto la **suelda** (Auto Weld; `Enter` mientras se arrastra suelda cualquier punto). Flip H / Flip V. Caja: esquinas/lados escalan (`Shift` = proporción, `Alt` = desde el centro), círculo de arriba rota (`Shift` = 15°). Opciones: posición X/Y, Auto Weld, Auto Fill, Auto Stroke, Show Bezier Handles (arrastrar una manija la vuelve manual, `Alt` = esquina) |
| Add Point | `A` | Arrastrar en el vacío = curva nueva; desde la punta de una curva = continuarla; desde otro punto = rama soldada; clic o arrastre sobre una curva = insertar un punto. Soltar sobre otro punto lo **suelda** (sobre el primero, cierra la curva). `Shift` = 45°. Opciones: Auto Weld, Auto Fill, Auto Stroke, Sharp Corners, Show Bezier Handles |
| Curvature | `C` | Arrastrar a la izquierda = curva más aguda, a la derecha = más redonda, en los puntos seleccionados (sin selección, en el más cercano). Arrastrar una manija Bézier = cambiar su ángulo y largo. Clic = seleccionar. Opciones: Curvature, Peak, Smooth, Reset (manijas a automáticas) |
| Draw Shape | `E` | Rectangle u Oval. `Shift` = cuadrado/círculo, `Alt` = desde el centro |
| Delete Edge | `D` | Clic en un segmento (se resalta al pasar el mouse). Los puntos quedan; un relleno que usaba ese borde se pierde |
| Select Shape | `Q` | Clic en un relleno o contorno (`Shift` suma, `Alt` resta). La forma seleccionada se ve con una trama sobre el relleno y el contorno resaltado. Orden de apilado y borrar en la barra de opciones |
| Create Shape | `U` | Seleccionar los puntos del borde y `Enter`: si cierran un contorno hay relleno, si no, solo trazo |
| Transform Layer | `M` | Arrastrar = mover la capa; manijas = escalar/rotar **alrededor del origen**. Opciones: Position, Scale (%), Angle con `Reset`, voltear |
| Set Origin | `O` | Clic = nuevo origen (la capa no se mueve) |

| Acción | Cómo |
|---|---|
| Curvatura | Herramienta Curvature (`C`), o con puntos seleccionados (barra de opciones): **Peak**, **Smooth** o el valor de Curvature |
| Estilo de formas | Panel Style: con formas seleccionadas (con cualquier herramienta; las formas recién dibujadas quedan seleccionadas) edita esas formas, y el cambio queda como valor por defecto. Las casillas **Fill** y **Stroke** los quitan o los vuelven a poner. **Deselect** (o `Esc`) = editar los valores por defecto. Una forma sin relleno ni trazo se puede seguir seleccionando |
| Borrar | `Supr`/`Retroceso`: puntos seleccionados (o formas con `Q`) |
| Seleccionar todo | `Ctrl+A` (puntos, o formas con `Q`) |
| Mover con teclado | Flechas (`Shift` = 10): puntos seleccionados, o la capa con `M`/`O` |
| Cancelar / deseleccionar | `Esc` |
| Capas | Panel Layers: nueva capa vectorial, nuevo grupo, borrar, subir/bajar; clic derecho = Rename, Group, Ungroup, Move To; ojo y candado |
| Zoom / mover la vista | Rueda del mouse / botón central o `Espacio` + arrastrar |
| Encuadrar escenario | View → Fit Stage |
| Deshacer / Rehacer | `Ctrl+Z` / `Ctrl+Shift+Z` o `Ctrl+Y` |
| Nuevo / Abrir / Guardar | `Ctrl+N` / `Ctrl+O` / `Ctrl+S` (`Ctrl+Shift+S` = guardar como) |

Los proyectos se guardan como archivos `.aia` (JSON).

## Estructura

```
crates/
  aia-core/    # documento estilo Moho (capas, puntos, curvas, formas), comandos, undo/redo (sin UI ni GPU)
  aia-render/  # render del documento con vello sobre wgpu
  aia-ai/      # base de configuración de IA por agente (sin conexión todavía)
  aia-app/     # ejecutable: nav de modos, canvas compartido, herramientas, paneles
```
