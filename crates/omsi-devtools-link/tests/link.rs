use omsi_devtools_link::{
    GameLink, LauncherLink, LinkState, MAX_EVENTS_PER_SNAPSHOT, MAX_PENDING_CONTROL,
    MAX_PENDING_SNAPSHOTS,
};
use omsi_devtools_proto::{
    encode, Command, Event, EventKind, Hello, Snapshot, ToGame, ToLauncher, VarValue,
};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// One direction of an in-memory stream (no socket, so the tests run on every platform).
type Buffer = (Mutex<(VecDeque<u8>, bool)>, Condvar);

#[derive(Clone, Default)]
struct Pipe(Arc<Buffer>);

impl Pipe {
    fn close(&self) {
        self.0 .0.lock().unwrap().1 = true;
        self.0 .1.notify_all();
    }
}
impl Read for Pipe {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let (m, c) = &*self.0;
        let mut g = m.lock().unwrap();
        while g.0.is_empty() {
            if g.1 {
                return Ok(0);
            }
            g = c.wait(g).unwrap();
        }
        let n = out.len().min(g.0.len());
        for b in out.iter_mut().take(n) {
            *b = g.0.pop_front().unwrap();
        }
        Ok(n)
    }
}
impl Write for Pipe {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let (m, c) = &*self.0;
        m.lock().unwrap().0.extend(data);
        c.notify_all();
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn pair() -> (GameLink, LauncherLink, Pipe, Pipe) {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let game = GameLink::start(to_game.clone(), to_launcher.clone(), "game");
    let tool = LauncherLink::start(to_launcher.clone(), to_game.clone(), "tool");
    (game, tool, to_game, to_launcher)
}

fn wait(what: &str, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !f() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn tool_hello(to_game: &Pipe, protocol: u32) {
    let mut w = to_game.clone();
    w.write_all(
        &encode(&ToGame::Hello(Hello {
            protocol,
            build: "tool".into(),
        }))
        .unwrap(),
    )
    .unwrap();
}

fn snap(frame: u64, vars: &[(&str, f32)], events: &[&str]) -> Snapshot {
    Snapshot {
        frame,
        sim_time_s: frame as f64 / 60.0,
        vars: vars
            .iter()
            .map(|(k, v)| (k.to_string(), VarValue::Float(*v)))
            .collect(),
        events: events
            .iter()
            .map(|n| Event {
                frame,
                kind: EventKind::Macro,
                name: n.to_string(),
            })
            .collect(),
        physics: None,
    }
}

#[test]
fn both_ends_become_ready_and_messages_flow_both_ways() {
    let (game, tool, _, _) = pair();
    wait("ready", || {
        game.state() == LinkState::Ready && tool.state() == LinkState::Ready
    });
    game.send_snapshot(snap(1, &[("speed", 3.0)], &[]));
    let m = tool.recv_timeout(Duration::from_secs(5)).expect("snapshot");
    assert_eq!(m, ToLauncher::Snapshot(snap(1, &[("speed", 3.0)], &[])));

    let cmd = Command::Subscribe {
        names: vec!["speed".into()],
    };
    tool.send(cmd.clone());
    let mut got = vec![];
    wait("command", || {
        got.extend(game.poll_commands(0));
        !got.is_empty()
    });
    assert_eq!(got, vec![cmd.clone()]);
    game.send_applied(2, cmd.clone());
    assert_eq!(
        tool.recv_timeout(Duration::from_secs(5)),
        Some(ToLauncher::Applied {
            frame: 2,
            command: cmd
        })
    );
}

#[test]
fn a_command_that_fails_the_checks_is_rejected_and_not_returned() {
    let (game, tool, _, _) = pair();
    tool.send(Command::SetVariable {
        name: "../evil".into(),
        value: VarValue::Float(1.0),
    });
    tool.send(Command::Reload { file_index: 9 });
    tool.send(Command::SetVariable {
        name: "ok".into(),
        value: VarValue::Float(2.0),
    });
    let mut ok = vec![];
    wait("the valid command", || {
        ok.extend(game.poll_commands(2));
        !ok.is_empty()
    });
    assert_eq!(
        ok,
        vec![Command::SetVariable {
            name: "ok".into(),
            value: VarValue::Float(2.0)
        }]
    );
    let mut rejected = 0;
    while rejected < 2 {
        match tool.recv_timeout(Duration::from_secs(5)) {
            Some(ToLauncher::Rejected { .. }) => rejected += 1,
            Some(_) => {}
            None => panic!("expected two rejections"),
        }
    }
}

#[test]
fn another_protocol_version_is_refused_and_nothing_is_applied() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let game = GameLink::start(to_game.clone(), to_launcher.clone(), "game");
    // a tool of another version: its hello, then a command
    let mut w = to_game.clone();
    w.write_all(
        &encode(&ToGame::Hello(Hello {
            protocol: 999,
            build: "future".into(),
        }))
        .unwrap(),
    )
    .unwrap();
    w.write_all(&encode(&ToGame::Command(Command::Reload { file_index: 0 })).unwrap())
        .unwrap();
    wait("refusal", || game.state() == LinkState::Refused);
    assert!(game.poll_commands(1).is_empty());
    // nothing but the game's own hello was sent
    game.send_snapshot(snap(1, &[], &[]));
    std::thread::sleep(Duration::from_millis(50));
    let mut r = to_launcher.clone();
    to_launcher.close();
    let mut bytes = vec![];
    r.read_to_end(&mut bytes).unwrap();
    let (m, used): (ToLauncher, usize) = omsi_devtools_proto::decode(&bytes).unwrap();
    assert!(matches!(m, ToLauncher::Hello(_)));
    assert_eq!(used, bytes.len(), "only the hello was written");
}

#[test]
fn a_stream_that_starts_with_something_else_than_a_hello_is_refused() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let game = GameLink::start(to_game.clone(), to_launcher, "game");
    let mut w = to_game;
    w.write_all(&encode(&ToGame::Command(Command::Reload { file_index: 0 })).unwrap())
        .unwrap();
    wait("refusal", || game.state() == LinkState::Refused);
    assert!(game.poll_commands(1).is_empty());
}

/// A launcher that does not read: the game's calls still return at once, the queue stays
/// bounded, and the oldest snapshots are folded into the next so nothing they said is lost.
#[test]
fn a_slow_launcher_never_blocks_the_game_and_loses_no_change() {
    struct Stuck;
    impl Write for Stuck {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            std::thread::sleep(Duration::from_secs(3600));
            Ok(0)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let to_game = Pipe::default();
    let game = GameLink::start(to_game, Stuck, "game");
    let start = Instant::now();
    let n = (MAX_PENDING_SNAPSHOTS * 4) as u64;
    for f in 0..n {
        game.send_snapshot(snap(
            f,
            &[(&format!("v{f}"), f as f32)],
            &[&format!("e{f}")],
        ));
    }
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(game.folded_snapshots() > 0);
    // (the first snapshot was taken by the writer, which is stuck on it)
    assert!(game.folded_snapshots() as usize >= n as usize - MAX_PENDING_SNAPSHOTS - 1);
}

/// A writer that holds every write until it is opened, like a launcher that is behind.
#[derive(Clone)]
struct Gated(Pipe, Arc<(Mutex<bool>, Condvar)>);
impl Write for Gated {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let (m, c) = &*self.1;
        let mut open = m.lock().unwrap();
        while !*open {
            open = c.wait(open).unwrap();
        }
        drop(open);
        self.0.write(data)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn folding_keeps_the_latest_value_and_every_event() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let game = GameLink::start(
        to_game.clone(),
        Gated(to_launcher.clone(), gate.clone()),
        "game",
    );
    tool_hello(&to_game, omsi_devtools_proto::PROTOCOL_VERSION);
    wait("ready", || game.state() == LinkState::Ready);
    let total = MAX_PENDING_SNAPSHOTS as u64 * 3;
    for f in 0..total {
        game.send_snapshot(snap(f, &[("x", f as f32)], &[&format!("e{f}")]));
    }
    assert!(game.folded_snapshots() > 0);
    // the launcher catches up
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    // (its reply hello is not needed to read: the game writes regardless)
    let mut events = vec![];
    let mut last = None;
    let mut buf: Vec<u8> = vec![];
    let mut r = to_launcher.clone();
    let end = Instant::now() + Duration::from_secs(5);
    while events.len() < total as usize {
        assert!(Instant::now() < end, "timed out, got {events:?}");
        let mut chunk = [0u8; 4096];
        let n = {
            // (read what is there; the pipe blocks when empty, so only read when it holds bytes)
            if to_launcher.0 .0.lock().unwrap().0.is_empty() {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            r.read(&mut chunk).unwrap()
        };
        buf.extend_from_slice(&chunk[..n]);
        while let Ok((m, used)) = omsi_devtools_proto::decode::<ToLauncher>(&buf) {
            buf.drain(..used);
            if let ToLauncher::Snapshot(s) = m {
                events.extend(s.events.iter().map(|e| e.name.clone()));
                last = Some(s);
            }
        }
    }
    let want: Vec<String> = (0..total).map(|f| format!("e{f}")).collect();
    assert_eq!(events, want);
    assert_eq!(
        last.unwrap().vars,
        vec![("x".into(), VarValue::Float((total - 1) as f32))]
    );
}

#[test]
fn a_closed_stream_ends_the_link_and_sending_is_harmless() {
    let (game, tool, to_game, to_launcher) = pair();
    wait("ready", || game.state() == LinkState::Ready);
    to_game.close();
    to_launcher.close();
    wait("closed", || {
        game.state() == LinkState::Closed && tool.state() == LinkState::Closed
    });
    game.send_snapshot(snap(1, &[], &[]));
    game.send_rejected("late");
    tool.send(Command::Reload { file_index: 0 });
    assert!(game.poll_commands(1).is_empty());
}

/// Reads one byte at a time, as a stream may split its frames anywhere.
struct OneByte(Pipe);
impl Read for OneByte {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = out.len().min(1);
        self.0.read(&mut out[..n])
    }
}

fn frames(bytes: &[u8]) -> Vec<ToLauncher> {
    let (mut at, mut out) = (0, vec![]);
    while at < bytes.len() {
        let (m, used) = omsi_devtools_proto::decode::<ToLauncher>(&bytes[at..]).unwrap();
        at += used;
        out.push(m);
    }
    out
}

fn drain(p: &Pipe) -> Vec<u8> {
    p.0 .0.lock().unwrap().0.drain(..).collect()
}

#[test]
fn frames_split_into_single_bytes_are_read() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let game = GameLink::start(OneByte(to_game.clone()), to_launcher, "game");
    tool_hello(&to_game, omsi_devtools_proto::PROTOCOL_VERSION);
    let cmd = Command::SetVariable {
        name: "x".into(),
        value: VarValue::Text("hello".into()),
    };
    to_game
        .clone()
        .write_all(&encode(&ToGame::Command(cmd.clone())).unwrap())
        .unwrap();
    let mut got = vec![];
    wait("command", || {
        got.extend(game.poll_commands(0));
        !got.is_empty()
    });
    assert_eq!(got, vec![cmd]);
}

#[test]
fn a_stream_that_ends_before_any_hello_is_closed_not_refused() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let game = GameLink::start(to_game.clone(), to_launcher.clone(), "game");
    let tool = LauncherLink::start(to_launcher.clone(), Pipe::default(), "tool");
    to_game.close();
    to_launcher.close();
    wait("closed", || {
        game.state() == LinkState::Closed && tool.state() == LinkState::Closed
    });
}

#[test]
fn a_second_hello_ends_the_link() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let game = GameLink::start(to_game.clone(), to_launcher, "game");
    tool_hello(&to_game, omsi_devtools_proto::PROTOCOL_VERSION);
    wait("ready", || game.state() == LinkState::Ready);
    tool_hello(&to_game, omsi_devtools_proto::PROTOCOL_VERSION);
    wait("closed", || game.state() == LinkState::Closed);
}

#[test]
fn the_launcher_refuses_another_version_and_sends_no_command_to_it() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let tool = LauncherLink::start(to_launcher.clone(), to_game.clone(), "tool");
    // commands queued before the game answers wait for its hello
    assert!(tool.send(Command::Reload { file_index: 0 }));
    let mut w = to_launcher.clone();
    w.write_all(
        &encode(&ToLauncher::Hello(Hello {
            protocol: 999,
            build: "future".into(),
        }))
        .unwrap(),
    )
    .unwrap();
    wait("refusal", || tool.state() == LinkState::Refused);
    drop(tool);
    std::thread::sleep(Duration::from_millis(100));
    to_game.close();
    let mut bytes = vec![];
    to_game.clone().read_to_end(&mut bytes).unwrap();
    let (m, used) = omsi_devtools_proto::decode::<ToGame>(&bytes).unwrap();
    assert!(matches!(m, ToGame::Hello(_)));
    assert_eq!(used, bytes.len(), "only the hello was written");
}

#[test]
fn nothing_but_the_hello_is_sent_before_the_peers_hello_is_accepted() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let game = GameLink::start(to_game.clone(), to_launcher.clone(), "game");
    game.send_snapshot(snap(1, &[("a", 1.0)], &[]));
    game.send_rejected("early");
    std::thread::sleep(Duration::from_millis(100));
    let early = frames(&drain(&to_launcher));
    assert_eq!(early.len(), 1);
    assert!(matches!(early[0], ToLauncher::Hello(_)));
    tool_hello(&to_game, omsi_devtools_proto::PROTOCOL_VERSION);
    // now the control message and the snapshot follow
    let mut seen = vec![];
    wait("both", || {
        seen.extend(frames(&drain(&to_launcher)));
        seen.len() >= 2
    });
    assert!(matches!(seen[0], ToLauncher::Rejected { .. }));
    assert!(matches!(seen[1], ToLauncher::Snapshot(_)));
}

#[test]
fn too_many_commands_are_answered_rejected_and_counted() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let game = GameLink::start(to_game.clone(), to_launcher.clone(), "game");
    tool_hello(&to_game, omsi_devtools_proto::PROTOCOL_VERSION);
    wait("ready", || game.state() == LinkState::Ready);
    let n = MAX_PENDING_CONTROL + 5;
    let mut w = to_game.clone();
    for i in 0..n {
        let c = Command::SetVariable {
            name: format!("v{i}"),
            value: VarValue::Float(1.0),
        };
        w.write_all(&encode(&ToGame::Command(c)).unwrap()).unwrap();
    }
    // (the game does not poll: the inbox fills)
    wait("overflow", || game.lost().commands == 5);
    let got = game.poll_commands(0);
    assert_eq!(got.len(), MAX_PENDING_CONTROL);
    let mut rejected = 0;
    let mut seen = vec![];
    wait("rejections", || {
        seen.extend(frames(&drain(&to_launcher)));
        rejected = seen
            .iter()
            .filter(|m| matches!(m, ToLauncher::Rejected { .. }))
            .count();
        rejected >= 5
    });
}

#[test]
fn the_control_queue_is_bounded_and_counts_what_it_drops() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    // (no peer hello: nothing is written, the queue only fills)
    let game = GameLink::start(to_game, to_launcher, "game");
    for i in 0..MAX_PENDING_CONTROL + 3 {
        game.send_rejected(format!("r{i}"));
    }
    assert_eq!(game.lost().control, 3);
}

#[test]
fn a_message_too_big_for_a_frame_is_left_out_and_the_link_stays_up() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let game = GameLink::start(to_game.clone(), to_launcher.clone(), "game");
    tool_hello(&to_game, omsi_devtools_proto::PROTOCOL_VERSION);
    wait("ready", || game.state() == LinkState::Ready);
    let mut big = snap(1, &[], &[]);
    big.vars = vec![(
        "big".into(),
        VarValue::Text("x".repeat(omsi_devtools_proto::MAX_FRAME_LEN + 1)),
    )];
    game.send_snapshot(big);
    wait("oversized", || game.lost().oversized == 1);
    assert_eq!(game.state(), LinkState::Ready);
    game.send_snapshot(snap(2, &[("a", 1.0)], &[]));
    let mut seen = vec![];
    wait("the next snapshot", || {
        seen.extend(frames(&drain(&to_launcher)));
        seen.iter()
            .any(|m| matches!(m, ToLauncher::Snapshot(s) if s.frame == 2))
    });
}

#[test]
fn folding_merges_several_variables_the_newest_wins_and_events_are_capped() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let game = GameLink::start(
        to_game.clone(),
        Gated(to_launcher.clone(), gate.clone()),
        "game",
    );
    tool_hello(&to_game, omsi_devtools_proto::PROTOCOL_VERSION);
    wait("ready", || game.state() == LinkState::Ready);
    // the writer takes the first message and sticks on the gate; the rest queue and fold
    game.send_snapshot(snap(0, &[], &[]));
    std::thread::sleep(Duration::from_millis(50));
    let mut first = snap(1, &[("a", 1.0), ("b", 1.0)], &[]);
    first.physics = Some(omsi_devtools_proto::PhysicsSnapshot {
        position: [1.0, 2.0, 3.0],
        velocity: [0.0; 3],
        wheels: vec![],
    });
    game.send_snapshot(first);
    for f in 2..(MAX_PENDING_SNAPSHOTS as u64 + 4) {
        // (b is only in the first; a changes every time; each carries many events)
        let names: Vec<String> = (0..MAX_EVENTS_PER_SNAPSHOT / 2)
            .map(|i| format!("{f}-{i}"))
            .collect();
        let refs: Vec<&str> = names.iter().map(|x| x.as_str()).collect();
        game.send_snapshot(snap(f, &[("a", f as f32)], &refs));
    }
    assert!(game.lost().events > 0, "the event cap was reached");
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    // the tool applies what it receives, in order: the result must be the latest of each
    let mut state: std::collections::HashMap<String, VarValue> = Default::default();
    let mut physics = None;
    let mut done = false;
    wait("the last snapshot", || {
        for m in frames(&drain(&to_launcher)) {
            if let ToLauncher::Snapshot(s) = m {
                assert!(s.events.len() <= MAX_EVENTS_PER_SNAPSHOT);
                for (k, v) in s.vars {
                    state.insert(k, v);
                }
                physics = s.physics.or(physics.take());
                done |= s.frame == MAX_PENDING_SNAPSHOTS as u64 + 3;
            }
        }
        done
    });
    assert_eq!(
        state.get("a"),
        Some(&VarValue::Float(MAX_PENDING_SNAPSHOTS as f32 + 3.0))
    );
    assert_eq!(state.get("b"), Some(&VarValue::Float(1.0)));
    assert!(
        physics.is_some(),
        "the physics of a folded snapshot is kept"
    );
}

#[test]
fn a_dropped_launcher_end_reports_closed_and_refuses_more_commands() {
    let (to_game, to_launcher) = (Pipe::default(), Pipe::default());
    let tool = LauncherLink::start(to_launcher, to_game, "tool");
    for _ in 0..256 {
        tool.send(Command::Reload { file_index: 0 });
    }
    // (not ready: the commands wait, the 257th is refused and counted)
    assert!(!tool.send(Command::Reload { file_index: 0 }));
    assert_eq!(tool.dropped_commands(), 1);
}
