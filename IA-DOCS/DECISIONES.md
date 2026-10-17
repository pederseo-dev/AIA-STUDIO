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
