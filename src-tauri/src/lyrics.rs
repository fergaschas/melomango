//! Synchronised lyrics from LRCLIB (https://lrclib.net), a free and open lyrics database.
//!
//! Lookup order: exact match by artist + title + duration (`/api/get`), then a search
//! (`/api/search`) picking the closest duration, preferring synced lyrics.
//! Results (including "not found") are cached in memory per track.

use std::{collections::HashMap, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::models::{trim_featuring, Track};

const API: &str = "https://lrclib.net/api";
const CACHE_LIMIT: usize = 300;
/// Search results further than this from the track's duration are probably another version.
const MAX_DURATION_DIFF: f64 = 20.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_ms: u32,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize)]
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
    cache: Mutex<HashMap<String, Lyrics>>,
}

impl LyricsService {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, cache: Mutex::new(HashMap::new()) }
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

        // 1. Exact match (LRCLIB allows ±2 s on the duration).
        for artist_name in dedup([track.artist.as_str(), artist]) {
            let response = self
                .http
                .get(format!("{API}/get"))
                .query(&[("artist_name", artist_name), ("track_name", title), ("duration", &duration)])
                .send()
                .await?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                continue;
            }
            let record: Record = response.error_for_status()?.json().await?;
            if record.has_synced() || record.instrumental {
                return Ok(Some(record));
            }
        }

        // 2. Search and pick the closest version.
        let results: Vec<Record> = self
            .http
            .get(format!("{API}/search"))
            .query(&[("track_name", title), ("artist_name", artist)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pick_best(results, track.duration_secs as f64))
    }
}

fn dedup<'a>(items: [&'a str; 2]) -> Vec<&'a str> {
    if items[0] == items[1] { vec![items[0]] } else { items.to_vec() }
}

/// "Shakira, Burna Boy" / "Bad Bunny & Drake" / "A feat. B" -> first artist.
fn main_artist(artist: &str) -> &str {
    let lower = artist.to_ascii_lowercase();
    let cut = [",", " & ", " x ", " feat", " ft.", " with "]
        .iter()
        .filter_map(|sep| lower.find(sep))
        .min()
        .unwrap_or(artist.len());
    artist[..cut].trim()
}

/// Closest duration wins; synced lyrics beat plain ones within the allowed difference.
fn pick_best(results: Vec<Record>, duration: f64) -> Option<Record> {
    let diff = |r: &Record| r.duration.map_or(f64::MAX, |d| (d - duration).abs());
    results
        .into_iter()
        .filter(|r| r.has_synced() || r.plain_lyrics.is_some() || r.instrumental)
        .filter(|r| diff(r) <= MAX_DURATION_DIFF)
        .min_by(|a, b| {
            b.has_synced()
                .cmp(&a.has_synced())
                .then(diff(a).total_cmp(&diff(b)))
        })
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
    Some((min * 60 + sec) * 1000 + ms)
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
    }

    #[test]
    fn main_artist_takes_the_first() {
        assert_eq!(main_artist("Shakira, Burna Boy"), "Shakira");
        assert_eq!(main_artist("Bad Bunny & Drake"), "Bad Bunny");
        assert_eq!(main_artist("Eminem feat. Rihanna"), "Eminem");
        assert_eq!(main_artist("Rosalía"), "Rosalía");
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
}
