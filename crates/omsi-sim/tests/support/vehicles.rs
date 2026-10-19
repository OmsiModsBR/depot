//! The synthetic vehicles of the crate's tests.

use omsi_vehicle::vehicle::Coupling;
use omsi_vehicle::{Axle, Vehicle};

/// A 12 m solo bus on two axles.
pub fn bus() -> Vehicle {
    let axle = |long: f32, spring: f32, max_force: f32, driven: bool| Axle {
        long,
        max_width: 2.4,
        min_width: 1.76,
        wheel_diameter: 0.94,
        spring,
        max_force,
        damper: 20.0,
        driven,
        inertia_inv: 0.0,
    };
    Vehicle {
        mass: 10.2,
        moment_of_inertia: [600.0, 90.0, 400.0],
        cog_height: 1.2,
        rolling_resistance: 1000.0,
        rot_pnt_long: -2.7,
        inv_min_turn_radius: 0.13,
        axles: vec![
            axle(3.238, 240.0, 90.0, false),
            axle(-2.637, 280.0, 116.0, true),
        ],
        bounding_box: Some([2.472, 11.663, 2.491, 0.0, 0.001, 1.622]),
        ..Default::default()
    }
}

/// The stock GN92's two sections: a pusher, whose front section drives no wheel;
/// `puller` drives the front section's rear axle instead.
pub fn gn92(puller: bool) -> (Vehicle, Vehicle) {
    let axle = |long: f32, max_width: f32, min_width: f32, driven: bool| Axle {
        long,
        max_width,
        min_width,
        wheel_diameter: 0.94,
        spring: 280.0,
        max_force: 116.0,
        damper: 20.0,
        driven,
        inertia_inv: 0.0,
    };
    let front = Vehicle {
        mass: 8.0,
        moment_of_inertia: [300.0, 80.0, 300.0],
        cog_height: 1.2,
        rolling_resistance: 900.0,
        rot_pnt_long: -2.7,
        inv_min_turn_radius: 0.13,
        axles: vec![
            Axle {
                spring: 240.0,
                max_force: 90.0,
                ..axle(3.238, 2.25, 1.76, false)
            },
            axle(-2.637, 2.4, 1.2, puller),
        ],
        coupling_back: Some(Coupling {
            pos: [0.0, -4.331, 0.315],
        }),
        ..Default::default()
    };
    let rear = Vehicle {
        mass: 7.2,
        moment_of_inertia: [150.0, 40.0, 150.0],
        cog_height: 1.2,
        rolling_resistance: 500.0,
        rot_pnt_long: -0.387,
        axles: vec![axle(-0.387, 2.4, 1.2, !puller)],
        coupling_front: Some(Coupling {
            pos: [0.0, 4.169, 0.315],
        }),
        coupling_front_character: Some([52.5, -20.0, 20.0, 1.0]),
        ..Default::default()
    };
    (front, rear)
}
