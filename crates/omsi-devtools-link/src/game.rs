use crate::{io, state, LinkError, LinkState};
use omsi_devtools_proto::{
    check_hello, validate_command, Command, FrameError, Hello, Snapshot, ToGame, ToLauncher,
    PROTOCOL_VERSION,
};
use std::collections::{HashSet, VecDeque};
use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Snapshots waiting for a slow launcher. Past this the oldest is folded into the next one.
pub const MAX_PENDING_SNAPSHOTS: usize = 8;
/// Other messages (applied or rejected commands) waiting to be sent, and commands waiting to
/// be read. Past this the newest is dropped and counted.
pub const MAX_PENDING_CONTROL: usize = 256;
/// Events one snapshot may carry. Folding past this drops the oldest events and counts them.
pub const MAX_EVENTS_PER_SNAPSHOT: usize = 1024;

/// What the link had to throw away because the launcher or the tool was behind. Every count
/// is also a gap the run record can mark.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Lost {
    /// Applied/rejected messages dropped because their queue was full.
    pub control: u64,
    /// Commands from the tool dropped (answered `Rejected`) because the inbox was full.
    pub commands: u64,
    /// Events dropped from a folded snapshot past `MAX_EVENTS_PER_SNAPSHOT`.
    pub events: u64,
    /// Messages not sent because one frame cannot hold them.
    pub oversized: u64,
}

#[derive(Default)]
struct Queues {
    snapshots: VecDeque<Snapshot>,
    control: VecDeque<ToLauncher>,
    closed: bool,
    folded: u64,
    lost: Lost,
}

impl Queues {
    fn push_control(&mut self, msg: ToLauncher) -> bool {
        if self.control.len() >= MAX_PENDING_CONTROL {
            self.lost.control += 1;
            return false;
        }
        self.control.push_back(msg);
        true
    }
}

struct Shared {
    q: Mutex<Queues>,
    wake: Condvar,
    inbox: Mutex<VecDeque<Command>>,
    state: state::Cell,
}

impl Shared {
    /// A state change the writer must not miss: made under the queue lock, which the writer
    /// holds between looking at the state and waiting.
    fn change(&self, f: impl FnOnce(&state::Cell)) {
        let q = self.q.lock().unwrap();
        f(&self.state);
        drop(q);
        self.wake.notify_all();
    }
    fn close(&self) {
        let mut q = self.q.lock().unwrap();
        q.closed = true;
        self.state.close();
        drop(q);
        self.wake.notify_all();
    }
}

/// The game's end of the link. Every call returns without waiting for the launcher or the
/// stream, and holds a lock only for a queue push or a drain.
pub struct GameLink {
    shared: Arc<Shared>,
}

impl GameLink {
    /// Starts the link over `reader` and `writer` (the two halves of one stream). Two threads
    /// take the blocking reads and writes. Nothing but the hello is sent before the other
    /// side's hello has passed `check_hello`, and nothing is read as a command before it.
    /// The reader thread ends when the stream does (a blocked read cannot be interrupted
    /// portably), so a stream the other side never closes keeps it, and only it, alive.
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

    fn live(&self) -> bool {
        !matches!(self.state(), LinkState::Refused | LinkState::Closed)
    }

    /// Queues a snapshot for the launcher. When the launcher is behind, the oldest waiting
    /// snapshot is folded into the next one: the latest value of each variable and the events
    /// are kept (up to `MAX_EVENTS_PER_SNAPSHOT`, past which the oldest events are dropped
    /// and counted in `lost()`), and only the in-between frames are gone.
    pub fn send_snapshot(&self, snap: Snapshot) {
        if !self.live() {
            return;
        }
        let mut q = self.shared.q.lock().unwrap();
        if q.snapshots.len() >= MAX_PENDING_SNAPSHOTS {
            if let Some(old) = q.snapshots.pop_front() {
                q.folded += 1;
                let mut lost = q.lost;
                let next = q.snapshots.front_mut().expect("capacity is above one");
                fold(old, next, &mut lost);
                q.lost = lost;
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
        if !self.live() {
            return;
        }
        let pushed = self.shared.q.lock().unwrap().push_control(msg);
        if pushed {
            self.shared.wake.notify_one();
        }
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

    /// What was thrown away so far.
    pub fn lost(&self) -> Lost {
        self.shared.q.lock().unwrap().lost
    }
}

impl Drop for GameLink {
    fn drop(&mut self) {
        // (the writer ends now; the reader ends with the stream)
        self.shared.close();
    }
}

/// `old` is about to be dropped: what it said that `next` does not say is moved into `next`.
fn fold(old: Snapshot, next: &mut Snapshot, lost: &mut Lost) {
    let newer: HashSet<&str> = next.vars.iter().map(|(k, _)| k.as_str()).collect();
    let mut vars: Vec<_> = old
        .vars
        .into_iter()
        .filter(|(k, _)| !newer.contains(k.as_str()))
        .collect();
    vars.append(&mut next.vars);
    next.vars = vars;
    let mut events = old.events;
    events.append(&mut next.events);
    if events.len() > MAX_EVENTS_PER_SNAPSHOT {
        let extra = events.len() - MAX_EVENTS_PER_SNAPSHOT;
        events.drain(..extra);
        lost.events += extra as u64;
    }
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
                if q.closed {
                    return;
                }
                match s.state.get() {
                    LinkState::Refused | LinkState::Closed => return,
                    // nothing else is sent to a peer whose hello is not accepted yet
                    LinkState::Connecting => {}
                    LinkState::Ready => {
                        if let Some(m) = q.control.pop_front() {
                            break m;
                        }
                        if let Some(m) = q.snapshots.pop_front() {
                            break ToLauncher::Snapshot(m);
                        }
                    }
                }
                q = s.wake.wait_timeout(q, Duration::from_secs(1)).unwrap().0;
            }
        };
        match io::write_frame(&mut w, &msg) {
            Ok(()) => {}
            // one message too big for a frame is left out and counted; the link stays up
            Err(LinkError::Frame(FrameError::TooLong(_))) => {
                s.q.lock().unwrap().lost.oversized += 1;
            }
            Err(_) => {
                s.close();
                return;
            }
        }
    }
}

fn read_loop<R: Read>(s: Arc<Shared>, mut r: R) {
    match io::read_frame::<ToGame, _>(&mut r) {
        Ok(ToGame::Hello(h)) if check_hello(&h).is_ok() => s.change(|c| c.ready()),
        // another version, or a first message that is not a hello
        Ok(_) | Err(LinkError::Frame(_)) => {
            s.change(|c| c.refuse());
            return;
        }
        // the stream ended or failed before a hello
        Err(_) => {
            s.close();
            return;
        }
    }
    loop {
        let m = io::read_frame::<ToGame, _>(&mut r);
        if s.state.get() == LinkState::Closed {
            return;
        }
        match m {
            Ok(ToGame::Command(c)) => {
                let mut inbox = s.inbox.lock().unwrap();
                if inbox.len() >= MAX_PENDING_CONTROL {
                    drop(inbox);
                    let mut q = s.q.lock().unwrap();
                    q.lost.commands += 1;
                    let pushed = q.push_control(ToLauncher::Rejected {
                        reason: "too many commands waiting".into(),
                    });
                    drop(q);
                    if pushed {
                        s.wake.notify_one();
                    }
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
