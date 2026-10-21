//! What the content takes on disk, for the Setup page: the OMSI 2 folder and the content
//! folder, by type of content (Vehicles, Sceneryobjects, ...), by what is in each (a bus's
//! folder, a map's) and by kind of file - and how many maps, buses, objects, splines and
//! archives there are. Only read: nothing is written.

use std::path::{Path, PathBuf};

/// A part of the content and what it takes, with its parts, the largest first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Node {
    pub name: String,
    pub bytes: u64,
    pub kids: Vec<Node>,
}

/// How many of each there are, in both folders.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    /// Folders in `maps` with a `global.cfg`.
    pub maps: usize,
    /// `.bus` and `.ovh` files in `Vehicles`.
    pub vehicles: usize,
    /// `.sco` files in `Sceneryobjects`.
    pub sceneryobjects: usize,
    /// `.sli` files in `Splines`.
    pub splines: usize,
    /// `.zip` files in `Archives` (used in place).
    pub archives: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Usage {
    /// The content: its parts are the folders, theirs the types, then what is in each type's
    /// folder, then the kinds of file.
    pub tree: Node,
    pub counts: Counts,
}

/// The types of content, as OMSI 2's folders are named, and the rest.
pub const TYPES: [&str; 8] = [
    "Vehicles",
    "Sceneryobjects",
    "Texture",
    "Maps",
    "Sounds",
    "Splines",
    "Archives",
    "Other",
];

/// What is in each type's folder beyond this many parts is put together as one.
const MOST_PARTS: usize = 40;

/// A part of a type's folder while it is read: its name and its bytes by kind of file.
type Part = (String, Vec<(&'static str, u64)>);

fn type_of(top: &str) -> usize {
    TYPES[..7]
        .iter()
        .position(|t| t.eq_ignore_ascii_case(top))
        .unwrap_or(7)
}

/// The kind of a file, by its extension.
pub fn kind_of(file: &str) -> &'static str {
    let ext = file
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "dds" | "bmp" | "tga" | "jpg" | "jpeg" | "png" => "Textures",
        "o3d" | "x" => "Models",
        "wav" | "ogg" | "mp3" => "Sounds",
        "osc" => "Scripts",
        "zip" => "Archives",
        "cfg" | "bus" | "ovh" | "sco" | "sli" | "map" | "txt" | "ini" | "hof" | "ttp" | "ttl"
        | "ttr" | "oft" | "cti" | "owt" => "Settings and text",
        _ => "Other files",
    }
}

/// Reads `folders` ((name, path)) once each. A folder inside another one listed (a content
/// folder inside the OMSI 2 folder) is counted only as itself.
pub fn scan(folders: &[(String, PathBuf)]) -> Usage {
    let mut counts = Counts::default();
    let skip: Vec<PathBuf> = folders.iter().map(|(_, p)| p.clone()).collect();
    let mut tree = Node {
        name: "content".into(),
        ..Node::default()
    };
    for (name, dir) in folders {
        let node = folder(name, dir, &skip, &mut counts);
        if node.bytes > 0 {
            tree.bytes += node.bytes;
            tree.kids.push(node);
        }
    }
    Usage { tree, counts }
}

fn folder(name: &str, dir: &Path, skip: &[PathBuf], counts: &mut Counts) -> Node {
    // (type -> part -> kind -> bytes)
    let mut types: Vec<Vec<Part>> = vec![Vec::new(); TYPES.len()];
    for (top, path, is_dir) in entries(dir, skip) {
        let t = type_of(&top);
        if !is_dir {
            // (a file at the top: a part of its own, of the rest)
            let bytes = std::fs::symlink_metadata(&path)
                .map(|m| m.len())
                .unwrap_or(0);
            types[t].push((top.clone(), vec![(kind_of(&top), bytes)]));
            continue;
        }
        if t == 7 {
            let mut kinds = Vec::new();
            walk(&path, skip, &mut kinds, &mut |_| {});
            types[t].push((top, kinds));
            continue;
        }
        let mut loose = Vec::new();
        for (part, p, sub_dir) in entries(&path, skip) {
            if t == 3
                && sub_dir
                && entries(&p, skip)
                    .iter()
                    .any(|(n, _, d)| !d && n.eq_ignore_ascii_case("global.cfg"))
            {
                counts.maps += 1;
            }
            let mut count = |f: &str| {
                let f = f.to_ascii_lowercase();
                match t {
                    0 if f.ends_with(".bus") || f.ends_with(".ovh") => counts.vehicles += 1,
                    1 if f.ends_with(".sco") => counts.sceneryobjects += 1,
                    5 if f.ends_with(".sli") => counts.splines += 1,
                    6 if f.ends_with(".zip") => counts.archives += 1,
                    _ => {}
                }
            };
            if sub_dir {
                let mut kinds = Vec::new();
                walk(&p, skip, &mut kinds, &mut count);
                types[t].push((part, kinds));
            } else {
                count(&part);
                let bytes = std::fs::symlink_metadata(&p).map(|m| m.len()).unwrap_or(0);
                if t == 6 {
                    // (an archive is a part of its own: it is a mod)
                    types[t].push((part.clone(), vec![(kind_of(&part), bytes)]));
                } else {
                    add(&mut loose, kind_of(&part), bytes);
                }
            }
        }
        if !loose.is_empty() {
            types[t].push(("Loose files".into(), loose));
        }
    }
    let mut node = Node {
        name: name.into(),
        ..Node::default()
    };
    for (t, parts) in types.into_iter().enumerate() {
        let mut ty = Node {
            name: TYPES[t].into(),
            ..Node::default()
        };
        for (part, kinds) in parts {
            let mut p = Node {
                name: part,
                ..Node::default()
            };
            for (k, b) in kinds {
                p.bytes += b;
                p.kids.push(Node {
                    name: k.into(),
                    bytes: b,
                    kids: Vec::new(),
                });
            }
            if p.bytes > 0 {
                largest_first(&mut p.kids);
                ty.bytes += p.bytes;
                ty.kids.push(p);
            }
        }
        largest_first(&mut ty.kids);
        if ty.kids.len() > MOST_PARTS {
            let rest: Vec<Node> = ty.kids.split_off(MOST_PARTS - 1);
            let mut more = Node {
                name: format!("{} more", rest.len()),
                ..Node::default()
            };
            let mut kinds = Vec::new();
            for r in rest {
                more.bytes += r.bytes;
                for k in r.kids {
                    add(&mut kinds, leak_kind(&k.name), k.bytes);
                }
            }
            more.kids = kinds
                .into_iter()
                .map(|(k, b)| Node {
                    name: k.into(),
                    bytes: b,
                    kids: Vec::new(),
                })
                .collect();
            largest_first(&mut more.kids);
            ty.kids.push(more);
        }
        if ty.bytes > 0 {
            node.bytes += ty.bytes;
            node.kids.push(ty);
        }
    }
    largest_first(&mut node.kids);
    node
}

/// The `&'static` name of a kind `kind_of` gave.
fn leak_kind(name: &str) -> &'static str {
    [
        "Textures",
        "Models",
        "Sounds",
        "Scripts",
        "Archives",
        "Settings and text",
    ]
    .into_iter()
    .find(|k| *k == name)
    .unwrap_or("Other files")
}

fn largest_first(v: &mut [Node]) {
    v.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
}

fn add(kinds: &mut Vec<(&'static str, u64)>, kind: &'static str, bytes: u64) {
    match kinds.iter_mut().find(|(k, _)| *k == kind) {
        Some((_, b)) => *b += bytes,
        None => kinds.push((kind, bytes)),
    }
}

/// The entries of `dir` (name, path, whether a folder), links left out (a link could lead in
/// a circle or out of the folder), and the folders of `skip` other than `dir` itself.
fn entries(dir: &Path, skip: &[PathBuf]) -> Vec<(String, PathBuf, bool)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() && skip.iter().any(|s| s == &p) {
            continue;
        }
        out.push((e.file_name().to_string_lossy().to_string(), p, ft.is_dir()));
    }
    out
}

/// Every file under `dir`, added by kind; `count` sees each file's name.
fn walk(
    dir: &Path,
    skip: &[PathBuf],
    kinds: &mut Vec<(&'static str, u64)>,
    count: &mut dyn FnMut(&str),
) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for (name, p, is_dir) in entries(&d, skip) {
            if is_dir {
                stack.push(p);
            } else {
                count(&name);
                add(
                    kinds,
                    kind_of(&name),
                    std::fs::symlink_metadata(&p).map(|m| m.len()).unwrap_or(0),
                );
            }
        }
    }
}

/// The parts of the tree by type, both folders together, the largest first (the chart's
/// legend).
pub fn by_type(tree: &Node) -> Vec<(&'static str, u64)> {
    let mut out: Vec<(&'static str, u64)> = TYPES.iter().map(|t| (*t, 0)).collect();
    for f in &tree.kids {
        for t in &f.kids {
            if let Some(o) = out.iter_mut().find(|(n, _)| *n == t.name) {
                o.1 += t.bytes;
            }
        }
    }
    out.retain(|(_, b)| *b > 0);
    out.sort_by_key(|t| std::cmp::Reverse(t.1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(p: &Path, bytes: usize) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn the_content_by_folder_type_part_and_kind() {
        let base = std::env::temp_dir().join(format!("depot-disk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("OMSI 2");
        file(&root.join("Vehicles/MAN_SD200/SD200.bus"), 10);
        file(&root.join("Vehicles/MAN_SD200/Texture/a.dds"), 1000);
        file(&root.join("Vehicles/MAN_SD200/Model/m.o3d"), 500);
        file(&root.join("maps/Grundorf/global.cfg"), 20);
        file(&root.join("maps/Grundorf/tile_0_0.map"), 80);
        file(&root.join("maps/Empty/readme.txt"), 5);
        file(&root.join("Sceneryobjects/Trees/oak.sco"), 30);
        file(&root.join("Splines/Roads/str.sli"), 40);
        file(&root.join("Omsi.exe"), 300);
        // the content folder inside the OMSI 2 folder: counted once, as itself
        let content = root.join("depot");
        file(&content.join("Vehicles/MAN_SD200/SD200_repaint.ovh"), 7);
        file(&content.join("Archives/pack.zip"), 900);
        let u = scan(&[
            ("OMSI 2 folder".into(), root.clone()),
            ("Depot content folder".into(), content.clone()),
        ]);
        assert_eq!(
            u.counts,
            Counts {
                maps: 1,
                vehicles: 2,
                sceneryobjects: 1,
                splines: 1,
                archives: 1
            }
        );
        assert_eq!(
            u.tree.bytes,
            10 + 1000 + 500 + 20 + 80 + 5 + 30 + 40 + 300 + 7 + 900
        );
        let omsi = &u
            .tree
            .kids
            .iter()
            .find(|n| n.name == "OMSI 2 folder")
            .unwrap();
        assert_eq!(omsi.bytes, 1985);
        let vehicles = &omsi.kids[0];
        assert_eq!((vehicles.name.as_str(), vehicles.bytes), ("Vehicles", 1510));
        let bus = &vehicles.kids[0];
        assert_eq!(bus.name, "MAN_SD200");
        assert_eq!(
            bus.kids
                .iter()
                .map(|k| (k.name.as_str(), k.bytes))
                .collect::<Vec<_>>(),
            [
                ("Textures", 1000),
                ("Models", 500),
                ("Settings and text", 10)
            ]
        );
        let other = omsi.kids.iter().find(|n| n.name == "Other").unwrap();
        assert_eq!(other.kids[0].name, "Omsi.exe");
        let legend = by_type(&u.tree);
        assert_eq!(legend[0], ("Vehicles", 1517));
        assert!(legend.contains(&("Archives", 900)));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn many_parts_end_in_one() {
        let base = std::env::temp_dir().join(format!("depot-disk-many-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for k in 0..50 {
            file(&base.join(format!("Sceneryobjects/o{k:02}/x.sco")), 100 + k);
        }
        let u = scan(&[("OMSI 2 folder".into(), base.clone())]);
        let objects = &u.tree.kids[0].kids[0];
        assert_eq!(objects.kids.len(), MOST_PARTS);
        assert_eq!(objects.kids.last().unwrap().name, "11 more");
        assert_eq!(
            objects.kids.iter().map(|k| k.bytes).sum::<u64>(),
            objects.bytes
        );
        assert_eq!(u.counts.sceneryobjects, 50);
        let _ = std::fs::remove_dir_all(&base);
    }
}
