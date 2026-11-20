//! A playlist from pasted text: one song per line, like "Artista - Título".
//!
//! The songs are metadata only (source "lista"): when played, the matcher finds each one
//! in the playable sources, like songs imported from Spotify.

use crate::models::Track;
use crate::text::fingerprint;

/// Source id of songs from a pasted list.
pub const SOURCE: &str = "lista";
/// Lines read at most (a paste, not a library).
pub const MAX_LINES: usize = 2000;

pub struct Parsed {
    pub tracks: Vec<Track>,
    /// Non-empty lines that weren't songs (comments, too long…).
    pub skipped: usize,
}

/// Reads one song per line: "Artista - Título" (also –, — or a tab), "Título by Artista",
/// numbered lines ("1. …", "01 - …") and lines with just the title.
pub fn parse(text: &str) -> Parsed {
    let mut tracks: Vec<Track> = Vec::new();
    let mut skipped = 0;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()).take(MAX_LINES) {
        match parse_line(line) {
            Some(track) if !tracks.iter().any(|t| t.id == track.id) => tracks.push(track),
            Some(_) => {} // the same song twice
            None => skipped += 1,
        }
    }
    Parsed { tracks, skipped }
}

fn parse_line(line: &str) -> Option<Track> {
    if line.starts_with('#') || line.starts_with("//") || line.chars().count() > 300 {
        return None;
    }
    let line = strip_numbering(line);
    let (artist, title) = split_song(line);
    let (artist, title) = (clean(artist), clean(title));
    if title.is_empty() {
        return None;
    }
    Some(Track {
        id: fingerprint(&format!("{}\n{}", artist.to_lowercase(), title.to_lowercase())),
        source: SOURCE.into(),
        title,
        artist,
        duration_secs: 0,
        artwork_url: None,
        genre: None,
    })
}

/// "1. ", "3) ", "01 - " at the start of a line. Only 1–3 digits followed by one of those:
/// "1984 - Van Halen" and "50 Cent - In da Club" are songs, not numbered lines.
fn strip_numbering(line: &str) -> &str {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 3 {
        return line;
    }
    let rest = &line[digits..];
    let after = if let Some(after) = rest.strip_prefix(['.', ')']) {
        after
    } else if let Some(sep) = [" - ", " – ", " — "].into_iter().find(|sep| rest.starts_with(sep)) {
        &rest[sep.len()..]
    } else {
        return line;
    };
    let after = after.trim_start();
    if after.is_empty() { line } else { after }
}

/// (artist, title).
fn split_song(line: &str) -> (&str, &str) {
    for sep in [" - ", " – ", " — ", "\t", " | "] {
        if let Some((artist, title)) = line.split_once(sep) {
            return (artist, title);
        }
    }
    // "Song by Artist"
    if let Some(pos) = line.to_lowercase().rfind(" by ")
        && line.is_char_boundary(pos)
        && line.is_char_boundary(pos + 4)
    {
        return (&line[pos + 4..], &line[..pos]);
    }
    ("", line)
}

fn clean(s: &str) -> String {
    s.trim().trim_matches(['"', '«', '»', '“', '”']).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn songs(text: &str) -> Vec<(String, String)> {
        parse(text).tracks.into_iter().map(|t| (t.artist, t.title)).collect()
    }

    #[test]
    fn reads_common_formats() {
        let text = "Vetusta Morla - Copenhague\n\
                    ROSALÍA – Saoko\n\
                    Daft Punk — One More Time\n\
                    Queen\tBohemian Rhapsody\n\
                    Get Lucky by Daft Punk\n\
                    1. Izal - La mujer de verde\n\
                    02 - Love of Lesbian - Club de fans de John Boy\n\
                    3) \"Estopa - Como Camarón\"\n\
                    50 Cent - In da Club\n\
                    Despacito";
        assert_eq!(
            songs(text),
            [
                ("Vetusta Morla", "Copenhague"),
                ("ROSALÍA", "Saoko"),
                ("Daft Punk", "One More Time"),
                ("Queen", "Bohemian Rhapsody"),
                ("Daft Punk", "Get Lucky"),
                ("Izal", "La mujer de verde"),
                ("Love of Lesbian", "Club de fans de John Boy"),
                ("Estopa", "Como Camarón"),
                ("50 Cent", "In da Club"),
                ("", "Despacito"),
            ]
            .map(|(a, t)| (a.to_string(), t.to_string()))
        );
    }

    #[test]
    fn skips_comments_blank_lines_and_repeats() {
        let parsed = parse("# Mi lista\n\n  \nIzal - Copacabana\nizal - copacabana\n// otra nota\n");
        assert_eq!(parsed.tracks.len(), 1, "the same song once");
        assert_eq!(parsed.skipped, 2);
        assert_eq!(parsed.tracks[0].source, SOURCE);
    }

    #[test]
    fn years_are_not_numbering() {
        assert_eq!(songs("1984 - Van Halen"), [("1984".to_string(), "Van Halen".to_string())]);
    }

    #[test]
    fn same_line_same_id() {
        let a = parse("Izal - Copacabana").tracks.remove(0);
        let b = parse("1. IZAL - COPACABANA").tracks.remove(0);
        assert_eq!(a.id, b.id);
    }
}
