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
    format!(
        "log truncated at {size}: of the lines after this one, only errors and the end of the game were written\n"
    )
}

/// What marks a line that is still written after the cap: an error, a panic, and the lines
/// that say how a game ended. The launcher reads the end of `game.log` for exactly these to
/// tell a crash from a clean end, so a log that is cut must not lose them.
pub const LAST_WORDS: [&str; 5] = [
    " ERROR ",
    "the game stopped on an error",
    "the graphics device was lost",
    "starting again with safer graphics",
    "game ends",
];

/// Whether `line` is one of the [`LAST_WORDS`].
pub fn is_last_word(line: &[u8]) -> bool {
    LAST_WORDS.iter().any(|word| {
        line.windows(word.len())
            .any(|piece| piece == word.as_bytes())
    })
}

/// The room a log keeps free for its [`LAST_WORDS`]: 1/64 of the cap (480 KB of 30 MB, a
/// panic with its backtrace many times over).
fn reserve(cap: u64) -> u64 {
    cap / 64
}

/// Whether `add` more bytes fit a log that holds `len`, the marker's room kept free: the
/// marker itself always fits, so a log that is cut says so.
fn fits(len: u64, add: usize, cap: u64) -> bool {
    len.saturating_add(add as u64)
        .saturating_add(truncation_marker(cap).len() as u64)
        <= cap
}

/// A log output that takes `cap` bytes at most. Each `write` is one whole line (the logger
/// hands over a line at a time). The last 1/64 of the cap is kept for the [`LAST_WORDS`]:
/// the first line that does not fit before it is replaced by the truncation marker, and of
/// the lines after it only those are written, while they fit the cap. Dropped lines are
/// reported as written, so the program goes on as before.
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
        if !self.full {
            if fits(
                self.written.saturating_add(reserve(self.cap)),
                buf.len(),
                self.cap,
            ) {
                self.inner.write_all(buf)?;
                self.written += buf.len() as u64;
                return Ok(buf.len());
            }
            self.full = true;
            let marker = truncation_marker(self.cap);
            self.inner.write_all(marker.as_bytes())?;
            self.written += marker.len() as u64;
        }
        // (past the marker: an error, a panic, the end of the game)
        if is_last_word(buf) && self.written.saturating_add(buf.len() as u64) <= self.cap {
            self.inner.write_all(buf)?;
            self.written += buf.len() as u64;
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
/// than two programs write at the same instant. Before that slack it keeps the same room as
/// [`Capped`] for the [`LAST_WORDS`], which are written after the marker too.
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
        // (a length that cannot be read is no reason to lose the line: it is written)
        let Ok(mut len) = self.file.metadata().map(|m| m.len()) else {
            self.file.write_all(buf)?;
            return Ok(buf.len());
        };
        let slack = self.cap / 64;
        let short = buf.len() as u64 <= slack;
        if !self.full {
            let taken = len.saturating_add(slack).saturating_add(reserve(self.cap));
            if short && fits(taken, buf.len(), self.cap) {
                self.file.write_all(buf)?;
                return Ok(buf.len());
            }
            self.full = true;
            // (the other program's marker may be there already: a second one goes in only
            // while the cap holds)
            let marker = truncation_marker(self.cap);
            if len + marker.len() as u64 <= self.cap {
                self.file.write_all(marker.as_bytes())?;
                len += marker.len() as u64;
            }
        }
        // (past the marker: an error, a panic)
        if short
            && is_last_word(buf)
            && len.saturating_add(slack).saturating_add(buf.len() as u64) <= self.cap
        {
            self.file.write_all(buf)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.file.flush()
    }
}
