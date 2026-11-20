# Protocolo de plugins de Melomango (v1)

Un plugin es una **carpeta** con un manifiesto `plugin.json` y un **ejecutable**. Se puede escribir en
cualquier lenguaje: Melomango solo lo ejecuta y lee JSON de su salida estándar.

Al instalarlo, Melomango copia la carpeta a `%APPDATA%\com.melomango.app\plugins\<id>\`
(en macOS `~/Library/Application Support/com.melomango.app/plugins/`, en Linux `~/.local/share/com.melomango.app/plugins/`).

> ⚠️ Un plugin es un programa que se ejecuta en tu equipo con tus permisos. Instala solo plugins en los que confíes.

## `plugin.json`

```json
{
  "id": "youtube",
  "name": "YouTube",
  "version": "0.1.0",
  "protocol": 1,
  "description": "Todo el catálogo de YouTube, vía yt-dlp.",
  "executable": "melomango-youtube",
  "args": [],
  "capabilities": ["search", "trending"]
}
```

| Campo | Obligatorio | Descripción |
|---|---|---|
| `id` | sí | Minúsculas, números, `-` y `_` (máx. 40). `audius` está reservado. |
| `name` | sí | Nombre visible en la app. |
| `version` | sí | Versión del plugin. |
| `protocol` | sí | Debe ser `1`. |
| `executable` | sí | Ruta relativa dentro de la carpeta. En Windows se prueba con `.exe` si no tiene extensión. |
| `args` | no | Argumentos que se anteponen a cada comando. |
| `capabilities` | no | `"trending"` o `"radio"` si implementa esos comandos. `"library"` para plugins de biblioteca y `"requirements"` si necesita programas externos (ver abajo). En los plugins de música, `search` y `stream` son obligatorios. |
| `description` | no | Texto corto para la lista de plugins. |
| `author`, `homepage`, `tags` | no | Metadatos para el buscador de plugins (los usa el script de publicación del índice). |

## Comandos

Cada llamada ejecuta el programa una vez (directorio de trabajo: la carpeta del plugin), con un límite de 60 s.

```text
<executable> <args...> search <texto>   → {"tracks": [Track, ...]}
<executable> <args...> trending         → {"tracks": [Track, ...]}
<executable> <args...> stream <id>      → {"url": "https://...", "headers": {"User-Agent": "..."}}
<executable> <args...> radio <id>       → {"tracks": [Track, ...]}   (opcional, capacidad "radio")
```

`Track`:

```json
{ "id": "fcnDmrtj6Sk", "title": "Dai Dai", "artist": "Shakira, Burna Boy",
  "durationSecs": 241, "artworkUrl": "https://...", "genre": null }
```

- `artworkUrl` y `genre` son opcionales.
- La URL de `stream` debe aceptar peticiones HTTP con `Range` (Melomango pide el audio en trozos de 1 MiB).
  Puede redirigir. Las cabeceras de `headers` se envían en cada petición.
- Si una URL caduca, Melomango vuelve a llamar a `stream` automáticamente.
- `radio` devuelve canciones parecidas a `<id>` para la radio automática (el plugin de YouTube usa la lista «Mix» de
  cada vídeo). Sin esa capacidad, Melomango busca más canciones del mismo artista.

## Plugins de biblioteca (`"library"`)

Traen **metadatos** de otro servicio (por ejemplo, las playlists de Spotify) sin reproducir nada. Melomango importa
sus colecciones como playlists propias y, al reproducir una canción, la busca en las fuentes de música instaladas
(YouTube primero), recordando el resultado.

```text
<executable> status            → {"connected": true, "user": "Nombre"}
<executable> connect           → igual que status, cuando el usuario termina de iniciar sesión (hasta 10 min)
<executable> disconnect        → {"connected": false}
<executable> collections       → {"collections": [Collection, ...]}
<executable> tracks <id>       → {"tracks": [Track, ...]}
```

`Collection`:

```json
{ "id": "playlist:37i9dQZF1DX", "name": "Para correr", "kind": "playlist", "owner": "Xavi",
  "trackCount": 58, "artworkUrl": "https://...", "importable": true }
```

- `kind`: `liked`, `playlist`, `album`… (decide el icono). `importable: false` si el servicio no deja leerla.
- `connect` debe guiar al usuario por su cuenta (normalmente, abriendo el navegador) y no terminar hasta que acabe.
- Los `Track` llevan `title`, `artist` y `durationSecs` fiables: se usan para encontrar la canción en otras fuentes.

## Requisitos (`"requirements"`)

Para plugins que necesitan un programa externo (el de YouTube necesita yt-dlp). Tras instalar el plugin, Melomango
pregunta qué falta y, si el plugin sabe instalarlo, se lo pide; la pestaña *Instalados* muestra el estado.

```text
<executable> requirements               → {"requirements": [Requirement, ...]}
<executable> install-requirement <id>   → la misma lista, cuando ha terminado (hasta 10 min)
```

```json
{ "id": "yt-dlp", "name": "yt-dlp", "description": "Obtiene el audio de YouTube.",
  "installed": false, "installable": true }
```

`installable: true` significa que el plugin puede instalarlo solo (por ejemplo, descargándolo o con `winget`). Si no, Melomango
avisa para que el usuario lo instale a mano.

## Datos del plugin

Cada llamada recibe la variable de entorno **`MELOMANGO_DATA_DIR`**: una carpeta propia del plugin
(`<datos de la app>/plugin-data/<id>/`) para guardar ajustes y sesiones. Sobrevive a las actualizaciones del plugin
y se borra al desinstalarlo.

## Errores

Termina con código distinto de 0 y escribe el motivo en **stderr**. Melomango muestra la última línea.

## Repositorios de plugins (índices)

La app descarga plugins desde **índices**: archivos JSON servidos por `https://`. El oficial es
[fergaschas/melomango-plugins](https://github.com/fergaschas/melomango-plugins)
(`https://raw.githubusercontent.com/fergaschas/melomango-plugins/main/index.json`), y se pueden añadir otros
desde **Plugins → Explorar → Repositorios**.

```json
{
  "version": 1,
  "name": "Melomango oficial",
  "plugins": [
    {
      "id": "youtube",
      "name": "YouTube",
      "version": "0.1.0",
      "description": "Todo el catálogo de YouTube, vía yt-dlp.",
      "author": "Melomango",
      "homepage": "https://github.com/fergaschas/melomango-plugins/tree/main/youtube",
      "tags": ["youtube", "música"],
      "downloads": {
        "windows-x86_64": {
          "url": "https://github.com/.../youtube-0.1.0-windows-x86_64.zip",
          "sha256": "3f1c…"
        }
      }
    }
  ]
}
```

- Claves de `downloads`: `<os>-<arch>` con los valores de Rust (`windows`, `macos`, `linux` × `x86_64`, `aarch64`).
- Cada descarga es un **zip** con `plugin.json` en la raíz (o dentro de una única carpeta) y el ejecutable.
- Antes de instalar, la app comprueba que el SHA-256 coincide y que el `id` del zip es el del índice.
  Las URL deben ser `https://` y el zip no puede superar 200 MB.
