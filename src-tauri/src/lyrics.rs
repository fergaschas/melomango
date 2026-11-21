//! Synchronised lyrics from LRCLIB (https://lrclib.net), a free and open lyrics database.
//!
//! Lookup order: exact match by artist + title + duration (`/api/get`), then a search
//! (`/api/search`) picking the closest duration, preferring synced lyrics.
//! Results (including "not found") are cached in memory per track.

use std::{collections::HashMap, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::{
    models::Track,
    text::{main_artist, trim_featuring},
};

const API: &str = "https://lrclib.net/api";
const CACHE_LIMIT: usize = 300;
/// Search results further than this from the track's duration are probably another version.
const MAX_DURATION_DIFF: f64 = 20.0;

#[derive(Debug, Clone, Serialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_ms: u32,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub found: bool,
    pub instrumental: bool,
    /// Empty when only plain lyrics exist.
    pub synced: Vec<LyricLine>,
    pub plain: Option<String>,
    /// Duration of the matched recording, to hint at offsets (e.g. long video intros).
    pub matched_duration_secs: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

impl Record {
    fn has_synced(&self) -> bool {
        self.synced_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty())
    }
}

pub struct LyricsService {
    http: reqwest::Client,
    /// LRCLIB, or a local server in tests.
    api: String,
    cache: Mutex<HashMap<String, Lyrics>>,
}

impl LyricsService {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, api: API.to_string(), cache: Mutex::new(HashMap::new()) }
    }

    pub async fn get(&self, track: &Track) -> anyhow::Result<Lyrics> {
        let key = format!("{}/{}", track.source, track.id);
        if let Some(hit) = self.cache.lock().unwrap().get(&key) {
            return Ok(hit.clone());
        }
        let lyrics = self.lookup(track).await?.map(into_lyrics).unwrap_or_default();
        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, lyrics.clone());
        Ok(lyrics)
    }

    async fn lookup(&self, track: &Track) -> anyhow::Result<Option<Record>> {
        let title = trim_featuring(&track.title);
        let artist = main_artist(&track.artist);
        let duration = track.duration_secs.to_string();

        // 1. Exact match (LRCLIB allows ±2 s on the duration), if the duration is known.
        let artists = if track.duration_secs > 0 { dedup([track.artist.as_str(), artist]) } else { Vec::new() };
        for artist_name in artists {
            let response = self
                .http
                .get(format!("{}/get", self.api))
                .query(&[("artist_name", artist_name), ("track_name", title), ("duration", &duration)])
                .send()
                .await;
            // Not found, but also any other failure (429, 5xx, a broken answer): the search
            // below may still find it, so it is not a reason to give up.
            let record = match response {
                Ok(r) if r.status().is_success() => r.json::<Record>().await.ok(),
                _ => None,
            };
            if let Some(record) = record.filter(|r| r.has_synced() || r.instrumental) {
                return Ok(Some(record));
            }
        }

        // 2. Search and pick the closest version.
        let results: Vec<Record> = self
            .http
            .get(format!("{}/search", self.api))
            .query(&[("track_name", title), ("artist_name", artist)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pick_best(results, track.duration_secs as f64))
    }
}

/// Durations (in seconds) of the recordings LRCLIB knows for this song, most common
/// first, at most three. Used to match tracks imported without a duration.
pub async fn known_durations(http: &reqwest::Client, track: &Track) -> anyhow::Result<Vec<u32>> {
    let results: Vec<Record> = http
        .get(format!("{API}/search"))
        .query(&[("track_name", trim_featuring(&track.title)), ("artist_name", main_artist(&track.artist))])
        .timeout(std::time::Duration::from_secs(6))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(common_durations(results.iter().filter_map(|r| r.duration)))
}

/// Groups durations within ±2 s and returns the most frequent groups (at most three).
fn common_durations(durations: impl Iterator<Item = f64>) -> Vec<u32> {
    let mut groups: Vec<(u32, usize)> = Vec::new();
    for d in durations.filter(|d| *d > 0.0).map(|d| d.round() as u32) {
        match groups.iter_mut().find(|(g, _)| g.abs_diff(d) <= 2) {
            Some(group) => group.1 += 1,
            None => groups.push((d, 1)),
        }
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.1));
    groups.into_iter().take(3).map(|(d, _)| d).collect()
}

fn dedup(items: [&str; 2]) -> Vec<&str> {
    if items[0] == items[1] { vec![items[0]] } else { items.to_vec() }
}

/// Closest duration wins; synced lyrics beat plain ones within the allowed difference.
fn pick_best(results: Vec<Record>, duration: f64) -> Option<Record> {
    let diff = |r: &Record| r.duration.map_or(f64::MAX, |d| (d - duration).abs());
    results
        .into_iter()
        .filter(|r| r.has_synced() || r.plain_lyrics.is_some() || r.instrumental)
        // Unknown duration (e.g. imported from Spotify's data export): any version will do.
        .filter(|r| duration <= 0.0 || diff(r) <= MAX_DURATION_DIFF)
        .min_by(|a, b| b.has_synced().cmp(&a.has_synced()).then(diff(a).total_cmp(&diff(b))))
}

fn into_lyrics(record: Record) -> Lyrics {
    Lyrics {
        found: true,
        instrumental: record.instrumental,
        synced: record.synced_lyrics.as_deref().map(parse_lrc).unwrap_or_default(),
        plain: record.plain_lyrics.filter(|p| !p.trim().is_empty()),
        matched_duration_secs: record.duration,
    }
}

/// Parses LRC text: `[mm:ss.xx]` timestamps (several per line allowed); metadata
/// tags like `[ar:...]` and untimed lines are ignored.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            if let Some(ms) = parse_timestamp(&stripped[..end]) {
                times.push(ms);
            }
            rest = &stripped[end + 1..];
        }
        let text = rest.trim();
        lines.extend(times.into_iter().map(|time_ms| LyricLine { time_ms, text: text.to_string() }));
    }
    lines.sort_by_key(|l| l.time_ms);
    lines
}

fn parse_timestamp(tag: &str) -> Option<u32> {
    let (min, sec) = tag.split_once(':')?;
    let min: u32 = min.trim().parse().ok()?;
    let (sec, frac) = sec.split_once('.').unwrap_or((sec, ""));
    let sec: u32 = sec.trim().parse().ok()?;
    if sec >= 60 || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // ".5" -> 500 ms, ".09" -> 90 ms, ".123" -> 123 ms
    let ms: u32 = format!("{frac:0<3}")[..3].parse().ok()?;
    // The minutes come from outside: a huge number is a broken line, not a time.
    min.checked_mul(60)?.checked_add(sec)?.checked_mul(1000)?.checked_add(ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(duration: f64, synced: bool) -> Record {
        Record {
            duration: Some(duration),
            instrumental: false,
            plain_lyrics: Some("letra".into()),
            synced_lyrics: synced.then(|| "[00:01.00] hola".into()),
        }
    }

    #[test]
    fn parses_lrc() {
        let lrc = "[ar:Shakira]\n[00:00.09] Oh-eh\n[00:12.5][01:02.123]Coro\n[00:05.80]\nsin tiempo\n";
        assert_eq!(
            parse_lrc(lrc),
            vec![
                LyricLine { time_ms: 90, text: "Oh-eh".into() },
                LyricLine { time_ms: 5800, text: "".into() },
                LyricLine { time_ms: 12500, text: "Coro".into() },
                LyricLine { time_ms: 62123, text: "Coro".into() },
            ]
        );
    }

    #[test]
    fn rejects_bad_timestamps() {
        assert_eq!(parse_timestamp("ar:Shakira"), None);
        assert_eq!(parse_timestamp("00:75.00"), None);
        assert_eq!(parse_timestamp("02:03"), Some(123_000));
        assert_eq!(parse_timestamp("10:00.1"), Some(600_100));
        // Used to wrap around in release (a wrong time) and panic in debug.
        assert_eq!(parse_timestamp("99999999:00"), None);
        assert_eq!(parse_timestamp("4294967295:00"), None);
    }

    /// A one-request-at-a-time HTTP server: `/api/get` fails with `get_status`, `/api/search`
    /// answers `search_body`. Returns its base URL.
    async fn fake_lrclib(get_status: u16, search_body: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/api", listener.local_addr().unwrap());
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = vec![0; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);
                let (status, body) =
                    if request.starts_with("GET /api/search") { (200, search_body) } else { (get_status, "{}") };
                let answer = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(answer.as_bytes()).await;
            }
        });
        url
    }

    #[tokio::test]
    async fn a_failing_exact_lookup_falls_back_to_the_search() {
        let search =
            r#"[{"duration": 200.0, "instrumental": false, "plainLyrics": "hola", "syncedLyrics": "[00:01.00] hola"}]"#;
        for status in [429, 500, 503] {
            let mut service = LyricsService::new(reqwest::Client::new());
            service.api = fake_lrclib(status, search).await;
            let track = Track {
                id: "1".into(),
                source: "youtube".into(),
                title: "Canción".into(),
                artist: "Artista".into(),
                duration_secs: 200,
                artwork_url: None,
                genre: None,
            };
            let lyrics = service.get(&track).await.unwrap_or_else(|e| panic!("{status}: {e:#}"));
            assert_eq!(lyrics.synced.len(), 1, "with /get answering {status}");
        }
    }

    #[test]
    fn picks_synced_closest_version() {
        let best = pick_best(vec![rec(200.0, false), rec(230.0, true), rec(241.0, true)], 240.0).unwrap();
        assert_eq!(best.duration, Some(241.0));
        // Synced beats plain even if the plain one is a bit closer.
        let best = pick_best(vec![rec(240.0, false), rec(236.0, true)], 240.0).unwrap();
        assert!(best.has_synced());
        // Too far away: probably a different version.
        assert!(pick_best(vec![rec(100.0, true)], 240.0).is_none());
    }

    #[tokio::test]
    #[ignore = "necesita red"]
    async fn fetches_real_lyrics() {
        let service = LyricsService::new(reqwest::Client::builder().user_agent("Melomango-tests").build().unwrap());
        let track = Track {
            id: "fcnDmrtj6Sk".into(),
            source: "youtube".into(),
            title: "Dai Dai".into(),
            artist: "Shakira, Burna Boy".into(),
            duration_secs: 241,
            artwork_url: None,
            genre: None,
        };
        let lyrics = service.get(&track).await.unwrap();
        assert!(lyrics.found);
        assert!(lyrics.synced.len() > 10, "{lyrics:?}");
    }

    #[test]
    fn groups_common_durations() {
        let found = common_durations([244.0, 290.0, 243.6, 245.0, 290.4, 703.0].into_iter());
        assert_eq!(found, [244, 290, 703]);
        assert!(common_durations([0.0].into_iter()).is_empty());
    }
}
