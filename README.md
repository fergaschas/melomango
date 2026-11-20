<p align="center"><img src="app-icon.svg" width="128" alt="Melomango"></p>

# Melomango

Reproductor de música en streaming gratuito para escritorio, hecho con **Tauri v2 + React + TypeScript + Rust**.
Sin pagar ni un mango.

Playlists, Me gusta, letras sincronizadas (LRCLIB), importación de playlists de Spotify y fuentes ampliables con plugins.

Reproductor con cola editable (reproducir a continuación, añadir a la cola, arrastrar para reordenar), aleatorio,
repetir (lista o canción) y atajos de teclado (pulsa **?** para verlos). Al cerrar recuerda la cola, la canción, la
posición y el volumen.

Si una canción importada (de Spotify) suena con el vídeo equivocado, pulsa el botón ⇄ junto a la canción que suena, o
**No es esta canción…** en su menú +, y elige la versión buena: Melomango la recuerda.

**Radio automática**: cuando se acaba la cola, Melomango sigue con canciones parecidas (con el plugin de YouTube, de
la lista «Mix» de cada vídeo; si no, del mismo artista). Se activa o desactiva en el panel de la cola, y **Iniciar
radio** en el menú + de cualquier canción la empieza desde ahí.

**Artistas y álbumes**: pulsa el nombre de un artista en cualquier canción para ver sus canciones populares, sus
álbumes, sencillos y EP, y artistas parecidos; cada álbum tiene su página. Los datos vienen de la API pública de
[Deezer](https://developers.deezer.com/api) (sin cuenta) y la música suena desde YouTube, como las canciones
importadas de Spotify. **Atrás** (o Alt+←) vuelve a la página anterior.

**Buscar**: los resultados se separan en pestañas: **Todo** (el artista que mejor encaja, canciones y filas de cada
tipo), **Canciones** (de la fuente elegida), **Artistas**, **Álbumes** y **Playlists** (tus playlists que coinciden y
playlists públicas de Deezer, que puedes reproducir o guardar en las tuyas y sincronizar después).

**Bandeja del sistema**: Melomango tiene un icono junto al reloj; un clic abre la ventana y su menú pausa, pasa de
canción o sale. Al cerrar la ventana sigue sonando en la bandeja (se puede cambiar en **Ajustes**), y abrir
Melomango otra vez muestra la ventana que ya está en marcha.

**Discord**: mientras suena algo, tu perfil muestra «Escuchando Melomango» con la canción, el artista, la portada y
el tiempo. Melomango habla con la app de Discord de tu equipo (no envía nada por internet) y se puede desactivar en
**Ajustes**, abajo en la barra lateral.

**Historial** (en la barra lateral): lo que has escuchado, por días. Una canción cuenta tras sonar 30 segundos (o la
mitad, si es más corta). Se guarda solo en tu equipo y se puede borrar.

Fuentes:

- **Audius** (integrada): plataforma descentralizada con API abierta y canciones completas, legal y gratis.
- **YouTube** (plugin instalable desde la app): todo el catálogo de YouTube, vía [yt-dlp](https://github.com/yt-dlp/yt-dlp).
- Cualquier otra que alguien escriba como plugin: ver [plugins/PROTOCOL.md](plugins/PROTOCOL.md).

Los plugins oficiales viven en [fergaschas/melomango-plugins](https://github.com/fergaschas/melomango-plugins).

## Instalar

Descarga el instalador de la página de [Releases](https://github.com/fergaschas/melomango/releases):

- **`Melomango_<versión>_x64-setup.exe`** (recomendado): se instala para tu usuario, sin pedir permisos de administrador.
- **`Melomango_<versión>_x64_es-ES.msi`**: para instalaciones gestionadas o para todos los usuarios.

Windows SmartScreen puede avisar de que la app no está firmada: pulsa *Más información → Ejecutar de todas formas*.

## Actualizaciones automáticas

Desde la versión 0.2.0, Melomango busca actualizaciones al arrancar (y con **Buscar actualizaciones**, abajo en la
barra lateral). Si hay una versión nueva, ofrece instalarla: la descarga, comprueba su **firma** y se reinicia.

Lee `latest.json` de la última release de GitHub. Solo acepta paquetes firmados con la clave privada del proyecto
(la clave pública está en `src-tauri/tauri.conf.json`).

> Quien tenga la 0.1.0 debe instalar la 0.2.0 a mano una vez: esa versión todavía no traía el actualizador.

## Publicar una versión

1. Sube el número de versión en `package.json`, `src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json` (los tres iguales).
2. Haz commit y push.
3. Publica:

   ```powershell
   npm run release -- --publish --notes "Qué hay de nuevo"
   ```

El script compila los instaladores firmados, genera `latest.json` y crea la release `v<versión>` con todo.
Sin `--publish` solo compila (los archivos quedan en `src-tauri/target/release/bundle/`).

**Clave de firma:** `~/.tauri/melomango.key` (o la variable `TAURI_SIGNING_PRIVATE_KEY`). Guárdala con copia de
seguridad y no la subas nunca al repo: sin ella no se pueden publicar actualizaciones para quien ya tiene la app.

El icono sale de [`app-icon.svg`](app-icon.svg): si lo cambias, regenera todos los tamaños con `npx tauri icon app-icon.svg`.

## Desarrollo

```powershell
npm install
npm run tauri dev
```

Tests:

```powershell
cd src-tauri; cargo test                     # unitarios
cd src-tauri; cargo test -- --include-ignored # + pruebas reales: Audius, y descarga e instalación del plugin de YouTube
```

## Plugins

Un plugin es una carpeta con `plugin.json` y un ejecutable que habla un protocolo JSON muy simple
(`search`, `trending`, `stream`). Se puede escribir en cualquier lenguaje: [plugins/PROTOCOL.md](plugins/PROTOCOL.md).

### Buscar e instalar plugins

**Plugins → Explorar** muestra los plugins de los repositorios configurados, con buscador. Al pulsar **Instalar**,
la app descarga el zip, comprueba su SHA-256 y lo instala. Sin reiniciar, aparece una pestaña nueva junto a Audius.

- El repositorio oficial viene configurado. Puedes añadir otros (cualquier `index.json` servido por HTTPS) en
  **Explorar → Repositorios**. El formato está en [plugins/PROTOCOL.md](plugins/PROTOCOL.md).
- Para probar un plugin en desarrollo: **Instalados → Instalar desde carpeta…**

### Spotify (importar playlists)

Hay dos formas. En las dos solo se copian los datos de las canciones: al reproducir, Melomango busca cada una en
YouTube (por artista, título y duración) y recuerda el resultado, así que conviene tener el plugin de YouTube.

#### Con un archivo (sin cuenta de desarrollador ni Premium)

En **⬇ Importar playlists → Spotify sin Premium → Elegir archivo…** puedes elegir:

- **El zip de [Exportify](https://exportify.app/)** (al momento): inicias sesión con tu cuenta de Spotify, pulsas
  *Export All* y eliges `spotify_playlists.zip`. Trae duraciones y portadas. Exportify es un servicio de terceros
  de código abierto: le das permiso para leer tus playlists. También valen sus CSV sueltos y CSV de otras
  herramientas con columnas de título y artista (y, opcionalmente, nombre de playlist).
- **Tu exportación de datos de Spotify** (tarda unos días): en
  [spotify.com → Cuenta → Privacidad](https://www.spotify.com/account/privacy/), marca *Datos de la cuenta* y pide
  «Descargar tus datos»; cuando llegue el correo, elige `my_spotify_data.zip`. No trae duraciones: Melomango las
  consulta en LRCLIB al reproducir para elegir la versión correcta.

Se importan tus playlists y tus Me gusta (no los álbumes guardados: ninguno de los dos archivos trae sus canciones).
Para actualizar, vuelve a exportar e importar: las playlists se reemplazan, no se duplican.

#### Pegando un enlace

En **⬇ Importar playlists → Pegar un enlace** vale el enlace de una playlist o un álbum público de Spotify (se copian
las 100 primeras canciones), YouTube o YouTube Music (con el plugin de YouTube 0.5.0 o posterior) o Deezer. Sin
iniciar sesión, y luego se actualizan con **Sincronizar**.

#### Con el plugin (al momento y con sincronización; necesita Premium)

El plugin **Spotify** importa tus playlists, tus «Canciones que te gustan» y tus álbumes guardados usando la API de
Spotify. Desde febrero de 2026, Spotify exige que el dueño de la app de desarrollador tenga **Premium**.

1. **Plugins → Explorar**, busca «spotify» e instálalo.
2. En la barra lateral, junto a *Playlists*, pulsa **⬇ Importar playlists → Conectar Spotify**.
3. Se abre el navegador: la primera vez te guía para crear tu propia app gratuita en
   [developer.spotify.com](https://developer.spotify.com/dashboard) (Redirect URI `http://127.0.0.1:43821/callback`)
   y pegar su *Client ID*. Después inicias sesión en Spotify.
4. Elige qué importar. Las playlists importadas tienen un botón **Sincronizar** para traer los cambios.

Limitaciones de Spotify: solo deja leer las playlists que has creado o en las que colaboras (no las que solo
sigues ni las editoriales como «Éxitos de hoy»).

### YouTube

En Melomango: **Plugins → Explorar**, busca «youtube» y pulsa **Instalar**. El plugin necesita
[yt-dlp](https://github.com/yt-dlp/yt-dlp): el plugin descarga su propia copia desde las releases oficiales de
yt-dlp (comprobando la huella SHA-256), en la versión en carpeta, que arranca más rápido, y la actualiza solo una vez
al día. En *Instalados* verás «Necesita: yt-dlp ✓». Si la descarga falla, lo instala con `winget`.

> ⚖️ Reproducir YouTube fuera de su reproductor va contra sus Términos de Servicio. Por eso es un plugin
> opcional e independiente de la app: úsalo para uso personal y bajo tu responsabilidad.

## Arquitectura

```
src/                          Frontend (React)
  lib/api.ts                  invoke() a Rust + URL del protocolo stream://
  hooks/usePlayer.ts          cola, <audio>, Media Session (teclas multimedia / SMTC)
  components/                 TrackList, PlayerBar, PluginsDialog, iconos
src-tauri/src/
  lib.rs                      estado, comandos y registro del protocolo
  sources/mod.rs              trait MusicSource + Registry
  sources/audius.rs           fuente integrada Audius
  sources/plugin.rs           carga, instalación y ejecución de plugins
  catalog.rs                  repositorios de plugins: índice, descarga y verificación SHA-256
  matcher.rs                  empareja canciones importadas (Spotify) con una fuente reproducible
  stream.rs                   protocolo stream://: proxy con soporte Range (seek)
plugins/
  PROTOCOL.md                 especificación de plugins y del formato de índice
```

**Flujo de reproducción:** el `<audio>` pide `stream://localhost/<fuente>/<id>` (en Windows `http://stream.localhost/...`).
Rust pide a la fuente la URL del audio (y sus cabeceras), sigue las redirecciones una vez, reenvía la cabecera `Range`
y devuelve el trozo de audio (máx. 1 MiB por petición). La resolución se cachea por pista, se comparte entre peticiones
simultáneas y se repite si la URL caduca.

Los plugins instalados viven en `%APPDATA%\com.melomango.app\plugins\<id>\`.

## Siguientes pasos

- [x] Playlists y Me gusta (SQLite)
- [ ] Caché de audio en disco / modo offline
- [ ] Vistas de artista y álbum
- [x] Letras sincronizadas con LRCLIB
- [ ] Más plugins: SoundCloud, Jamendo, Subsonic/Navidrome, archivos locales
- [x] Icono propio e instalador
