//! `stream://` protocol: proxies audio from the sources to the WebView's `<audio>`.
//!
//! The frontend asks for `stream://localhost/<source>/<track_id>` (on Windows
//! `http://stream.localhost/...`). We forward the browser's Range header so seeking
//! works, add whatever headers the source requires, and avoid CORS issues.
//!
//! Tauri protocol responses carry the whole body in memory, so open-ended ranges
//! (`bytes=N-`) are capped at `CHUNK` bytes: the media element simply asks for the
//! next piece when it needs it.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, bail, Context};
use percent_encoding::percent_decode_str;
use tauri::http::{header, Request, Response};
use tokio::sync::{OnceCell, Semaphore};

use crate::{
    downloads::{self, Downloads},
    library::{DownloadFile, Library},
    matcher,
    models::StreamInfo,
    sources::{MusicSource, Registry},
};

const CHUNK: u64 = 1024 * 1024;
/// Up to ~5 s waiting for a fresh URL to start answering, polling every 250 ms.
const PROBE_RETRIES: u32 = 20;
const PROBE_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(250);
/// Background resolutions running at once (each may start a plugin process).
const PREFETCH_LIMIT: usize = 3;

type Slot = Arc<OnceCell<StreamInfo>>;

/// Resolved stream per track. Resolving can be slow (a plugin may take seconds), so
/// concurrent requests for the same track share a single resolution, and the app
/// resolves likely next tracks ahead of time (see [`prefetch`]).
pub struct StreamCache {
    slots: Mutex<HashMap<String, Slot>>,
    prefetching: Semaphore,
}

impl Default for StreamCache {
    fn default() -> Self {
        Self { slots: Mutex::default(), prefetching: Semaphore::new(PREFETCH_LIMIT) }
    }
}

impl StreamCache {
    fn slot(&self, key: &str) -> Slot {
        self.slots.lock().unwrap().entry(key.to_string()).or_default().clone()
    }

    /// Forgets the resolved stream of `key` (e.g. after the user picks another recording).
    pub fn forget(&self, key: &str) {
        self.slots.lock().unwrap().remove(key);
    }

    /// Drops `slot` unless someone already replaced it with a fresher one.
    fn invalidate(&self, key: &str, slot: &Slot) {
        let mut slots = self.slots.lock().unwrap();
        if slots.get(key).is_some_and(|s| Arc::ptr_eq(s, slot)) {
            slots.remove(key);
        }
    }
}

/// Resolves a track's stream in advance (the next one in the queue, a hovered row) so
/// that playing it starts right away. Skipped if it is already resolved or too many
/// prefetches are running.
pub async fn prefetch(
    registry: &Registry,
    library: &Library,
    http: &reqwest::Client,
    cache: &StreamCache,
    source_id: &str,
    track_id: &str,
) -> anyhow::Result<()> {
    let slot = cache.slot(&format!("{source_id}/{track_id}"));
    if slot.initialized() {
        return Ok(());
    }
    let Ok(_permit) = cache.prefetching.try_acquire() else {
        return Ok(());
    };
    slot.get_or_try_init(|| open(registry, library, http, source_id, track_id)).await?;
    Ok(())
}

pub async fn handle(
    registry: Arc<Registry>,
    library: &Library,
    http: &reqwest::Client,
    cache: &StreamCache,
    downloads: &Downloads,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    match serve(&registry, library, http, cache, downloads, &request).await {
        Ok(response) => response,
        Err(e) => Response::builder()
            .status(502)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            // The player loads streams with CORS (for volume normalization) and reads this text.
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(format!("{e:#}").into_bytes())
            .unwrap(),
    }
}

async fn serve(
    registry: &Registry,
    library: &Library,
    http: &reqwest::Client,
    cache: &StreamCache,
    downloads: &Downloads,
    request: &Request<Vec<u8>>,
) -> anyhow::Result<Response<Vec<u8>>> {
    let path = percent_decode_str(request.uri().path().trim_start_matches('/'))
        .decode_utf8()
        .context("ruta no válida")?;
    let (source_id, track_id) = path
        .split_once('/')
        .ok_or_else(|| anyhow!("se esperaba <fuente>/<id>, llegó {path}"))?;
    let open = || open(registry, library, http, source_id, track_id);

    let requested = capped_range(
        request
            .headers()
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok()),
    );

    // Downloaded: straight from disk, no connection needed.
    let local = downloads.local(library, source_id, track_id);
    let Some((start, end)) = requested else {
        return unsatisfiable(local.as_ref().map(|(_, file)| file.size));
    };
    if let Some((path, file)) = local {
        return serve_file(&path, &file, start, end);
    }
    let range = format!("bytes={start}-{end}");

    let key = format!("{source_id}/{track_id}");
    let slot = cache.slot(&key);
    let was_cached = slot.initialized();
    let info = slot
        .get_or_try_init(open)
        .await?
        .clone();
    let mut upstream = fetch(http, &info, &range).await?;

    // Signed URLs expire: if a cached one fails, resolve again from the source.
    if !upstream.status().is_success() && was_cached {
        cache.invalidate(&key, &slot);
        let slot = cache.slot(&key);
        let info = slot.get_or_try_init(open).await?.clone();
        upstream = fetch(http, &info, &range).await?;
    }

    let mut response = Response::builder()
        .status(upstream.status().as_u16())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");
    for name in [header::CONTENT_TYPE, header::CONTENT_RANGE] {
        if let Some(value) = upstream.headers().get(name.as_str()) {
            response = response.header(name, value.as_bytes());
        }
    }
    let body = upstream.bytes().await?.to_vec();
    Ok(response
        .header(header::CONTENT_LENGTH, body.len())
        .body(body)?)
}

/// 416: the requested range can't be served (`size` is known for downloaded files).
fn unsatisfiable(size: Option<u64>) -> anyhow::Result<Response<Vec<u8>>> {
    let mut response = Response::builder()
        .status(416)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");
    if let Some(size) = size {
        response = response.header(header::CONTENT_RANGE, format!("bytes */{size}"));
    }
    Ok(response.body(Vec::new())?)
}

/// Answers a range request (`start..=end`, already capped) from a downloaded file.
fn serve_file(path: &std::path::Path, file: &DownloadFile, start: u64, end: u64) -> anyhow::Result<Response<Vec<u8>>> {
    if start >= file.size {
        return unsatisfiable(Some(file.size));
    }
    let body = downloads::read_range(path, start, end, file.size)?;
    let last = start + body.len() as u64 - 1;
    Ok(Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .status(206)
        .header(header::CONTENT_TYPE, &file.mime)
        .header(header::CONTENT_RANGE, format!("bytes {start}-{last}/{}", file.size))
        .header(header::CONTENT_LENGTH, body.len())
        .body(body)?)
}

pub(crate) async fn open(
    registry: &Registry,
    library: &Library,
    http: &reqwest::Client,
    source_id: &str,
    track_id: &str,
) -> anyhow::Result<StreamInfo> {
    // Tracks from metadata-only sources (e.g. Spotify) are matched to a playable one.
    let (source, play_id) = matcher::resolve(registry, library, http, source_id, track_id).await?;
    resolve(http, source.as_ref(), &play_id).await
}

/// Asks the source for the stream and follows redirects once, so later chunks go
/// straight to the final server.
async fn resolve(
    http: &reqwest::Client,
    source: &dyn MusicSource,
    track_id: &str,
) -> anyhow::Result<StreamInfo> {
    let mut info = source.stream(track_id).await?;
    // Some servers (YouTube) answer 403 for the first moments after issuing a URL.
    let mut attempt = 0;
    loop {
        let probe = fetch(http, &info, "bytes=0-0").await?;
        if probe.status().is_success() {
            info.url = probe.url().to_string();
            return Ok(info);
        }
        if probe.status() != reqwest::StatusCode::FORBIDDEN || attempt >= PROBE_RETRIES {
            bail!("el servidor de audio respondió {}", probe.status());
        }
        attempt += 1;
        tokio::time::sleep(PROBE_RETRY_DELAY).await;
    }
}

pub(crate) async fn fetch(http: &reqwest::Client, info: &StreamInfo, range: &str) -> anyhow::Result<reqwest::Response> {
    let mut request = http.get(&info.url);
    for (name, value) in &info.headers {
        if !name.eq_ignore_ascii_case("range") && !name.eq_ignore_ascii_case("host") {
            request = request.header(name, value);
        }
    }
    Ok(request.header(header::RANGE, range).send().await?)
}

/// Turns the browser's Range header into `start..=end`, at most `CHUNK` bytes. None when
/// it can't be satisfied (it ends before it starts): any script in the WebView can send one.
fn capped_range(header: Option<&str>) -> Option<(u64, u64)> {
    let (start, end) = header
        .and_then(|h| h.strip_prefix("bytes="))
        .and_then(|r| r.split(',').next())
        .and_then(|r| r.split_once('-'))
        .and_then(|(s, e)| Some((s.trim().parse::<u64>().ok()?, e.trim().parse::<u64>().ok())))
        .unwrap_or((0, None));
    let max_end = start.saturating_add(CHUNK - 1);
    let end = end.map_or(max_end, |e| e.min(max_end));
    (end >= start).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_range_is_capped() {
        assert_eq!(capped_range(Some("bytes=0-")), Some((0, CHUNK - 1)));
        assert_eq!(capped_range(Some("bytes=500-")), Some((500, 500 + CHUNK - 1)));
    }

    #[test]
    fn small_range_is_kept() {
        assert_eq!(capped_range(Some("bytes=10-99")), Some((10, 99)));
    }

    #[test]
    fn missing_or_invalid_range_starts_at_zero() {
        assert_eq!(capped_range(None), Some((0, CHUNK - 1)));
        assert_eq!(capped_range(Some("bytes=-500")), Some((0, CHUNK - 1)));
    }

    #[test]
    fn malformed_ranges_do_not_overflow() {
        assert_eq!(capped_range(Some("bytes=100-50")), None);
        assert_eq!(capped_range(Some("bytes=18446744073709551615-")), Some((u64::MAX, u64::MAX)));
    }

    #[test]
    fn downloaded_files_answer_bad_ranges_with_416() {
        let dir = std::env::temp_dir().join(format!("melomango-range-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.bin");
        std::fs::write(&path, b"0123456789").unwrap();
        let file = DownloadFile { file: "song.bin".into(), mime: "audio/webm".into(), size: 10 };
        assert_eq!(serve_file(&path, &file, u64::MAX, u64::MAX).unwrap().status(), 416);
        let ok = serve_file(&path, &file, 2, 4).unwrap();
        assert_eq!((ok.status().as_u16(), ok.body().as_slice()), (206, &b"234"[..]));
        assert_eq!(unsatisfiable(Some(10)).unwrap().headers()[header::CONTENT_RANGE], "bytes */10");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn invalidate_keeps_a_newer_slot() {
        let cache = StreamCache::default();
        let old = cache.slot("a");
        cache.invalidate("a", &old);
        let new = cache.slot("a");
        assert!(!Arc::ptr_eq(&old, &new));
        cache.invalidate("a", &old); // stale handle: must not drop `new`
        assert!(Arc::ptr_eq(&new, &cache.slot("a")));
    }

    /// Downloads the YouTube plugin from the official repository, installs it and streams
    /// through it. Needs network and yt-dlp: `cargo test -- --ignored`
    #[tokio::test]
    #[ignore = "necesita red y yt-dlp"]
    async fn streams_through_the_youtube_plugin() {
        use crate::{catalog, sources::plugin};

        let http = reqwest::Client::builder().user_agent("Melomango-tests").build().unwrap();
        let plugins_dir = std::env::temp_dir().join(format!("melomango-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&plugins_dir);

        let listing = catalog::browse(&http, &[catalog::RegistryInfo {
            url: catalog::OFFICIAL_INDEX.into(),
            official: true,
        }])
        .await;
        assert!(listing.errors.is_empty(), "{:?}", listing.errors);
        assert!(listing.plugins.iter().any(|p| p.id == "youtube" && p.available));
        catalog::install(&http, catalog::OFFICIAL_INDEX, "youtube", &plugins_dir)
            .await
            .unwrap();

        let youtube: Arc<dyn MusicSource> =
            Arc::new(plugin::discover(&plugins_dir, &plugins_dir.join("data")).pop().unwrap());
        let track = youtube.search("shakira dai dai").await.unwrap().into_iter().next().unwrap();
        assert_eq!(track.source, "youtube");

        let registry = Arc::new(Registry::new(vec![youtube]));
        let library = Library::in_memory();
        let cache = StreamCache::default();
        let downloads = Downloads::new(std::env::temp_dir().join("melomango-test-downloads"));

        // Prefetching resolves the stream ahead of time.
        prefetch(&registry, &library, &http, &cache, "youtube", &track.id).await.unwrap();
        assert!(cache.slot(&format!("youtube/{}", track.id)).initialized());

        let request = |range: &str| {
            Request::builder()
                .uri(format!("http://stream.localhost/youtube%2F{}", track.id))
                .header(header::RANGE, range)
                .body(Vec::new())
                .unwrap()
        };

        let first = handle(registry.clone(), &library, &http, &cache, &downloads, request("bytes=0-")).await;
        assert_eq!(first.status(), 206, "{}", String::from_utf8_lossy(first.body()));
        assert_eq!(first.body().len() as u64, CHUNK);
        assert!(first.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with("audio/"));

        let second = handle(registry.clone(), &library, &http, &cache, &downloads, request("bytes=1048576-")).await;
        assert_eq!(second.status(), 206);

        // A track imported from a metadata-only service (as from Spotify) is matched to
        // YouTube on first play and the match is remembered.
        let imported = crate::models::Track {
            id: "4fakeSpotifyId".into(),
            source: "spotify".into(),
            title: "Dai Dai".into(),
            artist: "Shakira, Burna Boy".into(),
            duration_secs: 223,
            artwork_url: None,
            genre: None,
        };
        library.import_playlist("spotify", "liked", "Me gusta de Spotify", &[imported]).unwrap();
        let spotify_request = Request::builder()
            .uri("http://stream.localhost/spotify%2F4fakeSpotifyId")
            .header(header::RANGE, "bytes=0-")
            .body(Vec::new())
            .unwrap();
        let matched = handle(registry.clone(), &library, &http, &cache, &downloads, spotify_request).await;
        assert_eq!(matched.status(), 206, "{}", String::from_utf8_lossy(matched.body()));
        let (play_source, _) = library.track_match("spotify", "4fakeSpotifyId").unwrap().unwrap();
        assert_eq!(play_source, "youtube");

        // Songs saved from Audius (built in until 0.12) play the same way, from YouTube.
        let from_audius = crate::models::Track {
            id: "D7KyD".into(),
            source: "audius".into(),
            title: "Dai Dai".into(),
            artist: "Shakira".into(),
            duration_secs: 223,
            artwork_url: None,
            genre: None,
        };
        library.set_liked(&from_audius, true).unwrap();
        let audius_request = Request::builder()
            .uri("http://stream.localhost/audius%2FD7KyD")
            .header(header::RANGE, "bytes=0-")
            .body(Vec::new())
            .unwrap();
        let matched = handle(registry, &library, &http, &cache, &downloads, audius_request).await;
        assert_eq!(matched.status(), 206, "{}", String::from_utf8_lossy(matched.body()));
        assert_eq!(library.track_match("audius", "D7KyD").unwrap().unwrap().0, "youtube");

        let _ = std::fs::remove_dir_all(&plugins_dir);
    }
}
