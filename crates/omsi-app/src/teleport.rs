//! Putting the player's bus somewhere else: a place picked on the map, or back on the ground
//! where it last stood after falling through the world.

use crate::core::app::App;

/// Our bus put at `at` facing `heading` (stopped), with the walker back at its wheel.
pub(crate) fn teleport(app: &mut App, at: glam::DVec3, heading: f64) {
    if app.on_foot.is_some() {
        app.back_to_bus();
    }
    // on the level at the height asked for (a car park under a building, a road under a
    // bridge), else the highest ground there (a place picked on the map, at no height)
    let ground = app.world.as_ref().and_then(|w| {
        let near = (at.z != 0.0)
            .then(|| {
                crate::scene::drive_probe(&w.terrains, &w.surfaces, at.x, at.y, at.z + 1.5).below
            })
            .flatten()
            .filter(|b| (at.z - b).abs() < 3.0);
        near.or_else(|| w.walk_height(at.x, at.y))
    });
    let z = ground.unwrap_or(at.z);
    if let Some(p) = app.player.as_mut() {
        // (as a joining player's bus is moved off an occupied spawn: `lan::clear_spawn`)
        let origin = glam::DVec3::new(at.x, at.y, z);
        p.vehicle.teleport(origin, heading);
        // (settled on the ground there; where its tiles are still to be read - a street far
        // off picked on the map - it waits at the street's height for them, as the frame
        // holds a bus with no ground under it, instead of dropping through first)
        if ground.is_some() {
            for _ in 0..3 {
                p.vehicle.update(1.0 / 30.0);
            }
        }
        log::info!(
            "teleported to ({:.1}, {:.1}) heading {heading:.0}",
            at.x,
            at.y
        );
    }
}

/// The bus fallen through the world (a hole in the ground, a tile that was not there yet,
/// a mod's road without a surface) fell for ever. Where it last stood on the ground is
/// kept every second; a bus more than 8 m under the ground there is - or 40 m under where
/// it last stood, where there is no ground - is put back there.
pub(crate) fn guard_fall(app: &mut App, dt: f32) {
    let Some(p) = app.player.as_ref() else { return };
    let at = p.vehicle.position;
    // the ground under the bus (the face at or below it: on a car park's lower level the
    // building's roof is not its ground - measured from the roof, a bus driving under it
    // had fallen through the world and was put up there), and the highest there is
    let (under, ground) = match app.world.as_ref() {
        Some(w) => (
            crate::scene::drive_probe(&w.terrains, &w.surfaces, at.x, at.y, at.z + 1.5).below,
            w.walk_height(at.x, at.y),
        ),
        None => (None, None),
    };
    app.safe_age += dt;
    let fallen = match (under, ground) {
        (Some(_), _) => false,
        (None, Some(g)) => at.z < g - 8.0,
        (None, None) => app.safe_pose.map(|s| at.z < s.0.z - 40.0).unwrap_or(false),
    };
    if fallen {
        if let Some((pos, heading)) = app.safe_pose {
            log::warn!("the bus fell through the world at ({:.1}, {:.1}, {:.1}): put back at ({:.1}, {:.1})", at.x, at.y, at.z, pos.x, pos.y);
            teleport(app, pos, heading);
            app.service_msg = Some((
                "The bus fell through the ground: it was put back where it last stood".into(),
                5.0,
            ));
        }
        return;
    }
    if app.safe_age >= 1.0 {
        if let Some(g) = under.filter(|g| (at.z - g).abs() < 2.5) {
            app.safe_age = 0.0;
            app.safe_pose = Some((glam::DVec3::new(at.x, at.y, g), p.vehicle.heading));
        }
    }
}
