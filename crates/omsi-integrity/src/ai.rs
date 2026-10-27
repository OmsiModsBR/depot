//! The map's AI lists: the vehicles of `ailists.cfg` (and its chrono updates), the random
//! traffic groups of `unsched_vehgroups.txt`, the parked cars of the `parklist_p*.txt`, the
//! people of `humans.txt` and `drivers.txt`, and the AI groups the timetable's tours name.

use crate::map::Checker;
use crate::report::Severity;
use omsi_cfg::{resolve_path, vfs, CfgFile};
use omsi_map::ailists::{load_list, parse_unsched_vehgroups, AiLists};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

/// Above this an `ailists.cfg` is not recommended, except on high-performance computers (an
/// observation of the Depot maintainers, not a limit of the game).
pub const AILISTS_RECOMMENDED_MAX: u64 = 16 * 1024;

impl Checker<'_> {
    /// Everything the map's AI lists name (`chrono_dirs`: every chrono folder, active or not).
    pub(crate) fn ai_lists(&mut self, map_dir: &Path, chrono_dirs: &[PathBuf]) {
        // the lists of the map, then of each scenario that has one (its update, else a full
        // list of its own, as the game reads them)
        let mut lists: Vec<(PathBuf, AiLists)> = Vec::new();
        let own = resolve_path(map_dir, "ailists.cfg");
        if vfs::is_file(&own) {
            if let Some(l) = self.ai_list_file(&own) {
                lists.push((own, l));
            }
        }
        for c in chrono_dirs {
            let file = ["ailists_#upd.cfg", "ailists.cfg"]
                .iter()
                .map(|n| resolve_path(c, n))
                .find(|p| vfs::is_file(p));
            if let Some(p) = file {
                if let Some(l) = self.ai_list_file(&p) {
                    lists.push((p, l));
                }
            }
        }
        // (group names are compared as the game does, without case: it lower-cases the file)
        let groups: HashSet<String> = lists
            .iter()
            .flat_map(|(_, l)| l.groups.iter().map(|g| g.name.trim().to_ascii_lowercase()))
            .collect();
        for (path, l) in &lists {
            if self.stopped() {
                return;
            }
            self.ai_vehicles(path, l, lists.first().map(|(_, m)| m));
        }
        self.random_groups(map_dir, &groups, !lists.is_empty());
        self.parked_cars(map_dir);
        self.people(map_dir, "humans.txt", "human-missing");
        self.people(map_dir, "drivers.txt", "driver-missing");
        self.tours(map_dir, chrono_dirs, &groups, !lists.is_empty());
    }

    /// Reads one AI list, with what its size costs.
    fn ai_list_file(&mut self, path: &Path) -> Option<AiLists> {
        let file = self.rel(path);
        if let Some(len) = vfs::read(path).ok().map(|b| b.len() as u64) {
            if len > AILISTS_RECOMMENDED_MAX {
                self.add(
                    Severity::NotOptimized,
                    "ai-list-large",
                    file.clone(),
                    "",
                    format!(
                        "The AI list is {:.1} KB; more than 16 KB is not recommended except on \
                         high-performance computers.",
                        len as f64 / 1024.0
                    ),
                    Some(
                        "Every vehicle an AI list names can be loaded while the map runs, so a \
                         long list means more models and textures in memory and more stutter \
                         when new traffic appears. Above 16 KB this is recommended only on \
                         high-performance computers (an observation of the Depot maintainers)."
                            .into(),
                    ),
                );
            }
        }
        match CfgFile::read(path) {
            Ok(f) => Some(AiLists::parse(&f)),
            Err(e) => {
                self.add(
                    Severity::Error,
                    "ai-list-unreadable",
                    file,
                    "",
                    format!("The AI list cannot be read: {e}"),
                    None,
                );
                None
            }
        }
    }

    /// The vehicles of one AI list's groups and depots; `map` is the map's own list, whose
    /// depot file a scenario's update keeps when it names none.
    fn ai_vehicles(&mut self, path: &Path, l: &AiLists, map: Option<&AiLists>) {
        let by = self.rel(path);
        for g in &l.groups {
            for v in &g.vehicles {
                self.ai_vehicle(&v.file, &by);
            }
            let hof = g.hof.clone().or_else(|| {
                map?.groups
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case(&g.name) && m.is_depot == g.is_depot)?
                    .hof
                    .clone()
            });
            for t in &g.typgroups {
                let Some(found) = self.ai_vehicle(&t.file, &by) else {
                    continue;
                };
                let Some(hof) = hof.as_deref().filter(|h| !h.trim().is_empty()) else {
                    continue;
                };
                let Some(dir) = found.parent() else { continue };
                if omsi_vehicle::hof::depot_in(dir, hof).is_none() {
                    self.add(
                        Severity::Warning,
                        "depot-missing",
                        format!("{}/{}.hof", self.rel(dir), hof.trim()),
                        &by,
                        format!(
                            "The depot \"{}\" the AI depot \"{}\" names is not in this vehicle's \
                             folder; these buses have no depot file to take their lines and \
                             destinations from.",
                            hof.trim(),
                            g.name.trim()
                        ),
                        None,
                    );
                }
            }
        }
    }

    /// One vehicle of an AI list, where it is when it is there.
    fn ai_vehicle(&mut self, rel: &str, by: &str) -> Option<PathBuf> {
        let rel = rel.trim();
        if rel.is_empty() {
            return None;
        }
        let path = resolve_path(self.root, rel);
        if vfs::is_file(&path) {
            return Some(path);
        }
        self.add(
            Severity::Error,
            "ai-vehicle-missing",
            rel.replace('\\', "/"),
            by,
            "A vehicle an AI list names is missing; it cannot drive in the traffic.".into(),
            None,
        );
        None
    }

    /// The groups the random traffic is made of.
    fn random_groups(&mut self, map_dir: &Path, groups: &HashSet<String>, have_lists: bool) {
        let path = resolve_path(map_dir, "unsched_vehgroups.txt");
        if !vfs::is_file(&path) {
            return;
        }
        let Ok(f) = CfgFile::read(&path) else { return };
        let by = self.rel(&path);
        for (name, _) in parse_unsched_vehgroups(&f) {
            let key = name.trim().to_ascii_lowercase();
            if key.is_empty() || groups.contains(&key) {
                continue;
            }
            self.add(
                Severity::Warning,
                "ai-group-unknown",
                format!("{by}: {}", name.trim()),
                &by,
                if have_lists {
                    format!(
                        "The random traffic group \"{}\" is in no AI list; no vehicles drive for it.",
                        name.trim()
                    )
                } else {
                    format!(
                        "The random traffic group \"{}\" has no AI list to take its vehicles from \
                         (the map has no ailists.cfg).",
                        name.trim()
                    )
                },
                None,
            );
        }
    }

    /// The parked cars of every parking list (`parklist_p.txt`, `parklist_p_<n>.txt`): the
    /// ones that are there are checked as any object of the map.
    fn parked_cars(&mut self, map_dir: &Path) {
        let mut files: Vec<PathBuf> = vfs::list_dir(map_dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|(n, is_dir)| {
                let n = n.to_string_lossy().to_ascii_lowercase();
                !is_dir
                    && n.ends_with(".txt")
                    && (n == "parklist_p.txt"
                        || n.strip_prefix("parklist_p_")
                            .and_then(|r| r.strip_suffix(".txt"))
                            .is_some_and(|i| {
                                !i.is_empty() && i.bytes().all(|b| b.is_ascii_digit())
                            }))
            })
            .map(|(n, _)| map_dir.join(n))
            .collect();
        files.sort();
        for path in files {
            let by = self.rel(&path);
            let text = vfs::read(&path)
                .map(|b| omsi_cfg::decode_text(&b))
                .unwrap_or_default();
            // (the game takes the lines that name a scenery object, as the runtime does)
            let cars: BTreeSet<String> = text
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| l.to_ascii_lowercase().ends_with(".sco"))
                .collect();
            for car in cars {
                if self.stopped() {
                    return;
                }
                if vfs::is_file(&resolve_path(self.root, &car)) {
                    self.object(&car, &by);
                } else {
                    self.add(
                        Severity::Warning,
                        "parked-car-missing",
                        car.replace('\\', "/"),
                        &by,
                        "A parked car a parking list names is missing; it is never parked.".into(),
                        None,
                    );
                }
            }
        }
    }

    /// The people (`humans.txt`) or drivers (`drivers.txt`) the map draws from.
    fn people(&mut self, map_dir: &Path, list: &str, code: &'static str) {
        let path = resolve_path(map_dir, list);
        if !vfs::is_file(&path) {
            return;
        }
        let by = self.rel(&path);
        let mut seen = HashSet::new();
        for line in load_list(&path) {
            if !seen.insert(line.to_ascii_lowercase()) {
                continue;
            }
            if vfs::is_file(&resolve_path(self.root, &line)) {
                continue;
            }
            let message = if code == "driver-missing" {
                "A driver figure drivers.txt names is missing; the buses' drivers are taken \
                 from the others."
            } else {
                "A person humans.txt names is missing; the map's people are taken from the \
                 others."
            };
            self.add(
                Severity::Warning,
                code,
                line.replace('\\', "/"),
                &by,
                message.into(),
                None,
            );
        }
    }

    /// The AI groups the tours of the timetable's lines take their buses from.
    fn tours(
        &mut self,
        map_dir: &Path,
        chrono_dirs: &[PathBuf],
        groups: &HashSet<String>,
        have_lists: bool,
    ) {
        let mut lines = omsi_timetable::TimetableData::load(map_dir).lines;
        for c in chrono_dirs {
            lines.extend(omsi_timetable::TimetableData::load(c).lines);
        }
        for line in lines {
            let unknown: BTreeSet<String> = line
                .tours
                .iter()
                .map(|t| t.ai_group.trim().to_string())
                .filter(|g| !g.is_empty() && !groups.contains(&g.to_ascii_lowercase()))
                .collect();
            if unknown.is_empty() {
                continue;
            }
            let names = unknown
                .iter()
                .map(|g| format!("\"{g}\""))
                .collect::<Vec<_>>()
                .join(", ");
            let file = self.rel(&line.path);
            self.add(
                Severity::Warning,
                "tour-group-unknown",
                file,
                "",
                if have_lists {
                    format!(
                        "Tours of this line name the AI group {names}, which no AI list has; \
                         no AI bus can be taken for them."
                    )
                } else {
                    format!(
                        "Tours of this line name the AI group {names}, but the map has no \
                         ailists.cfg; no AI bus can be taken for them."
                    )
                },
                None,
            );
        }
    }
}
