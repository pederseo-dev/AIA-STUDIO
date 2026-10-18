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

## Interfaz

- **Barra superior**: menús y los tres modos — **Draw** (`Ctrl+1`), **Bones** (`Ctrl+2`), **Timeline** (`Ctrl+3`).
- **Izquierda**: barra de íconos con las herramientas del modo (nombre y atajo al pasar el mouse) y panel lateral con la configuración (`Ctrl+B` lo oculta).
- **Derecha**: chat de IA del modo activo, próximamente (`Ctrl+Alt+B` lo oculta).
- La app recuerda el modo, los paneles abiertos y el tamaño de la ventana.

## Uso del modo Draw

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
  aia-ai/      # base de configuración de IA por modo (sin chat todavía)
  aia-app/     # ejecutable: nav de modos, canvas compartido, herramientas, paneles
```
