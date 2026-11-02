use crate::{io, state, LinkState};
use omsi_devtools_proto::{check_hello, Command, Hello, ToGame, ToLauncher, PROTOCOL_VERSION};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

/// The launcher's end of the link: what the tool window talks to.
pub struct LauncherLink {
    commands: Sender<Command>,
    incoming: Receiver<ToLauncher>,
    state: Arc<state::Cell>,
}

impl LauncherLink {
    /// Starts the link over `reader` and `writer`, the two halves of the stream the launcher
    /// owns for this run. Same handshake rule as the game's end.
    pub fn start<R, W>(reader: R, writer: W, build: impl Into<String>) -> LauncherLink
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let state = Arc::new(state::Cell::new());
        let (cmd_tx, cmd_rx) = channel::<Command>();
        let (in_tx, in_rx) = channel::<ToLauncher>();
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
                // (ends when the link is dropped: the sender is gone)
                while let Ok(c) = cmd_rx.recv() {
                    if matches!(s.get(), LinkState::Refused | LinkState::Closed) {
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
                    _ => {
                        s.refuse();
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
        }
    }

    pub fn state(&self) -> LinkState {
        self.state.get()
    }

    /// Asks the game to do `command`. The game checks it and answers `Applied` or `Rejected`.
    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
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
