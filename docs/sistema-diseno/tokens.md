<!-- Generado por scripts/tokens.ts desde design/tokens/ (versión 1.0.0). No lo edites a mano:  cambia los tokens y ejecuta `npm run tokens` en el repo melomango. -->

# Tokens de diseño

La referencia de los tokens de Melomango, versión **1.0.0**: qué hay, cómo se llama cada uno
en CSS, en Kotlin y en Material 3, y sus valores. Este archivo se genera; el porqué está en
[color.md](color.md) y el formato y el flujo de trabajo en [design/tokens/](../../design/tokens/).

## Semánticos

Los que usan las apps. En escritorio, `var(--color-…)`; en el móvil, `MelomangoTokens.light` o
`MelomangoTokens.dark` (o el rol de Material 3 de `MaterialTheme.colorScheme`).

| Token | CSS | Kotlin | Material 3 | Claro | Oscuro | Para qué |
|---|---|---|---|---|---|---|
| `color.bg` | `--color-bg` | `bg` | `background`, `inverseOnSurface` | `#fffaf2` | `#17110b` | Fondo de la app. |
| `color.surface` | `--color-surface` | `surface` | `onTertiary`, `surface`, `onError`, `surfaceBright`, `surfaceContainerLowest` | `#ffffff` | `#221910` | Superficie: listas, tarjetas, barra del reproductor. |
| `color.surface.soft` | `--color-surface-soft` | `surfaceSoft` | `surfaceVariant`, `surfaceDim`, `surfaceContainerLow`, `surfaceContainer` | `#fff3df` | `#2c2014` | Superficie suave: fila al pasar el ratón, campos, paneles. |
| `color.surface.hover` | `--color-surface-hover` | `surfaceHover` | `primaryContainer`, `tertiaryContainer`, `surfaceContainerHigh` | `#ffeccc` | `#362818` | Lo elegido: la fila que suena, la entrada activa de la barra lateral. |
| `color.surface.raised` | `--color-surface-raised` | `surfaceRaised` | — | `#ffffff` | `#221910` | Diálogos, menús y hojas. |
| `color.border` | `--color-border` | `border` | `secondaryContainer`, `outline`, `outlineVariant`, `surfaceContainerHighest` | `#f2e2c6` | `#3d2d1c` | Bordes y separadores (decorativos). |
| `color.text` | `--color-text` | `text` | `onSecondaryContainer`, `onBackground`, `onSurface`, `inverseSurface` | `#2e1f0f` | `#f7eadb` | Texto principal. |
| `color.text.muted` | `--color-text-muted` | `textMuted` | `onSurfaceVariant` | `#7c674e` | `#b59d80` | Texto secundario: artista, duración, notas. Pasa AA en todas las superficies. |
| `color.text.disabled` | `--color-text-disabled` | `textDisabled` | — | `#b8a68f` | `#6b5a47` | Texto desactivado (sin requisito de contraste). |
| `color.accent` | `--color-accent` | `accent` | `primary`, `surfaceTint` | `#ffa21f` | `#ffa21f` | Relleno de acento: el botón principal, lo elegido. No para texto sobre claro. |
| `color.accent.deep` | `--color-accent-deep` | `accentDeep` | `secondary` | `#f07800` | `#ff8f1f` | Acento intenso: hover del acento, iconos de marca grandes. |
| `color.accent.text` | `--color-accent-text` | `accentText` | `onPrimaryContainer`, `tertiary`, `onTertiaryContainer` | `#ad5000` | `#ffb85a` | Texto e iconos naranjas sobre superficies claras. |
| `color.accent.on` | `--color-accent-on` | `accentOn` | `onPrimary`, `onSecondary` | `#2e1f0f` | `#17110b` | Texto e iconos sobre el acento o el degradado (oscuro: el blanco no llega a 3:1). |
| `color.accent.inverse` | `--color-accent-inverse` | `accentInverse` | `inversePrimary` | `#ffa21f` | `#ad5000` | Acento sobre una superficie invertida (el botón de un aviso). |
| `color.accent.gradient` | `--color-accent-gradient` | `accentGradient` | — | `#ffd25e` → `#ffa21f` → `#ff7a1a` | `#ffd25e` → `#ffa21f` → `#ff7a1a` | El degradado mango, a 135°: la firma de Melomango. |
| `color.focus` | `--color-focus` | `focus` | — | `#ad5000` | `#ffa21f` | Anillo de foco del teclado (3:1 o más sobre todas las superficies). |
| `color.focus.halo` | `--color-focus-halo` | `focusHalo` | — | `#ffa21f2e` | `#ffa21f2e` | Halo suave alrededor de un campo enfocado. |
| `color.state.hover` | `--color-state-hover` | `stateHover` | — | `#fff3df` | `#2c2014` | Fondo al pasar el ratón por algo que se puede pulsar. |
| `color.state.pressed` | `--color-state-pressed` | `statePressed` | — | `#ffeccc` | `#362818` | Fondo mientras se pulsa o de lo que está activo. |
| `color.success` | `--color-success` | `success` | — | `#2e7d32` | `#7fd08a` | Texto e icono de éxito. |
| `color.success.bg` | `--color-success-bg` | `successBg` | — | `#e8f5e9` | `#1f3323` | Fondo de un aviso de éxito. |
| `color.warning` | `--color-warning` | `warning` | — | `#8a5a00` | `#ffd27a` | Texto e icono de aviso. |
| `color.warning.bg` | `--color-warning-bg` | `warningBg` | — | `#fff4d6` | `#3a2a0c` | Fondo de un aviso. |
| `color.danger` | `--color-danger` | `danger` | `error`, `onErrorContainer` | `#c62828` | `#ff8a80` | Texto e icono de error. |
| `color.danger.bg` | `--color-danger-bg` | `dangerBg` | `errorContainer` | `#fdecea` | `#3d1f1c` | Fondo de un error. |
| `color.info` | `--color-info` | `info` | — | `#1f5f99` | `#8cc4ff` | Texto e icono informativo. |
| `color.info.bg` | `--color-info-bg` | `infoBg` | — | `#e7f1fb` | `#15263a` | Fondo de un aviso informativo. |
| `color.lyric.past` | `--color-lyric-past` | `lyricPast` | — | `#2e1f0f99` | `#f7eadb94` | Líneas de la letra ya cantadas: el texto al 60 %. |
| `color.lyric.current` | `--color-lyric-current` | `lyricCurrent` | — | `#a34b00` | `#ffb85a` | La línea que suena (más oscura que el texto naranja: va sobre el panel mango). |
| `color.lyric.future` | `--color-lyric-future` | `lyricFuture` | — | `#2e1f0f80` | `#f7eadb66` | Líneas que vienen: el texto al 50 % (3:1, texto grande). |
| `color.backdrop` | `--color-backdrop` | `backdrop` | `scrim` | `#3c230a47` | `#0000008c` | Velo detrás de un diálogo. |
| `color.decor.glow.top` | `--color-decor-glow-top` | `decorGlowTop` | — | `#ffe6b3` | `#3a240c` | Brillo de la parte de arriba del contenido. |
| `color.decor.glow.mid` | `--color-decor-glow-mid` | `decorGlowMid` | — | `#fff3df` | `#24180d` | Mitad del brillo del contenido, antes del fondo. |
| `color.decor.panel` | `--color-decor-panel` | `decorPanel` | — | `#fff3df` → `#ffe6b3` | `#24180d` → `#33200b` | Fondo de los paneles de letra y cola. |
| `color.decor.cover-soft` | `--color-decor-cover-soft` | `decorCoverSoft` | — | `#ffe6b3` → `#ffc46b` | `#5a3a12` → `#a8641a` | Portada de lo que aún no tiene imagen (playlist vacía, canción sin carátula…). |
| `color.decor.glass` | `--color-decor-glass` | `decorGlass` | — | `#ffffff8c` | `#ffffff12` | Cristal sobre el brillo (botones de la cabecera). |
| `color.decor.topbar` | `--color-decor-topbar` | `decorTopbar` | — | `#fffaf2d9` | `#17110bd9` | Barra superior translúcida. |
| `color.decor.plugin-badge` | `--color-decor-plugin-badge` | `decorPluginBadge` | — | `#ffe6b3` | `#4a3214` | Etiqueta de plugin. |
| `color.data.1` | `--color-data-1` | `data1` | — | `#b06d00` | `#e69f00` | Datos 1; jugador 1 del karaoke. |
| `color.data.2` | `--color-data-2` | `data2` | — | `#2a7fb8` | `#56b4e9` | Datos 2; jugador 2 del karaoke. |
| `color.data.3` | `--color-data-3` | `data3` | — | `#00805c` | `#009e73` |  |
| `color.data.4` | `--color-data-4` | `data4` | — | `#a6508a` | `#cc79a7` |  |
| `color.data.5` | `--color-data-5` | `data5` | — | `#8f1d1d` | `#e5534b` |  |
| `color.data.6` | `--color-data-6` | `data6` | — | `#24549c` | `#4c8fd6` |  |
| `shadow.card` | `--shadow-card` | — | — | `0 6px 24px #b464141f` | `0 6px 24px #00000059` | Sombra de tarjetas, listas y portadas. |

## Comunes a los dos temas

| Token | CSS | Kotlin | Valor | Para qué |
|---|---|---|---|---|
| `size.player-height` | `--size-player-height` | `MelomangoTokens.sizePlayerHeight` | `92px` | Alto de la barra del reproductor de escritorio. |
| `opacity.disabled` | `--opacity-disabled` | `MelomangoTokens.opacityDisabled` | `0.4` | Opacidad de un control desactivado. |

## Primitivos

La paleta en bruto, de la que eligen los semánticos. No se usa directamente en las apps.

| Rampa | Pasos |
|---|---|
| `mango` | 50 `#fff1d6` · 100 `#ffe6b3` · 200 `#ffc46b` · 300 `#ffb85a` · 500 `#ffa21f` · 600 `#ff8f1f` · 700 `#f07800` · 800 `#ad5000` · 900 `#a34b00` · sol `#ffd25e` · coral `#ff7a1a` |
| `crema` | 0 `#ffffff` · 50 `#fffaf2` · 100 `#fff3df` · 200 `#ffeccc` · 300 `#f2e2c6` · 400 `#b8a68f` · 600 `#7c674e` · 900 `#2e1f0f` |
| `tostado` | 50 `#f7eadb` · 300 `#b59d80` · 500 `#6b5a47` · 700 `#3d2d1c` · 750 `#362818` · 800 `#2c2014` · 850 `#221910` · 900 `#17110b` |
| `caramelo` | 500 `#a8641a` · 700 `#5a3a12` · 750 `#4a3214` · 800 `#3a240c` · 850 `#33200b` · 900 `#24180d` |
| `verde` | 50 `#e8f5e9` · 300 `#7fd08a` · 700 `#2e7d32` · 900 `#1f3323` |
| `ambar` | 50 `#fff4d6` · 300 `#ffd27a` · 700 `#8a5a00` · 900 `#3a2a0c` |
| `rojo` | 50 `#fdecea` · 300 `#ff8a80` · 700 `#c62828` · 900 `#3d1f1c` |
| `azul` | 50 `#e7f1fb` · 300 `#8cc4ff` · 700 `#1f5f99` · 900 `#15263a` |
| `datos` | claro.1 `#b06d00` · claro.2 `#2a7fb8` · claro.3 `#00805c` · claro.4 `#a6508a` · claro.5 `#8f1d1d` · claro.6 `#24549c` · oscuro.1 `#e69f00` · oscuro.2 `#56b4e9` · oscuro.3 `#009e73` · oscuro.4 `#cc79a7` · oscuro.5 `#e5534b` · oscuro.6 `#4c8fd6` |
| `blanco` | blanco `#ffffff` |
| `negro` | negro `#000000` |

## Contraste

Lo comprueba `npm test` con las parejas de [contraste.json](../../design/tokens/contraste.json): 4,5:1
para texto normal y 3:1 para texto grande, iconos y el foco. Los degradados cuentan con su parada peor.

| Tema | Texto o marca | Sobre | Contraste | Mínimo | Pasa |
|---|---|---|---|---|---|
| claro | `color.text` | `color.bg` | 15,32 | 4,50 | sí |
| claro | `color.text` | `color.surface` | 15,92 | 4,50 | sí |
| claro | `color.text` | `color.surface.soft` | 14,51 | 4,50 | sí |
| claro | `color.text` | `color.surface.hover` | 13,74 | 4,50 | sí |
| claro | `color.text` | `color.surface.raised` | 15,92 | 4,50 | sí |
| claro | `color.text.muted` | `color.bg` | 5,17 | 4,50 | sí |
| claro | `color.text.muted` | `color.surface` | 5,37 | 4,50 | sí |
| claro | `color.text.muted` | `color.surface.soft` | 4,90 | 4,50 | sí |
| claro | `color.text.muted` | `color.surface.hover` | 4,64 | 4,50 | sí |
| claro | `color.text.muted` | `color.surface.raised` | 5,37 | 4,50 | sí |
| claro | `color.accent.text` | `color.bg` | 5,15 | 4,50 | sí |
| claro | `color.accent.text` | `color.surface` | 5,35 | 4,50 | sí |
| claro | `color.accent.text` | `color.surface.soft` | 4,88 | 4,50 | sí |
| claro | `color.accent.text` | `color.surface.hover` | 4,62 | 4,50 | sí |
| claro | `color.accent.text` | `color.surface.raised` | 5,35 | 4,50 | sí |
| claro | `color.accent.on` | `color.accent` | 7,91 | 4,50 | sí |
| claro | `color.accent.on` | `color.accent.deep` | 5,60 | 4,50 | sí |
| claro | `color.accent.on` | `color.accent.gradient` | 6,10 | 4,50 | sí |
| claro | `color.accent.inverse` | `color.text` | 7,91 | 3,00 | sí |
| claro | `color.bg` | `color.text` | 15,32 | 4,50 | sí |
| claro | `color.focus` | `color.bg` | 5,15 | 3,00 | sí |
| claro | `color.focus` | `color.surface` | 5,35 | 3,00 | sí |
| claro | `color.focus` | `color.surface.soft` | 4,88 | 3,00 | sí |
| claro | `color.focus` | `color.surface.hover` | 4,62 | 3,00 | sí |
| claro | `color.success` | `color.success.bg` | 4,56 | 4,50 | sí |
| claro | `color.success` | `color.bg` | 4,93 | 4,50 | sí |
| claro | `color.warning` | `color.warning.bg` | 5,41 | 4,50 | sí |
| claro | `color.warning` | `color.bg` | 5,70 | 4,50 | sí |
| claro | `color.danger` | `color.danger.bg` | 4,92 | 4,50 | sí |
| claro | `color.danger` | `color.bg` | 5,41 | 4,50 | sí |
| claro | `color.danger` | `color.surface` | 5,62 | 4,50 | sí |
| claro | `color.info` | `color.info.bg` | 5,82 | 4,50 | sí |
| claro | `color.info` | `color.bg` | 6,40 | 4,50 | sí |
| claro | `color.lyric.current` | `color.bg` | 5,67 | 4,50 | sí |
| claro | `color.lyric.current` | `color.decor.panel` | 4,83 | 4,50 | sí |
| claro | `color.lyric.past` | `color.bg` | 4,24 | 3,00 | sí |
| claro | `color.lyric.past` | `color.decor.panel` | 3,99 | 3,00 | sí |
| claro | `color.lyric.future` | `color.bg` | 3,17 | 3,00 | sí |
| claro | `color.lyric.future` | `color.decor.panel` | 3,04 | 3,00 | sí |
| claro | `color.data.1` | `color.bg` | 4,01 | 3,00 | sí |
| claro | `color.data.1` | `color.surface` | 4,17 | 3,00 | sí |
| claro | `color.data.2` | `color.bg` | 4,19 | 3,00 | sí |
| claro | `color.data.2` | `color.surface` | 4,35 | 3,00 | sí |
| claro | `color.data.3` | `color.bg` | 4,76 | 3,00 | sí |
| claro | `color.data.3` | `color.surface` | 4,95 | 3,00 | sí |
| claro | `color.data.4` | `color.bg` | 4,89 | 3,00 | sí |
| claro | `color.data.4` | `color.surface` | 5,08 | 3,00 | sí |
| claro | `color.data.5` | `color.bg` | 8,56 | 3,00 | sí |
| claro | `color.data.5` | `color.surface` | 8,89 | 3,00 | sí |
| claro | `color.data.6` | `color.bg` | 7,16 | 3,00 | sí |
| claro | `color.data.6` | `color.surface` | 7,44 | 3,00 | sí |
| oscuro | `color.text` | `color.bg` | 15,83 | 4,50 | sí |
| oscuro | `color.text` | `color.surface` | 14,61 | 4,50 | sí |
| oscuro | `color.text` | `color.surface.soft` | 13,41 | 4,50 | sí |
| oscuro | `color.text` | `color.surface.hover` | 12,04 | 4,50 | sí |
| oscuro | `color.text` | `color.surface.raised` | 14,61 | 4,50 | sí |
| oscuro | `color.text.muted` | `color.bg` | 7,22 | 4,50 | sí |
| oscuro | `color.text.muted` | `color.surface` | 6,67 | 4,50 | sí |
| oscuro | `color.text.muted` | `color.surface.soft` | 6,12 | 4,50 | sí |
| oscuro | `color.text.muted` | `color.surface.hover` | 5,50 | 4,50 | sí |
| oscuro | `color.text.muted` | `color.surface.raised` | 6,67 | 4,50 | sí |
| oscuro | `color.accent.text` | `color.bg` | 10,93 | 4,50 | sí |
| oscuro | `color.accent.text` | `color.surface` | 10,09 | 4,50 | sí |
| oscuro | `color.accent.text` | `color.surface.soft` | 9,26 | 4,50 | sí |
| oscuro | `color.accent.text` | `color.surface.hover` | 8,32 | 4,50 | sí |
| oscuro | `color.accent.text` | `color.surface.raised` | 10,09 | 4,50 | sí |
| oscuro | `color.accent.on` | `color.accent` | 9,31 | 4,50 | sí |
| oscuro | `color.accent.on` | `color.accent.deep` | 8,20 | 4,50 | sí |
| oscuro | `color.accent.on` | `color.accent.gradient` | 7,18 | 4,50 | sí |
| oscuro | `color.accent.inverse` | `color.text` | 4,52 | 3,00 | sí |
| oscuro | `color.bg` | `color.text` | 15,83 | 4,50 | sí |
| oscuro | `color.focus` | `color.bg` | 9,31 | 3,00 | sí |
| oscuro | `color.focus` | `color.surface` | 8,60 | 3,00 | sí |
| oscuro | `color.focus` | `color.surface.soft` | 7,89 | 3,00 | sí |
| oscuro | `color.focus` | `color.surface.hover` | 7,08 | 3,00 | sí |
| oscuro | `color.success` | `color.success.bg` | 7,26 | 4,50 | sí |
| oscuro | `color.success` | `color.bg` | 10,07 | 4,50 | sí |
| oscuro | `color.warning` | `color.warning.bg` | 9,73 | 4,50 | sí |
| oscuro | `color.warning` | `color.bg` | 13,15 | 4,50 | sí |
| oscuro | `color.danger` | `color.danger.bg` | 6,52 | 4,50 | sí |
| oscuro | `color.danger` | `color.bg` | 8,20 | 4,50 | sí |
| oscuro | `color.danger` | `color.surface` | 7,57 | 4,50 | sí |
| oscuro | `color.info` | `color.info.bg` | 8,36 | 4,50 | sí |
| oscuro | `color.info` | `color.bg` | 10,21 | 4,50 | sí |
| oscuro | `color.lyric.current` | `color.bg` | 10,93 | 4,50 | sí |
| oscuro | `color.lyric.current` | `color.decor.panel` | 9,06 | 4,50 | sí |
| oscuro | `color.lyric.past` | `color.bg` | 5,90 | 3,00 | sí |
| oscuro | `color.lyric.past` | `color.decor.panel` | 5,35 | 3,00 | sí |
| oscuro | `color.lyric.future` | `color.bg` | 3,42 | 3,00 | sí |
| oscuro | `color.lyric.future` | `color.decor.panel` | 3,29 | 3,00 | sí |
| oscuro | `color.data.1` | `color.bg` | 8,31 | 3,00 | sí |
| oscuro | `color.data.1` | `color.surface` | 7,68 | 3,00 | sí |
| oscuro | `color.data.2` | `color.bg` | 8,12 | 3,00 | sí |
| oscuro | `color.data.2` | `color.surface` | 7,49 | 3,00 | sí |
| oscuro | `color.data.3` | `color.bg` | 5,47 | 3,00 | sí |
| oscuro | `color.data.3` | `color.surface` | 5,05 | 3,00 | sí |
| oscuro | `color.data.4` | `color.bg` | 6,12 | 3,00 | sí |
| oscuro | `color.data.4` | `color.surface` | 5,65 | 3,00 | sí |
| oscuro | `color.data.5` | `color.bg` | 5,06 | 3,00 | sí |
| oscuro | `color.data.5` | `color.surface` | 4,67 | 3,00 | sí |
| oscuro | `color.data.6` | `color.bg` | 5,54 | 3,00 | sí |
| oscuro | `color.data.6` | `color.surface` | 5,11 | 3,00 | sí |

## Colores de datos con daltonismo

La pareja que más se parece de los colores de datos, con visión normal y simulando cada tipo de
daltonismo (Machado 2009). Tiene que superar ΔE 10.

| Tema | Visión | Pareja más parecida | ΔE |
|---|---|---|---|
| claro | normal | `color.data.2` y `color.data.6` | 22,4 |
| claro | protanopia | `color.data.2` y `color.data.4` | 15,1 |
| claro | deuteranopia | `color.data.2` y `color.data.6` | 15,8 |
| claro | tritanopia | `color.data.2` y `color.data.3` | 11,1 |
| oscuro | normal | `color.data.2` y `color.data.6` | 19,1 |
| oscuro | protanopia | `color.data.3` y `color.data.5` | 13,7 |
| oscuro | deuteranopia | `color.data.2` y `color.data.6` | 13,9 |
| oscuro | tritanopia | `color.data.3` y `color.data.6` | 12,8 |
