//! Text helpers shared by the modules that compare songs and artists (matcher, lyrics,
//! artist pages), so they all read a name the same way.

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

/// The base letter of an accented one (lowercase input): «á» -> «a», «ç» -> «c», «ø» -> «o».
pub fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'ā' | 'ą' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'ē' | 'ę' | 'ě' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'ī' | 'ı' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'ø' | 'ō' | 'ő' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'ū' | 'ů' | 'ű' => 'u',
        'ç' | 'ć' | 'č' => 'c',
        'ñ' | 'ń' | 'ň' => 'n',
        'ś' | 'š' | 'ş' | 'ș' => 's',
        'ź' | 'ż' | 'ž' => 'z',
        'ý' | 'ÿ' => 'y',
        'ğ' => 'g',
        'ł' => 'l',
        'ř' => 'r',
        'ț' | 'ţ' | 'ť' => 't',
        'ď' => 'd',
        other => other,
    }
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
    fn folds_accents_beyond_spanish() {
        let folded: String = "ça não møre ząb".chars().map(fold).collect();
        assert_eq!(folded, "ca nao more zab");
    }
}
