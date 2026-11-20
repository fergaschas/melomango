//! Plugin catalogue: remote indexes ("repositories") listing plugins that can be
//! downloaded and installed from the app.
//!
//! An index is a JSON file served over HTTPS (format in `plugins/PROTOCOL.md`). The
//! official one lives in the melomango-plugins GitHub repo; users can add more by URL.
//! Downloads are zip files verified against the SHA-256 in the index before installing.

use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::sources::plugin;

pub const OFFICIAL_INDEX: &str = "https://raw.githubusercontent.com/fergaschas/melomango-plugins/main/index.json";
const INDEX_VERSION: u32 = 1;
const MAX_DOWNLOAD_BYTES: usize = 200 * 1024 * 1024;
/// An index lists plugins: even a big one is a few hundred KB.
const MAX_INDEX_BYTES: usize = 5 * 1024 * 1024;

/// The client for repositories and plugin downloads. Only https, redirects included: an
/// index (or zip) sent through http could be swapped on the way, SHA-256 and all, and a
/// plugin runs code on this computer. The shared client follows https → http redirects.
pub fn client(user_agent: &str) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(user_agent)
        .https_only(true)
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(std::time::Duration::from_secs(30))
        .build()
}

#[derive(Debug, Deserialize)]
struct Index {
    version: u32,
    #[serde(default)]
    name: Option<String>,
    plugins: Vec<IndexEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexEntry {
    id: String,
    name: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    /// Keyed by platform, e.g. "windows-x86_64" (see `platform()`).
    #[serde(default)]
    downloads: HashMap<String, Download>,
}

#[derive(Debug, Clone, Deserialize)]
struct Download {
    url: String,
    sha256: String,
}

/// A plugin as shown in the in-app browser.
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub tags: Vec<String>,
    pub registry: String,
    pub registry_name: String,
    /// There is a build for this OS/architecture.
    pub available: bool,
}

#[derive(Debug, Serialize, specta::Type)]
pub struct Catalog {
    pub plugins: Vec<CatalogPlugin>,
    /// One message per repository that could not be read.
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, specta::Type)]
pub struct RegistryInfo {
    pub url: String,
    pub official: bool,
}

/// "windows-x86_64", "macos-aarch64", "linux-x86_64"...
pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

// ---------- Repositories (persisted list of extra index URLs) ----------

pub struct Registries {
    file: PathBuf,
}

#[derive(Default, Serialize, Deserialize, specta::Type)]
struct RegistriesFile {
    extra: Vec<String>,
}

impl Registries {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    fn extra(&self) -> Vec<String> {
        fs::read_to_string(&self.file)
            .ok()
            .and_then(|t| serde_json::from_str::<RegistriesFile>(&t).ok())
            .map(|f| f.extra)
            .unwrap_or_default()
    }

    fn save(&self, extra: Vec<String>) -> anyhow::Result<()> {
        let text = serde_json::to_string_pretty(&RegistriesFile { extra })?;
        fs::write(&self.file, text).with_context(|| format!("no se pudo guardar {}", self.file.display()))
    }

    pub fn list(&self) -> Vec<RegistryInfo> {
        std::iter::once(RegistryInfo { url: OFFICIAL_INDEX.into(), official: true })
            .chain(self.extra().into_iter().map(|url| RegistryInfo { url, official: false }))
            .collect()
    }

    pub fn add(&self, url: &str) -> anyhow::Result<()> {
        let url = url.trim();
        require_https(url)?;
        let mut extra = self.extra();
        if url == OFFICIAL_INDEX || extra.iter().any(|u| u == url) {
            bail!("ese repositorio ya está añadido");
        }
        extra.push(url.to_string());
        self.save(extra)
    }

    pub fn remove(&self, url: &str) -> anyhow::Result<()> {
        if url == OFFICIAL_INDEX {
            bail!("el repositorio oficial no se puede quitar");
        }
        let extra: Vec<String> = self.extra().into_iter().filter(|u| u != url).collect();
        self.save(extra)
    }
}

// ---------- Browsing ----------

async fn fetch_index(http: &reqwest::Client, url: &str) -> anyhow::Result<Index> {
    require_https(url)?;
    let response = http.get(url).send().await?.error_for_status()?;
    let bytes = read_limited(response, MAX_INDEX_BYTES, "el índice es demasiado grande").await?;
    let mut index: Index = serde_json::from_slice(&bytes).context("el índice no tiene un formato válido")?;
    // Ids end up in folder names: one like "../x" is not a plugin anyone should install.
    index.plugins.retain(|p| plugin::validate_id(&p.id).is_ok());
    if index.version != INDEX_VERSION {
        bail!("versión de índice {} no soportada", index.version);
    }
    Ok(index)
}

pub async fn browse(http: &reqwest::Client, registries: &[RegistryInfo]) -> Catalog {
    let platform = platform();
    let mut catalog = Catalog { plugins: Vec::new(), errors: Vec::new() };
    for registry in registries {
        match fetch_index(http, &registry.url).await {
            Ok(index) => {
                let registry_name = index.name.clone().unwrap_or_else(|| registry.url.clone());
                catalog.plugins.extend(index.plugins.into_iter().map(|p| CatalogPlugin {
                    available: p.downloads.contains_key(&platform),
                    id: p.id,
                    name: p.name,
                    version: p.version,
                    description: p.description,
                    author: p.author,
                    homepage: p.homepage,
                    tags: p.tags,
                    registry: registry.url.clone(),
                    registry_name: registry_name.clone(),
                }));
            }
            Err(e) => catalog.errors.push(format!("{}: {e:#}", registry.url)),
        }
    }
    catalog
}

// ---------- Installing ----------

/// Where the installed plugin `id` came from, if installing it from `registry_url` would
/// change that (another repository's plugin with the same id). It would replace the
/// installed one and, since data folders go by id, inherit its sessions and tokens.
/// Plugins installed before origins were recorded count as official: they came from there.
pub fn origin_change(plugins_dir: &Path, registry_url: &str, id: &str) -> Option<String> {
    if !plugin::is_installed(plugins_dir, id) {
        return None;
    }
    let installed_from = plugin::origin(plugins_dir, id).unwrap_or_else(|| OFFICIAL_INDEX.to_string());
    (installed_from != registry_url).then_some(installed_from)
}

/// Downloads `id` from the index at `registry_url`, verifies it and installs it.
/// Returns the installed plugin id.
///
/// Replacing a plugin that came from another repository needs `replace_other_origin`
/// (the user agreed), and then the old one's data (sessions, tokens) is deleted.
pub async fn install(
    http: &reqwest::Client,
    registry_url: &str,
    id: &str,
    plugins_dir: &Path,
    data_root: &Path,
    replace_other_origin: bool,
) -> anyhow::Result<String> {
    plugin::validate_id(id)?;
    let other_origin = origin_change(plugins_dir, registry_url, id);
    if let Some(from) = &other_origin
        && !replace_other_origin
    {
        bail!("ya tienes instalado un plugin \"{id}\" de otro repositorio ({from})");
    }
    let index = fetch_index(http, registry_url).await?;
    let entry = index
        .plugins
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| anyhow!("el repositorio ya no ofrece el plugin {id}"))?;
    let download = entry
        .downloads
        .get(&platform())
        .ok_or_else(|| anyhow!("{} no está disponible para {}", entry.name, platform()))?;
    require_https(&download.url)?;

    let bytes = download_limited(http, &download.url).await?;
    verify_sha256(&bytes, &download.sha256)?;

    let staging = std::env::temp_dir().join(format!(
        "melomango-plugin-{id}-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
    ));
    // Unzipping and copying block: on the pool for blocking work, not the runtime's threads.
    let (expected, plugins_dir, data_root, origin) =
        (entry.id, plugins_dir.to_path_buf(), data_root.to_path_buf(), registry_url.to_string());
    tokio::task::spawn_blocking(move || {
        let result = (|| {
            zip::ZipArchive::new(Cursor::new(bytes))
                .context("la descarga no es un zip válido")?
                .extract(&staging)
                .context("no se pudo descomprimir el plugin")?;
            let root = plugin_root(&staging)?;
            // Checked before touching anything installed: a package claiming another id
            // ("youtube" inside "cool-eq") must not replace that plugin.
            let packaged = plugin::PluginSource::load(&root)?.id().to_string();
            if packaged != expected {
                bail!("el paquete contiene el plugin \"{packaged}\", no \"{expected}\"");
            }
            plugin::install(&root, &plugins_dir, Some(&origin))
        })();
        let _ = fs::remove_dir_all(&staging);
        if let (Ok(installed), Some(_)) = (&result, &other_origin) {
            // A different plugin now: it starts without the previous one's sessions and tokens.
            plugin::forget_data(installed, &data_root);
        }
        result
    })
    .await?
}

async fn download_limited(http: &reqwest::Client, url: &str) -> anyhow::Result<Vec<u8>> {
    let response = http.get(url).send().await?.error_for_status()?;
    read_limited(response, MAX_DOWNLOAD_BYTES, "el plugin es demasiado grande").await
}

/// The body of `response`, failing with `too_big` past `max` bytes (without reading on).
async fn read_limited(mut response: reqwest::Response, max: usize, too_big: &str) -> anyhow::Result<Vec<u8>> {
    if response.content_length().is_some_and(|len| len as usize > max) {
        bail!("{too_big}");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > max {
            bail!("{too_big}");
        }
    }
    Ok(bytes)
}

fn verify_sha256(bytes: &[u8], expected: &str) -> anyhow::Result<()> {
    let actual: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    if !actual.eq_ignore_ascii_case(expected.trim()) {
        bail!("la descarga no coincide con la huella SHA-256 del índice (¿archivo alterado?)");
    }
    Ok(())
}

/// The folder with `plugin.json`: the zip root, or its single top-level folder.
fn plugin_root(dir: &Path) -> anyhow::Result<PathBuf> {
    if dir.join("plugin.json").is_file() {
        return Ok(dir.to_path_buf());
    }
    let subdirs: Vec<PathBuf> = fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    match subdirs.as_slice() {
        [only] if only.join("plugin.json").is_file() => Ok(only.clone()),
        _ => bail!("el paquete no contiene plugin.json"),
    }
}

fn require_https(url: &str) -> anyhow::Result<()> {
    if !url.starts_with("https://") {
        bail!("solo se aceptan direcciones https://");
    }
    Ok(())
}

/// The YouTube plugin installed from the official repository into a temporary folder,
/// for tests against the real service. Returns the source and the folder to delete.
#[cfg(test)]
pub async fn youtube_for_tests(
    http: &reqwest::Client,
) -> (std::sync::Arc<dyn crate::sources::MusicSource>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "melomango-yt-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    install(http, OFFICIAL_INDEX, "youtube", &dir, &dir.join("data"), false).await.unwrap();
    let source = crate::sources::plugin::discover(&dir, &dir.join("data")).pop().expect("plugin de YouTube");
    (std::sync::Arc::new(source), dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_checked() {
        // sha256("abc")
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify_sha256(b"abc", abc).is_ok());
        assert!(verify_sha256(b"abc", &abc.to_uppercase()).is_ok());
        assert!(verify_sha256(b"abd", abc).is_err());
    }

    #[test]
    fn parses_an_index() {
        let index: Index = serde_json::from_str(
            r#"{"version":1,"name":"Oficial","plugins":[{"id":"youtube","name":"YouTube","version":"0.1.0",
                "tags":["video"],"downloads":{"windows-x86_64":{"url":"https://x/y.zip","sha256":"ab"}}}]}"#,
        )
        .unwrap();
        assert_eq!(index.plugins[0].downloads["windows-x86_64"].url, "https://x/y.zip");
    }

    #[test]
    fn registries_add_and_remove() {
        let file = std::env::temp_dir().join(format!("melomango-registries-{}.json", std::process::id()));
        let _ = fs::remove_file(&file);
        let regs = Registries::new(file.clone());
        assert_eq!(regs.list().len(), 1);
        assert!(regs.add("http://insecure.example/index.json").is_err());
        regs.add("https://example.com/index.json").unwrap();
        assert!(regs.add("https://example.com/index.json").is_err());
        assert_eq!(regs.list().len(), 2);
        assert!(regs.remove(OFFICIAL_INDEX).is_err());
        regs.remove("https://example.com/index.json").unwrap();
        assert_eq!(regs.list().len(), 1);
        let _ = fs::remove_file(&file);
    }

    #[test]
    fn notices_a_plugin_from_another_repository() {
        let dir = std::env::temp_dir().join(format!("melomango-origin-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let plugins = dir.join("plugins");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("src/plugin.json"),
            r#"{"id":"spotify","name":"Spotify","version":"1","protocol":1,"executable":"plugin.json"}"#,
        )
        .unwrap();
        let third_party = "https://example.com/index.json";

        assert_eq!(origin_change(&plugins, third_party, "spotify"), None, "not installed yet");
        // Installed before origins were recorded: it came from the official repository.
        plugin::install(&dir.join("src"), &plugins, None).unwrap();
        assert_eq!(origin_change(&plugins, OFFICIAL_INDEX, "spotify"), None);
        assert_eq!(origin_change(&plugins, third_party, "spotify").as_deref(), Some(OFFICIAL_INDEX));

        plugin::install(&dir.join("src"), &plugins, Some(third_party)).unwrap();
        assert_eq!(origin_change(&plugins, third_party, "spotify"), None, "updates from the same place");
        assert_eq!(origin_change(&plugins, OFFICIAL_INDEX, "spotify").as_deref(), Some(third_party));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn the_catalog_client_refuses_plain_http() {
        let http = client("Melomango-tests").unwrap();
        let err = http.get("http://127.0.0.1:9/index.json").send().await.unwrap_err();
        assert!(err.is_builder() || err.to_string().contains("scheme"), "{err}");
    }

    #[tokio::test]
    async fn refuses_a_bad_id_before_touching_anything() {
        let dir = std::env::temp_dir().join(format!("melomango-badid-{}", std::process::id()));
        let err = install(&reqwest::Client::new(), OFFICIAL_INDEX, "../evil", &dir, &dir, false).await.unwrap_err();
        assert!(err.to_string().contains("no válido"), "{err}");
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn bodies_beyond_the_limit_are_refused() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket.read(&mut [0; 1024]).await;
                let body = vec![b'x'; 4096];
                let head = "HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n";
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(format!("{:x}\r\n", body.len()).as_bytes()).await;
                let _ = socket.write_all(&body).await;
                let _ = socket.write_all(b"\r\n0\r\n\r\n").await;
            }
        });
        let response = reqwest::get(&url).await.unwrap();
        let err = read_limited(response, 1024, "demasiado grande").await.unwrap_err();
        assert!(err.to_string().contains("demasiado grande"));
        let response = reqwest::get(&url).await.unwrap();
        assert_eq!(read_limited(response, 8192, "x").await.unwrap().len(), 4096);
    }

    /// The official repository and GitHub's download redirects work with the https-only
    /// client the app uses: `cargo test -- --ignored https_only`
    #[tokio::test]
    #[ignore = "necesita red"]
    async fn installs_from_the_official_repository_with_the_https_only_client() {
        let http = client("Melomango-tests").unwrap();
        let dir = std::env::temp_dir().join(format!("melomango-https-only-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(install(&http, OFFICIAL_INDEX, "youtube", &dir, &dir.join("data"), false).await.unwrap(), "youtube");
        assert!(dir.join("youtube/plugin.json").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_plugin_root_in_subfolder() {
        let dir = std::env::temp_dir().join(format!("melomango-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("youtube")).unwrap();
        fs::write(dir.join("youtube/plugin.json"), "{}").unwrap();
        assert_eq!(plugin_root(&dir).unwrap(), dir.join("youtube"));
        let _ = fs::remove_dir_all(&dir);
    }
}
