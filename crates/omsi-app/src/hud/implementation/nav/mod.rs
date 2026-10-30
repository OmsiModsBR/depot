//! The route navigator: a small tilted 3D map in a corner of the screen, after the Route
//! Advisor of Euro Truck Simulator 2, made for a bus driver's duty.
//!
//! * The map turns with the bus and zooms out with speed; the camera looks over the bus
//!   from behind and above, so the road ahead fills the picture.
//! * Roads are drawn from the traffic network's lanes (dark casing, grey surface); the
//!   trip's route from the first stop to the last lies on them with arrows, coloured by
//!   how busy each stretch is (blue empty, green, yellow, red, dark red jammed), and its
//!   stops as markers (the last is a chequered flag).
//! * Leaving the route is noticed after a moment: a way back is searched on the lanes
//!   (Dijkstra, towards any lane of the route still ahead) and drawn instead - the route
//!   is recalculated as often as the driver goes wrong.
//! * Traffic: every AI vehicle near the bus is on the map, and roads whose cars crawl or
//!   stand are tinted amber or deep red (smoothed over seconds, so one car at a red light
//!   is no jam); the route itself takes the colour where it runs into one, and the header
//!   says how long it costs.
//! * The header: speed, the line, passengers aboard, day and time; the next stop with its
//!   distance and whether the bus is early or late; the speed limit sign and signals ahead
//!   on the map.
//!
//! The parts: [`minimap`] (the corner map and its header), [`city`] (the city map window),
//! [`route`] (the route on the lanes and the way back to it), [`geometry`] (roads from the
//! traffic network), [`streets`] (street names along them), [`lines`] (the bus lines panel)
//! and [`draw`] (the markers and icons both maps share). The navigator's objects (`Navigator`,
//! `NavFrame`, `CityMap`, the colours) are in `hud::domain::nav`.
//!
//! Everything is drawn with `omsi-ui` into a texture of its own (4x MSAA) that the game
//! shows as a premultiplied overlay. Roads are built once per area and kept on the GPU;
//! their width is in metres near the camera and in pixels far away, so zooming rebuilds
//! nothing.

use glam::{DMat3, DVec2, DVec3, Mat4, Vec2, Vec3};

pub(crate) mod city;
pub(crate) mod draw;
pub(crate) mod geometry;
pub(crate) mod lines;
pub(crate) mod minimap;
mod route;
mod streets;

use super::super::domain::nav::*;
pub(crate) use draw::stop_requested;
pub(crate) use geometry::{city_roads, simplify, MapRoad};

fn words(lang: &str) -> Words {
    match lang.to_ascii_uppercase().as_str() {
        "DEU" | "DE" | "GER" => Words {
            kmh: "km/h",
            days: ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
            off_route: "Abseits der Route",
            rerouting: "Route wird neu berechnet",
            recalculated: "Route neu berechnet",
            jam: "Stau",
            slow: "Zähfließend",
            no_duty: "Freie Fahrt",
            last_stop: "Endhaltestelle",
            on_time: "pünktlich",
        },
        "FRA" | "FR" => Words {
            kmh: "km/h",
            days: ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"],
            off_route: "Hors itinéraire",
            rerouting: "Recalcul de l'itinéraire",
            recalculated: "Itinéraire recalculé",
            jam: "Bouchon",
            slow: "Ralentissement",
            no_duty: "Conduite libre",
            last_stop: "Terminus",
            on_time: "à l'heure",
        },
        "RUS" | "RU" => Words {
            kmh: "км/ч",
            days: ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"],
            off_route: "Вне маршрута",
            rerouting: "Перестроение маршрута",
            recalculated: "Маршрут перестроен",
            jam: "Пробка",
            slow: "Затруднено",
            no_duty: "Свободная езда",
            last_stop: "Конечная",
            on_time: "по графику",
        },
        "PTB" | "PT" | "POR" | "BRA" => Words {
            kmh: "km/h",
            days: ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"],
            off_route: "Fora da rota",
            rerouting: "Recalculando a rota",
            recalculated: "Rota recalculada",
            jam: "Congestionamento",
            slow: "Trânsito lento",
            no_duty: "Direção livre",
            last_stop: "Ponto final",
            on_time: "no horário",
        },
        _ => Words {
            kmh: "km/h",
            days: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
            off_route: "Off route",
            rerouting: "Recalculating route",
            recalculated: "Route recalculated",
            jam: "Traffic jam",
            slow: "Slow traffic",
            no_duty: "Free drive",
            last_stop: "Final stop",
            on_time: "on time",
        },
    }
}

/// The duty as the navigator shows it: line, terminus, the stops from the next one on,
/// and the current trip (a key that changes with it, and its name).
#[allow(clippy::type_complexity)]
pub fn duty_parts(
    duty: Option<&crate::schedule::PlayerDuty>,
) -> (
    Option<String>,
    Option<String>,
    Vec<NavStop>,
    Option<(String, String)>,
) {
    let Some(d) = duty else {
        return (None, None, Vec::new(), None);
    };
    let Some(trip) = d.trips.get(d.trip_index) else {
        return (None, None, Vec::new(), None);
    };
    let line = if trip.line.trim().is_empty() {
        d.line.trim()
    } else {
        trip.line.trim()
    };
    let stops = trip
        .stops
        .iter()
        .skip(d.next_stop)
        .filter(|s| s.stops)
        .map(|s| NavStop {
            object_id: s.object_id,
            position: s.position.unwrap_or(DVec3::ZERO),
            name: s.name.clone(),
            arrival: s.arr,
        })
        .collect();
    (
        Some(line.to_string()),
        Some(trip.terminus.clone()),
        stops,
        Some((format!("{}/{}", d.trip_index, trip.name), trip.name.clone())),
    )
}

/// Screen position of a world point (relative to the anchor) in target pixels.
pub fn project(vp: Mat4, viewport: [f32; 4], p: Vec3) -> Option<Vec2> {
    let c = vp * p.extend(1.0);
    if c.w <= 0.1 {
        return None;
    }
    let n = c.truncate() / c.w;
    Some(Vec2::new(
        viewport[0] + (n.x * 0.5 + 0.5) * viewport[2],
        viewport[1] + (0.5 - n.y * 0.5) * viewport[3],
    ))
}

fn angle_diff(a: f64, b: f64) -> f64 {
    let mut d = (b - a) % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d < -180.0 {
        d += 360.0;
    }
    d
}

fn ease(dt: f32, tau: f32) -> f64 {
    (1.0 - (-dt / tau.max(1e-3)).exp()) as f64
}

/// A `navigator_corner` of the form `at x,y` (where the navigator was dragged to, #940).
pub fn placed_at(corner: &str) -> Option<[f32; 2]> {
    let (x, y) = corner.trim().strip_prefix("at")?.trim().split_once(',')?;
    let (x, y) = (x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?);
    (x.is_finite() && y.is_finite()).then(|| [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)])
}

/// The GPS is a separate render texture composed after the scene's post AA.
fn map_samples(format: wgpu::TextureFormat) -> u32 {
    if format
        .guaranteed_format_features(wgpu::Features::empty())
        .flags
        .sample_count_supported(4)
    {
        4
    } else {
        1
    }
}

impl<'a> NavFrame<'a> {
    fn clone_ref(&self) -> NavFrame<'a> {
        NavFrame {
            traffic: self.traffic,
            players: self.players.clone(),
            bus: self.bus,
            heading: self.heading,
            focus: self.focus,
            speed_kmh: self.speed_kmh,
            outside_temp: self.outside_temp,
            inside_temp: self.inside_temp,
            line: self.line.clone(),
            terminus: self.terminus.clone(),
            stops: self.stops.clone(),
            delay: self.delay,
            passengers: self.passengers,
            time: self.time,
            weekday: self.weekday,
            date: self.date,
            car_line: self.car_line,
            language: self.language,
            screen: self.screen,
            ui_scale: self.ui_scale,
            follow_window: self.follow_window,
            dt: self.dt,
            stop_requested: self.stop_requested,
            info_rect: self.info_rect,
        }
    }
}

/// Rotation of a 2D direction by a compass heading (for tests).
#[allow(dead_code)]
pub fn heading_vec(h: f64) -> DVec2 {
    let m = DMat3::from_rotation_z(-h.to_radians());
    m.transform_vector2(DVec2::Y)
}
