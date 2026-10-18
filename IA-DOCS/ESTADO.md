# Estado del proyecto

_Última actualización: 2026-10-17_

## Visión

AIA Studio (Artificial Intelligence Animation Studio): aplicación de escritorio para
animación 2D **vectorial** profesional. Se arranca con herramientas básicas y se escala.
Tres **modos** de trabajo específicos, **cada uno con su propio chat de IA**:

- **Draw**: crear y editar la geometría base y su estilo.
- **Bones**: armar el esqueleto (jerarquía, IK, pesos), estilo Spine/Moho.
- **Timeline**: animar; los cambios crean keyframes y **nunca** tocan la geometría base.

No es objetivo (por ahora): animación tradicional cuadro a cuadro ni dibujo a mano alzada.

## Interfaz (estilo VS Code)

```
┌ Archivo Editar Ver                                   [◧] [⬓] [◨]   ┐  menús + paneles (Ctrl+B/J/Alt+B)
├               [Draw]   [Bones]   [Timeline]                          ┤  modos (Ctrl+1/2/3)
├──┬──────────────┬──────────────────────────────┬─────────────────────┤
│🛠│ panel lateral │           canvas             │  chat IA del modo   │
│  │ (config del  │   (compartido entre modos)    │  (próximamente)     │
│  │  modo)       │                              │                     │
│  │              ├──────────────────────────────┤                     │
│  │              │ panel inferior: mensajes o    │                     │
│  │              │ línea de tiempo (Timeline)    │                     │
├──┴──────────────┴──────────────────────────────┴─────────────────────┤
│ barra de estado: modo · zoom · herramienta · mensajes               │
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

## Problemas conocidos / resueltos

- **Vulkan + Intel UHD crashea** → DX12 por defecto en Windows. Ver `DECISIONES.md`.
- **Maximizar al crear la ventana** dejaba la superficie de render más grande que el área visible
  (se cortaba la barra de estado) → ahora se maximiza en el primer frame y solo en la primera ejecución.

## Advertencias de diseño pendientes

1. En Timeline, mover una forma debe crear/modificar un keyframe, nunca la geometría base.
   Cada modo maneja su propio input del canvas (ya preparado en `modes/`).
2. Editar el dibujo después de riggear/animar puede romper pesos o keyframes: requerirá validaciones.
3. Historial de undo único para los tres modos (decisión tomada; es lo más predecible).

## Próximos pasos (debatir antes de implementar)

1. Transformar (escalar/rotar) con manijas en Selección.
2. Selección de varios nodos (marco) y alinear/distribuir.
3. Panel de configuración de IA (proveedores, modelos, skills por modo) y chat de Draw con los `Command` como herramientas.
4. Modo Bones.
5. Modo Timeline.
