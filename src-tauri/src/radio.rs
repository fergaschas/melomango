//! Automatic radio: when the queue runs out, more songs like the last one.
//!
//! Sources that know related songs (the YouTube plugin, from each video's "Mix")
//! answer `radio`; for the rest, more songs by the same artist. Tracks from
//! metadata-only services (Spotify) start from the recording they were matched to.

use std::collections::HashSet;

use crate::{
    library::Library,
    matcher,
    models::Track,
    text::{main_artist, trim_featuring},
    sources::Registry,
};

/// Songs added to the queue each time the radio fills it.
const BATCH: usize = 25;
/// Longer results are usually concerts or mixes (unless the seed is that long too).
const MAX_SECS: u32 = 10 * 60;

/// Songs like `seed`, without any whose "source/id" is in `exclude` (already queued).
pub async fn tracks(
    registry: &Registry,
    library: &Library,
    http: &reqwest::Client,
    seed: &Track,
    exclude: &HashSet<String>,
) -> anyhow::Result<Vec<Track>> {
    let (source, play_id) = matcher::resolve(registry, library, http, &seed.source, &seed.id).await?;
    let found = match source.radio(&play_id).await {
        Ok(Some(found)) if !found.is_empty() => found,
        // No radio here (or it failed): more by the same artist.
        _ => source.search(main_artist(&seed.artist)).await?,
    };
    Ok(pick(seed, &play_id, found, exclude))
}

/// Drops the seed, what is already queued and repeated songs (the same song often has
/// several uploads), keeping the source's order.
fn pick(seed: &Track, play_id: &str, found: Vec<Track>, exclude: &HashSet<String>) -> Vec<Track> {
    let mut seen_keys: HashSet<String> = exclude.clone();
    seen_keys.insert(format!("{}/{}", seed.source, seed.id));
    let mut seen_songs: HashSet<String> = HashSet::from([song_key(seed)]);
    let max_secs = MAX_SECS.max(seed.duration_secs * 3 / 2);
    found
        .into_iter()
        .filter(|t| t.id != play_id)
        .filter(|t| t.duration_secs <= max_secs)
        .filter(|t| seen_keys.insert(format!("{}/{}", t.source, t.id)))
        .filter(|t| seen_songs.insert(song_key(t)))
        .take(BATCH)
        .collect()
}

/// "artist|title" in lowercase, to spot the same song uploaded twice.
fn song_key(t: &Track) -> String {
    format!("{}|{}", main_artist(&t.artist).to_lowercase(), trim_featuring(&t.title).to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(source: &str, id: &str, artist: &str, title: &str) -> Track {
        Track {
            source: source.into(),
            id: id.into(),
            title: title.into(),
            artist: artist.into(),
            duration_secs: 200,
            artwork_url: None,
            genre: None,
        }
    }

    #[test]
    fn skips_the_seed_queued_and_repeated_songs() {
        let seed = t("spotify", "s1", "Vetusta Morla", "Copenhague");
        let found = vec![
            t("youtube", "seedvid", "Vetusta Morla", "Copenhague"), // the recording it plays from
            t("youtube", "a", "Vetusta Morla", "Copenhague"),       // another upload of the seed
            t("youtube", "b", "Vetusta Morla", "Cuarteles de Invierno"),
            t("youtube", "c", "Vetusta Morla", "Cuarteles de invierno"), // same song again
            t("youtube", "d", "Love of Lesbian", "Club de fans de John Boy"),
            t("youtube", "e", "Izal", "Pequeña gran revolución"),
        ];
        let exclude = HashSet::from(["youtube/d".to_string()]);
        let picked: Vec<String> = pick(&seed, "seedvid", found, &exclude).into_iter().map(|t| t.id).collect();
        assert_eq!(picked, ["b", "e"]);
    }

    #[test]
    fn leaves_out_concert_length_videos() {
        let seed = t("youtube", "x", "A", "Seed");
        let mut long = t("youtube", "live", "A", "Directo completo");
        long.duration_secs = 680;
        let picked = pick(&seed, "x", vec![long.clone(), t("youtube", "ok", "B", "Song")], &HashSet::new());
        assert_eq!(picked.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["ok"]);
        // A long seed (a 15-minute piece) accepts long results.
        let mut long_seed = seed.clone();
        long_seed.duration_secs = 900;
        assert_eq!(pick(&long_seed, "x", vec![long], &HashSet::new()).len(), 1);
    }

    #[test]
    fn keeps_at_most_a_batch() {
        let seed = t("youtube", "x", "A", "Seed");
        let found = (0..60).map(|i| t("youtube", &format!("v{i}"), "B", &format!("Song {i}"))).collect();
        assert_eq!(pick(&seed, "x", found, &HashSet::new()).len(), BATCH);
    }
}
