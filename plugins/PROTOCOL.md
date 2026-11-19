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
