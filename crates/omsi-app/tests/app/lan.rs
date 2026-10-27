use glam::DVec3;
use omsi_net::Pose;
use openomsi_game::testing::lan::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

/// The natural weather and the cycle need no file: a client takes them from any host.
#[test]
fn a_hosts_natural_weather_or_cycle_is_taken_without_a_file() {
    use clap::Parser;
    let args = openomsi_game::testing::cli::Args::parse_from(["depot"]);
    for w in ["natural", "Natural", "cycle"] {
        assert_eq!(host_weather(&args, w), Ok(Some(w.to_string())), "{w}");
    }
    assert_eq!(host_weather(&args, ""), Ok(None));
    assert!(host_weather(&args, "weather/none_such.owt").is_err());
}

/// Every session starts with every bus offered: a server joined before (on a phone the
/// launcher and the game share one process) no longer limits a drive alone or the next
/// server, and an answer late for an ended session is dropped (#1183).
#[test]
fn a_new_session_forgets_the_last_servers_buses() {
    let mut o = ServerOffers {
        session: 0,
        list: None,
    };
    let first = o.reset();
    assert!(o.answer(first, &["Vehicles/MAN_SD200/MAN_SD77.bus".to_string()]));
    assert!(o
        .list
        .as_ref()
        .is_some_and(|l| offers(l, "Vehicles/MAN_SD200/MAN_SD77.bus")));
    let second = o.reset();
    assert!(o.list.is_none());
    // (the first server's answer, late)
    assert!(!o.answer(first, &["Vehicles/MAN_SD200/MAN_SD77.bus".to_string()]));
    assert!(o.list.is_none());
    assert!(o.answer(second, &["Vehicles/MAN_SD202/MAN_D92.bus".to_string()]));
    assert!(o
        .list
        .as_ref()
        .is_some_and(|l| !offers(l, "Vehicles/MAN_SD200/MAN_SD77.bus")
            && offers(l, "Vehicles/MAN_SD202/MAN_D92.bus")));
}

/// Whom a joining game asks for the buses offered: a server's web address, or a host's
/// address over UDP (its status page is found from there); nobody for a code or a search
/// (#1183).
#[test]
fn the_buses_offered_are_asked_at_an_address() {
    assert_eq!(
        offers_query_target("https://abc.trycloudflare.com").as_deref(),
        Some("https://abc.trycloudflare.com")
    );
    assert_eq!(
        offers_query_target(" 203.0.113.5:27015 ").as_deref(),
        Some("203.0.113.5:27015")
    );
    assert_eq!(
        offers_query_target("bus.example.org").as_deref(),
        Some("bus.example.org")
    );
    assert_eq!(
        offers_query_target("27015").as_deref(),
        Some("127.0.0.1:27015")
    );
    assert_eq!(offers_query_target("auto"), None);
    assert_eq!(offers_query_target(""), None);
    let code = omsi_net::SessionCode {
        session: 0x1234_5678_9abc,
        port: 27015,
        ips: vec!["192.168.1.20".parse().unwrap()],
        protocol: omsi_net::PROTOCOL as u8,
    }
    .encode();
    assert_eq!(offers_query_target(&code), None);
}

/// A server's `vehicles` list (as its `server.cfg` writes it, or every bus under its own
/// content folder) against the game's lists' `Vehicles/<folder>/<file>`: the same file
/// only (#1183).
#[test]
fn a_server_offers_the_buses_of_its_list_only() {
    let list = offered_keys(&[
        "Vehicles\\MAN_SD200\\MAN_SD77.bus".to_string(),
        "OMSI 2/Vehicles/MAN_SD202/MAN_D92.bus".to_string(),
    ]);
    assert!(offers(&list, "Vehicles/MAN_SD200/MAN_SD77.bus"));
    assert!(offers(&list, "vehicles/man_sd202/man_d92.bus"));
    assert!(offers(
        &list,
        "Archives/pack.zip/Vehicles/MAN_SD202/MAN_D92.bus"
    ));
    assert!(!offers(&list, "Vehicles/MAN_SD200/MAN_SD83.bus"));
    assert!(!offers(&list, "Vehicles/MAN_SD202/MAN_D86.bus"));
    // (the same file name in another folder is another bus)
    assert!(!offers(&list, "Vehicles/Mod_SD200/MAN_SD77.bus"));
    assert!(!offers(
        &offered_keys(&[]),
        "Vehicles/MAN_SD200/MAN_SD77.bus"
    ));
}

/// The maps show another player with their bus, on foot where they walk, riding in a
/// third player's bus with that bus, and not at all riding in ours (#1011, #1080).
#[test]
fn the_maps_show_a_player_where_they_are() {
    let bus = (DVec3::new(100.0, 200.0, 5.0), 90.0);
    let third = |id: u32| (id == 7).then_some((DVec3::new(-50.0, 10.0, 0.0), 180.0));
    let mut pose = Pose {
        id: 3,
        ..Default::default()
    };
    let p = nav_player(&pose, " Anna ", bus, 2, third).unwrap();
    assert_eq!(
        (p.position, p.heading, p.name.as_str()),
        (bus.0, 90.0, "Anna")
    );
    pose.walker = Some(omsi_net::Walker {
        x: 1.0,
        y: 2.0,
        z: 3.0,
        heading: 45.0,
        ..Default::default()
    });
    let p = nav_player(&pose, "", bus, 2, third).unwrap();
    assert_eq!(
        (p.position, p.heading, p.name.as_str()),
        (DVec3::new(1.0, 2.0, 3.0), 45.0, "player 3")
    );
    pose.walker.as_mut().unwrap().aboard = Some(omsi_net::Aboard {
        owner: 7,
        ..Default::default()
    });
    assert_eq!(
        nav_player(&pose, "Anna", bus, 2, third).unwrap().position,
        DVec3::new(-50.0, 10.0, 0.0)
    );
    pose.walker.as_mut().unwrap().aboard = Some(omsi_net::Aboard {
        owner: 2,
        ..Default::default()
    });
    assert!(nav_player(&pose, "Anna", bus, 2, third).is_none());
}

/// What is seen comes before what is heard in the capped values list: the AA-FR Agora
/// L's sound variables filled it in name order before its roller blind's scroll.
#[test]
fn the_roller_blind_scroll_is_in_the_sync_table() {
    let root = omsi_cfg::flags::ROOT
        .os()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/AA-FR_BusBundle/2002_Agora_L_4d_main.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = omsi_sim::VehicleType::load(&root, &bus).expect("Agora L");
    let t = SyncTable::new(&ty, &[]);
    assert!(t.values.len() <= omsi_net::wire::MAX_VALUES);
    for want in ["Rollband_Linie_Trans", "Rollband_Linie_Trans_2"] {
        assert!(
            t.values.iter().any(|v| v.0.eq_ignore_ascii_case(want)),
            "no {want}: {}",
            t.describe()
        );
    }
}

/// An articulated bus's rear section is in its sync table: its lamps, displays' switches
/// and outside sounds (the AA-FR Agora L's rear section stood dark and silent in the
/// other players' games).
#[test]
fn the_rear_section_is_in_the_sync_table() {
    let root = omsi_cfg::flags::ROOT
        .os()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/AA-FR_BusBundle/2002_Agora_L_3d_main.bus");
    let trail = root.join("Vehicles/AA-FR_BusBundle/2002_Agora_L_3d_trail.bus");
    if !bus.exists() || !trail.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = omsi_sim::VehicleType::load(&root, &bus).expect("Agora L");
    let part = Arc::new(omsi_sim::VehicleType::load(&root, &trail).expect("Agora L trail"));
    let alone = SyncTable::new(&ty, &[]);
    let whole = SyncTable::new(&ty, &[part]);
    let count = |t: &SyncTable| t.lamps.len() + t.switches.len();
    assert!(
        count(&whole) > count(&alone),
        "alone {}, whole {}",
        alone.describe(),
        whole.describe()
    );
    assert!(
        !whole.part_sounds.is_empty(),
        "no sounds for the rear section: {}",
        whole.describe()
    );
    assert_ne!(whole.hash, alone.hash);
}

#[test]
fn day_numbers_and_clock_gaps() {
    assert_eq!(day_number(1990, 1) - day_number(1989, 365), 1);
    assert_eq!(day_number(1989, 1) - day_number(1988, 366), 1);
    let a = omsi_sim::SimClock {
        year: 1990,
        day_of_year: 1,
        time: 10.0,
        ..Default::default()
    };
    let b = omsi_sim::SimClock {
        year: 1989,
        day_of_year: 365,
        time: 86390.0,
        ..Default::default()
    };
    assert!((clock_gap(&a, &b) - 20.0).abs() < 1e-9);
    assert!((clock_gap(&b, &a) + 20.0).abs() < 1e-9);
    assert_eq!(parse_date("1989-05-30"), Some((1989, 150)));
    assert_eq!(parse_date("x"), None);
}

#[test]
fn the_hosts_clock_moves_on_past_midnight() {
    let h = omsi_net::HostClock {
        world: omsi_net::WorldInfo {
            date: "1989-12-31".into(),
            time: 86399.5,
            ..Default::default()
        },
        at: Instant::now() - Duration::from_secs(2),
        speed: 1.0,
    };
    let c = host_clock_now(&h).unwrap();
    assert_eq!((c.year, c.day_of_year), (1990, 1));
    assert!((c.time - 1.5).abs() < 0.1, "{}", c.time);
}

#[test]
fn engine_fed_names() {
    for n in [
        "Wheel_RotationSpeed_1_R",
        "Velocity",
        "AI_Light",
        "door_0",
        "StreetCond",
        "Timegap",
    ] {
        assert!(engine_fed(n), "{n}");
    }
    for n in [
        "engine_n",
        "engine_throttle_injection",
        "M_Wheel",
        "doorSpeed_0",
        "wiperpos",
        "cockpit_hupe_volume",
        "lights_stand",
    ] {
        assert!(!engine_fed(n), "{n}");
    }
}
