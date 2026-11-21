# Auditoría de la interfaz de escritorio y móvil

KAN-85, primera tarea del sistema de diseño común (épica KAN-84). Recoge lo que hay hoy en las dos
apps para que las demás tareas partan de datos: los tokens (KAN-87), el color (KAN-88), la
tipografía (KAN-89), los componentes (KAN-92) y las migraciones (KAN-97 a KAN-100). Los principios y
el tono de los textos están en [principios.md](principios.md).

**Qué se ha mirado** (21/11/2026):

- **Escritorio:** 0.13.0, rama `emisoras` (`62227be`), ya con la pantalla Emisoras. `src/App.css`
  tiene 2.949 líneas y 1.480 declaraciones, más los `.tsx`.
- **Móvil:** `melomango-mobile` en `9e9d502` (0.1.0-dev), con los 23 archivos de `ui/`.
- **Valores contados** con [`scripts/design-inventory.mjs`](../../scripts/design-inventory.mjs).
  Se puede volver a pasar para medir cómo avanza la migración:
  `node scripts/design-inventory.mjs [ruta a melomango-mobile]`.
- **Capturas:** escritorio en modo dev a 1200×780; móvil en el emulador (1080×2400, con la región
  en Estados Unidos, por eso las emisoras son de allí). Las dos apps parten de una instalación
  limpia y tienen la misma canción sonando.

## Lo más importante

1. **Tipografía y formas son las de cada plataforma, no las de Melomango.** Escritorio usa Segoe
   UI Variable con 19 tamaños de letra escritos a mano. El móvil usa Roboto y la escala por
   defecto de Material 3. Los títulos no se parecen: un título de sección mide 17 px en escritorio
   y 22 sp en el móvil.
2. **El naranja se usa de dos formas.** Escritorio rellena con el degradado mango y escribe en
   blanco encima, con un contraste de 1,4 a 2,6:1, que no llega al mínimo. El móvil usa el mango
   plano con texto oscuro encima (7,9:1). Lo que diga KAN-88 tiene que valer para las dos.
3. **El texto atenuado del tema claro no llega a AA:** `--muted` sobre el fondo da 4,21:1 y se usa
   57 veces.
4. **Errores claros:** el aviso del tema oscuro de escritorio no se lee; las carátulas de YouTube
   salen con franjas negras en las dos apps; y el fondo de error del modo oscuro se ha desviado
   entre las dos (`#3d1f1c` frente a `#3B1A16`). Están todos en [Diferencias](#4-diferencias).
5. **La paleta del móvil es una copia parcial a mano:** le faltan el degradado, los colores de
   éxito y los fondos secundarios. Es justo lo que tiene que resolver la fuente única de tokens
   (KAN-87).

## 1. Valores

### Color

**Paleta compartida.** El móvil la copia de `App.css` en `ui/theme/Color.kt` y la reparte en los
roles de Material 3 (`Theme.kt`).

| Token (escritorio) | Claro | Oscuro | En el móvil |
|---|---|---|---|
| `--bg` | `#fffaf2` | `#17110b` | `background`; igual |
| `--surface` | `#ffffff` | `#221910` | `surface`, `surfaceContainerLowest`; igual |
| `--surface-soft` | `#fff3df` | `#2c2014` | `surfaceVariant`, `surfaceContainer`(`Low`); igual |
| `--surface-hover` | `#ffeccc` | `#362818` | `primaryContainer`, `surfaceContainerHigh`; igual |
| `--border` | `#f2e2c6` | `#3d2d1c` | `outline`, `surfaceContainerHighest`, `secondaryContainer`; igual |
| `--text` | `#2e1f0f` | `#f7eadb` | `onBackground`, `onSurface`, `onPrimary` (claro); igual |
| `--muted` | `#8c7558` | `#b59d80` | `onSurfaceVariant`; igual |
| `--accent` | `#ffa21f` | `#ffa21f` | `primary` (`MangoRipe`); igual |
| `--accent-deep` | `#f07800` | `#ff8f1f` | `secondary`; igual |
| `--accent-text` | `#b45300` | `#ffb85a` | `tertiary`; igual |
| `--danger` | `#c62828` | `#ff8a80` | `error`; igual |
| `--danger-bg` | `#fdecea` | **`#3d1f1c`** | `errorContainer`: **`#3B1A16` en oscuro** |
| `--success`, `--success-bg` | `#2e7d32`, `#e8f5e9` | `#7fd08a`, `#1f3323` | no existe |
| `--mango-gradient` | `#ffd25e → #ffa21f → #ff7a1a` | igual | no existe |
| `--content-glow`, `--panel-bg`, `--cover-soft` | degradados crema | degradados tostados | no existen |
| `--glass`, `--topbar-bg`, `--backdrop` | transparencias | transparencias | no existen (Material) |
| `--lyric-future`, `--lyric-past` | `--text` al 38 % y 55 % | al 38 % y 58 % | `onSurface` al 45 % para las dos (no distingue las pasadas) |
| `--plugin-badge` | `#ffe2b8` | `#4a3214` | no aplica (sin plugins) |

**Escritos fuera de las variables en escritorio:** 19 colores en 29 sitios.

- `#fff` para el texto y los iconos sobre el degradado: botón de reproducir, botón principal,
  pestaña de fuente, portadas generadas, logo vacío de emisora. Son 7 usos, más el pulgar del
  interruptor.
- `#2b1d0e` para el texto sobre `--accent` (pestaña de búsqueda y control segmentado). Es casi
  `--text`, pero no es ninguna variable.
- **Sombras con color propio:** 15 valores `rgba(...)` distintos de cuatro familias: naranja
  (240,120,0), tostado (180,100,20 y 120,60,0), hueso (46,31,15) y negro. Ninguna cambia con el
  tema.
- `.toast` usa `background: var(--text)` con `color: #fff`: en oscuro es texto blanco sobre crema
  (ver errores).

**Móvil:** fuera de `theme/` solo aparece `Color.Transparent` (9 veces). Usa 14 roles de
`MaterialTheme.colorScheme`. El más usado es `onSurfaceVariant` (el atenuado), con 49 usos, seguido
de `primary` (13) y `tertiary` (11).

**Escritorio:** la variable más usada es `--muted` (57 usos), seguida de `--border` (38),
`--surface` (27) y `--accent-text` (22).

**Contraste de los pares que más se ven** (WCAG; AA pide 4,5:1 para texto normal y 3:1 para texto
grande e iconos). Es un adelanto para KAN-88:

| Par | Contraste | |
|---|---:|---|
| `--muted` sobre `--bg` (claro) | 4,21 | **no llega** para texto pequeño |
| `--muted` sobre `--surface-soft` (claro) | 3,99 | **no llega** |
| `--accent-text` sobre `--bg` (claro) | 4,84 | sí |
| `--accent-text` sobre `--surface-hover` (claro, fila que suena) | 4,34 | **no llega** |
| blanco sobre `--accent` | 2,01 | **no llega** (botones de escritorio) |
| blanco sobre el inicio del degradado (`#ffd25e`) | 1,44 | **no llega** |
| `--text` sobre `--accent` (móvil, claro) | 7,91 | sí |
| `--accent` como texto sobre `--bg` (claro) | 1,94 | solo vale para decoración |
| `--muted` sobre `--bg` (oscuro) | 7,22 | sí |
| `--accent-text` sobre `--bg` (oscuro) | 10,93 | sí |
| `--danger` sobre `--danger-bg` | 4,92 (claro) · 6,52 (oscuro) | sí |

### Tipografía

| | Escritorio | Móvil |
|---|---|---|
| Fuente | `"Segoe UI Variable", "Segoe UI", Inter, system-ui`, base 15 px | Roboto (la del sistema, por defecto de Material 3) |
| Tamaños | **19 distintos** escritos a mano: 0,72 · 0,75 · 0,78 · 0,8 · 0,82 · 0,84 · 0,85 · 0,86 · 0,88 · 0,9 · 1,05 · 1,15 · 1,25 · 1,35 · 1,4 · 1,6 · 1,8 · 2 · 2,6 rem | Ningún `.sp` a mano: 11 estilos de `MaterialTheme.typography` (`bodySmall` ×22, `bodyMedium` ×10, `bodyLarge` ×7, `titleLarge` ×6, `titleSmall` ×6, `headlineMedium` ×5…) |
| Más usados | 0,8 rem ×14 · 0,85 rem ×12 · 0,78 rem ×8 · 0,82 rem ×7 | `bodySmall` (12 sp) ×22 |
| Pesos | 600 ×25 · 700 ×14 · 400 ×3 · 800 ×3 · 500 ×1 | `Bold` ×22 · `SemiBold` ×12 · `Medium` ×1 |
| Interlineado | 6 valores (1,1 a 1,6) | el de cada estilo de Material |

De 0,72 a 0,9 rem hay diez tamaños que se distinguen por menos de un píxel. Por ejemplo, 0,84,
0,85 y 0,86 rem dan 12,6, 12,75 y 12,9 px. El móvil, en cambio, ya va por estilos con nombre: le
falta que esos estilos tengan los tamaños de Melomango.

### Radios

| Escritorio (10 valores, 77 usos) | Móvil (29 usos) |
|---|---|
| 6 px ×2 · 8 px ×11 · 10 px ×13 · 12 px ×9 · 14 px ×8 · 16 px ×5 · 18 px ×1 · 20 px ×2 · 50 % ×11 · 999 px ×15 | `CircleShape` ×11 · 6 dp ×8 · 12 dp ×2 · 28 dp ×2 · 8, 10, 14, 16, 24 dp ×1 · 0 dp ×1; el resto, las formas de Material (botones de píldora, tarjetas de 12 dp, hojas de 28 dp) |

### Espaciado

| Escritorio (padding, margin y gap: 19 valores, 290 usos) | Móvil (dp en padding, `spacedBy` y `Spacer`) |
|---|---|
| 1 ×3 · 2 ×18 · 3 ×1 · 4 ×32 · 5 ×4 · 6 ×43 · 7 ×3 · 8 ×48 · 10 ×44 · 12 ×35 · 14 ×14 · 16 ×16 · 18 ×10 · 20 ×1 · 22 ×3 · 24 ×7 · 26 ×1 · 28 ×5 · 32 ×2 (px) | 4 ×15 · 6 ×4 · 8 ×33 · 10 ×1 · 12 ×11 · 16 ×38 · 20 ×2 · 24 ×5 |

El móvil ya sigue casi del todo la rejilla de 4 (4, 8, 12, 16 y 24). Escritorio mezcla pasos de 2
(6, 10, 14, 18, 22, 26) con valores sueltos (3, 5, 7).

**Tamaños fijos en el móvil:** carátulas de 44, 48, 52 y 56 dp en las filas, y de 72, 96 y 132 dp
en las cabeceras (132 dp) y en Tu resumen. En escritorio: 44 px en las filas, 58 px en el reproductor
y 148 px en las cabeceras.

### Sombras y elevación

- **Escritorio:** 20 sombras distintas en 25 usos. Solo `--shadow` (4 usos) cambia con el tema; las
  demás van a mano, casi todas con un brillo naranja bajo los elementos de mango.
- **Móvil:** elevación tonal de Material (1, 2 y 3 dp) y una sombra de 6 dp.

### Movimiento

- **Escritorio:** 9 duraciones. Para los cambios de estado: 0,12 s ×8, 0,15 s ×4, 0,18 · 0,2 ·
  0,25 s. Para animaciones en bucle: 0,8, 1,1, 1,6 y 2,4 s. `prefers-reduced-motion` se respeta en
  los interruptores y en el punto de «En directo», pero no en el resto.
- **Móvil:** las animaciones por defecto de Compose y Material, sin ningún `tween` propio.

### Iconos

| | Escritorio | Móvil |
|---|---|---|
| Juego | 35 iconos propios en `Icons.tsx`, casi todos trazados de Material copiados como SVG | 46 de `material-icons` (`Icons.Default.*`, `Icons.AutoMirrored.*`) |
| Tamaños | 12 ×2 · 14 ×7 · 16 ×34 · 18 ×30 · 20 ×15 · 22 ×3 · 64 ×4 (px) | 24 dp por defecto; también 12, 18, 36, 52, 56 y 72 dp |
| Diferencias | «Importar» es una flecha a una bandeja; Tu resumen, unas barras | «Importar» es `Input`; Tu resumen, `ShowChart`; el corazón relleno se usa también para el estado de la fila |

## 2. Componentes

| Pieza | Escritorio | Móvil | Nota |
|---|---|---|---|
| Fila de canción | `TrackRow`: número, carátula, título y artista (enlaces), género, corazón, menú +, duración; arrastrar con Alt+↑/↓ | `TrackRow` / `DraggableTrackRow`: carátula, título, «artista · duración», corazón pequeño si gusta, menú ⋮ | Densidad de cada plataforma; misma información |
| Carátula | `Artwork`; sin imagen, el degradado mango vacío | `Artwork`; sin imagen, `surfaceContainerHigh` con una nota musical | Carátula vacía distinta; las dos con franjas negras en las miniaturas de YouTube |
| Portada de colección | `PlaylistCover`: Me gusta (corazón blanco sobre degradado) o mosaico 2×2 | `PlaylistCover`: Me gusta en **naranja plano con corazón oscuro** | Error de identidad |
| Cabecera de colección | `.collection-header`: portada de 148 px, antetítulo en mayúsculas, título de 2,6 rem, botones | `CollectionHeader`: portada de 132 dp, antetítulo con mayúscula inicial | Mismo diseño a otra escala; el antetítulo no |
| Botón principal | `.btn.primary`: degradado, texto blanco y sombra naranja | `PrimaryButton` (Material `Button`): mango plano, texto oscuro | Ver KAN-88 |
| Botón secundario | `.btn.ghost`: superficie con borde | `SecondaryButton` (`OutlinedButton`) | Parecidos |
| Control segmentado | `.segmented`: el elegido en mango con texto `#2b1d0e` | `SegmentedButton`: el elegido en `surfaceHover` con ✓ | Distintos |
| Pestañas | `.search-tab` (píldoras; la activa en mango) | `PrimaryTabRow` y `PrimaryScrollableTabRow` (subrayado) | De plataforma, con el mismo color |
| Buscador | `SearchBox` con sugerencias y recientes | `SearchScreen` con `Suggestions` | Parecidos |
| Reproductor | `PlayerBar` (barra fija abajo) y `MiniPlayer` (ventana siempre encima) | `MiniPlayer` encima de la barra inferior y `NowPlayingScreen` (Sonando, Letra, Cola) | De plataforma |
| Directo (emisoras) | `PlayerBar` y `MiniPlayer` con «En directo» | `LiveMiniPlayer`, `LiveNowPlaying` | Hecho igual en las dos (KAN-160) |
| Letra | `LyricsPanel`, con el desfase ±0,5 s | `LyricsView`, sin el desfase (KAN-42) | Falta en el móvil |
| Cola | `QueuePanel`, con la radio automática | `QueueView`; la radio automática está en Ajustes | Sitio distinto |
| Menú de canción | `AddToPlaylistMenu` (menú emergente) | `TrackMenuSheet` (hoja inferior) | De plataforma |
| Añadir a playlist | dentro del menú | `AddToPlaylistDialog` | De plataforma |
| «No es esta canción» | `MatchDialog` | `MatchDialog` | Igual |
| Confirmar | `ask()` de Tauri (diálogo del sistema) | `ConfirmDialog` (Material) | Escritorio no se ve como Melomango |
| Aviso | `.toast` propio | `Snackbar` | Ver error 1 |
| Error | `ErrorNotice` (Detalles y Reintentar) | `ErrorNotice` | Igual |
| Estado vacío y de carga | `<p className="status">` | `StatusText` | Igual |
| Título de sección | `<h2>` en `.catalog-section` | `SectionTitle` | Tamaños distintos |
| Tarjetas (álbum, artista) | `CardGrid` / `.album-card` | `CardRow` / `CardItem` | Rejilla frente a fila deslizable |
| Navegación | `Sidebar` + `Atrás` (Alt+←) | `BottomBar` (Inicio, Buscar, Biblioteca) + `TopBar` | De plataforma |
| Biblioteca | en la barra lateral | pantalla `LibraryScreen` con `Entry` | De plataforma |
| Filtro y orden | `TrackTools`: campo y desplegable «Ordenar por» | `TrackTools`: campo e icono de orden | Igual función |
| Emisoras | `StationsView`, `StationRow`, `CountryDialog` | `StationsScreen`, `StationRow`, `CountryDialog` | Igual (KAN-162, KAN-164) |
| Historial y Tu resumen | `HistoryView`, `SummaryView` | `HistoryScreen`, `SummaryScreen` | Igual |
| Descargas | `DownloadsView` | `DownloadsScreen` | Igual |
| Importar | `ServicesDialog` (diálogo) | `ImportScreen` (pantalla) | De plataforma |
| Ajustes | `SettingsDialog` (diálogo) | `SettingsScreen` (pantalla) | De plataforma; opciones distintas |
| Actualización | `UpdateCard` | `UpdateCard` | Igual |
| Solo escritorio | `PluginsDialog`, `YouTubeSetup`, `ShortcutsDialog`, barra de fuentes, bandeja y Discord | — | No aplican al móvil |
| Solo móvil | — | `IconCover` (iconos de Biblioteca), botón «Aleatorio» en las cabeceras | El «Aleatorio» de las cabeceras le falta a escritorio |

## 3. Capturas

Escritorio a la izquierda y móvil a la derecha, a la misma altura. El minirreproductor de
escritorio va a su tamaño real.

| | Claro | Oscuro |
|---|---|---|
| Inicio | ![Inicio, claro](capturas/inicio-claro.webp) | ![Inicio, oscuro](capturas/inicio-oscuro.webp) |
| Búsqueda | ![Búsqueda, claro](capturas/busqueda-claro.webp) | ![Búsqueda, oscuro](capturas/busqueda-oscuro.webp) |
| Artista | ![Artista, claro](capturas/artista-claro.webp) | ![Artista, oscuro](capturas/artista-oscuro.webp) |
| Álbum | ![Álbum, claro](capturas/album-claro.webp) | ![Álbum, oscuro](capturas/album-oscuro.webp) |
| Lista (Me gusta) | ![Me gusta, claro](capturas/lista-me-gusta-claro.webp) | ![Me gusta, oscuro](capturas/lista-me-gusta-oscuro.webp) |
| Reproductor | ![Reproductor, claro](capturas/reproductor-claro.webp) | ![Reproductor, oscuro](capturas/reproductor-oscuro.webp) |
| Letra | ![Letra, claro](capturas/letra-claro.webp) | ![Letra, oscuro](capturas/letra-oscuro.webp) |
| Cola | ![Cola, claro](capturas/cola-claro.webp) | ![Cola, oscuro](capturas/cola-oscuro.webp) |
| Minirreproductor y Biblioteca | ![Minirreproductor, claro](capturas/minirreproductor-biblioteca-claro.webp) | ![Minirreproductor, oscuro](capturas/minirreproductor-biblioteca-oscuro.webp) |
| Emisoras | ![Emisoras, claro](capturas/emisoras-claro.webp) | ![Emisoras, oscuro](capturas/emisoras-oscuro.webp) |
| Ajustes | ![Ajustes, claro](capturas/ajustes-claro.webp) | ![Ajustes, oscuro](capturas/ajustes-oscuro.webp) |

## 4. Diferencias

### Errores, que se corrigen

| # | Qué | Dónde | Tarea |
|---|---|---|---|
| 1 | **El aviso no se lee en oscuro:** texto `#fff` sobre `var(--text)`, que en oscuro es `#f7eadb` (1,2:1) | escritorio, `.toast` | KAN-97 (o antes: es una línea) |
| 2 | **Franjas negras en las carátulas de YouTube:** se pide `i.ytimg.com/vi/<id>/hqdefault.jpg`, que es 4:3 y trae el vídeo 16:9 con franjas negras dentro. Se ve en Inicio, en la cola, en el reproductor y en Sonando | las dos (plugin de YouTube `main.rs` y `YouTubeSource.kt`) | KAN-92 (carátula): recortar o pedir la miniatura 16:9 (`mqdefault`) |
| 3 | Fondo de error del modo oscuro: `#3d1f1c` frente a `#3B1A16` | móvil, `Color.kt` | KAN-87 |
| 4 | Portada de Me gusta en naranja plano con corazón oscuro, en vez del degradado con corazón blanco | móvil, `PlaylistCover` | KAN-100 |
| 5 | Portadas generadas distintas: sin imagen, escritorio pone el degradado mango (sin icono en las canciones) y el móvil `surfaceContainerHigh` con una nota (también para artistas y emisoras) | las dos, `Artwork` | [principios.md](principios.md#identidad) fija cuál; KAN-98 y KAN-100 |
| 6 | Dos degradados en escritorio: `--mango-gradient` y otro de dos paradas (`--accent → --accent-deep`) en las cabeceras de Historial, Tu resumen, Descargas y Emisoras y en artista o álbum sin foto | escritorio, `.history-cover`, `.artist-photo.placeholder` | KAN-97 |
| 7 | Antetítulo de las cabeceras: «COLECCIÓN» en mayúsculas espaciadas frente a «Colección» | las dos | KAN-89 / KAN-92 |
| 8 | Control segmentado: el elegido va en mango en escritorio y en `surfaceHover` con ✓ en el móvil | las dos | KAN-92 |
| 9 | Texto atenuado en claro por debajo de AA (4,21:1 sobre el fondo, 3,99:1 sobre `surface-soft`) | las dos | KAN-88 |
| 10 | Texto blanco sobre el degradado (1,4–2,6:1) en botones y pestañas | escritorio | KAN-88 |
| 11 | El título de la fila que suena (`--accent-text` sobre `--surface-hover`) da 4,34:1 | las dos | KAN-88 |
| 12 | Diez tamaños de letra que se distinguen por menos de un píxel (0,72–0,9 rem) | escritorio | KAN-89 |
| 13 | Sombras de color fijas, que no cambian con el tema (15 `rgba` distintos) | escritorio | KAN-90 |
| 14 | `#2b1d0e` escrito a mano para el texto sobre el mango | escritorio | KAN-87 |
| 15 | Al volver del minirreproductor, la ventana creció de 1200×780 a 1216×819 (está por confirmar en la versión instalada: parece que se guarda el tamaño exterior y se repone como interior) | escritorio, `mini.rs` | tarea aparte |
| 16 | El botón «Aleatorio» de las cabeceras de álbum y lista solo existe en el móvil | escritorio | KAN-98 |
| 17 | El desfase de la letra solo existe en escritorio | móvil | KAN-42 (ya existe) |
| 18 | La caché no tiene la opción «No» en el móvil | móvil | KAN-43 (ya existe) |
| 19 | `prefers-reduced-motion` solo se respeta en dos sitios | escritorio | KAN-94 |

### De cada plataforma, que se quedan

- **Navegación:** barra lateral con Atrás y Alt+← en escritorio; barra inferior con Inicio, Buscar
  y Biblioteca, y la flecha atrás del sistema, en el móvil.
- **Densidad:** las filas de escritorio llevan número, género, corazón y duración en columnas. Las
  del móvil juntan «artista · duración» y dejan el resto en el menú ⋮.
- **Menús:** emergentes en escritorio y hojas inferiores en el móvil. Ajustes e Importar son
  diálogos en escritorio y pantallas en el móvil.
- **Reproductor:** barra fija con letra y cola en paneles laterales (y minirreproductor en otra
  ventana) en escritorio. En el móvil, mini sobre la barra inferior y pantalla completa con
  Sonando, Letra y Cola.
- **Hover y atajos:** solo en escritorio (el número de la fila pasa a ▶; Q, Y, L, M…). Gestos y
  pulsación larga, solo en el móvil.
- **Pestañas:** píldoras en escritorio, pestañas subrayadas en el móvil. El color del elemento
  activo debe ser el mismo.
- **Opciones de cada plataforma:** fundido, normalizar el volumen, bandeja, Discord y plugins en
  escritorio; radio automática en Ajustes en el móvil.
- **Fuente del sistema:** Segoe UI Variable en Windows y Roboto en Android pueden quedarse si la
  escala de tamaños y pesos es la misma (lo decide KAN-89).

### Por decidir

- **El mango con texto blanco o con texto oscuro** (KAN-88): el oscuro pasa AA y el blanco no.
- **Un juego de iconos o dos** (KAN-91): escritorio ya copia los trazos de Material, así que
  podría usar los mismos 46 del móvil.
- **Dónde va la radio automática:** en la cola (escritorio) o en Ajustes (móvil).
