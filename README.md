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

## Uso del panel Dibujo

| Acción | Cómo |
|---|---|
| Selección / mover | `V`, clic para seleccionar, `Shift`+clic para sumar, arrastrar para mover |
| Rectángulo | `R`, arrastrar (`Shift` = cuadrado) |
| Elipse | `E`, arrastrar (`Shift` = círculo) |
| Borrar selección | `Supr` o `Retroceso` |
| Cancelar / deseleccionar | `Esc` |
| Zoom | Rueda del mouse |
| Mover la vista | Botón central, o `Espacio` + arrastrar |
| Encuadrar escenario | Menú Ver → Encuadrar escenario |
| Deshacer / Rehacer | `Ctrl+Z` / `Ctrl+Shift+Z` o `Ctrl+Y` |
| Nuevo / Abrir / Guardar | `Ctrl+N` / `Ctrl+O` / `Ctrl+S` (`Ctrl+Shift+S` = guardar como) |

Los proyectos se guardan como archivos `.aia` (JSON).

## Estructura

```
crates/
  aia-core/    # documento, comandos, undo/redo (sin UI ni GPU)
  aia-render/  # render del documento con vello sobre wgpu
  aia-ai/      # base de configuración de IA por panel (sin chat todavía)
  aia-app/     # ejecutable: ventana, paneles acoplables, herramientas
```
