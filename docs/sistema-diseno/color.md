# Color

KAN-88. La paleta de Melomango en claro y oscuro, con el contraste comprobado. Los valores y la
tabla completa de roles (CSS, Kotlin y Material 3) están en [tokens.md](tokens.md), que se genera de
[design/tokens/](../../design/tokens/); aquí está el porqué. Parte de la [auditoría](auditoria.md) y
de los [principios](principios.md): cálido, no chillón.

## Rampas

| Rampa | Para | Notas |
|---|---|---|
| `mango` 50–900, `sol`, `coral` | El acento | El 500 (`#ffa21f`) es el mango de la marca. `sol` y `coral` son los extremos del degradado. Del 800 hacia abajo, naranja que se lee sobre crema |
| `crema` 0–900 | Neutros del tema claro | Crema de fondo (50), pulpa (100) y hueso de mango para el texto (900) |
| `tostado` 50–900 | Neutros del tema oscuro | Hueso tostado de fondo (900), texto crema (50) |
| `caramelo` | Brillos y portadas suaves en oscuro | |
| `verde`, `ambar`, `rojo`, `azul` | Éxito, aviso, error, información | 50 y 900 de fondo; 700 y 300 de texto |
| `datos` | Gráficas y jugadores del karaoke | De Okabe-Ito, ajustados por tema |

Los pasos no son todos los de 50 a 900: solo existen los que se usan, con sitio para añadir más.

## Lo que cambia respecto a lo que había

| Qué | Antes | Ahora | Por qué |
|---|---|---|---|
| Texto atenuado en claro | `#8c7558` (4,21:1) | `#7c674e` (4,64:1 o más) | AA en todas las superficies claras, también en la fila que suena |
| Naranja de texto en claro | `#b45300` (4,34:1 sobre la fila que suena) | `#ad5000` (4,62:1 o más) | Lo mismo |
| La línea de la letra que suena, en claro | el naranja de texto | `#a34b00` | Va sobre el panel mango, más oscuro que la crema |
| Líneas de la letra que vienen, en claro | texto al 38 % (2,3:1) | al 50 % (3:1) | Texto grande: 3:1 como mínimo |
| Líneas que vienen, en oscuro | al 38 % | al 40 % | Igual en los dos temas |
| Texto sobre el mango | blanco en escritorio (1,4–2,6:1), oscuro en el móvil | **oscuro en las dos** (`color.accent.on`, 5,6:1 o más en todo el degradado) | El blanco no llega ni a 3:1 |
| Foco del teclado en claro | el mango (1,9:1) | el naranja de texto | 3:1 sobre todas las superficies |
| Fondo de error en oscuro | `#3d1f1c` en escritorio, `#3B1A16` en el móvil | `#3d1f1c` | Se había desviado |
| `onSecondary` en el móvil | blanco sobre `#f07800` (2,8:1) | `color.accent.on` | Lo mismo que el mango |
| Aviso e información | no existían | `color.warning`, `color.info`, con su fondo | Los cuatro estados |
| Estados de interacción | colores a ojo en cada regla | `color.state.hover`, `color.state.pressed`, `color.focus`, `opacity.disabled` | Definidos una vez |
| Colores de datos | no existían | `color.data.1` a `color.data.6` | Resumen mensual y karaoke (KAN-67): el 1 y el 2, un jugador cada uno |

**Ya se ven:**
- **Escritorio** (sus nombres de siempre son alias de los tokens): el atenuado, el naranja de texto y
  las líneas de la letra que vienen y las ya cantadas.
- **Móvil** (todo el `ColorScheme` sale de los tokens): el atenuado, el naranja de texto, el fondo de
  error, `onSecondary`, `onError` y el velo de los diálogos.

**Llegan con la migración:**
- **Escritorio (KAN-97):** aún escribe en sus reglas el blanco sobre el mango y el foco con valores
  propios. Empezará a usar `--color-accent-on`, `--color-focus`, `--color-lyric-current` y los
  estados al pasarse a los nombres de los tokens.
- **Móvil (KAN-99):** la letra, que hoy pinta las líneas con `onSurface` al 45 %, pasará a
  `MelomangoTokens.*.lyricPast`, `.lyricCurrent` y `.lyricFuture`.

## Roles

Los nombres semánticos dicen para qué es cada color, no cómo es. La tabla completa, con su nombre
en CSS, en Kotlin y en Material 3, está en [tokens.md](tokens.md#semánticos). Resumidos:

| Para | Token | Material 3 |
|---|---|---|
| Fondo de la app | `color.bg` | `background` |
| Superficies: normal, suave, elegida, elevada | `color.surface`, `.soft`, `.hover`, `.raised` | `surface`, `surfaceContainer`, `surfaceContainerHigh`, `surfaceContainerLowest` |
| Borde | `color.border` | `outline`, `outlineVariant` |
| Texto: principal, secundario, desactivado | `color.text`, `.muted`, `.disabled` | `onSurface`, `onSurfaceVariant` |
| Acento: relleno, intenso, como texto, encima | `color.accent`, `.deep`, `.text`, `.on` | `primary`, `secondary`, `tertiary`, `onPrimary` |
| Degradado mango | `color.accent.gradient` | (`MelomangoTokens.*.accentGradient`) |
| Foco | `color.focus`, `.halo` | (indicador de foco) |
| Estados | `color.success`, `.warning`, `.danger`, `.info`, con `.bg` | `error`, `errorContainer` |
| Letra | `color.lyric.past`, `.current`, `.future` | — |
| Velo de los diálogos | `color.backdrop` | `scrim` |
| Datos | `color.data.1`–`.6` | — |

**Reglas de uso:**

- **Texto naranja:** siempre `color.accent.text`, nunca `color.accent`. Sobre crema, el mango da
  1,9:1.
- **Encima del mango o del degradado:** `color.accent.on`, oscuro en los dos temas.
- **Un estado se muestra con su color y algo más** (un icono o un texto), nunca solo con el color.
- **Colores de datos:** en orden. Para dos series, el 1 y el 2: naranja y azul, que se distinguen con
  cualquier tipo de daltonismo.

## Cómo se comprueba

`npm test` y la CI:

- **Contraste:** cada pareja de [contraste.json](../../design/tokens/contraste.json) en los dos
  temas. El texto con transparencia se mezcla antes con el fondo, y en los degradados cuenta su
  parada peor.
- **Colores de datos:** se distinguen entre sí (ΔE 10 o más) con visión normal y simulando
  protanopía, deuteranopía y tritanopía (Machado 2009).
- **Los dos temas** tienen los mismos tokens, todas las referencias existen y cada rol de Material 3
  apunta a un color.

Los resultados de cada pareja están en [tokens.md](tokens.md#contraste).
