# Estado del proyecto

_Última actualización: 2026-10-17_

## Visión

AIA Studio (Artificial Intelligence Animation Studio): aplicación de escritorio para
animación 2D **vectorial** profesional. Se arranca con herramientas básicas y se escala.
Tres paneles principales, **cada uno con su propio chat de IA**:

- **Dibujo**: formas vectoriales.
- **Huesos**: rigging 2D (jerarquía, IK, pesos), estilo Spine/Moho.
- **Animación**: línea de tiempo con keyframes y curvas.

No es objetivo (por ahora): animación tradicional cuadro a cuadro ni dibujo a mano alzada.

## v1 — Panel Dibujo básico ✅ (funcionando)

- Ventana con los 3 paneles acoplables (egui_dock). Huesos y Animación son placeholders.
- Canvas renderizado con vello sobre wgpu: pan (botón central o Espacio + arrastrar), zoom con rueda anclado al cursor.
- Herramientas: Selección/mover (V), Rectángulo (R), Elipse (E). `Shift` = cuadrado/círculo o sumar a la selección.
- Estilo: relleno y trazo (color, grosor). "Aplicar a la selección". Al seleccionar se carga el estilo de la forma.
- Capas: crear, borrar, ocultar, bloquear, elegir capa activa.
- Borrar selección (Supr/Retroceso), Esc cancela.
- Undo/Redo por comandos (Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y).
- Archivo: Nuevo, Abrir, Guardar, Guardar como (`.aia` en JSON). Avisa si hay cambios sin guardar (también al cerrar).
- Base de IA: tipos de configuración en `aia-ai` (proveedores, modelos y skills por panel). Sin UI ni chat.
- Tests: 9 (core: comandos, undo, JSON; ai: config por panel; render: cámara; app: rectángulo con Shift). Correr con `cargo test --workspace`.

## Problemas conocidos

- **Vulkan + Intel UHD crashea** (error de segmentación al iniciar). Se usa DX12 por defecto en Windows. Ver `DECISIONES.md`.
- Al arrancar maximizada, la barra de estado puede quedar detrás de la barra de tareas de Windows (pendiente de revisar).
- El lápiz a mano alzada se quitó a pedido del usuario (no se necesita en esta etapa).

## Próximos pasos (a debatir con el usuario antes de implementar)

1. **Herramienta Pluma**: crear y editar nodos Bézier (la herramienta vectorial principal).
2. Edición de nodos de formas existentes, transformar (escalar/rotar) con manijas.
3. Panel de configuración de IA (proveedores, modelos, skills por panel) y luego el chat del panel Dibujo con los `Command` como herramientas.
4. Panel Huesos.
5. Panel Animación (timeline).
