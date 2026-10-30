//! The settled pictures kept on disk, by bus, paint and shape, and the small pictures drawn
//! ahead of time for the Vehicles page.

use super::super::domain::{look::Look, shape::Shape};
use std::path::PathBuf;

fn fnv(parts: &[&str]) -> u64 {
    // (FNV-1a: the same number from one run to the next)
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in parts.join("\0").bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Which bus and paint a picture shows (not the light: the same bus at another hour is the
/// same bus).
pub fn tag(look: &Look) -> u64 {
    fnv(&[&look.root.to_string_lossy(), &look.bus, &look.paint])
}

/// The file the settled picture of `look`'s bus and paint, for a preview of `shape`, is kept in.
/// Its name changes with everything the picture depends on: the bus file and its folder (a bus
/// changed or a paint added is drawn anew, never shown as it was), the build (which draws
/// it), and the shape it was drawn for (a page's preview is wider than another's, and frames
/// the bus elsewhere: a picture kept for one page was shown stretched on the other).
pub fn cache_file(look: &Look, shape: Shape) -> PathBuf {
    let bus = look.root.join(&look.bus);
    cache_file_with(
        look,
        shape,
        &bus.parent().map(fingerprint).unwrap_or_default(),
    )
}

/// `cache_file` with the fingerprint of the bus's folder already read.
pub fn cache_file_with(look: &Look, shape: Shape, fingerprint: &str) -> PathBuf {
    let small = shape == thumb_shape();
    // (the small pictures drawn ahead of time are not drawn again with every build: they
    // change with the bus and with `THUMB_REVISION`, bumped when the showroom's drawing does)
    let build = if small {
        THUMB_REVISION
    } else {
        crate::core::startup::BUILD
    };
    let h = fnv(&[
        &look.root.to_string_lossy(),
        &look.bus,
        &look.paint,
        fingerprint,
        build,
        &format!("{}:{}", shape.0, shape.1),
    ]);
    let dir = omsi_launcher_lib::data_dir().join("previews");
    let dir = if small { dir.join("small") } else { dir };
    dir.join(format!("{h:016x}.png"))
}

/// The shape of the small pictures drawn ahead of time for the Vehicles page's paint tiles.
pub fn thumb_shape() -> Shape {
    Shape::of(THUMB_SIZE.0, THUMB_SIZE.1, 0.5)
}

/// The size they are drawn at.
pub const THUMB_SIZE: (u32, u32) = (336, 208);

/// Changes when the showroom draws a bus differently (the small pictures are drawn again).
const THUMB_REVISION: &str = "thumb-2";

/// How many small pictures are kept at most, in a folder of their own (the oldest go), and
/// how many one run draws, so that it never prunes its own pictures.
pub const KEEP_SMALL: usize = 6000;
pub const RUN_MAX: usize = 4000;

/// The files under a bus's folder (its .bus, model, textures, paints, scripts), as their count,
/// their total size and the newest change: any of them edited, added or taken away changes it.
/// (Read off the launcher's thread: a bus folder can hold hundreds of files.)
fn fingerprint(dir: &std::path::Path) -> String {
    fn walk(dir: &std::path::Path, depth: u32, acc: &mut (u64, u64, u64)) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                if depth < 4 {
                    walk(&e.path(), depth + 1, acc);
                }
                continue;
            }
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            acc.0 += 1;
            acc.1 += m.len();
            acc.2 = acc.2.max(t);
        }
    }
    let mut acc = (0, 0, 0);
    walk(dir, 0, &mut acc);
    format!("{}:{}:{}", acc.0, acc.1, acc.2)
}

/// Which of `looks` (paints of one bus) have no kept picture at `shape` yet. Reads the bus's
/// folder once, off the launcher's thread.
pub fn missing_kept(looks: Vec<Look>, shape: Shape) -> Vec<Look> {
    let Some(first) = looks.first() else {
        return looks;
    };
    let fp = first
        .root
        .join(&first.bus)
        .parent()
        .map(fingerprint)
        .unwrap_or_default();
    looks
        .into_iter()
        .filter(|l| !cache_file_with(l, shape, &fp).exists())
        .collect()
}

/// The picture kept for `look`'s bus and paint at `shape`, if there is one (read off the
/// launcher's thread: it reads a file and walks the bus's folder).
pub fn kept_picture(look: &Look, shape: Shape) -> Option<image::RgbaImage> {
    image::open(cache_file(look, shape))
        .ok()
        .map(|i| i.to_rgba8())
}

/// The small picture drawn ahead of time of `bus` (`Vehicles/<folder>/<file>`) in `paint`
/// (empty: the bus's own) under the installation `root`, if the launcher has drawn it: the
/// in-game "Add vehicle" window shows it. Read off the game's thread.
pub fn kept_thumb(root: &std::path::Path, bus: &str, paint: &str) -> Option<image::RgbaImage> {
    let look = Look {
        root: root.to_path_buf(),
        bus: bus.to_string(),
        paint: paint.to_string(),
        ..Default::default()
    };
    kept_picture(&look, thumb_shape())
}

/// What names the kept picture of `look`'s bus and paint at `shape` for the launcher's own
/// small copies of it (the same on every run; not the file's name, which needs the disk).
pub fn kept_key(look: &Look, shape: Shape) -> String {
    kept_key_of(tag(look), shape)
}

/// `kept_key` from the tag the helper reports when it has kept a picture.
pub fn kept_key_of(tag: u64, shape: Shape) -> String {
    format!("paint:{tag:016x}:{}:{}", shape.0, shape.1)
}

/// The widest picture kept (a bigger preview is kept smaller: the live one replaces it).
pub(super) const KEEP_W: u32 = 1280;

/// How many kept pictures there are at most (the oldest go).
pub(super) const KEEP: usize = 300;

/// Only the `keep` newest pictures in `dir`.
pub fn prune(dir: &std::path::Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .map(|e| {
            (
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH),
                e.path(),
            )
        })
        .collect();
    if files.len() <= keep {
        return;
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, f) in files.drain(keep..) {
        let _ = std::fs::remove_file(f);
    }
}
