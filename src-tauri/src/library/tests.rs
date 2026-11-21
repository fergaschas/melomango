use super::*;

fn track(id: &str) -> Track {
    Track {
        id: id.into(),
        source: "audius".into(),
        title: format!("Canción {id}"),
        artist: "Artista".into(),
        duration_secs: 100,
        artwork_url: Some(format!("https://img/{id}.jpg")),
        genre: None,
    }
}

fn ids(tracks: &[Track]) -> Vec<&str> {
    tracks.iter().map(|t| t.id.as_str()).collect()
}

#[test]
fn likes_newest_first_and_unlike() {
    let lib = Library::in_memory();
    lib.set_liked(&track("a"), true).unwrap();
    lib.set_liked(&track("b"), true).unwrap();
    lib.set_liked(&track("a"), true).unwrap(); // idempotent
    assert_eq!(ids(&lib.liked_tracks().unwrap()), ["b", "a"]);
    assert_eq!(lib.liked_keys().unwrap().len(), 2);

    lib.set_liked(&track("b"), false).unwrap();
    assert_eq!(ids(&lib.liked_tracks().unwrap()), ["a"]);
    assert_eq!(lib.liked_keys().unwrap(), ["audius/a"]);
}

#[test]
fn liking_updates_stored_metadata() {
    let lib = Library::in_memory();
    lib.set_liked(&track("a"), true).unwrap();
    let mut renamed = track("a");
    renamed.title = "Nuevo título".into();
    lib.set_liked(&renamed, true).unwrap();
    assert_eq!(lib.liked_tracks().unwrap()[0].title, "Nuevo título");
}

#[test]
fn playlist_lifecycle() {
    let lib = Library::in_memory();
    let p = lib.create_playlist("  Para correr  ").unwrap();
    assert_eq!(p.name, "Para correr");
    assert_eq!(p.track_count, 0);
    assert!(lib.create_playlist("   ").is_err());

    assert!(lib.add_to_playlist(p.id, &track("a")).unwrap());
    assert!(lib.add_to_playlist(p.id, &track("b")).unwrap());
    assert!(lib.add_to_playlist(p.id, &track("c")).unwrap());
    assert!(!lib.add_to_playlist(p.id, &track("a")).unwrap(), "sin duplicados");

    let full = lib.playlist(p.id).unwrap();
    assert_eq!(ids(&full.tracks), ["a", "b", "c"]);
    assert_eq!(full.summary.track_count, 3);
    assert_eq!(full.summary.duration_secs, 300);
    assert_eq!(full.summary.artwork_urls.len(), 3);

    lib.move_in_playlist(p.id, 2, 0).unwrap();
    assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["c", "a", "b"]);
    lib.move_in_playlist(p.id, 0, 2).unwrap();
    assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["a", "b", "c"]);

    lib.remove_from_playlist(p.id, "audius", "b").unwrap();
    assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["a", "c"]);
    assert!(lib.add_to_playlist(p.id, &track("d")).unwrap());
    assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["a", "c", "d"]);

    lib.rename_playlist(p.id, "Domingo").unwrap();
    assert_eq!(lib.playlists().unwrap()[0].name, "Domingo");

    lib.delete_playlist(p.id).unwrap();
    assert!(lib.playlists().unwrap().is_empty());
    assert!(lib.playlist(p.id).is_err());
    assert!(lib.add_to_playlist(p.id, &track("a")).is_err());
    assert!(lib.delete_playlist(p.id).is_err());
}

#[test]
fn same_id_from_different_sources_are_different_tracks() {
    let lib = Library::in_memory();
    let mut yt = track("a");
    yt.source = "youtube".into();
    lib.set_liked(&track("a"), true).unwrap();
    lib.set_liked(&yt, true).unwrap();
    assert_eq!(lib.liked_tracks().unwrap().len(), 2);
}

#[test]
fn import_creates_then_resyncs_the_same_playlist() {
    let lib = Library::in_memory();
    let sp = |id: &str| Track { source: "spotify".into(), ..track(id) };

    let first = lib.import_playlist("spotify", "playlist:abc", "Verano", &[sp("1"), sp("2"), sp("1")]).unwrap();
    assert_eq!(first.name, "Verano");
    assert_eq!(first.track_count, 2, "duplicados fuera");
    let full = lib.playlist(first.id).unwrap();
    assert_eq!(ids(&full.tracks), ["1", "2"]);
    assert_eq!(full.link.as_ref().unwrap().remote_id, "playlist:abc");

    // The user renames it; a re-sync keeps the name and replaces the tracks.
    lib.rename_playlist(first.id, "Mi verano").unwrap();
    let again = lib.import_playlist("spotify", "playlist:abc", "Verano", &[sp("3"), sp("2")]).unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(again.name, "Mi verano");
    assert_eq!(ids(&lib.playlist(first.id).unwrap().tracks), ["3", "2"]);

    assert_eq!(lib.imported_from("spotify").unwrap()["playlist:abc"], first.id);
    assert!(lib.playlist(first.id).unwrap().link.is_some());

    // Deleting the playlist drops the link too.
    lib.delete_playlist(first.id).unwrap();
    assert!(lib.imported_from("spotify").unwrap().is_empty());
}

#[test]
fn a_songs_plays_are_found_through_an_index() {
    let lib = Library::in_memory();
    let plan: Vec<String> = lib
        .with(|c| {
            c.prepare("EXPLAIN QUERY PLAN SELECT COUNT(*) FROM plays WHERE source = 'a' AND id = 'b'")?
                .query_map([], |r| r.get::<_, String>(3))?
                .collect()
        })
        .unwrap();
    assert!(plan.iter().any(|step| step.contains("plays_track")), "{plan:?}");
}

#[test]
fn opens_with_wal_and_normal_sync() {
    let path = std::env::temp_dir().join(format!("melomango-sync-{}.db", std::process::id()));
    let lib = Library::open(&path).unwrap();
    let (journal, sync): (String, i64) = lib
        .with(|c| {
            Ok((
                c.pragma_query_value(None, "journal_mode", |r| r.get(0))?,
                c.pragma_query_value(None, "synchronous", |r| r.get(0))?,
            ))
        })
        .unwrap();
    assert_eq!((journal.as_str(), sync), ("wal", 1), "1 = NORMAL");
    drop(lib);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
}

#[test]
fn remembering_the_same_tracks_again_writes_nothing() {
    let lib = Library::in_memory();
    let tracks = [track("a"), track("b")];
    lib.remember_tracks(&tracks).unwrap();
    let changes = || lib.with(|c| Ok(c.total_changes())).unwrap();
    let before = changes();
    lib.remember_tracks(&tracks).unwrap();
    assert_eq!(changes(), before, "unchanged: no row written");
    let with_cover = Track { artwork_url: Some("https://x/new.jpg".into()), ..track("a") };
    lib.remember_tracks(&[with_cover]).unwrap();
    assert_eq!(changes(), before + 1, "a new cover is saved");
    // Missing data never overwrites what is known.
    lib.remember_tracks(&[Track { artwork_url: None, duration_secs: 0, ..track("a") }]).unwrap();
    assert_eq!(changes(), before + 1);
    assert_eq!(lib.track("audius", "a").unwrap().unwrap().artwork_url.as_deref(), Some("https://x/new.jpg"));
}

#[test]
fn a_busy_month_is_summarised_quickly() {
    let lib = Library::in_memory();
    let base = 1_780_000_000_000; // a moment in 2026
    // 20.000 plays of 5.000 songs by 2.000 artists: the old linear lookups took seconds.
    lib.with(|c| {
        let tx = c.transaction()?;
        for i in 0..20_000i64 {
            let t = Track { artist: format!("Artista {}", i % 2_000), ..track(&format!("s{}", i % 5_000)) };
            upsert_track(&tx, &t)?;
            tx.execute(
                "INSERT INTO plays (source, id, played_at) VALUES (?1, ?2, ?3)",
                params![t.source, t.id, base + i * 1000],
            )?;
        }
        tx.commit()
    })
    .unwrap();
    let month = lib.summary_months().unwrap()[0].clone();
    let started = std::time::Instant::now();
    let summary = lib.month_summary(&month).unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
    assert_eq!(summary.plays as usize, 20_000);
    assert_eq!((summary.songs, summary.artists), (5_000, 2_000));
    assert_eq!(summary.top_tracks[0].plays, 4);
}

#[test]
fn choosing_what_to_drop_from_a_big_cache_is_quick() {
    let lib = Library::in_memory();
    // 500 cached songs and 20.000 plays: without the index on plays, every cached song
    // scanned all the plays (about 10 million rows read per pass).
    lib.with(|c| {
        let tx = c.transaction()?;
        for i in 0..500i64 {
            let t = track(&format!("c{i}"));
            upsert_track(&tx, &t)?;
            tx.execute(
                "INSERT INTO downloads (source, id, file, mime, size, kind, downloaded_at)
                 VALUES (?1, ?2, ?3, 'audio/webm', 1000, 'cache', ?4)",
                params![t.source, t.id, format!("c{i}.webm"), i],
            )?;
        }
        for i in 0..20_000i64 {
            let t = track(&format!("c{}", i % 1_000));
            upsert_track(&tx, &t)?;
            tx.execute(
                "INSERT INTO plays (source, id, played_at) VALUES (?1, ?2, ?3)",
                params![t.source, t.id, 1_000 + i],
            )?;
        }
        tx.commit()
    })
    .unwrap();
    let started = std::time::Instant::now();
    let order = lib.cache_eviction_order().unwrap();
    assert!(started.elapsed() < std::time::Duration::from_millis(500), "{:?}", started.elapsed());
    assert_eq!(order.len(), 500);
    // Least recently played first: c0 was last played at play 19.000, c499 at 19.499.
    assert_eq!((order[0].id.as_str(), order[499].id.as_str()), ("c0", "c499"));
}

#[test]
fn importing_several_playlists_is_all_or_nothing() {
    let lib = Library::in_memory();
    let sp = |id: &str| Track { source: "spotify".into(), ..track(id) };
    // The second playlist fails to save (as a full disk or a broken row would).
    lib.with(|c| {
        c.execute_batch(
            "CREATE TRIGGER broken BEFORE INSERT ON playlists WHEN NEW.name = 'Rota'
             BEGIN SELECT RAISE(ABORT, 'disco lleno'); END;",
        )
    })
    .unwrap();
    let (a, b) = ([sp("1")], [sp("2")]);
    let result = lib.import_many("spotify-export", &[("playlist:A", "A", &a[..]), ("playlist:Rota", "Rota", &b[..])]);
    assert!(result.is_err());
    assert!(lib.playlists().unwrap().is_empty(), "the first one is not left half imported");

    let ok = lib.import_many("spotify-export", &[("playlist:A", "A", &a[..]), ("playlist:B", "  ", &b[..])]).unwrap();
    let names: Vec<_> = ok.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["A", "Playlist importada"], "a playlist without a name gets one");
}

#[test]
fn resyncing_keeps_when_each_song_was_added() {
    let lib = Library::in_memory();
    let sp = |id: &str| Track { source: "spotify".into(), ..track(id) };
    let first = lib.import_playlist("spotify", "playlist:abc", "Verano", &[sp("1"), sp("2")]).unwrap();
    // As if it had been imported a while ago.
    let long_ago = 1_000;
    lib.with(|c| c.execute("UPDATE playlist_tracks SET added_at = ?1", [long_ago])).unwrap();

    lib.import_playlist("spotify", "playlist:abc", "Verano", &[sp("3"), sp("2"), sp("1")]).unwrap();
    let synced = lib.playlist(first.id).unwrap();
    assert_eq!(ids(&synced.tracks), ["3", "2", "1"]);
    assert_eq!(&synced.added_at[1..], [long_ago, long_ago], "songs that were already there");
    assert!(synced.added_at[0] > long_ago, "the new song is added now");
}

#[test]
fn imports_without_duration_keep_known_metadata() {
    let lib = Library::in_memory();
    let full = Track { source: "spotify".into(), ..track("x") };
    lib.import_playlist("spotify", "liked", "Me gusta de Spotify", std::slice::from_ref(&full)).unwrap();
    let bare = Track { duration_secs: 0, artwork_url: None, ..full.clone() };
    lib.import_playlist("spotify-export", "liked", "Me gusta de Spotify", &[bare]).unwrap();
    let stored = lib.track("spotify", "x").unwrap().unwrap();
    assert_eq!(stored.duration_secs, full.duration_secs);
    assert_eq!(stored.artwork_url, full.artwork_url);
}

#[test]
fn creates_a_playlist_with_tracks() {
    let lib = Library::in_memory();
    let p = lib.create_playlist_with("Pegada", &[track("a"), track("b"), track("a")]).unwrap();
    assert_eq!((p.name.as_str(), p.track_count), ("Pegada", 2));
    assert_eq!(ids(&lib.playlist(p.id).unwrap().tracks), ["a", "b"]);
    assert!(lib.playlist(p.id).unwrap().link.is_none(), "not linked: nothing to sync");
    let full = lib.playlist(p.id).unwrap();
    assert_eq!(full.added_at.len(), full.tracks.len());
    assert!(full.added_at.iter().all(|t| *t > 0));
    assert!(lib.create_playlist_with("  ", &[]).is_err());
}

#[test]
fn remembers_track_matches() {
    let lib = Library::in_memory();
    let sp = Track { source: "spotify".into(), ..track("x") };
    lib.import_playlist("spotify", "liked", "Me gusta de Spotify", &[sp]).unwrap();
    assert_eq!(lib.track("spotify", "x").unwrap().unwrap().title, "Canción x");
    assert!(lib.track_match("spotify", "x").unwrap().is_none());
    lib.set_track_match("spotify", "x", "youtube", "abc").unwrap();
    lib.set_track_match("spotify", "x", "youtube", "def").unwrap();
    assert_eq!(lib.track_match("spotify", "x").unwrap(), Some(("youtube".into(), "def".into())));
}

#[test]
fn records_and_lists_plays() {
    let lib = Library::in_memory();
    lib.record_play_at(&track("a"), 1_000).unwrap();
    lib.record_play_at(&track("b"), 2_000).unwrap();
    lib.record_play_at(&track("a"), 3_000).unwrap();

    let plays = lib.recent_plays(10).unwrap();
    let order: Vec<(&str, i64)> = plays.iter().map(|p| (p.track.id.as_str(), p.played_at)).collect();
    assert_eq!(order, [("a", 3_000), ("b", 2_000), ("a", 1_000)]);
    assert_eq!(plays[0].track.title, "Canción a");
    assert_eq!(lib.recent_plays(1).unwrap().len(), 1);

    lib.clear_history().unwrap();
    assert!(lib.recent_plays(10).unwrap().is_empty());
    // The tracks themselves (and likes, playlists…) stay.
    assert!(lib.track("audius", "a").unwrap().is_some());
}

#[test]
fn summarises_a_month() {
    let lib = Library::in_memory();
    let mut duet = track("d");
    duet.artist = "Shakira, Burna Boy".into();
    let mut shakira = track("s");
    shakira.artist = "Shakira".into();
    // 2026-11-10 12:00 local-ish (UTC noon keeps the same day in most zones).
    let nov = 1_794_312_000_000i64;
    let oct = nov - 31 * 86_400_000;
    for (t, when) in
        [(&shakira, nov), (&duet, nov + 1), (&shakira, nov + 2), (&track("a"), nov + 3), (&track("a"), oct)]
    {
        lib.record_play_at(t, when).unwrap();
    }
    let months = lib.summary_months().unwrap();
    assert_eq!(months.len(), 2);
    assert!(months[0] > months[1], "newest first");

    let s = lib.month_summary(&months[0]).unwrap();
    assert_eq!((s.plays, s.songs), (4, 3));
    assert_eq!(s.minutes, 4 * 100 / 60, "track() lasts 100 s");
    assert_eq!(s.top_tracks[0].track.id, "s");
    assert_eq!(s.top_tracks[0].plays, 2);
    assert_eq!(s.top_artists[0].name, "Shakira");
    assert_eq!(s.top_artists[0].plays, 3, "the duet counts for both artists");
    assert!(s.top_artists.iter().any(|a| a.name == "Burna Boy"));
    assert_eq!(s.artists, 3);

    let empty = lib.month_summary("1999-01").unwrap();
    assert_eq!((empty.plays, empty.top_tracks.len()), (0, 0));
}

#[test]
fn keeps_a_bounded_history() {
    let lib = Library::in_memory();
    lib.with(|c| {
        upsert_track(c, &track("a"))?;
        // Pretend MAX_PLAYS plays already happened.
        c.execute("INSERT INTO plays (play_id, source, id, played_at) VALUES (?1, 'audius', 'a', 0)", [1])?;
        c.execute("UPDATE sqlite_sequence SET seq = ?1 WHERE name = 'plays'", [MAX_PLAYS])
    })
    .unwrap();
    lib.record_play_at(&track("b"), 5).unwrap();
    let plays = lib.recent_plays(10).unwrap();
    assert_eq!(plays.len(), 1, "the oldest play is dropped");
    assert_eq!(plays[0].track.id, "b");
}

#[test]
fn additive_steps_run_once_each_in_order() {
    let lib = Library::in_memory();
    let steps = ["ALTER TABLE tracks ADD COLUMN extra_a TEXT", "ALTER TABLE tracks ADD COLUMN extra_b TEXT"];
    lib.with(|c| apply_additive_steps(c, &steps[..1])).unwrap();
    // Opening again (or a version with one more step) applies only what is new.
    lib.with(|c| apply_additive_steps(c, &steps)).unwrap();
    lib.with(|c| apply_additive_steps(c, &steps)).unwrap();
    let columns: Vec<String> = lib
        .with(|c| c.prepare("SELECT name FROM pragma_table_info('tracks')")?.query_map([], |r| r.get(0))?.collect())
        .unwrap();
    assert!(columns.contains(&"extra_a".to_string()) && columns.contains(&"extra_b".to_string()));
    // An older Melomango (fewer steps) doesn't lower the count.
    lib.with(|c| apply_additive_steps(c, &[])).unwrap();
    let done: i64 =
        lib.with(|c| c.query_row("SELECT value FROM meta WHERE key = 'additive_steps'", [], |r| r.get(0))).unwrap();
    assert_eq!(done, 2);
}

#[test]
fn migrates_a_v1_database() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA_V1).unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    conn.execute("INSERT INTO playlists (name, created_at, updated_at) VALUES ('Vieja', 0, 0)", []).unwrap();
    let lib = Library::init(conn).unwrap();
    assert_eq!(lib.playlists().unwrap()[0].name, "Vieja");
    assert!(lib.imported_from("spotify").unwrap().is_empty());
}

#[test]
fn reopening_keeps_data() {
    let path = std::env::temp_dir().join(format!("melomango-lib-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let lib = Library::open(&path).unwrap();
        lib.set_liked(&track("a"), true).unwrap();
        lib.create_playlist("Guardada").unwrap();
    }
    let lib = Library::open(&path).unwrap();
    assert_eq!(lib.liked_tracks().unwrap().len(), 1);
    assert_eq!(lib.playlists().unwrap()[0].name, "Guardada");
    drop(lib);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[test]
fn downloads_are_listed_found_and_removed() {
    let lib = Library::in_memory();
    let file = |name: &str| DownloadFile { file: name.into(), mime: "audio/webm".into(), size: 1234 };
    lib.add_download(&track("a"), &file("a.webm"), StoredKind::Download).unwrap();
    lib.add_download(&track("b"), &file("b.webm"), StoredKind::Download).unwrap();
    let listed = lib.downloads().unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].size, 1234);
    let mut keys = lib.downloaded_keys().unwrap();
    keys.sort();
    assert_eq!(keys, ["audius/a", "audius/b"]);
    assert_eq!(lib.download_file("audius", "a").unwrap(), Some(file("a.webm")));
    assert_eq!(lib.download_file("audius", "zzz").unwrap(), None);
    assert_eq!(lib.remove_download("audius", "a").unwrap().as_deref(), Some("a.webm"));
    assert_eq!(lib.remove_download("audius", "a").unwrap(), None);
    assert_eq!(ids(&lib.downloads().unwrap().into_iter().map(|d| d.track).collect::<Vec<_>>()), ["b"]);
    // Downloading again replaces the entry.
    lib.add_download(&track("b"), &file("b2.webm"), StoredKind::Download).unwrap();
    assert_eq!(lib.download_file("audius", "b").unwrap().unwrap().file, "b2.webm");
}

#[test]
fn the_cache_is_apart_from_downloads_and_drops_the_least_played_first() {
    let lib = Library::in_memory();
    let file = |name: &str, size| DownloadFile { file: name.into(), mime: "audio/webm".into(), size };
    lib.add_download(&track("old"), &file("old", 100), StoredKind::Cache).unwrap();
    lib.add_download(&track("new"), &file("new", 200), StoredKind::Cache).unwrap();
    lib.add_download(&track("mine"), &file("mine", 999), StoredKind::Download).unwrap();
    assert_eq!(lib.cache_usage().unwrap(), (2, 300));
    assert!(lib.downloaded_keys().unwrap() == ["audius/mine"], "cached songs aren't downloads");
    assert_eq!(lib.downloads().unwrap().len(), 1);
    // Playing "old" now makes "new" the least recently used.
    lib.record_play_at(&track("old"), now() + 60_000).unwrap();
    let order: Vec<String> = lib.cache_eviction_order().unwrap().into_iter().map(|c| c.id).collect();
    assert_eq!(order, ["new", "old"]);
    // Downloading a cached song keeps it for good.
    assert_eq!(lib.stored_kind("audius", "old").unwrap(), Some(StoredKind::Cache));
    lib.keep_cached("audius", "old").unwrap();
    assert_eq!(lib.stored_kind("audius", "old").unwrap(), Some(StoredKind::Download));
    assert_eq!(lib.cache_usage().unwrap(), (1, 200));
}

#[test]
fn favourites_are_played_twice_or_liked() {
    let lib = Library::in_memory();
    lib.record_play(&track("a")).unwrap();
    assert!(!lib.is_favourite("audius", "a").unwrap());
    lib.record_play(&track("a")).unwrap();
    assert!(lib.is_favourite("audius", "a").unwrap());
    lib.set_liked(&track("b"), true).unwrap();
    assert!(lib.is_favourite("audius", "b").unwrap());
    assert!(!lib.is_favourite("audius", "zzz").unwrap());
}

/// A library left by an older Melomango (e.g. 0.5.0, format 2) opens and keeps its data.
#[test]
fn upgrades_a_library_from_an_older_version() {
    for old in [1, 2] {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_V1).unwrap();
        if old >= 2 {
            conn.execute_batch(SCHEMA_V2).unwrap();
        }
        conn.pragma_update(None, "user_version", old).unwrap();
        conn.execute(
            "INSERT INTO tracks (source, id, title, artist, duration_secs) VALUES ('audius', 'a', 'Vieja', 'X', 100)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO likes (source, id, liked_at) VALUES ('audius', 'a', 1)", []).unwrap();

        let lib = Library::init(conn).unwrap();
        assert_eq!(ids(&lib.liked_tracks().unwrap()), ["a"], "from format {old}");
        lib.record_play(&track("a")).unwrap();
        assert_eq!(lib.recent_plays(10).unwrap().len(), 1);
        assert!(lib.downloads().unwrap().is_empty());
        let version: i32 = lib.with(|c| c.pragma_query_value(None, "user_version", |r| r.get(0))).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }
}

/// A library from a newer Melomango is refused with an error the app recognises.
#[test]
fn refuses_a_library_from_a_newer_version() {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
    let error = Library::init(conn).err().expect("debería negarse");
    assert_eq!(error.downcast_ref::<NewerLibrary>().map(|e| e.found), Some(SCHEMA_VERSION + 1));
}

/// A failed upgrade leaves the library as it was, so the next start can retry.
#[test]
fn a_failed_upgrade_changes_nothing() {
    let path = std::env::temp_dir().join(format!("melomango-upgrade-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(SCHEMA_V1).unwrap();
        conn.execute_batch(SCHEMA_V2).unwrap();
        // In the way of the format-3 step, after its first statement ("plays") worked:
        // its index can't take a name already used.
        conn.execute_batch("CREATE TABLE plays_recent (x INTEGER);").unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
    }
    assert!(Library::open(&path).is_err());
    let conn = Connection::open(&path).unwrap();
    let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    assert_eq!(version, 2, "still the old format");
    let half_done: i32 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name IN ('plays', 'downloads')", [], |r| r.get(0))
        .unwrap();
    assert_eq!(half_done, 0, "nothing half-done");
    drop(conn);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
