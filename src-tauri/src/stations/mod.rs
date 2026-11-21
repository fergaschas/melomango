//! Live radio stations («Emisoras»), from the free [Radio Browser](https://www.radio-browser.info/)
//! directory. The design, shared with the mobile app, is in `docs/emisoras.md`.
//!
//! A station is not a `Track`: it has no length, isn't downloaded or matched to YouTube and
//! doesn't count as a play. Its audio goes through the `stream://` protocol (see [`live`]).

pub mod live;
pub mod location;

use std::{
    collections::HashMap,
    hash::{BuildHasher, Hasher},
    sync::Mutex,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::text::fold;

/// Lists every Radio Browser server. They ask apps to pick one from here, never a fixed one.
const SERVERS_URL: &str = "https://all.api.radio-browser.info/json/servers";
const SERVERS_FOR: Duration = Duration::from_secs(24 * 60 * 60);
/// Lists of stations change slowly: the same screen opened again doesn't ask again.
const LIST_FOR: Duration = Duration::from_secs(10 * 60);
const MAX_LISTS: usize = 64;
/// «Cerca de ti»: stations within this many meters.
const NEAR_METERS: u32 = 50_000;

/// A station, as the app shows and saves it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Station {
    /// Radio Browser's `stationuuid`: the key.
    pub uuid: String,
    pub name: String,
    /// The stream itself (`url_resolved`).
    pub url: String,
    pub homepage: Option<String>,
    /// Only `https://` logos: the window's CSP blocks the rest.
    pub favicon: Option<String>,
    /// ISO 3166-1 code ("ES").
    pub country: String,
    /// Region as the station wrote it («Catalunya», «Catalonia»…): only to show.
    pub state: Option<String>,
    /// Lowercase genres.
    pub tags: Vec<String>,
    /// «MP3», «AAC»… Empty when unknown.
    pub codec: String,
    /// kbps; 0 when unknown.
    pub bitrate: u32,
    pub lat: Option<f64>,
    pub long: Option<f64>,
    /// Meters from the user, only in «Cerca de ti».
    pub distance_m: Option<f64>,
}

/// A country of the selector, with how many stations it has.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Country {
    /// ISO 3166-1 code ("ES").
    pub code: String,
    /// In English, as Radio Browser has it (the app shows its own name).
    pub name: String,
    pub stations: u32,
}

/// A station as Radio Browser sends it (only what the app uses; many fields can be null).
#[derive(Debug, Deserialize)]
struct ApiStation {
    stationuuid: String,
    name: Option<String>,
    url_resolved: Option<String>,
    homepage: Option<String>,
    favicon: Option<String>,
    countrycode: Option<String>,
    state: Option<String>,
    tags: Option<String>,
    codec: Option<String>,
    bitrate: Option<i64>,
    hls: Option<i64>,
    clickcount: Option<i64>,
    geo_lat: Option<f64>,
    geo_long: Option<f64>,
    geo_distance: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ApiCountry {
    name: String,
    iso_3166_1: String,
    stationcount: i64,
}

#[derive(Debug, Deserialize)]
struct ApiServer {
    name: String,
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// What the app keeps of a response: no HLS (it doesn't play them yet), nothing without a
/// stream, and each stream once (the one with more listeners). `distances`: the request
/// was «Cerca de ti».
fn clean(raw: Vec<ApiStation>, distances: bool) -> Vec<Station> {
    let mut stations: Vec<(Station, i64)> = Vec::new();
    let mut by_url: HashMap<String, usize> = HashMap::new();
    for s in raw {
        let (Some(url), Some(name)) = (non_empty(s.url_resolved), non_empty(s.name)) else { continue };
        if s.hls == Some(1) {
            continue;
        }
        let clicks = s.clickcount.unwrap_or(0);
        let mut tags: Vec<String> = Vec::new();
        for tag in s.tags.unwrap_or_default().split(',') {
            let tag = tag.trim().to_lowercase();
            if !tag.is_empty() && !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        let codec = s.codec.unwrap_or_default().trim().to_uppercase();
        let station = Station {
            uuid: s.stationuuid,
            name,
            url: url.clone(),
            homepage: non_empty(s.homepage),
            favicon: non_empty(s.favicon).filter(|f| f.starts_with("https://")),
            country: s.countrycode.unwrap_or_default().trim().to_uppercase(),
            state: non_empty(s.state),
            tags,
            codec: if codec == "UNKNOWN" { String::new() } else { codec },
            bitrate: s.bitrate.unwrap_or(0).clamp(0, u32::MAX as i64) as u32,
            lat: s.geo_lat,
            long: s.geo_long,
            distance_m: if distances { s.geo_distance } else { None },
        };
        match by_url.get(&url) {
            Some(&i) if stations[i].1 < clicks => stations[i] = (station, clicks),
            Some(_) => {}
            None => {
                by_url.insert(url, stations.len());
                stations.push((station, clicks));
            }
        }
    }
    stations.into_iter().map(|(s, _)| s).collect()
}

/// The words of a search, without accents or case: «SER Miranda» -> ["ser", "miranda"].
fn words(query: &str) -> Vec<String> {
    fold(query).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// Radio Browser finds the text only as written, in one piece: it is asked for the longest
/// word and the app keeps the stations whose name has every word.
fn has_all_words(name: &str, words: &[String]) -> bool {
    let name = fold(name);
    words.iter().all(|w| name.contains(w.as_str()))
}

fn longest(words: &[String]) -> Option<&str> {
    words.iter().max_by_key(|w| w.chars().count()).map(String::as_str)
}

/// About 1 km: the most the app ever sends of where the user is.
pub fn round_coord(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn country_code(code: &str) -> anyhow::Result<String> {
    let code = code.trim().to_ascii_uppercase();
    if code.len() != 2 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
        bail!("código de país no válido: {code}");
    }
    Ok(code)
}

/// Station ids go into a URL path: only what a uuid has.
fn check_uuid(uuid: &str) -> anyhow::Result<&str> {
    if uuid.is_empty() || uuid.len() > 64 || !uuid.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        bail!("identificador de emisora no válido");
    }
    Ok(uuid)
}

/// Radio Browser servers in random order, so each app spreads its requests.
fn shuffled(mut servers: Vec<String>) -> Vec<String> {
    servers.sort();
    servers.dedup();
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    for i in (1..servers.len()).rev() {
        hasher.write_usize(i);
        let j = (hasher.finish() % (i as u64 + 1)) as usize;
        servers.swap(i, j);
    }
    servers
}

fn encode(text: &str) -> String {
    utf8_percent_encode(text, NON_ALPHANUMERIC).to_string()
}

/// The Radio Browser client: picks a server, moves to the next one if it fails, and
/// remembers lists for a few minutes.
pub struct RadioBrowser {
    http: reqwest::Client,
    servers: Mutex<Option<(Instant, Vec<String>)>>,
    lists: Mutex<HashMap<String, (Instant, Vec<Station>)>>,
}

impl RadioBrowser {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, servers: Mutex::default(), lists: Mutex::default() }
    }

    async fn servers(&self) -> anyhow::Result<Vec<String>> {
        let known = self.servers.lock().unwrap().clone();
        if let Some((at, list)) = &known
            && at.elapsed() < SERVERS_FOR
        {
            return Ok(list.clone());
        }
        let fetched: anyhow::Result<Vec<ApiServer>> =
            async { Ok(self.http.get(SERVERS_URL).send().await?.error_for_status()?.json().await?) }.await;
        match fetched {
            Ok(list) if !list.is_empty() => {
                let names = shuffled(list.into_iter().map(|s| s.name).collect());
                *self.servers.lock().unwrap() = Some((Instant::now(), names.clone()));
                Ok(names)
            }
            // The list of servers is down: the old one still works.
            _ if known.is_some() => Ok(known.unwrap().1),
            Ok(_) => Err(anyhow!("Radio Browser no tiene servidores")),
            Err(e) => Err(e.context("no se pudo pedir la lista de servidores de Radio Browser")),
        }
    }

    /// `path` (with its query) from the first server that answers.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let mut last = None;
        for server in self.servers().await? {
            let answer: anyhow::Result<T> = async {
                Ok(self.http.get(format!("https://{server}{path}")).send().await?.error_for_status()?.json().await?)
            }
            .await;
            match answer {
                Ok(value) => return Ok(value),
                Err(e) => last = Some(e.context(format!("el servidor {server} falló"))),
            }
        }
        Err(last.unwrap_or_else(|| anyhow!("no hay servidores de Radio Browser")))
    }

    async fn stations(&self, path: &str, distances: bool) -> anyhow::Result<Vec<Station>> {
        if let Some((at, list)) = self.lists.lock().unwrap().get(path)
            && at.elapsed() < LIST_FOR
        {
            return Ok(list.clone());
        }
        let raw: Vec<ApiStation> = self.get(path).await?;
        let list = clean(raw, distances);
        let mut lists = self.lists.lock().unwrap();
        if lists.len() >= MAX_LISTS {
            lists.retain(|_, (at, _)| at.elapsed() < LIST_FOR);
            if lists.len() >= MAX_LISTS {
                lists.clear();
            }
        }
        lists.insert(path.to_string(), (Instant::now(), list.clone()));
        Ok(list)
    }

    /// «Cerca de ti»: within 50 km, the closest first.
    pub async fn near(&self, lat: f64, long: f64) -> anyhow::Result<Vec<Station>> {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&long) {
            bail!("ubicación no válida");
        }
        let path = format!(
            "/json/stations/search?geo_lat={:.2}&geo_long={:.2}&geo_distance={NEAR_METERS}&hidebroken=true&limit=100",
            round_coord(lat),
            round_coord(long),
        );
        let mut stations = self.stations(&path, true).await?;
        stations.sort_by(|a, b| a.distance_m.unwrap_or(f64::MAX).total_cmp(&b.distance_m.unwrap_or(f64::MAX)));
        Ok(stations)
    }

    /// «Lo más escuchado en {país}».
    pub async fn top(&self, country: &str) -> anyhow::Result<Vec<Station>> {
        let country = country_code(country)?;
        self.stations(
            &format!(
                "/json/stations/search?countrycode={country}&order=clickcount&reverse=true&hidebroken=true&limit=50"
            ),
            false,
        )
        .await
    }

    /// By name (every word, in any order) or, if no name matches, by genre.
    pub async fn search(&self, query: &str, country: Option<&str>) -> anyhow::Result<Vec<Station>> {
        let words = words(query);
        let Some(longest) = longest(&words) else { return Ok(Vec::new()) };
        let country = match country {
            Some(c) => format!("&countrycode={}", country_code(c)?),
            None => String::new(),
        };
        let tail = format!("{country}&order=clickcount&reverse=true&hidebroken=true&limit=100");
        let by_name = self.stations(&format!("/json/stations/search?name={}{tail}", encode(longest)), false).await?;
        let found: Vec<Station> = by_name.into_iter().filter(|s| has_all_words(&s.name, &words)).collect();
        if !found.is_empty() {
            return Ok(found);
        }
        let genre = words.join(" ");
        self.stations(&format!("/json/stations/search?tag={}{tail}", encode(&genre)), false).await
    }

    /// Countries with stations, for the selector.
    pub async fn countries(&self) -> anyhow::Result<Vec<Country>> {
        let raw: Vec<ApiCountry> = self.get("/json/countries").await?;
        Ok(countries(raw))
    }

    /// The station as it is now (its stream may have moved). None if it is gone or is HLS.
    pub async fn by_uuid(&self, uuid: &str) -> anyhow::Result<Option<Station>> {
        let raw: Vec<ApiStation> = self.get(&format!("/json/stations/byuuid/{}", check_uuid(uuid)?)).await?;
        Ok(clean(raw, false).into_iter().find(|s| s.uuid == uuid))
    }

    /// Counts a listen for the station's popularity (once a day per address). Nobody waits
    /// for it, and a failure doesn't matter.
    pub async fn click(&self, uuid: &str) {
        let Ok(uuid) = check_uuid(uuid) else { return };
        let _ = self.get::<serde_json::Value>(&format!("/json/url/{uuid}")).await;
    }
}

fn countries(raw: Vec<ApiCountry>) -> Vec<Country> {
    let mut list: Vec<Country> = raw
        .into_iter()
        .filter(|c| c.stationcount > 0)
        .filter_map(|c| {
            let code = country_code(&c.iso_3166_1).ok()?;
            Some(Country { code, name: c.name, stations: c.stationcount.min(u32::MAX as i64) as u32 })
        })
        .collect();
    list.sort_by(|a, b| a.code.cmp(&b.code));
    list.dedup_by(|a, b| a.code == b.code);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<ApiStation> {
        let text = match name {
            "top" => include_str!("fixtures/top.json"),
            "near" => include_str!("fixtures/near.json"),
            "search" => include_str!("fixtures/search.json"),
            _ => unreachable!(),
        };
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn reads_a_saved_answer_and_leaves_out_hls() {
        let raw = fixture("top");
        assert_eq!(raw.len(), 13);
        let stations = clean(raw, false);
        assert_eq!(stations.len(), 12, "the HLS one is left out");
        let first = &stations[0];
        assert_eq!(first.name, "Cadena 100");
        assert_eq!(first.uuid, "960c37c6-0601-11e8-ae97-52543be04c81");
        assert_eq!(first.url, "http://cadena100-streamers-mp3.flumotion.com/cope/cadena100.mp3");
        assert_eq!(first.country, "ES");
        assert_eq!((first.codec.as_str(), first.bitrate), ("MP3", 128));
        assert_eq!(first.tags, ["music"]);
        assert_eq!(first.state, None, "empty region");
        assert_eq!((first.lat, first.distance_m), (None, None));
        assert!(stations.iter().all(|s| s.favicon.as_deref().is_none_or(|f| f.starts_with("https://"))));
    }

    #[test]
    fn near_keeps_the_distance() {
        let stations = clean(fixture("near"), true);
        assert_eq!(stations.len(), 6);
        let first = &stations[0];
        assert_eq!(first.name, "972'CostaBrava");
        assert_eq!(first.state.as_deref(), Some("Catalonia"));
        assert_eq!(first.codec, "AAC");
        assert!(first.distance_m.unwrap() < 200.0);
        assert!(first.lat.is_some() && first.long.is_some());
        assert_eq!(first.tags[0], "ambient lounge");
    }

    #[test]
    fn each_stream_once_the_most_listened() {
        let mut raw = fixture("search");
        assert_eq!(raw.len(), 1);
        let mut copy: ApiStation = serde_json::from_str(
            r#"{"stationuuid":"copy","name":"SER Miranda (copia)","url_resolved":"","clickcount":1000000}"#,
        )
        .unwrap();
        copy.url_resolved = raw[0].url_resolved.clone();
        raw.push(copy);
        raw.push(
            serde_json::from_str(r#"{"stationuuid":"other","name":"Otra","url_resolved":"http://other"}"#).unwrap(),
        );
        let stations = clean(raw, false);
        let ids: Vec<&str> = stations.iter().map(|s| s.uuid.as_str()).collect();
        assert_eq!(ids, ["copy", "other"], "the one with more listeners stays, in the first one's place");
    }

    #[test]
    fn unknown_codec_and_missing_fields() {
        let raw: Vec<ApiStation> = serde_json::from_str(
            r#"[{"stationuuid":"a","name":"  Ràdio  ","url_resolved":"http://x","codec":"UNKNOWN","bitrate":-1,
                 "tags":"Rock, rock,,Pop ","favicon":"http://insecure/logo.png","hls":0},
                {"stationuuid":"b","name":"Sin stream","url_resolved":""},
                {"stationuuid":"c","name":null,"url_resolved":"http://y"}]"#,
        )
        .unwrap();
        let stations = clean(raw, false);
        assert_eq!(stations.len(), 1);
        let s = &stations[0];
        assert_eq!(s.name, "Ràdio");
        assert_eq!((s.codec.as_str(), s.bitrate), ("", 0));
        assert_eq!(s.tags, ["rock", "pop"]);
        assert_eq!(s.favicon, None, "http logos are blocked by the CSP");
    }

    #[test]
    fn search_needs_every_word_in_any_order() {
        let q = words("SER Miranda");
        assert_eq!(q, ["ser", "miranda"]);
        assert_eq!(longest(&q), Some("miranda"));
        assert!(has_all_words("Cadena SER - Miranda de Ebro", &q));
        assert!(!has_all_words("Radio Miranda", &q));
        let q = words("radio girona");
        assert!(has_all_words("Taronja ràdio Girona", &q));
        assert!(words("  -- ").is_empty());
    }

    #[test]
    fn countries_have_valid_codes_and_stations() {
        let raw: Vec<ApiCountry> = serde_json::from_str(include_str!("fixtures/countries.json")).unwrap();
        let mut list = countries(raw);
        assert!(list.iter().any(|c| c.code == "ES" && c.stations > 1000));
        list.retain(|c| c.code.len() == 2);
        assert!(list.windows(2).all(|w| w[0].code < w[1].code));
        let odd = vec![
            ApiCountry { name: "Nowhere".into(), iso_3166_1: "".into(), stationcount: 3 },
            ApiCountry { name: "Empty".into(), iso_3166_1: "XX".into(), stationcount: 0 },
        ];
        assert!(countries(odd).is_empty());
    }

    #[test]
    fn coordinates_are_rounded_to_about_a_kilometre() {
        assert_eq!(round_coord(41.98365), 41.98);
        assert_eq!(round_coord(-3.70379), -3.7);
        assert_eq!(round_coord(2.825), 2.83);
    }

    #[test]
    fn only_safe_ids_and_codes_go_into_urls() {
        assert!(check_uuid("960c37c6-0601-11e8-ae97-52543be04c81").is_ok());
        assert!(check_uuid("../json/servers").is_err());
        assert!(check_uuid("").is_err());
        assert_eq!(country_code("es").unwrap(), "ES");
        assert!(country_code("ESP").is_err());
        assert!(country_code("E&").is_err());
    }

    #[test]
    fn servers_are_shuffled_without_losing_any() {
        let servers: Vec<String> = ["a", "b", "c", "d", "b"].map(String::from).to_vec();
        let mut out = shuffled(servers);
        out.sort();
        assert_eq!(out, ["a", "b", "c", "d"]);
    }

    /// Asks the real Radio Browser. `cargo test -- --ignored`
    #[tokio::test]
    #[ignore = "necesita red"]
    async fn asks_radio_browser() {
        let http = reqwest::Client::builder().user_agent("Melomango-tests").build().unwrap();
        let rb = RadioBrowser::new(http);
        let top = rb.top("ES").await.unwrap();
        assert!(top.len() > 20);
        let found = rb.search("SER Miranda", Some("ES")).await.unwrap();
        assert!(found.iter().any(|s| s.name.contains("Miranda")), "{found:?}");
        let near = rb.near(41.98, 2.82).await.unwrap();
        assert!(near.windows(2).all(|w| w[0].distance_m <= w[1].distance_m));
        let again = rb.by_uuid(&top[0].uuid).await.unwrap().unwrap();
        assert_eq!(again.uuid, top[0].uuid);
        assert!(rb.countries().await.unwrap().iter().any(|c| c.code == "ES"));
    }
}
