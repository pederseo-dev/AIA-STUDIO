# Estado del proyecto

_Última actualización: 2026-10-18_

## Visión

AIA Studio (Artificial Intelligence Animation Studio): aplicación de escritorio para
animación 2D **vectorial** profesional. Se arranca con herramientas básicas y se escala.
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

## Interfaz (estilo VS Code)

```
┌ Archivo Editar Ver                                   [◧] [⬓] [◨]   ┐  menús + paneles (Ctrl+B/J/Alt+B)
├                    [Design]   [Animation]                          ┤  modos (Ctrl+1/2)
├──┬──────────────┬──────────────────────────────┬─────────────────────┤
│🛠│ panel lateral │           canvas             │  chat IA con        │
│  │ (config del  │   (compartido entre modos)    │  selector de agente │
│  │  modo)       │                              │                     │
│  │              ├──────────────────────────────┤                     │
│  │              │ panel inferior: mensajes o    │                     │
│  │              │ línea de tiempo (Animation)   │                     │
├──┴──────────────┴──────────────────────────────┴─────────────────────┤
│ barra de estado: modo · zoom · herramienta · agente · mensajes      │
└──────────────────────────────────────────────────────────────────────┘
```
- Barra de íconos izquierda: herramientas del modo; tooltip con nombre + atajo.
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

## Problemas conocidos / resueltos

- **Vulkan + Intel UHD crashea** → DX12 por defecto en Windows. Ver `DECISIONES.md`.
- **Maximizar al crear la ventana** dejaba la superficie de render más grande que el área visible
  (se cortaba la barra de estado) → ahora se maximiza en el primer frame y solo en la primera ejecución.

## Advertencias de diseño pendientes

1. En Animation, mover una forma debe crear/modificar un keyframe, nunca la geometría base.
   Cada modo maneja su propio input del canvas (ya preparado en `modes/`).
2. Editar el dibujo después de riggear/animar puede romper pesos o keyframes: requerirá validaciones.
3. Historial de undo único para los dos modos (decisión tomada; es lo más predecible).
4. Pedidos que cruzan agentes: se resolverán con un botón "Pasar a <agente>" (orquestador: más adelante).

## Próximos pasos (debatir antes de implementar)

1. Fase B del dibujo en Design (ver DECISIONES): paleta de colores, estilo de trazo, gradientes,
   opacidad, grosor variable por nodo, booleanas, grilla e imanes, texto.
2. Fase C (puente a los huesos): grupos, punto de pivote, importar SVG/imágenes, deformadores.
3. Panel de configuración de IA (proveedores, modelos, skills por agente) y agente Draw con los
   `Command` como herramientas, resumen de escena y captura del canvas.
4. Herramienta Hueso en Design + agente Rigging.
5. Modo Animation (keyframes, línea de tiempo) + agente Animation.
