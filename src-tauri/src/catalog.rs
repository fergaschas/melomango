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

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::sources::plugin;

pub const OFFICIAL_INDEX: &str =
    "https://raw.githubusercontent.com/fergaschas/melomango-plugins/main/index.json";
const INDEX_VERSION: u32 = 1;
const MAX_DOWNLOAD_BYTES: usize = 200 * 1024 * 1024;

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
#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct Catalog {
    pub plugins: Vec<CatalogPlugin>,
    /// One message per repository that could not be read.
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
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

#[derive(Default, Serialize, Deserialize)]
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
    let index: Index = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("el índice no tiene un formato válido")?;
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

/// Downloads `id` from the index at `registry_url`, verifies it and installs it.
/// Returns the installed plugin id.
pub async fn install(
    http: &reqwest::Client,
    registry_url: &str,
    id: &str,
    plugins_dir: &Path,
) -> anyhow::Result<String> {
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
    let result = (|| {
        zip::ZipArchive::new(Cursor::new(bytes))
            .context("la descarga no es un zip válido")?
            .extract(&staging)
            .context("no se pudo descomprimir el plugin")?;
        let root = plugin_root(&staging)?;
        let installed = plugin::install(&root, plugins_dir)?;
        if installed != entry.id {
            let _ = fs::remove_dir_all(plugins_dir.join(&installed));
            bail!("el paquete contiene el plugin \"{installed}\", no \"{}\"", entry.id);
        }
        Ok(installed)
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

async fn download_limited(http: &reqwest::Client, url: &str) -> anyhow::Result<Vec<u8>> {
    let mut response = http.get(url).send().await?.error_for_status()?;
    if response.content_length().is_some_and(|len| len as usize > MAX_DOWNLOAD_BYTES) {
        bail!("el plugin es demasiado grande");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX_DOWNLOAD_BYTES {
            bail!("el plugin es demasiado grande");
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
    let subdirs: Vec<PathBuf> = fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
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
    fn finds_plugin_root_in_subfolder() {
        let dir = std::env::temp_dir().join(format!("melomango-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("youtube")).unwrap();
        fs::write(dir.join("youtube/plugin.json"), "{}").unwrap();
        assert_eq!(plugin_root(&dir).unwrap(), dir.join("youtube"));
        let _ = fs::remove_dir_all(&dir);
    }
}
