//! What was played: the history and the monthly summary.

use super::*;

impl Library {
    // ---------- History ----------

    /// Records that `track` was listened to now.
    pub fn record_play(&self, track: &Track) -> anyhow::Result<()> {
        self.record_play_at(track, now())
    }

    pub(super) fn record_play_at(&self, track: &Track, played_at: i64) -> anyhow::Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            upsert_track(&tx, track)?;
            tx.execute(
                "INSERT INTO plays (source, id, played_at) VALUES (?1, ?2, ?3)",
                params![track.source, track.id, played_at],
            )?;
            let last = tx.last_insert_rowid();
            tx.execute("DELETE FROM plays WHERE play_id <= ?1", [last - MAX_PLAYS])?;
            tx.commit()
        })
    }

    /// The latest plays, newest first.
    pub fn recent_plays(&self, limit: u32) -> anyhow::Result<Vec<Play>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT t.*, p.played_at FROM plays p JOIN tracks t USING (source, id)
                 ORDER BY p.played_at DESC, p.play_id DESC LIMIT ?1",
            )?;
            let rows =
                stmt.query_map([limit], |r| Ok(Play { track: track_from_row(r)?, played_at: r.get("played_at")? }))?;
            rows.collect()
        })
    }

    /// Months ("YYYY-MM", local time) with plays, newest first.
    pub fn summary_months(&self) -> anyhow::Result<Vec<String>> {
        self.with(|c| {
            c.prepare(
                "SELECT DISTINCT strftime('%Y-%m', played_at / 1000, 'unixepoch', 'localtime') AS m
                 FROM plays ORDER BY m DESC",
            )?
            .query_map([], |r| r.get(0))?
            .collect()
        })
    }

    /// Totals and favourites of `month` ("YYYY-MM", local time).
    pub fn month_summary(&self, month: &str) -> anyhow::Result<MonthSummary> {
        const TOP_TRACKS: usize = 10;
        const TOP_ARTISTS: usize = 5;
        let rows: Vec<(Track, i64)> = self.with(|c| {
            c.prepare(
                "SELECT t.*, p.played_at FROM plays p JOIN tracks t USING (source, id)
                 WHERE strftime('%Y-%m', p.played_at / 1000, 'unixepoch', 'localtime') = ?1
                 ORDER BY p.played_at DESC",
            )?
            .query_map([month], |r| Ok((track_from_row(r)?, r.get("played_at")?)))?
            .collect()
        })?;

        // Per song: plays and the latest one (ties go to the most recent). The vectors keep
        // first-seen order (newest first); the maps find an entry without scanning them.
        use std::collections::HashMap;
        let mut songs: Vec<(Track, u32)> = Vec::new();
        let mut song_at: HashMap<(&str, &str), usize> = HashMap::new();
        let mut artists: Vec<(String, u32, Option<String>)> = Vec::new();
        let mut artist_at: HashMap<String, usize> = HashMap::new();
        let mut minutes_secs: u64 = 0;
        for (track, _) in &rows {
            minutes_secs += track.duration_secs as u64;
            match song_at.get(&(track.source.as_str(), track.id.as_str())) {
                Some(&i) => songs[i].1 += 1,
                None => {
                    song_at.insert((&track.source, &track.id), songs.len());
                    songs.push((track.clone(), 1));
                }
            }
            for name in split_artists(&track.artist) {
                match artist_at.get(&name.to_lowercase()) {
                    Some(&i) => {
                        let (_, n, art) = &mut artists[i];
                        *n += 1;
                        if art.is_none() {
                            art.clone_from(&track.artwork_url);
                        }
                    }
                    None => {
                        artist_at.insert(name.to_lowercase(), artists.len());
                        artists.push((name.to_string(), 1, track.artwork_url.clone()));
                    }
                }
            }
        }
        let distinct_songs = songs.len() as u32;
        let distinct_artists = artists.len() as u32;
        // Stable sorts: equal counts keep the most recent first.
        songs.sort_by_key(|s| std::cmp::Reverse(s.1));
        artists.sort_by_key(|a| std::cmp::Reverse(a.1));
        Ok(MonthSummary {
            month: month.to_string(),
            plays: rows.len() as u32,
            minutes: (minutes_secs / 60) as u32,
            songs: distinct_songs,
            artists: distinct_artists,
            top_tracks: songs.into_iter().take(TOP_TRACKS).map(|(track, plays)| TrackCount { track, plays }).collect(),
            top_artists: artists
                .into_iter()
                .take(TOP_ARTISTS)
                .map(|(name, plays, artwork_url)| ArtistCount { name, plays, artwork_url })
                .collect(),
        })
    }

    pub fn clear_history(&self) -> anyhow::Result<()> {
        self.with(|c| c.execute("DELETE FROM plays", []).map(drop))
    }
}
