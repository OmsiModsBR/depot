use glam::DVec3;
use openomsi_game::testing::voice::*;
use serde_json::Value;
use std::io::Read;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

#[test]
fn the_hosts_answer_round_trips() {
    let s = VoiceServer {
        server_uid: "abc+/=Def".into(),
        channel: "OMSI - In game".into(),
        password: "p|w 100%".into(),
        range: 25.0,
        radio: false,
    };
    let c = VoiceServer::command(Some(&s));
    assert!(!c.contains('|') && c.len() < omsi_net::MAX_CHAT);
    assert_eq!(VoiceServer::parse_command(&c), Some(Some(s)));
    // (an older host naming no voice_radio: the bus radio is on)
    assert_eq!(
        VoiceServer::parse_command("voice 20 AbC= 5 -"),
        Some(Some(VoiceServer {
            server_uid: "AbC=".into(),
            channel: "5".into(),
            password: String::new(),
            range: 20.0,
            radio: true
        }))
    );
    // (an older host naming no voice server's unique id: no voice chat)
    let open = VoiceServer {
        server_uid: String::new(),
        channel: "12".into(),
        password: String::new(),
        range: 20.0,
        radio: true,
    };
    assert_eq!(
        VoiceServer::parse_command(&VoiceServer::command(Some(&open))),
        Some(None)
    );
    assert_eq!(
        VoiceServer::parse_command(&VoiceServer::command(None)),
        Some(None)
    );
    assert_eq!(VoiceServer::parse_command("trigger x"), None);
}

#[test]
fn server_cfg_keys() {
    let kv = parse_kv(
        "# x\nvoice_server_uid = AbC=\nvoice_channel = 5\nvoice_range = 1000\nvoice_radio = 0\n",
    );
    let s = VoiceServer::from_kv(|k| kv.get(k).cloned()).unwrap();
    assert_eq!(
        (s.server_uid.as_str(), s.channel.as_str(), s.range, s.radio),
        ("AbC=", "5", 200.0, false)
    );
    assert!(VoiceServer::from_kv(|_| None).is_none());
    // no unique id: the plugin would move players on whatever server they are on
    let kv = parse_kv("voice_channel = 5\n");
    assert!(VoiceServer::from_kv(|k| kv.get(k).cloned()).is_none());
    // too long to tell the players whole
    let kv = parse_kv(&format!(
        "voice_server_uid = AbC=\nvoice_channel = 5\nvoice_channel_password = {}\n",
        "p-".repeat(40)
    ));
    assert!(VoiceServer::from_kv(|k| kv.get(k).cloned()).is_none());
    let kv = parse_kv(&format!(
        "voice_server_uid = AbC=\nvoice_channel = 5\nvoice_channel_password = {}\n",
        "p".repeat(100)
    ));
    assert!(VoiceServer::from_kv(|k| kv.get(k).cloned()).is_some());
}

fn scratch_key(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omsi-voice-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("voice-plugin.key")
}

#[test]
fn the_key_is_made_once_and_kept() {
    let path = scratch_key("key");
    let k = plugin_key(&path).unwrap();
    assert_eq!(k.len(), 64);
    assert!(k.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(plugin_key(&path).as_deref(), Some(k.as_str()));
    assert_ne!(random_hex(32), random_hex(32));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn the_hint_to_start_greenteaspeak_goes_after_a_while() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut v = Voice::new(port);
    v.key = Some(scratch_key("hint"));
    v.set_server(Some(VoiceServer {
        server_uid: "UID".into(),
        channel: "7".into(),
        password: String::new(),
        range: 20.0,
        radio: true,
    }));
    v.tick(1.0, ("Max", 1), None, &[]);
    assert!(v
        .hud_line()
        .is_some_and(|l| l.contains("start GreenTeaSpeak")));
    for _ in 0..12 {
        v.tick(1.0, ("Max", 1), None, &[]);
    }
    assert_eq!(v.hud_line(), None);
}

#[test]
fn a_plugin_that_refuses_is_said_so() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut v = Voice::new(port);
    v.key = Some(scratch_key("refused"));
    v.set_server(Some(VoiceServer {
        server_uid: "UID".into(),
        channel: "7".into(),
        password: String::new(),
        range: 20.0,
        radio: true,
    }));
    v.tick(0.1, ("Max", 1), None, &[]);
    let (mut conn, _) = listener.accept().unwrap();
    // Follow the plugin protocol: read hello before refusing. Closing with the
    // client's unread hello can reset the TCP connection on Windows and discard
    // the refusal before the game sees it.
    conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut first = String::new();
    BufReader::new(conn.try_clone().unwrap())
        .read_line(&mut first)
        .unwrap();
    let hello: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(hello["type"], "hello");
    conn.write_all(
        b"{\"type\":\"refused\",\"error\":\"another Project Depot game is already linked\"}\n",
    )
    .unwrap();
    drop(conn);
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && v.refused.is_none() {
        v.tick(0.01, ("Max", 1), None, &[]);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        v.hud_line().as_deref(),
        Some("Voice: another Project Depot game is already linked")
    );
    assert!(!v.status.linked);
    let _ = std::fs::remove_dir_all(v.key.as_ref().unwrap().parent().unwrap());
}

#[test]
fn nicknames_fit_and_tell_players_apart() {
    assert_eq!(nickname("Max Mustermann", 3), "Max Mustermann #3");
    assert_eq!(nickname("  a|b\tc ", 12), "a b c #12");
    assert_eq!(nickname("", 2), "Driver #2");
    let long = nickname("Maximilian Alexander von Mustermannshausen", 1234);
    assert!(
        long.chars().count() <= MAX_NICK && long.ends_with(" #1234"),
        "{long}"
    );
}

#[test]
fn yaw_is_gtas_heading() {
    assert_eq!(saltychat_yaw(0.0), 0.0);
    assert_eq!(saltychat_yaw(90.0), -90.0); // east: GTA -90
    assert_eq!(saltychat_yaw(270.0), 90.0);
    assert_eq!(saltychat_yaw(-90.0), 90.0);
}

#[test]
fn a_bus_muffles_the_voice() {
    let me = Listener {
        at: DVec3::ZERO,
        yaw: 0.0,
        inside: Some(1),
        on_radio: true,
        radio_keyed: false,
    };
    let near = Speaker {
        id: 2,
        name: "b".into(),
        at: DVec3::new(10.0, 0.0, 0.0),
        inside: None,
        on_radio: false,
        radio_keyed: false,
    };
    assert_eq!(heard_volume(&me, &near, 20.0), Some(0.18));
    let aboard = Speaker {
        inside: Some(1),
        ..near.clone()
    };
    assert_eq!(heard_volume(&me, &aboard, 20.0), None);
}

#[test]
fn the_plugin_is_told_the_session_and_the_positions() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut v = Voice::new(port);
    let key_file = scratch_key("link");
    v.key = Some(key_file.clone());
    v.set_server(Some(VoiceServer {
        server_uid: "UID".into(),
        channel: "7".into(),
        password: String::new(),
        range: 20.0,
        radio: true,
    }));
    let me = Listener {
        at: DVec3::new(1_234_567.0, -2_000_000.0, 50.0),
        yaw: 90.0,
        inside: None,
        on_radio: true,
        radio_keyed: true,
    };
    let others = [Speaker {
        id: 2,
        name: "Anna".into(),
        at: DVec3::new(1_234_570.0, -2_000_000.0, 51.6),
        inside: None,
        on_radio: true,
        radio_keyed: true,
    }];
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut conn = None;
    listener.set_nonblocking(true).unwrap();
    while conn.is_none() && Instant::now() < deadline {
        v.tick(0.2, ("Max", 1), Some(me), &others);
        if let Ok((c, _)) = listener.accept() {
            conn = Some(c);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut conn = conn.expect("the game links to the plugin");
    conn.set_nonblocking(false).unwrap();
    // the first line is hello with the key of the file
    let mut first = String::new();
    BufReader::new(conn.try_clone().unwrap())
        .read_line(&mut first)
        .unwrap();
    let hello: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(hello["type"], "hello");
    assert_eq!(
        hello["key"].as_str(),
        std::fs::read_to_string(&key_file).ok().as_deref()
    );
    conn.write_all(b"{\"type\":\"welcome\"}\n{\"type\":\"state\",\"inChannel\":true}\n{\"type\":\"talk\",\"nickname\":\"Anna #2\",\"talking\":true}\n").unwrap();
    conn.set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let mut got = String::new();
    while Instant::now() < deadline && !got.contains("\"players\"") {
        v.tick(0.2, ("Max", 1), Some(me), &others);
        let mut buf = [0u8; 4096];
        if let Ok(n) = conn.read(&mut buf) {
            got.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
    }
    let lines: Vec<Value> = got
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let init = lines
        .iter()
        .find(|l| l["type"] == "initiate")
        .expect("initiate");
    assert_eq!(
        (
            init["serverUid"].as_str(),
            init["channel"].as_str(),
            init["nickname"].as_str()
        ),
        (Some("UID"), Some("7"), Some("Max #1"))
    );
    let me_line = lines.iter().find(|l| l["type"] == "self").expect("self");
    assert_eq!(
        (me_line["x"].as_f64(), me_line["yaw"].as_f64()),
        (Some(-433.0), Some(-90.0))
    );
    assert_eq!(
        (me_line["onRadio"].as_bool(), me_line["keyed"].as_bool()),
        (Some(true), Some(true))
    );
    let players = lines
        .iter()
        .find(|l| l["type"] == "players")
        .expect("players");
    assert_eq!(players["players"][0]["nickname"], "Anna #2");
    assert_eq!(players["players"][0]["x"].as_f64(), Some(-430.0));
    assert!(players["players"][0]["volume"].is_null());
    assert_eq!(players["players"][0]["radio"].as_bool(), Some(true));
    // what the plugin said came in
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until && !(v.status.in_channel && v.speaks("Anna", 2)) {
        v.tick(0.01, ("Max", 1), Some(me), &others);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(v.status.in_channel);
    assert!(v.speaks("Anna", 2));
    assert!(v.on_radio("Anna", 2));
    assert_eq!(v.hud_line(), None);
    let _ = std::fs::remove_dir_all(key_file.parent().unwrap());
}
