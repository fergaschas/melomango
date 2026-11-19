//! `stream://` protocol: proxies audio from the sources to the WebView's `<audio>`.
//!
//! The frontend asks for `stream://localhost/<source>/<track_id>` (on Windows
//! `http://stream.localhost/...`). We forward the browser's Range header so seeking
//! works, and avoid CORS and odd upstream headers.
//!
//! Tauri protocol responses carry the whole body in memory, so open-ended ranges
//! (`bytes=N-`) are capped at `CHUNK` bytes: the media element simply asks for the
//! next piece when it needs it.

use std::{collections::HashMap, sync::Mutex};

use anyhow::{anyhow, Context};
use percent_encoding::percent_decode_str;
use tauri::http::{header, Request, Response};

use crate::sources::Registry;

const CHUNK: u64 = 1024 * 1024;

/// Final (post-redirect) audio URL per track, so each chunk skips the redirect chain.
#[derive(Default)]
pub struct StreamCache {
    urls: Mutex<HashMap<String, String>>,
}

impl StreamCache {
    fn get(&self, key: &str) -> Option<String> {
        self.urls.lock().unwrap().get(key).cloned()
    }
    fn insert(&self, key: String, url: String) {
        self.urls.lock().unwrap().insert(key, url);
    }
    fn remove(&self, key: &str) {
        self.urls.lock().unwrap().remove(key);
    }
}

pub async fn handle(
    registry: &Registry,
    http: &reqwest::Client,
    cache: &StreamCache,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    match serve(registry, http, cache, &request).await {
        Ok(response) => response,
        Err(e) => Response::builder()
            .status(502)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(format!("{e:#}").into_bytes())
            .unwrap(),
    }
}

async fn serve(
    registry: &Registry,
    http: &reqwest::Client,
    cache: &StreamCache,
    request: &Request<Vec<u8>>,
) -> anyhow::Result<Response<Vec<u8>>> {
    let path = percent_decode_str(request.uri().path().trim_start_matches('/'))
        .decode_utf8()
        .context("ruta no válida")?;
    let (source_id, track_id) = path
        .split_once('/')
        .ok_or_else(|| anyhow!("se esperaba <fuente>/<id>, llegó {path}"))?;
    let source = registry.get(source_id)?;

    let range = capped_range(
        request
            .headers()
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok()),
    );

    let key = format!("{source_id}/{track_id}");
    let cached = cache.get(&key);
    let url = match &cached {
        Some(url) => url.clone(),
        None => source.stream_url(track_id).await?,
    };
    let mut upstream = http.get(&url).header(header::RANGE, &range).send().await?;

    // Signed URLs expire: if the cached one fails, resolve again from the source.
    if !upstream.status().is_success() && cached.is_some() {
        cache.remove(&key);
        let url = source.stream_url(track_id).await?;
        upstream = http.get(&url).header(header::RANGE, &range).send().await?;
    }
    if upstream.status().is_success() {
        cache.insert(key, upstream.url().to_string());
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

/// Turns the browser's Range header into one requesting at most `CHUNK` bytes.
fn capped_range(header: Option<&str>) -> String {
    let (start, end) = header
        .and_then(|h| h.strip_prefix("bytes="))
        .and_then(|r| r.split(',').next())
        .and_then(|r| r.split_once('-'))
        .and_then(|(s, e)| Some((s.trim().parse::<u64>().ok()?, e.trim().parse::<u64>().ok())))
        .unwrap_or((0, None));
    let max_end = start + CHUNK - 1;
    let end = end.map_or(max_end, |e| e.min(max_end));
    format!("bytes={start}-{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_range_is_capped() {
        assert_eq!(capped_range(Some("bytes=0-")), format!("bytes=0-{}", CHUNK - 1));
        assert_eq!(
            capped_range(Some("bytes=500-")),
            format!("bytes=500-{}", 500 + CHUNK - 1)
        );
    }

    #[test]
    fn small_range_is_kept() {
        assert_eq!(capped_range(Some("bytes=10-99")), "bytes=10-99");
    }

    #[test]
    fn missing_or_invalid_range_starts_at_zero() {
        assert_eq!(capped_range(None), format!("bytes=0-{}", CHUNK - 1));
        assert_eq!(capped_range(Some("bytes=-500")), format!("bytes=0-{}", CHUNK - 1));
    }

    /// End-to-end against the real Audius API: `cargo test -- --ignored`
    #[tokio::test]
    #[ignore = "necesita red"]
    async fn streams_a_trending_audius_track() {
        use crate::sources::{audius::Audius, MusicSource};
        use std::sync::Arc;

        let http = reqwest::Client::new();
        let audius = Arc::new(Audius::new(http.clone()));
        let track = audius.trending().await.unwrap().into_iter().next().expect("sin tendencias");
        let registry = Registry::new(vec![audius]);
        let cache = StreamCache::default();

        let request = |range: &str| {
            Request::builder()
                .uri(format!("http://stream.localhost/audius%2F{}", track.id))
                .header(header::RANGE, range)
                .body(Vec::new())
                .unwrap()
        };

        let first = handle(&registry, &http, &cache, request("bytes=0-")).await;
        assert_eq!(first.status(), 206, "{}", String::from_utf8_lossy(first.body()));
        assert_eq!(first.body().len() as u64, CHUNK);
        assert!(first.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with("audio/"));

        // Second chunk comes from the cached final URL.
        let second = handle(&registry, &http, &cache, request("bytes=1048576-")).await;
        assert_eq!(second.status(), 206);
        assert!(
            second.headers()[header::CONTENT_RANGE]
                .to_str()
                .unwrap()
                .starts_with("bytes 1048576-")
        );
    }
}
