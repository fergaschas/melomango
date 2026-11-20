//! Installable plugins: external programs that speak the protocol in `plugins/PROTOCOL.md`.
//!
//! A plugin is a folder with a `plugin.json` manifest and an executable. Installing copies
//! the folder into `<app data>/plugins/<id>/`. Each call runs the executable once:
//!
//! ```text
//! <executable> <args...> search <query>   -> {"tracks": [Track, ...]}
//! <executable> <args...> trending         -> {"tracks": [Track, ...]}
//! <executable> <args...> stream <id>      -> {"url": "...", "headers": {...}}
//! ```
//!
//! Plugins with the "library" capability provide metadata only (e.g. Spotify playlists):
//!
//! ```text
//! <executable> status | connect | disconnect     -> {"connected": bool, "user": "..."}
//! <executable> collections                       -> {"collections": [Collection, ...]}
//! <executable> tracks <collection id>            -> {"tracks": [Track, ...]}
//! ```
//!
//! Plugins with the "requirements" capability can install what they need (e.g. yt-dlp):
//!
//! ```text
//! <executable> requirements               -> {"requirements": [Requirement, ...]}
//! <executable> install-requirement <id>   -> {"requirements": [Requirement, ...]}
//! ```
//!
//! Every call gets `MELOMANGO_DATA_DIR`: a folder of its own that survives plugin updates.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{bail, Context};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::MusicSource;
use crate::models::{SourceInfo, SourceKind, StreamInfo, Track};

pub const PROTOCOL_VERSION: u32 = 1;
const MANIFEST: &str = "plugin.json";
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
/// `connect` waits for the user to log in; `install-requirement` may download software.
const LONG_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub protocol: u32,
    #[serde(default)]
    pub description: Option<String>,
    /// Path relative to the plugin folder. On Windows ".exe" is tried when missing.
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

pub struct PluginSource {
    manifest: Manifest,
    dir: PathBuf,
    exe: PathBuf,
    data_dir: Option<PathBuf>,
}

/// Login state of a library plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryStatus {
    pub connected: bool,
    #[serde(default)]
    pub user: Option<String>,
}

/// A playlist, album or similar set of tracks offered by a library plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    /// "liked", "playlist", "album"...
    pub kind: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub track_count: u32,
    #[serde(default)]
    pub artwork_url: Option<String>,
    /// False when the service won't hand out its tracks (e.g. playlists you only follow).
    #[serde(default = "yes")]
    pub importable: bool,
}

fn yes() -> bool {
    true
}

/// Something a plugin needs installed on the computer (e.g. yt-dlp).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub installed: bool,
    /// The plugin can install it by itself (e.g. with winget).
    #[serde(default)]
    pub installable: bool,
}

impl PluginSource {
    /// Reads and validates the plugin in `dir`.
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(MANIFEST);
        let text = fs::read_to_string(&path)
            .with_context(|| format!("no se encontró {}", path.display()))?;
        let manifest: Manifest =
            serde_json::from_str(&text).with_context(|| format!("{} no es válido", path.display()))?;

        validate_id(&manifest.id)?;
        if manifest.protocol != PROTOCOL_VERSION {
            bail!(
                "el plugin usa el protocolo {} y Melomango entiende el {PROTOCOL_VERSION}",
                manifest.protocol
            );
        }
        let exe = resolve_executable(dir, &manifest.executable)?;
        Ok(Self {
            manifest,
            dir: dir.to_path_buf(),
            exe,
            data_dir: None,
        })
    }

    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    fn has(&self, capability: &str) -> bool {
        self.manifest.capabilities.iter().any(|c| c == capability)
    }

    /// Plays music (search + stream). Metadata-only library plugins are not sources.
    pub fn is_source(&self) -> bool {
        !self.has("library") || self.has("search")
    }

    pub fn is_library(&self) -> bool {
        self.has("library")
    }

    async fn call<T: DeserializeOwned>(&self, command: &str, arg: Option<&str>) -> anyhow::Result<T> {
        let timeout = match command {
            "connect" | "install-requirement" => LONG_TIMEOUT,
            _ => CALL_TIMEOUT,
        };
        let mut cmd = tokio::process::Command::new(&self.exe);
        cmd.args(&self.manifest.args)
            .arg(command)
            .args(arg)
            .current_dir(&self.dir)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        if let Some(data_dir) = &self.data_dir {
            fs::create_dir_all(data_dir).ok();
            cmd.env("MELOMANGO_DATA_DIR", data_dir);
        }
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashing

        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .with_context(|| format!("el plugin {} tardó demasiado", self.manifest.name))?
            .with_context(|| format!("no se pudo ejecutar el plugin {}", self.manifest.name))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let message = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("sin detalles");
            bail!("{}: {}", self.manifest.name, message.trim());
        }
        serde_json::from_slice(&output.stdout)
            .with_context(|| format!("respuesta no válida del plugin {}", self.manifest.name))
    }

    async fn tracks(&self, command: &str, arg: Option<&str>) -> anyhow::Result<Vec<Track>> {
        #[derive(Deserialize)]
        struct TrackList {
            tracks: Vec<Track>,
        }
        let list: TrackList = self.call(command, arg).await?;
        Ok(list
            .tracks
            .into_iter()
            .map(|mut t| {
                t.source = self.manifest.id.clone();
                t
            })
            .collect())
    }

    /// Name and songs of one of the source's playlists (capability "playlist"), if it can.
    pub async fn playlist(&self, id: &str) -> anyhow::Result<Option<(String, Vec<Track>)>> {
        if !self.has("playlist") {
            return Ok(None);
        }
        #[derive(Deserialize)]
        struct List {
            #[serde(default)]
            title: String,
            tracks: Vec<Track>,
        }
        let list: List = self.call("playlist", Some(id)).await?;
        let tracks = list
            .tracks
            .into_iter()
            .map(|mut t| {
                t.source = self.manifest.id.clone();
                t
            })
            .collect();
        Ok(Some((list.title, tracks)))
    }

    // ---------- Requirements ----------

    pub async fn requirements(&self) -> anyhow::Result<Vec<Requirement>> {
        if !self.has("requirements") {
            return Ok(Vec::new());
        }
        #[derive(Deserialize)]
        struct List {
            requirements: Vec<Requirement>,
        }
        Ok(self.call::<List>("requirements", None).await?.requirements)
    }

    pub async fn install_requirement(&self, id: &str) -> anyhow::Result<Vec<Requirement>> {
        #[derive(Deserialize)]
        struct List {
            requirements: Vec<Requirement>,
        }
        Ok(self.call::<List>("install-requirement", Some(id)).await?.requirements)
    }

    // ---------- Library plugins ----------

    pub async fn library_status(&self) -> anyhow::Result<LibraryStatus> {
        self.call("status", None).await
    }

    /// Opens the service's login in the browser and waits for it to finish.
    pub async fn connect(&self) -> anyhow::Result<LibraryStatus> {
        self.call("connect", None).await
    }

    pub async fn disconnect(&self) -> anyhow::Result<LibraryStatus> {
        self.call("disconnect", None).await
    }

    pub async fn collections(&self) -> anyhow::Result<Vec<Collection>> {
        #[derive(Deserialize)]
        struct List {
            collections: Vec<Collection>,
        }
        Ok(self.call::<List>("collections", None).await?.collections)
    }

    pub async fn collection_tracks(&self, collection_id: &str) -> anyhow::Result<Vec<Track>> {
        self.tracks("tracks", Some(collection_id)).await
    }
}

#[async_trait]
impl MusicSource for PluginSource {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            id: self.manifest.id.clone(),
            name: self.manifest.name.clone(),
            kind: SourceKind::Plugin,
            version: Some(self.manifest.version.clone()),
            description: self.manifest.description.clone(),
            supports_trending: self.has("trending"),
            is_library: self.is_library(),
        }
    }

    async fn search(&self, query: &str) -> anyhow::Result<Vec<Track>> {
        self.tracks("search", Some(query)).await
    }

    async fn trending(&self) -> anyhow::Result<Vec<Track>> {
        if !self.info().supports_trending {
            return Ok(Vec::new());
        }
        self.tracks("trending", None).await
    }

    async fn radio(&self, track_id: &str) -> anyhow::Result<Option<Vec<Track>>> {
        if !self.has("radio") {
            return Ok(None);
        }
        self.tracks("radio", Some(track_id)).await.map(Some)
    }

    async fn stream(&self, track_id: &str) -> anyhow::Result<StreamInfo> {
        self.call("stream", Some(track_id)).await
    }
}

/// Loads every plugin under `plugins_dir`, skipping (and reporting) broken ones.
/// Each plugin gets `data_root/<id>` as its data folder.
pub fn discover(plugins_dir: &Path, data_root: &Path) -> Vec<PluginSource> {
    let Ok(entries) = fs::read_dir(plugins_dir) else {
        return Vec::new();
    };
    let mut plugins: Vec<PluginSource> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| match PluginSource::load(&e.path()) {
            Ok(mut p) => {
                p.data_dir = Some(data_root.join(p.id()));
                Some(p)
            }
            Err(err) => {
                eprintln!("[plugins] ignorando {}: {err:#}", e.path().display());
                None
            }
        })
        .collect();
    plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    plugins
}

/// Copies the plugin folder `src` into `plugins_dir/<id>`, replacing an older version.
pub fn install(src: &Path, plugins_dir: &Path) -> anyhow::Result<String> {
    let plugin = PluginSource::load(src)?;
    let target = plugins_dir.join(plugin.id());
    if src.canonicalize().ok() == target.canonicalize().ok() {
        return Ok(plugin.manifest.id); // already installed in place
    }
    if target.exists() {
        fs::remove_dir_all(&target)
            .with_context(|| format!("no se pudo reemplazar {}", target.display()))?;
    }
    copy_dir(src, &target)?;
    // Make sure the copy is usable before reporting success.
    PluginSource::load(&target)?;
    Ok(plugin.manifest.id)
}

/// Removes the plugin and its data (sessions, settings).
pub fn uninstall(id: &str, plugins_dir: &Path, data_root: &Path) -> anyhow::Result<()> {
    validate_id(id)?;
    let target = plugins_dir.join(id);
    if !target.exists() {
        bail!("el plugin {id} no está instalado");
    }
    fs::remove_dir_all(&target).with_context(|| format!("no se pudo borrar {}", target.display()))?;
    let data = data_root.join(id);
    if data.exists() {
        fs::remove_dir_all(&data).ok();
    }
    Ok(())
}

fn validate_id(id: &str) -> anyhow::Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 40
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok {
        bail!("id de plugin no válido: \"{id}\" (usa minúsculas, números, - y _)");
    }
    Ok(())
}

fn resolve_executable(dir: &Path, executable: &str) -> anyhow::Result<PathBuf> {
    let relative = Path::new(executable);
    if relative.is_absolute() || relative.components().any(|c| c == std::path::Component::ParentDir) {
        bail!("el ejecutable debe estar dentro de la carpeta del plugin");
    }
    let path = dir.join(relative);
    if path.is_file() {
        return Ok(path);
    }
    if cfg!(windows) && path.extension().is_none() {
        let exe = path.with_extension("exe");
        if exe.is_file() {
            return Ok(exe);
        }
    }
    bail!("no se encontró el ejecutable {}", path.display())
}

fn copy_dir(src: &Path, dst: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)
                .with_context(|| format!("no se pudo copiar {}", entry.path().display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plugin(dir: &Path, manifest: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(MANIFEST), manifest).unwrap();
        fs::write(dir.join(if cfg!(windows) { "bin.exe" } else { "bin" }), b"").unwrap();
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("melomango-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn installs_lists_and_uninstalls() {
        let root = temp_dir("install");
        let src = root.join("src");
        let plugins = root.join("plugins");
        write_plugin(
            &src,
            r#"{"id":"demo","name":"Demo","version":"1.0.0","protocol":1,"executable":"bin","capabilities":["trending"]}"#,
        );

        assert_eq!(install(&src, &plugins).unwrap(), "demo");
        let found = discover(&plugins, &root.join("data"));
        assert_eq!(found.len(), 1);
        let info = found[0].info();
        assert_eq!(info.id, "demo");
        assert!(info.supports_trending);

        uninstall("demo", &plugins, &root.join("data")).unwrap();
        assert!(discover(&plugins, &root.join("data")).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    /// A fake plugin (a .cmd script) that needs "yt-dlp" and can install it.
    #[cfg(windows)]
    #[tokio::test]
    async fn reads_and_installs_requirements() {
        let root = temp_dir("reqs");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(MANIFEST),
            r#"{"id":"demo","name":"Demo","version":"1","protocol":1,"executable":"bin.cmd","capabilities":["search","requirements"]}"#,
        )
        .unwrap();
        fs::write(
            root.join("bin.cmd"),
            "@echo off\r\n\
             if \"%1\"==\"requirements\" echo {\"requirements\":[{\"id\":\"yt-dlp\",\"name\":\"yt-dlp\",\"installed\":false,\"installable\":true}]}\r\n\
             if \"%1\"==\"install-requirement\" echo {\"requirements\":[{\"id\":\"%2\",\"name\":\"yt-dlp\",\"installed\":true,\"installable\":true}]}\r\n",
        )
        .unwrap();

        let plugin = PluginSource::load(&root).unwrap();
        let before = plugin.requirements().await.unwrap();
        assert_eq!(before[0].id, "yt-dlp");
        assert!(!before[0].installed && before[0].installable);
        let after = plugin.install_requirement("yt-dlp").await.unwrap();
        assert!(after[0].installed);

        // Without the capability the plugin is not even asked.
        let manifest = fs::read_to_string(root.join(MANIFEST)).unwrap().replace(",\"requirements\"", "");
        fs::write(root.join(MANIFEST), manifest).unwrap();
        assert!(PluginSource::load(&root).unwrap().requirements().await.unwrap().is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_bad_manifests() {
        let root = temp_dir("reject");
        let cases = [
            (r#"{"id":"Bad Id","name":"X","version":"1","protocol":1,"executable":"bin"}"#, "no válido"),
            (r#"{"id":"x","name":"X","version":"1","protocol":99,"executable":"bin"}"#, "protocolo"),
            (r#"{"id":"x","name":"X","version":"1","protocol":1,"executable":"../bin"}"#, "dentro"),
            (r#"{"id":"x","name":"X","version":"1","protocol":1,"executable":"nope"}"#, "no se encontró"),
        ];
        for (i, (manifest, expected)) in cases.iter().enumerate() {
            let dir = root.join(i.to_string());
            write_plugin(&dir, manifest);
            let err = PluginSource::load(&dir).err().expect(manifest).to_string();
            assert!(err.contains(expected), "{manifest}: {err}");
        }
        let _ = fs::remove_dir_all(&root);
    }
}
