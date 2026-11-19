# 🥭 Melomango

Reproductor de música en streaming gratuito para escritorio, hecho con **Tauri v2 + React + TypeScript + Rust**.
Sin pagar ni un mango.

La primera fuente es [Audius](https://audius.org): plataforma descentralizada con API abierta y canciones completas, legal y gratis.

## Desarrollo

```powershell
npm install
npm run tauri dev
```

Tests del backend:

```powershell
cd src-tauri; cargo test
```

## Arquitectura

```
src/                        Frontend (React)
  lib/api.ts                invoke() a Rust + URL del protocolo stream://
  hooks/usePlayer.ts        cola, <audio>, Media Session (teclas multimedia / SMTC)
  components/               TrackList, PlayerBar, iconos
src-tauri/src/
  lib.rs                    estado, comandos (search, trending) y registro del protocolo
  sources/mod.rs            trait MusicSource + Registry
  sources/audius.rs         implementación de Audius
  stream.rs                 protocolo stream://: proxy con soporte Range (seek)
```

**Flujo de reproducción:** el `<audio>` pide `stream://localhost/<fuente>/<id>` (en Windows `http://stream.localhost/...`).
Rust resuelve la URL real con la fuente, reenvía la cabecera `Range` y devuelve el trozo de audio (máx. 1 MiB por petición).
La URL final tras las redirecciones se cachea por pista y se vuelve a resolver si caduca.

## Añadir una fuente nueva

1. Crea `src-tauri/src/sources/<nombre>.rs` e implementa `MusicSource` (`search`, `trending`, `stream_url`).
2. Regístrala en `Registry::new(...)` en `lib.rs`.
3. Pasa su id como `source` desde el frontend.

Candidatas: Jamendo, Internet Archive, Subsonic/Navidrome (tu propia biblioteca), archivos locales.

## Siguientes pasos

- [ ] Playlists y "me gusta" en SQLite (`tauri-plugin-sql`)
- [ ] Caché de audio en disco / modo offline
- [ ] Vistas de artista y álbum
- [ ] Letras sincronizadas con LRCLIB
- [ ] Selector de fuente en la UI
- [ ] Icono propio: `npm run tauri icon ruta/al/logo.png`
