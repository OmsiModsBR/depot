//! The poles of a declared trolleybus on hand-made wire.

use glam::{DMat3, DVec2, DVec3};
use omsi_trolley::testing::{spline, tile};
use omsi_trolley::{
    build_tile_wires, declaration_from, find_declaration, parse_params, parse_registry, BodyPose,
    Convention, ConventionParams, DewireReason, PoleEvent, PoleInputs, PolePhase, PoleSim,
    TrolleyDeclaration, WireNetwork,
};

const WIRE: &str = "Splines\\Net\\wire_x.sli";

const PARAMS: &str = "\
[global]
spline_keywords=wire_x
wire_height=5.8
wire_x_left=-0.255
wire_x_right=0.255
manual_capture_distance=0.45
lost_reset_frames=90
max_pitch=55
[physics]
pole_raise_duration=1.5
pole_lower_duration=1.0
spring_nominal_force_n=82
spring_reference_elevation_deg=25
spring_gain_n_per_deg=1.15
contact_force_max_n=160
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

[vehicle:off]
enabled=0
bus_file=Vehicles\\T\\other.bus

[vehicle:artic]
bus_file=Vehicles\\T\\front.bus
pole_carrier_bus_file=Vehicles\\T\\rear.bus
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

/// A body at (x, y) on the ground heading `heading` degrees (0 north, clockwise).
fn body(x: f64, y: f64, heading: f64) -> BodyPose {
    BodyPose {
        position: DVec3::new(x, y, 0.0),
        rotation: DMat3::from_rotation_z(-heading.to_radians()),
    }
}

/// A straight wire from (0, 0) north, `length` metres, in spans of `span` metres.
fn straight(length: f64, span: f64) -> WireNetwork {
    let n = (length / span) as usize;
    let splines = (0..n)
        .map(|i| {
            spline(
                i as i64 + 1,
                WIRE,
                [0.0, i as f64 * span, 0.0],
                0.0,
                span,
                0.0,
            )
        })
        .collect();
    let mut net = WireNetwork::default();
    net.insert_tile(build_tile_wires(
        (0, 0),
        DVec2::ZERO,
        &tile(splines, vec![]),
        &params(),
    ));
    net
}

fn grab(sim: &mut PoleSim, d: &TrolleyDeclaration, net: &WireNetwork, at: &BodyPose) {
    sim.step(
        d,
        net,
        at,
        PoleInputs {
            grab: [true, true],
            reset: false,
            retract: false,
        },
        1.0 / 60.0,
    );
    sim.step(d, net, at, PoleInputs::default(), 1.0 / 60.0);
}

fn raise(sim: &mut PoleSim, d: &TrolleyDeclaration, net: &WireNetwork, at: &BodyPose) {
    grab(sim, d, net, at);
    for _ in 0..120 {
        sim.step(d, net, at, PoleInputs::default(), 1.0 / 60.0);
    }
}

#[test]
fn poles_start_on_their_hooks_in_the_modelled_pose() {
    let d = decl();
    let mut sim = PoleSim::default();
    let o = sim.step(
        &d,
        &straight(100.0, 50.0),
        &body(0.0, 20.0, 0.0),
        PoleInputs::default(),
        0.016,
    );
    assert!(!o.wire_present);
    for (i, p) in o.poles.iter().enumerate() {
        assert_eq!((p.yaw, p.pitch, p.force), (0.0, 0.0, 0.0));
        assert_eq!(p.shoe_pos, d.poles[i].rest_shoe);
    }
}

#[test]
fn raised_poles_rise_over_the_raise_time_and_press_on_their_own_wires() {
    let d = decl();
    let net = straight(100.0, 50.0);
    let at = body(0.0, 20.0, 0.0);
    let mut sim = PoleSim::default();
    grab(&mut sim, &d, &net, &at);
    assert_eq!(sim.poles[0].phase, PolePhase::Rising);
    let half = sim.step(&d, &net, &at, PoleInputs::default(), 0.7);
    assert!(!half.wire_present, "still rising");
    let full = sim.step(&d, &net, &at, PoleInputs::default(), 0.8);
    assert!(full.wire_present);
    assert!(half.poles[0].pitch > 0.0 && half.poles[0].pitch < full.poles[0].pitch);
    let [l, r] = full.poles;
    assert!((l.shoe_pos.x + 0.255).abs() < 1e-6 && (r.shoe_pos.x - 0.255).abs() < 1e-6);
    assert!((l.shoe_pos.z - 5.8).abs() < 1e-6);
    assert!(l.shoe_pos.y < 0.0, "trailing behind the foot");
    assert!((l.shoe_pos - d.poles[0].base).length() - d.poles[0].length() < 1e-6);
    assert!(l.force > 0.0 && l.on_wire);
}

#[test]
fn no_wire_in_reach_leaves_the_poles_down() {
    let d = decl();
    let net = straight(100.0, 50.0);
    // two metres beside the wire
    let at = body(2.0, 20.0, 0.0);
    let mut sim = PoleSim::default();
    grab(&mut sim, &d, &net, &at);
    assert_eq!(sim.poles[0].phase, PolePhase::Hooked);
    sim.step(
        &d,
        &net,
        &at,
        PoleInputs {
            grab: [true, false],
            reset: false,
            retract: false,
        },
        0.016,
    );
    assert!(sim.events.contains(&PoleEvent::NoWire { pole: 0 }));
}

#[test]
fn each_pole_takes_its_own_wire_when_the_bus_stands_off_centre() {
    let d = decl();
    let net = straight(100.0, 50.0);
    // 0.3 m right of the line: the right wire is nearer the left pole's shoe than the left one
    let at = body(0.3, 20.0, 0.0);
    let mut sim = PoleSim::default();
    raise(&mut sim, &d, &net, &at);
    let o = sim.step(&d, &net, &at, PoleInputs::default(), 0.016);
    assert!(o.wire_present);
    assert!(o.poles[0].shoe_pos.x < o.poles[1].shoe_pos.x);
    assert!((o.poles[0].shoe_pos.x - (-0.555)).abs() < 1e-6);
}

#[test]
fn the_shoes_follow_a_straight_line_across_its_joints() {
    let d = decl();
    let net = straight(300.0, 20.0);
    let mut sim = PoleSim::default();
    raise(&mut sim, &d, &net, &body(0.0, 20.0, 0.0));
    let mut joints = 0;
    let mut y = 20.0;
    while y < 250.0 {
        y += 50.0 / 3.6 / 60.0;
        let o = sim.step(
            &d,
            &net,
            &body(0.0, y, 0.0),
            PoleInputs::default(),
            1.0 / 60.0,
        );
        assert!(o.wire_present, "on the wire at y = {y}");
        assert!(sim.poles[0].reach_error < 1e-9);
        joints += sim
            .events
            .iter()
            .filter(|e| matches!(e, PoleEvent::Joint { pole: 0, .. }))
            .count();
    }
    assert!(joints >= 10, "{joints} joints passed");
}

#[test]
fn a_reversed_splice_and_a_loose_joint_are_passed() {
    let d = decl();
    let splines = vec![
        spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 60.0, 0.0),
        // runs back south from 120 to 60: its end meets the first's end
        spline(2, WIRE, [0.0, 120.0, 0.0], 180.0, 60.0, 0.0),
        // starts 0.8 m after the second's start
        spline(3, WIRE, [0.0, 120.8, 0.0], 0.0, 60.0, 0.0),
    ];
    let mut net = WireNetwork::default();
    net.insert_tile(build_tile_wires(
        (0, 0),
        DVec2::ZERO,
        &tile(splines, vec![]),
        &params(),
    ));
    let mut sim = PoleSim::default();
    raise(&mut sim, &d, &net, &body(0.0, 20.0, 0.0));
    let mut y = 20.0;
    while y < 170.0 {
        y += 0.2;
        let o = sim.step(
            &d,
            &net,
            &body(0.0, y, 0.0),
            PoleInputs::default(),
            1.0 / 60.0,
        );
        assert!(o.wire_present, "on the wire at y = {y}");
        // the shoes keep their sides across the reversed span
        assert!(
            o.poles[0].shoe_pos.x < 0.0 && o.poles[1].shoe_pos.x > 0.0,
            "y = {y}"
        );
    }
    assert_eq!(o_spline(&sim, &net), 3);
}

fn o_spline(sim: &PoleSim, net: &WireNetwork) -> i64 {
    net.span(sim.poles[0].cursor.unwrap().span)
        .unwrap()
        .spline_id
}

#[test]
fn the_shoes_stay_on_a_curve_as_the_body_turns_under_them() {
    let d = decl();
    // a right-hand curve of 25 m radius after 30 m of straight
    let curve_len = std::f64::consts::FRAC_PI_2 * 25.0;
    let splines = vec![
        spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 30.0, 0.0),
        spline(2, WIRE, [0.0, 30.0, 0.0], 0.0, curve_len, 25.0),
        spline(3, WIRE, [25.0, 55.0, 0.0], 90.0, 40.0, 0.0),
    ];
    let mut net = WireNetwork::default();
    net.insert_tile(build_tile_wires(
        (0, 0),
        DVec2::ZERO,
        &tile(splines, vec![]),
        &params(),
    ));
    let mut sim = PoleSim::default();
    raise(&mut sim, &d, &net, &body(0.0, 20.0, 0.0));
    // drive the bus along the centre line, rolled 3 degrees in the curve
    let path = |s: f64| -> (f64, f64, f64) {
        if s < 30.0 {
            (0.0, s, 0.0)
        } else if s < 30.0 + curve_len {
            let a = (s - 30.0) / 25.0;
            (25.0 - 25.0 * a.cos(), 30.0 + 25.0 * a.sin(), a.to_degrees())
        } else {
            (25.0 + (s - 30.0 - curve_len), 55.0, 90.0)
        }
    };
    let mut s = 20.0;
    let mut max_yaw: f64 = 0.0;
    while s < 85.0 {
        s += 0.15;
        let (x, y, h) = path(s);
        let in_curve = (30.0..30.0 + curve_len).contains(&s);
        let mut pose = body(x, y, h);
        if in_curve {
            pose.rotation *= DMat3::from_rotation_y(3f64.to_radians());
        }
        let o = sim.step(&d, &net, &pose, PoleInputs::default(), 1.0 / 60.0);
        assert!(o.wire_present, "on the wire at s = {s}");
        assert!(sim.poles.iter().all(|p| p.reach_error < 1e-9), "s = {s}");
        max_yaw = max_yaw.max(o.poles[0].yaw.abs());
    }
    // (the chord from the foot to the shoe, 6 m back on a 25 m arc, turns about 1.7 degrees
    // off the body's heading, more with the roll)
    assert!(
        max_yaw > 1.5,
        "the poles swing out in the curve ({max_yaw:.1} degrees)"
    );
}

#[test]
fn raising_takes_the_same_time_at_any_frame_rate() {
    let d = decl();
    let net = straight(100.0, 50.0);
    let at = body(0.0, 20.0, 0.0);
    for fps in [20.0, 60.0, 200.0] {
        let mut sim = PoleSim::default();
        sim.step(
            &d,
            &net,
            &at,
            PoleInputs {
                grab: [true, true],
                reset: false,
                retract: false,
            },
            1.0 / fps,
        );
        let mut t = 0.0;
        while sim.poles[0].phase != PolePhase::OnWire {
            sim.step(&d, &net, &at, PoleInputs::default(), 1.0 / fps);
            t += 1.0 / fps;
            assert!(t < 3.0);
        }
        assert!((t - 1.5).abs() <= 1.0 / fps + 1e-9, "{fps} fps: {t:.3} s");
    }
}

#[test]
fn a_second_press_lowers_the_pole_to_its_hook() {
    let d = decl();
    let net = straight(100.0, 50.0);
    let at = body(0.0, 20.0, 0.0);
    let mut sim = PoleSim::default();
    raise(&mut sim, &d, &net, &at);
    sim.step(
        &d,
        &net,
        &at,
        PoleInputs {
            grab: [true, false],
            reset: false,
            retract: false,
        },
        0.016,
    );
    assert_eq!(sim.poles[0].phase, PolePhase::Lowering);
    assert_eq!(sim.poles[1].phase, PolePhase::OnWire);
    for _ in 0..70 {
        sim.step(&d, &net, &at, PoleInputs::default(), 1.0 / 60.0);
    }
    assert_eq!(sim.poles[0].phase, PolePhase::Hooked);
}

#[test]
fn a_long_stall_puts_the_poles_back_and_a_short_one_does_not() {
    let d = decl();
    let net = straight(100.0, 50.0);
    let at = body(0.0, 20.0, 0.0);
    let mut sim = PoleSim::default();
    raise(&mut sim, &d, &net, &at);
    sim.step(&d, &net, &at, PoleInputs::default(), 1.0);
    assert_eq!(sim.poles[0].phase, PolePhase::OnWire, "a one-second hitch");
    sim.step(&d, &net, &at, PoleInputs::default(), 5.0);
    assert_eq!(sim.poles[0].phase, PolePhase::Hooked);
}

#[test]
fn a_vehicle_is_found_by_its_bus_file_and_its_carrier_by_its_part() {
    let c = Convention {
        params: params(),
        entries: parse_registry(REGISTRY),
        params_path: Default::default(),
        registry_path: None,
    };
    let root = "C:\\Games\\OMSI 2\\";
    let (d, carrier) = find_declaration(&c, &format!("{root}vehicles\\t\\RIGID.bus"), &[])
        .unwrap()
        .unwrap();
    assert_eq!((d.entry.as_str(), carrier), ("rigid", 0));
    let (d, carrier) = find_declaration(
        &c,
        &format!("{root}Vehicles/T/front.bus"),
        &[&format!("{root}Vehicles\\T\\rear.bus")],
    )
    .unwrap()
    .unwrap();
    assert_eq!((d.entry.as_str(), carrier), ("artic", 1));
    assert!(
        find_declaration(&c, &format!("{root}Vehicles\\T\\front.bus"), &[])
            .unwrap()
            .is_err(),
        "rear part missing"
    );
    assert!(
        find_declaration(&c, &format!("{root}Vehicles\\T\\other.bus"), &[]).is_none(),
        "disabled"
    );
    assert!(
        find_declaration(&c, &format!("{root}Vehicles\\T2\\rigid.bus"), &[]).is_none(),
        "another folder"
    );
}

#[test]
fn a_parameter_the_poles_need_must_be_in_the_file() {
    let text = PARAMS.replace("pole_raise_duration=1.5\n", "");
    let e =
        declaration_from(&parse_registry(REGISTRY)[0], &parse_params(&text).unwrap()).unwrap_err();
    assert_eq!(e.to_string(), "[physics] has no pole_raise_duration");
}

/// Run frames of `dt` until `done` (at most `limit` seconds); the seconds it took.
fn run_until(
    sim: &mut PoleSim,
    d: &TrolleyDeclaration,
    net: &WireNetwork,
    at: &BodyPose,
    dt: f64,
    limit: f64,
    done: impl Fn(&PoleSim) -> bool,
) -> f64 {
    let mut t = 0.0;
    while !done(sim) {
        sim.step(d, net, at, PoleInputs::default(), dt);
        t += dt;
        assert!(t < limit, "still not done after {limit} s");
    }
    t
}

#[test]
fn a_pole_too_far_from_its_wire_dewires_springs_up_and_the_retriever_hooks_it() {
    let d = decl();
    let net = straight(100.0, 50.0);
    let mut sim = PoleSim::default();
    raise(&mut sim, &d, &net, &body(0.0, 20.0, 0.0));
    // the bus moves 7 m to the side: beyond the poles' reach
    let away = body(7.0, 20.0, 0.0);
    let mut dewired_at = None;
    let mut highest: f64 = 0.0;
    let mut t = 0.0;
    while sim.poles[0].phase != PolePhase::Hooked {
        let o = sim.step(&d, &net, &away, PoleInputs::default(), 1.0 / 60.0);
        t += 1.0 / 60.0;
        assert!(t < 5.0);
        if dewired_at.is_none() && sim.poles[0].phase == PolePhase::Dewired {
            dewired_at = Some(t);
            assert_eq!(sim.poles[0].dewire, Some(DewireReason::Reach));
        }
        if sim.poles[0].phase == PolePhase::Dewired {
            assert!(o.poles[0].dewired && !o.wire_present);
            highest = highest.max(o.poles[0].pitch);
        }
    }
    let at: f64 = dewired_at.expect("it dewired");
    assert!((at - 0.12).abs() < 0.04, "after the reach time ({at:.3} s)");
    assert!(highest > 15.0, "it sprang up ({highest:.1} degrees)");
    let o = sim.step(&d, &net, &away, PoleInputs::default(), 1.0 / 60.0);
    assert!(
        !o.poles[0].dewired && o.poles[0].pitch == 0.0,
        "back on the hook"
    );
}

#[test]
fn a_short_stretch_out_of_reach_does_not_dewire_whatever_the_frame_rate() {
    let d = decl();
    let net = straight(100.0, 50.0);
    for fps in [20.0, 60.0, 200.0] {
        let mut sim = PoleSim::default();
        raise(&mut sim, &d, &net, &body(0.0, 20.0, 0.0));
        // out of reach for 0.1 s (under the 0.12 s the file allows), then back
        let frames = (0.1 * fps) as usize;
        for _ in 0..frames {
            sim.step(
                &d,
                &net,
                &body(7.0, 20.0, 0.0),
                PoleInputs::default(),
                1.0 / fps,
            );
        }
        let o = sim.step(
            &d,
            &net,
            &body(0.0, 20.0, 0.0),
            PoleInputs::default(),
            1.0 / fps,
        );
        assert!(o.wire_present, "{fps} fps");
        // and 0.2 s out of reach does
        for _ in 0..(0.2 * fps) as usize {
            sim.step(
                &d,
                &net,
                &body(7.0, 20.0, 0.0),
                PoleInputs::default(),
                1.0 / fps,
            );
        }
        assert_eq!(sim.poles[0].phase, PolePhase::Dewired, "{fps} fps");
    }
}

#[test]
fn the_end_of_the_wire_dewires_the_shoe() {
    let d = decl();
    let net = straight(60.0, 60.0);
    let mut sim = PoleSim::default();
    raise(&mut sim, &d, &net, &body(0.0, 20.0, 0.0));
    let mut y = 20.0;
    while sim.poles[0].phase == PolePhase::OnWire {
        y += 0.2;
        sim.step(
            &d,
            &net,
            &body(0.0, y, 0.0),
            PoleInputs::default(),
            1.0 / 60.0,
        );
        assert!(y < 80.0);
    }
    assert_eq!(sim.poles[0].dewire, Some(DewireReason::Gap));
    // the shoe trails 6 m behind the foot (2.24 m ahead of the origin): it runs off the end
    // when the bus is about 64 m along
    assert!((62.0..66.0).contains(&y), "at y = {y:.1}");
}

#[test]
fn the_retract_control_takes_both_poles_down_halfway_pauses_and_hooks_them() {
    let d = decl();
    let net = straight(100.0, 50.0);
    let at = body(0.0, 20.0, 0.0);
    let mut sim = PoleSim::default();
    raise(&mut sim, &d, &net, &at);
    let on = sim.step(&d, &net, &at, PoleInputs::default(), 1.0 / 60.0);
    let retract = PoleInputs {
        retract: true,
        ..Default::default()
    };
    sim.step(&d, &net, &at, retract, 1.0 / 60.0);
    assert!(sim.poles.iter().all(|p| p.phase == PolePhase::Retracting));
    // halfway after half the lowering time (0.5 s), and still there during the pause
    let mid = sim.step(&d, &net, &at, retract, 0.5);
    assert!((mid.poles[0].pitch - on.poles[0].pitch / 2.0).abs() < 0.5);
    assert!(!mid.poles[0].dewired && !mid.wire_present);
    let paused = sim.step(&d, &net, &at, PoleInputs::default(), 0.4);
    assert_eq!(paused.poles[0].pitch, mid.poles[0].pitch);
    let t = run_until(&mut sim, &d, &net, &at, 1.0 / 60.0, 2.0, |s| {
        s.poles.iter().all(|p| p.phase == PolePhase::Hooked)
    });
    assert!(t < 0.8, "down {t:.2} s after the pause");
}
