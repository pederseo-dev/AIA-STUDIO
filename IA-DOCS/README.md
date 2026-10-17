# IA-DOCS — Contexto del proyecto para la IA

Esta carpeta guarda todo lo necesario para que un chat nuevo de IA pueda retomar
el trabajo en AIA Studio sin perder contexto. **Leer en este orden:**

1. [IA-RULES.md](IA-RULES.md) — reglas de trabajo del usuario (obligatorias).
2. [ESTADO.md](ESTADO.md) — qué está hecho, qué falta y próximos pasos.
3. [ARQUITECTURA.md](ARQUITECTURA.md) — stack, crates y principios de diseño.
4. [DECISIONES.md](DECISIONES.md) — registro de decisiones y por qué se tomaron.
5. [FUENTES.md](FUENTES.md) — de dónde salió el conocimiento técnico (docs, código, skills).

## Cómo mantener esta carpeta

- Al cerrar cada etapa o tomar una decisión importante, actualizar `ESTADO.md` y `DECISIONES.md`.
- Cada vez que se consulte documentación, código fuente de un crate o una skill, anotarlo en `FUENTES.md`.
- Las reglas en `IA-RULES.md` las escribe el usuario; la IA no las modifica.
