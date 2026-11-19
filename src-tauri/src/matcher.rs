//! Matching metadata-only tracks (e.g. imported from Spotify) to playable ones.
//!
//! When such a track is played, we search the playable sources for "artist title",
//! score the candidates by title, artist and duration, and remember the winner in the
//! library so the search only happens once per track.

use std::sync::Arc;

use anyhow::{anyhow, bail};

use crate::{
    library::Library,
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

/// The source and id to stream for `source/track_id`, matching it if needed.
pub async fn resolve(
    registry: &Registry,
    library: &Library,
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
    let query = format!("{} {}", main_artist(&track.artist), trim_featuring(&track.title));
    for source in sources {
        let Ok(candidates) = source.search(&query).await else { continue };
        if let Some(best) = best_match(&track, &candidates) {
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
pub fn best_match<'a>(wanted: &Track, candidates: &'a [Track]) -> Option<&'a Track> {
    candidates
        .iter()
        .map(|c| (score(wanted, c), c))
        .filter(|(s, _)| *s >= MIN_SCORE)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

/// 0..1: how likely `candidate` is the same recording as `wanted`.
fn score(wanted: &Track, candidate: &Track) -> f64 {
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

    // Duration: same length ≈ 1, a minute off ≈ 0. Videos often have a few extra seconds.
    let diff = (wanted.duration_secs as f64 - candidate.duration_secs as f64).abs();
    let duration = (1.0 - diff / 60.0).max(0.0);

    // Penalise live versions, covers, remixes... unless the original is one too.
    let wanted_all = words(&wanted.title);
    let other_version = OTHER_VERSIONS
        .iter()
        .any(|w| candidate_text.iter().any(|c| c == w) && !wanted_all.iter().any(|t| t == w));

    let mut s = 0.5 * title + 0.25 * artist + 0.25 * duration;
    if title < 0.5 {
        s *= 0.5; // a different song by the same artist
    }
    if other_version {
        s -= 0.3;
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
    fn ignores_accents_and_case() {
        assert_eq!(words("Despechá — ROSALÍA"), ["despecha", "rosalia"]);
        let wanted = t("spotify", "1", "ROSALÍA", "DESPECHÁ", 157);
        let candidates = vec![t("youtube", "ok", "Rosalia", "Despecha (Official Video)", 160)];
        assert_eq!(best_match(&wanted, &candidates).unwrap().id, "ok");
    }
}
