//! The user's library: liked tracks, playlists and listening history, stored in SQLite
//! (`library.db` in the app data folder).
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

const SCHEMA_VERSION: i32 = 3;
/// Older plays are dropped beyond this many.
const MAX_PLAYS: i64 = 20_000;

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

/// v3: listening history, one row per play.
const SCHEMA_V3: &str = "
CREATE TABLE plays (
    play_id   INTEGER PRIMARY KEY AUTOINCREMENT,
    source    TEXT NOT NULL,
    id        TEXT NOT NULL,
    played_at INTEGER NOT NULL,
    FOREIGN KEY (source, id) REFERENCES tracks (source, id)
);
CREATE INDEX plays_recent ON plays (played_at);
";

/// Tables that older versions simply ignore are created without bumping the version, so
/// going back to an older Melomango still opens the library.
const ADDITIVE: &str = "
CREATE TABLE IF NOT EXISTS downloads (
    source        TEXT NOT NULL,
    id            TEXT NOT NULL,
    file          TEXT NOT NULL,
    mime          TEXT NOT NULL,
    size          INTEGER NOT NULL,
    -- 'download': kept until the user removes it; 'cache': saved on its own, may be dropped.
    kind          TEXT NOT NULL DEFAULT 'download',
    downloaded_at INTEGER NOT NULL,
    PRIMARY KEY (source, id),
    FOREIGN KEY (source, id) REFERENCES tracks (source, id)
);
-- A song's plays: is it a favourite, when was it last played (cache eviction).
CREATE INDEX IF NOT EXISTS plays_track ON plays (source, id, played_at);
";

/// The library was written by a newer Melomango than this one (it can't be read safely).
#[derive(Debug)]
pub struct NewerLibrary {
    pub found: i32,
}

impl std::fmt::Display for NewerLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "la biblioteca es de una versión más nueva de Melomango (formato {})", self.found)
    }
}

impl std::error::Error for NewerLibrary {}

/// A song saved to listen without a connection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    pub track: Track,
    pub size: u64,
    pub downloaded_at: i64,
}

/// Why a song's audio is on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredKind {
    /// The user downloaded it: kept until they remove it.
    Download,
    /// Saved on its own because it is played often: dropped when the cache is full.
    Cache,
}

impl StoredKind {
    fn as_str(self) -> &'static str {
        match self {
            StoredKind::Download => "download",
            StoredKind::Cache => "cache",
        }
    }

    fn parse(s: &str) -> Self {
        if s == "cache" {
            StoredKind::Cache
        } else {
            StoredKind::Download
        }
    }
}

/// A cached song, as the cache sees it when making room.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedSong {
    pub source: String,
    pub id: String,
    pub file: String,
    pub size: u64,
}

/// Where a downloaded song's audio is (file name inside the downloads folder).
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadFile {
    pub file: String,
    pub mime: String,
    pub size: u64,
}

/// A track the user listened to, and when (milliseconds since the epoch).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Play {
    pub track: Track,
    pub played_at: i64,
}

/// What the user listened to in one month (for "Tu resumen del mes").
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthSummary {
    /// "YYYY-MM", in local time.
    pub month: String,
    pub plays: u32,
    /// Approximate: the length of every song played (unknown lengths count as 0).
    pub minutes: u32,
    pub songs: u32,
    pub artists: u32,
    pub top_tracks: Vec<TrackCount>,
    pub top_artists: Vec<ArtistCount>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackCount {
    pub track: Track,
    pub plays: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistCount {
    pub name: String,
    pub plays: u32,
    /// Cover of their most played song, to show something.
    pub artwork_url: Option<String>,
}

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
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    #[serde(flatten)]
    pub summary: PlaylistSummary,
    pub tracks: Vec<Track>,
    /// When each track (same order) was added, in milliseconds since the epoch.
    pub added_at: Vec<i64>,
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

    fn init(mut conn: Connection) -> anyhow::Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // With WAL this is still safe against corruption; a power cut can only lose the
        // last moments. FULL made every small write (a like, a play) wait for the disk.
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(NewerLibrary { found: version }.into());
        }
        // All or nothing: closing the app halfway through must not leave a library that
        // fails the next time ("table already exists").
        let tx = conn.transaction()?;
        if version < 1 {
            tx.execute_batch(SCHEMA_V1)?;
        }
        if version < 2 {
            tx.execute_batch(SCHEMA_V2)?;
        }
        if version < 3 {
            tx.execute_batch(SCHEMA_V3)?;
        }
        tx.execute_batch(ADDITIVE)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;
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
            c.query_row(
                "SELECT kind FROM downloads WHERE source = ?1 AND id = ?2",
                params![source, id],
                |r| r.get::<_, String>(0),
            )
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
            c.query_row(
                "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM downloads WHERE kind = 'cache'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
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

    // ---------- History ----------

    /// Records that `track` was listened to now.
    pub fn record_play(&self, track: &Track) -> anyhow::Result<()> {
        self.record_play_at(track, now())
    }

    fn record_play_at(&self, track: &Track, played_at: i64) -> anyhow::Result<()> {
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
            let rows = stmt.query_map([limit], |r| {
                Ok(Play { track: track_from_row(r)?, played_at: r.get("played_at")? })
            })?;
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
        songs.sort_by(|a, b| b.1.cmp(&a.1));
        artists.sort_by(|a, b| b.1.cmp(&a.1));
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
    // Songs already there keep the date they were added; only new ones get today's.
    let added_before: std::collections::HashMap<(String, String), i64> = tx
        .prepare("SELECT source, id, added_at FROM playlist_tracks WHERE playlist_id = ?1")?
        .query_map([id], |r| Ok(((r.get(0)?, r.get(1)?), r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", [id])?;
    let mut position = 0i64;
    for track in tracks {
        upsert_track(tx, track)?;
        let added_at = added_before
            .get(&(track.source.clone(), track.id.clone()))
            .copied()
            .unwrap_or(ts);
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

fn upsert_track(c: &Connection, t: &Track) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO tracks (source, id, title, artist, duration_secs, artwork_url, genre)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (source, id) DO UPDATE SET
           title = excluded.title, artist = excluded.artist,
           -- Some imports (e.g. Spotify's data export) lack durations and covers: keep known ones.
           duration_secs = CASE WHEN excluded.duration_secs > 0 THEN excluded.duration_secs ELSE tracks.duration_secs END,
           artwork_url = COALESCE(excluded.artwork_url, tracks.artwork_url),
           genre = COALESCE(excluded.genre, tracks.genre)
         -- Nothing new (the same suggestion again, on every key typed): no write at all.
         WHERE tracks.title IS NOT excluded.title
            OR tracks.artist IS NOT excluded.artist
            OR (excluded.duration_secs > 0 AND tracks.duration_secs IS NOT excluded.duration_secs)
            OR (excluded.artwork_url IS NOT NULL AND tracks.artwork_url IS NOT excluded.artwork_url)
            OR (excluded.genre IS NOT NULL AND tracks.genre IS NOT excluded.genre)",
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

/// "Shakira, Burna Boy" / "A & B" / "A feat. B" -> each artist (empty names left out).
fn split_artists(artist: &str) -> Vec<&str> {
    // ASCII-only lowercasing keeps every byte where it was, so positions in `lower` are valid
    // in `artist` too. `to_lowercase` can make a letter longer («İ» -> «i̇») and then slicing
    // cuts a character in half. The separators are ASCII, so nothing else is needed.
    let lower = artist.to_ascii_lowercase();
    let mut parts = Vec::new();
    let mut start = 0;
    let separators = [", ", ",", " & ", " feat. ", " ft. ", " featuring "];
    let mut i = 0;
    while i < artist.len() {
        if let Some(sep) = separators.iter().find(|s| lower[i..].starts_with(*s)) {
            parts.push(artist[start..i].trim());
            i += sep.len();
            start = i;
        } else {
            i += artist[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    parts.push(artist[start..].trim());
    parts.into_iter().filter(|p| !p.is_empty()).collect()
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
    fn a_songs_plays_are_found_through_an_index() {
        let lib = Library::in_memory();
        let plan: Vec<String> = lib
            .with(|c| {
                c.prepare("EXPLAIN QUERY PLAN SELECT COUNT(*) FROM plays WHERE source = 'a' AND id = 'b'")?
                    .query_map([], |r| r.get::<_, String>(3))?
                    .collect()
            })
            .unwrap();
        assert!(plan.iter().any(|step| step.contains("plays_track")), "{plan:?}");
    }

    #[test]
    fn opens_with_wal_and_normal_sync() {
        let path = std::env::temp_dir().join(format!("melomango-sync-{}.db", std::process::id()));
        let lib = Library::open(&path).unwrap();
        let (journal, sync): (String, i64) = lib
            .with(|c| Ok((c.pragma_query_value(None, "journal_mode", |r| r.get(0))?, c.pragma_query_value(None, "synchronous", |r| r.get(0))?)))
            .unwrap();
        assert_eq!((journal.as_str(), sync), ("wal", 1), "1 = NORMAL");
        drop(lib);
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
    }

    #[test]
    fn remembering_the_same_tracks_again_writes_nothing() {
        let lib = Library::in_memory();
        let tracks = [track("a"), track("b")];
        lib.remember_tracks(&tracks).unwrap();
        let changes = || lib.with(|c| Ok(c.total_changes())).unwrap();
        let before = changes();
        lib.remember_tracks(&tracks).unwrap();
        assert_eq!(changes(), before, "unchanged: no row written");
        let with_cover = Track { artwork_url: Some("https://x/new.jpg".into()), ..track("a") };
        lib.remember_tracks(&[with_cover]).unwrap();
        assert_eq!(changes(), before + 1, "a new cover is saved");
        // Missing data never overwrites what is known.
        lib.remember_tracks(&[Track { artwork_url: None, duration_secs: 0, ..track("a") }]).unwrap();
        assert_eq!(changes(), before + 1);
        assert_eq!(lib.track("audius", "a").unwrap().unwrap().artwork_url.as_deref(), Some("https://x/new.jpg"));
    }

    #[test]
    fn a_busy_month_is_summarised_quickly() {
        let lib = Library::in_memory();
        let base = 1_780_000_000_000; // a moment in 2026
        // 20.000 plays of 5.000 songs by 2.000 artists: the old linear lookups took seconds.
        lib.with(|c| {
            let tx = c.transaction()?;
            for i in 0..20_000i64 {
                let t = Track { artist: format!("Artista {}", i % 2_000), ..track(&format!("s{}", i % 5_000)) };
                upsert_track(&tx, &t)?;
                tx.execute("INSERT INTO plays (source, id, played_at) VALUES (?1, ?2, ?3)", params![t.source, t.id, base + i * 1000])?;
            }
            tx.commit()
        })
        .unwrap();
        let month = lib.summary_months().unwrap()[0].clone();
        let started = std::time::Instant::now();
        let summary = lib.month_summary(&month).unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
        assert_eq!(summary.plays as usize, 20_000 - 0);
        assert_eq!((summary.songs, summary.artists), (5_000, 2_000));
        assert_eq!(summary.top_tracks[0].plays, 4);
    }

    #[test]
    fn importing_several_playlists_is_all_or_nothing() {
        let lib = Library::in_memory();
        let sp = |id: &str| Track { source: "spotify".into(), ..track(id) };
        // The second playlist fails to save (as a full disk or a broken row would).
        lib.with(|c| {
            c.execute_batch(
                "CREATE TRIGGER broken BEFORE INSERT ON playlists WHEN NEW.name = 'Rota'
                 BEGIN SELECT RAISE(ABORT, 'disco lleno'); END;",
            )
        })
        .unwrap();
        let (a, b) = ([sp("1")], [sp("2")]);
        let result = lib.import_many("spotify-export", &[("playlist:A", "A", &a[..]), ("playlist:Rota", "Rota", &b[..])]);
        assert!(result.is_err());
        assert!(lib.playlists().unwrap().is_empty(), "the first one is not left half imported");

        let ok = lib.import_many("spotify-export", &[("playlist:A", "A", &a[..]), ("playlist:B", "  ", &b[..])]).unwrap();
        let names: Vec<_> = ok.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["A", "Playlist importada"], "a playlist without a name gets one");
    }

    #[test]
    fn resyncing_keeps_when_each_song_was_added() {
        let lib = Library::in_memory();
        let sp = |id: &str| Track { source: "spotify".into(), ..track(id) };
        let first = lib.import_playlist("spotify", "playlist:abc", "Verano", &[sp("1"), sp("2")]).unwrap();
        // As if it had been imported a while ago.
        let long_ago = 1_000;
        lib.with(|c| c.execute("UPDATE playlist_tracks SET added_at = ?1", [long_ago])).unwrap();

        lib.import_playlist("spotify", "playlist:abc", "Verano", &[sp("3"), sp("2"), sp("1")]).unwrap();
        let synced = lib.playlist(first.id).unwrap();
        assert_eq!(ids(&synced.tracks), ["3", "2", "1"]);
        assert_eq!(&synced.added_at[1..], [long_ago, long_ago], "songs that were already there");
        assert!(synced.added_at[0] > long_ago, "the new song is added now");
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
    fn creates_a_playlist_with_tracks() {
        let lib = Library::in_memory();
        let p = lib.create_playlist_with("Pegada", &[track("a"), track("b"), track("a")]).unwrap();
        assert_eq!((p.name.as_str(), p.track_count), ("Pegada", 2));
        assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["a", "b"]);
        assert!(lib.playlist(p.id).unwrap().link.is_none(), "not linked: nothing to sync");
        let full = lib.playlist(p.id).unwrap();
        assert_eq!(full.added_at.len(), full.tracks.len());
        assert!(full.added_at.iter().all(|t| *t > 0));
        assert!(lib.create_playlist_with("  ", &[]).is_err());
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
    fn records_and_lists_plays() {
        let lib = Library::in_memory();
        lib.record_play_at(&track("a"), 1_000).unwrap();
        lib.record_play_at(&track("b"), 2_000).unwrap();
        lib.record_play_at(&track("a"), 3_000).unwrap();

        let plays = lib.recent_plays(10).unwrap();
        let order: Vec<(&str, i64)> = plays.iter().map(|p| (p.track.id.as_str(), p.played_at)).collect();
        assert_eq!(order, [("a", 3_000), ("b", 2_000), ("a", 1_000)]);
        assert_eq!(plays[0].track.title, "Canción a");
        assert_eq!(lib.recent_plays(1).unwrap().len(), 1);

        lib.clear_history().unwrap();
        assert!(lib.recent_plays(10).unwrap().is_empty());
        // The tracks themselves (and likes, playlists…) stay.
        assert!(lib.track("audius", "a").unwrap().is_some());
    }

    #[test]
    fn summarises_a_month() {
        let lib = Library::in_memory();
        let mut duet = track("d");
        duet.artist = "Shakira, Burna Boy".into();
        let mut shakira = track("s");
        shakira.artist = "Shakira".into();
        // 2026-11-10 12:00 local-ish (UTC noon keeps the same day in most zones).
        let nov = 1_794_312_000_000i64;
        let oct = nov - 31 * 86_400_000;
        for (t, when) in [(&shakira, nov), (&duet, nov + 1), (&shakira, nov + 2), (&track("a"), nov + 3), (&track("a"), oct)] {
            lib.record_play_at(t, when).unwrap();
        }
        let months = lib.summary_months().unwrap();
        assert_eq!(months.len(), 2);
        assert!(months[0] > months[1], "newest first");

        let s = lib.month_summary(&months[0]).unwrap();
        assert_eq!((s.plays, s.songs), (4, 3));
        assert_eq!(s.minutes, 4 * 100 / 60, "track() lasts 100 s");
        assert_eq!(s.top_tracks[0].track.id, "s");
        assert_eq!(s.top_tracks[0].plays, 2);
        assert_eq!(s.top_artists[0].name, "Shakira");
        assert_eq!(s.top_artists[0].plays, 3, "the duet counts for both artists");
        assert!(s.top_artists.iter().any(|a| a.name == "Burna Boy"));
        assert_eq!(s.artists, 3);

        let empty = lib.month_summary("1999-01").unwrap();
        assert_eq!((empty.plays, empty.top_tracks.len()), (0, 0));
    }

    #[test]
    fn splits_artist_lines() {
        assert_eq!(split_artists("Shakira, Burna Boy"), ["Shakira", "Burna Boy"]);
        assert_eq!(split_artists("Daft Punk feat. Pharrell Williams"), ["Daft Punk", "Pharrell Williams"]);
        assert_eq!(split_artists("Bad Bunny & Drake"), ["Bad Bunny", "Drake"]);
        assert_eq!(split_artists("ROSALÍA"), ["ROSALÍA"]);
        assert!(split_artists("").is_empty());
        // Letters that grow when lowercased used to panic (and close the app).
        assert_eq!(split_artists("İbrahim Tatlıses & X"), ["İbrahim Tatlıses", "X"]);
        assert_eq!(split_artists("Ⱥ, B"), ["Ⱥ", "B"]);
        assert_eq!(split_artists("ẞ, Bob"), ["ẞ", "Bob"]);
        assert_eq!(split_artists("İİİ FEAT. Bob"), ["İİİ", "Bob"]);
    }

    #[test]
    fn keeps_a_bounded_history() {
        let lib = Library::in_memory();
        lib.with(|c| {
            upsert_track(c, &track("a"))?;
            // Pretend MAX_PLAYS plays already happened.
            c.execute("INSERT INTO plays (play_id, source, id, played_at) VALUES (?1, 'audius', 'a', 0)", [1])?;
            c.execute("UPDATE sqlite_sequence SET seq = ?1 WHERE name = 'plays'", [MAX_PLAYS])
        })
        .unwrap();
        lib.record_play_at(&track("b"), 5).unwrap();
        let plays = lib.recent_plays(10).unwrap();
        assert_eq!(plays.len(), 1, "the oldest play is dropped");
        assert_eq!(plays[0].track.id, "b");
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

    #[test]
    fn downloads_are_listed_found_and_removed() {
        let lib = Library::in_memory();
        let file = |name: &str| DownloadFile { file: name.into(), mime: "audio/webm".into(), size: 1234 };
        lib.add_download(&track("a"), &file("a.webm"), StoredKind::Download).unwrap();
        lib.add_download(&track("b"), &file("b.webm"), StoredKind::Download).unwrap();
        let listed = lib.downloads().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].size, 1234);
        let mut keys = lib.downloaded_keys().unwrap();
        keys.sort();
        assert_eq!(keys, ["audius/a", "audius/b"]);
        assert_eq!(lib.download_file("audius", "a").unwrap(), Some(file("a.webm")));
        assert_eq!(lib.download_file("audius", "zzz").unwrap(), None);
        assert_eq!(lib.remove_download("audius", "a").unwrap().as_deref(), Some("a.webm"));
        assert_eq!(lib.remove_download("audius", "a").unwrap(), None);
        assert_eq!(ids(&lib.downloads().unwrap().into_iter().map(|d| d.track).collect::<Vec<_>>()), ["b"]);
        // Downloading again replaces the entry.
        lib.add_download(&track("b"), &file("b2.webm"), StoredKind::Download).unwrap();
        assert_eq!(lib.download_file("audius", "b").unwrap().unwrap().file, "b2.webm");
    }

    #[test]
    fn the_cache_is_apart_from_downloads_and_drops_the_least_played_first() {
        let lib = Library::in_memory();
        let file = |name: &str, size| DownloadFile { file: name.into(), mime: "audio/webm".into(), size };
        lib.add_download(&track("old"), &file("old", 100), StoredKind::Cache).unwrap();
        lib.add_download(&track("new"), &file("new", 200), StoredKind::Cache).unwrap();
        lib.add_download(&track("mine"), &file("mine", 999), StoredKind::Download).unwrap();
        assert_eq!(lib.cache_usage().unwrap(), (2, 300));
        assert!(lib.downloaded_keys().unwrap() == ["audius/mine"], "cached songs aren't downloads");
        assert_eq!(lib.downloads().unwrap().len(), 1);
        // Playing "old" now makes "new" the least recently used.
        lib.record_play_at(&track("old"), now() + 60_000).unwrap();
        let order: Vec<String> = lib.cache_eviction_order().unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(order, ["new", "old"]);
        // Downloading a cached song keeps it for good.
        assert_eq!(lib.stored_kind("audius", "old").unwrap(), Some(StoredKind::Cache));
        lib.keep_cached("audius", "old").unwrap();
        assert_eq!(lib.stored_kind("audius", "old").unwrap(), Some(StoredKind::Download));
        assert_eq!(lib.cache_usage().unwrap(), (1, 200));
    }

    #[test]
    fn favourites_are_played_twice_or_liked() {
        let lib = Library::in_memory();
        lib.record_play(&track("a")).unwrap();
        assert!(!lib.is_favourite("audius", "a").unwrap());
        lib.record_play(&track("a")).unwrap();
        assert!(lib.is_favourite("audius", "a").unwrap());
        lib.set_liked(&track("b"), true).unwrap();
        assert!(lib.is_favourite("audius", "b").unwrap());
        assert!(!lib.is_favourite("audius", "zzz").unwrap());
    }

    /// A library left by an older Melomango (e.g. 0.5.0, format 2) opens and keeps its data.
    #[test]
    fn upgrades_a_library_from_an_older_version() {
        for old in [1, 2] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch(SCHEMA_V1).unwrap();
            if old >= 2 {
                conn.execute_batch(SCHEMA_V2).unwrap();
            }
            conn.pragma_update(None, "user_version", old).unwrap();
            conn.execute(
                "INSERT INTO tracks (source, id, title, artist, duration_secs) VALUES ('audius', 'a', 'Vieja', 'X', 100)",
                [],
            )
            .unwrap();
            conn.execute("INSERT INTO likes (source, id, liked_at) VALUES ('audius', 'a', 1)", []).unwrap();

            let lib = Library::init(conn).unwrap();
            assert_eq!(ids(&lib.liked_tracks().unwrap()), ["a"], "from format {old}");
            lib.record_play(&track("a")).unwrap();
            assert_eq!(lib.recent_plays(10).unwrap().len(), 1);
            assert!(lib.downloads().unwrap().is_empty());
            let version: i32 = lib.with(|c| c.pragma_query_value(None, "user_version", |r| r.get(0))).unwrap();
            assert_eq!(version, SCHEMA_VERSION);
        }
    }

    /// A library from a newer Melomango is refused with an error the app recognises.
    #[test]
    fn refuses_a_library_from_a_newer_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
        let error = Library::init(conn).err().expect("debería negarse");
        assert_eq!(error.downcast_ref::<NewerLibrary>().map(|e| e.found), Some(SCHEMA_VERSION + 1));
    }

    /// A failed upgrade leaves the library as it was, so the next start can retry.
    #[test]
    fn a_failed_upgrade_changes_nothing() {
        let path = std::env::temp_dir().join(format!("melomango-upgrade-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(SCHEMA_V1).unwrap();
            conn.execute_batch(SCHEMA_V2).unwrap();
            // In the way of the format-3 step, after its first statement ("plays") worked:
            // its index can't take a name already used.
            conn.execute_batch("CREATE TABLE plays_recent (x INTEGER);").unwrap();
            conn.pragma_update(None, "user_version", 2).unwrap();
        }
        assert!(Library::open(&path).is_err());
        let conn = Connection::open(&path).unwrap();
        let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, 2, "still the old format");
        let half_done: i32 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name IN ('plays', 'downloads')", [], |r| r.get(0))
            .unwrap();
        assert_eq!(half_done, 0, "nothing half-done");
        drop(conn);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
}
