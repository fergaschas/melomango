//! Importing a playlist or album from a pasted link: Spotify (its public embed page, no
//! login, at most 100 songs), YouTube / YouTube Music (through the YouTube plugin) and
//! Deezer (its public API).

use anyhow::{Context, bail};
use serde_json::Value;

use crate::models::Track;

/// Plugin ids imported playlists are linked to, so "Sincronizar" knows where to look.
pub const SPOTIFY_ORIGIN: &str = "spotify-link";
pub const YOUTUBE_ORIGIN: &str = "youtube-link";
/// Spotify's embed page lists at most this many songs.
pub const SPOTIFY_EMBED_LIMIT: usize = 100;
/// Spotify serves the embed page to browsers.
const BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

#[derive(Debug, Clone, PartialEq)]
pub enum Link {
    SpotifyPlaylist(String),
    SpotifyAlbum(String),
    YouTubePlaylist(String),
    DeezerPlaylist(String),
    DeezerAlbum(String),
}

impl Link {
    /// Where the imported copy comes from, as (plugin id, remote id) for the library.
    pub fn origin(&self) -> (&'static str, String) {
        match self {
            Link::SpotifyPlaylist(id) => (SPOTIFY_ORIGIN, format!("playlist:{id}")),
            Link::SpotifyAlbum(id) => (SPOTIFY_ORIGIN, format!("album:{id}")),
            Link::YouTubePlaylist(id) => (YOUTUBE_ORIGIN, format!("playlist:{id}")),
            Link::DeezerPlaylist(id) => (crate::artists::PLAYLIST_ORIGIN, format!("playlist:{id}")),
            Link::DeezerAlbum(id) => (crate::artists::PLAYLIST_ORIGIN, format!("album:{id}")),
        }
    }

    /// The link an imported playlist came from, from its library origin.
    pub fn from_origin(plugin_id: &str, remote_id: &str) -> Option<Self> {
        let (kind, id) = remote_id.split_once(':')?;
        let id = id.to_string();
        Some(match (plugin_id, kind) {
            (SPOTIFY_ORIGIN, "playlist") => Link::SpotifyPlaylist(id),
            (SPOTIFY_ORIGIN, "album") => Link::SpotifyAlbum(id),
            (YOUTUBE_ORIGIN, "playlist") => Link::YouTubePlaylist(id),
            (crate::artists::PLAYLIST_ORIGIN, "playlist") => Link::DeezerPlaylist(id),
            (crate::artists::PLAYLIST_ORIGIN, "album") => Link::DeezerAlbum(id),
            _ => return None,
        })
    }
}

/// Recognises a pasted playlist or album link (or a `spotify:playlist:…` URI).
pub fn parse(text: &str) -> Option<Link> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("spotify:") {
        let (kind, id) = rest.split_once(':')?;
        return spotify(kind, id);
    }
    let url = url_parts(text)?;
    let host = url.host.trim_start_matches("www.").trim_start_matches("m.");
    let segments: Vec<&str> = url.path.split('/').filter(|s| !s.is_empty()).collect();
    match host {
        "open.spotify.com" | "play.spotify.com" => {
            // /playlist/ID, /intl-es/playlist/ID, /embed/playlist/ID
            let pos = segments.iter().position(|s| *s == "playlist" || *s == "album")?;
            spotify(segments[pos], segments.get(pos + 1)?)
        }
        "youtube.com" | "music.youtube.com" | "youtu.be" => {
            let list = url.query_param("list")?;
            // "Mix" lists (RD…) are generated on the fly, not real playlists.
            valid_id(&list).filter(|l| !l.starts_with("RD")).map(Link::YouTubePlaylist)
        }
        "deezer.com" | "deezer.page.link" => {
            let pos = segments.iter().position(|s| *s == "playlist" || *s == "album")?;
            let id = segments.get(pos + 1)?;
            if !id.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            Some(if segments[pos] == "playlist" {
                Link::DeezerPlaylist(id.to_string())
            } else {
                Link::DeezerAlbum(id.to_string())
            })
        }
        _ => None,
    }
}

fn spotify(kind: &str, id: &str) -> Option<Link> {
    let id = valid_id(id)?;
    match kind {
        "playlist" => Some(Link::SpotifyPlaylist(id)),
        "album" => Some(Link::SpotifyAlbum(id)),
        _ => None,
    }
}

fn valid_id(id: &str) -> Option<String> {
    let id = id.split(['?', '#', '&']).next()?;
    (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .then(|| id.to_string())
}

struct UrlParts<'a> {
    host: String,
    path: &'a str,
    query: &'a str,
}

impl UrlParts<'_> {
    fn query_param(&self, name: &str) -> Option<String> {
        self.query.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == name).then(|| v.to_string())
        })
    }
}

fn url_parts(text: &str) -> Option<UrlParts<'_>> {
    let rest = text.strip_prefix("https://").or_else(|| text.strip_prefix("http://")).unwrap_or(text);
    let (host, after) = match rest.find(['/', '?']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let after = after.split('#').next().unwrap_or("");
    let (path, query) = after.split_once('?').unwrap_or((after, ""));
    if !host.contains('.') {
        return None;
    }
    Some(UrlParts { host: host.to_ascii_lowercase(), path, query })
}

// ---------- Spotify (public embed page) ----------

/// Name and songs of a public Spotify playlist or album, from its embed page.
pub async fn spotify_collection(http: &reqwest::Client, kind: &str, id: &str) -> anyhow::Result<(String, Vec<Track>)> {
    let html = http
        .get(format!("https://open.spotify.com/embed/{kind}/{id}"))
        .header(reqwest::header::USER_AGENT, BROWSER_UA)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .context("no se pudo conectar con Spotify")?
        .error_for_status()
        .context("Spotify no encontró esa playlist (¿es pública?)")?
        .text()
        .await?;
    parse_embed(&html)
}

fn parse_embed(html: &str) -> anyhow::Result<(String, Vec<Track>)> {
    let start = html
        .find(r#"<script id="__NEXT_DATA__""#)
        .and_then(|i| html[i..].find('>').map(|j| i + j + 1))
        .context("la página de Spotify ha cambiado: no encuentro la lista de canciones")?;
    let end = html[start..].find("</script>").context("página de Spotify incompleta")? + start;
    let data: Value = serde_json::from_str(&html[start..end]).context("datos de Spotify no válidos")?;
    let entity = find_entity(&data).context("Spotify no devolvió canciones para ese enlace")?;
    let name = entity["name"].as_str().or(entity["title"].as_str()).unwrap_or("Playlist de Spotify").trim().to_string();
    let tracks: Vec<Track> =
        entity["trackList"].as_array().map(|list| list.iter().filter_map(embed_track).collect()).unwrap_or_default();
    if tracks.is_empty() {
        bail!("esa playlist de Spotify está vacía o no es pública");
    }
    Ok((name, tracks))
}

/// The object holding `trackList` (its place in the page data changes over time).
fn find_entity(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(map) => {
            if map.get("trackList").is_some_and(Value::is_array) {
                return Some(value);
            }
            map.values().find_map(find_entity)
        }
        Value::Array(list) => list.iter().find_map(find_entity),
        _ => None,
    }
}

fn embed_track(item: &Value) -> Option<Track> {
    let id = item["uri"].as_str()?.strip_prefix("spotify:track:")?;
    let title = item["title"].as_str()?.trim();
    // Artists come as "A,\u{a0}B": normal spaces for the matcher.
    let artist = item["subtitle"].as_str().unwrap_or("").replace('\u{a0}', " ");
    Some(Track {
        id: id.to_string(),
        source: "spotify".into(),
        title: title.to_string(),
        artist: if artist.trim().is_empty() { "Desconocido".into() } else { artist.trim().to_string() },
        duration_secs: (item["duration"].as_f64().unwrap_or(0.0) / 1000.0).round() as u32,
        artwork_url: None,
        genre: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_links() {
        let cases = [
            (
                "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc",
                Link::SpotifyPlaylist("37i9dQZF1DXcBWIGoYBM5M".into()),
            ),
            (
                "https://open.spotify.com/intl-es/album/2noRn2Aes5aoNVsU6iWThc",
                Link::SpotifyAlbum("2noRn2Aes5aoNVsU6iWThc".into()),
            ),
            ("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M", Link::SpotifyPlaylist("37i9dQZF1DXcBWIGoYBM5M".into())),
            (
                "https://www.youtube.com/playlist?list=PL4fGSI1pDJn6puJdseH2Rt9sMvt9E2M4i",
                Link::YouTubePlaylist("PL4fGSI1pDJn6puJdseH2Rt9sMvt9E2M4i".into()),
            ),
            ("https://music.youtube.com/playlist?list=OLAK5uy_kXyz", Link::YouTubePlaylist("OLAK5uy_kXyz".into())),
            ("https://www.youtube.com/watch?v=kJQP7kiw5Fk&list=PLabc123", Link::YouTubePlaylist("PLabc123".into())),
            ("https://www.deezer.com/es/playlist/1479458365", Link::DeezerPlaylist("1479458365".into())),
            ("deezer.com/album/302127", Link::DeezerAlbum("302127".into())),
        ];
        for (text, want) in cases {
            assert_eq!(parse(text), Some(want), "{text}");
        }
        for bad in [
            "https://www.youtube.com/watch?v=kJQP7kiw5Fk",
            "https://www.youtube.com/watch?v=kJQP7kiw5Fk&list=RDkJQP7kiw5Fk",
            "https://open.spotify.com/track/abc",
            "https://example.com/playlist/1",
            "hola",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn origins_round_trip() {
        for link in [
            Link::SpotifyPlaylist("a".into()),
            Link::SpotifyAlbum("b".into()),
            Link::YouTubePlaylist("c".into()),
            Link::DeezerPlaylist("1".into()),
            Link::DeezerAlbum("2".into()),
        ] {
            let (plugin, remote) = link.origin();
            assert_eq!(Link::from_origin(plugin, &remote), Some(link));
        }
    }

    #[test]
    fn reads_the_embed_page() {
        let html = r#"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{
            "name":"Indie para correr","trackList":[
              {"uri":"spotify:track:1","title":"Copenhague","subtitle":"Vetusta Morla","duration":304000},
              {"uri":"spotify:track:2","title":"Dai Dai","subtitle":"Shakira, Burna Boy","duration":223400},
              {"uri":"spotify:episode:3","title":"Un podcast","subtitle":"X","duration":1}
            ]}}}}}}</script></html>"#;
        let (name, tracks) = parse_embed(html).unwrap();
        assert_eq!(name, "Indie para correr");
        assert_eq!(tracks.len(), 2, "episodes are skipped");
        assert_eq!((tracks[0].source.as_str(), tracks[0].id.as_str(), tracks[0].duration_secs), ("spotify", "1", 304));
        assert_eq!(tracks[1].artist, "Shakira, Burna Boy");
        assert!(parse_embed("<html></html>").is_err());
    }

    /// Against Spotify's real embed page: `cargo test -- --ignored links`
    #[tokio::test]
    #[ignore = "necesita red"]
    async fn reads_a_real_spotify_playlist() {
        let http = reqwest::Client::new();
        let (name, tracks) = spotify_collection(&http, "playlist", "37i9dQZF1DXcBWIGoYBM5M").await.unwrap();
        assert!(!name.is_empty());
        assert!(tracks.len() >= 10);
        assert!(tracks.iter().all(|t| t.duration_secs > 0 && !t.artist.is_empty()));
    }
}
