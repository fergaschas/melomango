//! The user's playlists, and those imported from other services (kept in sync).

use super::*;

impl Library {
    // ---------- Playlists ----------

    pub fn playlists(&self) -> anyhow::Result<Vec<PlaylistSummary>> {
        self.with(|c| {
            let ids: Vec<i64> = c
                .prepare("SELECT id FROM playlists ORDER BY updated_at DESC, id DESC")?
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            ids.into_iter().map(|id| summary(c, id)).collect()
        })
    }

    pub fn create_playlist(&self, name: &str) -> anyhow::Result<PlaylistSummary> {
        let name = clean_name(name)?;
        self.with(|c| {
            let ts = now();
            c.execute("INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)", params![name, ts])?;
            summary(c, c.last_insert_rowid())
        })
    }

    /// A new playlist with `tracks` (duplicates once), in one go.
    pub fn create_playlist_with(&self, name: &str, tracks: &[Track]) -> anyhow::Result<PlaylistSummary> {
        let name = clean_name(name)?;
        self.with(|c| {
            let tx = c.transaction()?;
            let ts = now();
            tx.execute("INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)", params![name, ts])?;
            let id = tx.last_insert_rowid();
            let mut position = 0i64;
            for track in tracks {
                upsert_track(&tx, track)?;
                position += tx.execute(
                    "INSERT OR IGNORE INTO playlist_tracks (playlist_id, source, id, position, added_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, track.source, track.id, position, ts],
                )? as i64;
            }
            let result = summary(&tx, id)?;
            tx.commit()?;
            Ok(result)
        })
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> anyhow::Result<()> {
        let name = clean_name(name)?;
        let changed = self.with(|c| {
            c.execute("UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1", params![id, name, now()])
        })?;
        ensure_found(changed)
    }

    pub fn delete_playlist(&self, id: i64) -> anyhow::Result<()> {
        let changed = self.with(|c| c.execute("DELETE FROM playlists WHERE id = ?1", [id]))?;
        ensure_found(changed)
    }

    pub fn playlist(&self, id: i64) -> anyhow::Result<Playlist> {
        let exists =
            self.with(|c| c.query_row("SELECT 1 FROM playlists WHERE id = ?1", [id], |_| Ok(())).optional())?;
        if exists.is_none() {
            bail!("la playlist no existe");
        }
        self.with(|c| {
            let rows = c
                .prepare(
                    "SELECT t.*, p.added_at FROM playlist_tracks p JOIN tracks t USING (source, id)
                     WHERE p.playlist_id = ?1 ORDER BY p.position",
                )?
                .query_map([id], |r| Ok((track_from_row(r)?, r.get::<_, i64>("added_at")?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let (tracks, added_at) = rows.into_iter().unzip();
            Ok(Playlist { summary: summary(c, id)?, tracks, added_at, link: link(c, id)? })
        })
    }

    /// Appends `track`. Returns false if it was already in the playlist.
    pub fn add_to_playlist(&self, id: i64, track: &Track) -> anyhow::Result<bool> {
        self.playlist_exists(id)?;
        self.with(|c| {
            let tx = c.transaction()?;
            upsert_track(&tx, track)?;
            let added = tx.execute(
                "INSERT OR IGNORE INTO playlist_tracks (playlist_id, source, id, position, added_at)
                 VALUES (?1, ?2, ?3,
                         (SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?1),
                         ?4)",
                params![id, track.source, track.id, now()],
            )? == 1;
            if added {
                touch(&tx, id)?;
            }
            tx.commit()?;
            Ok(added)
        })
    }

    pub fn remove_from_playlist(&self, id: i64, source: &str, track_id: &str) -> anyhow::Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            tx.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND source = ?2 AND id = ?3",
                params![id, source, track_id],
            )?;
            renumber(&tx, id, None)?;
            touch(&tx, id)?;
            tx.commit()
        })
    }

    /// Moves the track at index `from` to index `to` (0-based, as shown in the UI).
    pub fn move_in_playlist(&self, id: i64, from: usize, to: usize) -> anyhow::Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            renumber(&tx, id, Some((from, to)))?;
            touch(&tx, id)?;
            tx.commit()
        })
    }

    // ---------- Imported playlists ----------

    /// Creates (or refreshes) the playlist linked to `remote_id` of `plugin_id` with
    /// exactly `tracks`, in order. A previously imported playlist keeps its name; a new one
    /// without a name gets a default one.
    pub fn import_playlist(
        &self,
        plugin_id: &str,
        remote_id: &str,
        name: &str,
        tracks: &[Track],
    ) -> anyhow::Result<PlaylistSummary> {
        let mut imported = self.import_many(plugin_id, &[(remote_id, name, tracks)])?;
        Ok(imported.remove(0))
    }

    /// [`import_playlist`](Self::import_playlist) for several playlists, all or none: in
    /// one transaction, so a failure halfway doesn't leave half an import saved.
    pub fn import_many(
        &self,
        plugin_id: &str,
        playlists: &[(&str, &str, &[Track])],
    ) -> anyhow::Result<Vec<PlaylistSummary>> {
        self.with(|c| {
            let tx = c.transaction()?;
            let ts = now();
            let imported = playlists
                .iter()
                .map(|(remote_id, name, tracks)| {
                    let name = clean_name(name).unwrap_or_else(|_| "Playlist importada".into());
                    import_into(&tx, plugin_id, remote_id, &name, tracks, ts)
                })
                .collect::<rusqlite::Result<Vec<_>>>()?;
            tx.commit()?;
            Ok(imported)
        })
    }

    pub fn playlist_link(&self, id: i64) -> anyhow::Result<Option<PlaylistLink>> {
        self.with(|c| link(c, id))
    }

    /// remote id -> playlist id, for everything imported from `plugin_id`.
    pub fn imported_from(&self, plugin_id: &str) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        self.with(|c| {
            c.prepare("SELECT remote_id, playlist_id FROM playlist_links WHERE plugin_id = ?1")?
                .query_map([plugin_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })
    }

    fn playlist_exists(&self, id: i64) -> anyhow::Result<()> {
        let found = self.with(|c| c.query_row("SELECT 1 FROM playlists WHERE id = ?1", [id], |_| Ok(())).optional())?;
        if found.is_none() {
            bail!("la playlist no existe");
        }
        Ok(())
    }
}

/// One playlist of an import, inside the caller's transaction.
fn import_into(
    tx: &rusqlite::Transaction,
    plugin_id: &str,
    remote_id: &str,
    name: &str,
    tracks: &[Track],
    ts: i64,
) -> rusqlite::Result<PlaylistSummary> {
    let existing: Option<i64> = tx
        .query_row(
            "SELECT playlist_id FROM playlist_links WHERE plugin_id = ?1 AND remote_id = ?2",
            params![plugin_id, remote_id],
            |r| r.get(0),
        )
        .optional()?;
    let id = match existing {
        Some(id) => id,
        None => {
            tx.execute("INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)", params![name, ts])?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO playlist_links (playlist_id, plugin_id, remote_id, synced_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, plugin_id, remote_id, ts],
            )?;
            id
        }
    };
    // Songs already there keep the date they were added; only new ones get today's.
    let added_before: std::collections::HashMap<(String, String), i64> = tx
        .prepare("SELECT source, id, added_at FROM playlist_tracks WHERE playlist_id = ?1")?
        .query_map([id], |r| Ok(((r.get(0)?, r.get(1)?), r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", [id])?;
    let mut position = 0i64;
    for track in tracks {
        upsert_track(tx, track)?;
        let added_at = added_before.get(&(track.source.clone(), track.id.clone())).copied().unwrap_or(ts);
        position += tx.execute(
            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, source, id, position, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, track.source, track.id, position, added_at],
        )? as i64;
    }
    tx.execute("UPDATE playlist_links SET synced_at = ?2 WHERE playlist_id = ?1", params![id, ts])?;
    touch(tx, id)?;
    summary(tx, id)
}

fn clean_name(name: &str) -> anyhow::Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("la playlist necesita un nombre");
    }
    Ok(name.chars().take(100).collect())
}

fn ensure_found(changed: usize) -> anyhow::Result<()> {
    if changed == 0 {
        bail!("la playlist no existe");
    }
    Ok(())
}

fn touch(c: &Connection, playlist_id: i64) -> rusqlite::Result<()> {
    c.execute("UPDATE playlists SET updated_at = ?2 WHERE id = ?1", params![playlist_id, now()])?;
    Ok(())
}

/// Rewrites positions as 0..n, optionally moving one item first.
fn renumber(c: &Connection, playlist_id: i64, mv: Option<(usize, usize)>) -> rusqlite::Result<()> {
    let mut keys: Vec<(String, String)> = c
        .prepare("SELECT source, id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position")?
        .query_map([playlist_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if let Some((from, to)) = mv
        && from < keys.len()
    {
        let item = keys.remove(from);
        keys.insert(to.min(keys.len()), item);
    }
    let mut stmt =
        c.prepare("UPDATE playlist_tracks SET position = ?4 WHERE playlist_id = ?1 AND source = ?2 AND id = ?3")?;
    for (position, (source, id)) in keys.iter().enumerate() {
        stmt.execute(params![playlist_id, source, id, position as i64])?;
    }
    Ok(())
}

fn summary(c: &Connection, id: i64) -> rusqlite::Result<PlaylistSummary> {
    let (name, track_count, duration_secs): (String, u32, u32) = c.query_row(
        "SELECT p.name, COUNT(t.id), COALESCE(SUM(t.duration_secs), 0)
         FROM playlists p
         LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
         LEFT JOIN tracks t ON t.source = pt.source AND t.id = pt.id
         WHERE p.id = ?1 GROUP BY p.id",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let artwork_urls = c
        .prepare(
            "SELECT t.artwork_url FROM playlist_tracks pt JOIN tracks t USING (source, id)
             WHERE pt.playlist_id = ?1 AND t.artwork_url IS NOT NULL ORDER BY pt.position LIMIT 4",
        )?
        .query_map([id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(PlaylistSummary { id, name, track_count, duration_secs, artwork_urls })
}

fn link(c: &Connection, playlist_id: i64) -> rusqlite::Result<Option<PlaylistLink>> {
    c.query_row(
        "SELECT plugin_id, remote_id, synced_at FROM playlist_links WHERE playlist_id = ?1",
        [playlist_id],
        |r| Ok(PlaylistLink { plugin_id: r.get(0)?, remote_id: r.get(1)?, synced_at: r.get(2)? }),
    )
    .optional()
}
