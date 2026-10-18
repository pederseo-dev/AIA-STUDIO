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

- **Primera barra**: menús a la izquierda; a la derecha, botones para mostrar u ocultar la barra de modos (`Ctrl+M`), el panel lateral (`Ctrl+B`), el panel inferior (`Ctrl+J`) y el chat (`Ctrl+Alt+B`).
- **Segunda barra**: los dos modos — **Design** (`Ctrl+1`, dibujo y, más adelante, huesos) y **Animation** (`Ctrl+2`, nunca modifica la base).
- **Izquierda**: barra de íconos con las herramientas del modo (nombre y atajo al pasar el mouse) y panel lateral con la configuración.
- **Abajo**: panel general — mensajes en Design, línea de tiempo en Animation.
- **Derecha**: chat de IA con selector de agentes — **Draw** y **Rigging** en Design, **Animation** en Animation. Se elige solo según el modo y también se puede elegir a mano. Las conversaciones se guardan en el proyecto (la conexión con modelos llega después).
- La app recuerda el modo, los paneles abiertos y el tamaño de la ventana.

## Uso del modo Design (dibujo)

| Acción | Cómo |
|---|---|
| Selección / mover | `V`, clic para seleccionar, `Shift`+clic para sumar, arrastrar para mover, arrastrar en el vacío = marco |
| Escalar / rotar | Manijas de la caja de selección: esquinas y lados escalan (`Shift` = proporcional, `Alt` = desde el centro), círculo superior rota (`Shift` = de a 15°) |
| Transformar exacto | Panel lateral → Transformar: X, Y, ancho, alto, rotación, mantener proporción, voltear |
| Copiar / cortar / pegar / duplicar | `Ctrl+C` / `Ctrl+X` / `Ctrl+V` / `Ctrl+D` |
| Orden de apilado | `RePág` subir, `AvPág` bajar, `Inicio` al frente, `Fin` al fondo |
| Mover con teclado | Flechas (`Shift` = 10 unidades); en Editar nodos mueven los nodos seleccionados |
| Capas y formas | Panel lateral → Capas: doble clic renombra, clic derecho = menú (mover selección a la capa, subir, bajar, borrar) |
| Pluma | `P`: clic = nodo de esquina, arrastrar = nodo con curva, `Alt` = solo manija de salida, `Shift` = 45°, clic en el primer nodo = cerrar, `Enter`/`Esc`/doble clic = terminar |
| Editar nodos | `A`: clic en una forma, `Shift`+clic o marco para varios nodos, arrastrar nodos y manijas, clic sobre el trazado = agregar nodo, `Alt`+arrastrar manija = esquina, `Supr` = borrar nodo. Tipo de nodo (Esquina/Suave/Simétrico) y cerrar trazado en el panel lateral |
| Rectángulo | `R`, arrastrar (`Shift` = cuadrado, `Alt` = desde el centro) |
| Elipse | `E`, arrastrar (`Shift` = círculo, `Alt` = desde el centro) |
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
  aia-ai/      # base de configuración de IA por agente (sin conexión todavía)
  aia-app/     # ejecutable: nav de modos, canvas compartido, herramientas, paneles
```
