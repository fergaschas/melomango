//! Reads the data export that Spotify sends on request (Account → Privacy →
//! "Download your data"), so playlists and liked songs can be imported without a
//! developer Client ID nor Premium.
//!
//! The export is a zip (`my_spotify_data.zip`) with, among others:
//!
//! ```text
//! Spotify Account Data/Playlist1.json   {"playlists": [{"name", "items": [{"track": {...}} | {"localTrack": {...}}]}]}
//! Spotify Account Data/YourLibrary.json {"tracks": [{"artist", "album", "track", "uri"}], ...}
//! ```
//!
//! It has no durations nor covers: tracks are matched to YouTube by title and artist.
//!
//! The same import also takes Exportify's CSVs (see [`crate::exportify`]).

use crate::text::fingerprint;
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::Path,
};

use anyhow::{bail, Context};
use percent_encoding::percent_decode_str;
use serde::Deserialize;

use crate::{exportify, models::Track};

/// Plugin id the imported playlists are linked to (they can't be synced).
pub const ORIGIN: &str = "spotify-export";
/// Imported tracks keep Spotify's ids, so they match those imported with the plugin.
const SOURCE: &str = "spotify";
const MAX_FILE: u64 = 100 * 1024 * 1024;
/// A zip may hold many files: what is read from one, in total.
const MAX_ZIP_TOTAL: u64 = 500 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 20_000;
/// For a playlist that comes without a name.
const UNNAMED: &str = "Playlist sin nombre";

#[derive(Debug)]
pub struct Collection {
    pub remote_id: String,
    pub name: String,
    pub tracks: Vec<Track>,
}

/// What an export holds, and the files (or parts) that couldn't be read and were skipped.
#[derive(Debug)]
pub struct Import {
    pub collections: Vec<Collection>,
    /// One line per skipped file: "Playlist3.json: no es válido…".
    pub problems: Vec<String>,
}

/// Reads every collection in the given files: Spotify's export (zip or its JSON files)
/// or Exportify's (zip or its CSV files).
///
/// A file that can't be read (a CSV saved by Excel in another format, a damaged JSON) is
/// skipped and reported in `problems` instead of stopping the whole import. It fails only
/// when nothing at all could be read.
pub fn read(paths: &[impl AsRef<Path>]) -> anyhow::Result<Import> {
    let mut files = Vec::new();
    let mut problems = Vec::new();
    for path in paths {
        let path = path.as_ref();
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let is_zip = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"));
        let read = if is_zip {
            files_in_zip(path, &mut problems)
        } else {
            fs::File::open(path)
                .with_context(|| format!("no se pudo abrir {}", path.display()))
                .and_then(read_capped)
                .map(|data| vec![(name.clone(), data)])
        };
        match read {
            Ok(found) => files.extend(found),
            Err(e) => problems.push(format!("{name}: {e:#}")),
        }
    }

    let mut collections = Vec::new();
    let mut liked = Vec::new();
    let mut seen_names: HashMap<String, u32> = HashMap::new();
    let mut add = |collections: &mut Vec<Collection>, name: String, tracks: Vec<Track>| {
        if tracks.is_empty() {
            return;
        }
        let name = if name.trim().is_empty() { UNNAMED.to_string() } else { name };
        let n = seen_names.entry(name.clone()).or_default();
        *n += 1;
        let remote_id = if *n == 1 { format!("playlist:{name}") } else { format!("playlist:{name}#{n}") };
        collections.push(Collection { remote_id, name, tracks });
    };
    let mut recognised = false;
    // Playlist1.json, Playlist2.json… in order.
    files.sort_by(|a, b| natural_key(&a.0).cmp(&natural_key(&b.0)));
    for (name, data) in &files {
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let parsed: anyhow::Result<()> = (|| {
            if is_playlist_file(base) {
                recognised = true;
                let file: PlaylistFile = serde_json::from_slice(data).context("no es un JSON válido")?;
                for playlist in file.playlists {
                    let tracks: Vec<Track> = playlist.items.iter().filter_map(PlaylistItem::to_track).collect();
                    add(&mut collections, playlist.name.trim().to_string(), tracks);
                }
            } else if base.eq_ignore_ascii_case("YourLibrary.json") {
                recognised = true;
                let file: LibraryFile = serde_json::from_slice(data).context("no es un JSON válido")?;
                liked.extend(file.tracks.iter().filter_map(LibraryTrack::to_track));
            } else if is_csv(base) {
                match exportify::read(base, data)? {
                    exportify::Content::Liked(tracks) => {
                        recognised = true;
                        liked.extend(tracks);
                    }
                    exportify::Content::Playlists(lists) => {
                        recognised = true;
                        for (name, tracks) in lists {
                            add(&mut collections, name, tracks);
                        }
                    }
                    exportify::Content::Ignored => {}
                }
            }
            Ok(())
        })();
        if let Err(e) = parsed {
            problems.push(format!("{base}: {e:#}"));
        }
    }
    if !liked.is_empty() {
        collections.insert(0, Collection { remote_id: "liked".into(), name: "Me gusta de Spotify".into(), tracks: liked });
    }
    if collections.is_empty() {
        if !problems.is_empty() {
            bail!("no se pudo importar nada:\n{}", problems.join("\n"));
        }
        if !recognised {
            bail!(
                "no encontré playlists ni Me gusta: elige el zip que te envió Spotify (my_spotify_data.zip), \
                 el de Exportify (spotify_playlists.zip) o sus archivos JSON o CSV"
            );
        }
    }
    Ok(Import { collections, problems })
}

/// The files worth reading in a zip. An entry that can't be read goes to `problems`; a zip
/// that can't be opened, or that is too big (a zip bomb, or the wrong file), is an error.
fn files_in_zip(path: &Path, problems: &mut Vec<String>) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
    let file = fs::File::open(path).with_context(|| format!("no se pudo abrir {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).context("el archivo no es un zip válido")?;
    if zip.len() > MAX_ZIP_ENTRIES {
        bail!("el zip tiene demasiados archivos ({}): no parece una exportación de Spotify", zip.len());
    }
    let mut out = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let entry = match zip.by_index(i) {
            Ok(entry) => entry,
            Err(e) => {
                problems.push(format!("archivo {} del zip: {e}", i + 1));
                continue;
            }
        };
        let name = entry.name().to_string();
        let base = name.rsplit('/').next().unwrap_or(&name).to_string();
        if !(is_playlist_file(&base) || base.eq_ignore_ascii_case("YourLibrary.json") || is_csv(&base)) {
            continue;
        }
        match read_capped(entry) {
            Ok(data) => {
                total += data.len() as u64;
                if total > MAX_ZIP_TOTAL {
                    bail!("el zip es demasiado grande: no parece una exportación de Spotify");
                }
                out.push((name, data));
            }
            Err(e) => problems.push(format!("{base}: {e:#}")),
        }
    }
    Ok(out)
}

fn read_capped(reader: impl Read) -> anyhow::Result<Vec<u8>> {
    let mut data = Vec::new();
    reader.take(MAX_FILE + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_FILE {
        bail!("archivo demasiado grande");
    }
    Ok(data)
}

fn is_csv(base: &str) -> bool {
    base.to_ascii_lowercase().ends_with(".csv")
}

/// `Playlist1.json`, `Playlist12.json`… (also `Playlists.json` in some exports).
fn is_playlist_file(base: &str) -> bool {
    let lower = base.to_ascii_lowercase();
    lower
        .strip_prefix("playlist")
        .and_then(|rest| rest.strip_suffix(".json"))
        .is_some_and(|middle| middle == "s" || middle.chars().all(|c| c.is_ascii_digit()))
}

/// Sorts "Playlist2" before "Playlist10".
fn natural_key(name: &str) -> (String, u64) {
    let digits: String = name.chars().filter(char::is_ascii_digit).collect();
    let text: String = name.chars().filter(|c| !c.is_ascii_digit()).collect();
    (text, digits.parse().unwrap_or(0))
}

#[derive(Deserialize)]
struct PlaylistFile {
    #[serde(default)]
    playlists: Vec<ExportPlaylist>,
}

#[derive(Deserialize)]
struct ExportPlaylist {
    #[serde(default)]
    name: String,
    #[serde(default)]
    items: Vec<PlaylistItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistItem {
    track: Option<ExportTrack>,
    local_track: Option<LocalTrack>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportTrack {
    #[serde(default)]
    track_name: String,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    track_uri: String,
}

#[derive(Deserialize)]
struct LocalTrack {
    #[serde(default)]
    uri: String,
}

#[derive(Deserialize)]
struct LibraryFile {
    #[serde(default)]
    tracks: Vec<LibraryTrack>,
}

#[derive(Deserialize)]
struct LibraryTrack {
    #[serde(default)]
    artist: String,
    #[serde(default)]
    track: String,
    #[serde(default)]
    uri: String,
}

impl PlaylistItem {
    fn to_track(&self) -> Option<Track> {
        if let Some(t) = &self.track {
            return track(&t.track_uri, &t.track_name, &t.artist_name, 0);
        }
        local_track(&self.local_track.as_ref()?.uri)
    }
}

impl LibraryTrack {
    fn to_track(&self) -> Option<Track> {
        track(&self.uri, &self.track, &self.artist, 0)
    }
}

pub(crate) fn track(uri: &str, title: &str, artist: &str, duration_secs: u32) -> Option<Track> {
    let (title, artist) = (title.trim(), artist.trim());
    if title.is_empty() {
        return None;
    }
    let id = match uri.strip_prefix("spotify:track:") {
        Some(id) if !id.is_empty() => id.to_string(),
        // Without a Spotify id, one derived from the song keeps re-imports stable.
        _ => format!("local:{}", fingerprint(&format!("{artist}\n{title}"))),
    };
    Some(Track {
        id,
        source: SOURCE.into(),
        title: title.into(),
        artist: if artist.is_empty() { "Desconocido".into() } else { artist.into() },
        duration_secs,
        artwork_url: None,
        genre: None,
    })
}

/// `spotify:local:<artist>:<album>:<title>:<seconds>`, URL-encoded with `+` for spaces.
fn local_track(uri: &str) -> Option<Track> {
    let parts: Vec<String> = uri
        .strip_prefix("spotify:local:")?
        .split(':')
        .map(|p| percent_decode_str(&p.replace('+', " ")).decode_utf8_lossy().into_owned())
        .collect();
    let [artist, _album, title, seconds] = parts.as_slice() else { return None };
    track("", title, artist, seconds.parse().unwrap_or(0))
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const PLAYLISTS: &str = r#"{"playlists": [
        {"name": "Para correr", "lastModifiedDate": "2026-01-02", "items": [
            {"track": {"trackName": "Dai Dai", "artistName": "Shakira", "albumName": "Dai Dai", "trackUri": "spotify:track:4fakeSpotifyId"},
             "episode": null, "localTrack": null, "audiobook": null, "addedDate": "2026-01-01"},
            {"track": null, "episode": {"episodeName": "Podcast", "showName": "X", "episodeUri": "spotify:episode:1"}, "localTrack": null},
            {"track": null, "episode": null, "localTrack": {"uri": "spotify:local:Los+Planetas:Una+semana:Segundo+premio:245"}}
        ]},
        {"name": "Solo podcasts", "items": [{"track": null, "episode": {"episodeUri": "spotify:episode:2"}}]},
        {"name": "Para correr", "items": [
            {"track": {"trackName": "Otra", "artistName": "Alguien", "trackUri": "spotify:track:5other"}}
        ]}
    ]}"#;
    const LIBRARY: &str = r#"{"tracks": [
        {"artist": "Rosalía", "album": "Motomami", "track": "Saoko", "uri": "spotify:track:2saoko"}
    ], "albums": [{"artist": "Rosalía", "album": "Motomami", "uri": "spotify:album:1"}], "shows": [], "episodes": []}"#;

    fn export_zip(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("my_spotify_data.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in [
            ("Spotify Account Data/Playlist1.json", PLAYLISTS),
            ("Spotify Account Data/YourLibrary.json", LIBRARY),
            ("Spotify Account Data/Userdata.json", r#"{"username": "x"}"#),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(data.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("melomango-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_playlists_and_likes_from_the_zip() {
        let dir = temp_dir("export");
        let Import { collections, problems } = read(&[export_zip(&dir)]).unwrap();
        assert!(problems.is_empty(), "{problems:?}");

        let names: Vec<&str> = collections.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Me gusta de Spotify", "Para correr", "Para correr"]);
        let ids: Vec<&str> = collections.iter().map(|c| c.remote_id.as_str()).collect();
        assert_eq!(ids, ["liked", "playlist:Para correr", "playlist:Para correr#2"]);

        let liked = &collections[0].tracks[0];
        assert_eq!((liked.source.as_str(), liked.id.as_str(), liked.title.as_str()), ("spotify", "2saoko", "Saoko"));

        // Podcast episodes are skipped; local files keep their title, artist and duration.
        let run = &collections[1].tracks;
        assert_eq!(run.len(), 2);
        assert_eq!(run[0].id, "4fakeSpotifyId");
        assert_eq!((run[1].title.as_str(), run[1].artist.as_str(), run[1].duration_secs), ("Segundo premio", "Los Planetas", 245));
        assert!(run[1].id.starts_with("local:"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_loose_json_files() {
        let dir = temp_dir("export-json");
        let path = dir.join("YourLibrary.json");
        fs::write(&path, LIBRARY).unwrap();
        let collections = read(&[path]).unwrap().collections;
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].tracks[0].artist, "Rosalía");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_other_files() {
        let dir = temp_dir("export-other");
        let path = dir.join("Userdata.json");
        fs::write(&path, "{}").unwrap();
        assert!(read(&[path]).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_exportify_zip_and_merges_liked_songs() {
        let dir = temp_dir("exportify");
        let path = dir.join("spotify_playlists.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in [
            ("liked.csv", "\"Track URI\",\"Track Name\",\"Artist Name(s)\",\"Track Duration (ms)\"\n\"spotify:track:2saoko\",\"Saoko\",\"ROSALÍA\",\"137875\"\n"),
            ("para_correr.csv", "\"Track URI\",\"Track Name\",\"Artist Name(s)\"\n\"spotify:track:x\",\"Dai Dai\",\"Shakira\"\n"),
            ("saved_albums.csv", "\"Album URI\",\"Album Name\"\n\"spotify:album:1\",\"RAM\"\n"),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(data.as_bytes()).unwrap();
        }
        zip.finish().unwrap();

        let collections = read(&[path, export_zip(&dir)]).unwrap().collections;
        let names: Vec<&str> = collections.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Me gusta de Spotify", "Para correr", "Para correr", "Para correr"]);
        // Both exports' liked songs end up together (the same track twice is deduplicated on import).
        assert_eq!(collections[0].tracks.len(), 2);
        assert!(collections[0].tracks.iter().any(|t| t.id == "2saoko" && t.duration_secs == 138));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_bad_file_is_skipped_and_reported_instead_of_stopping_the_import() {
        let dir = temp_dir("export-partial");
        let good = dir.join("YourLibrary.json");
        fs::write(&good, LIBRARY).unwrap();
        let broken = dir.join("Playlist1.json");
        fs::write(&broken, "{\"playlists\": [ esto no es JSON").unwrap();
        let missing = dir.join("Playlist2.json");

        let Import { collections, problems } = read(&[good, broken, missing]).unwrap();
        assert_eq!(collections.len(), 1, "the liked songs still come in");
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].starts_with("Playlist2.json") || problems[1].starts_with("Playlist2.json"));
        assert!(problems.iter().any(|p| p.starts_with("Playlist1.json") && p.contains("JSON")));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nothing_readable_is_an_error_that_lists_why() {
        let dir = temp_dir("export-nothing");
        let broken = dir.join("Playlist1.json");
        fs::write(&broken, "nope").unwrap();
        let err = read(&[broken]).unwrap_err().to_string();
        assert!(err.contains("no se pudo importar nada") && err.contains("Playlist1.json"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unnamed_playlist_gets_a_name() {
        let dir = temp_dir("export-unnamed");
        let path = dir.join("Playlist1.json");
        fs::write(&path, r#"{"playlists": [{"name": "  ", "items": [{"track": {"trackName": "Saoko", "artistName": "Rosalía", "trackUri": "spotify:track:1"}}]}]}"#).unwrap();
        let collections = read(&[path]).unwrap().collections;
        assert_eq!(collections[0].name, UNNAMED);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn zips_with_too_many_files_are_refused() {
        let dir = temp_dir("export-many");
        let path = dir.join("many.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for i in 0..=MAX_ZIP_ENTRIES {
            zip.start_file(format!("f{i}.txt"), zip::write::SimpleFileOptions::default()).unwrap();
        }
        zip.finish().unwrap();
        let err = read(&[path]).unwrap_err().to_string();
        assert!(err.contains("demasiados archivos"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recognises_playlist_file_names() {
        assert!(is_playlist_file("Playlist1.json"));
        assert!(is_playlist_file("Playlist12.json"));
        assert!(!is_playlist_file("PlaylistX.json"));
        assert!(!is_playlist_file("YourLibrary.json"));
    }

    #[test]
    fn local_ids_are_stable() {
        let a = track("", "Canción", "Artista", 0).unwrap();
        let b = track("", "Canción", "Artista", 0).unwrap();
        assert_eq!(a.id, b.id);
    }
}
