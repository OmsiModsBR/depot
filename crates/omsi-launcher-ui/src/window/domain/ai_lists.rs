//! A map's own AI lists while they are edited on the AI traffic tab: the lists as read and as
//! changed, the changes between them, and the windows over the tab.

use omsi_integrity::traffic::VehicleFile;
use omsi_map::ailists::DepotEntry;
use omsi_map::ailists_edit::{AiListsDoc, EditError, GroupKind, ParkListDoc};
use omsi_map::backup::Backup;
use omsi_map::ttl_edit::{TtlDoc, TtlEditError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

/// A parking list of the map and its lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Park {
    /// Its name in the map folder.
    pub file: String,
    /// 0 for `parklist_p.txt`, n for `parklist_p_<n>.txt`.
    pub index: usize,
    pub doc: ParkListDoc,
}

/// A timetable line of the map (`TTData/<name>.ttl`) and its tours' AI groups.
#[derive(Debug, Clone, PartialEq)]
pub struct Ttl {
    /// Its path in the map folder, with `/`.
    pub file: String,
    pub doc: TtlDoc,
}

/// The map's own lists, as edited.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lists {
    /// The name `ailists.cfg` has in the map folder (`ailists.cfg` when there is none yet).
    pub ailists_file: String,
    pub ailists: AiListsDoc,
    pub parks: Vec<Park>,
    /// The map's timetable lines.
    pub ttls: Vec<Ttl>,
}

impl Lists {
    /// The lists in the map folder `map_dir` on the disk.
    pub fn load(map_dir: &Path) -> Lists {
        let mut l = Lists {
            ailists_file: "ailists.cfg".into(),
            ..Default::default()
        };
        let mut names: Vec<String> = std::fs::read_dir(map_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort_by_key(|n| n.to_ascii_lowercase());
        for n in names {
            let lower = n.to_ascii_lowercase();
            let read = || std::fs::read(map_dir.join(&n)).unwrap_or_default();
            if lower == "ailists.cfg" {
                l.ailists = AiListsDoc::from_bytes(&read());
                l.ailists_file = n.clone();
                continue;
            }
            let index = if lower == "parklist_p.txt" {
                Some(0)
            } else {
                lower
                    .strip_prefix("parklist_p_")
                    .and_then(|x| x.strip_suffix(".txt"))
                    .and_then(|x| x.parse().ok())
            };
            if let Some(index) = index {
                l.parks.push(Park {
                    file: n.clone(),
                    index,
                    doc: ParkListDoc::from_bytes(&read()),
                });
            }
        }
        l.parks.sort_by_key(|p| p.index);
        // the lines: TTData's `.ttl` files, whatever case the folder has
        let tt = std::fs::read_dir(map_dir)
            .into_iter()
            .flatten()
            .flatten()
            .find(|e| {
                e.path().is_dir()
                    && e.file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case("ttdata")
            });
        if let Some(tt) = tt {
            let folder = tt.file_name().to_string_lossy().into_owned();
            let mut files: Vec<String> = std::fs::read_dir(tt.path())
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.to_ascii_lowercase().ends_with(".ttl"))
                .collect();
            files.sort_by_key(|n| n.to_ascii_lowercase());
            for n in files {
                let bytes = std::fs::read(tt.path().join(&n)).unwrap_or_default();
                l.ttls.push(Ttl {
                    file: format!("{folder}/{n}"),
                    doc: TtlDoc::from_bytes(&bytes),
                });
            }
        }
        l
    }

    /// The files that differ from `saved`, with what to write.
    /// (A file that does not read back byte for byte is never written: see `cannot_edit`.)
    pub fn to_write(&self, saved: &Lists) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        if self.ailists != saved.ailists && self.ailists.exact() {
            out.push((self.ailists_file.clone(), self.ailists.to_bytes()));
        }
        for p in &self.parks {
            let before = saved.parks.iter().find(|s| s.file == p.file);
            if before.is_none_or(|b| b.doc != p.doc) && p.doc.exact() {
                out.push((p.file.clone(), p.doc.to_bytes()));
            }
        }
        for t in &self.ttls {
            let before = saved.ttls.iter().find(|s| s.file == t.file);
            if before.is_none_or(|b| b.doc != t.doc) && t.doc.exact() {
                out.push((t.file.clone(), t.doc.to_bytes()));
            }
        }
        out
    }
}

/// A change from the lists as saved to the lists now.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Weight {
        group: String,
        file: String,
        from: f32,
        to: f32,
    },
    Added {
        group: String,
        file: String,
        weight: Option<f32>,
    },
    Removed {
        group: String,
        file: String,
    },
    GroupAdded(String),
    GroupRemoved(String),
    Parked {
        list: String,
        file: String,
        from: usize,
        to: usize,
    },
    /// A bus of a depot's fleet: `what` is its number and plate.
    BusAdded {
        group: String,
        what: String,
    },
    BusRemoved {
        group: String,
        what: String,
    },
    BusChanged {
        group: String,
        what: String,
    },
    /// A tour of a timetable line now takes its buses from another group.
    TourGroup {
        line: String,
        tour: String,
        from: String,
        to: String,
    },
}

/// The vehicles of a group by file (any case), each with its weights in order.
pub(crate) fn by_file(doc: &AiListsDoc, g: usize) -> Vec<(String, Vec<f32>)> {
    let mut out: Vec<(String, Vec<f32>)> = Vec::new();
    for v in &doc.groups()[g].vehicles {
        match out
            .iter_mut()
            .find(|(f, _)| f.eq_ignore_ascii_case(&v.file))
        {
            Some(e) => e.1.push(v.weight),
            None => out.push((v.file.clone(), vec![v.weight])),
        }
    }
    out
}

/// What changed from `saved` to `now`, group by group and list by list.
pub fn changes(saved: &Lists, now: &Lists) -> Vec<Change> {
    let mut out = Vec::new();
    let (a, b) = (&saved.ailists, &now.ailists);
    for (gi, g) in b.groups().iter().enumerate() {
        let name = g.name.trim().to_string();
        let weights = g.kind.has_weights();
        let new = by_file(b, gi);
        let old = a.find(&name).map(|i| by_file(a, i));
        if old.is_none() {
            out.push(Change::GroupAdded(name.clone()));
        }
        let old = old.unwrap_or_default();
        for (file, ws) in &new {
            let was = old
                .iter()
                .find(|(f, _)| f.eq_ignore_ascii_case(file))
                .map(|(_, w)| w.as_slice())
                .unwrap_or(&[]);
            for (i, w) in ws.iter().enumerate() {
                match was.get(i) {
                    Some(o) if o != w => out.push(Change::Weight {
                        group: name.clone(),
                        file: file.clone(),
                        from: *o,
                        to: *w,
                    }),
                    Some(_) => {}
                    None => out.push(Change::Added {
                        group: name.clone(),
                        file: file.clone(),
                        weight: weights.then_some(*w),
                    }),
                }
            }
            for _ in ws.len()..was.len() {
                out.push(Change::Removed {
                    group: name.clone(),
                    file: file.clone(),
                });
            }
        }
        for (file, was) in &old {
            if !new.iter().any(|(f, _)| f.eq_ignore_ascii_case(file)) {
                for _ in was {
                    out.push(Change::Removed {
                        group: name.clone(),
                        file: file.clone(),
                    });
                }
            }
        }
    }
    for g in a.groups() {
        if b.find(&g.name).is_none() {
            out.push(Change::GroupRemoved(g.name.trim().to_string()));
        }
    }
    bus_changes(saved, now, &mut out);
    for p in &now.parks {
        let before = saved
            .parks
            .iter()
            .find(|s| s.file == p.file)
            .map(|s| s.doc.cars())
            .unwrap_or_default();
        let after = p.doc.cars();
        let count = |list: &[(String, usize)], f: &str| {
            list.iter()
                .find(|(x, _)| x.eq_ignore_ascii_case(f))
                .map(|(_, n)| *n)
                .unwrap_or(0)
        };
        let mut files: Vec<&String> = after.iter().map(|(f, _)| f).collect();
        for (f, _) in &before {
            if !files.iter().any(|x| x.eq_ignore_ascii_case(f)) {
                files.push(f);
            }
        }
        for f in files {
            let (from, to) = (count(&before, f), count(&after, f));
            if from != to {
                out.push(Change::Parked {
                    list: p.file.clone(),
                    file: f.clone(),
                    from,
                    to,
                });
            }
        }
    }
    out
}

/// The buses of every depot group, each with the vehicle file of its list.
pub(crate) fn fleet_of(doc: &AiListsDoc, g: usize) -> Vec<(String, DepotEntry)> {
    doc.groups()[g]
        .fleets
        .iter()
        .flat_map(|f| f.buses.iter().map(|b| (f.file.clone(), b.entry.clone())))
        .collect()
}

/// A bus as the lists name it: its number and plate.
pub(crate) fn bus_name(e: &DepotEntry) -> String {
    if e.registration.trim().is_empty() {
        e.number.clone()
    } else {
        format!("{} · {}", e.number, e.registration)
    }
}

/// The buses added, taken out or changed in the depots' fleet lists, and the tours that
/// changed their group.
pub(crate) fn bus_changes(saved: &Lists, now: &Lists, out: &mut Vec<Change>) {
    let (a, b) = (&saved.ailists, &now.ailists);
    for (gi, g) in b.groups().iter().enumerate() {
        let Some(oi) = a.find(&g.name) else { continue };
        let name = g.name.trim().to_string();
        let mut old = fleet_of(a, oi);
        let mut added = Vec::new();
        for (file, e) in fleet_of(b, gi) {
            match old
                .iter()
                .position(|(f, o)| f.eq_ignore_ascii_case(&file) && *o == e)
            {
                Some(i) => {
                    old.remove(i);
                }
                None => added.push((file, e)),
            }
        }
        for (file, e) in added {
            // (the same number in the same list: one bus changed, not two)
            let same = old.iter().position(|(f, o)| {
                f.eq_ignore_ascii_case(&file) && o.number.trim() == e.number.trim()
            });
            match same {
                Some(i) => {
                    old.remove(i);
                    out.push(Change::BusChanged {
                        group: name.clone(),
                        what: bus_name(&e),
                    });
                }
                None => out.push(Change::BusAdded {
                    group: name.clone(),
                    what: bus_name(&e),
                }),
            }
        }
        for (_, e) in old {
            out.push(Change::BusRemoved {
                group: name.clone(),
                what: bus_name(&e),
            });
        }
    }
    for t in &now.ttls {
        let Some(before) = saved.ttls.iter().find(|s| s.file == t.file) else {
            continue;
        };
        let line = std::path::Path::new(&t.file)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        for (n, o) in t.doc.tours().iter().zip(before.doc.tours()) {
            if n.ai_group != o.ai_group {
                out.push(Change::TourGroup {
                    line: line.clone(),
                    tour: n.number.trim().to_string(),
                    from: o.ai_group.trim().to_string(),
                    to: n.ai_group.trim().to_string(),
                });
            }
        }
    }
}

/// A window over the tab.
pub enum ListsDialog {
    /// Pick a vehicle to add to group `group`.
    AddVehicle {
        group: usize,
        search: String,
        /// 0 cars and trucks, 1 buses, 2 all.
        kind: usize,
        sel: Option<usize>,
        weight: f32,
    },
    /// The group to move the chosen vehicle to.
    MoveTo {
        sel: Option<usize>,
    },
    NewGroup {
        name: String,
        error: bool,
    },
    /// A bus of a depot's fleet, new (`bus` none) or changed: group, fleet list, the fields.
    Bus(BusForm),
    /// The group a tour takes its buses from: the line's file, the tour's place in it.
    TourGroup {
        file: String,
        tour: usize,
        sel: Option<usize>,
    },
    Save,
    Backups {
        list: Vec<Backup>,
        sel: usize,
    },
    /// Leave the editing with changes not saved.
    Leave,
}

/// The editing of one map's lists.
pub struct Editor {
    /// The map's folder (as the AI traffic tab names it) and its name.
    pub folder: String,
    pub name: String,
    pub map_dir: PathBuf,
    pub now: Lists,
    pub saved: Lists,
    pub undo: Vec<Lists>,
    pub redo: Vec<Lists>,
    pub group: usize,
    pub vehicle: Option<usize>,
    pub park: usize,
    pub car: Option<usize>,
    /// The line and tour looked at on the bus tab, and the bus (fleet list, bus) of the group.
    pub line: usize,
    pub tour: usize,
    pub bus: Option<(usize, usize)>,
    pub dialog: Option<ListsDialog>,
    /// The vehicles there are to add, once read.
    pub vehicles: Option<Vec<VehicleFile>>,
    pub reading: Option<Receiver<Vec<VehicleFile>>>,
    /// Whether a file named by the lists is there (by its name in lower case).
    pub exists: HashMap<String, bool>,
    /// The vehicles' own names, by file in lower case: from the lists as read, and from the vehicles there
    /// are to add once read.
    pub names: HashMap<String, String>,
}

impl Editor {
    pub fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn dirty(&self) -> bool {
        self.now != self.saved
    }

    /// Change the lists with `f`, kept for Undo when something changed.
    pub fn act(
        &mut self,
        f: impl FnOnce(&mut Lists) -> Result<(), EditError>,
    ) -> Result<(), EditError> {
        let before = self.now.clone();
        f(&mut self.now)?;
        if self.now != before {
            self.undo.push(before);
            self.redo.clear();
        }
        Ok(())
    }

    /// Like [`act`](Self::act) for a timetable line.
    pub fn act_ttl(
        &mut self,
        f: impl FnOnce(&mut Lists) -> Result<(), TtlEditError>,
    ) -> Result<(), TtlEditError> {
        let before = self.now.clone();
        f(&mut self.now)?;
        if self.now != before {
            self.undo.push(before);
            self.redo.clear();
        }
        Ok(())
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.now, prev));
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.now, next));
        }
    }

    pub(crate) fn exists(&mut self, root: &Path, file: &str) -> bool {
        *self
            .exists
            .entry(file.to_ascii_lowercase())
            .or_insert_with(|| {
                omsi_cfg::vfs::is_file(&omsi_cfg::resolve_path(root, &file.replace('\\', "/")))
            })
    }
}

/// The editor's counts for the car groups and parking lists tabs.
pub fn counts(e: &Editor) -> (usize, usize) {
    (
        e.now
            .ailists
            .groups()
            .iter()
            .filter(|g| g.kind != GroupKind::Depot)
            .count(),
        e.now.parks.len(),
    )
}

/// The fields of a bus of a fleet list while they are typed.
pub struct BusForm {
    pub(crate) group: usize,
    pub(crate) fleet: usize,
    /// The bus changed (its place in the list); none for a new one.
    pub(crate) bus: Option<usize>,
    pub(crate) number: String,
    pub(crate) plate: String,
    pub(crate) paint: String,
    pub(crate) from: String,
    pub(crate) until: String,
    pub(crate) error: Option<&'static str>,
}
