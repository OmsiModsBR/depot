use crate::{io, state, LinkError, LinkState};
use omsi_devtools_proto::{
    check_hello, validate_command, Command, Hello, Snapshot, ToGame, ToLauncher, PROTOCOL_VERSION,
};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};

/// Snapshots waiting for a slow launcher. Past this the oldest is folded into the next one.
pub const MAX_PENDING_SNAPSHOTS: usize = 8;
/// Other messages (applied or rejected commands) waiting to be sent, and commands waiting to
/// be read. Past this the newest is dropped and counted.
pub const MAX_PENDING_CONTROL: usize = 256;

#[derive(Default)]
struct Queues {
    snapshots: VecDeque<Snapshot>,
    control: VecDeque<ToLauncher>,
    closed: bool,
    folded: u64,
    dropped: u64,
}

struct Shared {
    q: Mutex<Queues>,
    wake: Condvar,
    inbox: Mutex<VecDeque<Command>>,
    state: state::Cell,
}

impl Shared {
    fn close(&self) {
        self.state.close();
        self.q.lock().unwrap().closed = true;
        self.wake.notify_all();
    }
}

/// The game's end of the link. Every call returns at once: the game's frame never waits for
/// the launcher, the stream or a lock held for longer than a queue push.
pub struct GameLink {
    shared: Arc<Shared>,
}

impl GameLink {
    /// Starts the link over `reader` and `writer` (the two halves of one stream). Two threads
    /// take the blocking reads and writes; nothing is sent before the hello, and nothing
    /// else is read before the other side's hello has passed `check_hello`.
    pub fn start<R, W>(reader: R, writer: W, build: impl Into<String>) -> GameLink
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let shared = Arc::new(Shared {
            q: Mutex::new(Queues::default()),
            wake: Condvar::new(),
            inbox: Mutex::new(VecDeque::new()),
            state: state::Cell::new(),
        });
        let hello = Hello {
            protocol: PROTOCOL_VERSION,
            build: build.into(),
        };
        let s = shared.clone();
        std::thread::Builder::new()
            .name("devtools-write".into())
            .spawn(move || write_loop(s, writer, hello))
            .expect("thread");
        let s = shared.clone();
        std::thread::Builder::new()
            .name("devtools-read".into())
            .spawn(move || read_loop(s, reader))
            .expect("thread");
        GameLink { shared }
    }

    pub fn state(&self) -> LinkState {
        self.shared.state.get()
    }

    /// Queues a snapshot for the launcher. When the launcher is behind, the oldest waiting
    /// snapshot is folded into the next one, so no variable change and no event is lost, only
    /// the in-between frames.
    pub fn send_snapshot(&self, snap: Snapshot) {
        if matches!(self.state(), LinkState::Refused | LinkState::Closed) {
            return;
        }
        let mut q = self.shared.q.lock().unwrap();
        if q.snapshots.len() >= MAX_PENDING_SNAPSHOTS {
            if let Some(old) = q.snapshots.pop_front() {
                q.folded += 1;
                let next = q.snapshots.front_mut().expect("capacity is above one");
                fold(old, next);
            }
        }
        q.snapshots.push_back(snap);
        drop(q);
        self.shared.wake.notify_one();
    }

    /// Tells the launcher a command was applied at `frame` (the run record marks it).
    pub fn send_applied(&self, frame: u64, command: Command) {
        self.send_control(ToLauncher::Applied { frame, command });
    }

    pub fn send_rejected(&self, reason: impl Into<String>) {
        self.send_control(ToLauncher::Rejected {
            reason: reason.into(),
        });
    }

    fn send_control(&self, msg: ToLauncher) {
        if matches!(self.state(), LinkState::Refused | LinkState::Closed) {
            return;
        }
        let mut q = self.shared.q.lock().unwrap();
        if q.control.len() >= MAX_PENDING_CONTROL {
            q.dropped += 1;
            return;
        }
        q.control.push_back(msg);
        drop(q);
        self.shared.wake.notify_one();
    }

    /// The commands that arrived since the last call, without waiting. Each one is checked
    /// with `validate_command` (`reload_files` is the length of the game's reload list); a
    /// command that fails is answered with `Rejected` and left out. The game applies the rest
    /// at the start of its frame.
    pub fn poll_commands(&self, reload_files: u32) -> Vec<Command> {
        let taken: Vec<Command> = self.shared.inbox.lock().unwrap().drain(..).collect();
        let mut ok = Vec::with_capacity(taken.len());
        for c in taken {
            match validate_command(&c, reload_files) {
                Ok(()) => ok.push(c),
                Err(e) => self.send_rejected(e.to_string()),
            }
        }
        ok
    }

    /// Snapshots folded into a later one because the launcher was behind.
    pub fn folded_snapshots(&self) -> u64 {
        self.shared.q.lock().unwrap().folded
    }

    /// Messages and commands dropped because their queue was full.
    pub fn dropped_messages(&self) -> u64 {
        self.shared.q.lock().unwrap().dropped
    }
}

impl Drop for GameLink {
    fn drop(&mut self) {
        // (the reader may be blocked in the stream; it ends when the stream does)
        self.shared.close();
    }
}

/// `old` is about to be dropped: what it said that `next` does not say is moved into `next`.
fn fold(old: Snapshot, next: &mut Snapshot) {
    let mut vars = old.vars;
    vars.retain(|(k, _)| !next.vars.iter().any(|(n, _)| n == k));
    vars.append(&mut next.vars);
    next.vars = vars;
    let mut events = old.events;
    events.append(&mut next.events);
    next.events = events;
    if next.physics.is_none() {
        next.physics = old.physics;
    }
}

fn write_loop<W: Write>(s: Arc<Shared>, mut w: W, hello: Hello) {
    if io::write_frame(&mut w, &ToLauncher::Hello(hello)).is_err() {
        s.close();
        return;
    }
    loop {
        let msg = {
            let mut q = s.q.lock().unwrap();
            loop {
                if q.closed || matches!(s.state.get(), LinkState::Refused) {
                    return;
                }
                if let Some(m) = q.control.pop_front() {
                    break m;
                }
                if let Some(m) = q.snapshots.pop_front() {
                    break ToLauncher::Snapshot(m);
                }
                q = s.wake.wait(q).unwrap();
            }
        };
        if io::write_frame(&mut w, &msg).is_err() {
            s.close();
            return;
        }
    }
}

fn read_loop<R: Read>(s: Arc<Shared>, mut r: R) {
    let first: Result<ToGame, LinkError> = io::read_frame(&mut r);
    match first {
        Ok(ToGame::Hello(h)) if check_hello(&h).is_ok() => s.state.ready(),
        _ => {
            s.state.refuse();
            s.wake.notify_all();
            return;
        }
    }
    loop {
        match io::read_frame::<ToGame, _>(&mut r) {
            Ok(ToGame::Command(c)) => {
                let mut inbox = s.inbox.lock().unwrap();
                if inbox.len() >= MAX_PENDING_CONTROL {
                    drop(inbox);
                    s.q.lock().unwrap().dropped += 1;
                } else {
                    inbox.push_back(c);
                }
            }
            // a second hello is not part of the protocol
            Ok(ToGame::Hello(_)) | Err(_) => {
                s.close();
                return;
            }
        }
    }
}
