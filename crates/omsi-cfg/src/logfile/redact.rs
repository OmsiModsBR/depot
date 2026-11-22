//! Private names kept out of the logs: the user's folder becomes a placeholder, and so do the
//! account's and the computer's names, in every log line and in every text the program
//! hands out for a report.

use std::borrow::Cow;
use std::sync::OnceLock;

/// What stands for the user's folder (the home or profile folder) in a log.
pub const USER_FOLDER: &str = "USER_FOLDER";
/// What stands for the account's name in a path outside the user's folder
/// (`/run/media/<name>/...`).
pub const USER_NAME: &str = "USER_NAME";
/// What stands for the computer's name in a path (`\\\\<name>\\share`).
pub const COMPUTER_NAME: &str = "COMPUTER_NAME";

/// The shortest name replaced on its own: a shorter one is a word of ordinary text more often
/// than a name (the user's folder is replaced whatever its length).
const SHORTEST_NAME: usize = 3;

/// One piece of a folder as it is looked for in a text.
#[derive(Debug, Clone, PartialEq)]
enum Piece {
    /// This character, in either letter case.
    Char(char),
    /// One or more path separators, `/` or `\` (a path written by the debug format doubles
    /// its backslashes).
    Separators,
}

/// The private names of one machine, and their replacement in a text.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    /// The user's folders, the longest first.
    folders: Vec<Vec<Piece>>,
    /// For each user's folder, the folder it is in (with the separator after it) and its
    /// own name as Windows shortens one (see [`short_name_at`]).
    short: Vec<(Vec<Piece>, Vec<char>)>,
    /// The names replaced where they are a piece of a path, with what stands for each.
    names: Vec<(Vec<char>, &'static str)>,
    /// The first character of every folder and name, lower-cased where it is ASCII.
    firsts: Vec<char>,
}

impl Redactor {
    /// The redactor for the user's folders `folders` (as the system gives them, any slash
    /// style), the account's name and the computer's name.
    pub fn new(folders: &[&str], account: Option<&str>, computer: Option<&str>) -> Self {
        let mut pieces: Vec<Vec<Piece>> = folders
            .iter()
            .map(|f| folder_pieces(f))
            // (a folder of one piece, `/` or `C:\`, is nobody's own)
            .filter(|p| p.iter().filter(|x| **x == Piece::Separators).count() >= 1 && p.len() > 3)
            .collect();
        pieces.sort_by_key(|p| std::cmp::Reverse(p.len()));
        pieces.dedup();
        let short = pieces.iter().filter_map(|p| short_form(p)).collect();
        let mut names = Vec::new();
        for (name, stands) in [(account, USER_NAME), (computer, COMPUTER_NAME)] {
            let name: Vec<char> = name.unwrap_or("").trim().chars().collect();
            if name.len() >= SHORTEST_NAME && !names.iter().any(|(n, _)| *n == name) {
                names.push((name, stands));
            }
        }
        let mut firsts: Vec<char> = pieces
            .iter()
            .flat_map(|p| match p.first() {
                Some(Piece::Char(c)) => vec![c.to_ascii_lowercase()],
                // (a folder that starts with its separator: either slash starts it)
                Some(Piece::Separators) => vec!['/', '\\'],
                None => Vec::new(),
            })
            .chain(names.iter().map(|(n, _)| n[0].to_ascii_lowercase()))
            .collect();
        firsts.sort_unstable();
        firsts.dedup();
        Self {
            folders: pieces,
            short,
            names,
            firsts,
        }
    }

    /// The redactor of this machine, from its environment: the home folder (`HOME`,
    /// `USERPROFILE`), the account (`USERNAME`, `USER`, `LOGNAME`) and the computer
    /// (`COMPUTERNAME`, `HOSTNAME`, on Linux `/etc/hostname`).
    pub fn of_this_machine() -> &'static Redactor {
        static OWN: OnceLock<Redactor> = OnceLock::new();
        OWN.get_or_init(|| {
            let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
            let folders: Vec<String> = ["HOME", "USERPROFILE"]
                .into_iter()
                .filter_map(var)
                .collect();
            let account = ["USERNAME", "USER", "LOGNAME"].into_iter().find_map(var);
            let computer = ["COMPUTERNAME", "HOSTNAME"]
                .into_iter()
                .find_map(var)
                .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
                .map(|c| c.trim().to_string());
            let folders: Vec<&str> = folders.iter().map(String::as_str).collect();
            Redactor::new(&folders, account.as_deref(), computer.as_deref())
        })
    }

    /// `text` with the user's folder replaced by `USER_FOLDER` (the rest of a path stays),
    /// and the account's and the computer's names, where they are a piece of a path, by
    /// their placeholders. A text that holds none of them comes back as it is, with nothing
    /// copied.
    pub fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let mut out = String::new();
        let mut copied = 0;
        let mut before: Option<char> = None;
        let mut at = 0;
        while let Some(c) = text[at..].chars().next() {
            // (most characters start nothing: an ASCII one that is no folder's and no
            // name's first is passed at once)
            if c.is_ascii() && !self.firsts.contains(&c.to_ascii_lowercase()) {
                before = Some(c);
                at += 1;
                continue;
            }
            let here = &text[at..];
            let found = self
                .folders
                .iter()
                .find_map(|f| folder_at(here, f, before).map(|len| (len, USER_FOLDER)))
                .or_else(|| {
                    self.short.iter().find_map(|(parent, name)| {
                        short_folder_at(here, parent, name, before).map(|len| (len, USER_FOLDER))
                    })
                })
                .or_else(|| {
                    self.names.iter().find_map(|(name, stands)| {
                        name_at(here, name, before).map(|len| (len, *stands))
                    })
                });
            match found {
                Some((len, stands)) => {
                    out.push_str(&text[copied..at]);
                    out.push_str(stands);
                    at += len;
                    copied = at;
                    before = text[..at].chars().next_back();
                }
                None => {
                    before = Some(c);
                    at += c.len_utf8();
                }
            }
        }
        if copied == 0 {
            return Cow::Borrowed(text);
        }
        out.push_str(&text[copied..]);
        Cow::Owned(out)
    }
}

/// A folder as it is looked for: its characters, each run of separators as one piece, the
/// separators at its end left out.
fn folder_pieces(folder: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for c in folder.trim().chars() {
        if c == '/' || c == '\\' {
            if pieces.last() != Some(&Piece::Separators) {
                pieces.push(Piece::Separators);
            }
        } else {
            pieces.push(Piece::Char(c));
        }
    }
    while pieces.len() > 1 && pieces.last() == Some(&Piece::Separators) {
        pieces.pop();
    }
    pieces
}

/// Whether two characters are the same letter, whatever their case (a drive letter, a
/// Windows folder name).
fn same_letter(a: char, b: char) -> bool {
    a == b
        || if a.is_ascii() && b.is_ascii() {
            a.eq_ignore_ascii_case(&b)
        } else {
            a.to_lowercase().eq(b.to_lowercase())
        }
}

/// A character a folder's or a name's last one may be followed by: not one that would make
/// it another, longer name (`/home/ann` in `/home/anna`).
fn ends_a_name(next: Option<char>) -> bool {
    !next.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// How many bytes of `text`, from its start, are the pieces `pieces`, whatever follows; None
/// when it does not start with them. A space of the folder is also met as `%20` (a path
/// written as a `file:` address).
fn pieces_at(text: &str, pieces: &[Piece]) -> Option<usize> {
    let mut len = 0;
    for piece in pieces {
        let rest = &text[len..];
        match piece {
            Piece::Char(' ') if rest.starts_with("%20") => len += 3,
            Piece::Char(want) => {
                let c = rest.chars().next()?;
                if !same_letter(c, *want) {
                    return None;
                }
                len += c.len_utf8();
            }
            Piece::Separators => {
                let run = rest.chars().take_while(|c| *c == '/' || *c == '\\').count();
                if run == 0 {
                    return None;
                }
                len += run;
            }
        }
    }
    Some(len)
}

/// Whether a folder may start after the character `before`: not in the middle of another
/// path (`/home/ann` in `/mnt/home/ann`), which is a folder of its own.
fn starts_a_folder(folder: &[Piece], before: Option<char>) -> bool {
    match folder.first() {
        Some(Piece::Separators) => ends_a_name(before),
        // (a drive letter: no letter before it)
        _ => !before.is_some_and(|c| c.is_alphanumeric()),
    }
}

/// How many bytes of `text`, from its start, are the folder `folder` (`before` is the
/// character before it); None when it does not start with it.
fn folder_at(text: &str, folder: &[Piece], before: Option<char>) -> Option<usize> {
    if !starts_a_folder(folder, before) {
        return None;
    }
    let len = pieces_at(text, folder)?;
    ends_a_name(text[len..].chars().next()).then_some(len)
}

/// A user's folder split for its short Windows form: the folder it is in, with the
/// separator after it, and its own name without the characters a short name leaves out
/// (spaces and dots). None for a folder that is in no other.
fn short_form(folder: &[Piece]) -> Option<(Vec<Piece>, Vec<char>)> {
    let cut = folder.iter().rposition(|p| *p == Piece::Separators)?;
    let name: Vec<char> = folder[cut + 1..]
        .iter()
        .filter_map(|p| match p {
            Piece::Char(c) if *c != ' ' && *c != '.' => Some(*c),
            _ => None,
        })
        .collect();
    (cut > 0 && !name.is_empty()).then(|| (folder[..=cut].to_vec(), name))
}

/// How many bytes of `text`, from its start, are the short name Windows gives the folder
/// name `name` (`JONATH~1` for `Jonathan Smith`: up to six of its characters, `~` and a
/// number; with many folders alike, two of its characters, four of a checksum, `~` and a
/// number). The temporary folder is often given with it. None when `text` starts with no
/// such name.
fn short_name_at(text: &str, name: &[char]) -> Option<usize> {
    let end = text
        .find(|c: char| !(c.is_alphanumeric() || matches!(c, '~' | '.' | '_' | '-' | '$')))
        .unwrap_or(text.len());
    let (stem, number) = text[..end].split_once('~')?;
    let stem: Vec<char> = stem.chars().collect();
    let same = stem
        .iter()
        .zip(name)
        .take_while(|(a, b)| same_letter(**a, **b))
        .count();
    let own = !stem.is_empty()
        && stem.len() <= 6
        && same >= name.len().min(2)
        && stem[same..].iter().all(|c| c.is_ascii_hexdigit());
    // (the number, and an extension a folder with a dot in its name keeps)
    let numbered = number.starts_with(|c: char| c.is_ascii_digit())
        && number
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.');
    (own && numbered).then_some(end)
}

/// How many bytes of `text`, from its start, are a user's folder with its own name in the
/// short Windows form: the folder it is in (`parent`), then the short name of `name`.
fn short_folder_at(
    text: &str,
    parent: &[Piece],
    name: &[char],
    before: Option<char>,
) -> Option<usize> {
    if !starts_a_folder(parent, before) {
        return None;
    }
    let len = pieces_at(text, parent)?;
    short_name_at(&text[len..], name).map(|short| len + short)
}

/// How many bytes of `text`, from its start, are the name `name` as a whole piece of a path
/// (`before` is the character before it): a separator stands before or after it. None when
/// it does not start with it. A name in running text is left alone: the program never
/// writes the account's or the computer's name itself, and an account called `user` or
/// `root` must not take those words out of every line.
fn name_at(text: &str, name: &[char], before: Option<char>) -> Option<usize> {
    if !ends_a_name(before) {
        return None;
    }
    let mut rest = text.chars();
    let mut len = 0;
    for want in name {
        let c = rest.next()?;
        if !same_letter(c, *want) {
            return None;
        }
        len += c.len_utf8();
    }
    let after = rest.next();
    let separator = |c: Option<char>| matches!(c, Some('/') | Some('\\'));
    (ends_a_name(after) && (separator(before) || separator(after))).then_some(len)
}

/// A log output whose lines go through a [`Redactor`] first. Each `write` is one whole line
/// (the logger hands over a line at a time), so a name is never cut in two.
pub struct Redacting<W: std::io::Write> {
    inner: W,
    redactor: &'static Redactor,
}

impl<W: std::io::Write> Redacting<W> {
    /// `inner`, its lines redacted by `redactor`.
    pub fn new(inner: W, redactor: &'static Redactor) -> Self {
        Self { inner, redactor }
    }

    /// The output itself (what a test reads back).
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: std::io::Write> std::io::Write for Redacting<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // (the logger writes text; bytes that are no text are made text first, so that
        // nothing passes without the replacement)
        let text = String::from_utf8_lossy(buf);
        self.inner
            .write_all(self.redactor.redact(&text).as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
