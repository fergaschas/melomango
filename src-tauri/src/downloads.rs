//! Downloads: a song's audio saved in the app data folder (`downloads/`) to listen
//! without a connection. The `stream://` protocol serves a downloaded song from disk
//! (see [`Downloads::local`]) before trying the network.
//!
//! The same folder holds the automatic cache: songs played often (or liked) are saved
//! on their own, up to a size limit; the least recently played go first.

use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use anyhow::{anyhow, bail, Context};
use serde::Serialize;
use tokio::sync::Semaphore;

use crate::{
    library::{DownloadFile, Library, StoredKind},
    models::Track,
    sources::Registry,
    stream,
};

/// Songs downloading at once.
const PARALLEL: usize = 2;
/// Bytes asked per request (servers like YouTube throttle bigger ones).
const PIECE: u64 = 2 * 1024 * 1024;
/// How often a download waits for the same song already on its way.
const WAIT_STEP: std::time::Duration = std::time::Duration::from_millis(250);

/// What the UI hears about a download.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// "source/id"
    pub key: String,
    pub state: State,
    pub received: u64,
    pub total: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Queued,
    Downloading,
    Done,
    Failed,
    Cancelled,
}

pub struct Downloads {
    dir: PathBuf,
    /// Downloads queued or running, with the flag that cancels them.
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
    slots: Semaphore,
    /// Bytes the automatic cache may use (0 = off). Set by the app on startup.
    cache_limit: AtomicU64,
}

impl Downloads {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, active: Mutex::default(), slots: Semaphore::new(PARALLEL), cache_limit: AtomicU64::new(0) }
    }

    /// Changes the cache's size (0 turns it off and empties it) and makes room if needed.
    pub fn set_cache_limit(&self, library: &Library, bytes: u64) -> anyhow::Result<()> {
        self.cache_limit.store(bytes, Ordering::Relaxed);
        self.make_room(library)
    }

    /// Deletes every cached song (downloads stay). One that can't be deleted (a file in
    /// use) doesn't stop the rest; the first error is reported at the end.
    pub fn clear_cache(&self, library: &Library) -> anyhow::Result<()> {
        let mut first_error = None;
        for song in library.cache_eviction_order()? {
            if let Err(e) = self.delete_stored(library, &song.source, &song.id) {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Drops the least recently played cached songs until the cache fits its limit
    /// (skipping any that can't be deleted right now).
    fn make_room(&self, library: &Library) -> anyhow::Result<()> {
        let limit = self.cache_limit.load(Ordering::Relaxed);
        let (_, mut used) = library.cache_usage()?;
        let mut first_error = None;
        for song in library.cache_eviction_order()? {
            if used <= limit {
                break;
            }
            match self.delete_stored(library, &song.source, &song.id) {
                Ok(()) => used = used.saturating_sub(song.size),
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Deletes what no row points to: unfinished `.part` files (the app closed mid-download)
    /// and files whose row is gone. Run at startup, before any download starts.
    pub fn clean_orphans(&self, library: &Library) {
        let Ok(known) = library.stored_files() else {
            return; // without the list, deleting anything could delete a good song
        };
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_file() && !known.contains(&name) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    /// Saves a song to the cache if it is played often and not on disk yet (after a play).
    pub async fn cache_if_favourite(
        &self,
        registry: &Registry,
        library: &Library,
        http: &reqwest::Client,
        track: &Track,
    ) {
        if self.cache_limit.load(Ordering::Relaxed) == 0
            || self.local(library, &track.source, &track.id).is_some()
            || !library.is_favourite(&track.source, &track.id).unwrap_or(false)
        {
            return;
        }
        self.download(registry, library, http, track, StoredKind::Cache, |_| {}).await;
    }

    /// The downloaded file of a song, if it is downloaded (and the file is still there).
    pub fn local(&self, library: &Library, source: &str, id: &str) -> Option<(PathBuf, DownloadFile)> {
        let file = library.download_file(source, id).ok()??;
        let path = self.dir.join(&file.file);
        if path.is_file() {
            Some((path, file))
        } else {
            // Deleted by hand: forget it.
            let _ = library.remove_download(source, id);
            None
        }
    }

    /// Downloads `track` (for the user, or into the cache), reporting progress. A song
    /// already on disk isn't fetched again: a cached one the user downloads is just kept.
    pub async fn download(
        &self,
        registry: &Registry,
        library: &Library,
        http: &reqwest::Client,
        track: &Track,
        kind: StoredKind,
        report: impl Fn(Progress),
    ) {
        let key = format!("{}/{}", track.source, track.id);
        let progress = |state, received, total, message: Option<String>| Progress {
            key: key.clone(),
            state,
            received,
            total,
            message,
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut announced = false;
        // The same song may be on its way already (e.g. into the cache): wait for it.
        loop {
            if let Some((_, file)) = self.local(library, &track.source, &track.id) {
                if kind == StoredKind::Download && library.stored_kind(&track.source, &track.id).ok().flatten() == Some(StoredKind::Cache) {
                    if let Err(e) = library.keep_cached(&track.source, &track.id) {
                        report(progress(State::Failed, 0, None, Some(format!("{e:#}"))));
                        return;
                    }
                }
                report(progress(State::Done, file.size, Some(file.size), None));
                return;
            }
            {
                let mut active = self.active.lock().unwrap();
                if !active.contains_key(&key) {
                    active.insert(key.clone(), cancelled.clone());
                    break;
                }
            }
            if kind == StoredKind::Cache {
                return; // someone else is on it
            }
            if !announced {
                report(progress(State::Queued, 0, None, None));
                announced = true;
            }
            tokio::time::sleep(WAIT_STEP).await;
        }
        if !announced {
            report(progress(State::Queued, 0, None, None));
        }

        let result = async {
            let _slot = self.slots.acquire().await?;
            if cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }
            report(progress(State::Downloading, 0, None, None));
            self.fetch_to_disk(registry, library, http, track, &cancelled, |received, total| {
                report(progress(State::Downloading, received, total, None))
            })
            .await
        }
        .await;

        let outcome = match result {
            // Cancelled while the last piece arrived: the file is not kept.
            Ok(Some(file)) if cancelled.load(Ordering::Relaxed) => {
                let _ = fs::remove_file(self.dir.join(&file.file));
                progress(State::Cancelled, 0, None, None)
            }
            Ok(Some(file)) => match library.add_download(track, &file, kind) {
                // Cancelled just before it was recorded: `remove` found nothing to delete then.
                Ok(()) if cancelled.load(Ordering::Relaxed) => {
                    let _ = self.delete_stored(library, &track.source, &track.id);
                    progress(State::Cancelled, 0, None, None)
                }
                Ok(()) => {
                    if kind == StoredKind::Cache {
                        let _ = self.make_room(library);
                    }
                    progress(State::Done, file.size, Some(file.size), None)
                }
                Err(e) => {
                    let _ = fs::remove_file(self.dir.join(&file.file));
                    progress(State::Failed, 0, None, Some(format!("{e:#}")))
                }
            },
            Ok(None) => progress(State::Cancelled, 0, None, None),
            Err(e) => progress(State::Failed, 0, None, Some(format!("{e:#}"))),
        };
        // Only now: until the song is recorded, someone asking for it must see it on its way
        // (or it would start a second download into the same file).
        self.active.lock().unwrap().remove(&key);
        report(outcome);
    }

    /// Cancels a download in progress, or deletes a finished one.
    pub fn remove(&self, library: &Library, source: &str, id: &str) -> anyhow::Result<()> {
        if let Some(flag) = self.active.lock().unwrap().get(&format!("{source}/{id}")) {
            flag.store(true, Ordering::Relaxed);
        }
        self.delete_stored(library, source, id)
    }

    /// The file first, then the row: if the file can't be deleted, the row still points to it
    /// (and it can be tried again) instead of leaving a file nothing knows about.
    fn delete_stored(&self, library: &Library, source: &str, id: &str) -> anyhow::Result<()> {
        if let Some(file) = library.download_file(source, id)? {
            let path = self.dir.join(&file.file);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e).with_context(|| format!("no se pudo borrar {}", path.display())),
            }
            library.remove_download(source, id)?;
        }
        Ok(())
    }

    /// Saves the whole audio to a file, piece by piece. `None` if cancelled.
    async fn fetch_to_disk(
        &self,
        registry: &Registry,
        library: &Library,
        http: &reqwest::Client,
        track: &Track,
        cancelled: &AtomicBool,
        on_progress: impl Fn(u64, Option<u64>),
    ) -> anyhow::Result<Option<DownloadFile>> {
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};
        // tokio::fs: writing to disk must not block the runtime's threads.
        tokio::fs::create_dir_all(&self.dir).await.context("no se pudo crear la carpeta de descargas")?;
        let stem = file_stem(&track.source, &track.id);
        let part = self.dir.join(format!("{stem}.part"));
        let mut out = tokio::fs::File::create(&part).await.context("no se pudo crear el archivo")?;

        let outcome = async {
            let mut info = stream::open(registry, library, http, &track.source, &track.id).await?;
            let mut refreshed = false;
            let mut received = 0u64;
            let mut total: Option<u64> = None;
            let mut mime = String::new();
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    return Ok(None);
                }
                let range = format!("bytes={received}-{}", received + PIECE - 1);
                let response = stream::fetch(http, &info, &range).await?;
                let status = response.status();
                // Signed URLs expire (long downloads, or a stale resolution): resolve again once.
                if (status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::GONE) && !refreshed {
                    refreshed = true;
                    info = stream::open(registry, library, http, &track.source, &track.id).await?;
                    continue;
                }
                // Asked past the end: done (when the size was never told, anything received counts).
                if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && total.map_or(received > 0, |t| t == received) {
                    break;
                }
                if !status.is_success() {
                    bail!("el servidor de audio respondió {status}");
                }
                if mime.is_empty() {
                    mime = header(&response, "content-type").unwrap_or_else(|| "audio/mpeg".into());
                }
                let whole = status == reqwest::StatusCode::OK; // the server ignored the range
                if whole && received > 0 {
                    // It sends the whole song again, from the start: keep that instead of
                    // appending it after what we already had.
                    out.set_len(0).await.context("no se pudo escribir el archivo")?;
                    out.seek(SeekFrom::Start(0)).await.context("no se pudo escribir el archivo")?;
                    received = 0;
                    total = None;
                }
                if total.is_none() {
                    total = if whole {
                        response.content_length()
                    } else {
                        header(&response, "content-range").and_then(|r| total_from_content_range(&r))
                    };
                }
                let bytes = response.bytes().await?;
                if bytes.is_empty() {
                    break;
                }
                out.write_all(&bytes).await.context("no se pudo escribir el archivo")?;
                received += bytes.len() as u64;
                on_progress(received, total);
                if whole || total.is_some_and(|t| received >= t) {
                    break;
                }
            }
            if received == 0 {
                bail!("el servidor no envió audio");
            }
            Ok(Some((received, mime)))
        }
        .await;

        // tokio finishes writes in the background: wait for them before renaming or deleting.
        let flushed = out.flush().await;
        drop(out);
        let outcome = match (outcome, flushed) {
            (Ok(Some(_)), Err(e)) => Err(anyhow::Error::from(e).context("no se pudo escribir el archivo")),
            (outcome, _) => outcome,
        };
        match outcome {
            Ok(Some((size, mime))) => {
                let file = format!("{stem}.{}", extension(&mime));
                tokio::fs::rename(&part, self.dir.join(&file)).await.context("no se pudo guardar el archivo")?;
                Ok(Some(DownloadFile { file, mime, size }))
            }
            other => {
                let _ = tokio::fs::remove_file(&part).await;
                other.map(|_| None)
            }
        }
    }
}

fn header(response: &reqwest::Response, name: &str) -> Option<String> {
    response.headers().get(name)?.to_str().ok().map(str::to_string)
}

/// "bytes 0-99/1234" → 1234
fn total_from_content_range(value: &str) -> Option<u64> {
    value.rsplit_once('/')?.1.trim().parse().ok()
}

/// A safe, unique file name for a song (ids may contain any character).
fn file_stem(source: &str, id: &str) -> String {
    let readable: String = format!("{source}-{id}")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(60)
        .collect();
    // A hash of the exact key, so different ids never share a file.
    format!("{readable}-{}", crate::text::fingerprint(&format!("{source}/{id}")))
}

fn extension(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or("").trim() {
        "audio/webm" | "video/webm" => "webm",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" | "video/mp4" => "m4a",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/ogg" | "audio/opus" => "ogg",
        "audio/aac" => "aac",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/wav" | "audio/x-wav" => "wav",
        _ => "audio",
    }
}

/// Reads `start..=end` of a downloaded file (end clamped to its size).
pub fn read_range(path: &Path, start: u64, end: u64, size: u64) -> anyhow::Result<Vec<u8>> {
    if start >= size || end < start {
        return Err(anyhow!("fuera del archivo"));
    }
    let end = end.min(size - 1);
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0; (end - start + 1) as usize];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_size_from_content_range() {
        assert_eq!(total_from_content_range("bytes 0-99/1234"), Some(1234));
        assert_eq!(total_from_content_range("bytes 0-99/*"), None);
    }

    #[test]
    fn file_names_are_safe_and_distinct() {
        let a = file_stem("youtube", "dQw4w9WgXcQ");
        assert!(a.starts_with("youtube-dQw4w9WgXcQ-"));
        let b = file_stem("lista", "Artista – Título/raro?");
        assert!(b.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{b}");
        assert_ne!(file_stem("x", "a/b"), file_stem("x", "a_b"), "same readable part, different key");
    }

    #[test]
    fn extension_from_content_type() {
        assert_eq!(extension("audio/webm; codecs=\"opus\""), "webm");
        assert_eq!(extension("audio/mp4"), "m4a");
        assert_eq!(extension("application/octet-stream"), "audio");
    }

    #[test]
    fn reads_a_range_clamped_to_the_file() {
        let dir = std::env::temp_dir().join(format!("melomango-dl-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.bin");
        fs::write(&path, b"0123456789").unwrap();
        assert_eq!(read_range(&path, 2, 4, 10).unwrap(), b"234");
        assert_eq!(read_range(&path, 8, 99, 10).unwrap(), b"89");
        assert!(read_range(&path, 10, 12, 10).is_err());
        assert!(read_range(&path, 5, 2, 10).is_err(), "end before start must not overflow");
        fs::remove_dir_all(&dir).unwrap();
    }

    fn song(id: &str) -> Track {
        Track {
            id: id.into(),
            source: "youtube".into(),
            title: format!("Canción {id}"),
            artist: "Artista".into(),
            duration_secs: 180,
            artwork_url: None,
            genre: None,
        }
    }

    fn stored(name: &str) -> DownloadFile {
        DownloadFile { file: name.into(), mime: "audio/webm".into(), size: 3 }
    }

    #[test]
    fn startup_cleans_unfinished_and_unknown_files() {
        let dir = std::env::temp_dir().join(format!("melomango-dl-orphans-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let library = Library::in_memory();
        let downloads = Downloads::new(dir.clone());
        fs::write(dir.join("kept.webm"), b"abc").unwrap();
        library.add_download(&song("a"), &stored("kept.webm"), StoredKind::Download).unwrap();
        fs::write(dir.join("half.part"), b"ab").unwrap();
        fs::write(dir.join("forgotten.webm"), b"abc").unwrap();

        downloads.clean_orphans(&library);
        let mut left: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect();
        left.sort();
        assert_eq!(left, ["kept.webm"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn deleting_removes_the_file_before_forgetting_it() {
        let dir = std::env::temp_dir().join(format!("melomango-dl-delete-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let library = Library::in_memory();
        let downloads = Downloads::new(dir.clone());
        fs::write(dir.join("a.webm"), b"abc").unwrap();
        library.add_download(&song("a"), &stored("a.webm"), StoredKind::Download).unwrap();
        // Its file was deleted by hand: forgetting it still works.
        library.add_download(&song("b"), &stored("b.webm"), StoredKind::Download).unwrap();

        downloads.remove(&library, "youtube", "a").unwrap();
        downloads.remove(&library, "youtube", "b").unwrap();
        assert!(!dir.join("a.webm").exists());
        assert!(library.download_file("youtube", "a").unwrap().is_none());
        assert!(library.download_file("youtube", "b").unwrap().is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// End-to-end against YouTube (plugin from the official repository): downloads a song
    /// and plays it back through the `stream://` protocol, from disk. `cargo test -- --ignored`
    #[tokio::test]
    #[ignore = "necesita red y yt-dlp"]
    async fn downloads_and_serves_a_youtube_track() {
        use crate::sources::MusicSource;
        use crate::stream::{handle, StreamCache};
        use tauri::http::{header, Request};

        let http = reqwest::Client::builder().user_agent("Melomango-tests").build().unwrap();
        let (youtube, plugin_dir) = crate::catalog::youtube_for_tests(&http).await;
        let track = youtube.search("shakira dai dai").await.unwrap().into_iter().next().expect("sin resultados");
        let registry = Arc::new(Registry::new(vec![youtube]));
        let library = Library::in_memory();
        let dir = std::env::temp_dir().join(format!("melomango-dl-e2e-{}", std::process::id()));
        let downloads = Downloads::new(dir.clone());

        let events = Mutex::new(Vec::new());
        downloads.download(&registry, &library, &http, &track, StoredKind::Download, |p| events.lock().unwrap().push(p)).await;
        let events = events.into_inner().unwrap();
        let last = events.last().unwrap();
        assert_eq!(last.state, State::Done, "{events:?}");
        assert!(events.iter().any(|p| p.state == State::Downloading && p.received > 0));
        let (path, file) = downloads.local(&library, "youtube", &track.id).expect("descargada");
        assert_eq!(fs::metadata(&path).unwrap().len(), file.size);
        assert_eq!(last.total, Some(file.size));
        println!("{} → {} ({} bytes, {})", track.title, path.display(), file.size, file.mime);

        // Played from disk: an unreachable source would not matter now.
        let offline = Arc::new(Registry::new(Vec::<Arc<dyn MusicSource>>::new()));
        let request = Request::builder()
            .uri(format!("http://stream.localhost/youtube%2F{}", track.id))
            .header(header::RANGE, "bytes=100-")
            .body(Vec::new())
            .unwrap();
        let response = handle(offline, &library, &http, &StreamCache::default(), &downloads, request).await;
        assert_eq!(response.status(), 206, "{}", String::from_utf8_lossy(response.body()));
        assert_eq!(&response.body()[..10], &fs::read(&path).unwrap()[100..110]);
        assert!(response.headers()[header::CONTENT_RANGE].to_str().unwrap().ends_with(&format!("/{}", file.size)));

        downloads.remove(&library, "youtube", &track.id).unwrap();
        assert!(!path.exists());
        assert!(downloads.local(&library, "youtube", &track.id).is_none());
        fs::remove_dir_all(&dir).ok();
        fs::remove_dir_all(&plugin_dir).ok();
    }

    #[test]
    fn the_cache_keeps_to_its_limit() {
        let dir = std::env::temp_dir().join(format!("melomango-cache-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let library = Library::in_memory();
        let downloads = Downloads::new(dir.clone());
        let song = |id: &str| Track {
            id: id.into(),
            source: "audius".into(),
            title: id.into(),
            artist: "A".into(),
            duration_secs: 100,
            artwork_url: None,
            genre: None,
        };
        let store = |id: &str, kind| {
            fs::write(dir.join(id), vec![0u8; 100]).unwrap();
            let file = DownloadFile { file: id.into(), mime: "audio/mpeg".into(), size: 100 };
            library.add_download(&song(id), &file, kind).unwrap();
        };
        store("a", StoredKind::Cache);
        store("b", StoredKind::Cache);
        store("c", StoredKind::Cache);
        store("mine", StoredKind::Download);
        downloads.set_cache_limit(&library, 250).unwrap();
        assert_eq!(library.cache_usage().unwrap(), (2, 200), "the oldest one went");
        assert!(!dir.join("a").exists() && dir.join("b").exists());
        downloads.clear_cache(&library).unwrap();
        assert_eq!(library.cache_usage().unwrap(), (0, 0));
        assert!(dir.join("mine").exists(), "downloads are not the cache");
        assert!(downloads.local(&library, "audius", "mine").is_some());
        fs::remove_dir_all(&dir).ok();
    }

    /// End-to-end against YouTube: a song played twice is cached on its own, and a later
    /// download keeps that copy. `cargo test -- --ignored`
    #[tokio::test]
    #[ignore = "necesita red y yt-dlp"]
    async fn caches_a_song_played_twice() {
        let http = reqwest::Client::builder().user_agent("Melomango-tests").build().unwrap();
        let (youtube, plugin_dir) = crate::catalog::youtube_for_tests(&http).await;
        let track = youtube.search("vetusta morla copenhague").await.unwrap().into_iter().next().expect("sin resultados");
        let registry = Arc::new(Registry::new(vec![youtube]));
        let library = Library::in_memory();
        let dir = std::env::temp_dir().join(format!("melomango-cache-e2e-{}", std::process::id()));
        let downloads = Downloads::new(dir.clone());

        library.record_play(&track).unwrap();
        downloads.set_cache_limit(&library, 1 << 30).unwrap();
        downloads.cache_if_favourite(&registry, &library, &http, &track).await;
        assert!(downloads.local(&library, "youtube", &track.id).is_none(), "played once: not yet");

        library.record_play(&track).unwrap();
        downloads.cache_if_favourite(&registry, &library, &http, &track).await;
        let (_, file) = downloads.local(&library, "youtube", &track.id).expect("cacheada");
        assert_eq!(library.cache_usage().unwrap(), (1, file.size));
        assert!(library.downloaded_keys().unwrap().is_empty(), "not shown as a download");

        // Downloading it now just keeps the cached copy.
        let events = Mutex::new(Vec::new());
        downloads.download(&registry, &library, &http, &track, StoredKind::Download, |p| events.lock().unwrap().push(p)).await;
        let events = events.into_inner().unwrap();
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].state, State::Done);
        assert_eq!(library.cache_usage().unwrap(), (0, 0));
        assert_eq!(library.downloaded_keys().unwrap(), [format!("youtube/{}", track.id)]);

        // Turning the cache off doesn't touch downloads.
        downloads.set_cache_limit(&library, 0).unwrap();
        assert!(downloads.local(&library, "youtube", &track.id).is_some());
        fs::remove_dir_all(&dir).ok();
        fs::remove_dir_all(&plugin_dir).ok();
    }
}
