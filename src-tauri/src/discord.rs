//! "Escuchando Melomango" in Discord (Rich Presence).
//!
//! Talks to the Discord app running on this computer through its local IPC pipe
//! (`\\.\pipe\discord-ipc-N` on Windows, `discord-ipc-N` sockets elsewhere): nothing goes
//! over the internet from Melomango. Frames are `op` (u32 LE) + length (u32 LE) + JSON.
//!
//! A background task keeps the latest wanted activity and sends it when it can:
//! it connects when Discord is open (retrying every so often if it isn't), coalesces
//! quick changes and respects Discord's limit of a few updates per 20 seconds.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::watch,
};

/// Discord application whose name shows as "Escuchando <name>". Created at
/// https://discord.com/developers/applications; empty disables the feature.
pub const APPLICATION_ID: &str = "";
/// Shown as the small image (and as the large one when a track has no cover).
const ICON_URL: &str = "https://raw.githubusercontent.com/fergaschas/melomango/main/src-tauri/icons/128x128.png";
/// Discord allows about 5 activity updates per 20 seconds.
const MIN_INTERVAL: Duration = Duration::from_secs(4);
/// How often to look for Discord again while it isn't open.
const RETRY_EVERY: Duration = Duration::from_secs(20);
const IO_TIMEOUT: Duration = Duration::from_secs(5);

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
const OP_PING: u32 = 3;
const OP_PONG: u32 = 4;

/// What is playing, as the frontend reports it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub artwork_url: Option<String>,
    pub duration_secs: f64,
    /// Seconds into the track right now.
    pub position_secs: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Status {
    /// No application id configured.
    Unavailable,
    /// Nothing to show (paused, stopped or turned off).
    Idle,
    /// Something to show, but the Discord app isn't running here.
    NotRunning,
    Connected,
    Error { message: String },
}

pub struct Presence {
    wanted: watch::Sender<Option<NowPlaying>>,
    status: watch::Receiver<Status>,
}

impl Presence {
    /// The handle, and the task that does the work (the caller spawns it).
    pub fn new(client_id: &str) -> (Self, impl std::future::Future<Output = ()> + Send + 'static) {
        Self::with_pipes(client_id, default_pipes())
    }

    fn with_pipes(
        client_id: &str,
        pipes: Vec<String>,
    ) -> (Self, impl std::future::Future<Output = ()> + Send + 'static) {
        let (wanted, wanted_rx) = watch::channel(None);
        let initial = if client_id.is_empty() { Status::Unavailable } else { Status::Idle };
        let (status_tx, status) = watch::channel(initial);
        let task = run(client_id.to_string(), pipes, wanted_rx, status_tx);
        (Self { wanted, status }, task)
    }

    /// Shows `now` in Discord, or clears it with `None`.
    pub fn set(&self, now: Option<NowPlaying>) {
        self.wanted.send_if_modified(|current| {
            let changed = *current != now;
            *current = now;
            changed
        });
    }

    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }
}

async fn run(
    client_id: String,
    pipes: Vec<String>,
    mut wanted: watch::Receiver<Option<NowPlaying>>,
    status: watch::Sender<Status>,
) {
    if client_id.is_empty() {
        return;
    }
    let mut conn: Option<Connection> = None;
    let mut last_sent: Option<Option<NowPlaying>> = None;
    loop {
        let now = wanted.borrow_and_update().clone();
        if last_sent.as_ref() != Some(&now) {
            // A connection that broke (Discord closed or restarted) is noticed only when
            // writing to it: then one fresh connection right away, instead of showing an
            // error until the next retry (~24 s).
            for attempt in 0..2 {
                if conn.is_none() && now.is_some() {
                    match Connection::open(&pipes, &client_id).await {
                        Ok(c) => conn = Some(c),
                        Err(e) => {
                            let _ = status.send(if e.is::<NotRunning>() {
                                Status::NotRunning
                            } else {
                                Status::Error { message: format!("{e:#}") }
                            });
                            break;
                        }
                    }
                }
                let Some(c) = conn.as_mut() else {
                    if now.is_none() {
                        last_sent = Some(None);
                        let _ = status.send(Status::Idle);
                    }
                    break;
                };
                match c.set_activity(now.as_ref()).await {
                    Ok(()) => {
                        last_sent = Some(now.clone());
                        let _ = status.send(if now.is_some() { Status::Connected } else { Status::Idle });
                        break;
                    }
                    Err(e) => {
                        conn = None;
                        if attempt == 1 || now.is_none() {
                            let _ = status.send(Status::Error { message: format!("{e:#}") });
                        }
                    }
                }
            }
            tokio::time::sleep(MIN_INTERVAL).await;
        }

        let pending = last_sent.as_ref() != Some(&*wanted.borrow());
        if pending {
            // Something left to show (Discord wasn't open): look again later or on a change.
            tokio::select! {
                changed = wanted.changed() => if changed.is_err() { return },
                _ = tokio::time::sleep(RETRY_EVERY) => {}
            }
        } else if wanted.changed().await.is_err() {
            return; // the app is closing
        }
    }
}

/// Discord's activity for `now` ("Escuchando", with a progress bar while playing).
fn activity(now: &NowPlaying) -> Value {
    let start_ms = epoch_ms() - (now.position_secs * 1000.0) as i64;
    let mut activity = json!({
        "type": 2, // Listening
        "details": field(&now.title),
        "state": field(&now.artist),
        "assets": {
            "large_image": now.artwork_url.as_deref().filter(|u| u.starts_with("https://") && u.len() <= 256).unwrap_or(ICON_URL),
            "large_text": field(&now.title),
            "small_image": ICON_URL,
            "small_text": "Melomango",
        },
        "instance": false,
    });
    if now.duration_secs > 0.0 {
        activity["timestamps"] = json!({
            "start": start_ms,
            "end": start_ms + (now.duration_secs * 1000.0) as i64,
        });
    }
    activity
}

/// Discord wants 2–128 characters.
fn field(text: &str) -> String {
    let text: String = text.trim().chars().take(128).collect();
    if text.chars().count() < 2 { format!("{text:<2}") } else { text }
}

fn epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------- IPC ----------

#[derive(Debug)]
struct NotRunning;

impl std::fmt::Display for NotRunning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Discord no está abierto")
    }
}

impl std::error::Error for NotRunning {}

trait Pipe: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Pipe for T {}

struct Connection {
    pipe: Box<dyn Pipe>,
    nonce: u64,
}

impl Connection {
    async fn open(pipes: &[String], client_id: &str) -> anyhow::Result<Self> {
        let pipe = connect_any(pipes).await.ok_or(NotRunning)?;
        let mut conn = Self { pipe, nonce: 0 };
        conn.write(OP_HANDSHAKE, &json!({ "v": 1, "client_id": client_id })).await?;
        let (op, reply) = conn.read().await?;
        if op == OP_CLOSE || reply["evt"] != "READY" {
            let message = reply["message"].as_str().unwrap_or("respuesta inesperada");
            anyhow::bail!("Discord rechazó la conexión: {message}");
        }
        Ok(conn)
    }

    async fn set_activity(&mut self, now: Option<&NowPlaying>) -> anyhow::Result<()> {
        self.nonce += 1;
        let frame = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": now.map(activity) },
            "nonce": format!("melomango-{}", self.nonce),
        });
        self.write(OP_FRAME, &frame).await?;
        loop {
            let (op, reply) = self.read().await?;
            match op {
                OP_PING => self.write(OP_PONG, &reply).await?,
                OP_CLOSE => anyhow::bail!("Discord cerró la conexión"),
                _ if reply["evt"] == "ERROR" => {
                    anyhow::bail!("Discord: {}", reply["data"]["message"].as_str().unwrap_or("error"))
                }
                _ if reply["nonce"] == frame["nonce"] => return Ok(()),
                _ => {} // another event: keep waiting for our answer
            }
        }
    }

    async fn write(&mut self, op: u32, payload: &Value) -> anyhow::Result<()> {
        let body = serde_json::to_vec(payload)?;
        let mut frame = Vec::with_capacity(8 + body.len());
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&body);
        tokio::time::timeout(IO_TIMEOUT, self.pipe.write_all(&frame)).await??;
        Ok(())
    }

    async fn read(&mut self) -> anyhow::Result<(u32, Value)> {
        let read = async {
            let mut header = [0u8; 8];
            self.pipe.read_exact(&mut header).await?;
            let op = u32::from_le_bytes(header[..4].try_into().unwrap());
            let len = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
            anyhow::ensure!(len <= 64 * 1024, "mensaje de Discord demasiado grande");
            let mut body = vec![0u8; len];
            self.pipe.read_exact(&mut body).await?;
            Ok::<_, anyhow::Error>((op, serde_json::from_slice(&body)?))
        };
        tokio::time::timeout(IO_TIMEOUT, read).await?
    }
}

fn default_pipes() -> Vec<String> {
    (0..10)
        .map(|i| {
            if cfg!(windows) {
                format!(r"\\.\pipe\discord-ipc-{i}")
            } else {
                let dir = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
                    .iter()
                    .find_map(|v| std::env::var(v).ok())
                    .unwrap_or_else(|| "/tmp".into());
                format!("{dir}/discord-ipc-{i}")
            }
        })
        .collect()
}

#[cfg(windows)]
async fn connect_any(pipes: &[String]) -> Option<Box<dyn Pipe>> {
    use tokio::net::windows::named_pipe::ClientOptions;
    for name in pipes {
        if let Ok(client) = ClientOptions::new().open(name) {
            return Some(Box::new(client));
        }
    }
    None
}

#[cfg(unix)]
async fn connect_any(pipes: &[String]) -> Option<Box<dyn Pipe>> {
    for name in pipes {
        if let Ok(stream) = tokio::net::UnixStream::connect(name).await {
            return Some(Box::new(stream));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now_playing(title: &str) -> NowPlaying {
        NowPlaying {
            title: title.into(),
            artist: "Vetusta Morla".into(),
            artwork_url: Some("https://e-cdns-images.dzcdn.net/images/cover/x/500x500.jpg".into()),
            duration_secs: 304.0,
            position_secs: 10.0,
        }
    }

    #[test]
    fn builds_a_listening_activity() {
        let a = activity(&now_playing("Copenhague"));
        assert_eq!(a["type"], 2);
        assert_eq!(a["details"], "Copenhague");
        assert_eq!(a["state"], "Vetusta Morla");
        assert!(a["assets"]["large_image"].as_str().unwrap().starts_with("https://e-cdns"));
        let (start, end) = (a["timestamps"]["start"].as_i64().unwrap(), a["timestamps"]["end"].as_i64().unwrap());
        assert_eq!(end - start, 304_000);
        assert!((epoch_ms() - start - 10_000).abs() < 2_000, "started 10 s ago");

        let mut unknown = now_playing("A");
        unknown.duration_secs = 0.0;
        unknown.artwork_url = None;
        let a = activity(&unknown);
        assert_eq!(a["details"], "A ", "Discord needs at least 2 characters");
        assert!(a.get("timestamps").is_none());
        assert_eq!(a["assets"]["large_image"], ICON_URL);
    }

    #[test]
    fn long_texts_are_cut() {
        assert_eq!(field(&"x".repeat(300)).chars().count(), 128);
    }

    #[tokio::test]
    async fn without_an_application_id_it_stays_off() {
        let (presence, task) = Presence::with_pipes("", vec![]);
        task.await; // returns at once
        presence.set(Some(now_playing("Copenhague")));
        assert_eq!(presence.status(), Status::Unavailable);
    }

    #[tokio::test]
    async fn reports_when_discord_is_not_running() {
        let (presence, task) = Presence::with_pipes("123", vec![r"\\.\pipe\melomango-no-discord-here".into()]);
        let handle = tokio::spawn(task);
        presence.set(Some(now_playing("Copenhague")));
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(presence.status(), Status::NotRunning);
        handle.abort();
    }

    /// A fake Discord answering on its own pipe: handshake, then activity updates.
    #[cfg(windows)]
    #[tokio::test]
    async fn talks_to_discord() {
        use tokio::net::windows::named_pipe::ServerOptions;

        let name = format!(r"\\.\pipe\melomango-test-discord-{}", std::process::id());
        let mut server = ServerOptions::new().first_pipe_instance(true).create(&name).unwrap();
        let (frames_tx, mut frames) = tokio::sync::mpsc::unbounded_channel::<(u32, Value)>();
        let fake = tokio::spawn(async move {
            server.connect().await.unwrap();
            loop {
                let mut header = [0u8; 8];
                if server.read_exact(&mut header).await.is_err() {
                    return;
                }
                let op = u32::from_le_bytes(header[..4].try_into().unwrap());
                let len = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
                let mut body = vec![0u8; len];
                server.read_exact(&mut body).await.unwrap();
                let msg: Value = serde_json::from_slice(&body).unwrap();
                let reply = if op == OP_HANDSHAKE {
                    json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "v": 1 } })
                } else {
                    json!({ "cmd": "SET_ACTIVITY", "evt": null, "nonce": msg["nonce"], "data": msg["args"]["activity"] })
                };
                frames_tx.send((op, msg)).unwrap();
                let body = serde_json::to_vec(&reply).unwrap();
                let mut out = OP_FRAME.to_le_bytes().to_vec();
                out.extend_from_slice(&(body.len() as u32).to_le_bytes());
                out.extend_from_slice(&body);
                server.write_all(&out).await.unwrap();
            }
        });

        let (presence, task) = Presence::with_pipes("1234567890", vec![name]);
        let handle = tokio::spawn(task);
        presence.set(Some(now_playing("Copenhague")));

        let (op, handshake) = frames.recv().await.unwrap();
        assert_eq!((op, handshake["client_id"].as_str()), (OP_HANDSHAKE, Some("1234567890")));
        let (op, update) = frames.recv().await.unwrap();
        assert_eq!(op, OP_FRAME);
        assert_eq!(update["cmd"], "SET_ACTIVITY");
        assert_eq!(update["args"]["activity"]["details"], "Copenhague");
        assert_eq!(update["args"]["pid"], std::process::id());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(presence.status(), Status::Connected);

        // Pausing clears it (after the minimum interval between updates).
        presence.set(None);
        let (_, cleared) = tokio::time::timeout(Duration::from_secs(10), frames.recv()).await.unwrap().unwrap();
        assert!(cleared["args"]["activity"].is_null());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(presence.status(), Status::Idle);

        handle.abort();
        fake.abort();
    }

    /// Discord restarts: the first connection dies right after the handshake. The update
    /// goes out on a new connection straight away, not after the ~24 s retry.
    #[cfg(windows)]
    #[tokio::test]
    async fn reconnects_at_once_when_discord_restarts() {
        use tokio::net::windows::named_pipe::ServerOptions;

        let name = format!(r"\\.\pipe\melomango-test-discord-restart-{}", std::process::id());
        let mut first = ServerOptions::new().first_pipe_instance(true).create(&name).unwrap();
        let (updates_tx, mut updates) = tokio::sync::mpsc::unbounded_channel::<Value>();
        let pipe_name = name.clone();
        let fake = tokio::spawn(async move {
            async fn frame(server: &mut tokio::net::windows::named_pipe::NamedPipeServer) -> Option<(u32, Value)> {
                let mut header = [0u8; 8];
                server.read_exact(&mut header).await.ok()?;
                let op = u32::from_le_bytes(header[..4].try_into().unwrap());
                let mut body = vec![0u8; u32::from_le_bytes(header[4..].try_into().unwrap()) as usize];
                server.read_exact(&mut body).await.ok()?;
                Some((op, serde_json::from_slice(&body).unwrap()))
            }
            async fn reply(server: &mut tokio::net::windows::named_pipe::NamedPipeServer, value: Value) {
                let body = serde_json::to_vec(&value).unwrap();
                let mut out = OP_FRAME.to_le_bytes().to_vec();
                out.extend_from_slice(&(body.len() as u32).to_le_bytes());
                out.extend_from_slice(&body);
                let _ = server.write_all(&out).await;
            }
            // 1st connection: handshake, then Discord "closes".
            first.connect().await.unwrap();
            let mut second = ServerOptions::new().create(&pipe_name).unwrap();
            frame(&mut first).await.unwrap();
            reply(&mut first, json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "v": 1 } })).await;
            first.disconnect().unwrap();
            drop(first);
            // 2nd connection: works.
            second.connect().await.unwrap();
            while let Some((op, msg)) = frame(&mut second).await {
                let answer = if op == OP_HANDSHAKE {
                    json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "v": 1 } })
                } else {
                    updates_tx.send(msg.clone()).unwrap();
                    json!({ "cmd": "SET_ACTIVITY", "evt": null, "nonce": msg["nonce"], "data": msg["args"]["activity"] })
                };
                reply(&mut second, answer).await;
            }
        });

        let (presence, task) = Presence::with_pipes("1234567890", vec![name]);
        let handle = tokio::spawn(task);
        presence.set(Some(now_playing("Copenhague")));
        let update = tokio::time::timeout(Duration::from_secs(3), updates.recv()).await.expect("no esperó 24 s").unwrap();
        assert_eq!(update["args"]["activity"]["details"], "Copenhague");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(presence.status(), Status::Connected);

        handle.abort();
        fake.abort();
    }
}
