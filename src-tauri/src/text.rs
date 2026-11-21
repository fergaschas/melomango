//! Text helpers shared by the modules that compare and identify songs and artists
//! (matcher, radio, lyrics, artist pages, history, imports), so they all read a name the
//! same way.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// "Shakira, Burna Boy" / "Bad Bunny & Drake" / "Daft Punk feat. Pharrell" / "A x B"
/// -> the first artist.
pub fn main_artist(artist: &str) -> &str {
    // ASCII lowercasing keeps byte positions, so `cut` is valid in `artist` too.
    let lower = artist.to_ascii_lowercase();
    let cut = [",", "&", " x ", " feat", " ft.", " ft ", " with "]
        .iter()
        .filter_map(|sep| lower.find(sep))
        .min()
        .unwrap_or(artist.len());
    artist[..cut].trim()
}

/// "Shakira, Burna Boy" / "A & B" / "A feat. B" -> each artist (empty names left out).
pub fn split_artists(artist: &str) -> Vec<&str> {
    // ASCII-only lowercasing keeps every byte where it was, so positions in `lower` are valid
    // in `artist` too. `to_lowercase` can make a letter longer («İ» -> «i̇») and then slicing
    // cuts a character in half. The separators are ASCII, so nothing else is needed.
    let lower = artist.to_ascii_lowercase();
    let mut parts = Vec::new();
    let mut start = 0;
    let separators = [", ", ",", " & ", " feat. ", " ft. ", " featuring "];
    let mut i = 0;
    while i < artist.len() {
        if let Some(sep) = separators.iter().find(|s| lower[i..].starts_with(*s)) {
            parts.push(artist[start..i].trim());
            i += sep.len();
            start = i;
        } else {
            i += artist[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    parts.push(artist[start..].trim());
    parts.into_iter().filter(|p| !p.is_empty()).collect()
}

/// "Song (feat. Someone)" -> "Song". Lyrics databases usually list the bare title.
pub fn trim_featuring(title: &str) -> &str {
    let lower = title.to_ascii_lowercase();
    ["(feat", "(ft.", "[feat", "[ft.", " feat. ", " ft. "]
        .iter()
        .filter_map(|marker| lower.find(marker))
        .min()
        .map_or(title, |cut| title[..cut].trim_end())
}

/// Lowercase without accents: «ROSALÍA» -> «rosalia», «Ça» -> «ca», «Møre» -> «more».
///
/// Accents go by Unicode decomposition (NFD, then dropping the combining marks), so any
/// accented letter works, not just the ones in a hand-written table. A few letters are
/// not «base + accent» in Unicode and are mapped by hand.
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .map(|c| match c {
            'ø' => 'o',
            'ł' => 'l',
            'đ' => 'd',
            'ı' => 'i',
            other => other,
        })
        .collect()
}

/// Short stable hash (FNV-1a, 16 hex digits): the same text always gives the same id.
/// Used for ids of songs without one and for download file names, so it must never change.
pub fn fingerprint(text: &str) -> String {
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_artist_takes_the_first() {
        assert_eq!(main_artist("Shakira, Burna Boy"), "Shakira");
        assert_eq!(main_artist("Bad Bunny & Drake"), "Bad Bunny");
        assert_eq!(main_artist("Daft Punk feat. Pharrell Williams"), "Daft Punk");
        assert_eq!(main_artist("Daft Punk ft. Pharrell Williams"), "Daft Punk");
        assert_eq!(main_artist("Daft Punk FT Pharrell Williams"), "Daft Punk");
        assert_eq!(main_artist("Peso Pluma x Anitta"), "Peso Pluma");
        assert_eq!(main_artist("Eminem featuring Rihanna"), "Eminem");
        assert_eq!(main_artist("Rosalía"), "Rosalía");
        // Letters that grow when lowercased don't break the cut.
        assert_eq!(main_artist("İbrahim Tatlıses feat. X"), "İbrahim Tatlıses");
    }

    #[test]
    fn splits_artist_lines() {
        assert_eq!(split_artists("Shakira, Burna Boy"), ["Shakira", "Burna Boy"]);
        assert_eq!(split_artists("Daft Punk feat. Pharrell Williams"), ["Daft Punk", "Pharrell Williams"]);
        assert_eq!(split_artists("Bad Bunny & Drake"), ["Bad Bunny", "Drake"]);
        assert_eq!(split_artists("ROSALÍA"), ["ROSALÍA"]);
        assert!(split_artists("").is_empty());
        // Letters that grow when lowercased used to panic (and close the app).
        assert_eq!(split_artists("İbrahim Tatlıses & X"), ["İbrahim Tatlıses", "X"]);
        assert_eq!(split_artists("Ⱥ, B"), ["Ⱥ", "B"]);
        assert_eq!(split_artists("ẞ, Bob"), ["ẞ", "Bob"]);
        assert_eq!(split_artists("İİİ FEAT. Bob"), ["İİİ", "Bob"]);
    }

    #[test]
    fn trims_featuring_from_titles() {
        assert_eq!(trim_featuring("MIA (feat. Drake)"), "MIA");
        assert_eq!(trim_featuring("Song [Ft. X]"), "Song");
        assert_eq!(trim_featuring("Song feat. X"), "Song");
        assert_eq!(trim_featuring("Feather"), "Feather");
    }

    #[test]
    fn folds_any_accent() {
        assert_eq!(fold("ROSALÍA"), "rosalia");
        assert_eq!(fold("Ça Não Møre ząb"), "ca nao more zab");
        assert_eq!(fold("Dvořák Łódź İbrahim"), "dvorak lodz ibrahim");
        assert_eq!(fold("Beyoncé, Sigur Rós, Mötley Crüe"), "beyonce, sigur ros, motley crue");
    }

    #[test]
    fn fingerprints_never_change() {
        // Ids already stored and download file names depend on these exact values.
        assert_eq!(fingerprint(""), "cbf29ce484222325");
        assert_eq!(fingerprint("youtube/dQw4w9WgXcQ"), fingerprint("youtube/dQw4w9WgXcQ"));
        assert_ne!(fingerprint("a/b"), fingerprint("a_b"));
    }
}
