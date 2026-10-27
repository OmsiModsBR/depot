use openomsi_game::testing::radio::*;

#[test]
fn a_station_list_edited_keeps_the_files_comments_volume_and_frequencies() {
    let text = "# my stations\nvolume = 0.4\nOne = https://a.example/one.mp3 | 94.6 @ 100, 200\n# a note\nTwo = https://b.example/two\n";
    let list = station_lines(text);
    assert_eq!(
        list,
        vec![
            (
                "One".to_string(),
                "https://a.example/one.mp3 | 94.6 @ 100, 200".to_string()
            ),
            ("Two".to_string(), "https://b.example/two".to_string())
        ]
    );
    // the second removed, two added (one of them still without an address)
    let edited = vec![
        list[0].clone(),
        (
            "#Jazz = FM".to_string(),
            "https://c.example/jazz.m3u".to_string(),
        ),
        ("Later".to_string(), String::new()),
    ];
    let out = with_stations(text, &edited);
    assert_eq!(out, "# my stations\nvolume = 0.4\nOne = https://a.example/one.mp3 | 94.6 @ 100, 200\nJazz   FM = https://c.example/jazz.m3u\n# a note\n");
    let (stations, volume, frequencies) = parse_stations(&out);
    assert_eq!(
        stations.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
        ["One", "Jazz   FM"]
    );
    assert_eq!(volume, Some(0.4));
    assert_eq!(frequencies.len(), 1);
    // a file without stations gets them after what it has
    assert_eq!(
        with_stations("volume = 1\n", &edited[..1]),
        "volume = 1\nOne = https://a.example/one.mp3 | 94.6 @ 100, 200\n"
    );
    assert_eq!(station_lines(&default_text()).len(), DEFAULT_STATIONS.len());
}

#[test]
fn a_text_display_gets_plain_letters_and_no_line_break() {
    assert_eq!(display_line("Evropa 2", ""), "Evropa 2");
    assert_eq!(display_line("Evropa 2", "buffering …"), "Evropa 2");
    assert_eq!(display_line("Evropa 2", "evropa 2"), "Evropa 2");
    assert_eq!(
        display_line("Český rozhlas", "Dvořák – Žalm č. 23"),
        "Cesky rozhlas - Dvorak - Zalm c. 23"
    );
    assert_eq!(display_line("me@radio", "Наше Радио"), "me radio -");
}

#[test]
fn a_maps_stations_come_first_and_its_volume_counts_for_nothing() {
    let dir = std::env::temp_dir().join(format!("omsi_map_radio_{}", std::process::id()));
    let map = dir.join("maps").join("Mesto");
    std::fs::create_dir_all(&map).unwrap();
    std::fs::write(map.join("radio.cfg"), "# the town's stations\nvolume = 0.1\nMestske radio = http://example.org/mesto.mp3\nSecond = http://example.org/own.mp3\n").unwrap();
    let own = vec![
        (
            "Mine".to_string(),
            "http://example.org/mine.mp3".to_string(),
        ),
        ("Own".to_string(), "http://EXAMPLE.org/own.mp3".to_string()),
    ];
    let mut r = Radio {
        stations: own.clone(),
        own,
        frequencies: Default::default(),
        own_frequencies: Default::default(),
        map: String::new(),
        relisted: false,
        volume: 0.7,
        offset: 2,
        playing: None,
    };
    r.set_map(&dir, "maps/Mesto/global.cfg");
    let names: Vec<&str> = r.stations.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(names, ["Mestske radio", "Second", "Mine"]);
    assert_eq!((r.volume, r.offset, r.relisted), (0.7, 0, true));
    // a map without a radio.cfg: the player's own again
    r.set_map(&dir, "maps/Jinde/global.cfg");
    assert_eq!(
        r.stations.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
        ["Mine", "Own"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stations_frequencies_and_their_places() {
    let (stations, volume, on) = parse_stations(
        "volume = 0.5\nZurnal = http://example.org/Z.mp3 | 94.6 @ 25400, 990 | 90.9 MHz @ 2300,-720\nKiss = http://example.org/k.mp3 | 98,1\nPlain = http://example.org/p.mp3\nBad = http://example.org/b.mp3 | here @ 1,2 | 0\n",
    );
    assert_eq!(
        stations.iter().map(|s| s.1.as_str()).collect::<Vec<_>>(),
        [
            "http://example.org/Z.mp3",
            "http://example.org/k.mp3",
            "http://example.org/p.mp3",
            "http://example.org/b.mp3"
        ]
    );
    assert_eq!(volume, Some(0.5));
    assert_eq!(
        on["http://example.org/z.mp3"],
        [
            Frequency {
                mhz: "94.6".into(),
                at: Some((25400.0, 990.0))
            },
            Frequency {
                mhz: "90.9".into(),
                at: Some((2300.0, -720.0))
            }
        ]
    );
    assert_eq!(
        on["http://example.org/k.mp3"],
        [Frequency {
            mhz: "98.1".into(),
            at: None
        }]
    );
    assert!(
        !on.contains_key("http://example.org/p.mp3")
            && !on.contains_key("http://example.org/b.mp3")
    );
    // the nearest place counts
    assert_eq!(
        frequency_at(&on["http://example.org/z.mp3"], 24000.0, 0.0),
        Some("94.6")
    );
    assert_eq!(
        frequency_at(&on["http://example.org/z.mp3"], 5000.0, 0.0),
        Some("90.9")
    );
    assert_eq!(
        frequency_at(&on["http://example.org/k.mp3"], 5000.0, 0.0),
        Some("98.1")
    );
}

#[test]
fn a_long_line_runs_through_ten_characters() {
    assert_eq!(marquee("KISS", 10, 5.0), "KISS      ");
    let line = "Evropa 2 - Song";
    assert_eq!(marquee(line, 10, 0.0), "Evropa 2 -");
    assert_eq!(marquee(line, 10, 1.5), "Evropa 2 -");
    assert_eq!(marquee(line, 10, 2.0), "ropa 2 - S");
    // round again after the text and its gap: 18 characters, four a second
    assert_eq!(marquee(line, 10, 1.5 + 14.0 / 4.0), "g   Evropa");
    assert_eq!(marquee(line, 10, 1.5 + 18.0 / 4.0), "Evropa 2 -");
}
