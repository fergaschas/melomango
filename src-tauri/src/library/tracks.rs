//! Songs' metadata, and which recording plays each one (matches).

use super::*;

impl Library {
    // ---------- Track metadata and matches ----------

    /// Stores tracks' metadata (e.g. from artist and album pages) so they can be played
    /// through a match even if they are not in any playlist.
    pub fn remember_tracks(&self, tracks: &[Track]) -> anyhow::Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            for track in tracks {
                upsert_track(&tx, track)?;
            }
            tx.commit()
        })
    }

    /// Fills in the duration of a track imported without one.
    pub fn set_track_duration(&self, source: &str, id: &str, duration_secs: u32) -> anyhow::Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE tracks SET duration_secs = ?3 WHERE source = ?1 AND id = ?2 AND duration_secs = 0",
                params![source, id, duration_secs],
            )?;
            Ok(())
        })
    }

    pub fn track(&self, source: &str, id: &str) -> anyhow::Result<Option<Track>> {
        self.with(|c| {
            c.query_row("SELECT * FROM tracks WHERE source = ?1 AND id = ?2", params![source, id], track_from_row)
                .optional()
        })
    }

    /// The playable (source, id) a metadata-only track was matched to.
    pub fn track_match(&self, source: &str, id: &str) -> anyhow::Result<Option<(String, String)>> {
        self.with(|c| {
            c.query_row(
                "SELECT play_source, play_id FROM track_matches WHERE source = ?1 AND id = ?2",
                params![source, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
        })
    }

    pub fn set_track_match(&self, source: &str, id: &str, play_source: &str, play_id: &str) -> anyhow::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO track_matches (source, id, play_source, play_id, matched_at) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (source, id) DO UPDATE SET
                   play_source = excluded.play_source, play_id = excluded.play_id, matched_at = excluded.matched_at",
                params![source, id, play_source, play_id, now()],
            )?;
            Ok(())
        })
    }
}
