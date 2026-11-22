//! The size cap of a log file: a file stops growing at the cap, and its last line says so.

use std::fs::File;
use std::io::{Result, Write};
use std::path::Path;

/// The most a text log file holds (`game.log`, `launcher.log`): 30 MB.
pub const LOG_CAP_BYTES: u64 = 30 * 1024 * 1024;

/// The last line of a log that reached the cap of `cap` bytes.
pub fn truncation_marker(cap: u64) -> String {
    const MB: u64 = 1024 * 1024;
    let size = if cap >= MB && cap % MB == 0 {
        format!("{} MB", cap / MB)
    } else {
        format!("{cap} bytes")
    };
    format!("log truncated at {size}: the lines after this one were not written\n")
}

/// Whether `add` more bytes fit a log that holds `len`, the marker's room kept free: the
/// marker itself always fits, so a log that is cut says so.
fn fits(len: u64, add: usize, cap: u64) -> bool {
    len.saturating_add(add as u64)
        .saturating_add(truncation_marker(cap).len() as u64)
        <= cap
}

/// A log output that takes `cap` bytes at most. Each `write` is one whole line (the logger
/// hands over a line at a time): the first line that does not fit is replaced by the
/// truncation marker, and every line after it is dropped. Dropped lines are reported as
/// written, so the program goes on as before.
pub struct Capped<W: Write> {
    inner: W,
    cap: u64,
    written: u64,
    full: bool,
}

impl<W: Write> Capped<W> {
    /// `inner` capped at `cap` bytes, nothing written yet.
    pub fn new(inner: W, cap: u64) -> Self {
        Self {
            inner,
            cap,
            written: 0,
            full: false,
        }
    }

    /// The output itself (what a test reads back).
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for Capped<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.full {
            return Ok(buf.len());
        }
        if fits(self.written, buf.len(), self.cap) {
            self.inner.write_all(buf)?;
            self.written += buf.len() as u64;
        } else {
            self.full = true;
            self.inner
                .write_all(truncation_marker(self.cap).as_bytes())?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

/// A log file that several programs append to (`launcher.log`: the launcher and the preview
/// helper), capped by the file's own length, which holds every writer's lines. A writer whose
/// line does not fit writes the marker once and stops.
///
/// Another program's line can land between this one's look at the length and its write, so
/// a shared file keeps a slack of 1/64 of the cap free on top of the marker's room, and
/// takes no line longer than that slack: the file then stays within the cap while no more
/// than two programs write at the same instant.
pub struct CappedFile {
    file: File,
    cap: u64,
    full: bool,
}

impl CappedFile {
    /// The file `path`, opened to append (made when it is not there), capped at `cap` bytes.
    pub fn open(path: &Path, cap: u64) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            file,
            cap,
            full: false,
        })
    }
}

impl Write for CappedFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.full {
            return Ok(buf.len());
        }
        let len = self.file.metadata()?.len();
        let slack = self.cap / 64;
        if buf.len() as u64 <= slack && fits(len.saturating_add(slack), buf.len(), self.cap) {
            self.file.write_all(buf)?;
        } else {
            self.full = true;
            // (the other program's marker may be there already: a second one goes in only
            // while the cap holds)
            let marker = truncation_marker(self.cap);
            if len + marker.len() as u64 <= self.cap {
                self.file.write_all(marker.as_bytes())?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.file.flush()
    }
}
