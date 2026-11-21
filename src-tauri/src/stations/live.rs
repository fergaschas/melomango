//! Live radio through the `stream://` protocol (`stream://localhost/_live/<session>`).
//!
//! The window's CSP only lets `<audio>` load from `stream:`, so Rust proxies the station:
//! that also makes plain `http://` stations work, and lets us read what is playing (ICY
//! metadata) before the browser sees the audio.
//!
//! Tauri protocol responses are whole bodies, never an endless stream. So the live audio is
//! served like a file the browser reads in pieces (Range requests), with a made-up huge
//! size: one connection to the station fills a buffer, and each request takes the next bytes
//! from it, waiting for them if they haven't arrived yet. (With an unknown size, `bytes
//! 0-N/*`, Chromium takes the end of the first piece as the end of the file.) Seeking is
//! never offered, so the browser only reads ahead, plus a peek near the "end" of the file
//! when it looks for tags there, which gets silence.
//!
//! One station plays at a time: starting another stops the previous one, and a station
//! nobody asks for in a while stops on its own (paused and forgotten).

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail};
use percent_encoding::percent_decode_str;
use serde::Serialize;
use tauri::http::{Request, Response, header};
use tokio::sync::Notify;

/// The size the live stream pretends to have (1 TiB: days of audio at any bitrate).
pub const FAKE_SIZE: u64 = 1 << 40;
/// Path prefix of live streams in the `stream://` protocol (track paths are `<source>/<id>`).
pub const PREFIX: &str = "_live/";
/// Most bytes one answer carries.
const CHUNK: u64 = 256 * 1024;
/// An answer waits for at least this much (or `GATHER`), so the browser asks less often.
const MIN_BODY: u64 = 16 * 1024;
const GATHER: Duration = Duration::from_millis(300);
/// How long a request waits for the station before giving up.
const WAIT: Duration = Duration::from_secs(20);
/// Audio kept behind the newest byte, in case the browser reads a piece again.
const KEEP: usize = 16 * 1024 * 1024;
/// A request further ahead than this is a peek for tags at the "end": it gets silence.
const FAR_AHEAD: u64 = 4 * 1024 * 1024;
/// Without requests for this long (paused, closed), the connection to the station ends.
const IDLE: Duration = Duration::from_secs(60);
/// Playlists (`.m3u`, `.pls`) are small: anything bigger is not one.
const MAX_PLAYLIST: usize = 64 * 1024;

pub const NOT_RESPONDING: &str = "Esta emisora no responde ahora mismo. Prueba con otra.";
const UNSUPPORTED: &str = "Esta emisora emite en un formato que Melomango aún no puede reproducir. Prueba con otra.";

/// What a station says is playing (ICY `StreamTitle`), pushed to the frontend as the
/// `live-title` event. An empty title means it no longer says.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LiveTitle {
    pub session: u32,
    pub title: String,
}

pub type TitleSink = Arc<dyn Fn(LiveTitle) + Send + Sync>;

/// The audio received so far: bytes `start..end` of the station's stream (without metadata).
#[derive(Default)]
struct Buffer {
    data: VecDeque<u8>,
    start: u64,
    content_type: Option<String>,
    /// Why the connection ended, once it has (the stream ending counts too).
    finished: Option<String>,
}

impl Buffer {
    fn end(&self) -> u64 {
        self.start + self.data.len() as u64
    }

    fn push(&mut self, audio: &[u8]) {
        self.data.extend(audio);
        if self.data.len() > KEEP {
            let extra = self.data.len() - KEEP;
            self.data.drain(..extra);
            self.start += extra as u64;
        }
    }

    fn slice(&self, from: u64, to: u64) -> Vec<u8> {
        let a = (from - self.start) as usize;
        let b = (to - self.start) as usize;
        self.data.range(a..b).copied().collect()
    }
}

struct Session {
    id: u32,
    buffer: Mutex<Buffer>,
    /// Woken when audio arrives or the connection ends.
    arrived: Notify,
    last_request: Mutex<Instant>,
    reader: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Session {
    fn finish(&self, why: String) {
        let mut buffer = self.buffer.lock().unwrap();
        if buffer.finished.is_none() {
            buffer.finished = Some(why);
        }
        drop(buffer);
        self.arrived.notify_waiters();
    }

    fn stop(&self) {
        if let Some(reader) = self.reader.lock().unwrap().take() {
            reader.abort();
        }
        self.finish("detenida".into());
    }

    fn idle(&self) -> bool {
        self.last_request.lock().unwrap().elapsed() > IDLE
    }
}

/// The station playing now, if any.
#[derive(Default)]
pub struct LiveStreams {
    next_id: AtomicU32,
    current: Mutex<Option<Arc<Session>>>,
}

impl LiveStreams {
    /// Connects to `url` (stopping whatever played before) and returns the session the
    /// `stream://` URL names. Titles the station sends go to `on_title`.
    pub fn start(&self, http: reqwest::Client, url: String, on_title: TitleSink) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let session = Arc::new(Session {
            id,
            buffer: Mutex::default(),
            arrived: Notify::new(),
            last_request: Mutex::new(Instant::now()),
            reader: Mutex::default(),
        });
        let reader = tokio::spawn(read(session.clone(), http, url, on_title));
        *session.reader.lock().unwrap() = Some(reader);
        if let Some(old) = self.current.lock().unwrap().replace(session) {
            old.stop();
        }
        id
    }

    /// Stops the station (paused, or the player moved on to songs).
    pub fn stop(&self) {
        if let Some(old) = self.current.lock().unwrap().take() {
            old.stop();
        }
    }

    fn session(&self, id: u32) -> Option<Arc<Session>> {
        self.current.lock().unwrap().as_ref().filter(|s| s.id == id).cloned()
    }

    /// Answers a `stream://` request for a live session (`path` without the prefix).
    pub async fn handle(&self, path: &str, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
        if let Some(answer) = crate::stream::preflight(request) {
            return answer;
        }
        let range = request.headers().get(header::RANGE).and_then(|v| v.to_str().ok());
        let Some(session) = path.parse().ok().and_then(|id| self.session(id)) else {
            // An old session (another station started, or it stopped): nothing to play.
            return failure(410, "Esta emisora ya no está sonando.");
        };
        *session.last_request.lock().unwrap() = Instant::now();
        match serve(&session, range).await {
            Ok(response) => response,
            Err(message) => failure(502, &message),
        }
    }
}

/// The session in a `stream://` request, if it is for a live stream.
pub fn session_path(request: &Request<Vec<u8>>) -> Option<String> {
    let path = percent_decode_str(request.uri().path().trim_start_matches('/')).decode_utf8().ok()?;
    path.strip_prefix(PREFIX).map(str::to_string)
}

fn failure(status: u16, message: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        // The player reads this text to explain the failure (see usePlayer's streamFailure).
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(message.as_bytes().to_vec())
        .unwrap()
}

/// `bytes=start-end` -> (start, end inclusive), within the made-up size.
fn requested(range: Option<&str>) -> Option<(u64, u64)> {
    let (start, end) = range
        .and_then(|h| h.strip_prefix("bytes="))
        .and_then(|r| r.split(',').next())
        .and_then(|r| r.split_once('-'))
        .and_then(|(s, e)| Some((s.trim().parse::<u64>().ok()?, e.trim().parse::<u64>().ok())))
        .unwrap_or((0, None));
    let end = end.unwrap_or(FAKE_SIZE - 1).min(FAKE_SIZE - 1);
    (start <= end).then_some((start, end))
}

fn piece(content_type: &str, start: u64, body: Vec<u8>) -> Response<Vec<u8>> {
    let last = start + body.len() as u64 - 1;
    Response::builder()
        .status(206)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_RANGE, format!("bytes {start}-{last}/{FAKE_SIZE}"))
        .header(header::CONTENT_LENGTH, body.len())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body)
        .unwrap()
}

async fn serve(session: &Session, range: Option<&str>) -> Result<Response<Vec<u8>>, String> {
    let Some((start, end)) = requested(range) else {
        return Ok(Response::builder()
            .status(416)
            .header(header::CONTENT_RANGE, format!("bytes */{FAKE_SIZE}"))
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Vec::new())
            .unwrap());
    };
    let deadline = tokio::time::Instant::now() + WAIT;
    let mut gathering: Option<tokio::time::Instant> = None;
    loop {
        // Registered before looking, so audio arriving in between still wakes us.
        let arrived = session.arrived.notified();
        tokio::pin!(arrived);
        arrived.as_mut().enable();
        {
            let buffer = session.buffer.lock().unwrap();
            let have = buffer.end();
            let content_type = buffer.content_type.clone().unwrap_or_else(|| "audio/mpeg".into());
            if start > have + FAR_AHEAD {
                // Looking for tags at the end of the "file": there are none.
                let len = (end - start + 1).min(64 * 1024);
                return Ok(piece(&content_type, start, vec![0; len as usize]));
            }
            if start < buffer.start {
                return Err("La emisora se ha quedado atrás: vuelve a ponerla.".into());
            }
            let ready = have > start;
            let enough = have >= start + MIN_BODY || buffer.finished.is_some();
            let waited = gathering.is_some_and(|g| tokio::time::Instant::now() >= g);
            if ready && (enough || waited) {
                let to = (end + 1).min(have).min(start + CHUNK);
                return Ok(piece(&content_type, start, buffer.slice(start, to)));
            }
            if !ready && let Some(why) = &buffer.finished {
                return Err(why.clone());
            }
            if ready && gathering.is_none() {
                gathering = Some(tokio::time::Instant::now() + GATHER);
            }
        }
        let until = gathering.map_or(deadline, |g| g.min(deadline));
        if tokio::time::timeout_at(until, arrived).await.is_err() && gathering.is_none() {
            return Err(NOT_RESPONDING.into());
        }
    }
}

/// Keeps reading the station into the session's buffer until it ends, fails, is stopped or
/// nobody listens any more.
async fn read(session: Arc<Session>, http: reqwest::Client, url: String, on_title: TitleSink) {
    let result: anyhow::Result<()> = async {
        let mut response = connect(&http, &url).await?;
        let content_type = audio_type(response.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()));
        session.buffer.lock().unwrap().content_type = Some(content_type);
        let metaint = response
            .headers()
            .get("icy-metaint")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        let mut icy = IcyReader::new(metaint);
        let mut audio = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            audio.clear();
            let title = icy.feed(&chunk, &mut audio);
            session.buffer.lock().unwrap().push(&audio);
            session.arrived.notify_waiters();
            if let Some(title) = title {
                on_title(LiveTitle { session: session.id, title });
            }
            if session.idle() {
                return Ok(());
            }
        }
        Ok(())
    }
    .await;
    session.finish(match result {
        Ok(()) => NOT_RESPONDING.to_string(),
        Err(e) if e.to_string() == UNSUPPORTED => UNSUPPORTED.to_string(),
        Err(e) => {
            eprintln!("emisora {url}: {e:#}");
            NOT_RESPONDING.to_string()
        }
    });
}

/// Opens the station's audio. A playlist (`.m3u`, `.pls`) is followed to its first stream.
async fn connect(http: &reqwest::Client, url: &str) -> anyhow::Result<reqwest::Response> {
    let mut url = url.to_string();
    for _ in 0..2 {
        let response = http.get(&url).header("Icy-MetaData", "1").send().await?.error_for_status()?;
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if content_type.starts_with("text/html") {
            bail!(UNSUPPORTED);
        }
        let is_playlist = ["mpegurl", "scpls", "x-mpegurl"].iter().any(|t| content_type.contains(t));
        if !is_playlist {
            return Ok(response);
        }
        let text = playlist_text(response).await?;
        url = first_stream(&text).ok_or_else(|| anyhow!(UNSUPPORTED))?;
    }
    bail!(UNSUPPORTED)
}

async fn playlist_text(mut response: reqwest::Response) -> anyhow::Result<String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX_PLAYLIST {
            bail!(UNSUPPORTED);
        }
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The first stream of an `.m3u` or `.pls` playlist. None for HLS (`#EXT-X-…`), which the
/// app doesn't play yet.
fn first_stream(text: &str) -> Option<String> {
    if text.contains("#EXT-X-") {
        return None;
    }
    text.lines()
        .map(str::trim)
        .map(|line| match line.split_once('=') {
            // .pls: File1=http://…
            Some((key, value)) if key.to_ascii_lowercase().starts_with("file") => value.trim(),
            _ => line,
        })
        .find(|line| line.starts_with("http://") || line.starts_with("https://"))
        .map(str::to_string)
}

/// The type the `<audio>` element is told, from what the station said.
fn audio_type(content_type: Option<&str>) -> String {
    let t = content_type.unwrap_or_default().split(';').next().unwrap_or_default().trim().to_ascii_lowercase();
    match t.as_str() {
        "audio/aacp" | "audio/x-aac" | "audio/aac" => "audio/aac".into(),
        "application/ogg" | "audio/ogg" | "audio/x-ogg" => "audio/ogg".into(),
        "audio/mpeg" | "audio/mp3" | "audio/x-mpeg" => "audio/mpeg".into(),
        t if t.starts_with("audio/") => t.into(),
        // Missing or odd («application/octet-stream»): most stations are MP3, and the
        // browser looks at the bytes anyway.
        _ => "audio/mpeg".into(),
    }
}

/// Takes the ICY metadata out of a station's stream: every `metaint` bytes of audio comes
/// a length byte (×16) and that much text, such as `StreamTitle='Artista - Canción';`.
pub struct IcyReader {
    metaint: usize,
    /// Audio bytes left before the next metadata block.
    audio_left: usize,
    /// Inside a metadata block: its bytes still to come.
    meta_left: Option<usize>,
    meta: Vec<u8>,
    title: Option<String>,
}

impl IcyReader {
    /// `metaint` 0: the station sends no metadata, everything is audio.
    pub fn new(metaint: usize) -> Self {
        Self { metaint, audio_left: metaint, meta_left: None, meta: Vec::new(), title: None }
    }

    /// Appends the audio in `chunk` to `audio`. Returns the title if it changed.
    pub fn feed(&mut self, mut chunk: &[u8], audio: &mut Vec<u8>) -> Option<String> {
        if self.metaint == 0 {
            audio.extend_from_slice(chunk);
            return None;
        }
        let mut changed = None;
        while !chunk.is_empty() {
            match self.meta_left {
                None if self.audio_left > 0 => {
                    let n = self.audio_left.min(chunk.len());
                    audio.extend_from_slice(&chunk[..n]);
                    self.audio_left -= n;
                    chunk = &chunk[n..];
                }
                None => {
                    let len = chunk[0] as usize * 16;
                    chunk = &chunk[1..];
                    self.meta.clear();
                    if len == 0 {
                        self.audio_left = self.metaint;
                    } else {
                        self.meta_left = Some(len);
                    }
                }
                Some(left) => {
                    let n = left.min(chunk.len());
                    self.meta.extend_from_slice(&chunk[..n]);
                    chunk = &chunk[n..];
                    if left == n {
                        self.meta_left = None;
                        self.audio_left = self.metaint;
                        if let Some(title) = stream_title(&self.meta)
                            && self.title.as_ref() != Some(&title)
                        {
                            self.title = Some(title.clone());
                            changed = Some(title);
                        }
                    } else {
                        self.meta_left = Some(left - n);
                    }
                }
            }
        }
        changed
    }
}

/// `StreamTitle='…';` of a metadata block. Empty when the station sends nothing useful.
fn stream_title(meta: &[u8]) -> Option<String> {
    let meta = &meta[..meta.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1)];
    // Most stations send UTF-8; older ones, Latin-1.
    let text = match std::str::from_utf8(meta) {
        Ok(text) => text.to_string(),
        Err(_) => meta.iter().map(|&b| b as char).collect(),
    };
    let start = text.find("StreamTitle='")? + "StreamTitle='".len();
    let rest = &text[start..];
    let end = rest.find("';").unwrap_or_else(|| rest.trim_end_matches(';').trim_end_matches('\'').len());
    let title = rest[..end].trim();
    // « - » alone: a station that sends the separator with no artist or song.
    Some(if title.trim_matches(|c: char| c == '-' || c.is_whitespace()).is_empty() {
        String::new()
    } else {
        title.into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(text: &str) -> Vec<u8> {
        let mut meta = text.as_bytes().to_vec();
        meta.resize(meta.len().div_ceil(16) * 16, 0);
        let mut out = vec![(meta.len() / 16) as u8];
        out.extend(meta);
        out
    }

    #[test]
    fn takes_the_metadata_out_of_the_audio() {
        let mut stream = b"AAAA".to_vec();
        stream.extend(block("StreamTitle='Rosalía - La perla';"));
        stream.extend(b"BBBB");
        stream.push(0); // no metadata this time
        stream.extend(b"CCCC");
        stream.extend(block("StreamTitle='Rosalía - La perla';StreamUrl='';"));
        stream.extend(b"DD");

        // Fed whole and fed byte by byte, the result is the same.
        for size in [stream.len(), 1, 3] {
            let mut icy = IcyReader::new(4);
            let mut audio = Vec::new();
            let mut titles = Vec::new();
            for piece in stream.chunks(size) {
                titles.extend(icy.feed(piece, &mut audio));
            }
            assert_eq!(audio, b"AAAABBBBCCCCDD", "pieces of {size}");
            assert_eq!(titles, ["Rosalía - La perla"], "the same title again is not news");
        }
    }

    #[test]
    fn without_metaint_everything_is_audio() {
        let mut icy = IcyReader::new(0);
        let mut audio = Vec::new();
        assert_eq!(icy.feed(b"\x02StreamTitle", &mut audio), None);
        assert_eq!(audio, b"\x02StreamTitle");
    }

    #[test]
    fn reads_titles_in_utf8_and_latin1() {
        assert_eq!(stream_title(b"StreamTitle='Ana Bel\xc3\xa9n - Contamina';\0\0").unwrap(), "Ana Belén - Contamina");
        assert_eq!(stream_title(b"StreamTitle='Ana Bel\xe9n - Contamina';").unwrap(), "Ana Belén - Contamina");
        assert_eq!(stream_title(b"StreamTitle='Rock 'n' Roll';").unwrap(), "Rock 'n' Roll");
        assert_eq!(stream_title(b"StreamTitle=' - ';").unwrap(), "");
        assert_eq!(stream_title(b"StreamTitle='';").unwrap(), "");
        assert_eq!(stream_title(b"StreamUrl='x';"), None);
    }

    #[test]
    fn a_title_that_goes_away_is_news() {
        let mut icy = IcyReader::new(1);
        let mut audio = Vec::new();
        let mut stream = b"A".to_vec();
        stream.extend(block("StreamTitle='Canción';"));
        stream.extend(b"B");
        stream.extend(block("StreamTitle='';"));
        let mut titles = Vec::new();
        titles.extend(icy.feed(&stream, &mut audio));
        assert_eq!(audio, b"AB");
        // Only the latest change of one feed is returned.
        assert_eq!(titles, [""]);
    }

    #[test]
    fn ranges_stay_within_the_made_up_size() {
        assert_eq!(requested(Some("bytes=0-")), Some((0, FAKE_SIZE - 1)));
        assert_eq!(requested(Some("bytes=10-99")), Some((10, 99)));
        assert_eq!(requested(None), Some((0, FAKE_SIZE - 1)));
        assert_eq!(requested(Some("bytes=99-10")), None);
        assert_eq!(requested(Some(&format!("bytes={FAKE_SIZE}-"))), None);
    }

    #[test]
    fn follows_playlists_but_not_hls() {
        assert_eq!(first_stream("#EXTM3U\n#EXTINF:-1,Radio\nhttp://a/stream\n").as_deref(), Some("http://a/stream"));
        assert_eq!(
            first_stream("[playlist]\nNumberOfEntries=1\nFile1=https://b/live\nTitle1=B\n").as_deref(),
            Some("https://b/live")
        );
        assert_eq!(first_stream("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:10\nseg1.ts\n"), None);
        assert_eq!(first_stream("nothing here"), None);
    }

    #[test]
    fn tells_the_browser_a_type_it_knows() {
        assert_eq!(audio_type(Some("audio/aacp")), "audio/aac");
        assert_eq!(audio_type(Some("audio/mpeg; charset=x")), "audio/mpeg");
        assert_eq!(audio_type(Some("application/ogg")), "audio/ogg");
        assert_eq!(audio_type(Some("application/octet-stream")), "audio/mpeg");
        assert_eq!(audio_type(None), "audio/mpeg");
    }

    #[test]
    fn the_buffer_keeps_only_the_latest_audio() {
        let mut buffer = Buffer::default();
        buffer.push(&vec![1; KEEP]);
        buffer.push(&[2; 10]);
        assert_eq!((buffer.start, buffer.end()), (10, KEEP as u64 + 10));
        assert_eq!(buffer.slice(KEEP as u64 + 8, KEEP as u64 + 10), [2, 2]);
    }

    /// A tiny station on this computer: an HTTP answer with ICY metadata every 8 bytes,
    /// then `parts` pieces of audio, a little apart.
    async fn fake_station(parts: Vec<&'static [u8]>, title: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 2048];
            let n = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]).to_ascii_lowercase();
            assert!(request.contains("icy-metadata: 1"), "{request}");
            let head = "HTTP/1.1 200 OK\r\ncontent-type: audio/aacp\r\nicy-metaint: 8\r\nconnection: close\r\n\r\n";
            socket.write_all(head.as_bytes()).await.unwrap();
            let mut icy = Vec::new();
            for part in parts {
                icy.extend_from_slice(part);
            }
            // Metadata after the first 8 bytes of audio, then none (a zero) every 8 after.
            let mut out = Vec::new();
            for (i, piece) in icy.chunks(8).enumerate() {
                out.extend_from_slice(piece);
                if piece.len() == 8 {
                    if i == 0 { out.extend(block(&format!("StreamTitle='{title}';"))) } else { out.push(0) }
                }
            }
            for piece in out.chunks(10) {
                socket.write_all(piece).await.unwrap();
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        format!("http://{addr}/stream")
    }

    fn get(session: u32, range: &str) -> Request<Vec<u8>> {
        Request::builder()
            .uri(format!("http://stream.localhost/_live%2F{session}"))
            .header(header::RANGE, range)
            .body(Vec::new())
            .unwrap()
    }

    #[tokio::test]
    async fn serves_a_station_in_pieces_without_its_metadata() {
        let url = fake_station(vec![b"0123456789abcdef", b"ghijklmnopqrstuv"], "Radio - Prueba").await;
        let titles = Arc::new(Mutex::new(Vec::new()));
        let sink: TitleSink = {
            let titles = titles.clone();
            Arc::new(move |t| titles.lock().unwrap().push(t))
        };
        let live = LiveStreams::default();
        let id = live.start(reqwest::Client::new(), url, sink);
        let request = get(id, "bytes=0-");
        assert_eq!(session_path(&request).as_deref(), Some(id.to_string().as_str()));

        // The first piece waits for audio and is labelled as part of a huge file.
        let first = live.handle(&id.to_string(), &request).await;
        assert_eq!(first.status(), 206, "{}", String::from_utf8_lossy(first.body()));
        assert_eq!(first.headers()[header::CONTENT_TYPE], "audio/aac");
        let got = first.body().clone();
        assert!(!got.is_empty());
        assert_eq!(
            first.headers()[header::CONTENT_RANGE].to_str().unwrap(),
            format!("bytes 0-{}/{FAKE_SIZE}", got.len() - 1)
        );

        // Reading on from there gets the rest, with no metadata in between.
        let mut all = got;
        while all.len() < 32 {
            let next = live.handle(&id.to_string(), &get(id, &format!("bytes={}-", all.len()))).await;
            assert_eq!(next.status(), 206, "{}", String::from_utf8_lossy(next.body()));
            all.extend_from_slice(next.body());
        }
        assert_eq!(all, b"0123456789abcdefghijklmnopqrstuv");
        assert_eq!(*titles.lock().unwrap(), [LiveTitle { session: id, title: "Radio - Prueba".into() }]);

        // A peek near the "end" of the file gets silence, at once.
        let tail = live.handle(&id.to_string(), &get(id, &format!("bytes={}-", FAKE_SIZE - 128))).await;
        assert_eq!((tail.status().as_u16(), tail.body().len()), (206, 128));
        assert!(tail.body().iter().all(|&b| b == 0));

        // The station hung up: past its last byte there is nothing more, and it says so.
        let after = live.handle(&id.to_string(), &get(id, "bytes=32-")).await;
        assert_eq!(after.status(), 502);
        assert_eq!(String::from_utf8_lossy(after.body()), NOT_RESPONDING);

        // Another station replaces it: the old session is gone.
        live.stop();
        let gone = live.handle(&id.to_string(), &get(id, "bytes=0-")).await;
        assert_eq!(gone.status(), 410);
    }

    /// Listens to a real station (the most listened in Spain) for a few pieces.
    /// `cargo test -- --ignored`
    #[tokio::test]
    #[ignore = "necesita red"]
    async fn plays_a_real_station() {
        let http = reqwest::Client::builder().user_agent("Melomango-tests").build().unwrap();
        let top = crate::stations::RadioBrowser::new(http.clone()).top("ES").await.unwrap();
        let station = top.iter().find(|s| s.codec == "MP3").unwrap();
        let live = LiveStreams::default();
        let id = live.start(http, station.url.clone(), Arc::new(|_| {}));
        let mut audio = Vec::new();
        while audio.len() < 64 * 1024 {
            let piece = live.handle(&id.to_string(), &get(id, &format!("bytes={}-", audio.len()))).await;
            assert_eq!(piece.status(), 206, "{}: {}", station.name, String::from_utf8_lossy(piece.body()));
            audio.extend_from_slice(piece.body());
        }
        // MP3 frames start with 11 set bits: with the metadata taken out, they keep coming.
        let syncs = audio.windows(2).filter(|w| w[0] == 0xff && w[1] & 0xe0 == 0xe0).count();
        assert!(syncs > 20, "{}: {syncs} frames", station.name);
        live.stop();
    }

    #[tokio::test]
    async fn a_station_that_does_not_answer_fails_with_a_clear_message() {
        // Nothing listens on this port (bound and dropped right away).
        let addr = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        let live = LiveStreams::default();
        let id = live.start(reqwest::Client::new(), format!("http://{addr}/"), Arc::new(|_| {}));
        let answer = live.handle(&id.to_string(), &get(id, "bytes=0-")).await;
        assert_eq!(answer.status(), 502);
        assert_eq!(String::from_utf8_lossy(answer.body()), NOT_RESPONDING);
    }
}
