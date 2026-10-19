use glam::{DVec2, Vec2, Vec3};
use omsi_ui::{Atlas, Color, Painter, Rect};
use super::{BUS, DOT, METRO, TRAIN, TRAM, TROLLEY};

/// What a traffic vehicle is on the map: a trolleybus (its model has trolley poles to
/// raise, `cp_SHTANGALEV` or `shtanga_lev_rot`), a tram, a bus (one on a timetable or
/// showing a line), or any other car; and the line it shows, if it is public transport.
pub(super) fn traffic_kind(c: &crate::traffic::AiCar, surface: Option<&crate::map_surface::MapSurface>, line_of: Option<&dyn Fn(u64) -> Option<String>>) -> (Color, Option<String>, Icon) {
    let v = &c.vehicle;
    let shown = v
        .ty
        .program
        .str_var("SetLineTo")
        .and_then(|i| v.state.str_vars.get(i as usize))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "0");
    let trolley = v.var("cp_SHTANGALEV").is_some() || v.var("shtanga_lev_rot").is_some();
    if c.is_rail() {
        // a tram, a metro train or a train, by what the map says its vehicles are (the others: a tram)
        let kind = surface.and_then(|s| s.rail_kind(&v.ty.def.path.to_string_lossy()));
        let (color, icon) = match kind {
            Some(crate::map_surface::MODAL_METRO) => (METRO, Icon::Metro),
            Some(crate::map_surface::MODAL_RAIL) => (TRAIN, Icon::Train),
            _ => (TRAM, Icon::Tram),
        };
        return (color, shown.or_else(|| line_of.and_then(|f| f(c.id))), icon);
    }
    let color = if trolley {
        TROLLEY
    } else if c.bus.is_some() || shown.is_some() {
        BUS
    } else {
        return (DOT, None, Icon::Road);
    };
    (color, shown, Icon::Road)
}
/// `v` turned anticlockwise by `a` radians.
pub(super) fn rot(v: DVec2, a: f64) -> DVec2 {
    let (s, c) = a.sin_cos();
    DVec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// The player as a triangle (tip ahead, short base behind), after the radar's: a dark edge, a
/// white ring when the player is on a line, and the colour of the line's kind inside.
pub(super) fn player_triangle(ui: &mut Painter, at: Vec2, angle: f32, side: f32, fill: Color, ring: bool) {
    let (alt, base) = (0.905 * side, 0.85 * side);
    let rot = |v: Vec2| Vec2::new(v.x * angle.cos() - v.y * angle.sin(), v.x * angle.sin() + v.y * angle.cos());
    let tri = |k: f32, c: Color, ui: &mut Painter| {
        let (a, b, d) = (at + rot(Vec2::new(0.0, -0.62 * alt * k)), at + rot(Vec2::new(base * 0.5 * k, 0.38 * alt * k)), at + rot(Vec2::new(-base * 0.5 * k, 0.38 * alt * k)));
        ui.tri(a, b, d, c, c, c);
    };
    tri(1.5, Color::rgba(10, 10, 10, 0.9), ui);
    if ring {
        tri(1.25, Color::WHITE, ui);
    }
    tri(1.0, fill, ui);
}

/// What a vehicle looks like on the map: a road vehicle (bus, trolleybus), a tram, a metro train, a train.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Icon {
    Road,
    Tram,
    Metro,
    Train,
}

/// A bus, trolleybus, tram, metro train or train on the map as a pictogram, not a dot, turned to the way it
/// heads (`heading`: degrees clockwise from north), a dark edge round it, a windscreen where it can be driven
/// from. A road vehicle has a cut front; a tram is long and cut at both ends, with a joint in the middle; a metro
/// train is longer, square at both ends with a wide windscreen; a train is the longest, narrow, with a pointed nose.
/// `min_px` keeps it readable from far away; the line is written beside it.
pub(super) fn vehicle_icon(p: &mut Painter, at: Vec3, heading: f64, color: Color, icon: Icon, min_px: f32) {
    let h = heading.to_radians() as f32;
    let (sn, cs) = h.sin_cos();
    // local: x to the right, y forward; turned onto the world (x east, y north)
    let turn = |v: Vec2| Vec2::new(v.x * cs + v.y * sn, -v.x * sn + v.y * cs);
    let (body, glass): (&[(f32, f32)], &[&[(f32, f32)]]) = match icon {
        Icon::Road => (&[(-0.34, -1.0), (0.34, -1.0), (0.34, 0.55), (0.2, 1.0), (-0.2, 1.0), (-0.34, 0.55)], &[&[(-0.24, 0.5), (0.24, 0.5), (0.15, 0.8), (-0.15, 0.8)]]),
        Icon::Tram => (
            &[(-0.2, -1.6), (0.2, -1.6), (0.34, -1.25), (0.34, 1.25), (0.2, 1.6), (-0.2, 1.6), (-0.34, 1.25), (-0.34, -1.25)],
            &[&[(-0.22, 1.1), (0.22, 1.1), (0.14, 1.4), (-0.14, 1.4)], &[(-0.22, -1.1), (0.22, -1.1), (0.14, -1.4), (-0.14, -1.4)]],
        ),
        Icon::Metro => (
            &[(-0.26, -2.0), (0.26, -2.0), (0.3, -1.85), (0.3, 1.85), (0.26, 2.0), (-0.26, 2.0), (-0.3, 1.85), (-0.3, -1.85)],
            &[&[(-0.22, 1.62), (0.22, 1.62), (0.22, 1.85), (-0.22, 1.85)], &[(-0.22, -1.62), (0.22, -1.62), (0.22, -1.85), (-0.22, -1.85)]],
        ),
        Icon::Train => (&[(-0.28, -2.3), (0.28, -2.3), (0.28, 1.6), (0.0, 2.4), (-0.28, 1.6)], &[&[(-0.16, 1.55), (0.16, 1.55), (0.0, 2.05)]]),
    };
    let shape = |k: f32| -> Vec<Vec2> { body.iter().map(|&(x, y)| turn(Vec2::new(x * k, y * k))).collect() };
    let (m, px) = (6.0, min_px);
    p.world_shape(at, &shape(1.28), m, px * 1.28, Color::rgba(8, 8, 8, 0.92));
    p.world_shape(at, &shape(1.0), m, px, color);
    for g in glass {
        let g: Vec<Vec2> = g.iter().map(|&(x, y)| turn(Vec2::new(x, y))).collect();
        p.world_shape(at, &g, m, px, Color::rgba(240, 244, 248, 0.9));
    }
    if icon == Icon::Tram {
        // the joint of the two sections
        let joint: Vec<Vec2> = [(-0.34, -0.06), (0.34, -0.06), (0.34, 0.06), (-0.34, 0.06)].iter().map(|&(x, y)| turn(Vec2::new(x, y))).collect();
        p.world_shape(at, &joint, m, px, Color::rgba(8, 8, 8, 0.8));
    }
}

/// The route by how busy its roads are: empty, light, busy, heavy, jammed.
pub(super) const LEVEL: [Color; 5] = [
    Color::rgba(46, 116, 240, 1.0),
    Color::rgba(56, 178, 86, 1.0),
    Color::rgba(236, 192, 40, 1.0),
    Color::rgba(224, 56, 44, 1.0),
    Color::rgba(122, 16, 22, 1.0),
];
/// The arrows on it, in a colour that stands out from each.
pub(super) const ARROW: [Color; 5] = [
    Color::rgba(236, 244, 255, 1.0),
    Color::rgba(12, 66, 28, 1.0),
    Color::rgba(92, 58, 0, 1.0),
    Color::rgba(150, 240, 150, 1.0),
    Color::rgba(255, 206, 80, 1.0),
];
/// The part of the route already driven (city map).
pub(super) const DRIVEN: Color = Color::rgba(62, 70, 86, 1.0);
pub(super) const STREET: Color = Color::rgba(178, 178, 178, 1.0);

/// How busy a road is (0 empty … 4 jammed) from its congestion score.
pub(super) fn level(score: f32) -> usize {
    match score {
        s if s < 0.12 => 0,
        s if s < 0.40 => 1,
        s if s < 0.60 => 2,
        s if s < 0.80 => 3,
        _ => 4,
    }
}
pub(super) const TEXT: Color = Color::rgba(235, 235, 235, 1.0);
// (the second texts - units, the day, the times - bright enough to read on a lit cab)
pub(super) const TEXT_DIM: Color = Color::rgba(178, 178, 178, 1.0);
pub(super) const LATE: Color = Color::rgba(235, 85, 70, 1.0);
pub(super) const EARLY: Color = Color::rgba(90, 160, 240, 1.0);
pub(super) const ON_TIME: Color = Color::rgba(110, 200, 120, 1.0);
pub(super) const WARN: Color = Color::rgba(235, 170, 60, 1.0);
pub(super) const STOP_REQUEST: Color = Color::hex(0xF0A030);

pub(crate) fn stop_requested(vehicle: &omsi_sim::vehicle::VehicleInstance) -> bool {
    ["haltewunsch", "haltewunschlampe"].iter().any(|name| vehicle.var(name).is_some_and(|v| v > 0.5))
}

pub(super) fn stop_request_icon(ui: &mut Painter, atlas: &mut Atlas, requested: bool, mut row: Rect, scale: f32) -> Rect {
    if requested {
        let size = 32.0 * scale;
        ui.icon(atlas, "stop_request", Vec2::new(row.right() - size * 0.5, row.center().y), size, STOP_REQUEST);
        row.w -= size + 6.0 * scale;
    }
    row
}

/// An arrow at `at` pointing `angle` (radians, clockwise from up on the screen), `k` px from
/// its middle to its tip, on a dark outline `grow` times its size: the bus and the players.
pub(super) fn arrow(ui: &mut Painter, at: Vec2, angle: f32, k: f32, grow: f32, dark: Color, fill: Color) {
    let rot = |v: Vec2| Vec2::new(v.x * angle.cos() - v.y * angle.sin(), v.x * angle.sin() + v.y * angle.cos());
    let tip = at + rot(Vec2::new(0.0, -1.0) * k);
    let l = at + rot(Vec2::new(-0.7, 0.8) * k);
    let m = at + rot(Vec2::new(0.0, 0.4) * k);
    let r = at + rot(Vec2::new(0.7, 0.8) * k);
    let g = |p: Vec2| at + (p - at) * grow;
    ui.tri(g(tip), g(l), g(m), dark, dark, dark);
    ui.tri(g(tip), g(m), g(r), dark, dark, dark);
    ui.tri(tip, l, m, fill, fill, fill);
    ui.tri(tip, m, r, fill, fill, fill);
}

pub(super) fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// Stop positions stay on the route. Move only their labels, or omit a label if none of
/// the nearby positions fits. Call in route order so the next stop has first choice.
pub(super) fn stop_label_rect(p: Vec2, width: f32, s: f32, win: Rect, taken: &[Rect]) -> Option<Rect> {
    let h = 20.0 * s;
    let gap = 10.0 * s;
    let positions = [
        Vec2::new(p.x + gap, p.y - h * 0.5),
        Vec2::new(p.x - gap - width, p.y - h * 0.5),
        Vec2::new(p.x + gap, p.y - h - gap),
        Vec2::new(p.x + gap, p.y + gap),
        Vec2::new(p.x - gap - width, p.y - h - gap),
        Vec2::new(p.x - gap - width, p.y + gap),
    ];
    positions.into_iter().map(|q| Rect::new(q.x, q.y, width, h)).find(|r| {
        r.x >= 8.0 * s && r.right() <= win.right() - 8.0 * s && r.y >= 50.0 * s && r.bottom() <= win.bottom() - 8.0 * s
            && !taken.iter().any(|t| rects_overlap(t, r))
    })
}

pub(super) fn spaced_markers(points: impl IntoIterator<Item = (usize, Vec2)>, gap: f32) -> Vec<(usize, Vec2)> {
    let mut kept: Vec<(usize, Vec2)> = Vec::new();
    for (k, p) in points {
        if kept.iter().all(|(_, q)| p.distance(*q) >= gap) { kept.push((k, p)); }
    }
    kept
}
