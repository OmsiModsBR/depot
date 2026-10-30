use omsi_devtools_proto::*;

fn snap() -> ToLauncher {
    ToLauncher::Snapshot(Snapshot {
        frame: 5120,
        sim_time_s: 85.3,
        vars: vec![("throttle_in".into(), VarValue::Float(0.8))],
        events: vec![Event {
            frame: 5118,
            kind: EventKind::Macro,
            name: "M_door_open".into(),
        }],
        physics: Some(PhysicsSnapshot {
            position: [1.0, 2.0, 3.0],
            velocity: [0.0, 0.0, 10.0],
            wheels: vec![WheelSnapshot {
                load_n: 25000.0,
                slip: 0.01,
                suspension_travel_m: 0.05,
            }],
        }),
    })
}

#[test]
fn a_message_survives_a_round_trip() {
    let bytes = encode(&snap()).unwrap();
    let (back, used): (ToLauncher, usize) = decode(&bytes).unwrap();
    assert_eq!(back, snap());
    assert_eq!(used, bytes.len());
}

#[test]
fn two_frames_in_one_buffer_are_read_one_by_one() {
    let mut buf = encode(&snap()).unwrap();
    buf.extend(encode(&ToGame::Command(Command::Reload { file_index: 1 })).unwrap());
    let (_, used): (ToLauncher, usize) = decode(&buf).unwrap();
    let (second, _): (ToGame, usize) = decode(&buf[used..]).unwrap();
    assert_eq!(second, ToGame::Command(Command::Reload { file_index: 1 }));
}

#[test]
fn a_partial_frame_asks_for_more_bytes() {
    let bytes = encode(&snap()).unwrap();
    assert_eq!(
        decode::<ToLauncher>(&bytes[..2]),
        Err(FrameError::Incomplete)
    );
    assert_eq!(
        decode::<ToLauncher>(&bytes[..bytes.len() - 1]),
        Err(FrameError::Incomplete)
    );
}

#[test]
fn an_absurd_length_is_refused_before_reading() {
    let mut buf = u32::MAX.to_le_bytes().to_vec();
    buf.extend([0u8; 8]);
    assert!(matches!(
        decode::<ToLauncher>(&buf),
        Err(FrameError::TooLong(_))
    ));
}

#[test]
fn garbage_payload_is_malformed() {
    let mut buf = 3u32.to_le_bytes().to_vec();
    buf.extend([0xff, 0xff, 0xff]);
    assert_eq!(decode::<ToLauncher>(&buf), Err(FrameError::Malformed));
}

#[test]
fn handshake_refuses_another_version() {
    let ok = Hello {
        protocol: PROTOCOL_VERSION,
        build: "x".into(),
    };
    let old = Hello {
        protocol: PROTOCOL_VERSION + 1,
        build: "x".into(),
    };
    assert!(check_hello(&ok).is_ok());
    assert!(matches!(check_hello(&old), Err(HelloError { .. })));
}

#[test]
fn commands_with_paths_or_bad_values_are_refused() {
    let set = |name: &str, v| Command::SetVariable {
        name: name.into(),
        value: v,
    };
    assert!(validate_command(&set("throttle_in", VarValue::Float(0.5)), 2).is_ok());
    assert_eq!(
        validate_command(&set("", VarValue::Float(0.5)), 2),
        Err(CommandError::BadName(128))
    );
    assert_eq!(
        validate_command(&set("..\\x", VarValue::Float(0.5)), 2),
        Err(CommandError::PathLike)
    );
    assert_eq!(
        validate_command(&set("a", VarValue::Float(f32::NAN)), 2),
        Err(CommandError::NotFinite)
    );
    assert_eq!(
        validate_command(&Command::Reload { file_index: 2 }, 2),
        Err(CommandError::UnknownFile(2))
    );
    assert!(validate_command(&Command::Reload { file_index: 1 }, 2).is_ok());
}

#[test]
fn bytes_left_over_inside_a_frame_are_refused() {
    let mut payload = postcard::to_stdvec(&ToGame::Hello(Hello {
        protocol: 1,
        build: "x".into(),
    }))
    .unwrap();
    payload.push(0);
    let mut buf = (payload.len() as u32).to_le_bytes().to_vec();
    buf.extend(payload);
    assert_eq!(decode::<ToGame>(&buf), Err(FrameError::TrailingBytes(1)));
}

#[test]
fn the_length_limit_is_exact() {
    // postcard writes a string as a length varint plus its bytes, so the payload is the text
    // plus the enum tag and the length prefix; adjust the text until the payload is the limit.
    let payload_len = |n: usize| {
        postcard::to_stdvec(&ToLauncher::Rejected {
            reason: "x".repeat(n),
        })
        .unwrap()
        .len()
    };
    let mut n = MAX_FRAME_LEN;
    while payload_len(n) > MAX_FRAME_LEN {
        n -= payload_len(n) - MAX_FRAME_LEN;
    }
    assert_eq!(payload_len(n), MAX_FRAME_LEN);
    let at_limit = ToLauncher::Rejected {
        reason: "x".repeat(n),
    };
    let framed = encode(&at_limit).unwrap();
    assert_eq!(decode::<ToLauncher>(&framed).unwrap().1, framed.len());
    let over = ToLauncher::Rejected {
        reason: "x".repeat(n + 1),
    };
    assert!(matches!(encode(&over), Err(FrameError::TooLong(_))));
}

#[test]
fn text_and_subscription_limits_apply() {
    let set = |v| Command::SetVariable {
        name: "a".into(),
        value: v,
    };
    assert_eq!(
        validate_command(&set(VarValue::Text("x".repeat(MAX_TEXT + 1))), 0),
        Err(CommandError::TextTooLong(MAX_TEXT))
    );
    assert_eq!(
        validate_command(&set(VarValue::Text("a\nb".into())), 0),
        Err(CommandError::ControlInText)
    );
    let many = Command::Subscribe {
        names: vec!["v".into(); MAX_SUBSCRIPTION + 1],
    };
    assert_eq!(
        validate_command(&many, 0),
        Err(CommandError::TooManyNames(MAX_SUBSCRIPTION))
    );
}
