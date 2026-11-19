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

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{bail, Context};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize};

use super::MusicSource;
use crate::models::{SourceInfo, SourceKind, StreamInfo, Track};

pub const PROTOCOL_VERSION: u32 = 1;
const MANIFEST: &str = "plugin.json";
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
/// Ids taken by built-in sources.
const RESERVED_IDS: &[&str] = &["audius"];

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
        if RESERVED_IDS.contains(&manifest.id.as_str()) {
            bail!("el id \"{}\" está reservado", manifest.id);
        }
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
        })
    }

    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    async fn call<T: DeserializeOwned>(&self, command: &str, arg: Option<&str>) -> anyhow::Result<T> {
        let mut cmd = tokio::process::Command::new(&self.exe);
        cmd.args(&self.manifest.args)
            .arg(command)
            .args(arg)
            .current_dir(&self.dir)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashing

        let output = tokio::time::timeout(CALL_TIMEOUT, cmd.output())
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
            supports_trending: self.manifest.capabilities.iter().any(|c| c == "trending"),
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

    async fn stream(&self, track_id: &str) -> anyhow::Result<StreamInfo> {
        self.call("stream", Some(track_id)).await
    }
}

/// Loads every plugin under `plugins_dir`, skipping (and reporting) broken ones.
pub fn discover(plugins_dir: &Path) -> Vec<PluginSource> {
    let Ok(entries) = fs::read_dir(plugins_dir) else {
        return Vec::new();
    };
    let mut plugins: Vec<PluginSource> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| match PluginSource::load(&e.path()) {
            Ok(p) => Some(p),
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

pub fn uninstall(id: &str, plugins_dir: &Path) -> anyhow::Result<()> {
    validate_id(id)?;
    let target = plugins_dir.join(id);
    if !target.exists() {
        bail!("el plugin {id} no está instalado");
    }
    fs::remove_dir_all(&target).with_context(|| format!("no se pudo borrar {}", target.display()))
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
        let found = discover(&plugins);
        assert_eq!(found.len(), 1);
        let info = found[0].info();
        assert_eq!(info.id, "demo");
        assert!(info.supports_trending);

        uninstall("demo", &plugins).unwrap();
        assert!(discover(&plugins).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_bad_manifests() {
        let root = temp_dir("reject");
        let cases = [
            (r#"{"id":"audius","name":"X","version":"1","protocol":1,"executable":"bin"}"#, "reservado"),
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
