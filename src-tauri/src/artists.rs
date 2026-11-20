//! Artist, album and public playlist pages, and searching them, with data from Deezer's public API (https://api.deezer.com,
//! no account or key needed).
//!
//! Like tracks imported from Spotify, Deezer's are metadata only: they are stored in the
//! library with source "deezer" and, when played, matched to a playable source (YouTube
//! first) by the matcher.

use std::{collections::HashMap, sync::Mutex, time::Duration};

use anyhow::{Context, anyhow, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::models::Track;

const API: &str = "https://api.deezer.com";
/// Source id of Deezer's tracks (not playable by themselves).
pub const SOURCE: &str = "deezer";
/// Plugin id that saved Deezer playlists are linked to (so they can be synced).
pub const PLAYLIST_ORIGIN: &str = "deezer";
const SEARCH_RESULTS: &str = "20";
const TOP_TRACKS: u32 = 10;
const RELATED: u32 = 10;
const CACHE_LIMIT: usize = 100;
/// Songs per request when paging a playlist (the most Deezer gives at once).
const PLAYLIST_PAGE: usize = 1000;
/// Enough for any playlist (Deezer caps them at 10.000 songs), and a stop if it loops.
const MAX_PLAYLIST_PAGES: usize = 12;
/// Deezer allows ~50 requests every 5 s; past that it answers "Quota limit exceeded".
const QUOTA_RETRIES: u32 = 3;
const QUOTA_WAIT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistRef {
    pub id: String,
    pub name: String,
    pub picture_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub id: String,
    pub title: String,
    pub cover_url: Option<String>,
    /// "YYYY-MM-DD", when known.
    pub release_date: Option<String>,
    /// "album", "single", "ep" or "compile".
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistPage {
    pub artist: ArtistRef,
    pub fans: u64,
    pub top: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
    pub related: Vec<ArtistRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumPage {
    #[serde(flatten)]
    pub album: AlbumSummary,
    pub artist: ArtistRef,
    pub tracks: Vec<Track>,
    pub duration_secs: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistHit {
    #[serde(flatten)]
    pub artist: ArtistRef,
    pub fans: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumHit {
    #[serde(flatten)]
    pub album: AlbumSummary,
    pub artist_name: String,
}

/// A public Deezer playlist (made by a user or an editor).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPlaylist {
    pub id: String,
    pub title: String,
    pub picture_url: Option<String>,
    pub owner: String,
    pub track_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPage {
    #[serde(flatten)]
    pub playlist: PublicPlaylist,
    pub description: Option<String>,
    pub tracks: Vec<Track>,
    pub duration_secs: u32,
}

/// Quick suggestions while typing in the search box.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestions {
    pub artists: Vec<ArtistRef>,
    pub tracks: Vec<Track>,
}

/// Artists, albums and playlists matching a search.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSearch {
    pub artists: Vec<ArtistHit>,
    pub albums: Vec<AlbumHit>,
    pub playlists: Vec<PublicPlaylist>,
}

pub struct ArtistService {
    http: reqwest::Client,
    /// Deezer, or a local server in tests.
    api: String,
    /// First wait after a quota error; it doubles on each retry.
    quota_wait: Duration,
    artists: Mutex<HashMap<String, ArtistPage>>,
    albums: Mutex<HashMap<String, AlbumPage>>,
    playlists: Mutex<HashMap<String, PlaylistPage>>,
    suggestions: Mutex<HashMap<String, Suggestions>>,
}

impl ArtistService {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            api: API.to_string(),
            quota_wait: QUOTA_WAIT,
            artists: Mutex::default(),
            albums: Mutex::default(),
            playlists: Mutex::default(),
            suggestions: Mutex::default(),
        }
    }

    /// The page of the artist with Deezer id `id`, or else the best match for `name`.
    pub async fn artist(&self, name: &str, id: Option<&str>) -> anyhow::Result<ArtistPage> {
        // Cached by name only when the artist was looked up by name: an id chosen elsewhere
        // (a search result) may be a lesser-known namesake, not "the" artist with that name.
        let by_name = match id {
            Some(_) => None,
            None if name.trim().is_empty() => bail!("falta el nombre del artista"),
            None => Some(format!("name:{}", normalize(name))),
        };
        if let Some(page) = by_name.as_ref().and_then(|key| self.artists.lock().unwrap().get(key).cloned()) {
            return Ok(page);
        }
        let id = match id {
            Some(id) => id.to_string(),
            None => self.find_artist(name).await?,
        };
        if let Some(page) = self.artists.lock().unwrap().get(&format!("id:{id}")) {
            return Ok(page.clone());
        }

        let paths = [
            format!("/artist/{id}"),
            format!("/artist/{id}/top?limit={TOP_TRACKS}"),
            format!("/artist/{id}/albums?limit=200"),
            format!("/artist/{id}/related?limit={RELATED}"),
        ];
        let (info, top, albums, related) = tokio::join!(
            self.get::<RawArtist>(&paths[0]),
            self.get::<Page<RawTrack>>(&paths[1]),
            self.get::<Page<RawAlbum>>(&paths[2]),
            self.get::<Page<RawArtist>>(&paths[3]),
        );
        let info = info?;
        let page = ArtistPage {
            fans: info.nb_fan.unwrap_or(0),
            artist: info.into_ref(),
            top: top?.data.into_iter().map(|t| t.into_track(None)).collect(),
            albums: albums?.data.into_iter().map(RawAlbum::into_summary).collect(),
            // Related artists are a nicety: the page works without them.
            related: related.map(|r| r.data.into_iter().map(RawArtist::into_ref).collect()).unwrap_or_default(),
        };

        let mut cache = self.artists.lock().unwrap();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(format!("id:{id}"), page.clone());
        if let Some(key) = by_name {
            cache.insert(key, page.clone());
        }
        Ok(page)
    }

    pub async fn album(&self, id: &str) -> anyhow::Result<AlbumPage> {
        if let Some(page) = self.albums.lock().unwrap().get(id) {
            return Ok(page.clone());
        }
        let raw: RawAlbumFull = self.get(&format!("/album/{id}")).await?;
        let cover = raw.cover_medium.clone();
        let page = AlbumPage {
            artist: raw.artist.into_ref(),
            tracks: raw.tracks.data.into_iter().map(|t| t.into_track(cover.as_deref())).collect(),
            duration_secs: raw.duration.unwrap_or(0),
            album: AlbumSummary {
                id: raw.id.to_string(),
                title: raw.title,
                cover_url: raw.cover_big.or(raw.cover_medium),
                release_date: raw.release_date,
                kind: raw.record_type.unwrap_or_else(|| "album".into()),
            },
        };
        let mut cache = self.albums.lock().unwrap();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(id.to_string(), page.clone());
        Ok(page)
    }

    /// Artists, albums and public playlists matching `query`. Each kind is optional: the
    /// search fails only if none of them answered.
    pub async fn search(&self, query: &str) -> anyhow::Result<CatalogSearch> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(CatalogSearch { artists: Vec::new(), albums: Vec::new(), playlists: Vec::new() });
        }
        let params = [("q", query), ("limit", SEARCH_RESULTS)];
        let (artists, albums, playlists) = tokio::join!(
            self.get_query::<Page<RawArtist>>("/search/artist", &params),
            self.get_query::<Page<RawAlbumHit>>("/search/album", &params),
            self.get_query::<Page<RawPlaylist>>("/search/playlist", &params),
        );
        if let (Err(e), Err(_), Err(_)) = (&artists, &albums, &playlists) {
            bail!("{e:#}");
        }
        let mut artists: Vec<ArtistHit> = artists
            .map(|p| p.data)
            .unwrap_or_default()
            .into_iter()
            .map(|a| ArtistHit { fans: a.nb_fan.unwrap_or(0), artist: a.into_ref() })
            .collect();
        // The artist whose name is exactly the query goes first (Deezer sometimes ranks a
        // namesake higher); otherwise Deezer's order.
        let wanted = normalize(query);
        if let Some(pos) = artists
            .iter()
            .enumerate()
            .filter(|(_, a)| normalize(&a.artist.name) == wanted)
            .max_by_key(|(_, a)| a.fans)
            .map(|(i, _)| i)
        {
            let exact = artists.remove(pos);
            artists.insert(0, exact);
        }
        Ok(CatalogSearch {
            artists,
            albums: albums.map(|p| p.data).unwrap_or_default().into_iter().map(RawAlbumHit::into_hit).collect(),
            playlists: playlists
                .map(|p| p.data)
                .unwrap_or_default()
                .into_iter()
                .map(RawPlaylist::into_public)
                .collect(),
        })
    }

    /// A few artists and songs for what is being typed (cached: typing back and forth is free).
    pub async fn suggest(&self, query: &str) -> anyhow::Result<Suggestions> {
        let query = query.trim();
        let key = format!("suggest:{}", query.to_lowercase());
        if let Some(found) = self.suggestions.lock().unwrap().get(&key) {
            return Ok(found.clone());
        }
        let artist_params = [("q", query), ("limit", "3")];
        let track_params = [("q", query), ("limit", "5")];
        let (artists, tracks) = tokio::join!(
            self.get_query::<Page<RawArtist>>("/search/artist", &artist_params),
            self.get_query::<Page<RawTrack>>("/search/track", &track_params),
        );
        if let (Err(e), Err(_)) = (&artists, &tracks) {
            bail!("{e:#}");
        }
        let found = Suggestions {
            artists: artists.map(|p| p.data).unwrap_or_default().into_iter().map(RawArtist::into_ref).collect(),
            tracks: tracks.map(|p| p.data).unwrap_or_default().into_iter().map(|t| t.into_track(None)).collect(),
        };
        let mut cache = self.suggestions.lock().unwrap();
        if cache.len() >= 500 {
            cache.clear();
        }
        cache.insert(key, found.clone());
        Ok(found)
    }

    /// A public playlist and its songs. `fresh` skips the cache (when syncing a saved copy).
    pub async fn playlist(&self, id: &str, fresh: bool) -> anyhow::Result<PlaylistPage> {
        let key = format!("playlist:{id}");
        if !fresh && let Some(page) = self.playlists.lock().unwrap().get(&key) {
            return Ok(page.clone());
        }
        let raw: RawPlaylistFull = self.get(&format!("/playlist/{id}")).await?;
        let mut tracks = raw.tracks.data;
        // The playlist itself brings only its first songs (400): the rest, page by page.
        let total = raw.summary.nb_tracks.unwrap_or(0) as usize;
        let mut pages = 0;
        while tracks.len() < total && pages < MAX_PLAYLIST_PAGES {
            pages += 1;
            let (index, limit) = (tracks.len().to_string(), PLAYLIST_PAGE.to_string());
            let page: Page<RawTrack> =
                self.get_query(&format!("/playlist/{id}/tracks"), &[("index", &index), ("limit", &limit)]).await?;
            if page.data.is_empty() {
                break;
            }
            tracks.extend(page.data);
        }
        let page = PlaylistPage {
            description: raw.description.filter(|d| !d.trim().is_empty()),
            duration_secs: raw.duration.unwrap_or(0),
            tracks: tracks.into_iter().map(|t| t.into_track(None)).collect(),
            playlist: raw.summary.into_public(),
        };
        let mut cache = self.playlists.lock().unwrap();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, page.clone());
        Ok(page)
    }

    /// Deezer id of the artist called `name`: among exact name matches, the one with the
    /// most fans (several artists share names); otherwise Deezer's first result.
    async fn find_artist(&self, name: &str) -> anyhow::Result<String> {
        let name = name.trim();
        if name.is_empty() {
            bail!("falta el nombre del artista");
        }
        let found: Page<RawArtist> = self.get_query("/search/artist", &[("q", name), ("limit", "10")]).await?;
        pick_artist(name, &found.data).map(|a| a.id.to_string()).ok_or_else(|| anyhow!("Deezer no conoce a «{name}»"))
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        self.get_query(path, &[]).await
    }

    async fn get_query<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> anyhow::Result<T> {
        let mut wait = self.quota_wait;
        let mut attempt = 0;
        loop {
            let value: Value = self
                .http
                .get(format!("{}{path}", self.api))
                .query(query)
                .timeout(Duration::from_secs(15))
                .send()
                .await
                .context("no se pudo conectar con Deezer")?
                .error_for_status()?
                .json()
                .await?;
            // Deezer reports errors with status 200 and an "error" object.
            let Some(error) = value.get("error") else {
                return serde_json::from_value(value).context("respuesta de Deezer inesperada");
            };
            // Too many requests at once (an artist page makes four): wait and try again.
            if is_quota_error(error) && attempt < QUOTA_RETRIES {
                attempt += 1;
                tokio::time::sleep(wait).await;
                wait *= 2;
                continue;
            }
            let message = error["message"].as_str().unwrap_or("error desconocido");
            bail!("Deezer: {message}");
        }
    }
}

/// Deezer's "Quota limit exceeded" (code 4), which comes with status 200.
fn is_quota_error(error: &Value) -> bool {
    error["code"].as_u64() == Some(4) || error["message"].as_str().is_some_and(|m| m.contains("Quota limit"))
}

fn pick_artist<'a>(name: &str, found: &'a [RawArtist]) -> Option<&'a RawArtist> {
    let wanted = normalize(name);
    found
        .iter()
        .filter(|a| normalize(&a.name) == wanted)
        .max_by_key(|a| a.nb_fan.unwrap_or(0))
        .or_else(|| found.first())
}

/// Lowercase without accents or punctuation, to compare names ("ROSALÍA" = "Rosalía").
fn normalize(s: &str) -> String {
    crate::text::fold(s).chars().filter(|c| c.is_alphanumeric()).collect()
}

// ---------- Deezer's JSON ----------

#[derive(Deserialize)]
struct Page<T> {
    #[serde(default = "Vec::new")]
    data: Vec<T>,
}

#[derive(Deserialize)]
struct RawArtist {
    id: u64,
    name: String,
    picture_big: Option<String>,
    picture_medium: Option<String>,
    nb_fan: Option<u64>,
}

impl RawArtist {
    fn into_ref(self) -> ArtistRef {
        ArtistRef { id: self.id.to_string(), name: self.name, picture_url: self.picture_big.or(self.picture_medium) }
    }
}

#[derive(Deserialize)]
struct RawTrack {
    id: u64,
    title: String,
    duration: Option<u32>,
    artist: Option<NameOnly>,
    #[serde(default)]
    contributors: Vec<NameOnly>,
    album: Option<RawTrackAlbum>,
}

#[derive(Deserialize)]
struct NameOnly {
    name: String,
}

#[derive(Deserialize)]
struct RawTrackAlbum {
    cover_medium: Option<String>,
}

impl RawTrack {
    fn into_track(self, album_cover: Option<&str>) -> Track {
        // Featured artists come as contributors; the main artist alone otherwise.
        let artist = if self.contributors.len() > 1 {
            self.contributors.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
        } else {
            self.artist.map(|a| a.name).unwrap_or_else(|| "Desconocido".into())
        };
        Track {
            id: self.id.to_string(),
            source: SOURCE.into(),
            title: self.title,
            artist,
            duration_secs: self.duration.unwrap_or(0),
            artwork_url: self.album.and_then(|a| a.cover_medium).or_else(|| album_cover.map(String::from)),
            genre: None,
        }
    }
}

#[derive(Deserialize)]
struct RawAlbum {
    id: u64,
    title: String,
    cover_medium: Option<String>,
    release_date: Option<String>,
    record_type: Option<String>,
}

impl RawAlbum {
    fn into_summary(self) -> AlbumSummary {
        AlbumSummary {
            id: self.id.to_string(),
            title: self.title,
            cover_url: self.cover_medium,
            release_date: self.release_date,
            kind: self.record_type.unwrap_or_else(|| "album".into()),
        }
    }
}

#[derive(Deserialize)]
struct RawAlbumHit {
    id: u64,
    title: String,
    cover_medium: Option<String>,
    record_type: Option<String>,
    artist: Option<NameOnly>,
}

impl RawAlbumHit {
    fn into_hit(self) -> AlbumHit {
        AlbumHit {
            artist_name: self.artist.map(|a| a.name).unwrap_or_default(),
            album: AlbumSummary {
                id: self.id.to_string(),
                title: self.title,
                cover_url: self.cover_medium,
                release_date: None,
                kind: self.record_type.unwrap_or_else(|| "album".into()),
            },
        }
    }
}

#[derive(Deserialize)]
struct RawPlaylist {
    id: u64,
    title: String,
    picture_medium: Option<String>,
    picture_big: Option<String>,
    nb_tracks: Option<u32>,
    /// `user` in search results, `creator` on the playlist itself.
    #[serde(alias = "creator")]
    user: Option<NameOnly>,
}

impl RawPlaylist {
    fn into_public(self) -> PublicPlaylist {
        PublicPlaylist {
            id: self.id.to_string(),
            title: self.title,
            picture_url: self.picture_big.or(self.picture_medium),
            owner: self.user.map(|u| u.name).unwrap_or_default(),
            track_count: self.nb_tracks.unwrap_or(0),
        }
    }
}

#[derive(Deserialize)]
struct RawPlaylistFull {
    #[serde(flatten)]
    summary: RawPlaylist,
    description: Option<String>,
    duration: Option<u32>,
    tracks: Page<RawTrack>,
}

#[derive(Deserialize)]
struct RawAlbumFull {
    id: u64,
    title: String,
    cover_medium: Option<String>,
    cover_big: Option<String>,
    release_date: Option<String>,
    record_type: Option<String>,
    duration: Option<u32>,
    artist: RawArtist,
    tracks: Page<RawTrack>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artist(id: u64, name: &str, fans: u64) -> RawArtist {
        RawArtist { id, name: name.into(), picture_big: None, picture_medium: None, nb_fan: Some(fans) }
    }

    #[test]
    fn picks_the_most_followed_exact_match() {
        let found = [artist(1, "Rosalía", 269), artist(2, "ROSALÍA", 1_071_752), artist(3, "Rosal-IA", 56)];
        assert_eq!(pick_artist("Rosalía", &found).unwrap().id, 2);
        assert_eq!(pick_artist("Otra cosa", &found).unwrap().id, 1, "no exact match: Deezer's first");
        assert!(pick_artist("x", &[]).is_none());
    }

    #[test]
    fn tracks_list_featured_artists() {
        let raw: RawTrack = serde_json::from_value(serde_json::json!({
            "id": 42, "title": "Dai Dai", "duration": 223,
            "artist": {"name": "Shakira"},
            "contributors": [{"name": "Shakira"}, {"name": "Burna Boy"}],
            "album": {"cover_medium": "https://cdn/cover.jpg"}
        }))
        .unwrap();
        let t = raw.into_track(None);
        assert_eq!((t.source.as_str(), t.id.as_str()), ("deezer", "42"));
        assert_eq!(t.artist, "Shakira, Burna Boy");
        assert_eq!(t.duration_secs, 223);
        assert_eq!(t.artwork_url.as_deref(), Some("https://cdn/cover.jpg"));
    }

    #[test]
    fn album_tracks_take_the_album_cover() {
        let raw: RawTrack = serde_json::from_value(serde_json::json!({
            "id": 7, "title": "Copenhague", "duration": 304, "artist": {"name": "Vetusta Morla"}
        }))
        .unwrap();
        let t = raw.into_track(Some("https://cdn/album.jpg"));
        assert_eq!(t.artist, "Vetusta Morla");
        assert_eq!(t.artwork_url.as_deref(), Some("https://cdn/album.jpg"));
    }

    /// A fake Deezer: `answer(path with query, how many times it was asked)` gives the body.
    async fn fake_deezer(
        answer: fn(&str, usize) -> String,
    ) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = vec![0; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = request.split_whitespace().nth(1).unwrap_or("").to_string();
                let body = answer(&path, counter.fetch_add(1, Ordering::SeqCst));
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
            }
        });
        (url, calls)
    }

    fn songs(ids: std::ops::Range<u32>) -> String {
        let songs: Vec<String> = ids
            .map(|i| {
                format!(r#"{{"id": {i}, "title": "Canción {i}", "duration": 200, "artist": {{"name": "Artista"}}}}"#)
            })
            .collect();
        songs.join(",")
    }

    #[tokio::test]
    async fn a_big_playlist_is_read_whole_not_just_its_first_page() {
        // Deezer embeds only the first songs (400 of 1977 in «Top 2000»): the rest come paged.
        let (url, _) = fake_deezer(|path, _| {
            if path.starts_with("/playlist/7/tracks?index=2") {
                format!(r#"{{"data": [{}], "total": 5}}"#, songs(2..5))
            } else {
                format!(
                    r#"{{"id": 7, "title": "Grande", "nb_tracks": 5, "creator": {{"name": "x"}}, "tracks": {{"data": [{}]}}}}"#,
                    songs(0..2)
                )
            }
        })
        .await;
        let mut service = ArtistService::new(reqwest::Client::new());
        service.api = url;
        let page = service.playlist("7", true).await.unwrap();
        let ids: Vec<_> = page.tracks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["0", "1", "2", "3", "4"]);
    }

    #[tokio::test]
    async fn waits_and_retries_when_deezer_says_quota_exceeded() {
        let (url, calls) = fake_deezer(|_, call| {
            if call < 2 {
                r#"{"error": {"type": "Exception", "message": "Quota limit exceeded", "code": 4}}"#.into()
            } else {
                r#"{"data": []}"#.into()
            }
        })
        .await;
        let mut service = ArtistService::new(reqwest::Client::new());
        service.api = url;
        service.quota_wait = Duration::from_millis(1);
        let page: Page<RawArtist> = service.get("/search/artist").await.unwrap();
        assert!(page.data.is_empty());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3, "two refusals, then the answer");
    }

    #[tokio::test]
    async fn other_deezer_errors_are_not_retried() {
        let (url, calls) =
            fake_deezer(|_, _| r#"{"error": {"type": "DataException", "message": "no data", "code": 800}}"#.into())
                .await;
        let mut service = ArtistService::new(reqwest::Client::new());
        service.api = url;
        service.quota_wait = Duration::from_millis(1);
        let Err(err) = service.get::<Page<RawArtist>>("/artist/0").await else { panic!("should fail") };
        assert!(err.to_string().contains("no data"), "{err}");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn names_compare_without_case_or_accents() {
        assert_eq!(normalize("ROSALÍA"), normalize("Rosalía"));
        assert_eq!(normalize("Love Of Lesbian"), "loveoflesbian");
    }

    #[test]
    fn reads_playlists_from_search_and_page() {
        let hit: RawPlaylist = serde_json::from_value(serde_json::json!({
            "id": 5, "title": "Indie Español", "picture_medium": "https://cdn/p.jpg", "nb_tracks": 309,
            "user": {"name": "dmarinster"}
        }))
        .unwrap();
        let p = hit.into_public();
        assert_eq!((p.id.as_str(), p.owner.as_str(), p.track_count), ("5", "dmarinster", 309));

        let full: RawPlaylistFull = serde_json::from_value(serde_json::json!({
            "id": 5, "title": "Indie Español", "nb_tracks": 1, "creator": {"name": "dmarinster"},
            "description": " ", "duration": 200,
            "tracks": {"data": [{"id": 9, "title": "Copenhague", "duration": 304, "artist": {"name": "Vetusta Morla"}}]}
        }))
        .unwrap();
        assert_eq!(full.summary.user.as_ref().unwrap().name, "dmarinster");
        assert_eq!(full.tracks.data.len(), 1);
    }

    #[tokio::test]
    async fn an_empty_name_is_an_error_even_with_a_cache() {
        let service = ArtistService::new(reqwest::Client::new());
        let page = ArtistPage {
            artist: ArtistRef { id: "1".into(), name: "Alguien".into(), picture_url: None },
            fans: 0,
            top: Vec::new(),
            albums: Vec::new(),
            related: Vec::new(),
        };
        service.artists.lock().unwrap().insert(format!("name:{}", normalize("")), page);
        let Err(err) = service.artist("  ", None).await else { panic!("must fail") };
        assert!(err.to_string().contains("falta el nombre"), "{err}");
    }

    /// Against the real Deezer API: `cargo test -- --ignored artists`
    #[tokio::test]
    #[ignore = "necesita red"]
    async fn opening_a_namesake_does_not_change_who_a_name_points_to() {
        let service = ArtistService::new(reqwest::Client::builder().user_agent("Melomango-tests").build().unwrap());
        let found = service.search("rosalía").await.unwrap();
        let namesake = found.artists.iter().find(|a| a.artist.name != "ROSALÍA").expect("otro artista");
        service.artist("Rosalía", Some(&namesake.artist.id)).await.unwrap();
        assert_eq!(service.artist("Rosalía", None).await.unwrap().artist.name, "ROSALÍA");
    }

    /// Against the real Deezer API: «Top 2000 (2019)» embeds 400 of its 1977 songs.
    #[tokio::test]
    #[ignore = "necesita red"]
    async fn reads_a_real_playlist_bigger_than_its_first_page() {
        let service = ArtistService::new(reqwest::Client::builder().user_agent("Melomango-tests").build().unwrap());
        let page = service.playlist("7038645284", true).await.unwrap();
        assert_eq!(page.tracks.len(), page.playlist.track_count as usize);
        assert!(page.tracks.len() > 400, "{}", page.tracks.len());
    }

    /// Against the real Deezer API: `cargo test -- --ignored artists`
    #[tokio::test]
    #[ignore = "necesita red"]
    async fn loads_a_real_artist_and_album() {
        let service = ArtistService::new(reqwest::Client::builder().user_agent("Melomango-tests").build().unwrap());
        let page = service.artist("vetusta morla", None).await.unwrap();
        assert_eq!(page.artist.name, "Vetusta Morla");
        assert!(page.fans > 1000);
        assert_eq!(page.top.len(), TOP_TRACKS as usize);
        assert!(page.top.iter().all(|t| t.source == SOURCE && t.duration_secs > 0));
        assert!(!page.albums.is_empty());
        let album = service.album(&page.albums[0].id).await.unwrap();
        assert!(!album.tracks.is_empty());
        assert_eq!(album.artist.name, "Vetusta Morla");
        // Cached: the same page again without asking Deezer.
        assert_eq!(service.artist("Vetusta Morla", None).await.unwrap().artist.id, page.artist.id);

        let found = service.search("rosalía").await.unwrap();
        assert_eq!(found.artists[0].artist.name, "ROSALÍA", "the exact name first");
        assert!(found.albums.iter().any(|a| a.artist_name == "ROSALÍA"));
        assert!(!found.playlists.is_empty());
        let playlist = service.playlist(&found.playlists[0].id, false).await.unwrap();
        assert!(!playlist.tracks.is_empty());

        let hints = service.suggest("vetusta mo").await.unwrap();
        assert_eq!(hints.artists[0].name, "Vetusta Morla");
        assert!(!hints.tracks.is_empty());
    }
}
