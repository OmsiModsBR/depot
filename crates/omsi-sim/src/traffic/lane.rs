//! A lane of the traffic network: its kind, its map identity, its geometry along the way
//! and the rules on it (who may use it, how dense the traffic is, its priority).

use glam::DVec3;

use super::geometry::{cumulative, heading_dir, wrap_deg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneKind {
    Street,
    Sidewalk,
    Rail,
    /// `[path]` type 3: flight paths of AI aircraft.
    Air,
}

impl LaneKind {
    pub fn from_code(c: i32) -> LaneKind {
        match c {
            1 => LaneKind::Sidewalk,
            2 => LaneKind::Rail,
            3 => LaneKind::Air,
            _ => LaneKind::Street,
        }
    }
}

/// Identity of a lane in map terms: the tile, the spline/object id and the `[path]` index.
/// Timetable tracks reference lanes this way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LaneKey {
    pub tile: (i32, i32),
    pub id: i64,
    pub path: u16,
}

/// A sampled lane: points with headings, in world coordinates.
#[derive(Debug, Clone)]
pub struct Lane {
    pub key: Option<LaneKey>,
    /// True for the backwards lane of a two-way path.
    pub reversed: bool,
    pub kind: LaneKind,
    pub width: f32,
    /// World positions along the lane (in travel direction).
    pub points: Vec<DVec3>,
    /// Heading (degrees, clockwise from north) at each point: the tangent of the spline or
    /// arc the lane was sampled from, not the direction of the chord to the next point.
    pub headings: Vec<f32>,
    /// Signed curvature at each point (1/m, positive = turning right).
    pub curvature: Vec<f32>,
    /// Cumulative distance at each point.
    pub dist: Vec<f32>,
    pub speed_limit_kmh: f32,
    /// Lanes reachable from the end of this one.
    pub next: Vec<usize>,
    /// Traffic light controlling entry into this lane: (object instance, light index).
    pub traffic_light: Option<(usize, usize)>,
    /// Turn indicator for AI: 0 none, 1 left, 2 right.
    pub turn: i32,
    /// Source for debugging.
    pub source: u32,
    /// Lateral offset of the spline path (m, positive = right of the spline direction).
    pub offset: f32,
    /// Spline/object file the lane came from (debugging).
    pub name: String,
    /// `[rule] trafficdensity`: how much of the road traffic uses this lane (0 = none), of
    /// any random traffic group.
    pub density: f32,
    /// `[rule] trafficdensity <value> <group>` of the path, the last per group: the group
    /// is the random traffic group's place in the map's `unsched_vehgroups.txt`. A group
    /// without one takes its default there (Berlin-Spandau's GDR cars only drive where
    /// the Falkensee paths ask for them).
    pub group_density: Vec<(u16, f32)>,
    /// `[rule] no_cars`: cars keep off this lane.
    pub no_cars: bool,
    /// `[rule] bus` / `[rule] trucks` on the path: it is open to the AI vehicles of
    /// `[ai_veh_type]` 2 / 3 (see [`Lane::allows`]). The rules are switches: the value
    /// after them is not read (Grundorf's 110 `trucks` rules all say 0).
    pub rule_bus: bool,
    pub rule_trucks: bool,
    /// The spline this lane belongs to is editor-only: OMSI's invisible service roads at
    /// the edge of a map, where AI traffic drives with no road drawn under it.
    pub invisible: bool,
    /// Parallel lanes of the same road in the same direction (for lane changes).
    pub left: Option<usize>,
    pub right: Option<usize>,
    /// `[rule] priority` (`TPRIPriority`): who goes first where two paths of a junction
    /// meet. The stock maps put 192 on the straight paths of the main road and 64 on the
    /// paths coming out of a side road and leave the rest alone, so an unmarked path sits
    /// in between at `DEFAULT_PRIORITY`.
    pub priority: f32,
    /// Other `[path]`s of its object this lane blocks while taken (`[blockpath]`), beyond
    /// the places where they cross.
    pub blocks: Vec<u16>,
}

/// Priority of a path without a `[rule] priority`.
pub const DEFAULT_PRIORITY: f32 = 128.0;

/// How much of `unsched_vehgroups.txt` group `pool`'s traffic a path carries: its `[rule]
/// trafficdensity` for the group (`rules`, see `Lane::group_density`: the last rule of a
/// group counts), else the group's default there (`defaults`): 0 none, for the first group 1
/// its medium density, for any other k that of the k-th group on the same path.
pub fn pool_density(rules: &[(u16, f32)], defaults: &[i32], pool: usize) -> f32 {
    let mut u = pool;
    // (a default naming another group that names this one again would go round for ever)
    for _ in 0..=defaults.len() {
        if let Some(&(_, v)) = rules.iter().rev().find(|(k, _)| *k as usize == u) {
            return v;
        }
        match defaults.get(u).copied().unwrap_or(0) {
            d if d <= 0 => return 0.0,
            _ if u == 0 => return 1.0,
            d => u = d as usize - 1,
        }
    }
    0.0
}

impl Lane {
    /// May an AI vehicle of `[ai_veh_type]` `veh_type` drive here (as OMSI does, by
    /// the path's rules): a car (0) where there is no `no_cars`, a taxi (1) also where
    /// `bus` or `trucks` opens a no_cars path, a bus (2) only where `bus` and a truck (3)
    /// only where `trucks` is set. Other values (and the timetable's buses, -1) anywhere.
    pub fn allows(&self, veh_type: i32) -> bool {
        match veh_type {
            0 => !self.no_cars,
            1 => !self.no_cars || self.rule_bus || self.rule_trucks,
            2 => self.rule_bus,
            3 => self.rule_trucks,
            _ => true,
        }
    }

    /// How much of `unsched_vehgroups.txt` group `pool`'s traffic the lane carries (see
    /// [`pool_density`]).
    pub fn pool_density(&self, defaults: &[i32], pool: usize) -> f32 {
        pool_density(&self.group_density, defaults, pool)
    }

    /// Closest point of the lane's polyline to `p`: (distance along the lane, distance to it).
    pub fn nearest_point(&self, p: DVec3) -> Option<(f32, f64)> {
        let mut best: Option<(f32, f64)> = None;
        for k in 0..self.points.len().saturating_sub(1) {
            let a = self.points[k];
            let b = self.points[k + 1];
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
            let d = (a + ab * t - p).length();
            if best.map(|b| d < b.1).unwrap_or(true) {
                best = Some((
                    self.dist[k] + (self.dist[k + 1] - self.dist[k]) * t as f32,
                    d,
                ));
            }
        }
        best
    }

    pub fn length(&self) -> f32 {
        *self.dist.last().unwrap_or(&0.0)
    }

    /// Measure the lane again after its points were moved (an object's tilt).
    pub fn refresh(&mut self) {
        self.dist = cumulative(&self.points);
    }

    /// Segment and fraction along it at distance `s` (clamped to the lane).
    fn locate(&self, s: f32) -> (usize, f32) {
        // (a position gone NaN takes the lane's start rather than stopping the game)
        let s = if s.is_nan() {
            0.0
        } else {
            s.clamp(0.0, self.length())
        };
        let i = match self.dist.binary_search_by(|d| d.total_cmp(&s)) {
            Ok(i) => i.min(self.points.len() - 2),
            Err(i) => i.saturating_sub(1).min(self.points.len() - 2),
        };
        let (d0, d1) = (self.dist[i], self.dist[i + 1]);
        (i, if d1 > d0 { (s - d0) / (d1 - d0) } else { 0.0 })
    }

    /// Position and heading at distance `s`. Between two samples the lane is a cubic
    /// Hermite curve whose tangents are the sampled headings, so a bend stays round
    /// instead of becoming a chain of chords: a car following the chords turned in small
    /// jerks at every sample and at every lane joint.
    pub fn at(&self, s: f32) -> (DVec3, f32) {
        if self.points.len() < 2 {
            return (
                self.points.first().copied().unwrap_or(DVec3::ZERO),
                self.headings.first().copied().unwrap_or(0.0),
            );
        }
        let (i, t) = self.locate(s);
        let (h0, h1) = (self.headings[i], self.headings[i + 1]);
        (self.hermite(i, t as f64), h0 + wrap_deg(h1 - h0) * t)
    }

    fn hermite(&self, i: usize, t: f64) -> DVec3 {
        let (a, b) = (self.points[i], self.points[i + 1]);
        let lin = a.lerp(b, t);
        let chord = (b - a).truncate();
        let len = chord.length();
        if len < 1e-6 {
            return lin;
        }
        let d0 = heading_dir(self.headings[i] as f64);
        let d1 = heading_dir(self.headings[i + 1] as f64);
        // tangents far off the chord (a kink in hand-made samples) would make the curve loop
        let c = chord / len;
        if d0.dot(c) < 0.8 || d1.dot(c) < 0.8 {
            return lin;
        }
        let (t2, t3) = (t * t, t * t * t);
        let xy = a.truncate() * (2.0 * t3 - 3.0 * t2 + 1.0)
            + d0 * len * (t3 - 2.0 * t2 + t)
            + b.truncate() * (3.0 * t2 - 2.0 * t3)
            + d1 * len * (t3 - t2);
        DVec3::new(xy.x, xy.y, lin.z)
    }

    /// Position and heading at `s`, continued straight along the end tangents beyond either
    /// end of the lane.
    pub fn at_ext(&self, s: f32) -> (DVec3, f32) {
        if s < 0.0 {
            let h = self.start_heading();
            let d = heading_dir(h as f64) * s as f64;
            (self.start() + DVec3::new(d.x, d.y, 0.0), h)
        } else if s > self.length() {
            let h = self.end_heading();
            let d = heading_dir(h as f64) * (s - self.length()) as f64;
            (self.end() + DVec3::new(d.x, d.y, 0.0), h)
        } else {
            self.at(s)
        }
    }

    /// Curvature at `s` (1/m, positive = right).
    pub fn curvature_at(&self, s: f32) -> f32 {
        if self.points.len() < 2 || self.curvature.len() != self.points.len() {
            return 0.0;
        }
        let (i, t) = self.locate(s);
        self.curvature[i] + (self.curvature[i + 1] - self.curvature[i]) * t
    }

    pub fn start(&self) -> DVec3 {
        self.points[0]
    }

    pub fn end(&self) -> DVec3 {
        *self.points.last().unwrap()
    }

    pub fn start_heading(&self) -> f32 {
        self.headings[0]
    }

    pub fn end_heading(&self) -> f32 {
        *self.headings.last().unwrap()
    }
}

/// Speed limit of a flight path without a `[rule] speedlimit` (km/h): none, the aircraft
/// flies at its own speed.
pub const AIR_NO_LIMIT_KMH: f32 = 1000.0;
