use crate::{io, state, LinkError, LinkState};
use omsi_devtools_proto::{check_hello, Command, Hello, ToGame, ToLauncher, PROTOCOL_VERSION};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Duration;

/// Messages from the game waiting for the tool window. When the window does not read, the
/// link stops reading the stream, so the game's own queue (which is bounded, and never blocks
/// its frame) takes the lag.
pub const MAX_PENDING_FROM_GAME: usize = 1024;
const MAX_PENDING_COMMANDS: usize = 256;

/// The launcher's end of the link: what the tool window talks to.
pub struct LauncherLink {
    commands: SyncSender<Command>,
    incoming: Receiver<ToLauncher>,
    state: Arc<state::Cell>,
    dropped: Arc<AtomicU64>,
}

impl LauncherLink {
    /// Starts the link over `reader` and `writer`, the two halves of the stream the launcher
    /// owns for this run. Same handshake rule as the game's end: commands wait until the
    /// game's hello is accepted.
    pub fn start<R, W>(reader: R, writer: W, build: impl Into<String>) -> LauncherLink
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let state = Arc::new(state::Cell::new());
        let (cmd_tx, cmd_rx) = sync_channel::<Command>(MAX_PENDING_COMMANDS);
        let (in_tx, in_rx) = sync_channel::<ToLauncher>(MAX_PENDING_FROM_GAME);
        let hello = Hello {
            protocol: PROTOCOL_VERSION,
            build: build.into(),
        };
        let s = state.clone();
        std::thread::Builder::new()
            .name("devtools-write".into())
            .spawn(move || {
                let mut w = writer;
                if io::write_frame(&mut w, &ToGame::Hello(hello)).is_err() {
                    s.close();
                    return;
                }
                loop {
                    match s.wait_connected(Duration::from_millis(50)) {
                        LinkState::Connecting => continue,
                        LinkState::Ready => break,
                        _ => return,
                    }
                }
                // (ends when the link is dropped: the sender is gone)
                while let Ok(c) = cmd_rx.recv() {
                    if s.get() != LinkState::Ready {
                        return;
                    }
                    if io::write_frame(&mut w, &ToGame::Command(c)).is_err() {
                        s.close();
                        return;
                    }
                }
            })
            .expect("thread");
        let s = state.clone();
        std::thread::Builder::new()
            .name("devtools-read".into())
            .spawn(move || {
                let mut r = reader;
                match io::read_frame::<ToLauncher, _>(&mut r) {
                    Ok(ToLauncher::Hello(h)) if check_hello(&h).is_ok() => s.ready(),
                    Ok(_) | Err(LinkError::Frame(_)) => {
                        s.refuse();
                        return;
                    }
                    Err(_) => {
                        s.close();
                        return;
                    }
                }
                loop {
                    match io::read_frame::<ToLauncher, _>(&mut r) {
                        Ok(ToLauncher::Hello(_)) | Err(_) => {
                            s.close();
                            return;
                        }
                        Ok(m) => {
                            // (blocks while the window is behind: see MAX_PENDING_FROM_GAME)
                            if in_tx.send(m).is_err() {
                                return;
                            }
                        }
                    }
                }
            })
            .expect("thread");
        LauncherLink {
            commands: cmd_tx,
            incoming: in_rx,
            state,
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn state(&self) -> LinkState {
        self.state.get()
    }

    /// Asks the game to do `command`. The game checks it and answers `Applied` or `Rejected`.
    /// Returns false when too many commands are waiting (nothing is queued) or the link has
    /// ended.
    pub fn send(&self, command: Command) -> bool {
        match self.commands.try_send(command) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Commands refused by `send` because too many were waiting.
    pub fn dropped_commands(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// The next message from the game, without waiting.
    pub fn try_recv(&self) -> Option<ToLauncher> {
        self.incoming.try_recv().ok()
    }

    /// The next message from the game, waiting at most `wait`.
    pub fn recv_timeout(&self, wait: Duration) -> Option<ToLauncher> {
        self.incoming.recv_timeout(wait).ok()
    }
}

impl Drop for LauncherLink {
    fn drop(&mut self) {
        // (the writer ends with the command sender; the reader ends with the stream)
        self.state.close();
    }
}
