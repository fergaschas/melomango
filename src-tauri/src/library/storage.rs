//! Saved songs: downloads and the automatic cache.

use super::*;

impl Library {
    // ---------- Downloads ----------

    pub fn add_download(&self, track: &Track, file: &DownloadFile, kind: StoredKind) -> anyhow::Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            upsert_track(&tx, track)?;
            tx.execute(
                "INSERT OR REPLACE INTO downloads (source, id, file, mime, size, kind, downloaded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![track.source, track.id, file.file, file.mime, file.size, kind.as_str(), now()],
            )?;
            tx.commit()
        })
    }

    pub fn stored_kind(&self, source: &str, id: &str) -> anyhow::Result<Option<StoredKind>> {
        self.with(|c| {
            c.query_row("SELECT kind FROM downloads WHERE source = ?1 AND id = ?2", params![source, id], |r| {
                r.get::<_, String>(0)
            })
            .optional()
        })
        .map(|k| k.map(|k| StoredKind::parse(&k)))
    }

    /// A cached song the user now downloads: kept for good, no need to fetch it again.
    pub fn keep_cached(&self, source: &str, id: &str) -> anyhow::Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE downloads SET kind = 'download', downloaded_at = ?3 WHERE source = ?1 AND id = ?2",
                params![source, id, now()],
            )
        })?;
        Ok(())
    }

    /// Songs and bytes in the cache.
    pub fn cache_usage(&self) -> anyhow::Result<(u32, u64)> {
        self.with(|c| {
            c.query_row("SELECT COUNT(*), COALESCE(SUM(size), 0) FROM downloads WHERE kind = 'cache'", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
        })
    }

    /// Cached songs, the ones to drop first (least recently played) first.
    pub fn cache_eviction_order(&self) -> anyhow::Result<Vec<CachedSong>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT d.source, d.id, d.file, d.size FROM downloads d
                 WHERE d.kind = 'cache'
                 ORDER BY MAX(d.downloaded_at, COALESCE(
                     (SELECT MAX(p.played_at) FROM plays p WHERE p.source = d.source AND p.id = d.id), 0)),
                   d.rowid",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(CachedSong { source: r.get(0)?, id: r.get(1)?, file: r.get(2)?, size: r.get(3)? })
            })?;
            rows.collect()
        })
    }

    /// Whether a song is worth caching: played more than once, or liked.
    pub fn is_favourite(&self, source: &str, id: &str) -> anyhow::Result<bool> {
        self.with(|c| {
            c.query_row(
                "SELECT (SELECT COUNT(*) FROM plays WHERE source = ?1 AND id = ?2) >= 2
                     OR EXISTS (SELECT 1 FROM likes WHERE source = ?1 AND id = ?2)",
                params![source, id],
                |r| r.get(0),
            )
        })
    }

    /// Downloaded songs, newest first.
    pub fn downloads(&self) -> anyhow::Result<Vec<Download>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT t.*, d.size, d.downloaded_at FROM downloads d JOIN tracks t USING (source, id)
                 WHERE d.kind = 'download' ORDER BY d.downloaded_at DESC, d.rowid DESC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(Download { track: track_from_row(r)?, size: r.get("size")?, downloaded_at: r.get("downloaded_at")? })
            })?;
            rows.collect()
        })
    }

    /// "source/id" of every downloaded song.
    pub fn downloaded_keys(&self) -> anyhow::Result<Vec<String>> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT source || '/' || id FROM downloads WHERE kind = 'download'")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect()
        })
    }

    pub fn download_file(&self, source: &str, id: &str) -> anyhow::Result<Option<DownloadFile>> {
        self.with(|c| {
            c.query_row(
                "SELECT file, mime, size FROM downloads WHERE source = ?1 AND id = ?2",
                params![source, id],
                |r| Ok(DownloadFile { file: r.get(0)?, mime: r.get(1)?, size: r.get(2)? }),
            )
            .optional()
        })
    }

    /// The file names of every saved song (downloads and cache).
    pub fn stored_files(&self) -> anyhow::Result<std::collections::HashSet<String>> {
        self.with(|c| c.prepare("SELECT file FROM downloads")?.query_map([], |r| r.get(0))?.collect())
    }

    /// Forgets a download; returns its file name so the caller can delete it.
    pub fn remove_download(&self, source: &str, id: &str) -> anyhow::Result<Option<String>> {
        self.with(|c| {
            c.query_row(
                "DELETE FROM downloads WHERE source = ?1 AND id = ?2 RETURNING file",
                params![source, id],
                |r| r.get(0),
            )
            .optional()
        })
    }
}
