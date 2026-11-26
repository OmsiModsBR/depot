//! The petrol station: whether a bus stands in one of the map's `[petrolstation]` boxes,
//! where the pump, the wash and a repair without travel time are offered.

use crate::world::scene::World;

/// Does the vehicle's box overlap one of the loaded `[petrolstation]` objects? That is
/// OMSI's test for the pump, the wash and a repair without travel time.
pub(crate) fn at_petrol_station(world: &World, v: &omsi_sim::VehicleInstance) -> bool {
    petrol_station_test(world, v, true)
}

/// [`at_petrol_station`] without its debug lines: asked every frame (the Alt bar's wash and
/// refuel button).
pub(crate) fn on_petrol_station(world: &World, v: &omsi_sim::VehicleInstance) -> bool {
    petrol_station_test(world, v, false)
}

fn petrol_station_test(world: &World, v: &omsi_sim::VehicleInstance, log: bool) -> bool {
    let f = crate::gameplay::traffic_link::vehicle_box(v, [2.5, 11.5, 3.0, 0.0, 0.0, 1.5]);
    let me = omsi_sim::collision::Obb {
        center: glam::DVec2::new(f.x, f.y),
        half: glam::DVec2::new(f.width as f64 * 0.5, f.length as f64 * 0.5),
        heading: (f.heading as f64).to_radians(),
        z0: f.z - 3.0,
        z1: f.z + 4.0,
        velocity: glam::DVec2::ZERO,
        mass: 0.0,
        pole: None,
        id: -1,
    };
    let stations = world.petrol_stations.lock();
    if log && omsi_cfg::flags::DEBUG_SERVICES.is_set() {
        for p in stations.iter() {
            log::info!(
                "petrol station box at ({:.1}, {:.1}) {:.1} x {:.1} m: bus {:.1} m away",
                p.center.x,
                p.center.y,
                p.half.x * 2.0,
                p.half.y * 2.0,
                me.separation(p)
            );
        }
    }
    stations.iter().any(|p| me.separation(p) < 0.0)
}
