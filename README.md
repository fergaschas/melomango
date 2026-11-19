# 🥭 Melomango

Reproductor de música en streaming gratuito para escritorio, hecho con **Tauri v2 + React + TypeScript + Rust**.
Sin pagar ni un mango.

Fuentes:

- **Audius** (integrada): plataforma descentralizada con API abierta y canciones completas, legal y gratis.
- **YouTube** (plugin instalable desde la app): todo el catálogo de YouTube, vía [yt-dlp](https://github.com/yt-dlp/yt-dlp).
- Cualquier otra que alguien escriba como plugin: ver [plugins/PROTOCOL.md](plugins/PROTOCOL.md).

Los plugins oficiales viven en [fergaschas/melomango-plugins](https://github.com/fergaschas/melomango-plugins).

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

### YouTube

1. Instala yt-dlp: `winget install yt-dlp.yt-dlp` (o deja `yt-dlp.exe` dentro de la carpeta del plugin).
2. En Melomango: **Plugins → Explorar**, busca «youtube» y pulsa **Instalar**.

Si YouTube cambia algo y deja de funcionar, actualiza yt-dlp: `yt-dlp -U` (o `winget upgrade yt-dlp.yt-dlp`).

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

- [ ] Playlists y "me gusta" en SQLite (`tauri-plugin-sql`)
- [ ] Caché de audio en disco / modo offline
- [ ] Vistas de artista y álbum
- [ ] Letras sincronizadas con LRCLIB
- [ ] Más plugins: SoundCloud, Jamendo, Subsonic/Navidrome, archivos locales
- [ ] Icono propio: `npm run tauri icon ruta/al/logo.png`
