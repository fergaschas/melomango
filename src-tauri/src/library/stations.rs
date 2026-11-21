//! Favourite radio stations («Tus emisoras»). The whole station is saved, so the list shows
//! without a connection.

use super::*;
use crate::stations::Station;

fn station_from_row(r: &Row) -> rusqlite::Result<Station> {
    let tags: Option<String> = r.get("tags")?;
    Ok(Station {
        uuid: r.get("uuid")?,
        name: r.get("name")?,
        url: r.get("url")?,
        homepage: r.get("homepage")?,
        favicon: r.get("favicon")?,
        country: r.get::<_, Option<String>>("country")?.unwrap_or_default(),
        state: r.get("state")?,
        tags: tags.unwrap_or_default().split(',').filter(|t| !t.is_empty()).map(str::to_string).collect(),
        codec: r.get::<_, Option<String>>("codec")?.unwrap_or_default(),
        bitrate: r.get("bitrate")?,
        lat: r.get("lat")?,
        long: r.get("long")?,
        distance_m: None,
    })
}

impl Library {
    /// Favourite stations, the newest first.
    pub fn liked_stations(&self) -> anyhow::Result<Vec<Station>> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT * FROM stations ORDER BY liked_at DESC, rowid DESC")?;
            let rows = stmt.query_map([], station_from_row)?;
            rows.collect()
        })
    }

    pub fn is_station_liked(&self, uuid: &str) -> anyhow::Result<bool> {
        self.with(|c| c.query_row("SELECT EXISTS (SELECT 1 FROM stations WHERE uuid = ?1)", [uuid], |r| r.get(0)))
    }

    pub fn set_station_liked(&self, station: &Station, liked: bool) -> anyhow::Result<()> {
        self.with(|c| {
            if liked {
                c.execute(
                    "INSERT INTO stations
                       (uuid, name, url, homepage, favicon, country, state, tags, codec, bitrate, lat, long, liked_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                     ON CONFLICT (uuid) DO NOTHING",
                    params![
                        station.uuid,
                        station.name,
                        station.url,
                        station.homepage,
                        station.favicon,
                        station.country,
                        station.state,
                        station.tags.join(","),
                        station.codec,
                        station.bitrate,
                        station.lat,
                        station.long,
                        now()
                    ],
                )?;
            } else {
                c.execute("DELETE FROM stations WHERE uuid = ?1", [&station.uuid])?;
            }
            Ok(())
        })
    }

    /// A favourite as Radio Browser has it now (its stream may have moved). Keeps when it
    /// was saved; does nothing if it isn't a favourite.
    pub fn refresh_station(&self, station: &Station) -> anyhow::Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE stations SET name = ?2, url = ?3, homepage = ?4, favicon = ?5, country = ?6, state = ?7,
                   tags = ?8, codec = ?9, bitrate = ?10, lat = ?11, long = ?12
                 WHERE uuid = ?1",
                params![
                    station.uuid,
                    station.name,
                    station.url,
                    station.homepage,
                    station.favicon,
                    station.country,
                    station.state,
                    station.tags.join(","),
                    station.codec,
                    station.bitrate,
                    station.lat,
                    station.long
                ],
            )?;
            Ok(())
        })
    }
}
