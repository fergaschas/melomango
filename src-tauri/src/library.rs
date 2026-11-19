//! The user's library: liked tracks and playlists, stored in SQLite (`library.db` in
//! the app data folder).
//!
//! Track metadata is copied into the database when a track is liked or added to a
//! playlist, so the library still shows it even if the source is unavailable.

use std::{
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::models::Track;

const SCHEMA_VERSION: i32 = 2;

const SCHEMA_V1: &str = "
CREATE TABLE tracks (
    source        TEXT NOT NULL,
    id            TEXT NOT NULL,
    title         TEXT NOT NULL,
    artist        TEXT NOT NULL,
    duration_secs INTEGER NOT NULL,
    artwork_url   TEXT,
    genre         TEXT,
    PRIMARY KEY (source, id)
);
CREATE TABLE likes (
    source   TEXT NOT NULL,
    id       TEXT NOT NULL,
    liked_at INTEGER NOT NULL,
    PRIMARY KEY (source, id),
    FOREIGN KEY (source, id) REFERENCES tracks (source, id)
);
CREATE TABLE playlists (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    source      TEXT NOT NULL,
    id          TEXT NOT NULL,
    position    INTEGER NOT NULL,
    added_at    INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, source, id),
    FOREIGN KEY (source, id) REFERENCES tracks (source, id)
);
CREATE INDEX playlist_tracks_order ON playlist_tracks (playlist_id, position);
";

/// v2: playlists imported from another service, and which playable track each
/// metadata-only track (e.g. from Spotify) was matched to.
const SCHEMA_V2: &str = "
CREATE TABLE playlist_links (
    playlist_id INTEGER PRIMARY KEY REFERENCES playlists (id) ON DELETE CASCADE,
    plugin_id   TEXT NOT NULL,
    remote_id   TEXT NOT NULL,
    synced_at   INTEGER NOT NULL,
    UNIQUE (plugin_id, remote_id)
);
CREATE TABLE track_matches (
    source      TEXT NOT NULL,
    id          TEXT NOT NULL,
    play_source TEXT NOT NULL,
    play_id     TEXT NOT NULL,
    matched_at  INTEGER NOT NULL,
    PRIMARY KEY (source, id)
);
";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub track_count: u32,
    pub duration_secs: u32,
    /// Covers of the first tracks, for a mosaic.
    pub artwork_urls: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Playlist {
    #[serde(flatten)]
    pub summary: PlaylistSummary,
    pub tracks: Vec<Track>,
    /// Set when the playlist was imported from another service and can be re-synced.
    pub link: Option<PlaylistLink>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistLink {
    pub plugin_id: String,
    pub remote_id: String,
    pub synced_at: i64,
}

pub struct Library {
    conn: Mutex<Connection>,
}

impl Library {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("no se pudo abrir la biblioteca {}", path.display()))?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn init(conn: Connection) -> anyhow::Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            bail!("la biblioteca es de una versión más nueva de Melomango");
        }
        if version < 1 {
            conn.execute_batch(SCHEMA_V1)?;
        }
        if version < 2 {
            conn.execute_batch(SCHEMA_V2)?;
        }
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> anyhow::Result<T> {
        let mut conn = self.conn.lock().unwrap();
        Ok(f(&mut conn)?)
    }

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
                tx.execute(
                    "DELETE FROM likes WHERE source = ?1 AND id = ?2",
                    params![track.source, track.id],
                )?;
            }
            tx.commit()
        })
    }

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
            c.execute(
                "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
                params![name, ts],
            )?;
            summary(c, c.last_insert_rowid())
        })
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> anyhow::Result<()> {
        let name = clean_name(name)?;
        let changed = self.with(|c| {
            c.execute(
                "UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, name, now()],
            )
        })?;
        ensure_found(changed)
    }

    pub fn delete_playlist(&self, id: i64) -> anyhow::Result<()> {
        let changed = self.with(|c| c.execute("DELETE FROM playlists WHERE id = ?1", [id]))?;
        ensure_found(changed)
    }

    pub fn playlist(&self, id: i64) -> anyhow::Result<Playlist> {
        let exists = self.with(|c| {
            c.query_row("SELECT 1 FROM playlists WHERE id = ?1", [id], |_| Ok(()))
                .optional()
        })?;
        if exists.is_none() {
            bail!("la playlist no existe");
        }
        self.with(|c| {
            let tracks = c
                .prepare(
                    "SELECT t.* FROM playlist_tracks p JOIN tracks t USING (source, id)
                     WHERE p.playlist_id = ?1 ORDER BY p.position",
                )?
                .query_map([id], track_from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(Playlist { summary: summary(c, id)?, tracks, link: link(c, id)? })
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
    /// exactly `tracks`, in order. A previously imported playlist keeps its name.
    pub fn import_playlist(
        &self,
        plugin_id: &str,
        remote_id: &str,
        name: &str,
        tracks: &[Track],
    ) -> anyhow::Result<PlaylistSummary> {
        let name = clean_name(name)?;
        self.with(|c| {
            let tx = c.transaction()?;
            let ts = now();
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
                    tx.execute(
                        "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
                        params![name, ts],
                    )?;
                    let id = tx.last_insert_rowid();
                    tx.execute(
                        "INSERT INTO playlist_links (playlist_id, plugin_id, remote_id, synced_at) VALUES (?1, ?2, ?3, ?4)",
                        params![id, plugin_id, remote_id, ts],
                    )?;
                    id
                }
            };
            tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", [id])?;
            let mut position = 0i64;
            for track in tracks {
                upsert_track(&tx, track)?;
                position += tx.execute(
                    "INSERT OR IGNORE INTO playlist_tracks (playlist_id, source, id, position, added_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, track.source, track.id, position, ts],
                )? as i64;
            }
            tx.execute("UPDATE playlist_links SET synced_at = ?2 WHERE playlist_id = ?1", params![id, ts])?;
            touch(&tx, id)?;
            let result = summary(&tx, id)?;
            tx.commit()?;
            Ok(result)
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

    // ---------- Track metadata and matches ----------

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
            c.query_row(
                "SELECT * FROM tracks WHERE source = ?1 AND id = ?2",
                params![source, id],
                track_from_row,
            )
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

    fn playlist_exists(&self, id: i64) -> anyhow::Result<()> {
        let found = self.with(|c| {
            c.query_row("SELECT 1 FROM playlists WHERE id = ?1", [id], |_| Ok(()))
                .optional()
        })?;
        if found.is_none() {
            bail!("la playlist no existe");
        }
        Ok(())
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
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

fn upsert_track(c: &Connection, t: &Track) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO tracks (source, id, title, artist, duration_secs, artwork_url, genre)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (source, id) DO UPDATE SET
           title = excluded.title, artist = excluded.artist,
           -- Some imports (e.g. Spotify's data export) lack durations and covers: keep known ones.
           duration_secs = CASE WHEN excluded.duration_secs > 0 THEN excluded.duration_secs ELSE tracks.duration_secs END,
           artwork_url = COALESCE(excluded.artwork_url, tracks.artwork_url),
           genre = COALESCE(excluded.genre, tracks.genre)",
        params![t.source, t.id, t.title, t.artist, t.duration_secs, t.artwork_url, t.genre],
    )?;
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
    if let Some((from, to)) = mv {
        if from < keys.len() {
            let item = keys.remove(from);
            keys.insert(to.min(keys.len()), item);
        }
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

fn track_from_row(r: &Row) -> rusqlite::Result<Track> {
    Ok(Track {
        source: r.get("source")?,
        id: r.get("id")?,
        title: r.get("title")?,
        artist: r.get("artist")?,
        duration_secs: r.get("duration_secs")?,
        artwork_url: r.get("artwork_url")?,
        genre: r.get("genre")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            source: "audius".into(),
            title: format!("Canción {id}"),
            artist: "Artista".into(),
            duration_secs: 100,
            artwork_url: Some(format!("https://img/{id}.jpg")),
            genre: None,
        }
    }

    fn ids(tracks: &[Track]) -> Vec<&str> {
        tracks.iter().map(|t| t.id.as_str()).collect()
    }

    #[test]
    fn likes_newest_first_and_unlike() {
        let lib = Library::in_memory();
        lib.set_liked(&track("a"), true).unwrap();
        lib.set_liked(&track("b"), true).unwrap();
        lib.set_liked(&track("a"), true).unwrap(); // idempotent
        assert_eq!(ids(&lib.liked_tracks().unwrap()), ["b", "a"]);
        assert_eq!(lib.liked_keys().unwrap().len(), 2);

        lib.set_liked(&track("b"), false).unwrap();
        assert_eq!(ids(&lib.liked_tracks().unwrap()), ["a"]);
        assert_eq!(lib.liked_keys().unwrap(), ["audius/a"]);
    }

    #[test]
    fn liking_updates_stored_metadata() {
        let lib = Library::in_memory();
        lib.set_liked(&track("a"), true).unwrap();
        let mut renamed = track("a");
        renamed.title = "Nuevo título".into();
        lib.set_liked(&renamed, true).unwrap();
        assert_eq!(lib.liked_tracks().unwrap()[0].title, "Nuevo título");
    }

    #[test]
    fn playlist_lifecycle() {
        let lib = Library::in_memory();
        let p = lib.create_playlist("  Para correr  ").unwrap();
        assert_eq!(p.name, "Para correr");
        assert_eq!(p.track_count, 0);
        assert!(lib.create_playlist("   ").is_err());

        assert!(lib.add_to_playlist(p.id, &track("a")).unwrap());
        assert!(lib.add_to_playlist(p.id, &track("b")).unwrap());
        assert!(lib.add_to_playlist(p.id, &track("c")).unwrap());
        assert!(!lib.add_to_playlist(p.id, &track("a")).unwrap(), "sin duplicados");

        let full = lib.playlist(p.id).unwrap();
        assert_eq!(ids(&full.tracks), ["a", "b", "c"]);
        assert_eq!(full.summary.track_count, 3);
        assert_eq!(full.summary.duration_secs, 300);
        assert_eq!(full.summary.artwork_urls.len(), 3);

        lib.move_in_playlist(p.id, 2, 0).unwrap();
        assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["c", "a", "b"]);
        lib.move_in_playlist(p.id, 0, 2).unwrap();
        assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["a", "b", "c"]);

        lib.remove_from_playlist(p.id, "audius", "b").unwrap();
        assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["a", "c"]);
        assert!(lib.add_to_playlist(p.id, &track("d")).unwrap());
        assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["a", "c", "d"]);

        lib.rename_playlist(p.id, "Domingo").unwrap();
        assert_eq!(lib.playlists().unwrap()[0].name, "Domingo");

        lib.delete_playlist(p.id).unwrap();
        assert!(lib.playlists().unwrap().is_empty());
        assert!(lib.playlist(p.id).is_err());
        assert!(lib.add_to_playlist(p.id, &track("a")).is_err());
        assert!(lib.delete_playlist(p.id).is_err());
    }

    #[test]
    fn same_id_from_different_sources_are_different_tracks() {
        let lib = Library::in_memory();
        let mut yt = track("a");
        yt.source = "youtube".into();
        lib.set_liked(&track("a"), true).unwrap();
        lib.set_liked(&yt, true).unwrap();
        assert_eq!(lib.liked_tracks().unwrap().len(), 2);
    }

    #[test]
    fn import_creates_then_resyncs_the_same_playlist() {
        let lib = Library::in_memory();
        let sp = |id: &str| Track { source: "spotify".into(), ..track(id) };

        let first = lib.import_playlist("spotify", "playlist:abc", "Verano", &[sp("1"), sp("2"), sp("1")]).unwrap();
        assert_eq!(first.name, "Verano");
        assert_eq!(first.track_count, 2, "duplicados fuera");
        let full = lib.playlist(first.id).unwrap();
        assert_eq!(ids(&full.tracks), ["1", "2"]);
        assert_eq!(full.link.as_ref().unwrap().remote_id, "playlist:abc");

        // The user renames it; a re-sync keeps the name and replaces the tracks.
        lib.rename_playlist(first.id, "Mi verano").unwrap();
        let again = lib.import_playlist("spotify", "playlist:abc", "Verano", &[sp("3"), sp("2")]).unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(again.name, "Mi verano");
        assert_eq!(ids(&lib.playlist(first.id).unwrap().tracks), ["3", "2"]);

        assert_eq!(lib.imported_from("spotify").unwrap()["playlist:abc"], first.id);
        assert!(lib.playlist(first.id).unwrap().link.is_some());

        // Deleting the playlist drops the link too.
        lib.delete_playlist(first.id).unwrap();
        assert!(lib.imported_from("spotify").unwrap().is_empty());
    }

    #[test]
    fn imports_without_duration_keep_known_metadata() {
        let lib = Library::in_memory();
        let full = Track { source: "spotify".into(), ..track("x") };
        lib.import_playlist("spotify", "liked", "Me gusta de Spotify", &[full.clone()]).unwrap();
        let bare = Track { duration_secs: 0, artwork_url: None, ..full.clone() };
        lib.import_playlist("spotify-export", "liked", "Me gusta de Spotify", &[bare]).unwrap();
        let stored = lib.track("spotify", "x").unwrap().unwrap();
        assert_eq!(stored.duration_secs, full.duration_secs);
        assert_eq!(stored.artwork_url, full.artwork_url);
    }

    #[test]
    fn remembers_track_matches() {
        let lib = Library::in_memory();
        let sp = Track { source: "spotify".into(), ..track("x") };
        lib.import_playlist("spotify", "liked", "Me gusta de Spotify", &[sp]).unwrap();
        assert_eq!(lib.track("spotify", "x").unwrap().unwrap().title, "Canción x");
        assert!(lib.track_match("spotify", "x").unwrap().is_none());
        lib.set_track_match("spotify", "x", "youtube", "abc").unwrap();
        lib.set_track_match("spotify", "x", "youtube", "def").unwrap();
        assert_eq!(lib.track_match("spotify", "x").unwrap(), Some(("youtube".into(), "def".into())));
    }

    #[test]
    fn migrates_a_v1_database() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_V1).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO playlists (name, created_at, updated_at) VALUES ('Vieja', 0, 0)", []).unwrap();
        let lib = Library::init(conn).unwrap();
        assert_eq!(lib.playlists().unwrap()[0].name, "Vieja");
        assert!(lib.imported_from("spotify").unwrap().is_empty());
    }

    #[test]
    fn reopening_keeps_data() {
        let path = std::env::temp_dir().join(format!("melomango-lib-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let lib = Library::open(&path).unwrap();
            lib.set_liked(&track("a"), true).unwrap();
            lib.create_playlist("Guardada").unwrap();
        }
        let lib = Library::open(&path).unwrap();
        assert_eq!(lib.liked_tracks().unwrap().len(), 1);
        assert_eq!(lib.playlists().unwrap()[0].name, "Guardada");
        drop(lib);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
}
