use omsi_devtools_link::{GameLink, LauncherLink, LinkState, MAX_PENDING_SNAPSHOTS};
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
