//! Liked songs.

use super::*;

impl Library {
    // ---------- Likes ----------

    pub fn liked_tracks(&self) -> anyhow::Result<Vec<Track>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT t.* FROM likes l JOIN tracks t USING (source, id) ORDER BY l.liked_at DESC, l.rowid DESC",
            )?;
            let rows = stmt.query_map([], track_from_row)?;
            rows.collect()
        })
    }

    /// "source/id" of every liked track, for quick lookups in the UI.
    pub fn liked_keys(&self) -> anyhow::Result<Vec<String>> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT source || '/' || id FROM likes")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect()
        })
    }

    pub fn set_liked(&self, track: &Track, liked: bool) -> anyhow::Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            if liked {
                upsert_track(&tx, track)?;
                tx.execute(
                    "INSERT OR IGNORE INTO likes (source, id, liked_at) VALUES (?1, ?2, ?3)",
                    params![track.source, track.id, now()],
                )?;
            } else {
                tx.execute("DELETE FROM likes WHERE source = ?1 AND id = ?2", params![track.source, track.id])?;
            }
            tx.commit()
        })
    }
}
