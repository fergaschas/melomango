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

use anyhow::{Context, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::MusicSource;
use crate::models::{SourceInfo, SourceKind, StreamInfo, Track};

pub const PROTOCOL_VERSION: u32 = 1;
const MANIFEST: &str = "plugin.json";
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
/// `connect` waits for the user to log in; `install-requirement` may download software.
const LONG_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Most a plugin may answer: a few thousand tracks are a few MB.
const MAX_OUTPUT: u64 = 32 * 1024 * 1024;
/// Of its error output only the last line is shown.
const MAX_STDERR: u64 = 1024 * 1024;

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
    /// Held while a login (`connect`, up to 10 minutes) waits: only one at a time.
    connecting: tokio::sync::Mutex<()>,
}

/// Login state of a library plugin.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LibraryStatus {
    pub connected: bool,
    #[serde(default)]
    pub user: Option<String>,
}

/// A playlist, album or similar set of tracks offered by a library plugin.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
        let text = fs::read_to_string(&path).with_context(|| format!("no se encontró {}", path.display()))?;
        let manifest: Manifest =
            serde_json::from_str(&text).with_context(|| format!("{} no es válido", path.display()))?;

        validate_id(&manifest.id)?;
        if manifest.protocol != PROTOCOL_VERSION {
            bail!("el plugin usa el protocolo {} y Melomango entiende el {PROTOCOL_VERSION}", manifest.protocol);
        }
        let exe = resolve_executable(dir, &manifest.executable)?;
        Ok(Self { manifest, dir: dir.to_path_buf(), exe, data_dir: None, connecting: tokio::sync::Mutex::new(()) })
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
        let arg = safe_arg(command, arg)?;
        let mut cmd = tokio::process::Command::new(&self.exe);
        cmd.args(&self.manifest.args)
            .arg(command)
            .args(arg)
            .current_dir(&self.dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        if let Some(data_dir) = &self.data_dir {
            fs::create_dir_all(data_dir).ok();
            cmd.env("MELOMANGO_DATA_DIR", data_dir);
        }
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashing

        let name = &self.manifest.name;
        let mut child = cmd.spawn().with_context(|| format!("no se pudo ejecutar el plugin {name}"))?;
        // kill_on_drop only ends the plugin itself: what it started (yt-dlp, a cmd.exe in
        // between) would outlive a timeout. On Windows a job takes the whole tree with it.
        #[cfg(windows)]
        let _tree = child.raw_handle().and_then(job::KillTreeOnDrop::new);
        let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
        let run = async {
            let (out, err) = tokio::try_join!(read_capped(stdout, MAX_OUTPUT), read_capped(stderr, MAX_STDERR))?;
            anyhow::Ok((child.wait().await?, out, err))
        };
        let (status, stdout, stderr) = tokio::time::timeout(timeout, run)
            .await
            .with_context(|| format!("el plugin {name} tardó demasiado"))?
            .with_context(|| format!("el plugin {name} falló"))?;

        if !status.success() {
            let stderr = String::from_utf8_lossy(&stderr);
            let message = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("sin detalles");
            bail!("{name}: {}", message.trim());
        }
        serde_json::from_slice(&stdout).with_context(|| format!("respuesta no válida del plugin {name}"))
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
        // Each login waits up to 10 minutes: pressing again must not start another one.
        let Ok(_waiting) = self.connecting.try_lock() else {
            bail!("ya hay un inicio de sesión de {} esperando: termínalo en el navegador", self.manifest.name);
        };
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
        .filter(|e| e.path().is_dir() && !is_temporary(&e.file_name()))
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

/// Inside an installed plugin's folder: the repository (index URL) it was installed from.
const ORIGIN_FILE: &str = ".melomango-origin";

/// Folders used while installing (`.<id>.new`, `.<id>.old`): never loaded as plugins.
fn is_temporary(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

/// The repository the installed plugin `id` came from. None if unknown: installed from a
/// folder, or by a version of Melomango that didn't record it yet.
pub fn origin(plugins_dir: &Path, id: &str) -> Option<String> {
    validate_id(id).ok()?;
    let text = fs::read_to_string(plugins_dir.join(id).join(ORIGIN_FILE)).ok()?;
    Some(text.trim().to_string()).filter(|t| !t.is_empty())
}

pub fn is_installed(plugins_dir: &Path, id: &str) -> bool {
    validate_id(id).is_ok() && plugins_dir.join(id).join(MANIFEST).is_file()
}

/// Removes what an interrupted install or uninstall left behind. Run at startup, before
/// anything can be installing.
pub fn clean_leftovers(plugins_dir: &Path) {
    let Ok(entries) = fs::read_dir(plugins_dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path().is_dir() && is_temporary(&entry.file_name()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Copies the plugin folder `src` into `plugins_dir/<id>`, replacing an older version, and
/// records `origin` (the repository it came from). All or nothing: the new version is
/// copied and checked aside first, and if swapping it in fails the old one stays.
pub fn install(src: &Path, plugins_dir: &Path, origin: Option<&str>) -> anyhow::Result<String> {
    let plugin = PluginSource::load(src)?;
    let id = plugin.manifest.id;
    let target = plugins_dir.join(&id);
    if src.canonicalize().ok() == target.canonicalize().ok() {
        return Ok(id); // already installed in place
    }
    let new = plugins_dir.join(format!(".{id}.new"));
    let old = plugins_dir.join(format!(".{id}.old"));
    for leftover in [&new, &old] {
        if leftover.exists() {
            fs::remove_dir_all(leftover).with_context(|| format!("no se pudo limpiar {}", leftover.display()))?;
        }
    }

    // 1. The new version, aside, and only if it is usable.
    let staged = (|| {
        copy_dir(src, &new)?;
        if let Some(origin) = origin {
            fs::write(new.join(ORIGIN_FILE), origin).context("no se pudo guardar el origen del plugin")?;
        }
        PluginSource::load(&new)?;
        anyhow::Ok(())
    })();
    if let Err(e) = staged {
        let _ = fs::remove_dir_all(&new);
        return Err(e);
    }

    // 2. Swap. Renaming fails (and changes nothing) if the plugin is running right now.
    let replacing = target.exists();
    if replacing && let Err(e) = fs::rename(&target, &old) {
        let _ = fs::remove_dir_all(&new);
        return Err(e).with_context(|| {
            format!("no se pudo reemplazar {} (¿está en uso? Inténtalo en un momento)", target.display())
        });
    }
    if let Err(e) = fs::rename(&new, &target) {
        if replacing {
            let _ = fs::rename(&old, &target); // put the old version back
        }
        let _ = fs::remove_dir_all(&new);
        return Err(e).with_context(|| format!("no se pudo instalar en {}", target.display()));
    }

    // 3. The old version is no longer used; if it can't be deleted now, at the next start.
    if replacing {
        let _ = fs::remove_dir_all(&old);
    }
    Ok(id)
}

/// Removes the plugin and its data (sessions, settings).
pub fn uninstall(id: &str, plugins_dir: &Path, data_root: &Path) -> anyhow::Result<()> {
    validate_id(id)?;
    let target = plugins_dir.join(id);
    if !target.exists() {
        bail!("el plugin {id} no está instalado");
    }
    // Moved aside first: either it is gone in one step or it stays whole. What can't be
    // deleted now (a file in use) is no longer loaded and goes at the next start.
    let old = plugins_dir.join(format!(".{id}.old"));
    if old.exists() {
        fs::remove_dir_all(&old).with_context(|| format!("no se pudo limpiar {}", old.display()))?;
    }
    fs::rename(&target, &old)
        .with_context(|| format!("no se pudo borrar {} (¿está en uso? Inténtalo en un momento)", target.display()))?;
    let _ = fs::remove_dir_all(&old);
    forget_data(id, data_root);
    Ok(())
}

/// Deletes the plugin's data folder (its sessions, tokens and settings).
pub fn forget_data(id: &str, data_root: &Path) {
    if validate_id(id).is_ok() {
        let _ = fs::remove_dir_all(data_root.join(id));
    }
}

/// What the user typed (a search) or an id goes to the plugin as an argument, and a
/// plugin that hands it to another program (yt-dlp) could take «-x» for an option.
/// Searches lose leading dashes; ids may not start with one.
fn safe_arg<'a>(command: &str, arg: Option<&'a str>) -> anyhow::Result<Option<&'a str>> {
    let Some(arg) = arg else { return Ok(None) };
    if command == "search" {
        return Ok(Some(arg.trim_start_matches(|c: char| c == '-' || c.is_whitespace())));
    }
    if arg.starts_with('-') {
        bail!("id no válido: «{arg}»");
    }
    Ok(Some(arg))
}

/// Everything `reader` gives, up to `max` bytes: more is an error (and the plugin is ended).
async fn read_capped(reader: Option<impl tokio::io::AsyncRead + Unpin>, max: u64) -> anyhow::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut data = Vec::new();
    if let Some(reader) = reader {
        reader.take(max + 1).read_to_end(&mut data).await?;
    }
    if data.len() as u64 > max {
        bail!("respuesta demasiado grande");
    }
    Ok(data)
}

/// Windows job objects: ending a process together with all it started.
#[cfg(windows)]
mod job {
    use std::os::windows::io::RawHandle;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
        },
    };

    /// When dropped, the process given and everything it started are ended.
    pub struct KillTreeOnDrop(RawHandle);

    // The job handle is only closed, from whichever thread drops it.
    unsafe impl Send for KillTreeOnDrop {}
    unsafe impl Sync for KillTreeOnDrop {}

    impl KillTreeOnDrop {
        /// None if the process can't be put in a job (then only it is ended, as before).
        pub fn new(process: RawHandle) -> Option<Self> {
            // SAFETY: plain Win32 calls on a handle we own; the struct is zeroed as the API expects.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let guard = Self(job);
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let set = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                (set != 0 && AssignProcessToJobObject(job, process) != 0).then_some(guard)
            }
        }
    }

    impl Drop for KillTreeOnDrop {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateJobObjectW and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

pub(crate) fn validate_id(id: &str) -> anyhow::Result<()> {
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
            fs::copy(entry.path(), &to).with_context(|| format!("no se pudo copiar {}", entry.path().display()))?;
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

        assert_eq!(install(&src, &plugins, None).unwrap(), "demo");
        let found = discover(&plugins, &root.join("data"));
        assert_eq!(found.len(), 1);
        let info = found[0].info();
        assert_eq!(info.id, "demo");
        assert!(info.supports_trending);

        fs::create_dir_all(root.join("data/demo")).unwrap();
        uninstall("demo", &plugins, &root.join("data")).unwrap();
        assert!(discover(&plugins, &root.join("data")).is_empty());
        assert!(!root.join("data/demo").exists(), "its data goes too");
        assert_eq!(fs::read_dir(&plugins).unwrap().count(), 0, "nothing left behind");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn updating_swaps_the_whole_folder_and_records_the_origin() {
        let root = temp_dir("update");
        let plugins = root.join("plugins");
        let manifest = |version: &str| {
            format!(r#"{{"id":"demo","name":"Demo","version":"{version}","protocol":1,"executable":"bin"}}"#)
        };
        write_plugin(&root.join("v1"), &manifest("1"));
        fs::write(root.join("v1/only-in-v1.txt"), "x").unwrap();
        write_plugin(&root.join("v2"), &manifest("2"));

        install(&root.join("v1"), &plugins, Some("https://a/index.json")).unwrap();
        assert_eq!(origin(&plugins, "demo").as_deref(), Some("https://a/index.json"));
        install(&root.join("v2"), &plugins, Some("https://b/index.json")).unwrap();
        assert_eq!(origin(&plugins, "demo").as_deref(), Some("https://b/index.json"));
        assert!(!plugins.join("demo/only-in-v1.txt").exists(), "no files from the old version");
        let names: Vec<_> = fs::read_dir(&plugins).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(names, ["demo"], "no temporary folders left");

        // Without an origin (installed from a folder) none is reported.
        install(&root.join("v1"), &plugins, None).unwrap();
        assert_eq!(origin(&plugins, "demo"), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_broken_package_leaves_the_installed_version_alone() {
        let root = temp_dir("broken");
        let plugins = root.join("plugins");
        write_plugin(
            &root.join("good"),
            r#"{"id":"demo","name":"Demo","version":"1","protocol":1,"executable":"bin"}"#,
        );
        install(&root.join("good"), &plugins, None).unwrap();
        write_plugin(
            &root.join("bad"),
            r#"{"id":"demo","name":"Demo","version":"2","protocol":1,"executable":"nope"}"#,
        );
        assert!(install(&root.join("bad"), &plugins, None).is_err());
        assert_eq!(discover(&plugins, &root.join("data"))[0].manifest.version, "1");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn interrupted_installs_are_ignored_and_cleaned() {
        let root = temp_dir("leftovers");
        let plugins = root.join("plugins");
        let manifest = r#"{"id":"demo","name":"Demo","version":"1","protocol":1,"executable":"bin"}"#;
        write_plugin(&plugins.join(".demo.new"), manifest);
        write_plugin(&plugins.join(".demo.old"), manifest);
        assert!(discover(&plugins, &root.join("data")).is_empty());
        clean_leftovers(&plugins);
        assert_eq!(fs::read_dir(&plugins).unwrap().count(), 0);
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
    fn user_text_never_reaches_a_plugin_as_an_option() {
        assert_eq!(safe_arg("search", Some("--exec rm")).unwrap(), Some("exec rm"));
        assert_eq!(safe_arg("search", Some(" - Shakira")).unwrap(), Some("Shakira"));
        assert_eq!(safe_arg("search", Some("AC/DC - Back")).unwrap(), Some("AC/DC - Back"));
        assert!(safe_arg("stream", Some("-o/tmp/x")).is_err());
        assert_eq!(safe_arg("stream", Some("dQw4w9WgXcQ")).unwrap(), Some("dQw4w9WgXcQ"));
        assert_eq!(safe_arg("trending", None).unwrap(), None);
    }

    #[tokio::test]
    async fn output_beyond_the_limit_is_refused() {
        let endless = tokio::io::repeat(b'x');
        assert!(read_capped(Some(endless), 1024).await.is_err());
        let small: &[u8] = b"{}";
        assert_eq!(read_capped(Some(small), 1024).await.unwrap(), b"{}");
    }

    /// A process started by the plugin (here, ping started by cmd) ends with it.
    #[cfg(windows)]
    #[tokio::test]
    async fn the_whole_process_tree_ends_with_the_job() {
        let count = || {
            let out =
                std::process::Command::new("tasklist").args(["/FI", "IMAGENAME eq PING.EXE", "/NH"]).output().unwrap();
            String::from_utf8_lossy(&out.stdout).matches("PING.EXE").count()
        };
        let before = count();
        let mut child = tokio::process::Command::new("cmd")
            .args(["/C", "ping -n 30 127.0.0.1 >NUL"])
            .creation_flags(0x0800_0000)
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let tree = job::KillTreeOnDrop::new(child.raw_handle().unwrap()).expect("job");
        tokio::time::sleep(Duration::from_millis(800)).await;
        assert_eq!(count(), before + 1, "ping is running");
        drop(tree);
        let _ = child.wait().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(count(), before, "ping ended with the job");
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
