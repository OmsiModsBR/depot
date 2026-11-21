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
        ..Default::default()
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
        ..Default::default()
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

// ---------------------------------------------------------------------------------------
// The five conformance vehicles (spec 002, `contracts/test-vehicles.md`)
//
// Synthetic, plausible for their class, copied from no mod. Each section has its `[cog]`
// 0.3-0.8 m forward of the origin, so the static share about the origin (Classic) and about
// `[cog]` (Depot) differ measurably, and its `[rot_pnt_long]` as a `.bus` file gives it:
// the turning axle's place less the `[cog]` y (the turning line is counted from `[cog]`).

/// One axle of a conformance vehicle: `spring` kN/m and `max_force` kN per wheel.
fn conformance_axle(long: f32, spring: f32, max_force: f32, driven: bool) -> Axle {
    Axle {
        long,
        max_width: 2.4,
        min_width: 1.76,
        wheel_diameter: 0.94,
        spring,
        max_force,
        damper: 20.0,
        driven,
        inertia_inv: 0.0,
        ..Default::default()
    }
}

/// A section of a conformance vehicle: its axles as `(long, driven)`, the axle the turning
/// line goes through, its `[cog]` y and the places of its couplings.
struct Section<'a> {
    mass: f32,
    cog_y: f32,
    axles: &'a [(f32, bool)],
    turning_axle: f32,
    front: Option<f32>,
    back: Option<f32>,
}

fn conformance_section(s: &Section) -> Vehicle {
    let joint = |y: f32| Coupling {
        pos: [0.0, y, 0.315],
    };
    Vehicle {
        mass: s.mass,
        moment_of_inertia: [s.mass * 40.0, s.mass * 9.0, s.mass * 40.0],
        cog: Some([0.0, s.cog_y, 1.2]),
        cog_height: 1.2,
        rolling_resistance: s.mass * 90.0,
        rot_pnt_long: s.turning_axle - s.cog_y,
        inv_min_turn_radius: 0.12,
        axles: s
            .axles
            .iter()
            .map(|&(long, driven)| conformance_axle(long, 260.0, 110.0, driven))
            .collect(),
        coupling_front: s.front.map(joint),
        coupling_back: s.back.map(joint),
        coupling_front_character: s.front.map(|_| [52.5, -20.0, 20.0, 1.0]),
        ..Default::default()
    }
}

/// V1: a 4x2 solo bus, 12 m; steered front axle, driven rear axle.
pub fn v1_solo_4x2() -> Vec<Vehicle> {
    vec![conformance_section(&Section {
        mass: 10.2,
        cog_y: 0.5,
        axles: &[(3.2, false), (-2.6, true)],
        turning_axle: -2.6,
        front: None,
        back: None,
    })]
}

/// V2: a 6x2 rigid bus, 15 m; steered front axle, driven middle axle, steered tag axle.
pub fn v2_rigid_6x2() -> Vec<Vehicle> {
    vec![conformance_section(&Section {
        mass: 14.5,
        cog_y: 0.4,
        axles: &[(4.4, false), (-1.9, true), (-5.2, false)],
        turning_axle: -1.9,
        front: None,
        back: None,
    })]
}

/// V3: a 6x2 articulated bus, 18 m, a pusher: two axles in front, the driven one behind.
pub fn v3_articulated_6x2() -> Vec<Vehicle> {
    vec![
        conformance_section(&Section {
            mass: 8.5,
            cog_y: 0.5,
            axles: &[(3.4, false), (-2.6, false)],
            turning_axle: -2.6,
            front: None,
            back: Some(-4.3),
        }),
        conformance_section(&Section {
            mass: 7.5,
            cog_y: 0.4,
            axles: &[(-0.4, true)],
            turning_axle: -0.4,
            front: Some(4.2),
            back: None,
        }),
    ]
}

/// V4: a bi-articulated bus, 27 m, a puller (8x2): the front section's second axle drives.
pub fn v4_biarticulated() -> Vec<Vehicle> {
    vec![
        conformance_section(&Section {
            mass: 9.0,
            cog_y: 0.5,
            axles: &[(3.4, false), (-2.4, true)],
            turning_axle: -2.4,
            front: None,
            back: Some(-4.4),
        }),
        conformance_section(&Section {
            mass: 7.0,
            cog_y: 0.4,
            axles: &[(-0.3, false)],
            turning_axle: -0.3,
            front: Some(4.2),
            back: Some(-4.2),
        }),
        conformance_section(&Section {
            mass: 7.0,
            cog_y: 0.4,
            axles: &[(-0.3, false)],
            turning_axle: -0.3,
            front: Some(4.2),
            back: None,
        }),
    ]
}

/// V5: an articulated bus, 23 m, 8x2, a pusher: the rear section's first axle drives and
/// its fourth (last) axle steers.
pub fn v5_articulated_8x2() -> Vec<Vehicle> {
    vec![
        conformance_section(&Section {
            mass: 9.0,
            cog_y: 0.5,
            axles: &[(3.4, false), (-2.5, false)],
            turning_axle: -2.5,
            front: None,
            back: Some(-4.4),
        }),
        conformance_section(&Section {
            mass: 10.0,
            cog_y: 0.6,
            axles: &[(1.2, true), (-4.2, false)],
            turning_axle: 1.2,
            front: Some(5.6),
            back: None,
        }),
    ]
}

/// `defs` with axle `axle` of section `section` steerable (`achse_steer_angle` `lock_deg`,
/// `achse_steer_centring` `centring_deg_s`).
pub fn steerable(
    mut defs: Vec<Vehicle>,
    section: usize,
    axle: usize,
    lock_deg: f32,
    centring_deg_s: f32,
) -> Vec<Vehicle> {
    // (read from a `[newachse]` as a file gives it, so that the same scenario builds on a
    // commit that does not know the parameters: there the axle stays rigid, and the trace
    // diff shows what they change)
    let a = &defs[section].axles[axle];
    let text = format!(
        "[newachse]\nachse_long\n{}\nachse_maxwidth\n{}\nachse_minwidth\n{}\n\
         achse_raddurchmesser\n{}\nachse_feder\n{}\nachse_maxforce\n{}\n\
         achse_daempfer\n{}\nachse_antrieb\n{}\nachse_inertia_inv\n{}\n\
         achse_steer_angle\n{lock_deg}\nachse_steer_centring\n{centring_deg_s}\n",
        a.long,
        a.max_width,
        a.min_width,
        a.wheel_diameter,
        a.spring,
        a.max_force,
        a.damper,
        u8::from(a.driven),
        a.inertia_inv,
    );
    let parsed = Vehicle::parse(&omsi_cfg::CfgFile::from_str("steer.bus", &text));
    defs[section].axles[axle] = parsed.axles[0].clone();
    defs
}

/// V2 with its tag axle steerable: 15° each side, 200°/s (spec 004).
pub fn v2_tag_steer() -> Vec<Vehicle> {
    steerable(v2_rigid_6x2(), 0, 2, 15.0, 200.0)
}

/// V2 with the parameters on its front axle only, which the driver steers: no steerable
/// axle (spec 004).
pub fn v2_front_declared() -> Vec<Vehicle> {
    steerable(v2_rigid_6x2(), 0, 0, 15.0, 200.0)
}

/// The front section of an 8x2, a twin-steer: its second axle, ahead of the driven one,
/// steerable (12°, 200°/s), its fourth rigid (spec 004).
pub fn twin_steer_8x2() -> Vec<Vehicle> {
    steerable(
        vec![conformance_section(&Section {
            mass: 16.0,
            cog_y: 0.4,
            axles: &[(4.4, false), (2.6, false), (-2.0, true), (-3.4, false)],
            turning_axle: -2.7,
            front: None,
            back: None,
        })],
        0,
        1,
        12.0,
        200.0,
    )
}

/// A 15 m 6x2-4 whose `[cog]` stands 1.4 m ahead of its driven axle, its tag axle 1.5 m
/// behind that axle steerable by the 10.5° a turn about it asks at full lock (60°/s): at speed
/// in a turn its middle moves across faster than `GRIPPED_ACROSS` (spec 004).
pub fn tag_6x2_4_long() -> Vec<Vehicle> {
    let mut defs = vec![conformance_section(&Section {
        mass: 15.5,
        cog_y: -0.75,
        axles: &[(4.9, false), (-2.15, true), (-3.65, false)],
        turning_axle: -2.15,
        front: None,
        back: None,
    })];
    defs[0].inv_min_turn_radius = 0.124;
    steerable(defs, 0, 2, 10.5, 60.0)
}

/// The five vehicles by name, as the conformance tests are named after them.
pub fn conformance_vehicles() -> [(&'static str, Vec<Vehicle>); 5] {
    [
        ("v1_solo_4x2", v1_solo_4x2()),
        ("v2_rigid_6x2", v2_rigid_6x2()),
        ("v3_articulated_6x2", v3_articulated_6x2()),
        ("v4_biarticulated", v4_biarticulated()),
        ("v5_articulated_8x2", v5_articulated_8x2()),
    ]
}
