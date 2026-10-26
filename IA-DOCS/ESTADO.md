# Estado del proyecto

_Última actualización: 2026-10-25_

> **Rumbo actual (2026-10-25):** AIA Studio es **nuestra propia versión de Moho** (modelo de datos y
> herramientas lo más parecidos posible) con la diferencia de un **chat de IA con agentes especializados
> por área**. Diseño en [MODELO-V2.md](MODELO-V2.md) (aprobado). **Etapa 1 hecha** (v2.0) y layout de Moho en inglés (v2.1). Lo de v1–v1.4
> queda como historial: ese núcleo y esas herramientas se reemplazaron.

## Visión

AIA Studio (Artificial Intelligence Animation Studio): aplicación de escritorio para
animación 2D **vectorial** profesional al estilo de **Moho** (referencia: [REFERENCIA-MOHO.md](REFERENCIA-MOHO.md)),
intermedia entre principiante y profesional. Se arranca con herramientas básicas y se escala.
Dos **modos** de trabajo (como Rive y Spine):

- **Design**: construir el personaje: dibujo vectorial y, más adelante, huesos y pesos. Modifica la base.
- **Animation**: animar; los cambios crean keyframes y **nunca** tocan la base.

Un solo **panel de chat** con un **selector de agentes especializados**, cada uno con su propia
conversación, herramientas, modelo y skills:

- **Draw** y **Rigging** (disponibles en Design) · **Animation** (disponible en Animation).
- El selector sigue solo al modo y a la herramienta activa; elegir otro a mano dura hasta que
  cambie la sugerencia. En Animation solo trabaja el agente Animation (la IA no puede tocar la base).
- El documento es la memoria compartida; las conversaciones se guardan dentro del `.aia`.

No es objetivo (por ahora): animación tradicional cuadro a cuadro ni dibujo a mano alzada.

## Trabajo en curso (leer primero)

- Último commit: v2.2 (`51e6a65`). **Sin commit:** v2.3.
- **Falta prueba manual de v2.1–v2.3:** compila, 37 tests y clippy limpios, pero no se probó con
  el mouse (ver ARQUITECTURA §8: preguntar antes de simular entrada).
- Próximo paso acordado: que el usuario pruebe v2.1 y v2.2, commitear (usuario autor, Claude co-autor) y
  debatir la Etapa 2.

## Interfaz (layout de Moho, textos en inglés)

```
File Edit View                                [modos][Tools][abajo][Style/Layers][Chat]
                     [Design]   [Animation]                  ← modos (Ctrl+1/2, Ctrl+M)
Position X:[..] Y:[..] [Reset] Scale ...  ← barra de opciones de la herramienta activa
┌─Tools──────┐┌──────────────────────┐┌─Style────┐┌─AI Chat─┐
│Draw  ▣✥✎⌒  ││        canvas        ││Fill ■    ││ agentes │
│      ◇✂    ││
│────────────││
│Fill  ↖⊞    ││                      │├─Layers───┤│         │
│────────────││
│Layer ⤢⊕    │├──────────────────────┤│ árbol    ││         │
└────────────┘│ Messages / Timeline  │└──────────┘└─────────┘
ayuda de la herramienta (como Moho)                     zoom · agente
```
- Tools `Ctrl+B`, Style/Layers `Ctrl+Shift+B`, panel inferior `Ctrl+J`, chat `Ctrl+Alt+B`.
- Se recuerda entre sesiones: modo, paneles abiertos, tamaño de ventana y de paneles.

## Hecho ✅

**v1 — Dibujo vectorial básico**
- Canvas con vello sobre wgpu: pan (botón central o Espacio + arrastrar), zoom con rueda anclado al cursor.
  Se mantiene encuadrado al cambiar de tamaño hasta que el usuario mueve o hace zoom.
- Herramientas Draw: Selección/mover (V), Rectángulo (R), Elipse (E). `Shift` = cuadrado/círculo o sumar a la selección.
- Estilo: relleno y trazo (color, grosor), "Aplicar a la selección".
- Capas: crear, borrar, ocultar (ojo), bloquear (candado), capa activa.
- Undo/Redo global por comandos. Nuevo/Abrir/Guardar `.aia` (JSON), aviso de cambios sin guardar.

**v1.1 — Modos y layout estilo VS Code**
- Nav superior con modos Draw / Bones / Timeline (Ctrl+1/2/3). Se quitó egui_dock.
- Barra de íconos (Phosphor) + panel lateral colapsable + panel de chat a la derecha (placeholder).
- Bones y Timeline: placeholders (Timeline muestra el área de línea de tiempo bajo el canvas).
- `PanelId` renombrado a `Mode`; la config de IA es por modo (`ModeAiConfig`).

**v1.2 — Pluma y edición de nodos**
- Modelo propio `VectorPath` (nodos con manijas relativas y tipo Esquina/Suave/Simétrico); formato `.aia` v2.
  Rectángulos y elipses también son nodos editables.
- Pluma (P): esquina con clic, curva con arrastre, Alt rompe simetría, Shift 45°, cerrar en el primer nodo.
- Editar nodos (A): mover nodos y manijas, agregar nodo con clic en el trazado, borrar nodo, cambiar tipo, cerrar/abrir trazado.
- Comando `SetShapePath` (undo de toda edición de nodos).
- Dos barras superiores (menús + modos centrados más grandes), botones de paneles estilo VS Code,
  panel inferior general (mensajes / línea de tiempo) con Ctrl+J.
- Tests: 17 (`cargo test --workspace`).

**v1.3 — Fase A del modo Draw (edición básica completa)**
- Formas con nombre (para el panel y para la IA). Comandos `RenameShape`, `MoveShape`, `MoveLayer`.
- Selección: marco, caja de transformación con manijas (escalar/rotar; Shift/Alt), cursores de redimensión.
- Panel Transformar: X/Y/ancho/alto/rotación con vista previa al arrastrar, mantener proporción, voltear, orden.
- Editar nodos: varios nodos (Shift+clic y marco), mover y borrar en grupo, tipo para todos.
- Portapapeles vía texto JSON del sistema (Ctrl+C/X/V, entre ventanas), duplicar (Ctrl+D).
- Orden de apilado: RePág/AvPág/Inicio/Fin (independiente del idioma del teclado).
- Capas como árbol con sus formas: renombrar con doble clic, menú contextual, subir/bajar, mover selección.
- Flechas para mover (Shift = 10), Alt dibuja rectángulo/elipse desde el centro.
- Barra de modos ocultable (Ctrl+M).
- Tests: 26.

**v1.4 — Modos Design/Animation y agentes de IA**
- `Mode` = Design | Animation (Ctrl+1/2). El modo Bones se integra en Design (herramienta Hueso: pendiente).
- `Agent` = Draw | Rigging | Animation, con `mode()`, `available_in()` y `default_for()`.
- Panel de chat con selector de agentes (selección automática + override manual); entrada
  deshabilitada hasta que exista la configuración de IA.
- Formato `.aia` v3: `{ format_version, document, chats }` (`aia_core::project`); `Chats` guarda
  la conversación de cada agente y no pasa por undo/redo.
- Configuración de IA por agente (`AgentAiConfig`).
- Tests: 29.

**v2.0 — Etapa 1: base estilo Moho** (arranque limpio)
- Núcleo nuevo: árbol de capas (vectorial y grupo) con transformación, origen y opacidad; mallas de
  puntos compartidos, curvas (manijas automáticas con curvatura, como Moho, o manuales) y formas que
  referencian aristas (relleno con agujeros y contorno parcial). Formato `.aia` v4 (sin migración).
- Operaciones de estructura con reconstrucción de las formas: insertar punto en curva, borrar puntos
  (la curva sigue de vecino a vecino), borrar arista (se pierde el relleno), soldar (cierra curvas),
  crear forma desde puntos.
- Herramientas con los atajos de Moho: Seleccionar puntos (G), Transformar puntos (T, caja para
  escalar/rotar, manijas Bézier opcionales), Agregar punto (A, con soldadura automática), Dibujar forma
  (E, rectángulo/óvalo), Borrar arista (D), Seleccionar forma (Q, orden de apilado), Crear forma (U),
  Transformar capa (M, alrededor del origen), Origen (O, sin mover la capa).
- Relleno y trazo automáticos; Pico/Suave y curvatura numérica para los puntos seleccionados.
- Panel de estilo en vivo (formas nuevas o formas seleccionadas, solo cambia la propiedad tocada) y
  renombrar formas. Panel de capas en árbol: crear capa/grupo, borrar, subir/bajar, agrupar, sacar del
  grupo, mover a un grupo, ocultar, bloquear, renombrar.
- Paleta inteligente: en un grupo solo quedan las herramientas de capa.
- Cambios continuos (campos numéricos, colores, nombres) se deshacen en un solo paso (`execute_merged`).
- Tests: 35 (`cargo test --workspace`). Probado en la app con mouse y teclado simulados.
- **Quedó afuera respecto de v1.4** (se rehará sobre el modelo nuevo): copiar/pegar/duplicar formas.

**v2.1 — Layout de Moho e interfaz en inglés**
- Interfaz, mensajes y comentarios del código en inglés (los nombres de herramientas son los de Moho);
  la documentación sigue en español.
- Panel **Tools** a la izquierda con secciones (Draw, Fill, Layer) y botones en grilla.
- **Barra de opciones** arriba del canvas: opciones de la herramienta activa (posición/escala/ángulo con
  Reset para la capa, posición de los puntos, curvatura Peak/Smooth, Auto Fill/Stroke, etc.).
- **Style** y **Layers** en una columna a la derecha (Ctrl+Shift+B); el chat de IA en el extremo derecho.
- Barra de estado con la ayuda de la herramienta activa, como Moho.
- Compila, 35 tests y clippy sin avisos. **Falta probarlo a mano** (no se simuló mouse para no interferir
  con el usuario).

**v2.2 — Correcciones pedidas por el usuario**
- Herramienta **Curvature (C)** adelantada de la Etapa 2: arrastre horizontal sobre los puntos
  seleccionados (o el más cercano) cambia la curvatura en vivo; las manijas manuales vuelven a automáticas.
- "Show Bezier Handles" encendido por defecto (también se ven con Curvature).
- Líneas separadoras entre las secciones Draw, Fill y Layer del panel Tools.
- **Style** edita las formas seleccionadas con cualquier herramienta (antes solo con Select Shape);
  el encabezado muestra el nombre de la forma y **Deselect** vuelve a los valores por defecto. El cambio
  también queda como valor por defecto (como Moho). Las formas recién dibujadas quedan seleccionadas.
- Selección de formas como Moho: trama semitransparente sobre el relleno y contorno resaltado (antes
  un rectángulo punteado). Una forma sin relleno ni trazo se puede seguir seleccionando (bug corregido)
  y se ve con su área tramada y el contorno punteado.
- Íconos propios dibujados con trazos para Select Points, Transform Points, Add Point, Curvature,
  Draw Shape y Delete Edge, con las ideas de los de Moho (no se copian: tienen copyright).
  Las demás herramientas siguen con Phosphor.
- Según el manual de Moho: Transform Points mueve el punto tocado o la forma entera si se agarra desde
  adentro del relleno; **Auto Weld** (casilla) en Transform Points y Add Point: soltar una punta de curva
  sobre otro punto los suelda (se resalta el destino; `Enter` al arrastrar suelda cualquier punto) y la
  curva que se cierra recibe Auto Fill/Stroke. Flip H/V de puntos. Curvature arrastra las manijas Bézier
  (ángulo y largo) y suma **Reset**. Las opciones de herramienta son casillas con nombre, como Moho.
- Pendiente de verificar en Moho (el manual no lo dice): clic suelto con Add Point (¿punto aislado?) y
  Delete Edge sobre un punto (¿borra el punto? ¿la curva se une o se corta?).
- Tests: 36.

**v2.3 — Más como Moho**
- Delete Edge: el punto que queda sin ninguna curva se elimina (verificado en Moho Pro).
- Panel Tools más chico (botones de 22 px, sin marco si no están activos).
- El color de fondo llena todo el canvas y el escenario se marca con esquinas y un borde tenue, como
  Moho. Fondo por defecto blanco hueso (240, 240, 228) en vez de blanco puro.
- El origen de la capa activa se ve siempre (cruz con flechas en X y hacia arriba, gira con la capa).
- Las capas y grupos nuevos tienen el origen en el centro del escenario, como Moho (también el Reset
  de Set Origin).
- **Pivote de Transform Points** (como Moho): mira con círculo, arranca en el centro de la selección, se
  arrastra directamente, se queda donde se deja (por capa, no se guarda en el archivo) y acompaña a los
  puntos. **Rotar** = arrastrar en la franja entre la caja y el rectángulo exterior (también en Transform
  Layer, alrededor del origen); `Shift` = pasos de 45°. Las manijas de la caja son círculos.
- Barra de opciones de Transform Points copiada de Moho: Select Group (deshabilitado, sin grupos todavía),
  Reset (pivote al centro), Position X/Y, Scale X/Y (%), Angle (°) relativos alrededor del pivote,
  Auto-weld, Auto-fill, Auto-stroke, Show Bezier Handles, Fixed Bezier Handles (deshabilitado), Flip H/V.
  La barra muestra solo el ícono de la herramienta, como Moho.
- Tests: 38.

## Problemas conocidos / resueltos

- **Vulkan + Intel UHD crashea** → DX12 por defecto en Windows. Ver `DECISIONES.md`.
- **Entrada simulada durante una prueba** cayó sobre el escritorio del usuario (con Moho abierto).
  Ahora se pregunta antes de manejar la app con mouse/teclado simulados (ver ARQUITECTURA §8).
- **Maximizar al crear la ventana** dejaba la superficie de render más grande que el área visible
  (se cortaba la barra de estado) → ahora se maximiza en el primer frame y solo en la primera ejecución.

## Advertencias de diseño pendientes

1. En Animation, mover una forma debe crear/modificar un keyframe, nunca la geometría base.
   Cada modo maneja su propio input del canvas (ya preparado en `modes/`).
2. Editar el dibujo después de riggear/animar puede romper pesos o keyframes: requerirá validaciones.
3. Historial de undo único para los dos modos (decisión tomada; es lo más predecible).
4. Pedidos que cruzan agentes: se resolverán con un botón "Pasar a <agente>" (orquestador: más adelante).

## Próximos pasos (debatir antes de implementar)

Reemplazan el plan anterior (Fases B/C sobre el modelo v1). Detalle en [MODELO-V2.md](MODELO-V2.md):

1. ~~Etapa 1 — Base estilo Moho~~ (hecha, v2.0). Pendiente de esa etapa: copiar/pegar.
2. **Etapa 2 — Dibujo completo:** soldadura, grosor por punto, Hide Edge, Paint Bucket, grupos.
3. **Etapa 3 — Huesos** + agente Rigging.
4. **Etapa 4 — Animación** (pistas, línea de tiempo, interpolaciones) + agente Animation.
5. **Etapa 5 — IA:** configuración de proveedores y agente Draw (se puede adelantar).
6. Después: switch layers, actions, Smart Bones, morphs, onion skin, imágenes, exportar.
