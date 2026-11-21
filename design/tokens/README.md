# Tokens de diseño

La única fuente de los colores y medidas de Melomango, para escritorio y para el móvil (KAN-87).
Si algo de la apariencia se repite en las dos apps, sale de aquí; nunca se copia a mano.

## Qué hay

| Archivo | Qué es |
|---|---|
| `manifest.json` | La versión de los tokens y qué archivo es cada cosa |
| `primitivos.tokens.json` | La paleta en bruto (mango, crema, tostado, verde, ámbar, rojo, azul, datos). Las apps no la usan directamente |
| `semanticos.claro.tokens.json`, `semanticos.oscuro.tokens.json` | Para qué se usa cada color en cada tema (`color.text.muted`, `color.accent.on`…). Los dos tienen las mismas claves |
| `componentes.tokens.json` | Lo que no cambia con el tema y se usa en más de un sitio (`size.player-height`, `opacity.disabled`) |
| `material3.json` | Qué token semántico va en cada rol del `ColorScheme` de Material 3 del móvil |
| `contraste.json` | Las parejas cuyo contraste se comprueba, y los colores que deben distinguirse con daltonismo |

El formato es el estándar W3C Design Tokens (DTCG): `$value`, `$type`, `$description` y referencias
como `{mango.500}`. Las claves llamadas `default` son el valor del grupo: `color.text.default` se
llama `color.text`. Los degradados guardan su ángulo en `$extensions.com.melomango.angle`.

## Dónde vive y cómo llega a cada app

Viven aquí, en el repo de escritorio, y el móvil recibe un archivo generado. Es lo más sencillo que
deja **una sola copia y una versión** (`manifest.json`) sin paquetes ni submódulos: las dos apps las
hace la misma persona y el móvil ya toma de aquí `docs/emisoras.md` y `docs/sistema-diseno/`.

`npm run tokens` (es `scripts/tokens.ts`, sin dependencias) genera:

| Generado | Para |
|---|---|
| `src/tokens.css` | Escritorio: variables CSS en claro y oscuro, con `prefers-color-scheme` y `data-theme` como siempre. `App.css` las usa |
| `design/generated/MelomangoTokens.kt` | Móvil: `MelomangoTokens.light` y `.dark` (colores), las medidas y los dos `ColorScheme` de Material 3 |
| `docs/sistema-diseno/tokens.md` | La referencia: cada token con su nombre en CSS, Kotlin y Material 3, sus valores y el contraste |

Para el móvil: `npm run tokens -- --mobile ../melomango-mobile` copia además el `.kt` a
`app/src/main/kotlin/com/fgascon/melomango/ui/theme/MelomangoTokens.kt`, que es lo que usa
`MelomangoTheme`. Con `--check` no escribe nada y falla si algo está desactualizado (también con
`--mobile`, para comprobar la copia del móvil).

## Reglas

- **Los archivos generados no se editan.** Llevan una cabecera que lo dice. `npm test`
  (`tests/tokens.test.ts`) falla, también en la CI, si no coinciden con los tokens, igual que
  `bindings.ts`.
- **Cada cambio de token se hace en los dos temas**, o el test falla.
- **El contraste se comprueba solo.** Texto normal a 4,5:1, texto grande, iconos y foco a 3:1, en los
  dos temas, y los colores de datos tienen que distinguirse también con daltonismo. Un color nuevo
  que se use como texto se añade a `contraste.json`.
- **Las apps usan semánticos, nunca primitivos.** Si falta un uso, se crea el semántico.
- **Se sube la versión** de `manifest.json` cuando cambia un valor o un nombre. La lleva la cabecera
  de lo generado y `MelomangoTokens.VERSION`.

## Nombres

Un token tiene el mismo nombre en todas partes, con la forma de cada lenguaje:

| Token | CSS | Kotlin |
|---|---|---|
| `color.text.muted` | `--color-text-muted` | `MelomangoTokens.light.textMuted` / `.dark.textMuted` |
| `color.accent` | `--color-accent` | `MelomangoTokens.light.accent` (o `MaterialTheme.colorScheme.primary`) |
| `color.decor.cover-soft` | `--color-decor-cover-soft` | `MelomangoTokens.light.decorCoverSoft` |
| `size.player-height` | `--size-player-height` | `MelomangoTokens.sizePlayerHeight` |

Las sombras (`shadow.*`) solo se generan en CSS: Compose usa elevación. La tipografía, el
espaciado, los radios y el movimiento llegarán con KAN-89 y KAN-90, con el mismo generador.
