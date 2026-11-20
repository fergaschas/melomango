//! Matching metadata-only tracks (e.g. imported from Spotify) to playable ones.
//!
//! When such a track is played, we search the playable sources for "artist title",
//! score the candidates by title, artist and duration, and remember the winner in the
//! library so the search only happens once per track. Tracks imported without a
//! duration (Spotify's data export) get the durations LRCLIB knows for the song.

use std::sync::Arc;

use anyhow::{anyhow, bail};
use serde::Serialize;

use crate::{
    library::Library,
    lyrics,
    models::{trim_featuring, Track},
    sources::{MusicSource, Registry},
};

/// Preferred sources to play imported tracks from, best first. Any other installed
/// source is tried afterwards.
const PREFERRED: &[&str] = &["youtube", "audius"];
/// Words that mark another version of the song (unless the original title has them too).
const OTHER_VERSIONS: &[&str] = &[
    "live", "directo", "vivo", "cover", "karaoke", "instrumental", "remix", "sped", "slowed",
    "reverb", "8d", "nightcore", "acoustic", "acustico", "acústico",
];
/// Below this score a candidate is probably another song.
const MIN_SCORE: f64 = 0.45;
/// Without any known duration, longer results are usually concerts, mixes or TV shows.
const LONG_SECS: u32 = 8 * 60;
/// Candidates offered when the user fixes a match by hand.
const MAX_OPTIONS: usize = 30;

/// What the user picks from when the automatic match is wrong ("No es esta canción").
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchOptions {
    pub track: Track,
    pub query: String,
    /// The recording it plays now, if it was matched already.
    pub current: Option<MatchRef>,
    pub candidates: Vec<Track>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct MatchRef {
    pub source: String,
    pub id: String,
}

/// Searches the playable sources for `query` (by default "artist title") so the user
/// can choose the right recording for an imported track. With the default query the
/// best-scored candidates come first; a custom one keeps each source's own order.
pub async fn options(
    registry: &Registry,
    library: &Library,
    source_id: &str,
    track_id: &str,
    query: Option<String>,
) -> anyhow::Result<MatchOptions> {
    let track = library
        .track(source_id, track_id)?
        .ok_or_else(|| anyhow!("esta canción no está en tu biblioteca"))?;
    let custom = query.map(|q| q.trim().to_string()).filter(|q| !q.is_empty());
    let query = custom.clone().unwrap_or_else(|| default_query(&track));

    let mut candidates = Vec::new();
    let mut last_error = None;
    for source in playback_order(registry) {
        match source.search(&query).await {
            Ok(found) => candidates.extend(found),
            Err(e) => last_error = Some(e),
        }
    }
    if candidates.is_empty() {
        if let Some(e) = last_error {
            return Err(e);
        }
    }
    if custom.is_none() {
        let durations: Vec<u32> = (track.duration_secs > 0).then_some(track.duration_secs).into_iter().collect();
        candidates.sort_by(|a, b| score(&track, &durations, b).total_cmp(&score(&track, &durations, a)));
    }
    candidates.truncate(MAX_OPTIONS);

    let current = library
        .track_match(source_id, track_id)?
        .map(|(source, id)| MatchRef { source, id });
    Ok(MatchOptions { track, query, current, candidates })
}

fn default_query(track: &Track) -> String {
    format!("{} {}", main_artist(&track.artist), trim_featuring(&track.title))
}

/// The source and id to stream for `source/track_id`, matching it if needed.
pub async fn resolve(
    registry: &Registry,
    library: &Library,
    http: &reqwest::Client,
    source_id: &str,
    track_id: &str,
) -> anyhow::Result<(Arc<dyn MusicSource>, String)> {
    if let Ok(source) = registry.get(source_id) {
        return Ok((source, track_id.to_string()));
    }

    let track = library
        .track(source_id, track_id)?
        .ok_or_else(|| anyhow!("fuente desconocida: {source_id}"))?;

    if let Some((play_source, play_id)) = library.track_match(source_id, track_id)? {
        if let Ok(source) = registry.get(&play_source) {
            return Ok((source, play_id));
        }
    }

    let sources = playback_order(registry);
    if sources.is_empty() {
        bail!("no hay ninguna fuente para reproducir «{}»", track.title);
    }
    let durations = if track.duration_secs > 0 {
        vec![track.duration_secs]
    } else {
        // Best effort: without it we judge by title and artist alone.
        let found = lyrics::known_durations(http, &track).await.unwrap_or_default();
        if let Some(&most_common) = found.first() {
            library.set_track_duration(source_id, track_id, most_common)?;
        }
        found
    };
    let query = default_query(&track);
    for source in sources {
        let Ok(candidates) = source.search(&query).await else { continue };
        if let Some(best) = best_match_among(&track, &durations, &candidates) {
            library.set_track_match(source_id, track_id, &best.source, &best.id)?;
            return Ok((source, best.id.clone()));
        }
    }
    bail!(
        "no encontré «{} - {}» en ninguna fuente{}",
        track.artist,
        track.title,
        if registry.get("youtube").is_err() { " (instala el plugin de YouTube)" } else { "" }
    )
}

fn playback_order(registry: &Registry) -> Vec<Arc<dyn MusicSource>> {
    let mut ids: Vec<String> = registry.list().into_iter().map(|s| s.id).collect();
    ids.sort_by_key(|id| PREFERRED.iter().position(|p| p == id).unwrap_or(PREFERRED.len()));
    ids.iter().filter_map(|id| registry.get(id).ok()).collect()
}

/// The candidate that best matches `wanted`, if any is good enough.
#[cfg(test)]
pub fn best_match<'a>(wanted: &Track, candidates: &'a [Track]) -> Option<&'a Track> {
    let durations: Vec<u32> = (wanted.duration_secs > 0).then_some(wanted.duration_secs).into_iter().collect();
    best_match_among(wanted, &durations, candidates)
}

/// Like `best_match`, accepting any of several durations (versions) of the song.
pub fn best_match_among<'a>(wanted: &Track, durations: &[u32], candidates: &'a [Track]) -> Option<&'a Track> {
    candidates
        .iter()
        .map(|c| (score(wanted, durations, c), c))
        .filter(|(s, _)| *s >= MIN_SCORE)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

/// 0..1: how likely `candidate` is the same recording as `wanted`.
fn score(wanted: &Track, durations: &[u32], candidate: &Track) -> f64 {
    let wanted_title = words(trim_featuring(&wanted.title));
    let candidate_text = words(&format!("{} {}", candidate.artist, candidate.title));
    if wanted_title.is_empty() {
        return 0.0;
    }

    // Share of the title's words present in the candidate.
    let title = wanted_title.iter().filter(|w| candidate_text.contains(w)).count() as f64 / wanted_title.len() as f64;

    // The main artist appears in the candidate's artist or title.
    let artist_words = words(main_artist(&wanted.artist));
    let artist = if !artist_words.is_empty() && artist_words.iter().all(|w| candidate_text.contains(w)) { 1.0 } else { 0.0 };

    // Duration: same length ≈ 1, a minute off ≈ 0 (videos often have a few extra seconds),
    // against the closest known version. None known: judge by title and artist alone.
    let duration = durations
        .iter()
        .map(|d| (1.0 - (*d as f64 - candidate.duration_secs as f64).abs() / 60.0).max(0.0))
        .reduce(f64::max);

    // Penalise live versions, covers, remixes... unless the original is one too.
    let wanted_all = words(&wanted.title);
    let other_version = OTHER_VERSIONS
        .iter()
        .any(|w| candidate_text.iter().any(|c| c == w) && !wanted_all.iter().any(|t| t == w));

    let mut s = match duration {
        Some(duration) => 0.5 * title + 0.25 * artist + 0.25 * duration,
        None => (0.5 * title + 0.25 * artist) / 0.75,
    };
    if title < 0.5 {
        s *= 0.5; // a different song by the same artist
    }
    if other_version {
        s -= 0.3;
    }
    if duration.is_none() && candidate.duration_secs > LONG_SECS {
        s -= 0.2;
    }
    s
}

/// Lowercase alphanumeric words, accents removed.
fn words(text: &str) -> Vec<String> {
    let folded: String = text
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    folded.split_whitespace().map(str::to_string).collect()
}

fn main_artist(artist: &str) -> &str {
    artist.split([',', '&']).next().unwrap_or(artist).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(source: &str, id: &str, artist: &str, title: &str, secs: u32) -> Track {
        Track {
            source: source.into(),
            id: id.into(),
            title: title.into(),
            artist: artist.into(),
            duration_secs: secs,
            artwork_url: None,
            genre: None,
        }
    }

    #[test]
    fn picks_the_official_version() {
        let wanted = t("spotify", "1", "Shakira, Burna Boy", "Dai Dai", 223);
        let candidates = vec![
            t("youtube", "live", "Shakira", "Dai Dai (Live at the Final)", 250),
            t("youtube", "other", "Shakira", "Hips Don't Lie", 218),
            t("youtube", "ok", "Shakira, Burna Boy", "Dai Dai", 241),
            t("youtube", "karaoke", "Sing King", "Shakira - Dai Dai (Karaoke Version)", 224),
        ];
        assert_eq!(best_match(&wanted, &candidates).unwrap().id, "ok");
    }

    #[test]
    fn keeps_remixes_when_asked_for() {
        let wanted = t("spotify", "1", "Artista", "Canción (Remix)", 200);
        let candidates = vec![
            t("youtube", "orig", "Artista", "Canción", 180),
            t("youtube", "remix", "Artista", "Canción (Remix)", 201),
        ];
        assert_eq!(best_match(&wanted, &candidates).unwrap().id, "remix");
    }

    #[test]
    fn rejects_unrelated_results() {
        let wanted = t("spotify", "1", "Rosalía", "Despechá", 157);
        let candidates = vec![t("youtube", "x", "Otro", "Algo distinto", 157)];
        assert!(best_match(&wanted, &candidates).is_none());
    }

    #[test]
    fn matches_without_a_known_duration() {
        // Spotify's data export has no durations.
        let wanted = t("spotify", "1", "Rosalía", "Saoko", 0);
        let candidates = vec![
            t("youtube", "live", "Rosalía", "Saoko (Live)", 160),
            t("youtube", "ok", "ROSALÍA", "SAOKO (Official Video)", 137),
        ];
        assert_eq!(best_match(&wanted, &candidates).unwrap().id, "ok");
        let unrelated = vec![t("youtube", "x", "Otro", "Algo distinto", 137)];
        assert!(best_match(&wanted, &unrelated).is_none());
    }

    /// A playable source that answers every search with the same results.
    struct FakeSource(Vec<Track>);

    #[async_trait::async_trait]
    impl MusicSource for FakeSource {
        fn info(&self) -> crate::models::SourceInfo {
            crate::models::SourceInfo {
                id: "youtube".into(),
                name: "YouTube".into(),
                kind: crate::models::SourceKind::Plugin,
                version: None,
                description: None,
                supports_trending: false,
                is_library: false,
            }
        }
        async fn search(&self, _query: &str) -> anyhow::Result<Vec<Track>> {
            Ok(self.0.clone())
        }
        async fn trending(&self) -> anyhow::Result<Vec<Track>> {
            Ok(Vec::new())
        }
        async fn stream(&self, _id: &str) -> anyhow::Result<crate::models::StreamInfo> {
            anyhow::bail!("sin audio")
        }
    }

    #[tokio::test]
    async fn offers_candidates_to_fix_a_match() {
        let results = vec![
            t("youtube", "live", "Shakira", "Dai Dai (Live)", 250),
            t("youtube", "ok", "Shakira", "Dai Dai (Official Video)", 225),
            t("youtube", "other", "Shakira", "Hips Don't Lie", 218),
        ];
        let registry = Registry::new(vec![Arc::new(FakeSource(results))]);
        let library = Library::in_memory();
        library.import_playlist("spotify", "liked", "Me gusta", &[t("spotify", "1", "Shakira, Burna Boy", "Dai Dai", 223)]).unwrap();
        library.set_track_match("spotify", "1", "youtube", "live").unwrap();

        let options = options(&registry, &library, "spotify", "1", None).await.unwrap();
        assert_eq!(options.query, "Shakira Dai Dai");
        assert_eq!(options.candidates[0].id, "ok", "best scored first");
        assert_eq!(options.current, Some(MatchRef { source: "youtube".into(), id: "live".into() }));

        // A custom search keeps the source's own order.
        let custom = options_with(&registry, &library, "dai dai shakira").await;
        let ids: Vec<&str> = custom.candidates.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["live", "ok", "other"]);
        assert_eq!(custom.query, "dai dai shakira");

        assert!(super::options(&registry, &library, "spotify", "missing", None).await.is_err());
    }

    async fn options_with(registry: &Registry, library: &Library, query: &str) -> MatchOptions {
        options(registry, library, "spotify", "1", Some(format!("  {query} "))).await.unwrap()
    }

    #[test]
    fn accepts_any_known_version_length() {
        let wanted = t("spotify", "1", "Bad Bunny", "Tití Me Preguntó", 0);
        let candidates = vec![
            t("youtube", "tv", "Bad Bunny", "Tití Me Preguntó (Darwin) | The Voice", 703),
            t("youtube", "ok", "Bad Bunny", "Tití Me Preguntó (Video Oficial)", 290),
        ];
        assert_eq!(best_match_among(&wanted, &[244, 290], &candidates).unwrap().id, "ok");
        // Even with no known duration, a 12-minute video loses to a song-length one.
        assert_eq!(best_match_among(&wanted, &[], &candidates).unwrap().id, "ok");
    }

    #[test]
    fn ignores_accents_and_case() {
        assert_eq!(words("Despechá — ROSALÍA"), ["despecha", "rosalia"]);
        let wanted = t("spotify", "1", "ROSALÍA", "DESPECHÁ", 157);
        let candidates = vec![t("youtube", "ok", "Rosalia", "Despecha (Official Video)", 160)];
        assert_eq!(best_match(&wanted, &candidates).unwrap().id, "ok");
    }
}
