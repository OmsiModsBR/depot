use crate::support::scratch::ScratchPack;
use crate::vehicle::cases::coupling_test_type;
use glam::{DVec2, DVec3};
use omsi_script::{compile, CompileInput};
use omsi_sim::host::VehicleHost;
use omsi_sim::vehicle::AiFrame;
use omsi_sim::{Trolley, VehicleInstance};
use omsi_trolley::testing::{spline, tile};
use omsi_trolley::{build_tile_wires, declaration_from, parse_params, parse_registry, WireNetwork};
use std::sync::Arc;

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

/// An AI trolleybus weaving under a straight wire: the poles it shows each frame are the ones
/// for where it stands that frame. Its script copies a pole's angle as it reads it, and that
/// is the angle written for the frame, not the last frame's (the shoe, drawn a frame late,
/// stood off the wire by as far as the bus turned under it: at a switch taken at speed, a
/// hand's breadth).
#[test]
fn an_ai_trolleybus_shows_the_poles_of_the_frame_it_stands_in() {
    let pack = ScratchPack::new("sim-trolley-same-frame");
    let dir = pack.path();
    let (varlist, script) = (dir.join("vars.txt"), dir.join("poles.osc"));
    std::fs::write(&varlist, "shtanga_lev_rot\nseen\n").unwrap();
    std::fs::write(
        &script,
        "{frame_ai}\n(L.L.shtanga_lev_rot) (S.L.seen)\n{end}\n",
    )
    .unwrap();
    let program = compile(&CompileInput {
        varlists: vec![varlist],
        scripts: vec![script],
        ..Default::default()
    });
    assert!(program.errors.is_empty(), "{:?}", program.errors);
    let mut ty = coupling_test_type(None);
    Arc::get_mut(&mut ty).unwrap().program = Arc::new(program);
    let params = parse_params(PARAMS).unwrap();
    let decl = declaration_from(&parse_registry(REGISTRY)[0], &params).unwrap();
    let mut net = WireNetwork::default();
    net.insert_tile(build_tile_wires(
        (0, 0),
        DVec2::ZERO,
        &tile(
            vec![spline(
                1,
                "Splines\\wire_x.sli",
                [0.0, 0.0, 0.0],
                0.0,
                200.0,
                0.0,
            )],
            Vec::new(),
        ),
        &params,
    ));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(Default::default()));
    let mut t = Trolley::new(decl, 0, Arc::new(parking_lot::Mutex::new(net)));
    t.auto = true;
    v.trolley = Some(Box::new(t));
    let dt = 1.0 / 60.0;
    let mut y = 20.0;
    let mut frame = |v: &mut VehicleInstance, k: usize, speed: f32| {
        y += (speed * dt) as f64;
        v.position = DVec3::new(0.0, y, 0.0);
        v.heading = 12.0 * (k as f64 * 0.05).sin();
        v.update_ai(
            dt,
            &AiFrame {
                speed,
                odometer: y as f32,
                ..Default::default()
            },
        );
    };
    // standing: the poles go up by themselves
    for k in 0..300 {
        frame(&mut v, k, 0.0);
    }
    assert_eq!(
        v.var("shtanga_wire_present"),
        Some(1.0),
        "the poles went up"
    );
    let mut turned = 0.0f32;
    let mut last = v.var("shtanga_lev_rot").unwrap();
    for k in 300..600 {
        frame(&mut v, k, 14.0);
        let (seen, shown) = (v.var("seen").unwrap(), v.var("shtanga_lev_rot").unwrap());
        assert_eq!(
            v.var("shtanga_wire_present"),
            Some(1.0),
            "on the wire, frame {k}"
        );
        assert!(
            (seen - shown).abs() < 1e-4,
            "frame {k}: the script read {seen}, the frame's pole is at {shown}"
        );
        turned = turned.max((shown - last).abs());
        last = shown;
    }
    assert!(turned > 0.1, "the pole turned frame to frame ({turned})");
}
