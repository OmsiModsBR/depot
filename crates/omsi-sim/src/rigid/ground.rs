//! What the wheels stand on: the ground at a point ([`Ground`]) and what a tyre makes of it -
//! the envelope laid over the road along its length ([`tyre_line`]) and the faces it cannot
//! climb ([`WallFace`], [`track_walls`]).

use super::params::{
    EDGE, EDGE_HALVINGS, ENVELOPE, FACE_HALVINGS, FACE_SPAN, HALF_WIDTH, LATTICE, SIDE_REACH,
    TYRE_SAMPLES, WALL_BEFORE, WALL_BEHIND, WALL_FACING, WALL_GRAZE, WALL_KEEP,
};
use glam::{DVec2, DVec3, Vec2, Vec3};

/// The ground a wheel finds at one point: the highest face at or below the probe's top and
/// the lowest face above it (world heights).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GroundProbe {
    pub below: Option<f64>,
    pub above: Option<f64>,
}

/// What the wheels of a vehicle stand on.
pub trait Ground: Send + Sync {
    /// The faces at world (x, y): the highest at or below `top` and the lowest above it.
    fn probe(&self, x: f64, y: f64, top: f64) -> GroundProbe;

    /// A prober for one step of one vehicle - many points close together. It may keep what
    /// it looked up (the tiles under the vehicle) until it is dropped.
    fn session(&self) -> Box<dyn Fn(f64, f64, f64) -> GroundProbe + '_> {
        Box::new(move |x, y, top| self.probe(x, y, top))
    }
}

impl<F: Fn(f64, f64, f64) -> GroundProbe + Send + Sync> Ground for F {
    fn probe(&self, x: f64, y: f64, top: f64) -> GroundProbe {
        self(x, y, top)
    }
}

/// A face a tyre cannot climb, kept while the tyre is up against it: the samples that found
/// it are lost as soon as the hub is over it, and a wheel pushed that far went on through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallFace {
    /// A point of the face (world, ground plane) and its normal, towards the tyre.
    pub point: DVec2,
    pub normal: Vec2,
}

impl WallFace {
    /// How far the hub stands off the face (negative once it is past it).
    pub(super) fn distance(&self, hub: DVec3) -> f32 {
        (hub.truncate() - self.point).dot(self.normal.as_dvec2()) as f32
    }

    /// How far a tyre of radius `r` rolling along `fwd_h` reaches from its hub towards the
    /// face: its radius straight ahead, half its width to the side.
    pub(super) fn reach(&self, fwd_h: Vec3, r: f32) -> f32 {
        let n = self.normal.extend(0.0);
        let right_h = Vec3::new(fwd_h.y, -fwd_h.x, 0.0);
        r * n.dot(fwd_h).abs() + HALF_WIDTH * r * n.dot(right_h).abs()
    }
}

/// What a tyre finds along its length (see [`tyre_line`]).
pub(super) struct TyreLine {
    /// The world height the hub has to stand at, how fast that rises as the hub rolls
    /// forward (m per m) and where along the tyre (m ahead of the hub) the ground carries
    /// it. None when there is nothing below the tyre at all.
    pub(super) ground: Option<(f64, f32, f32)>,
    /// A face the tyre cannot climb ahead of the hub and one behind it, each as the last
    /// clear sample and the first one in the face (m ahead of the hub), when it is near
    /// enough to be touched within the substep.
    pub(super) faces: [Option<(f32, f32)>; 2],
    /// The samples either side of the hub are clear of faces: it is not sunk into a step.
    pub(super) clear: bool,
}

/// The ground under and around a tyre of radius `r` rolling along `fwd_h`.
///
/// The tyre is an envelope [`ENVELOPE`] radii long (a flattened circle) laid over the ground,
/// and the hub stands where the envelope first touches it: `need` = the highest ground
/// height plus the envelope's height over it. The ground is a polyline through samples
/// that stand still on the ground while the hub rolls past them (a lattice along the tyre's
/// direction), with an edge between two of them placed by halving. Anchored so, the
/// polyline is the same at every step, `need` changes smoothly as the hub rolls, and the
/// slope reported is exactly how fast it changes: the ground's own slope where the envelope
/// rests on a face of it, the envelope's where it slides over a corner. Only then does the
/// push that slope gives (see `step`) take from the body what lifting it over a step gains;
/// a slope from the samples that differed from it pumped energy into the body on steep
/// faces, and one from a comb carried with the hub read every grade over 6 % as a run of
/// kerbs.
///
/// A sample whose ground is higher than `z_top` (the most the tyre climbs) is no ground,
/// and within a radius, a sample and `look` metres of the hub it is a face the tyre runs
/// against if that face is lower than `wall_top`.
pub(super) fn tyre_line(probe: &dyn Fn(f64, f64, f64) -> GroundProbe, hub: DVec3, fwd_h: Vec3, r: f32, z_top: f64, wall_top: f64, look: f32) -> TyreLine {
    const N: usize = LATTICE;
    let a = r * ENVELOPE;
    let lift = |x: f32| r * (1.0 - (x / a).powi(2)).max(0.0).sqrt();
    // d need / d hub while the envelope slides over a corner x ahead of the hub
    let corner_slope = |x: f32| r * x / (a * a * (1.0 - (x / a).powi(2)).max(1e-3).sqrt());
    let spacing = 2.0 * a / (TYRE_SAMPLES - 1) as f32;
    let fwd = fwd_h.as_dvec3();
    let along = hub.x * fwd.x + hub.y * fwd.y;
    let first = ((along - a as f64) / spacing as f64).floor() - 2.0;
    // metres ahead of the hub of lattice point k: x(2) <= -a < x(3), x(N - 4) <= a < x(N - 3)
    let x_of = |k: usize| (first + k as f64) * spacing as f64 - along;
    let at = |dx: f64| {
        let p = hub + fwd * dx;
        probe(p.x, p.y, z_top)
    };
    let xs: [f32; N] = std::array::from_fn(|k| x_of(k) as f32);
    let mut below = [None; N];
    let mut wall = [false; N];
    for k in 0..N {
        let g = at(x_of(k));
        below[k] = g.below;
        wall[k] = xs[k].abs() <= r + spacing + look && g.above.map(|z| z < wall_top).unwrap_or(false);
    }
    let rise = |k: usize| match (below.get(k).copied().flatten(), below.get(k + 1).copied().flatten()) {
        (Some(g0), Some(g1)) => Some(g1 - g0),
        _ => None,
    };
    // An edge: a rise unlike those either side of it. A grade, however steep, rises alike
    // from sample to sample; a curve changes its rise by far less than EDGE over one
    // spacing. Every rise that decides the pieces over the envelope has both neighbours
    // (and they theirs), so a step is told the same way wherever the hub stands; told from
    // one neighbour, the level ground after a step at the end of the samples was an edge
    // too, the step no longer a lone one, and the hub jumped a centimetre as the lattice
    // moved on.
    let edge: [bool; N] = std::array::from_fn(|k| match (rise(k), k.checked_sub(1).and_then(rise), rise(k + 1)) {
        (Some(d), Some(l), Some(h)) => (d - l).abs() > EDGE && (d - h).abs() > EDGE,
        _ => false,
    });

    let mut best: Option<(f64, f32, f32)> = None;
    let mut offer = |need: f64, slope: f32, x: f32| {
        if best.map_or(true, |b| need > b.0) {
            best = Some((need, slope, x));
        }
    };
    // the highest the envelope has to stand over the piece of ground from (x0, z0) to (x1, z1)
    let mut piece = |x0: f32, z0: f64, x1: f32, z1: f64| {
        if x1 < -a || x0 > a {
            return;
        }
        if x1 - x0 < 1e-6 {
            offer(z0.max(z1) + lift(x0) as f64, corner_slope(x0), x0);
            return;
        }
        let m = ((z1 - z0) / (x1 - x0) as f64) as f32;
        let (lo, hi) = (x0.max(-a), x1.min(a));
        // where the envelope's own slope matches the ground's
        let t = m * a / r;
        let free = a * t / (1.0 + t * t).sqrt();
        let x = free.clamp(lo, hi);
        let need = z0 + (m * (x - x0)) as f64 + lift(x) as f64;
        // resting on the face, or at the end of the tyre (which rolls on with the hub), the
        // need rises with the ground; at a corner of the ground the envelope slides over it
        let slope = if x == free || x <= -a || x >= a { m } else { corner_slope(x) };
        offer(need, slope, x);
    };
    for k in 0..N {
        let Some(g0) = below[k] else { continue };
        let left = k > 0 && below[k - 1].is_some();
        let Some(g1) = below.get(k + 1).copied().flatten() else {
            if !left && xs[k].abs() <= a {
                piece(xs[k], g0, xs[k], g0);
            }
            continue;
        };
        let lone_edge = edge[k] && !(k > 0 && edge[k - 1]) && !edge.get(k + 1).copied().unwrap_or(false);
        if !lone_edge || xs[k + 1] < -a || xs[k] > a {
            // no step, one beyond the envelope, or a run of them: a steep face the samples
            // outline well enough
            piece(xs[k], g0, xs[k + 1], g1);
            continue;
        }
        // A step between the two. The ground either side runs on at its own grade (that of
        // the rise beyond it) up to where the step is: the point of the gap whose height is
        // nearer the far side's.
        let grade = |j: usize| rise(j).unwrap_or(0.0) / spacing as f64;
        let (m_lo, m_hi) = (grade(k - 1), grade(k + 1));
        let (x0, x1) = (x_of(k), x_of(k + 1));
        let near = |x: f64| g0 + m_lo * (x - x0);
        let far = |x: f64| g1 + m_hi * (x - x1);
        let (mut lo, mut hi) = (x0, x1);
        for _ in 0..EDGE_HALVINGS {
            let m = 0.5 * (lo + hi);
            match at(m).below {
                Some(z) if (z - far(m)).abs() < (z - near(m)).abs() => hi = m,
                Some(_) => lo = m,
                None => break,
            }
        }
        let c = 0.5 * (lo + hi);
        piece(xs[k], g0, c as f32, near(c));
        piece(c as f32, far(c), xs[k + 1], g1);
    }

    // the faces nearest the hub ahead and behind, bracketed by the clear sample before them
    let ahead = (0..N).find(|&k| xs[k] > 0.0 && wall[k]).filter(|&k| k > 0 && !wall[k - 1]).map(|k| (xs[k - 1], xs[k]));
    let behind = (0..N).rev().find(|&k| xs[k] <= 0.0 && wall[k]).filter(|&k| k + 1 < N && !wall[k + 1]).map(|k| (xs[k + 1], xs[k]));
    let clear = (0..N - 1).find(|&k| xs[k] <= 0.0 && xs[k + 1] > 0.0).map_or(true, |k| !wall[k] && !wall[k + 1]);
    TyreLine { ground: best, faces: [ahead, behind], clear }
}

/// The faces a tyre is up against after this step: those it was against that are still
/// there and near, and new ones its samples (and a look to either side, `look` metres
/// further than the tyre reaches) found.
#[allow(clippy::too_many_arguments)]
pub(super) fn track_walls(probe: &dyn Fn(f64, f64, f64) -> GroundProbe, old: &[WallFace], line: &TyreLine, hub: DVec3, fwd_h: Vec3, r: f32, z_top: f64, wall_top: f64, look: f32) -> Vec<WallFace> {
    let is_wall = |p: DVec2| probe(p.x, p.y, z_top).above.map(|z| z < wall_top).unwrap_or(false);
    let hub2 = hub.truncate();
    // A face stays while the hub is near it (as near as the samples find one) and has not
    // gone through by a whole radius, and while the ground just behind the line is still
    // the face and the ground just before it still clear (the face ends, or the line
    // drifted off a curved one).
    let mut out: Vec<WallFace> = old
        .iter()
        .copied()
        .filter(|f| {
            let d = f.distance(hub);
            let n = f.normal.as_dvec2();
            let foot = hub2 - n * d as f64;
            d > -r && d < WALL_KEEP * r && is_wall(foot - n * WALL_BEHIND) && !is_wall(foot + n * WALL_BEFORE)
        })
        .collect();
    let facing = |out: &[WallFace], dir: Vec2| out.iter().any(|f| f.normal.dot(-dir) > WALL_FACING);
    let fwd2 = fwd_h.truncate().normalize_or(Vec2::Y);
    for (clear, wall) in line.faces.iter().flatten().copied() {
        let dir = fwd2 * wall.signum();
        if !facing(&out, dir) {
            let d = halve(&is_wall, hub2, dir, clear * wall.signum(), wall.abs());
            out.push(find_face(&is_wall, hub2, dir, d));
        }
    }
    if line.clear {
        // beside the tyre (a bus sliding or turning into a platform)
        let reach = SIDE_REACH * HALF_WIDTH * r + look;
        for dir in [fwd2.perp(), -fwd2.perp()] {
            if !facing(&out, dir) && is_wall(hub2 + (dir * reach).as_dvec2()) {
                let d = halve(&is_wall, hub2, dir, 0.0, reach);
                out.push(find_face(&is_wall, hub2, dir, d));
            }
        }
    }
    out
}

/// Where between `clear` and `wall` metres from `from` along `dir` a face begins.
fn halve(is_wall: &dyn Fn(DVec2) -> bool, from: DVec2, dir: Vec2, clear: f32, wall: f32) -> f32 {
    let (mut a, mut b) = (clear, wall);
    for _ in 0..FACE_HALVINGS {
        let m = 0.5 * (a + b);
        if is_wall(from + (dir * m).as_dvec2()) {
            b = m;
        } else {
            a = m;
        }
    }
    0.5 * (a + b)
}

/// A face found `d` metres from the hub along `dir`, with its direction taken from where the
/// lines [`FACE_SPAN`] either side of that one meet it (square to `dir` where they miss it).
fn find_face(is_wall: &dyn Fn(DVec2) -> bool, hub2: DVec2, dir: Vec2, d: f32) -> WallFace {
    let across = dir.perp();
    let point = hub2 + (dir * d).as_dvec2();
    let side = |s: f32| {
        let from = hub2 + (across * s * FACE_SPAN).as_dvec2();
        let (a, b) = (d - 2.0 * FACE_SPAN, d + 2.0 * FACE_SPAN);
        let hit = !is_wall(from + (dir * a).as_dvec2()) && is_wall(from + (dir * b).as_dvec2());
        hit.then(|| from + (dir * halve(is_wall, from, dir, a, b)).as_dvec2())
    };
    let tangent = match (side(-1.0), side(1.0)) {
        (Some(p), Some(q)) => q - p,
        (Some(p), None) => point - p,
        (None, Some(q)) => q - point,
        (None, None) => across.as_dvec2(),
    };
    let mut normal = tangent.perp().normalize_or_zero().as_vec2();
    if normal.dot(dir) > 0.0 {
        normal = -normal;
    }
    if normal.dot(dir) > -WALL_GRAZE {
        // a face the lines graze is no face to push the tyre along
        normal = -dir;
    }
    WallFace { point, normal }
}

#[cfg(test)]
mod tests {
    use super::super::params::{CLIMB, WALL_HEIGHT};
    use super::super::tests::ramp;
    use super::*;

    /// On a uniform grade of any steepness the tyre reports the grade itself and stands where
    /// the envelope touches the ramp, with no more probes than on the flat. The old comb read
    /// every grade over 6.4 % as a run of kerbs: 0.043 for 6.5-9 %, 0.13 for 10-15 %, and 73
    /// probes a wheel.
    #[test]
    fn tyre_reads_any_grade_as_a_grade() {
        let r = 0.47f32;
        let a = (r * ENVELOPE) as f64;
        for pct in [0.0, 2.0, 5.0, 6.3, 6.5, 8.0, 9.0, 10.0, 12.0, 15.0, 20.0, 25.0] {
            let s = pct / 100.0;
            let count = std::cell::Cell::new(0);
            let g = ramp(s);
            let probe = |x: f64, y: f64, top: f64| {
                count.set(count.get() + 1);
                g(x, y, top)
            };
            for k in 0..40 {
                let y = 100.0 + k as f64 * 0.037;
                let hub = DVec3::new(0.0, y, y * s + r as f64);
                let z_top = y * s + (CLIMB * r) as f64 + 0.05;
                count.set(0);
                let line = tyre_line(&probe, hub, Vec3::Y, r, z_top, hub.z + WALL_HEIGHT, 0.0);
                let (need, slope, x) = line.ground.expect("ground");
                // the envelope touches where its own slope is the grade
                let t = s * a / r as f64;
                let xs = a * t / (1.0 + t * t).sqrt();
                let want = s * (y + xs) + r as f64 * (1.0 - (xs / a).powi(2)).sqrt();
                assert!((slope as f64 - s).abs() < 1e-4, "{pct} %: slope {slope} at {y}");
                assert!((need - want).abs() < 1e-5, "{pct} %: need {need}, want {want}");
                assert!((x as f64 - xs).abs() < 1e-3, "{pct} %: contact at {x}, want {xs}");
                assert!(line.faces == [None, None] && line.clear);
                assert!(count.get() <= LATTICE, "{pct} %: {} probes", count.get());
            }
        }
    }

    /// Walked over a step or a chamfered face (on the level or on a grade), the push the
    /// slope gives takes exactly what the hub is lifted by (∫ slope dx = Δneed), and the hub
    /// never jumps as the samples move on: the old corner slope gave back only 0.43 of the
    /// load for 0.6 of it on a steep face and pumped the body up.
    #[test]
    fn climbing_a_face_costs_what_it_lifts() {
        let r = 0.47f32;
        for (grade, height, angle) in [(0.0, 0.15, 90.0f64), (0.0, 0.2, 72.0), (0.0, 0.2, 60.0), (0.0, 0.1, 45.0), (0.0, 0.3, 80.0), (0.0, 0.05, 20.0), (0.0, 0.37, 90.0), (0.0, 0.37, 70.0), (0.1, 0.15, 90.0), (-0.08, 0.2, 75.0)] {
            let run = height / angle.to_radians().tan();
            let face = move |_x: f64, y: f64, top: f64| {
                let z = grade * y + ((y - 5.0) / run.max(1e-9)).clamp(0.0, 1.0) * height;
                if z <= top { GroundProbe { below: Some(z), above: None } } else { GroundProbe { below: Some(0.0), above: Some(z) } }
            };
            let need_at = |y: f64| {
                let hub = DVec3::new(0.3, y, r as f64 + grade * y);
                let top = hub.z - r as f64 + (CLIMB * r) as f64;
                tyre_line(&face, hub, Vec3::Y, r, top.max(grade * 5.0 + height), hub.z + WALL_HEIGHT, 0.0).ground.expect("ground")
            };
            let (y0, y1, dy) = (2.5, 7.0, 0.0005);
            let (mut work, mut y, mut need) = (0.0, y0, need_at(y0).0);
            while y < y1 {
                let slope = need_at(y + dy / 2.0).1 as f64;
                work += slope * dy;
                y += dy;
                let next = need_at(y).0;
                assert!((next - need - slope * dy).abs() < 1e-3, "{height} m at {angle}° on {grade}: the hub jumped by {} at {y}", next - need - slope * dy);
                need = next;
            }
            let lift = need_at(y).0 - need_at(y0).0;
            let want = height + grade * (y - y0);
            assert!((lift - want).abs() < 1e-3, "{height} m at {angle}° on {grade}: lifted {lift}, want {want}");
            assert!((work - lift).abs() < 0.003 * height.max(0.1), "{height} m at {angle}° on {grade}: pushed back {work} for {lift}");
        }
    }
}
