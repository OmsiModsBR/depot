//! A map's AI traffic as the game takes it on one day, to look at: the car groups and their
//! weights, the depots and their fleets, the timetable's lines with the AI group of each
//! tour, the parking lists, the people and drivers, and the random traffic's density over
//! the day. It only reads: nothing here changes a file.

use omsi_cfg::{resolve_path, vfs, CfgFile};
use omsi_map::ailists::{
    active_chrono_dirs, chrono_deactivated_lines, load_list, parse_unsched_trafficdens,
    parse_unsched_vehgroups, typgroup_entry_valid, AiLists, DepotEntry, UnschedGroup,
};
use omsi_timetable::{CarUse, TimetableData};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A vehicle of a car group.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GroupVehicle {
    /// As the list names it (from the OMSI 2 folder).
    pub file: String,
    /// Its `[friendlyname]` (manufacturer and model), empty when it has none or is missing.
    pub name: String,
    pub weight: f32,
    pub exists: bool,
    /// The AI list that names it (from the OMSI 2 folder).
    pub listed_in: String,
}

/// One vehicle type of a depot and its fleet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FleetType {
    pub file: String,
    pub name: String,
    pub exists: bool,
    pub listed_in: String,
    /// The fleet, each with whether it is in service on the day.
    pub entries: Vec<(DepotEntry, bool)>,
}

/// A group of `ailists.cfg`: plain cars, or a depot with its fleet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrafficGroup {
    pub name: String,
    pub is_depot: bool,
    /// A depot's file (`.hof` name).
    pub hof: Option<String>,
    pub vehicles: Vec<GroupVehicle>,
    pub fleet: Vec<FleetType>,
    /// Its density class in `unsched_vehgroups.txt`, when the random traffic uses it.
    pub random: Option<i32>,
}

impl TrafficGroup {
    /// The sum of the weights, which each weight is a share of.
    pub fn total_weight(&self) -> f32 {
        self.vehicles.iter().map(|v| v.weight.max(0.0)).sum()
    }

    /// How likely vehicle `i` is picked when the group gives one (0 to 1).
    pub fn chance(&self, i: usize) -> f32 {
        let total = self.total_weight();
        match self.vehicles.get(i) {
            Some(v) if total > 0.0 => v.weight.max(0.0) / total,
            _ => 0.0,
        }
    }

    /// The fleet in service on the day.
    pub fn in_service(&self) -> usize {
        self.fleet
            .iter()
            .flat_map(|t| &t.entries)
            .filter(|(_, on)| *on)
            .count()
    }
}

/// A tour of a timetable line.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrafficTour {
    pub number: String,
    pub ai_group: String,
    /// Whether an AI list has that group.
    pub group_known: bool,
    pub trips: usize,
    /// First and last departure, minutes after midnight.
    pub first: Option<f32>,
    pub last: Option<f32>,
}

/// A line of the timetable, as in force on the day.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrafficLine {
    pub name: String,
    pub file: String,
    pub priority: i32,
    pub user_allowed: bool,
    pub tours: Vec<TrafficTour>,
    /// The bus types its `car_use` gives (`[onlytypes]`: 1.0, `[types_prefered]`: its
    /// probability), when one is in force.
    pub types: Option<(f32, Vec<String>)>,
}

/// A parking list (`parklist_p.txt`, `parklist_p_<n>.txt`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParkList {
    pub file: String,
    /// 0 for `parklist_p.txt`, n for `parklist_p_<n>.txt` (what a parking space names).
    pub index: usize,
    /// Each car once, with how often the list names it and whether it is there.
    pub cars: Vec<Listed>,
}

impl ParkList {
    /// The list's lines that name a car (a car listed twice counted twice).
    pub fn total(&self) -> usize {
        self.cars.iter().map(|c| c.count).sum()
    }
}

/// An entry of a list file, with how often it is named and whether it is there.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Listed {
    pub file: String,
    pub count: usize,
    pub exists: bool,
}

/// The density of a random traffic group over the day (`unsched_trafficdens.txt`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Density {
    pub name: String,
    pub factor: f32,
    /// (day mask: 1 Monday to Friday, 2 Saturday, 4 Sunday, 0 every day; (hour, value)).
    pub curves: Vec<(i32, Vec<(f32, f32)>)>,
}

impl Density {
    /// The curve of a day: 0 a weekday, 1 Saturday, 2 Sunday or holiday (the first that
    /// says so, as the game picks it).
    pub fn curve(&self, day: usize) -> Option<&[(f32, f32)]> {
        let bit = 1 << day.min(2);
        self.curves
            .iter()
            .find(|(mask, _)| *mask == 0 || mask & bit != 0)
            .map(|(_, c)| c.as_slice())
    }

    /// The group's density at `hour` of a day: its factor times its curve (0 without one).
    pub fn at(&self, day: usize, hour: f32) -> f32 {
        match self.curve(day) {
            Some(c) => self.factor * omsi_map::global::curve_at(c, hour).max(0.0),
            None => 0.0,
        }
    }
}

/// The map's AI traffic on one day.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AiTraffic {
    /// The day, YYYYMMDD.
    pub date: i32,
    /// The chrono folders in force that day (from the map's folder), in the game's order.
    pub chrono: Vec<String>,
    /// The AI lists read, the map's first (from the OMSI 2 folder).
    pub lists: Vec<String>,
    /// The map's own `ailists.cfg` size in bytes (0: none).
    pub ailists_size: u64,
    pub groups: Vec<TrafficGroup>,
    /// The group the random traffic takes vehicles from without `unsched_vehgroups.txt`.
    pub default_group: usize,
    /// Whether the map has `unsched_vehgroups.txt`.
    pub has_random_groups: bool,
    pub lines: Vec<TrafficLine>,
    pub parklists: Vec<ParkList>,
    pub humans: Vec<Listed>,
    pub drivers: Vec<Listed>,
    pub density: Vec<Density>,
}

/// A vehicle's `[friendlyname]` (manufacturer and model), read once per file.
fn friendly(path: &Path, cache: &mut HashMap<PathBuf, String>) -> String {
    if let Some(n) = cache.get(path) {
        return n.clone();
    }
    let mut name = String::new();
    if let Ok(f) = CfgFile::read(path) {
        let mut r = f.reader();
        while let Some(k) = r.next_keyword() {
            if k == "friendlyname" {
                let maker = r.str().trim().to_string();
                let model = r.str().trim().to_string();
                name = format!("{maker} {model}").trim().to_string();
                break;
            }
        }
    }
    cache.insert(path.to_path_buf(), name.clone());
    name
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Each entry of a list once, in the list's order, with its count and whether it is there.
fn listed(root: &Path, lines: impl IntoIterator<Item = String>) -> Vec<Listed> {
    let mut out: Vec<Listed> = Vec::new();
    for l in lines {
        let l = l.trim().to_string();
        if l.is_empty() {
            continue;
        }
        match out.iter_mut().find(|x| x.file.eq_ignore_ascii_case(&l)) {
            Some(x) => x.count += 1,
            None => out.push(Listed {
                exists: vfs::is_file(&resolve_path(root, &l)),
                file: l,
                count: 1,
            }),
        }
    }
    out
}

impl AiTraffic {
    /// The AI traffic of the map in `map_dir` (a folder of `root\Maps`) on `date` (YYYYMMDD),
    /// with the chrono scenarios in force that day.
    pub fn load(root: &Path, map_dir: &Path, date: i32) -> AiTraffic {
        let chrono_dirs = active_chrono_dirs(map_dir, date);
        let mut t = AiTraffic {
            date,
            chrono: chrono_dirs.iter().map(|d| rel(map_dir, d)).collect(),
            ..Default::default()
        };
        let mut names = HashMap::new();

        // the map's AI list, then each scenario's update (else full list), merged by group
        // as the game does
        let own = resolve_path(map_dir, "ailists.cfg");
        t.ailists_size = vfs::read(&own).map(|b| b.len() as u64).unwrap_or(0);
        let mut files: Vec<PathBuf> = vec![own];
        for c in &chrono_dirs {
            if let Some(p) = ["ailists_#upd.cfg", "ailists.cfg"]
                .iter()
                .map(|n| resolve_path(c, n))
                .find(|p| vfs::is_file(p))
            {
                files.push(p);
            }
        }
        for (k, path) in files.iter().enumerate() {
            let Ok(list) = AiLists::load(path) else {
                continue;
            };
            let listed_in = rel(root, path);
            t.lists.push(listed_in.clone());
            if k == 0 {
                t.default_group = list.default_group;
            }
            for g in list.groups {
                let at = t
                    .groups
                    .iter()
                    .position(|x| x.name.eq_ignore_ascii_case(&g.name) && x.is_depot == g.is_depot);
                let i = match at {
                    Some(i) => i,
                    None => {
                        t.groups.push(TrafficGroup {
                            name: g.name.clone(),
                            is_depot: g.is_depot,
                            ..Default::default()
                        });
                        t.groups.len() - 1
                    }
                };
                let base = &mut t.groups[i];
                if g.hof.is_some() {
                    base.hof = g.hof.clone();
                }
                for v in &g.vehicles {
                    let path = resolve_path(root, &v.file);
                    let exists = vfs::is_file(&path);
                    base.vehicles.push(GroupVehicle {
                        file: v.file.clone(),
                        name: if exists {
                            friendly(&path, &mut names)
                        } else {
                            String::new()
                        },
                        weight: v.weight,
                        exists,
                        listed_in: listed_in.clone(),
                    });
                }
                for tg in &g.typgroups {
                    let path = resolve_path(root, &tg.file);
                    let exists = vfs::is_file(&path);
                    base.fleet.push(FleetType {
                        file: tg.file.clone(),
                        name: if exists {
                            friendly(&path, &mut names)
                        } else {
                            String::new()
                        },
                        exists,
                        listed_in: listed_in.clone(),
                        entries: tg
                            .entries
                            .iter()
                            .map(|e| (e.clone(), typgroup_entry_valid(e, date)))
                            .collect(),
                    });
                }
            }
        }

        // the random traffic: which groups, and how much of each over the day
        let uvg = resolve_path(map_dir, "unsched_vehgroups.txt");
        if let Ok(f) = CfgFile::read(&uvg) {
            t.has_random_groups = true;
            for (name, class) in parse_unsched_vehgroups(&f) {
                if let Some(g) = t
                    .groups
                    .iter_mut()
                    .find(|g| g.name.trim().eq_ignore_ascii_case(name.trim()))
                {
                    g.random.get_or_insert(class);
                }
            }
        }
        if let Ok(f) = CfgFile::read(resolve_path(map_dir, "unsched_trafficdens.txt")) {
            t.density = parse_unsched_trafficdens(&f)
                .into_iter()
                .map(
                    |UnschedGroup {
                         name,
                         factor,
                         densities,
                     }| Density {
                        name,
                        factor,
                        curves: densities,
                    },
                )
                .collect();
        }

        // the timetable in force that day, with the bus types of each line
        let deactivated = chrono_deactivated_lines(&chrono_dirs);
        let tt = TimetableData::load_with_chrono(map_dir, &chrono_dirs, &deactivated);
        let mut uses: Vec<CarUse> = CarUse::load_dir(map_dir);
        for c in &chrono_dirs {
            uses.extend(CarUse::load_dir(c));
        }
        uses.retain(|u| u.valid_on(date));
        for line in &tt.lines {
            let types = uses
                .iter()
                .rev()
                .find(|u| u.line.trim().eq_ignore_ascii_case(line.name.trim()))
                .and_then(|u| u.types().map(|(f, l)| (f, l.to_vec())));
            t.lines.push(TrafficLine {
                name: line.name.clone(),
                file: rel(root, &line.path),
                priority: line.priority,
                user_allowed: line.user_allowed,
                types,
                tours: line
                    .tours
                    .iter()
                    .map(|tour| {
                        let deps = tour.trips.iter().map(|x| x.departure);
                        TrafficTour {
                            number: tour.number.clone(),
                            ai_group: tour.ai_group.trim().to_string(),
                            group_known: t
                                .groups
                                .iter()
                                .any(|g| g.name.trim().eq_ignore_ascii_case(tour.ai_group.trim())),
                            trips: tour.trips.len(),
                            first: deps.clone().reduce(f32::min),
                            last: deps.reduce(f32::max),
                        }
                    })
                    .collect(),
            });
        }
        t.lines
            .sort_by(|a, b| natural_key(&a.name).cmp(&natural_key(&b.name)));

        // the parking lists
        let mut parks: Vec<(usize, PathBuf)> = vfs::list_dir(map_dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, is_dir)| !is_dir)
            .filter_map(|(n, _)| {
                let lower = n.to_string_lossy().to_ascii_lowercase();
                let index = if lower == "parklist_p.txt" {
                    0
                } else {
                    lower
                        .strip_prefix("parklist_p_")?
                        .strip_suffix(".txt")?
                        .parse()
                        .ok()?
                };
                Some((index, map_dir.join(n)))
            })
            .collect();
        parks.sort();
        for (index, path) in parks {
            let text = vfs::read(&path)
                .map(|b| omsi_cfg::decode_text(&b))
                .unwrap_or_default();
            t.parklists.push(ParkList {
                file: rel(root, &path),
                index,
                cars: listed(
                    root,
                    text.lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| l.to_ascii_lowercase().ends_with(".sco")),
                ),
            });
        }

        // the people
        t.humans = listed(root, load_list(&resolve_path(map_dir, "humans.txt")));
        t.drivers = listed(root, load_list(&resolve_path(map_dir, "drivers.txt")));
        t
    }

    /// The group a tour's AI group names.
    pub fn group(&self, name: &str) -> Option<&TrafficGroup> {
        self.groups
            .iter()
            .find(|g| g.name.trim().eq_ignore_ascii_case(name.trim()))
    }

    /// The tours (line and number) whose AI group is `group`.
    pub fn tours_of(&self, group: &str) -> Vec<String> {
        self.lines
            .iter()
            .flat_map(|l| {
                l.tours
                    .iter()
                    .filter(|t| t.ai_group.eq_ignore_ascii_case(group.trim()))
                    .map(move |t| format!("{}/{}", l.name, t.number))
            })
            .collect()
    }
}

/// Lines in the order people count them: "N36" after "236", "36" before "136".
fn natural_key(s: &str) -> (bool, u64, String) {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    match digits.parse::<u64>() {
        Ok(n) => (false, n, s.to_ascii_lowercase()),
        Err(_) => (true, 0, s.to_ascii_lowercase()),
    }
}

/// `HH:MM` of minutes after midnight.
pub fn clock(minutes: f32) -> String {
    let m = minutes.round().max(0.0) as i64;
    format!("{:02}:{:02}", (m / 60) % 24, m % 60)
}

/// `YYYY-MM-DD` of a YYYYMMDD day.
pub fn day(date: i32) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date / 10000,
        (date / 100) % 100,
        date % 100
    )
}

const NL: &str = "\r\n";

fn percent(x: f32) -> String {
    format!("{:.1} %", x * 100.0)
}

impl AiTraffic {
    /// The whole view as plain text, in English with Windows line endings (as the integrity
    /// report): `header` are (name, value) lines after the title.
    pub fn to_text(&self, header: &[(String, String)]) -> String {
        let mut o = String::new();
        let line = |o: &mut String, s: String| {
            o.push_str(&s);
            o.push_str(NL);
        };
        line(&mut o, "Project Depot - AI traffic".into());
        let wide = header
            .iter()
            .map(|(k, _)| k.chars().count())
            .chain(["AI lists".len()])
            .max()
            .unwrap_or(0);
        for (k, v) in header {
            line(&mut o, format!("{k:<wide$} : {v}"));
        }
        line(&mut o, format!("{:<wide$} : {}", "Day", day(self.date)));
        line(
            &mut o,
            format!(
                "{:<wide$} : {}",
                "Chrono",
                if self.chrono.is_empty() {
                    "none in force".to_string()
                } else {
                    self.chrono.join(", ")
                }
            ),
        );
        line(
            &mut o,
            format!(
                "{:<wide$} : {}",
                "AI lists",
                if self.lists.is_empty() {
                    "none".to_string()
                } else {
                    self.lists.join(", ")
                }
            ),
        );
        if self.ailists_size > 0 {
            line(
                &mut o,
                format!(
                    "{:<wide$} : {:.1} KB{}",
                    "Size",
                    self.ailists_size as f64 / 1024.0,
                    if self.ailists_size > crate::AILISTS_RECOMMENDED_MAX {
                        " (more than 16 KB: not recommended except on high-performance computers)"
                    } else {
                        ""
                    }
                ),
            );
        }

        let cars: Vec<(usize, &TrafficGroup)> = self
            .groups
            .iter()
            .enumerate()
            .filter(|(_, g)| !g.is_depot)
            .collect();
        line(&mut o, String::new());
        line(&mut o, format!("== Car groups ({}) ==", cars.len()));
        for (i, g) in cars {
            let mut notes = Vec::new();
            if i == self.default_group {
                notes.push("default".to_string());
            }
            if let Some(c) = g.random {
                notes.push(format!("random traffic, class {c}"));
            }
            let notes = if notes.is_empty() {
                String::new()
            } else {
                format!(" ({})", notes.join(", "))
            };
            line(
                &mut o,
                format!("{}{notes}: {} vehicles", g.name, g.vehicles.len()),
            );
            for (k, v) in g.vehicles.iter().enumerate() {
                line(
                    &mut o,
                    format!(
                        "    {:>6} {:>8}  {}{}{}",
                        format!("{}", v.weight),
                        percent(g.chance(k)),
                        v.file,
                        if v.name.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", v.name)
                        },
                        if v.exists { "" } else { " - missing" }
                    ),
                );
            }
        }

        let depots: Vec<&TrafficGroup> = self.groups.iter().filter(|g| g.is_depot).collect();
        line(&mut o, String::new());
        line(&mut o, format!("== Depots ({}) ==", depots.len()));
        for g in depots {
            let all: usize = g.fleet.iter().map(|t| t.entries.len()).sum();
            line(
                &mut o,
                format!(
                    "{} (depot file {}): {} of {all} in service",
                    g.name,
                    g.hof.as_deref().unwrap_or("none"),
                    g.in_service()
                ),
            );
            for t in &g.fleet {
                line(
                    &mut o,
                    format!(
                        "  {}{}{}",
                        t.file,
                        if t.name.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", t.name)
                        },
                        if t.exists { "" } else { " - missing" }
                    ),
                );
                for (e, on) in &t.entries {
                    let span = match (e.from, e.to) {
                        (None, None) => String::new(),
                        (f, t) => format!(
                            "  {} to {}",
                            f.map(day).unwrap_or_else(|| "...".into()),
                            t.map(day).unwrap_or_else(|| "...".into())
                        ),
                    };
                    line(
                        &mut o,
                        format!(
                            "    {:<8} {:<14} {}{span}{}",
                            e.number,
                            e.registration,
                            e.paint,
                            if *on { "" } else { " - not in service" }
                        ),
                    );
                }
            }
        }

        line(&mut o, String::new());
        line(&mut o, format!("== Lines ({}) ==", self.lines.len()));
        for l in &self.lines {
            let types = match &l.types {
                Some((f, list)) if *f >= 1.0 => format!(", only {}", list.join(", ")),
                Some((f, list)) => format!(", {} preferred ({})", list.join(", "), percent(*f)),
                None => String::new(),
            };
            line(
                &mut o,
                format!(
                    "{} ({}): {} tours, priority {}{types}",
                    l.name,
                    l.file,
                    l.tours.len(),
                    l.priority
                ),
            );
            for t in &l.tours {
                let span = match (t.first, t.last) {
                    (Some(a), Some(b)) => format!("{}-{}", clock(a), clock(b)),
                    _ => String::new(),
                };
                line(
                    &mut o,
                    format!(
                        "    {:<10} {:<20} {:>3} trips  {span}{}",
                        t.number,
                        t.ai_group,
                        t.trips,
                        if t.group_known {
                            ""
                        } else {
                            " - no AI list has this group"
                        }
                    ),
                );
            }
        }

        line(&mut o, String::new());
        line(
            &mut o,
            format!("== Parking lists ({}) ==", self.parklists.len()),
        );
        for p in &self.parklists {
            line(&mut o, format!("{}: {} cars", p.file, p.total()));
            for c in &p.cars {
                line(
                    &mut o,
                    format!(
                        "    {:>3}x {}{}",
                        c.count,
                        c.file,
                        if c.exists { "" } else { " - missing" }
                    ),
                );
            }
        }

        for (title, list) in [("People", &self.humans), ("Drivers", &self.drivers)] {
            line(&mut o, String::new());
            line(&mut o, format!("== {title} ({}) ==", list.len()));
            for c in list {
                line(
                    &mut o,
                    format!(
                        "    {:>3}x {}{}",
                        c.count,
                        c.file,
                        if c.exists { "" } else { " - missing" }
                    ),
                );
            }
        }

        line(&mut o, String::new());
        line(
            &mut o,
            format!("== Traffic density ({}) ==", self.density.len()),
        );
        for d in &self.density {
            line(&mut o, format!("{}: factor {}", d.name, d.factor));
            for (mask, curve) in &d.curves {
                let days = match mask {
                    0 => "every day".to_string(),
                    m => [(1, "weekdays"), (2, "Saturday"), (4, "Sunday")]
                        .iter()
                        .filter(|(b, _)| m & b != 0)
                        .map(|(_, n)| *n)
                        .collect::<Vec<_>>()
                        .join(", "),
                };
                let points = curve
                    .iter()
                    .map(|(h, v)| format!("{}={v}", clock(h * 60.0)))
                    .collect::<Vec<_>>()
                    .join(" ");
                line(&mut o, format!("    {days}: {points}"));
            }
        }
        o
    }
}
