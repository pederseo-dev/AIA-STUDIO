# Modelo v2 — AIA Studio estilo Moho (diseño para revisar)

_Estado: **aprobado** (2026-10-25). Etapa 1 implementada (v2.0); las secciones de animación (§5) e IA (§6) siguen siendo diseño._

Objetivo: **nuestra propia versión de Moho** (modelo de datos y herramientas lo más parecidos posible)
con una diferencia: un **chat de IA con agentes especializados por área** que trabajan con el mismo
sistema de comandos que el usuario. Referencia funcional: [REFERENCIA-MOHO.md](REFERENCIA-MOHO.md).

Se arranca limpio: el núcleo actual (`Shape` con `VectorPath` propio y transformación por forma) y las
herramientas del modo Design se reemplazan. Se conservan el canvas (vello↔egui, cámara), el layout de
paneles, los modos Design/Animation, el panel de chat con selector de agentes y la configuración de IA.

---

## 1. Tres niveles

```
┌──────────────────────────────────────────────────────────────┐
│ IA (aia-ai): vista de escena en texto/SVG + herramientas de   │
│ alto nivel por nombre ("crear_forma", "mover", "atar_a_hueso")│
└───────────────┬──────────────────────────────────────────────┘
                │ se traducen a un Batch de Commands (un solo undo)
┌───────────────▼──────────────────────────────────────────────┐
│ Commands (aia-core): operaciones atómicas con inverso, undo   │  ← los usan las herramientas
└───────────────┬──────────────────────────────────────────────┘     de la UI y la IA
┌───────────────▼──────────────────────────────────────────────┐
│ Documento estilo Moho: árbol de capas, mallas de puntos,      │
│ curvas y formas; animación en pistas separadas de la base     │
└──────────────────────────────────────────────────────────────┘
```

## 2. Documento

```
Document {
  settings: { width, height, background, fps, start_frame, end_frame },
  layers: Vec<Layer>,            // árbol; orden de abajo hacia arriba
  animation: Animation,          // pistas de keyframes (Etapa 4). La base NO se toca al animar.
  next_id,
}

Layer {
  id: LayerId, name, visible, locked,
  transform: LayerTransform { translation: Vec2, scale: Vec2, rotation: f64, flip_h, flip_v },
  origin: Point,                 // pivote de rotación/escala (Set Origin)
  opacity: f64,
  kind: LayerKind,
}

LayerKind =
  | Vector(Mesh)
  | Group  { children: Vec<Layer> }
  | Bone   { skeleton: Skeleton, children: Vec<Layer> }        // Etapa 3
  | Switch { children: Vec<Layer>, active: LayerId }           // más adelante
  | Image  { … }                                               // más adelante
```

- La transformación vive en la **capa** (como Moho). Los puntos se mueven directamente.
- La transformación final de una capa = la de sus padres × la propia (alrededor de su origen).

## 3. Malla vectorial (como `M_Mesh` de Moho)

```
Mesh {
  points: Vec<MeshPoint>,        // ids estables: el orden no importa
  curves: Vec<Curve>,
  shapes: Vec<Shape>,            // orden de apilado dentro de la capa (abajo → arriba)
  groups: Vec<PointGroup>,       // grupos de puntos con nombre (Etapa 2)
}

MeshPoint { id: PointId, pos: Point, width: f64 /* multiplicador de grosor, 1.0 */ }
          // más adelante: binding a huesos, color por punto

Curve { id: CurveId, closed: bool, nodes: Vec<CurveNode> }
CurveNode { point: PointId, mode: HandleMode, handle_in: Vec2, handle_out: Vec2 }

HandleMode =
  | Auto { curvature: f64 }      // manijas calculadas con los vecinos (comportamiento por defecto de Moho)
  | Corner | Smooth | Symmetric  // manijas manuales (modo Bézier de Moho Pro)

Shape {
  id: ShapeId, name,
  fill: Option<Vec<Loop>>,       // contornos cerrados ordenados (más de uno = agujeros)
  outline: Vec<EdgeRef>,         // aristas que se dibujan como trazo
  style: Style,                  // relleno, trazo, grosor, puntas
}
Loop = Vec<EdgeRef>   EdgeRef { curve: CurveId, segment: usize, reversed: bool }
// Hide Edge (Etapa 2) = quitar la arista de `outline`; no hace falta una lista aparte.
```

- **Puntos compartidos:** una curva referencia puntos por id; dos curvas que pasan por el mismo `PointId`
  están **soldadas**. Soldar = reemplazar un id por otro.
- **Geometría y apariencia separadas:** las curvas sin forma no se renderizan (en el editor se ven como guía).
  Las herramientas tienen *Auto Fill* / *Auto Stroke* para crear la forma al dibujar, como Moho.
- Las manijas son de la **curva en ese punto** (un punto con dos curvas tiene manijas en cada una).
- Insertar o borrar un punto en una curva cambia los índices de segmento: el comando reajusta los `EdgeRef`.

## 4. Comandos (aia-core)

Todos devuelven su inverso; `Batch` agrupa y revierte si algo falla.

| Grupo | Comandos |
|---|---|
| Puntos | `MovePoints { layer, moves: Vec<(PointId, Point)> }`, `SetPointWidths { … }` |
| Curvas | `SetCurve { layer, curve }` (reemplaza nodos y manijas) |
| Estructura | `ReplaceMesh { layer, mesh }`: para soldar, insertar/borrar puntos, borrar aristas, dividir. El inverso es la malla anterior (simple y robusto; las mallas son chicas) |
| Capa (extra) | `RenameLayer`, `SetLayerVisible`, `SetLayerLocked`, `SetLayerOpacity` (separados en vez de un `SetLayerProps`) |
| Formas | `InsertShape`, `RemoveShape`, `SetShapeStyle`, `RenameShape`, `ReorderShape` |
| Capas | `InsertLayer { parent, index, layer }`, `RemoveLayer`, `MoveLayer { parent, index }`, `SetLayerTransform`, `SetLayerOrigin`, `SetLayerProps { name, visible, locked, opacity }` |
| Animación (Etapa 4) | `SetKeyframe`, `RemoveKeyframe`, `SetKeyInterp`… solo tocan `Document.animation` |

**Design** solo emite comandos de base; **Animation** solo emite comandos de animación. Así la regla
"Animation nunca toca la base" la garantiza el tipo de comando, no la disciplina de la UI.

## 5. Animación (Etapa 4, se define ahora para no rehacer el modelo)

```
Animation { mainline: Vec<Track>, actions: Vec<Action> }
Action { name, kind: Regular | Morph | SmartBone, tracks: Vec<Track> }
Track  { target: PropertyRef, keys: Vec<Keyframe> }
Keyframe { frame, value, interp: Smooth|Linear|EaseIn|EaseOut|EaseInOut|Step|Bezier|Cycle|…, hold, interval }
PropertyRef = PointPos(layer, point) | LayerTranslation(layer) | LayerRotation(layer) | BoneAngle(layer, bone) | ShapeFill(layer, shape) | …
```

- La base del documento es el "frame 0" de Moho. Una pista guarda los cambios **sobre** la base.
- `evaluate(doc, frame) -> Pose` calcula todo para un frame (pistas → huesos → posiciones finales);
  el render dibuja la `Pose`. En Design, `Pose` = base.
- Actions, morphs y Smart Bones son **conjuntos de pistas**: el mismo mecanismo sirve para todo.

## 6. Interfaz de la IA (aia-ai)

- **Vista de escena:** árbol de capas con nombres; cada forma como trazado SVG (`M … C … Z`) + estilo;
  grupos de puntos; huesos con nombre. Los ids de los puntos se omiten salvo que la herramienta los pida.
- **Herramientas por agente**, fijas para mantener el caché de prompt:
  - Draw: `crear_forma(capa, nombre, svg_path, estilo)` (crea puntos y curvas y suelda los coincidentes),
    `editar_forma`, `mover/escalar/rotar(objetivo)`, `estilo`, `crear_capa`, `agrupar_puntos`, `soldar`.
  - Rigging (Etapa 3+): `crear_hueso`, `atar_capa`, `atar_puntos`, `fuerza`, `restricciones`.
  - Animation (Etapa 4+): `keyframe`, `interpolacion`, `crear_action`, `insertar_action`.
- Cada llamada se traduce a un `Batch` de comandos: lo que hace la IA queda en el historial y se deshace
  en un paso. Además, captura del canvas para que la IA vea el resultado.

## 7. Herramientas (como Moho, con paleta según el tipo de capa)

Etapa 1 (capa vectorial y capas):

| Herramienta | Atajo (Moho) | |
|---|---|---|
| Select Points | G | marco, clic en punto / curva / relleno; Shift suma, Alt resta |
| Transform Points | T | mover / escalar / rotar puntos seleccionados (caja con manijas) |
| Add Point | A | dibujar curvas: en el vacío, desde un extremo, sobre un segmento |
| Draw Shape | E | rectángulo y óvalo (Shift proporción, Alt centro), con Auto Fill / Auto Stroke |
| Delete Edge | D | borra un segmento |
| Select Shape | Q | seleccionar formas para el estilo |
| Create Shape | U | seleccionar puntos del borde + Espacio = crear forma |
| Transform Layer | M | mover / rotar / escalar la capa |
| Set Origin | O | pivote de la capa |
| Pan / Zoom | — | como ahora (botón central / Espacio, rueda) |

Etapa 2 suma: ~~Curvature (C)~~ (adelantada, v2.2), Line Width (W), Hide Edge (H), Paint Bucket (P), Weld y Auto Weld, grupos
de puntos, manijas Bézier visibles, Magnet (X).

## 8. Etapas

1. **Base estilo Moho**: borrar núcleo y herramientas actuales; árbol de capas (Vector + Group) con
   transformación y origen; malla de puntos/curvas/formas; comandos y undo; render; herramientas de la
   tabla; panel de estilo y de capas en árbol; formato `.aia` v4 (sin migración).
2. **Dibujo completo**: soldadura, curvatura, grosor por punto, ocultar aristas, bote de pintura, grupos.
3. **Huesos**: capa de huesos, agregar/transformar huesos, binding flexi / region / capa, fuerza.
4. **Animación**: pistas, `evaluate`, línea de tiempo, keyframes, interpolaciones; huesos animados.
5. **IA**: configuración de proveedores y agentes; agente Draw con la vista de escena y sus herramientas.
6. Después: switch layers, actions, Smart Bones, morphs, onion skin, imágenes, exportar.

## 9. Decisiones (confirmadas el 2026-10-25)

1. **Animación en pistas separadas** de la base (no dentro de cada propiedad como en Moho). Recomendado:
   garantiza que Animation no toque la base y unifica actions/morphs/Smart Bones.
2. **Manijas:** modo `Auto` con curvatura (Moho clásico) por defecto + modos manuales. Recomendado.
3. **Operaciones estructurales con `ReplaceMesh`** (inverso = copia de la malla). Recomendado por simple.
4. **Relleno = contornos ordenados de aristas** guardados en la forma (no recalcular regiones en cada frame).
5. **¿Cuándo entra la IA?** Propuesta: Etapa 5, cuando ya haya algo que dibujar y animar. Se puede adelantar
   el agente Draw a después de la Etapa 2, porque es la diferencia principal con Moho.

## 10. Advertencias

- Detectar regiones cerradas (Paint Bucket) y booleanas requieren algoritmos de grafos planos: Etapa 2+.
- Grosor variable por punto: vello no lo hace; hay que generar el contorno del trazo como relleno.
- Cambiar el tipo de binding después de crear Smart Bones las rompe (igual que en Moho): habrá que avisarlo.
- Editar la base después de animar puede dejar pistas apuntando a puntos borrados: `evaluate` debe ignorarlas
  y la UI avisarlo.
