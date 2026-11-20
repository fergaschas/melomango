//! CSV playlists, as exported by Exportify (https://exportify.app): one CSV per
//! playlist (`liked.csv` for Liked Songs) in `spotify_playlists.zip`, with a header row
//! in the language the user browsed Exportify in. Other tools' CSVs with similar
//! columns (title, artist, optionally a playlist name) work too.
//!
//! Unlike Spotify's own data export, these include durations and album covers.

use anyhow::{Context, bail};

use crate::{models::Track, spotify_export::track};

/// Column titles in Exportify's languages (from its translation files) plus common
/// ones from other tools, compared lowercased.
const TRACK_URI: &[&str] = &[
    "track uri",
    "track-uri",
    "låtens uri",
    "nummer uri",
    "parça uri",
    "uri da faixa",
    "uri de la canción",
    "uri della traccia",
    "uri du titre",
    "uri κομματιού",
    "رابط الأغنية (uri)",
    "トラックuri",
    "曲目 uri",
    "歌 link",
    "spotify uri",
    "uri",
    "spotify - id",
];
const TRACK_NAME: &[&str] = &[
    "track name",
    "track-name",
    "låtens namn",
    "nom du titre",
    "nombre de la canción",
    "nome da faixa",
    "nome della traccia",
    "nummernaam",
    "parça adı",
    "όνομα κομματιού",
    "اسم الأغنية",
    "トラック名",
    "曲目名称",
    "曲目名稱",
    "歌名",
    "title",
    "song",
    "song name",
    "track",
    "name",
];
const ARTIST_NAMES: &[&str] = &[
    "artist name(s)",
    "artist 名",
    "artistens namn",
    "künstlername(n)",
    "naam van artiest",
    "nom(s) de l'artiste",
    "nombre(s) del artista",
    "nome dell'artista",
    "nome(s) do artista",
    "sanatçı adı",
    "όνομα/τα καλλιτέχνη",
    "أسماء الفنانين",
    "アーティスト名",
    "艺人名称",
    "藝人名稱",
    "artist name",
    "artist",
    "artists",
    "artist(s)",
];
const DURATION_MS: &[&str] = &[
    "track duration (ms)",
    "track-dauer (ms)",
    "duración de la canción (ms)",
    "durata della traccia (ms)",
    "duração da faixa (ms)",
    "durée du titre (ms)",
    "låtlängd (ms)",
    "nummerduur (ms)",
    "parça süresi (ms)",
    "διάρκεια κομματιού (ms)",
    "مدة الأغنية (بالمللي ثانية)",
    "トラックの長さ（ミリ秒）",
    "曲目时长（毫秒）",
    "曲目時長（毫秒）",
    "曲目長度（毫秒）",
    "duration (ms)",
    "duration_ms",
];
const IMAGE_URL: &[&str] = &[
    "album image url",
    "album afbeelding-url",
    "album-bild-url",
    "albumets bild-url",
    "albüm resim url'si",
    "url da imagem do álbum",
    "url de l'image de l'album",
    "url de la imagen del álbum",
    "url dell'immagine dell'album",
    "url εικόνας άλμπουμ",
    "رابط صورة الألبوم",
    "アルバム画像url",
    "专辑封面 url",
    "專輯封面 url",
    "專輯封面條 link",
];
const PLAYLIST_NAME: &[&str] = &["playlist name", "playlist", "nombre de la playlist"];

pub enum Content {
    /// Exportify's `liked.csv`.
    Liked(Vec<Track>),
    /// (name, tracks) for each playlist in the file.
    Playlists(Vec<(String, Vec<Track>)>),
    /// A CSV with no tracks to import (e.g. Exportify's `saved_albums.csv`).
    Ignored,
}

pub fn read(file_name: &str, data: &[u8]) -> anyhow::Result<Content> {
    let stem = file_stem(file_name);
    let text = decode(data);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut reader = csv::ReaderBuilder::new().flexible(true).delimiter(delimiter(text)).from_reader(text.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .with_context(|| format!("{file_name} no es un CSV válido"))?
        .iter()
        .map(|h| h.trim().to_lowercase())
        .collect();
    let rows: Vec<csv::StringRecord> =
        reader.records().collect::<Result<_, _>>().with_context(|| format!("{file_name} no es un CSV válido"))?;

    let column = |aliases: &[&str]| aliases.iter().find_map(|a| headers.iter().position(|h| h == a));
    let (Some(name), Some(artist)) = (column(TRACK_NAME), column(ARTIST_NAMES)) else {
        if stem.eq_ignore_ascii_case("saved_albums")
            || headers.iter().any(|h| h.contains("album") && !h.contains("track"))
        {
            return Ok(Content::Ignored);
        }
        bail!("no reconozco las columnas de {file_name}: necesita al menos el título y el artista de cada canción");
    };
    // Fall back to whichever column holds Spotify track URIs.
    let uri = column(TRACK_URI).or_else(|| {
        (0..headers.len()).find(|&i| rows.iter().any(|r| r.get(i).is_some_and(|v| v.starts_with("spotify:"))))
    });
    let duration = column(DURATION_MS);
    let image = column(IMAGE_URL);
    let playlist = column(PLAYLIST_NAME);

    let field = |row: &csv::StringRecord, i: Option<usize>| i.and_then(|i| row.get(i)).unwrap_or("").trim().to_string();
    let mut groups: Vec<(String, Vec<Track>)> = Vec::new();
    for row in &rows {
        // Exportify joins several artists with ", " and escapes commas inside names as "\,".
        let artists = field(row, Some(artist)).replace("\\,", ",");
        let secs = field(row, duration).parse::<f64>().map_or(0, |ms| (ms / 1000.0).round() as u32);
        let Some(mut t) = track(&field(row, uri), &field(row, Some(name)), &artists, secs) else { continue };
        t.artwork_url = Some(field(row, image)).filter(|u| u.starts_with("https://"));

        let group = playlist.map(|p| field(row, Some(p))).filter(|p| !p.is_empty()).unwrap_or_default();
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, tracks)) => tracks.push(t),
            None => groups.push((group, vec![t])),
        }
    }

    if playlist.is_none() && stem.eq_ignore_ascii_case("liked") {
        return Ok(Content::Liked(groups.pop().map(|(_, t)| t).unwrap_or_default()));
    }
    Ok(Content::Playlists(
        groups
            .into_iter()
            .map(|(group, tracks)| (if group.is_empty() { display_name(stem) } else { group }, tracks))
            .collect(),
    ))
}

/// The text of a CSV: UTF-8, or else Windows-1252, what Excel saves with Spanish (and most
/// Western European) settings as «CSV (delimitado por comas)».
fn decode(data: &[u8]) -> std::borrow::Cow<'_, str> {
    if let Ok(text) = std::str::from_utf8(data) {
        return text.into();
    }
    /// 0x80..=0x9F in Windows-1252 (the rest of the bytes are the same as Latin-1).
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘',
        '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    data.iter()
        .map(|&b| if (0x80..0xa0).contains(&b) { HIGH[(b - 0x80) as usize] } else { b as char })
        .collect::<String>()
        .into()
}

/// The separator of a CSV, from its header row: `,`, or `;` (Excel with Spanish settings)
/// or a tab, whichever appears most outside quotes.
fn delimiter(text: &str) -> u8 {
    let header = text.lines().next().unwrap_or("");
    let mut counts = [0usize; 3];
    let mut quoted = false;
    for c in header.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => counts[0] += 1,
            ';' if !quoted => counts[1] += 1,
            '\t' if !quoted => counts[2] += 1,
            _ => {}
        }
    }
    let best = (0..3).max_by_key(|&i| (counts[i], std::cmp::Reverse(i))).unwrap_or(0);
    if counts[best] == 0 { b',' } else { [b',', b';', b'\t'][best] }
}

fn file_stem(file_name: &str) -> &str {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    base.rsplit_once('.').map_or(base, |(stem, _)| stem)
}

/// Exportify names files after the playlist, lowercased with `_` for spaces and
/// punctuation: "música_para_correr" -> "Música para correr".
fn display_name(stem: &str) -> String {
    let words = stem.replace('_', " ");
    let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Playlist importada".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: &str = "\"Track URI\",\"Track Name\",\"Artist URI(s)\",\"Artist Name(s)\",\"Album Name\",\"Album Image URL\",\"Track Duration (ms)\",\"Added By\",\"Added At\"\n\
        \"spotify:track:1\",\"Saoko\",\"spotify:artist:a\",\"ROSALÍA\",\"MOTOMAMI\",\"https://i.scdn.co/image/abc\",\"137875\",\"\",\"2026-01-01T00:00:00Z\"\n\
        \"spotify:track:2\",\"Get Lucky (feat. Pharrell Williams)\",\"spotify:artist:b, spotify:artist:c\",\"Daft Punk, Pharrell Williams\",\"RAM\",\"https://i.scdn.co/image/def\",\"248413\",\"\",\"\"\n\
        \"spotify:track:3\",\"Rap\",\"spotify:artist:d\",\"Tyler\\, The Creator, Kali Uchis\",\"X\",\"\",\"180000\",\"\",\"\"\n";

    fn playlists(content: Content) -> Vec<(String, Vec<Track>)> {
        match content {
            Content::Playlists(p) => p,
            _ => panic!("se esperaban playlists"),
        }
    }

    #[test]
    fn reads_an_exportify_playlist() {
        let lists = playlists(read("música_para_correr.csv", EN.as_bytes()).unwrap());
        assert_eq!(lists.len(), 1);
        let (name, tracks) = &lists[0];
        assert_eq!(name, "Música para correr");
        assert_eq!(tracks.len(), 3);
        assert_eq!((tracks[0].source.as_str(), tracks[0].id.as_str()), ("spotify", "1"));
        assert_eq!(tracks[0].duration_secs, 138);
        assert_eq!(tracks[0].artwork_url.as_deref(), Some("https://i.scdn.co/image/abc"));
        assert_eq!(tracks[1].artist, "Daft Punk, Pharrell Williams");
        assert_eq!(tracks[2].artist, "Tyler, The Creator, Kali Uchis");
        assert_eq!(tracks[2].artwork_url, None);
    }

    #[test]
    fn liked_csv_is_liked_songs() {
        assert!(matches!(read("liked.csv", EN.as_bytes()).unwrap(), Content::Liked(t) if t.len() == 3));
    }

    #[test]
    fn reads_headers_in_other_languages() {
        let es = "\u{feff}\"URI de la canción\",\"Nombre de la canción\",\"Nombre(s) del artista\",\"Duración de la canción (ms)\"\n\
            \"spotify:track:9\",\"Copenhague\",\"Vetusta Morla\",\"304000\"\n";
        let lists = playlists(read("indie.csv", es.as_bytes()).unwrap());
        let t = &lists[0].1[0];
        assert_eq!(
            (t.id.as_str(), t.title.as_str(), t.artist.as_str(), t.duration_secs),
            ("9", "Copenhague", "Vetusta Morla", 304)
        );
    }

    #[test]
    fn groups_by_a_playlist_column() {
        let csv = "Playlist name,Track name,Artist name\nVerano,A,X\nInvierno,B,Y\nVerano,C,Z\n";
        let lists = playlists(read("tunemymusic.csv", csv.as_bytes()).unwrap());
        let names: Vec<_> = lists.iter().map(|(n, t)| (n.as_str(), t.len())).collect();
        assert_eq!(names, [("Verano", 2), ("Invierno", 1)]);
        // Without Spotify URIs, ids are stable fingerprints.
        assert!(lists[0].1[0].id.starts_with("local:"));
    }

    #[test]
    fn skips_saved_albums_and_rejects_unknown_files() {
        let albums = "\"Album URI\",\"Album Name\",\"Album Artist Name(s)\",\"Album Type\"\n\"spotify:album:1\",\"RAM\",\"Daft Punk\",\"album\"\n";
        assert!(matches!(read("saved_albums.csv", albums.as_bytes()).unwrap(), Content::Ignored));
        assert!(read("gastos.csv", "fecha,importe\n2026-01-01,3\n".as_bytes()).is_err());
    }

    #[test]
    fn reads_a_csv_saved_by_spanish_excel() {
        // Excel with Spanish settings: «;» between columns, and Windows-1252 instead of UTF-8.
        let text = "Track URI;Track Name;Artist Name(s)\r\nspotify:track:1;Canción del Mariachi;Los Lobos, Antonio Banderas\r\nspotify:track:2;Pájaros de barro;Manolo García\r\n";
        let cp1252: Vec<u8> = text.chars().map(|c| u8::try_from(u32::from(c)).expect("Latin-1 text")).collect();
        assert!(std::str::from_utf8(&cp1252).is_err(), "really not UTF-8");
        let lists = playlists(read("para_la_carretera.csv", &cp1252).unwrap());
        let tracks = &lists[0].1;
        assert_eq!(tracks.len(), 2);
        assert_eq!(
            (tracks[0].title.as_str(), tracks[0].artist.as_str()),
            ("Canción del Mariachi", "Los Lobos, Antonio Banderas")
        );
        assert_eq!(tracks[1].title, "Pájaros de barro");
    }

    #[test]
    fn reads_tab_separated_files_and_windows_1252_punctuation() {
        let mut data = b"Track Name\tArtist Name(s)\n".to_vec();
        data.extend_from_slice(b"Don\x92t Stop Me Now\tQueen\n");
        let t = &playlists(read("x.csv", &data).unwrap())[0].1[0];
        assert_eq!((t.title.as_str(), t.artist.as_str()), ("Don’t Stop Me Now", "Queen"));
    }

    #[test]
    fn picks_the_separator_outside_quotes() {
        assert_eq!(delimiter("\"a;b\",c,d\n"), b',');
        assert_eq!(delimiter("a;b;c\n"), b';');
        assert_eq!(delimiter("a\tb\n"), b'\t');
        assert_eq!(delimiter("solo\n"), b',');
    }
}
