# Referencia: Moho (Lost Marble)

Resumen de cómo funciona Moho para usarlo como referencia de funcionalidades en AIA Studio.
**No es una lista de cosas a implementar**: cada punto se debate antes (ver IA-RULES).

_Armado el 2026-10-25 a partir del manual oficial de Moho 13.5 y de la documentación de
scripting Lua (mohoscripting.com). Ver la sección [Fuentes](#fuentes)._

> **Sobre "Lua":** Lua no es la documentación de Moho, es el lenguaje de *scripting* que usa Moho
> (herramientas y menús hechos por la comunidad). Su API es útil porque muestra el **modelo de datos
> interno** de Moho. Las *funcionalidades* están en el manual de usuario.

---

## 1. Ideas centrales de Moho

1. **Geometría y apariencia separadas.** Las herramientas de *Dibujo* crean puntos y curvas (invisibles
   en el render). Las herramientas de *Relleno* crean "shapes" (relleno y/o contorno) sobre esas curvas.
   Borrar un relleno no borra los puntos.
2. **Puntos compartidos y soldadura (weld).** Varias curvas pasan por el mismo punto; soldar une puntos
   para que se muevan juntos (nariz soldada a la cara). Todo es "puntos + curvas": un círculo, un texto
   o un trazo a mano son lo mismo.
3. **Frame 0 = setup, frame > 0 = animación.** En el frame 0 se arma el personaje (dibujo, huesos,
   binding). En cualquier otro frame, cualquier cambio crea un keyframe. Muchas herramientas de huesos
   solo existen en el frame 0.
   → *Equivale a nuestra separación Design / Animation.*
4. **Casi todo es animable.** Posición de cada punto, curvatura, grosor de línea por punto, colores,
   transformación de capa, visibilidad, orden de formas, parámetros de efectos, hueso padre, IK…
   Cada propiedad animable es un **canal** con sus keyframes.
5. **Paleta de herramientas inteligente:** las herramientas visibles dependen del tipo de capa activa
   (vector, hueso, imagen…) y del frame actual.
6. **Herramientas sobre la selección de puntos:** casi todas las herramientas de dibujo trabajan sobre
   los puntos seleccionados; si hay menos de dos seleccionados, sobre el más cercano al clic.

---

## 2. Modelo de datos (según la API Lua)

| Clase | Qué es | Datos importantes |
|---|---|---|
| `MohoDoc` | Documento | capas, tamaño, fps, rango de frames, estilos guardados |
| `MohoLayer` | Capa base (todas heredan) | `fTranslation`, `fScale` (Vec3), `fRotationX/Y/Z`, `fShear`, `fFlipH/V`, `Origin`, `fVisibility`, `fAlpha`, `fBlur`, sombra/shading/motion blur, máscara, hueso padre, **acciones** |
| `M_Mesh` | Contenido de una capa vectorial | lista de **puntos**, **curvas**, **shapes** y **grupos de puntos** |
| `M_Point` | Punto de control | `fAnimPos` (animado), `fParent` (hueso al que está atado; -1 libre, -2 flexi), `fWidth` (grosor de línea en ese punto, animable), color por punto, opacidad, oculto |
| `M_Curve` | Curva que pasa por varios puntos | `fClosed`, curvatura/manijas Bézier por punto (animables), `fStartPercent`/`fEndPercent` (exposición del trazo, animable), perfil de curva |
| `M_Shape` | Relleno y/o contorno | conjunto de **aristas** (segmentos de curvas), `fHasFill`, `fHasOutline`, `fMyStyle` + hasta 2 **estilos heredados**, nombre, oculto, `fComboMode` (Liquid Shapes: normal/sumar/restar/intersectar), efectos |
| `M_Style` | Estilo | color de relleno y de línea (animables), grosor, puntas redondeadas, **pincel** (brush) con jitter/espaciado, efectos (sombra, halo, borde suave, gradiente) |
| `M_PointGroup` | Grupo de puntos con nombre | para reseleccionar ("ojo_izq") |
| `BoneLayer` → `M_Skeleton` | Esqueleto de una capa de huesos | huesos, grupos de huesos, solver IK, modo de binding |
| `M_Bone` | Hueso | `fParent`, `fLength`, `fAnimAngle/Pos/Scale` (animables), `fStrength` (región de influencia), restricciones min/max de ángulo, ángulo independiente, squash & stretch, target IK, *control bones* (un hueso maneja ángulo/posición/escala de otro), dinámica (resorte/amortiguación/torque), flip, pesos (14.0) |
| `SwitchLayer` | Capa que muestra un hijo a la vez | canal de cadena con el nombre del hijo activo |
| `AnimChannel` | Canal animado (base) | keyframes por frame; tipos `AnimVal`, `AnimVec2`, `AnimVec3`, `AnimColor`, `AnimBool`, `AnimString` |
| `InterpSetting` | Ajuste de un keyframe | modo de interpolación, `hold` (frames de espera), `interval` (animar "en dos/en tres"), `stagger`, etiqueta de color, `val1`/`val2` (parámetros del modo) |

**Tipos de capa** (`LT_*`): vector, imagen, grupo, hueso, switch, partículas, nota, 3D, audio, patch, texto.

**Modos de interpolación** (`INTERP_*`): lineal, smooth, ease in/out, ease in, ease out, step, noisy,
cycle, pose, Bézier, bounce, elastic (+ stagger como opción).

**Modos de máscara** (`MM_*`): enmascarado, no enmascarado, sumar a la máscara, restar de la máscara
(también en versión invisible) y limpiar + sumar.

**Binding de huesos:** *flexi* (todos los huesos influyen en todos los puntos según la distancia y la fuerza)
y *region* (solo influyen los huesos en cuya región está el punto). Además está el binding directo
(punto → un hueso) y el binding de capa entera a un hueso.

### Comparación con el modelo de AIA Studio

| Moho | AIA Studio hoy | Nota |
|---|---|---|
| Puntos compartidos entre curvas + shapes como conjuntos de aristas | Cada `Shape` tiene su propio `VectorPath` | Decisión 2026-10-18: modelo tipo SVG por ser mejor para la IA. La soldadura de Moho **no** se puede copiar tal cual. |
| Grosor por punto (`fWidth`) | Grosor único por forma | Es la Fase B (grosor variable). Costoso: vello no lo hace directamente. |
| Curvatura por punto + manijas Bézier opcionales | Manijas Bézier por nodo (Esquina/Suave/Simétrico) | El nuestro se parece al modo Bézier de Moho Pro 13.5+. |
| Capas con transformación propia (posición, escala, rotación, origen) | Las capas no tienen transformación; la tienen las formas | A tener en cuenta para la Fase C (grupos y pivote). |
| Estilos guardados que se aplican a muchas formas | Estilo copiado en cada forma | Posible mejora (paleta/estilos compartidos). |
| Canales animados por propiedad | Sin animación todavía | Es la base del modo Animation. |

---

## 3. Herramientas de dibujo (capa vectorial)

| Herramienta | Atajo | Qué hace | Notas |
|---|---|---|---|
| Select Points | G | Selecciona puntos: marco, clic en punto, clic en curva (toda la curva), clic en relleno (todo el objeto). Shift suma, Alt resta, Ctrl = lazo | Opciones: grupos de puntos con nombre, **Weld Crossings** (soldar cruces), **Simplify**, **Split** (agregar puntos intermedios), voltear H/V |
| Transform Points | T | Mueve/escala/rota los puntos seleccionados (o el más cercano) | Caja roja con manijas; Shift+Alt en lado = mantener volumen (squash & stretch); Ctrl+flechas para empujar; **auto-weld** al soltar un extremo sobre otro punto |
| Add Point | A | Herramienta principal para dibujar curvas: clic+arrastre en vacío = curva nueva; en el extremo = continuarla; sobre un segmento = insertar punto; en un punto medio = rama soldada | Alt = forzar curva nueva; opciones auto-weld, auto-fill, auto-stroke, esquinas agudas (Ctrl invierte) |
| Curvature | C | Arrastrar izquierda/derecha = más aguda/más redonda la curva en los puntos | Botones Peak / Smooth |
| Freehand | F | Dibujo a mano alzada que se simplifica al soltar | Presión de tableta o ancho aleatorio, afinar inicio/fin, suavizado, reducción de puntos, auto-weld en cruces, recortar puntas sobrantes, auto-cerrar, unir trazos |
| Draw Shape | E | Rectángulo, óvalo, triángulo, estrella, flecha, espiral, grilla | Shift = proporción, Alt = desde el centro. Lo creado son puntos comunes, nada especial |
| Delete Edge | D | Borra un segmento de curva sin borrar los puntos | Si era parte de un relleno, el relleno se borra |
| Magnet (Pro) | X | Mueve puntos con caída suave dentro de un radio | Útil con muchos puntos (freehand, SVG importado) |
| Blob Brush | — | Pincel que genera una forma rellena; trazos sucesivos se suman | Ctrl = borrar; reducción de puntos |
| Eraser | — | Hace agujeros en formas rellenas | |
| Point Reduction | — | Pincel que simplifica los puntos por donde pasa | Ángulo de tolerancia |
| Scatter Brush | — | "Rocía" copias de una forma (preset o portapapeles) | Variación de ángulo, espaciado, color, tamaño |
| Perspective / Shear / Bend Points (Pro) | — | Deforman la selección de puntos (perspectiva, sesgo, curvar) | Animables |
| Noise (Pro) | N | Desordena los puntos al azar | Para variar copias |

**Booleanas en Moho 13.5:** se superponen formas, *Weld Crossings* y luego *Delete Edge* para quitar lo que
sobra. En Moho 14 aparecen las **Liquid Shapes** (combinar formas en vivo: sumar, restar, intersectar, mezclar).

**Manijas Bézier (Pro 13.5+):** "Show Bezier Handles" las muestra; Alt al arrastrar una manija = esquina.
"Fixed Bezier Handles" off (por defecto) = las manijas se reajustan solas al mover un punto (más "blandito",
mejor para animar); on = estilo Illustrator (no se mueven los vecinos).

## 4. Herramientas de relleno y estilo

| Herramienta | Atajo | Qué hace |
|---|---|---|
| Select Shape | Q | Selecciona rellenos/contornos; Shift suma, Alt resta; Ctrl+↑/↓ = la forma de abajo/arriba en ese punto |
| Create Shape | U | Se seleccionan los puntos del borde y Espacio crea el relleno (tablero = vista previa de la región) |
| Paint Bucket | P | Clic dentro de una región cerrada = relleno (no hace falta que las curvas estén soldadas; sirve para booleanas) |
| Delete Shape | — | Borra el relleno/contorno, no los puntos |
| Line Width | W | Grosor de línea **por punto** (en %), con modo imán |
| Hide Edge | H | Oculta segmentos del contorno sin tocar el relleno |
| Stroke Exposure (Pro) | — | Cuánto del trazo se ve (inicio/fin %), animable: escritura a mano, fluidos |
| Curve Profile (Pro) | — | Usa otra curva como perfil repetido a lo largo del borde (detalle sin más puntos) |
| Color Points | — | Color por punto que se mezcla sobre la forma (gradiente por puntos) |
| Eyedropper | — | Copia el estilo de una forma; Alt = "empujarlo" a otras |

**Ventana de estilo:** relleno y trazo (color con alfa, hex), pincel para el trazo, puntas redondeadas,
**dos efectos de relleno** (sombreado, borde suave, halo, gradiente lineal/radial/reflejado/angular animable
y que sigue a los huesos, textura de imagen, sombra, crayón), efectos de trazo, **estilos guardados**
(hasta 2 aplicados por forma, con "override" por propiedad), muestras de color (swatches), copiar/pegar estilo.
Modos: DEFAULTS (para formas nuevas), SHAPE (forma seleccionada), STYLE (estilo guardado).

## 5. Huesos

**Estructura:** una *capa de huesos* contiene el esqueleto y las capas hijas (vectores, imágenes) que este
deforma. Jerarquía padre→hijos; los huesos no tienen por qué tocarse. Nombres automáticos B1, B2…

| Herramienta | Qué hace |
|---|---|
| Select Bone | Selecciona (marco/lazo); nombre, color, etiqueta visible, bloquear, **shy bones** (ocultables), flip de extremo/lateral (animable) |
| Bone Constraints (diálogo) | Límites de ángulo min/max, **ángulo independiente** (el hijo no rota con el padre: pies paralelos al piso), **squash & stretch**, estiramiento IK máximo, solver IK en arco, ignorado por IK, **target** (IK), **control bones** (ángulo/posición/escala de un hueso manejan otro), dinámica (resorte, amortiguación, torque) |
| Add Bone | Clic+arrastre = hueso (hijo del seleccionado); clic = **pin bone** (largo cero, para caras). Se encadenan solos; Shift = 45° |
| Transform Bone | Mover (base), rotar (cerca), escalar (punta), con campos numéricos y "mostrar trayectoria" |
| Manipulate Bones | En frame 0 prueba el rig (temporal); en otro frame anima la cadena completa (IK) |
| Sketch Bones | Dibuja una cadena de huesos a mano alzada (cola, cuello, soga); en frames > 0 la reacomoda |
| Reparent Bone | Cambia el padre |
| Bone Strength | Ajusta la región de influencia de cada hueso |
| Offset Bone | Dibujar partes separadas y "llevarlas a su lugar" desde el frame 1 |
| Bind Layer | Ata una capa entera a un hueso (mano al antebrazo) |
| Bind Points | Ata puntos concretos a un hueso (Moho recomienda mejor binding automático + Strength) |
| Bone Physics (Pro) | Regiones de colisión de los huesos dentro de un grupo con física |

**Smart Bones (Pro):** se asocia una *acción* a la rotación de un hueso (una por sentido). Al rotar el
hueso, la acción se aplica proporcionalmente: corrige deformaciones de articulaciones (codo, rodilla,
bulto del músculo) o funciona como **dial de control** (hueso con fuerza 0 que maneja parpadeo,
expresión, giro de cabeza). Pueden manejar: posición de puntos, curvatura, grosor, colores, transformación
y opacidad de capas, otros huesos, switch layers, orden de capas, visibilidad, flip, seguir trayectoria.
Advertencia del manual: **cambiar el tipo de binding después rompe las Smart Bones**; primero se termina el rig.

**Otros comandos de huesos:** liberar/flexi-bind puntos y capas, usar solo los huesos seleccionados para el
flexi-binding de una capa, **articulación suave para un par de huesos**, ocultar huesos, *Make Smart Bone
Dial* (crea la smart bone automáticamente), congelar pose, resetear huesos.

**Moho 13.5 – Vitruvian Bones:** varios juegos de huesos + dibujos en una misma capa de huesos, y se cambia
cuál está activo (Alt+D / Alt+C): por ejemplo, el mismo brazo dibujado en distintas perspectivas.
**Wind Dynamics:** viento sobre huesos con dinámica.

## 6. Capas

**Herramientas de capa:** Transform Layer (mover, rotar, escalar; Alt = mover en Z; escala manteniendo volumen;
mostrar y editar trayectoria; alinear capas), **Set Origin** (pivote de la capa), **Follow Path** (la capa sigue
una curva; Alt = se curva a lo largo de ella; "rotar siguiendo el camino"), Rotate Layer XY (3D real), Shear
Layer, **Layer Selector** (clic en el dibujo selecciona su capa), Insert Text (con globos de diálogo), Eyedropper.

**Tipos de capa:**
- **Vector** (lo principal) · **Imagen** (PNG ideal; también video) · **Secuencia de imágenes** (dentro de un switch).
- **Grupo** (carpeta; transformación y efectos aplicados a todo lo de adentro).
- **Huesos** (grupo con esqueleto).
- **Switch**: muestra un solo hijo a la vez. Se usa para **lip-sync** (bocas Preston Blair: Rest, Etc, FV, L,
  MBP, O, E, AI, U, WQ; datos de Papagayo o análisis de audio), ojos, manos, frame by frame. Si todos los hijos
  son vectoriales con la misma cantidad de puntos, puede **interpolar** entre ellos.
- **Frame by frame**, **Partículas** (cantidad, vida, velocidad, dispersión, aceleración/gravedad…),
  **Nota** (post-it que no se renderiza), **Audio**, **Patch** (tapa la unión entre dos partes del cuerpo),
  **3D** (OBJ), **Texto**.

**Ajustes de capa (Layer Settings):** general (opacidad, blur, color de capa, no renderizar / solo render,
rotar siguiendo el camino), sombras, motion blur, **máscara** (una capa define la máscara del grupo: sumar,
restar, enmascarado/no enmascarado), vectores, orden por profundidad, imagen, audio, switch, partículas,
física, **huesos** (tipo de binding).

**Ventana de capas:** buscar, columnas, **layer comps** (combinaciones guardadas de visibilidad), colores de
capa, **referencias de capa** (instancias sincronizadas con un original), ajustes rápidos.

## 7. Cámara y espacio de trabajo

- **Cámara (animable, 3D real):** Track (mover; Alt = adelante/atrás → paralaje por la profundidad Z de las
  capas), Zoom (campo de visión), Roll, Pan/Tilt.
- **Espacio de trabajo (solo la vista):** Pan (botón derecho), Zoom (Shift+botón derecho), Rotate Workspace
  (rotar la hoja para dibujar), Orbit (ver la escena desde afuera de la cámara). `Home` = resetear la vista.

## 8. Línea de tiempo y animación

- **Timeline de la capa activa** (o de varias): un **canal por propiedad** que aparece recién cuando tiene keys.
  Se pueden ocultar canales, **silenciarlos (mute)**, separar X/Y/Z de un canal (por ejemplo, rebote en Y con
  cycle + traslado en X).
- **Keyframes:** se crean solos al cambiar algo en un frame > 0; doble clic = agregar; copiar/pegar (entre capas y
  documentos), **pegar relativo**, mover, seleccionar los de la derecha, etiquetas de color, **hold** (mantener N
  frames, se ve como "píldora"), **interval** (animar en dos/en tres), editar varios a la vez.
- **Interpolaciones:** smooth, linear, ease in/out, ease in, ease out, **Bézier** (manijas en el gráfico), step, noisy,
  **cycle** (volver a un frame: ciclos), bounce, elastic, stagger, "copiar la anterior"; hay una interpolación por defecto.
- **Vistas:** canales, **secuenciador** (bloques de capa para correrlos en el tiempo; audio/video), **Motion Graph**
  (curvas de valores; X rojo, Y verde, Z azul).
- **Onion skin:** hasta 8 frames marcados (relativos o fijos), transparencia o solo contorno (rojo antes, verde
  después), solo la capa activa o todas.
- **Marcadores** del documento o de capa; reproducir un segmento; rango de la animación; 24 fps por defecto.
- **Actions:** clips de animación reutilizables (parpadeo, ciclo de caminata). Se insertan **por referencia** (si se
  edita la action, cambia en todos lados) o **por copia**; atajos de teclado; exportar/importar; "convertir a
  relativa" para otro personaje con las mismas proporciones de huesos. Tipos: regulares, **morphs** (pose de un solo
  frame que se mezcla con otras en *Blend Morphs*: feliz + enojado) y **smart bones**.
- **Comandos de animación:** *Freeze Pose* / **Auto-Freeze Keyframes** (al tocar una parte, todos los huesos reciben
  key, así la pose no "se arrastra" desde el frame 0), reescalar keyframes, copiar frame, borrar animación desde el
  frame actual, poner la capa en el tiempo.

## 9. Otras funciones

- **Importar:** SVG/AI, PSD (capas), imágenes, video, audio, OBJ, Poser. **Exportar:** video/imágenes, SVG de un frame,
  FBX, guardar como versiones anteriores.
- **Trace Image** (vectorizar), **Triangulate 2D Mesh**, ocultar/mostrar puntos y formas, **freeze points**.
- **Character Wizard** (personaje armado con rig y caminata).
- **Video tracking** (puntos que siguen un video y mueven capas).
- **Física** (grupos con física, colisiones, motores).
- **Smart Warp / Perspective warp / Quad warp** (13.5): deformar una capa con una malla (por ejemplo, una grilla).
- **Scripts Lua:** herramientas y menús propios (`LM`, `LM.GUI`, `MOHO`).

---

## 10. Candidatos para debatir en AIA Studio

Ordenados por cercanía con lo que ya tenemos. Cada uno se debate antes de implementarlo.

**Dibujo (Design, Fase B/C)**
1. Seleccionar puntos de varias formas a la vez y transformarlos juntos (Moho trabaja todo sobre puntos).
2. Curvatura con arrastre (Peak/Smooth) como alternativa rápida a las manijas.
3. **Grosor de línea por nodo** (Line Width): ⚠ vello no hace trazos de grosor variable; habría que generar el
   contorno como relleno.
4. Booleanas (Liquid Shapes / Weld + Delete Edge): ⚠ kurbo no trae booleanas; requiere una librería.
5. Ocultar segmentos del contorno (Hide Edge) y **exposición del trazo** (Stroke Exposure): encajan bien con
   nuestro `VectorPath` (segmentos indexados).
6. Grupos de nodos con nombre ("parpado_sup"): muy útiles para la IA y para los huesos.
7. Estilos guardados y swatches; gradientes que sigan a los huesos.
8. Freehand / Blob Brush / Point Reduction: lo descartamos (sin dibujo a mano alzada); se podría reabrir.
9. Imán (Magnet) y Noise: deformadores simples.

**Capas**
10. Transformación y **origen (pivote) de capa**, grupos de capas (Fase C).
11. **Switch layers** (lip-sync, ojos, manos): ⚠ define cómo se organiza el personaje; conviene decidirlo antes de los huesos.
12. Máscaras de capa, layer comps, referencias de capa.

**Huesos (Design → herramienta Hueso + agente Rigging)**
13. Huesos con jerarquía, pin bones, fuerza/región de influencia, flexi vs region binding, binding de capa entera.
14. Restricciones de ángulo, ángulo independiente, IK con targets, squash & stretch.
15. **Smart Bones** (correcciones y diales de control): ⚠ dependen de tener actions y del binding definitivo.

**Animación (modo Animation + agente Animation)**
16. Canales por propiedad, keyframes con interpolación (smooth/linear/ease/step/Bézier/cycle), hold, interval.
17. Onion skin, motion graph, secuenciador.
18. Actions reutilizables (referencia/copia) y morphs mezclables: encajan con la IA ("hacé que parpadee").
19. Auto-Freeze de pose.

**Advertencias de diseño**
- Nuestro modelo es "nodos por forma"; el de Moho es "puntos compartidos + soldadura". Las funciones de Moho que
  dependen de la soldadura (Weld, Delete Edge, Paint Bucket sobre curvas sueltas) **no se trasladan directo**: hay
  que reinterpretarlas o reabrir la decisión del 2026-10-18.
- En Moho la transformación vive en la capa y en los puntos; en AIA vive en cada forma. Antes de los huesos conviene
  decidir dónde vive el pivote (forma, grupo o capa).
- Smart Bones, morphs y actions requieren un sistema de canales animables genérico: conviene diseñarlo una sola vez
  para todas las propiedades (como `AnimChannel` de Moho).

---

## Fuentes

- **Manual de usuario de Moho 13.5** (oficial, PDF): https://lostmarble.com/manual/13.5/Moho%20Users%20Manual.pdf
- **Manual de tutoriales de Moho 13.5** (oficial, PDF): https://lostmarble.com/manual/13.5/Moho%20Tutorial%20Manual.pdf
- Copias locales (no van a git): `IA-DOCS/fuentes/moho/` (PDF + texto UTF-8 para buscar con grep).
- Manual de Moho 14 (wiki online, necesita JavaScript; no se pudo leer automáticamente): https://manual.lostmarble.com/
  · Tutoriales: https://tutorial.lostmarble.com/ · Índice de manuales: https://lostmarble.com/support/kb/faq.php?id=8
- **API de scripting Lua** (no oficial, mantenida por la comunidad, cubre hasta Moho 14): https://mohoscripting.com/
  — clases consultadas: `M_Mesh`, `M_Point`, `M_Curve`, `M_Shape`, `M_Style`, `M_Bone`, `M_Skeleton`, `MohoLayer`,
  `AnimChannel`, `InterpSetting` y las constantes del módulo `MOHO`.

## Verificado en Moho Pro (2026-10-25, probando la app)

Detalles que el manual no explica, comprobados manejando Moho con mouse simulado:

- **Add Point, clic sin arrastrar en el vacío:** no crea nada que quede. Mientras el botón está apretado se
  ve un punto rojo temporal, pero al soltar sin arrastrar desaparece. Para empezar una curva hay que
  arrastrar (crea dos puntos).
- **Delete Edge, clic justo sobre un punto en medio de una curva:** no borra el punto. Borra uno de los dos
  segmentos que llegan a él (el de un lado) y el punto queda como punta de la curva. El punto del otro
  extremo del segmento borrado, que queda sin ninguna curva, desaparece.
- **Delete Edge al pasar el mouse:** no resalta nada (en AIA sí se resalta el segmento).
- Opciones de Add Point en la barra: Select Group, Auto-weld, Auto-fill, Auto-stroke, Sharp corners.
- **Transform Points, pivote:** con los puntos seleccionados se ve una mira con un círculo (al principio en
  el centro de la caja de selección), distinta de la cruz con flechas del origen de la capa.
  - Se arrastra directamente para moverla; los puntos no se mueven.
  - **La rotación** gira alrededor del pivote. Se rota arrastrando en la franja entre la caja de manijas y un
    segundo rectángulo exterior (no hay manija de rotación aparte).
  - **La escala** con una esquina no usa el pivote: el ancla es la esquina opuesta; el pivote se transforma
    junto con la selección.
  - El pivote no vuelve al centro con Deshacer ni al deseleccionar y volver a seleccionar: queda donde se dejó.
  - Barra de opciones de Transform Points: Select Group, Reset, Position X/Y, Scale X/Y, Angle, Auto-weld,
    Auto-fill, Auto-stroke, Show/Fixed Bezier Handles, Flip H/V.
  - Ayuda en la barra de estado: "Move/Scale/Rotate selected points (press <enter> to weld, hold <shift> to
    constrain, <alt> to disable auto-welding, <alt> to scale to center, <shift> while scaling to squash,
    <ctrl/cmd> to select points)".
