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
| `capabilities` | no | Incluye `"trending"` si el plugin implementa ese comando. `search` y `stream` son obligatorios. |
| `description` | no | Texto corto para la lista de plugins. |
| `author`, `homepage`, `tags` | no | Metadatos para el buscador de plugins (los usa el script de publicación del índice). |

## Comandos

Cada llamada ejecuta el programa una vez (directorio de trabajo: la carpeta del plugin), con un límite de 60 s.

```text
<executable> <args...> search <texto>   → {"tracks": [Track, ...]}
<executable> <args...> trending         → {"tracks": [Track, ...]}
<executable> <args...> stream <id>      → {"url": "https://...", "headers": {"User-Agent": "..."}}
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
