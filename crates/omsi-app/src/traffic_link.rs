//! What the traffic is told about the player and the LAN buses.

use crate::app::App;
use crate::humans;
use crate::player::Player;
use crate::scene::World;
use crate::traffic;
use glam::DVec3;
use omsi_render::{Camera, Renderer, Scene};

/// Position, heading, half extents and speed of the player's vehicle for the AI's
/// obstacle checks.
pub(crate) fn player_outline(p: &Player) -> traffic::PlayerBox {
    vehicle_outline(&p.vehicle, p.vehicle.motion().speed)
}

/// `player_outline` of any vehicle moving at `speed` (m/s).
pub(crate) fn vehicle_outline(v: &omsi_sim::VehicleInstance, speed: f32) -> traffic::PlayerBox {
    let bb =
        v.ty.def
            .bounding_box
            .unwrap_or([2.5, 11.0, 3.0, 0.0, 0.0, 1.5]);
    let h = v.heading.to_radians();
    let centre = v.position
        + DVec3::new(
            (bb[3] as f64) * h.cos() + (bb[4] as f64) * h.sin(),
            -(bb[3] as f64) * h.sin() + (bb[4] as f64) * h.cos(),
            0.0,
        );
    (centre, v.heading, bb[1] * 0.5, bb[0] * 0.5, speed)
}

/// Everything of the player's besides the bus's own box that the traffic has to keep out
/// of: the rear sections of an articulated bus or a coupled trailer (the traffic saw only
/// the front section and drove into the back of a turning GN92), and the vehicles placed
/// by hand from the vehicle list, with their trailers - under ids of their own beside the
/// LAN players'.
pub(crate) fn own_outlines(
    player: Option<&Player>,
    placed: &[Player],
) -> Vec<(u32, traffic::PlayerBox)> {
    let mut out = Vec::new();
    let mut add = |v: &omsi_sim::VehicleInstance, base: u32, whole: bool| {
        let speed = v.motion().speed;
        if whole {
            out.push((base, vehicle_outline(v, speed)));
        }
        for (k, t) in v.trailers.iter().enumerate() {
            let Some(bb) = t.ty.def.bounding_box else {
                continue;
            };
            let h = t.heading.to_radians();
            let centre = t.position
                + DVec3::new(
                    (bb[3] as f64) * h.cos() + (bb[4] as f64) * h.sin(),
                    -(bb[3] as f64) * h.sin() + (bb[4] as f64) * h.cos(),
                    0.0,
                );
            out.push((
                base + 1 + k as u32,
                (centre, t.heading, bb[1] * 0.5, bb[0] * 0.5, speed),
            ));
        }
    };
    if let Some(p) = player {
        add(&p.vehicle, 0xFFFF_0000, false);
    }
    for (i, q) in placed.iter().enumerate() {
        add(&q.vehicle, 0xFFFE_0000 - (i as u32) * 16, true);
    }
    out
}

/// What the traffic needs to know every frame besides the time: where the player looks
/// from, the day of the week, who walks the footpaths, what hides what.
pub(crate) fn traffic_inputs(
    t: &mut traffic::Traffic,
    cam: Option<&Camera>,
    aspect: f64,
    extent: Option<(f64, f64)>,
    fog: f64,
    clock: &omsi_sim::SimClock,
    humans: Option<&humans::Humans>,
    player: Option<&Player>,
    render: &omsi_render::RenderOptions,
) {
    if let Some(c) = cam {
        t.viewer = Some(
            traffic::Viewer::new(c, aspect, fog)
                .with_extent(extent)
                .with_culling(render.min_obj_size, render.max_obj_dist),
        );
    }
    t.weekday = clock.weekday();
    t.walkers = humans.map(|h| h.strollers()).unwrap_or_default();
    t.people = humans.map(|h| h.on_foot()).unwrap_or_default();
    // the player's obstacle boxes follow the streamed tiles; without a player the world's
    // own are asked
    t.occluders = player.and_then(|p| p.vehicle.collision.clone());
}

/// Posts (`[crashmode_pole]`) the vehicle knocked over this frame: laid on the ground from
/// their foot in the direction they were hit.
pub(crate) fn lay_down_poles(
    world: &World,
    renderer: &Renderer,
    scene: &mut Scene,
    vehicle: &mut omsi_sim::VehicleInstance,
) {
    for (id, push) in std::mem::take(&mut vehicle.knocked_now) {
        if let Some(pos) = world.lay_down_pole(renderer, scene, id, push) {
            log::info!("knocked over post {id} at ({:.1}, {:.1})", pos.x, pos.y);
        }
    }
}

/// Where a vehicle stands and how big it is: the centre of its box (reaching back over an
/// articulated bus's rear sections), heading in degrees clockwise from north, length and width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VehicleBox {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub heading: f32,
    pub length: f32,
    pub width: f32,
}

/// The box of a vehicle: centre, heading, length, width.
pub(crate) fn vehicle_box(v: &omsi_sim::VehicleInstance, fallback: [f32; 6]) -> VehicleBox {
    let bb = v.ty.def.bounding_box.unwrap_or(fallback);
    let h = v.heading.to_radians();
    let (cx, cy) = (bb[3] as f64, bb[4] as f64);
    let c = v.position
        + DVec3::new(
            cx * h.cos() + cy * h.sin(),
            -cx * h.sin() + cy * h.cos(),
            0.0,
        );
    // an articulated bus reaches back over its rear sections
    let mut length = bb[1];
    let mut centre = c;
    for t in &v.trailers {
        if let Some(tb) = t.ty.def.bounding_box {
            let back = (t.position - v.position).truncate().length() as f32 + tb[1] * 0.5;
            let extra = (back - (bb[1] * 0.5 - bb[4])).max(0.0);
            length += extra;
            centre -= DVec3::new(h.sin(), h.cos(), 0.0) * (extra as f64 * 0.5);
        }
    }
    VehicleBox {
        x: centre.x,
        y: centre.y,
        z: v.position.z,
        heading: v.heading as f32,
        length,
        width: bb[0],
    }
}

/// What a vehicle's indicators show: 0 off, 1 left, 2 right, 3 hazard - the indicator
/// switch where the script has one, else the lamps (as they are lit just now).
pub(crate) fn indicator(v: &omsi_sim::VehicleInstance) -> u8 {
    let on = |n: &str| v.var(n).unwrap_or(0.0) > 0.5;
    if on("lights_sw_warnblinker") {
        3
    } else {
        match v.var("lights_sw_blinker") {
            Some(s) if (0.5..2.5).contains(&s) => s.round() as u8,
            _ => match (on("lights_blinker_l"), on("lights_blinker_r")) {
                (true, true) => 3,
                (true, false) => 1,
                (false, true) => 2,
                _ => 0,
            },
        }
    }
}

/// Take the current random AI traffic off the road. Timetable buses are kept, and the
/// configured random traffic target will populate the roads again normally.
pub(crate) fn clear_ai_traffic(app: &mut App) {
    if app.is_lan_client() {
        app.service_msg = Some((
            "In a LAN session only the host can clear AI traffic".into(),
            3.0,
        ));
        return;
    }
    if let (Some(t), Some(w), Some(r), Some(scene)) = (
        app.traffic.as_mut(),
        app.world.as_ref(),
        app.renderer.as_ref(),
        app.scene.as_mut(),
    ) {
        let removed = t.clear_random(w, r, scene);
        app.service_msg = Some((format!("{removed} AI vehicles taken off the road"), 3.0));
    }
}
