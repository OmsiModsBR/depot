//! A record being written: the messages of one session as lines, up to the cap.

use crate::line::{command_line, event_line, header_line, snapshot_line, TRUNCATED_LINE};
use omsi_devtools_proto::{Hello, ToLauncher};
use std::io::{Result, Write};
use std::time::SystemTime;

/// The most a run record file holds: 50 MB.
pub const RECORD_CAP_BYTES: u64 = 50 * 1024 * 1024;

/// One session's record. The first line that does not fit is replaced by the truncated line,
/// whose room is always kept free, and every line after it is dropped: a dropped line is no
/// error, the tools go on.
pub struct RunRecord<W: Write> {
    out: W,
    cap: u64,
    written: u64,
    truncated: bool,
    commands: u64,
}

impl<W: Write> RunRecord<W> {
    /// Starts a record on `out`, at most `cap` bytes long, with its header line: the game's
    /// hello and the time the record started.
    pub fn start(out: W, cap: u64, game: &Hello, started: SystemTime) -> Result<Self> {
        let mut record = Self {
            out,
            cap,
            written: 0,
            truncated: false,
            commands: 0,
        };
        record.line(&header_line(game, started))?;
        Ok(record)
    }

    /// Records a message of the game: a snapshot as its line and a line for each of its
    /// events, an applied command as its line. A hello and a rejected command are no part
    /// of the record.
    pub fn message(&mut self, message: &ToLauncher) -> Result<()> {
        match message {
            ToLauncher::Snapshot(snapshot) => {
                if let Some(line) = snapshot_line(snapshot) {
                    self.line(&line)?;
                }
                for event in &snapshot.events {
                    self.line(&event_line(event))?;
                }
            }
            ToLauncher::Applied { frame, command } => {
                self.line(&command_line(*frame, command))?;
                if !self.truncated {
                    self.commands += 1;
                }
            }
            ToLauncher::Hello(_) | ToLauncher::Rejected { .. } => {}
        }
        Ok(())
    }

    /// How many applied commands the record holds. A record with any is of a run the tool
    /// changed, which is no 1:1 comparison run.
    pub fn commands(&self) -> u64 {
        self.commands
    }

    /// Whether the record reached its cap and ends with the truncated line.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// The output, flushed (what a test reads back).
    pub fn finish(mut self) -> Result<W> {
        self.out.flush()?;
        Ok(self.out)
    }

    fn line(&mut self, line: &str) -> Result<()> {
        if self.truncated {
            return Ok(());
        }
        let after = self.written + line.len() as u64 + TRUNCATED_LINE.len() as u64;
        if after <= self.cap {
            self.out.write_all(line.as_bytes())?;
            self.written += line.len() as u64;
        } else {
            self.truncated = true;
            self.out.write_all(TRUNCATED_LINE.as_bytes())?;
            self.written += TRUNCATED_LINE.len() as u64;
        }
        Ok(())
    }
}
