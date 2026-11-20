//! The user's library: liked tracks, playlists and listening history, stored in SQLite
//! (`library.db` in the app data folder).
//!
//! Track metadata is copied into the database when a track is liked or added to a
//! playlist, so the library still shows it even if the source is unavailable.

use crate::text::split_artists;
use std::{
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use rusqlite::{Connection, OptionalExtension, Row, params};
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
-- Small settings of the library itself (for now, how many ADDITIVE_STEPS were applied).
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value INTEGER NOT NULL
);
";

/// Additive changes that can't be written as `IF NOT EXISTS` (such as `ALTER TABLE ... ADD
/// COLUMN`), applied once each, in order. They have their own count (in `meta`), apart from
/// `user_version`, so an older Melomango still opens the library: a new column is one it
/// doesn't read. Append new steps at the end; never change or remove one.
const ADDITIVE_STEPS: &[&str] = &[];

/// Runs the `steps` not applied yet to this library.
fn apply_additive_steps(c: &Connection, steps: &[&str]) -> rusqlite::Result<()> {
    let done: i64 =
        c.query_row("SELECT value FROM meta WHERE key = 'additive_steps'", [], |r| r.get(0)).optional()?.unwrap_or(0);
    for step in steps.iter().skip(done.max(0) as usize) {
        c.execute_batch(step)?;
    }
    c.execute(
        "INSERT INTO meta (key, value) VALUES ('additive_steps', ?1)
         ON CONFLICT (key) DO UPDATE SET value = MAX(value, excluded.value)",
        [steps.len() as i64],
    )?;
    Ok(())
}

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
#[derive(Debug, Clone, Serialize, specta::Type)]
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
        if s == "cache" { StoredKind::Cache } else { StoredKind::Download }
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
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Play {
    pub track: Track,
    pub played_at: i64,
}

/// What the user listened to in one month (for "Tu resumen del mes").
#[derive(Debug, Clone, Serialize, specta::Type)]
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

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackCount {
    pub track: Track,
    pub plays: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistCount {
    pub name: String,
    pub plays: u32,
    /// Cover of their most played song, to show something.
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub track_count: u32,
    pub duration_secs: u32,
    /// Covers of the first tracks, for a mosaic.
    pub artwork_urls: Vec<String>,
}

#[derive(Debug, Serialize, specta::Type)]
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

#[derive(Debug, Clone, Serialize, PartialEq, specta::Type)]
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
        let conn =
            Connection::open(path).with_context(|| format!("no se pudo abrir la biblioteca {}", path.display()))?;
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
        apply_additive_steps(&tx, ADDITIVE_STEPS)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> anyhow::Result<T> {
        let mut conn = self.conn.lock().unwrap();
        Ok(f(&mut conn)?)
    }
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
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

mod history;
mod likes;
mod playlists;
mod storage;
mod tracks;

#[cfg(test)]
mod tests;
