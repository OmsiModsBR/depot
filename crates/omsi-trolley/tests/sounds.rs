//! The wire's sounds and sparks: switches passed both ways, crossings, sparks under the shoes
//! and the spark of a pole leaving the wire.

use glam::{DMat3, DVec2, DVec3};
use omsi_trolley::testing::{object, spline, tile};
use omsi_trolley::{
    build_tile_wires, declaration_from, parse_params, parse_registry, BodyPose, ConventionParams,
    PoleInputs, PolePhase, PoleSim, TrolleyDeclaration, WireNetwork, WireSound, WireSoundKind,
    WireWeather,
};

const WIRE: &str = "Splines\\Net\\wire_x.sli";
const Y: &str = "Sceneryobjects\\Net\\Troll_strelka_out_02_Y.sco";
const CROSSING: &str = "Sceneryobjects\\Net\\Troll_strelka_OnKrest.sco";

const PARAMS: &str = "\
[global]
spline_keywords=wire_x
wire_height=5.8
wire_x_left=-0.255
wire_x_right=0.255
manual_capture_distance=0.45
lost_reset_frames=90
max_pitch=55
switch_throttle_threshold=0.05
[physics]
pole_raise_duration=1.5
pole_lower_duration=1.0
spring_nominal_force_n=82
spring_reference_elevation_deg=25
spring_gain_n_per_deg=1.15
contact_force_max_n=160
complex_speed_limit_kmh=25
complex_overspeed_grace=0.16
contact_force_dewire_n=12
contact_force_dewire_time=0.18
contact_reach_release_error_m=0.10
contact_reach_release_time=0.12
dewire_yaw_limit_deg=87
dewire_free_rise_deg_s=42
dewire_catcher_delay=0.55
dewire_catcher_lower_deg_s=34
dewire_lateral_force_n=260
dewire_lateral_force_time=0.20
[sound]
switch_left_merg_chance=1
switch_left_sep_chance=1
switch_right_merg_chance=1
switch_right_sep_chance=1
cross_chance=1
cross_trigger_radius=0.85
min_distance=1.5
max_distance=35
volume_db=-8
interior_source_x=0
interior_source_y=2.7
interior_source_z=0
interior_cab_gain_db=-8
interior_salon_gain_db=-2
spark_volume_db=-3
dewire_volume_db=-3
spark_sample_gain=8
[sparks]
enabled=1
min_speed_kmh=1
min_throttle=0.05
interval_min_ms=8000
interval_max_ms=22000
duration_min_ms=28
duration_max_ms=70
rain_frequency_gain=2.5
snow_frequency_gain=5
ice_frequency_gain=8
";

const REGISTRY: &str = "\
[vehicle:rigid]
bus_file=Vehicles\\T\\rigid.bus
pole_base_left_x=-0.25
pole_base_right_x=0.25
pole_base_forward=2.24
pole_base_height=3.455
contact_left_x=-0.255
contact_right_x=0.255
contact_forward=-3.93
contact_height=5.41
";

fn params() -> ConventionParams {
    parse_params(PARAMS).unwrap()
}

fn decl() -> TrolleyDeclaration {
    declaration_from(&parse_registry(REGISTRY)[0], &params()).unwrap()
}

fn net(
    splines: Vec<omsi_map::tile::MapSpline>,
    objects: Vec<omsi_map::tile::MapObject>,
) -> WireNetwork {
    let mut n = WireNetwork::default();
    n.insert_tile(build_tile_wires(
        (0, 0),
        DVec2::ZERO,
        &tile(splines, objects),
        &params(),
    ));
    n
}

fn body(x: f64, y: f64, heading: f64) -> BodyPose {
    BodyPose {
        position: DVec3::new(x, y, 0.0),
        rotation: DMat3::from_rotation_z(-heading.to_radians()),
    }
}

/// What a drive along `path` (straight pieces between the points) heard, and how many
/// frames a spark flashed.
struct Drive {
    sounds: Vec<WireSound>,
    spark_frames: usize,
}

fn drive(
    n: &WireNetwork,
    path: &[(f64, f64)],
    demand: f64,
    kmh: f64,
    weather: WireWeather,
) -> Drive {
    let d = decl();
    let mut sim = PoleSim::default();
    let heading = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0).atan2(b.1 - a.1).to_degrees();
    let at = body(path[0].0, path[0].1, heading(path[0], path[1]));
    let press = PoleInputs {
        grab: [true, true],
        ..Default::default()
    };
    sim.step(&d, n, &at, press, 1.0 / 60.0);
    for _ in 0..100 {
        sim.step(&d, n, &at, PoleInputs::default(), 1.0 / 60.0);
    }
    assert!(
        sim.poles.iter().all(|p| p.phase == PolePhase::OnWire),
        "raised"
    );
    let mut out = Drive {
        sounds: Vec::new(),
        spark_frames: 0,
    };
    let step = kmh / 3.6 / 60.0;
    for w in path.windows(2) {
        let (a, b) = (DVec2::from(w[0]), DVec2::from(w[1]));
        let h = heading(w[0], w[1]);
        let len = a.distance(b);
        let mut s = 0.0;
        while s < len {
            s = (s + step).min(len);
            let p = a.lerp(b, s / len);
            let o = sim.step(
                &d,
                n,
                &body(p.x, p.y, h),
                PoleInputs {
                    switch_demand: demand,
                    speed_kmh: kmh,
                    weather,
                    ..Default::default()
                },
                1.0 / 60.0,
            );
            out.sounds.extend(sim.sounds.iter().copied());
            out.spark_frames += o.poles.iter().filter(|p| p.spark).count();
        }
    }
    out
}

/// The sounds heard, without the sparks.
fn kinds(d: &Drive) -> Vec<WireSoundKind> {
    d.sounds
        .iter()
        .map(|s| s.kind)
        .filter(|k| *k != WireSoundKind::Spark)
        .collect()
}

/// A Y switch at y = 51.4: the line ends at y = 50, the left branch goes on from (-0.5,
/// 52.85) 10 degrees left, the right one from (0.5, 52.85) 10 degrees right.
fn y_switch() -> WireNetwork {
    net(
        vec![
            spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 50.0, 0.0),
            spline(2, WIRE, [-0.5, 52.85, 0.0], -10.0, 40.0, 0.0),
            spline(3, WIRE, [0.5, 52.85, 0.0], 10.0, 40.0, 0.0),
        ],
        vec![object(9, Y, [0.0, 51.4, 0.0], 0.0)],
    )
}

#[test]
fn a_switch_left_for_a_way_sounds_once_on_that_side() {
    let n = y_switch();
    let path = [(0.0, 20.0), (0.0, 62.0)];
    let powered = drive(&n, &path, 1.0, 10.0, WireWeather::Dry);
    assert_eq!(
        kinds(&powered),
        [WireSoundKind::SwitchSeparate { left: true }]
    );
    let coasting = drive(&n, &path, 0.0, 10.0, WireWeather::Dry);
    assert_eq!(
        kinds(&coasting),
        [WireSoundKind::SwitchSeparate { left: false }]
    );
    let at = coasting.sounds[0].position;
    assert!(
        (at.y - 52.85).abs() < 0.5 && at.z > 5.0,
        "at the switch: {at}"
    );
}

#[test]
fn a_switch_come_in_to_from_a_way_sounds_once_on_that_side() {
    let n = y_switch();
    // back down a branch, through the switch and along the line
    let off = 32.15 * 10f64.to_radians().tan();
    for (x, left) in [(-0.5f64, true), (0.5, false)] {
        let d = drive(
            &n,
            &[
                (x + x.signum() * off, 85.0),
                (x, 52.85),
                (0.0, 50.0),
                (0.0, 20.0),
            ],
            0.0,
            10.0,
            WireWeather::Dry,
        );
        assert_eq!(kinds(&d), [WireSoundKind::SwitchMerge { left }]);
    }
}

#[test]
fn a_crossing_sounds_once_as_the_shoes_pass_it() {
    let n = net(
        vec![spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 100.0, 0.0)],
        vec![object(9, CROSSING, [0.0, 50.0, 0.0], 0.0)],
    );
    let d = drive(&n, &[(0.0, 20.0), (0.0, 80.0)], 0.0, 10.0, WireWeather::Dry);
    assert_eq!(kinds(&d), [WireSoundKind::Cross]);
}

#[test]
fn shoes_spark_now_and_then_under_power_and_more_often_in_the_rain() {
    let n = net(
        vec![spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 700.0, 0.0)],
        vec![],
    );
    let path = [(0.0, 20.0), (0.0, 620.0)];
    let sparks = |d: &Drive| {
        d.sounds
            .iter()
            .filter(|s| s.kind == WireSoundKind::Spark)
            .count()
    };
    let dry = drive(&n, &path, 1.0, 30.0, WireWeather::Dry);
    let rain = drive(&n, &path, 1.0, 30.0, WireWeather::Rain);
    let coasting = drive(&n, &path, 0.0, 30.0, WireWeather::Dry);
    // 72 s: a spark of each shoe every 8 to 22 s, two and a half times as often in the rain
    assert!((4..=18).contains(&sparks(&dry)), "dry: {}", sparks(&dry));
    assert!(sparks(&rain) > sparks(&dry), "rain: {}", sparks(&rain));
    assert!(dry.spark_frames > 0, "the sparks flash");
    assert_eq!(sparks(&coasting), 0, "no spark without power");
    assert_eq!(coasting.spark_frames, 0);
}

#[test]
fn a_pole_leaving_the_wire_under_power_sparks() {
    let n = net(
        vec![spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 50.0, 0.0)],
        vec![],
    );
    let d = drive(&n, &[(0.0, 20.0), (0.0, 75.0)], 1.0, 10.0, WireWeather::Dry);
    assert!(
        d.sounds
            .iter()
            .any(|s| s.kind == WireSoundKind::DewireSpark),
        "{:?}",
        d.sounds
    );
}
