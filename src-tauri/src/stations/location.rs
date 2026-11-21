//! Where the user is, for «Cerca de ti» and «Lo más escuchado en {país}».
//!
//! Only ever approximate: coordinates are rounded to about 1 km before they leave this
//! module, and the country comes from Windows' region setting, without asking anything.

use serde::Serialize;

use super::round_coord;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, specta::Type)]
pub struct Coords {
    pub lat: f64,
    pub long: f64,
}

/// The position Windows' location service gives (if the user allows it), rounded.
/// Blocks for up to a few seconds: call it off the async threads.
pub fn system_position() -> anyhow::Result<Coords> {
    let (lat, long) = platform::position()?;
    Ok(Coords { lat: round_coord(lat), long: round_coord(long) })
}

/// The country of the system's region setting ("ES"), if it is one.
pub fn system_country() -> Option<String> {
    platform::region().and_then(|code| {
        let code = code.trim().to_ascii_uppercase();
        (code.len() == 2 && code.bytes().all(|b| b.is_ascii_uppercase())).then_some(code)
    })
}

#[cfg(windows)]
mod platform {
    use anyhow::{Context, bail};
    use windows::{
        Devices::Geolocation::{GeolocationAccessStatus, Geolocator, PositionAccuracy},
        Foundation::TimeSpan,
        Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize},
    };

    /// 100-nanosecond units, as WinRT counts time.
    const fn secs(s: i64) -> TimeSpan {
        TimeSpan { Duration: s * 10_000_000 }
    }

    pub fn position() -> anyhow::Result<(f64, f64)> {
        // The pool's threads haven't set up WinRT; "already set up" is fine too.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let access = Geolocator::RequestAccessAsync()
            .and_then(|op| op.join())
            .context("no se pudo preguntar a Windows por la ubicación")?;
        if access == GeolocationAccessStatus::Denied {
            bail!(
                "Windows no deja usar la ubicación. Actívala en Configuración > Privacidad y seguridad > \
                 Ubicación, o elige el país a mano."
            );
        }
        let locator = Geolocator::new()?;
        locator.SetDesiredAccuracy(PositionAccuracy::Default)?;
        // A position up to an hour old is good enough, and waiting more than 10 s is not.
        let position = locator
            .GetGeopositionAsyncWithAgeAndTimeout(secs(3600), secs(10))
            .and_then(|op| op.join())
            .context("Windows no sabe dónde estás ahora mismo")?;
        let point = position.Coordinate()?.Point()?.Position()?;
        Ok((point.Latitude, point.Longitude))
    }

    pub fn region() -> Option<String> {
        use windows_sys::Win32::Globalization::GetUserDefaultGeoName;
        let mut name = [0u16; 16];
        let len = unsafe { GetUserDefaultGeoName(name.as_mut_ptr(), name.len() as i32) };
        // The length counts the final NUL.
        (len > 1).then(|| String::from_utf16_lossy(&name[..len as usize - 1]))
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn position() -> anyhow::Result<(f64, f64)> {
        anyhow::bail!("la ubicación del sistema solo está disponible en Windows: elige el país a mano")
    }

    /// From the locale ("es_ES.UTF-8" -> "ES").
    pub fn region() -> Option<String> {
        let lang = std::env::var("LC_ALL").or_else(|_| std::env::var("LANG")).ok()?;
        let region = lang.split(['.', '@']).next()?.split_once('_')?.1;
        Some(region.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_region_is_a_two_letter_country_or_nothing() {
        if let Some(code) = system_country() {
            assert_eq!(code.len(), 2);
        }
    }

    /// Asks Windows where this computer is (it may ask for permission). `cargo test -- --ignored`
    #[test]
    #[ignore = "pregunta la ubicación al sistema"]
    fn asks_windows_for_the_position() {
        let at = system_position().unwrap();
        assert_eq!(at.lat, round_coord(at.lat));
        println!("{at:?}");
    }
}
