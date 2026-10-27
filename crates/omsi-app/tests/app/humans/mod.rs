mod pax;
mod population_limit;
mod standing_walk;

use glam::{DVec2, DVec3, Mat4, Vec3};
use omsi_sim::people::crowd;
use omsi_sim::people::crowd::{Block, CrowdParams, Walker};
use omsi_sim::traffic::{LaneKind, Network};
use omsi_sim::VehicleInstance;
use openomsi_game::testing;
use openomsi_game::testing::humans::pax::*;
use openomsi_game::testing::humans::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A standing articulated bus's front section (2.55 x 10.09 m, heading north), the right
/// door's exit point 1.16 m right of its middle line.
fn standing_bus() -> Block {
    Block {
        center: DVec2::new(0.0, 0.618),
        half: DVec2::new(1.275, 5.043),
        heading: 0.0,
        vel: DVec2::ZERO,
    }
}

#[test]
fn a_standing_vehicle_is_in_the_way_only_of_who_walks_into_it() {
    let bus = [standing_bus()];
    // beside it, along its side towards the stop's point 1 m off it, near its front
    let beside = DVec2::new(1.275 + BODY_OUTSIDE, 1.0);
    let want = (DVec2::new(2.3, 7.0) - beside).normalize() * 1.1;
    assert!(!standing_vehicle_ahead(beside, want, &bus));
    // just off it, still within its box at the door
    let door = DVec2::new(1.159, 1.024);
    assert!(!standing_vehicle_ahead(door, want, &bus));
    // away from it, onto the pavement
    assert!(!standing_vehicle_ahead(door, DVec2::X * 1.1, &bus));
    // straight at its rear, half a metre short of it: in the way
    let behind = DVec2::new(0.0, 0.618 - 5.043 - 0.5);
    assert!(standing_vehicle_ahead(behind, DVec2::Y * 1.1, &bus));
    // at its side from across the pavement
    assert!(standing_vehicle_ahead(
        DVec2::new(2.0, 0.0),
        DVec2::NEG_X * 1.1,
        &bus
    ));
    // a moving one is the crowd's business
    let mut moving = bus;
    moving[0].vel = DVec2::Y * 3.0;
    assert!(!standing_vehicle_ahead(behind, DVec2::Y * 1.1, &moving));
}

/// Off the door of a standing bus towards the stop's point of the pavement, 1 m off its
/// side and 6 m on: walking along the bus, they are there in a few seconds - not stood
/// beside the door until the bus leaves (#57).
#[test]
fn off_a_standing_bus_people_walk_along_it_to_the_pavement() {
    let bus = [standing_bus()];
    let target = DVec2::new(2.3, 7.0);
    let mut w = Walker::new(DVec2::new(1.159, 1.024), BODY_OUTSIDE, 0);
    let params = CrowdParams::default();
    let dt = 1.0 / 60.0;
    let mut held = 0.0;
    for _ in 0..(8.0 / dt) as usize {
        let want = arrive(w.pos, target, 1.1);
        w.want = if standing_vehicle_ahead(w.pos, want, &bus) {
            held += dt;
            DVec2::ZERO
        } else {
            want
        };
        crowd::step(std::slice::from_mut(&mut w), &bus, &params, dt);
    }
    assert!(held < 0.1, "held {held:.1} s beside the bus");
    assert!((w.pos - target).length() < 0.5, "{:?}", w.pos);
}

/// Off a bus 3 m from the pavement's path (along y), walking onto it at 1.1 m/s: hardly
/// faster over any quarter of a second (pulled by the corridor all the way, over 1.5
/// m/s), and on the path in the end.
#[test]
fn off_a_bus_people_walk_onto_the_pavement_at_their_pace() {
    let walk = |held: bool| {
        let pace = 1.1;
        let mut w = Walker::new(DVec2::new(3.0, 0.0), 0.25, 0);
        let params = CrowdParams::default();
        let dt = 1.0 / 60.0;
        let mut track = vec![w.pos];
        for _ in 0..360 {
            let s = w.pos.y.max(0.0);
            let (a, b) = (DVec2::new(0.0, s - 2.0), DVec2::new(0.0, s + 2.5));
            w.want = (DVec2::new(0.0, s + 1.3) - w.pos).normalize_or_zero() * pace;
            let c = Some((a, b, 0.5));
            w.corridor = if held { c } else { path_corridor(w.pos, c) };
            crowd::step(std::slice::from_mut(&mut w), &[], &params, dt);
            track.push(w.pos);
        }
        let fastest = track
            .windows(16)
            .map(|k| (k[15] - k[0]).length() / (15.0 * dt))
            .fold(0.0, f64::max);
        (fastest, w.pos)
    };
    let (fastest, end) = walk(false);
    assert!(fastest < 1.3, "{fastest:.2} m/s");
    assert!(end.x.abs() < 0.55, "{end:?}");
    let held = walk(true).0;
    assert!(held > 1.5, "{held:.2} m/s");
    // on the path the corridor holds again
    assert!(path_corridor(DVec2::new(0.55, 0.0), Some((DVec2::ZERO, DVec2::Y, 0.5))).is_some());
    assert!(path_corridor(DVec2::new(3.0, 0.0), Some((DVec2::ZERO, DVec2::Y, 0.5))).is_none());
}

#[test]
fn map_humans_load_nested_paths_and_preserve_weights() {
    let root = std::env::temp_dir().join(format!("omsi-map-human-paths-{}", std::process::id()));
    let nested = root.join("Humans/JP_Test/Child_1");
    std::fs::create_dir_all(&nested).unwrap();
    // Synthetic definitions: no original passenger assets are required.
    std::fs::write(nested.join("Child_1.hum"), "[model]\nmodel.cfg\n").unwrap();
    std::fs::write(nested.join("model.cfg"), "").unwrap();
    let other = root.join("Humans/Other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("Man.hum"), "[model]\nmodel.cfg\n").unwrap();
    std::fs::write(other.join("model.cfg"), "").unwrap();
    let list = vec![
        "humans\\jp_test\\child_1\\child_1.hum".into(),
        "Humans/JP_Test/Child_1/Child_1.hum".into(),
        "JP_Test/Child_1/Child_1.hum".into(),
        "Humans/JP_Test/Missing.hum".into(),
    ];
    let picked = map_human_types(&root, &list);
    assert_eq!(picked.len(), 3);
    assert!(Arc::ptr_eq(&picked[0], &picked[1]));
    assert!(Arc::ptr_eq(&picked[1], &picked[2]));
    // (case-blind: a case-insensitive disk keeps the list's own spelling; and
    // separator-blind: on Windows `nested` keeps the slashes it was joined with, while
    // the resolved path is built with backslashes)
    let lower = |p: &Path| p.to_string_lossy().to_lowercase().replace('\\', "/");
    assert!(picked
        .iter()
        .all(|t| lower(&t.def.path).starts_with(&lower(&nested))));
    assert!(map_human_types(&root, &["Humans/Missing/None.hum".into()]).is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

/// Berlin 1991's pack: full fare, short haul, day ticket (adults), and two reduced
/// fares for 6..13.
fn berlin_91() -> omsi_content::tickets::TicketPack {
    let t = |name: &str, age: (i32, i32), day: bool, p: f32| omsi_content::tickets::Ticket {
        name: name.into(),
        age_min: age.0,
        age_max: age.1,
        day_ticket: day,
        probability: p,
        ..Default::default()
    };
    omsi_content::tickets::TicketPack {
        stamper_prop: 0.3,
        ticketbuy_prop: 0.2,
        tickets: vec![
            t("Fahrschein", (14, 200), false, 1.0),
            t("Kurzstrecke", (14, 200), false, 0.4),
            t("Tageskarte", (14, 200), true, 0.2),
            t("Ermaessigt", (6, 13), false, 1.0),
            t("Kurzstrecke Erm", (6, 13), false, 0.4),
        ],
        ..Default::default()
    }
}

#[test]
fn seats_counted_by_the_scripts_numbers() {
    let seat = |omsi_seat: usize| Seat {
        pos: Vec3::ZERO,
        floor: Vec3::ZERO,
        rot: 0.0,
        seated: true,
        height: 0.45,
        omsi_seat,
        switch_var: None,
        taken_var: None,
        group: 0,
    };
    // the driver's place is seat 0, a second section's numbers follow the first's
    let seats = [seat(1), seat(2), seat(4), seat(6)];
    assert_eq!(
        seat_numbers(&seats, [0, 2, 2, 3].into_iter()),
        [0, 1, 0, 0, 2, 0, 1]
    );
    assert!(seat_numbers(&[], [0].into_iter()).is_empty());
}

#[test]
fn tickets_by_age_and_time() {
    let mut h = Humans::new(Path::new("/nonexistent"));
    h.tickets = Some(Arc::new(berlin_91()));
    let count = |h: &mut Humans, age: f32| {
        let mut n = [0usize; 5];
        for _ in 0..4000 {
            n[h.pick_ticket(age).unwrap()] += 1;
        }
        n
    };
    // an adult (OMSI's default age of 40) never gets a reduced fare, a child only those
    h.time_of_day = 9.0 * 3600.0;
    let adult = count(&mut h, 40.0);
    assert_eq!(adult[3] + adult[4], 0);
    assert!(adult[2] > 300, "{adult:?}");
    let child = count(&mut h, 10.0);
    assert_eq!(child[0] + child[1] + child[2], 0);
    // day tickets sell best at 9:00, little early in the morning and late at night
    h.time_of_day = 1.0 * 3600.0;
    let early = count(&mut h, 40.0);
    assert!(early[2] * 4 < adult[2], "{early:?} vs {adult:?}");
    assert!(day_ticket_factor(9.0 * 3600.0) > 0.99);
    assert!(day_ticket_factor(0.0) < 0.01);
    assert!((day_ticket_factor(20.0 * 3600.0) - (1.0 - 39_600.0 / 56_376.0) as f32).abs() < 1e-3);
    // nobody in the age range: no ticket
    assert_eq!(h.pick_ticket(3.0), None);
}

fn lane(points: Vec<DVec3>, kind: LaneKind) -> omsi_sim::traffic::Lane {
    omsi_sim::traffic::LaneBuilder::polyline(points, kind, 2.5)
}

#[test]
fn pavement_corners_are_joined_and_routed() {
    // an L of pavement: the two paths meet at a right angle, which the road network's
    // heading rule leaves unlinked
    let mut net = Network::default();
    net.lanes.push(lane(
        vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 20.0, 0.0)],
        LaneKind::Sidewalk,
    ));
    net.lanes.push(lane(
        vec![DVec3::new(0.5, 20.3, 0.0), DVec3::new(30.0, 20.3, 0.0)],
        LaneKind::Sidewalk,
    ));
    net.lanes.push(lane(
        vec![DVec3::new(-5.0, 10.0, 0.0), DVec3::new(5.0, 10.0, 0.0)],
        LaneKind::Street,
    ));
    net.link(1.5);
    assert!(
        net.lanes[0].next.is_empty(),
        "the road rule does not join the corner"
    );
    let ped = PedNet::build(&net);
    // the corner is one junction: walking up the first path goes on round it
    let up = Leg {
        lane: 0,
        a: 5.0,
        b: 20.0,
    };
    let corner = ped.end_node(&net, &up).unwrap();
    assert!(
        ped.out[corner].iter().any(|&(l, fwd)| l == 1 && fwd),
        "{:?}",
        ped.out[corner]
    );
    // a dead end turns round
    let n = ped
        .end_node(
            &net,
            &Leg {
                lane: 1,
                a: 0.0,
                b: net.lanes[1].length(),
            },
        )
        .unwrap();
    let turn = ped.next_leg(&net, n, 1, 7).unwrap();
    assert_eq!(turn.lane, 1);
    assert!(turn.a > turn.b);
}

#[test]
fn a_vehicle_standing_on_a_crossing_holds_people_only_for_a_moment() {
    let mut net = Network::default();
    net.lanes.push(lane(
        vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 8.0, 0.0)],
        LaneKind::Sidewalk,
    ));
    net.lanes.push(lane(
        vec![DVec3::new(-30.0, 4.0, 0.0), DVec3::new(30.0, 4.0, 0.0)],
        LaneKind::Street,
    ));
    net.link(1.5);
    let mut ped = PedNet::build(&net);
    let h = Humans::new(Path::new("/nonexistent"));
    let next = Leg {
        lane: 0,
        a: 0.0,
        b: 8.0,
    };
    let stopped = [(DVec2::new(0.0, 4.0), DVec2::ZERO, 6.0)];
    let mut held = |s: f32| h.may_cross(Some(&mut ped), &net, &next, None, &stopped, 1.4, s);
    assert!(held(0.0).is_err());
    assert!(held(2.0).is_err());
    assert!(held(30.0).is_ok());
    // one driving across is waited for however long they have stood
    let moving = [(DVec2::new(-8.0, 4.0), DVec2::new(8.0, 0.0), 2.0)];
    assert!(h
        .may_cross(Some(&mut ped), &net, &next, None, &moving, 1.4, 30.0)
        .is_err());
}

#[test]
fn crossings_of_a_pavement_path_are_found() {
    let mut net = Network::default();
    net.lanes.push(lane(
        vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 8.0, 0.0)],
        LaneKind::Sidewalk,
    ));
    net.lanes.push(lane(
        vec![DVec3::new(-30.0, 4.0, 0.0), DVec3::new(30.0, 4.0, 0.0)],
        LaneKind::Street,
    ));
    net.link(1.5);
    let mut ped = PedNet::build(&net);
    let x = ped.crossings(&net, 0).to_vec();
    assert_eq!(x.len(), 1);
    assert!((x[0] - DVec2::new(0.0, 4.0)).length() < 1e-6);
}

/// An articulated bus: the front section's cabin and the rear section's (which only has
/// exits and a seat) become one network through the joint, numbered front first, and a
/// walk through the bent joint moves on without a jump.
#[test]
fn articulated_cabins_are_joined_through_the_bellows() {
    let dir = std::env::temp_dir().join(format!("omsi-humans-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
    // front: door 0 at the front right, exit 1 in the middle, link to the rear at 3
    write("paths_a.cfg", "[pathpnt]\n1.2\n4\n0.4\n[pathpnt]\n0\n4\n0.5\n[pathpnt]\n0\n0\n0.5\n[pathpnt]\n0\n-4.2\n0.6\n[pathpnt]\n1.2\n0\n0.4\n[pathlink]\n0\n1\n[pathlink]\n1\n2\n[pathlink]\n2\n3\n[pathlink]\n2\n4\n");
    write(
        "cabin_a.cfg",
        "[entry]\n0\n[exit]\n4\n[linkToPrevVeh]\n3\n[passpos]\n-0.5\n2\n1.0\n0.45\n0\n",
    );
    // rear: only exits, a seat and the link to the front at point 0
    write("paths_b.cfg", "[pathpnt]\n0\n3.6\n0.6\n[pathpnt]\n0\n0\n0.6\n[pathpnt]\n1.2\n0\n0.4\n[pathpnt]\n0\n-2\n0.6\n[pathlink]\n0\n1\n[pathlink]\n1\n2\n[pathlink]\n1\n3\n");
    write(
        "cabin_b.cfg",
        "[exit]\n2\n[linkToNextVeh]\n0\n[passpos]\n-0.5\n-2\n1.1\n0.45\n0\n",
    );
    let def = |cabin: &str, paths: &str| omsi_vehicle::Vehicle {
        path: dir.join("bus.bus"),
        passenger_cabin: Some(cabin.into()),
        paths: Some(paths.into()),
        bounding_box: Some([2.5, 9.0, 3.0, 0.0, 0.0, 1.5]),
        ..Default::default()
    };
    let (front, rear) = (
        def("cabin_a.cfg", "paths_a.cfg"),
        def("cabin_b.cfg", "paths_b.cfg"),
    );
    // couplings: the front's at y -4.3, the rear's own at y 4.0
    let (back, own) = (Vec3::new(0.0, -4.3, 0.3), Vec3::new(0.0, 4.0, 0.3));
    let offset = back - own;
    let cabin = Cabin::load_train(&[(&front, Vec3::ZERO, f32::INFINITY), (&rear, offset, back.y)])
        .expect("cabin");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(cabin.parts.len(), 2);
    assert_eq!(cabin.graph.points.len(), 9);
    assert_eq!(
        (cabin.entries.len(), cabin.exits.len(), cabin.seats.len()),
        (1, 2, 2)
    );
    // exit 1 is the rear section's door, where the rear file puts it
    assert!(
        (cabin.exits[1].inside - Vec3::new(1.2, -8.3, 0.4)).length() < 1e-4,
        "{:?}",
        cabin.exits[1].inside
    );
    // the seat in the rear is reached from the front door through the joint, along the
    // routing tables OMSI builds over the joined network
    let seat = &cabin.seats[1];
    assert!((seat.pos.y + 10.3).abs() < 1e-4);
    let all: Vec<Option<usize>> = (0..cabin.graph.points.len()).map(Some).collect();
    let to = cabin
        .omsi_nearest(seat.floor, &all, false, false, None, None)
        .unwrap();
    let mut at = cabin.entries[0].point.unwrap();
    let mut route = vec![cabin.graph.points[at]];
    while at != to {
        at = cabin.route_next(at, to).expect("a way on").0;
        route.push(cabin.graph.points[at]);
        assert!(route.len() < 20, "{route:?}");
    }
    assert!(
        route.iter().any(|p| (p.y + 4.2).abs() < 1e-4)
            && route.iter().any(|p| (p.y + 4.7).abs() < 1e-4),
        "{route:?}"
    );
    // and the nearest exit from there is the rear one
    let exit = cabin.omsi_nearest(seat.floor, &cabin.exit_points(), false, false, None, None);
    assert_eq!(exit, cabin.exits[1].point);
    // the rear section bent 30 degrees about the coupling: walking down the aisle moves on
    // smoothly, and the frames agree with the sections away from the joint
    let lead_rot = Mat4::IDENTITY;
    let bent = 30.0f64;
    let rot = Mat4::from_rotation_z((-bent).to_radians() as f32);
    let pos = back.as_dvec3() - rot.transform_point3(own).as_dvec3();
    let frames = [PartFrame {
        pos,
        rot,
        heading: bent,
        offset,
        joint_y: back.y,
        half: DVec2::new(1.25, 4.5),
        centre: DVec2::ZERO,
    }];
    // beside the aisle the two frames disagree by 0.31 m at the joint itself
    let at_joint = Vec3::new(0.6, back.y, 0.5);
    let rear_frame = pos + rot.transform_point3(at_joint - offset).as_dvec3();
    assert!((rear_frame - at_joint.as_dvec3()).length() > 0.3);
    let mut last = train_point(DVec3::ZERO, &lead_rot, &frames, Vec3::new(0.6, 0.0, 0.5));
    for k in 1..=100 {
        let y = -(k as f32) * 0.1;
        let p = train_point(DVec3::ZERO, &lead_rot, &frames, Vec3::new(0.6, y, 0.5));
        assert!(
            (p - last).length() < 0.14,
            "a jump of {:.3} m at y {y}",
            (p - last).length()
        );
        last = p;
    }
    let ahead = train_point(DVec3::ZERO, &lead_rot, &frames, Vec3::new(1.0, -1.0, 0.5));
    assert!((ahead - DVec3::new(1.0, -1.0, 0.5)).length() < 1e-4);
    let behind_joint = Vec3::new(1.0, -9.0, 0.5);
    let p = train_point(DVec3::ZERO, &lead_rot, &frames, behind_joint);
    assert!((p - (pos + rot.transform_point3(behind_joint - offset).as_dvec3())).length() < 1e-4);
    assert!((train_heading(0.0, &frames, behind_joint) - bent).abs() < 1e-9);
    assert!((train_heading(0.0, &frames, Vec3::new(0.0, back.y, 0.5)) - bent * 0.5).abs() < 1e-9);
}

/// The SD200's footsteps as its paths.cfg gives them to the links (#311): the stairs
/// sound as stairs, the front of the upper deck as its own floor, the aisle below as
/// the plain floor.
#[test]
fn footsteps_come_from_the_links_step_sound_pack() {
    let root = omsi_cfg::flags::ROOT
        .os()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_SD200/MAN_SD80.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let def = omsi_vehicle::Vehicle::load(&bus).expect("SD200");
    let cabin = Cabin::load_train(&[(&def, Vec3::ZERO, f32::INFINITY)]).expect("cabin");
    // the link nearest the point, and its pack
    let first = |p: Vec3| {
        let pts = &cabin.graph.points;
        let d = |l: &(i32, i32, bool)| {
            let (a, b) = (pts[l.0 as usize], pts[l.1 as usize]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            (a + ab * t - p).length()
        };
        let l = (0..cabin.links.len())
            .min_by(|&x, &y| d(&cabin.links[x]).total_cmp(&d(&cabin.links[y])))?;
        cabin.link_pack[l].map(|k| cabin.step_packs[k][0].to_ascii_lowercase())
    };
    assert_eq!(
        first(Vec3::new(-0.89, -1.61, 1.63)).as_deref(),
        Some("step_st_01.wav"),
        "the rear stairs"
    );
    assert_eq!(
        first(Vec3::new(0.0, 4.35, 2.5)).as_deref(),
        Some("step_ov_01.wav"),
        "the upper deck's front"
    );
    assert_eq!(
        first(Vec3::new(0.0, 0.84, 0.57)).as_deref(),
        Some("step_01.wav"),
        "the aisle below"
    );
}

/// The SD202's cabin: the stairs down from the upper deck end beside the rear exits, and
/// the walk from up there to an exit goes down the stairs.
#[test]
fn double_decker_exits_are_reached_down_the_stairs() {
    let root = omsi_cfg::flags::ROOT
        .os()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_SD202/MAN_D92.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let def = omsi_vehicle::Vehicle::load(&bus).expect("SD202");
    let cabin = Cabin::load_train(&[(&def, Vec3::ZERO, f32::INFINITY)]).expect("cabin");
    assert_eq!(cabin.exits.len(), 2);
    let exit = &cabin.exits[1];
    assert!(
        (exit.wait - Vec3::new(0.806, -1.26, 0.505)).length() < 1e-3,
        "{:?}",
        exit.wait
    );
    // from the upper deck the routing tables lead down both flights to the exit
    let all: Vec<Option<usize>> = (0..cabin.graph.points.len()).map(Some).collect();
    let upstairs = cabin
        .omsi_nearest(Vec3::new(0.0, -1.8, 2.46), &all, false, false, None, None)
        .unwrap();
    assert!((cabin.graph.points[upstairs].z - 2.46).abs() < 0.1);
    let to = exit.point.unwrap();
    let mut at = upstairs;
    let mut route = vec![cabin.graph.points[at]];
    while at != to {
        at = cabin.route_next(at, to).expect("a way down").0;
        route.push(cabin.graph.points[at]);
        assert!(route.len() < 60, "{route:?}");
    }
    assert!(
        route.iter().any(|p| (p.z - 1.82).abs() < 0.01)
            && route.iter().any(|p| (p.z - 1.205).abs() < 0.01),
        "{route:?}"
    );
}

#[test]
fn legs_run_both_ways() {
    let mut net = Network::default();
    net.lanes.push(lane(
        vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 10.0, 0.0)],
        LaneKind::Sidewalk,
    ));
    let back = Leg {
        lane: 0,
        a: 8.0,
        b: 2.0,
    };
    let (p, h) = back.at(&net, 1.0);
    assert!((p.y - 7.0).abs() < 1e-6);
    assert!((h - 180.0).abs() < 1e-6);
    assert!((back.project(&net, DVec3::new(0.3, 5.0, 0.0), 2.5) - 3.0).abs() < 0.11);
}

#[test]
fn doors_open_falls_back_when_exit_vars_are_undeclared() {
    let dir = std::env::temp_dir().join(format!("omsi-doors-open-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("test.bus"),
        "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
    )
    .unwrap();
    std::fs::write(dir.join("model.cfg"), "").unwrap();
    // Front door leaf 0 uses PAX_Entry0_Open. Rear door (door_2) has no PAX_Exit0_Open in varlist.
    std::fs::write(
        dir.join("vars.txt"),
        "door_0\ndoor_1\ndoor_2\nPAX_Entry0_Open\n",
    )
    .unwrap();
    std::fs::write(dir.join("main.osc"), "{init}\n{end}\n").unwrap();

    let ty = std::sync::Arc::new(omsi_sim::VehicleType::load(&dir, &dir.join("test.bus")).unwrap());
    let mut v = VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()));

    // Initially both entries and exit closed
    let (e, x) = Humans::doors_open(&v, 2, 1);
    assert_eq!(e, vec![false, false]);
    assert_eq!(x, vec![false]);

    // Front door leaf 0 opens via PAX_Entry0_Open
    v.set_var("PAX_Entry0_Open", 1.0);
    let (e, x) = Humans::doors_open(&v, 2, 1);
    assert_eq!(e, vec![true, false]);
    assert_eq!(x, vec![false]);

    // Rear door leaf 2 opens (falls back to door_2 since PAX_Exit0_Open is not in varlist)
    v.set_var("door_2", 1.0);
    let (e, x) = Humans::doors_open(&v, 2, 1);
    assert_eq!(e, vec![true, false]);
    assert_eq!(x, vec![true]);

    // Front door leaf 1 opens via door_1 fallback
    v.set_var("door_1", 1.0);
    let (e, x) = Humans::doors_open(&v, 2, 1);
    assert_eq!(e, vec![true, true]);
    assert_eq!(x, vec![true]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn doors_open_reads_pax_vars_the_script_writes_without_declaring() {
    let dir =
        std::env::temp_dir().join(format!("omsi-doors-open-undeclared-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("test.bus"),
        "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
    )
    .unwrap();
    std::fs::write(dir.join("model.cfg"), "").unwrap();
    std::fs::write(dir.join("vars.txt"), "door_0\n").unwrap();
    std::fs::write(
        dir.join("main.osc"),
        "{frame}\n1 (S.L.PAX_Entry0_Open)\n{end}\n",
    )
    .unwrap();

    let ty = std::sync::Arc::new(omsi_sim::VehicleType::load(&dir, &dir.join("test.bus")).unwrap());
    let mut v = VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()));
    v.set_var("door_0", 0.0);
    v.set_var("PAX_Entry0_Open", 1.0);
    let (e, _) = Humans::doors_open(&v, 1, 0);
    assert_eq!(e, vec![true]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn doors_open_3door_bus_handles_middle_and_rear_exits() {
    let dir = std::env::temp_dir().join(format!("omsi-doors-3door-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("test.bus"),
        "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
    )
    .unwrap();
    std::fs::write(dir.join("model.cfg"), "").unwrap();
    std::fs::write(
        dir.join("vars.txt"),
        "door_0\ndoor_1\ndoor_2\ndoor_3\ndoor_4\ndoor_5\n",
    )
    .unwrap();
    std::fs::write(dir.join("main.osc"), "{init}\n{end}\n").unwrap();

    let ty = std::sync::Arc::new(omsi_sim::VehicleType::load(&dir, &dir.join("test.bus")).unwrap());
    let mut v = VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()));

    // 3-door bus: 6 entries (all 3 doors), 4 exits (middle door leaves 2,3; rear door leaves 4,5)
    let (e, x) = Humans::doors_open(&v, 6, 4);
    assert_eq!(e, vec![false; 6]);
    assert_eq!(x, vec![false; 4]);

    // Middle doors (door_2 and door_3) open
    v.set_var("door_2", 1.0);
    v.set_var("door_3", 1.0);
    let (e, x) = Humans::doors_open(&v, 6, 4);
    assert_eq!(e, vec![false, false, true, true, false, false]);
    assert_eq!(x, vec![true, true, false, false]);

    // Rear doors (door_4 and door_5) open
    v.set_var("door_4", 1.0);
    v.set_var("door_5", 1.0);
    let (e, x) = Humans::doors_open(&v, 6, 4);
    assert_eq!(e, vec![false, false, true, true, true, true]);
    assert_eq!(x, vec![true, true, true, true]);

    std::fs::remove_dir_all(&dir).ok();
}

/// #189: on a kerb or pavement that slopes, somebody waiting stands on the surface and
/// somebody walking along it (to the door, back to a seat) stays on it within a hair; a
/// step down is followed at once, a step up within a kerb's height too.
#[test]
fn people_keep_the_floor_height_on_sloped_ground() {
    let surface = |x: f64, slope: f64| 0.15 + slope * x;
    for slope in [-0.15, -0.05, 0.0, 0.05, 0.15] {
        for x in [0.0, 2.5, 7.0] {
            assert_eq!(
                follow_surface(surface(x, slope) + 0.2, surface(x, slope), 0.0, 1.0 / 60.0),
                surface(x, slope)
            );
            assert_eq!(
                follow_surface(surface(x, slope) - 0.2, surface(x, slope), 0.0, 1.0 / 60.0),
                surface(x, slope)
            );
        }
        for dir in [1.0, -1.0] {
            let (mut x, mut z) = (3.0, surface(3.0, slope));
            let dt = 1.0 / 60.0;
            for _ in 0..300 {
                x += dir * 1.4 * dt;
                z = follow_surface(z, surface(x, slope), 1.4, dt);
                assert!(
                    (z - surface(x, slope)).abs() < 0.03,
                    "slope {slope}, dir {dir}: {:.3} m off",
                    z - surface(x, slope)
                );
            }
        }
    }
    assert_eq!(follow_surface(0.0, 2.0, 1.4, 1.0 / 60.0), 2.0);
    assert!((follow_surface(0.0, 0.2, 1.4, 0.5) - 0.2).abs() < 1e-9);
}

/// #171: a bus whose exit door opens only on a request. The passenger's request leaves
/// the passengers' side as `PAX_Exit<i>_Req` (`DoorWants`), reaches the AI bus's script
/// through `write_door_requests`, the script opens the door at the stop, and the open state
/// it reports (`PAX_Exit<i>_Open`) is what the passengers read back as the exit being open.
/// The door script here is a stand-in written for the test (open = asked for and at a
/// stop; the host reports "at a stop" as -1, as the AI scripts read it), not any bus's own.
#[test]
fn a_request_opens_the_manual_exit_door_of_an_ai_bus_and_the_passenger_sees_it() {
    let script = "{frame_ai}\n(L.L.PAX_Exit0_Req) (L.L.AI_Scheduled_AtStation) * -1 * (S.L.PAX_Exit0_Open)\n{end}\n";
    let vars = "PAX_Exit0_Req\nPAX_Exit0_Open\nPAX_Entry0_Open\nAI_Scheduled_AtStation\n";
    let mut bus = testing::script_test_vehicle(script, vars, "");
    let doors = |exit_req: bool| DoorWants {
        entry_req: vec![false],
        exit_req: vec![exit_req],
        entry_busy: vec![false],
        exit_busy: vec![false],
        places: Vec::new(),
    };
    let frame = |bus: &mut VehicleInstance, at_stop: bool, exit_req: bool| {
        // (the order of the app: the traffic sets the stop state, the passengers' requests go in,
        // the AI script runs, the passengers read the doors)
        bus.set_var("AI_Scheduled_AtStation", if at_stop { -1.0 } else { 0.0 });
        Humans::write_door_requests(bus, &doors(exit_req));
        bus.update_ai(0.05, &Default::default());
        Humans::doors_open(bus, 1, 1).1[0]
    };
    // driving, with a request: shut; at the stop without a request: shut (manual door)
    assert!(!frame(&mut bus, false, true), "open while driving");
    assert!(!frame(&mut bus, true, false), "opened without a request");
    // at the stop, with the request: open, and the passengers see it open
    assert!(
        frame(&mut bus, true, true),
        "a request at the stop did not open the exit door"
    );
    // the request is dropped when nobody asks any more
    assert!(!frame(&mut bus, true, false));
}

/// #718: a trailer on a lorry's hitch (`[coupling_front_character]` type 0) whose cabin
/// gives no `[linkToNextVeh]` is not walked into from the bus: it is a section of its
/// own, boarded, ridden in and left by its own doors. A bus joint (type 1) without the
/// points is still walked through at the aisle's ends.
#[test]
fn a_trailer_nobody_walks_into_is_boarded_by_its_own_doors() {
    let dir = std::env::temp_dir().join(format!("omsi-trailer-cabin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
    // the bus: door at the front right, an aisle, a seat; the trailer: a door, an aisle,
    // a seat and an exit, nothing that names the bus
    write("paths_bus.cfg", "[pathpnt]\n1.1\n4\n0.4\n[pathpnt]\n0\n4\n0.5\n[pathpnt]\n0\n-4\n0.5\n[pathlink]\n0\n1\n[pathlink]\n1\n2\n");
    write(
        "cabin_bus.cfg",
        "[entry]\n0\n[exit]\n0\n[stamper]\n1\n0.5\n4\n1.5\n[passpos]\n-0.5\n2\n1.0\n0.45\n0\n",
    );
    write("paths_trail.cfg", "[pathpnt]\n1.1\n2\n0.4\n[pathpnt]\n0\n2\n0.5\n[pathpnt]\n0\n-2\n0.5\n[pathlink]\n0\n1\n[pathlink]\n1\n2\n");
    write(
        "cabin_trail.cfg",
        "[entry]\n0\n[exit]\n0\n[passpos]\n-0.5\n-1\n1.0\n0.45\n0\n",
    );
    // trailers with a way in and none out, and the other way round
    write(
        "cabin_trail_in.cfg",
        "[entry]\n0\n[passpos]\n-0.5\n-1\n1.0\n0.45\n0\n",
    );
    write(
        "cabin_trail_out.cfg",
        "[exit]\n0\n[passpos]\n-0.5\n-1\n1.0\n0.45\n0\n",
    );
    // (a bus for its script's variables)
    write(
        "vars.bus",
        "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
    );
    write("model.cfg", "");
    write("vars.txt", "door_0\n");
    write("main.osc", "{init}\n{end}\n");
    let def = |cabin: &str, paths: &str, coupling: Option<f32>| omsi_vehicle::Vehicle {
        path: dir.join("bus.bus"),
        passenger_cabin: Some(cabin.into()),
        paths: Some(paths.into()),
        bounding_box: Some([2.5, 9.0, 3.0, 0.0, 0.0, 1.5]),
        coupling_front_character: coupling.map(|kind| [30.0, -10.0, 10.0, kind]),
        ..Default::default()
    };
    let bus = def("cabin_bus.cfg", "paths_bus.cfg", None);
    let offset = Vec3::new(0.0, -10.0, 0.0);
    let cabin_of = |cabin: &str, kind: Option<f32>| {
        let trail = def(cabin, "paths_trail.cfg", kind);
        Cabin::load_train(&[(&bus, Vec3::ZERO, f32::INFINITY), (&trail, offset, -5.0)])
            .expect("cabin")
    };
    let hitched = cabin_of("cabin_trail.cfg", Some(0.0));
    let jointed = cabin_of("cabin_trail.cfg", Some(1.0));
    let plain = cabin_of("cabin_trail.cfg", None);
    let no_exit = cabin_of("cabin_trail_in.cfg", Some(0.0));
    let no_entry = cabin_of("cabin_trail_out.cfg", Some(0.0));
    let jointed_no_exit = cabin_of("cabin_trail_in.cfg", Some(1.0));
    let ty = std::sync::Arc::new(omsi_sim::VehicleType::load(&dir, &dir.join("vars.bus")).unwrap());
    std::fs::remove_dir_all(&dir).ok();
    let v = VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()));
    assert_eq!(
        (jointed.groups, plain.groups),
        (1, 1),
        "a bus joint is walked through"
    );
    assert_eq!(jointed.links.len(), 5);
    assert_eq!(hitched.groups, 2);
    assert_eq!(
        hitched.links.len(),
        4,
        "nothing joins the bus to its trailer"
    );
    assert_eq!(hitched.parts.len(), 2);
    // its doors and its seat are the trailer's, behind the bus
    assert_eq!(
        (
            hitched.entries.len(),
            hitched.exits.len(),
            hitched.seats.len()
        ),
        (2, 2, 2)
    );
    assert_eq!(
        (
            hitched.entries[1].group,
            hitched.exits[1].group,
            hitched.seats[1].group
        ),
        (1, 1, 1)
    );
    assert!((hitched.entries[1].inside - Vec3::new(1.1, -8.0, 0.4)).length() < 1e-4);
    // somebody for the trailer's seat walks to the trailer's door, a rider in it to its exit
    let entries = hitched.in_group(hitched.entry_points(), Some(1));
    assert_eq!(entries, [None, Some(3)]);
    let near_bus_door = Vec3::new(1.6, 4.0, 0.0);
    assert_eq!(
        hitched.omsi_nearest(near_bus_door, &entries, false, false, None, None),
        Some(3)
    );
    assert_eq!(
        hitched.in_group(hitched.exit_points(), hitched.group_at(Some(5))),
        [None, Some(3)]
    );
    assert_eq!(
        hitched.in_group(hitched.exit_points(), Some(0)),
        [Some(0), None]
    );
    // with the trailer's doors shut (and no button) they wait at those, not at nothing:
    // the bus's door, the first of the list, is left out of it
    let here = Vec3::new(0.0, -8.0, 0.5);
    let exits = hitched.in_group(hitched.exit_points(), Some(1));
    assert_eq!(
        hitched.omsi_nearest(here, &exits, false, false, None, Some(&[false, false])),
        Some(3)
    );
    let flags = hitched.entry_flags();
    for avoid in [false, true] {
        assert_eq!(
            hitched.omsi_nearest(
                near_bus_door,
                &entries,
                avoid,
                false,
                Some(&flags),
                Some(&[false, false])
            ),
            Some(3)
        );
    }
    // (one group: the list's first, as OMSI)
    assert_eq!(
        jointed.omsi_nearest(
            here,
            &[None, Some(3)],
            false,
            false,
            None,
            Some(&[false, false])
        ),
        None
    );
    // a trailer of its own nobody can get into, or out of again, has its places off
    assert_eq!(places_off(&v, &hitched), [false, false]);
    assert_eq!(
        places_off(&v, &no_exit),
        [false, true],
        "nobody could get out of it"
    );
    assert_eq!(
        places_off(&v, &no_entry),
        [false, true],
        "nobody could get into it"
    );
    assert_eq!(
        places_off(&v, &jointed_no_exit),
        [false, false],
        "out through the bus"
    );
    // the bus's validator is out of a trailer passenger's reach
    assert_eq!(hitched.nearest_stamper(Vec3::new(1.1, -8.0, 0.4)), None);
    assert_eq!(hitched.nearest_stamper(Vec3::new(1.1, 4.0, 0.4)), Some(0));
    assert_eq!(jointed.nearest_stamper(Vec3::new(1.1, -8.0, 0.4)), Some(0));
}

/// #44: an articulated bus whose rear door has a button. Everybody walking up while the
/// doors are still shut makes for that door - the only one worth waiting at -, and
/// they stayed there once the driver opened the others, the bus boarded by its last
/// door alone. With a door open, a shut one with a button draws nobody.
#[test]
fn with_a_door_open_nobody_waits_at_a_shut_button_door() {
    let dir = std::env::temp_dir().join(format!("omsi-artic-entries-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
    // the front section: a front door (point 0) and a middle one (point 3) on an aisle
    // to the joint; the rear section: a door with a button (point 2) behind it
    write("paths_front.cfg", "[pathpnt]\n1.1\n4\n0.4\n[pathpnt]\n0\n4\n0.5\n[pathpnt]\n0\n-4\n0.5\n[pathpnt]\n1.1\n-2\n0.4\n[pathlink]\n0\n1\n[pathlink]\n1\n2\n[pathlink]\n3\n2\n");
    write(
        "cabin_front.cfg",
        "[entry]\n0\n[entry]\n3\n[exit]\n3\n[linkToPrevVeh]\n2\n[passpos]\n-0.5\n2\n1.0\n0.45\n0\n",
    );
    write("paths_rear.cfg", "[pathpnt]\n0\n3\n0.5\n[pathpnt]\n0\n-2\n0.5\n[pathpnt]\n1.1\n-2\n0.4\n[pathlink]\n0\n1\n[pathlink]\n1\n2\n");
    write("cabin_rear.cfg", "[entry]\n2\n{withbutton}\n[exit]\n2\n[linkToNextVeh]\n0\n[passpos]\n-0.5\n-1\n1.0\n0.45\n0\n");
    let def = |cabin: &str, paths: &str, coupling: Option<f32>| omsi_vehicle::Vehicle {
        path: dir.join("bus.bus"),
        passenger_cabin: Some(cabin.into()),
        paths: Some(paths.into()),
        bounding_box: Some([2.5, 9.0, 3.0, 0.0, 0.0, 1.5]),
        coupling_front_character: coupling.map(|kind| [30.0, -10.0, 10.0, kind]),
        ..Default::default()
    };
    let front = def("cabin_front.cfg", "paths_front.cfg", None);
    let rear = def("cabin_rear.cfg", "paths_rear.cfg", Some(1.0));
    let cabin = Cabin::load_train(&[
        (&front, Vec3::ZERO, f32::INFINITY),
        (&rear, Vec3::new(0.0, -10.0, 0.0), -5.0),
    ])
    .expect("cabin");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        (cabin.parts.len(), cabin.groups),
        (2, 1),
        "one saloon of two sections"
    );
    let entries = cabin.entry_points();
    assert_eq!(entries, [Some(0), Some(3), Some(6)]);
    let flags = cabin.entry_flags();
    assert_eq!(
        flags.iter().map(|f| f.1).collect::<Vec<_>>(),
        [false, false, true],
        "the rear door has the button"
    );
    // by the front door, by the middle one, and level with the rear one (bus frame)
    let (by_front, by_middle, by_rear) = (
        Vec3::new(1.6, 4.0, 0.0),
        Vec3::new(1.6, -2.0, 0.0),
        Vec3::new(1.6, -12.0, 0.0),
    );
    let nearest = |p: Vec3, open: &[bool]| {
        cabin.omsi_nearest(p, &entries, false, false, Some(&flags), Some(open))
    };
    // every door shut: the button door is the one to wait at, from wherever
    for p in [by_front, by_middle, by_rear] {
        assert_eq!(nearest(p, &[false, false, false]), Some(6));
    }
    // the front sections' doors open, the rear one shut: each to the nearest open door
    // (they walked on to the shut rear door)
    assert_eq!(nearest(by_front, &[true, true, false]), Some(0));
    assert_eq!(nearest(by_middle, &[true, true, false]), Some(3));
    assert_eq!(nearest(by_rear, &[true, true, false]), Some(3));
    // every door open: the nearest
    assert_eq!(nearest(by_front, &[true, true, true]), Some(0));
    assert_eq!(nearest(by_middle, &[true, true, true]), Some(3));
    assert_eq!(nearest(by_rear, &[true, true, true]), Some(6));
    // one section's door in a list narrowed to another's (#718) does not count as open
    let narrowed = [None, None, Some(6)];
    assert_eq!(
        cabin.omsi_nearest(
            by_front,
            &narrowed,
            false,
            false,
            Some(&flags),
            Some(&[true, true, false])
        ),
        Some(6)
    );
}

/// #722: a validator by each door - every `[stamper]` is kept, and a passenger stamps at
/// the one nearest where they came in (OMSI kept the file's last one only).
#[test]
fn passengers_stamp_at_the_validator_nearest_their_door() {
    let dir = std::env::temp_dir().join(format!("omsi-stampers-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("test.bus"),
        "[passengercabin]\ncabin.cfg\n\n[paths]\npaths.cfg\n",
    )
    .unwrap();
    // an aisle from the front door (y 5) to the rear one (y -4)
    let pts: String = [5.0, 4.0, -3.0, -4.0]
        .iter()
        .map(|y| format!("[pathpnt]\n0\n{y}\n0.5\n\n"))
        .collect();
    std::fs::write(
        dir.join("paths.cfg"),
        format!("{pts}[pathlink]\n0\n1\n\n[pathlink]\n1\n2\n\n[pathlink]\n2\n3\n"),
    )
    .unwrap();
    std::fs::write(
        dir.join("cabin.cfg"),
        "[entry]\n0\n\n[entry]\n3\n\n[stamper]\n1\n0.5\n4\n1.5\n\n[stamper]\n2\n0.5\n-3\n1.5\n\n[passpos]\n0\n0\n0.5\n0\n0\n",
    )
    .unwrap();
    let def = omsi_vehicle::Vehicle::load(&dir.join("test.bus")).unwrap();
    let cabin = Cabin::load_train(&[(&def, Vec3::ZERO, f32::INFINITY)]).expect("cabin");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(cabin.stampers.len(), 2);
    assert_eq!(cabin.stampers[1], (Some(2), Vec3::new(0.5, -3.0, 1.5)));
    assert_eq!(
        cabin.nearest_stamper(Vec3::new(0.0, 5.0, 0.5)),
        Some(0),
        "in by the front door"
    );
    assert_eq!(
        cabin.nearest_stamper(Vec3::new(0.0, -4.0, 0.5)),
        Some(1),
        "in by the rear door"
    );
}

/// #719: a five-door bus with two paths a door has ten entries. Past OMSI's eight,
/// an entry whose `PAX_Entry<n>_Open` the script gives is a door of its own, and its
/// people ask through its `PAX_Entry<n>_Req`; one the script does not know opens with
/// the eighth, as in OMSI, and asks through it (OMSI drops that request).
#[test]
fn doors_past_the_eighth_have_variables_of_their_own_or_go_with_the_eighth() {
    let bus = |name: &str, vars: &str| {
        let dir = std::env::temp_dir().join(format!(
            "omsi-doors-past-eight-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("test.bus"),
            "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
        )
        .unwrap();
        std::fs::write(dir.join("model.cfg"), "").unwrap();
        std::fs::write(dir.join("vars.txt"), vars).unwrap();
        std::fs::write(dir.join("main.osc"), "{init}\n{end}\n").unwrap();
        let ty =
            std::sync::Arc::new(omsi_sim::VehicleType::load(&dir, &dir.join("test.bus")).unwrap());
        std::fs::remove_dir_all(&dir).ok();
        VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()))
    };
    let mut req = vec![false; 10];
    req[9] = true;

    // the script has the ten doors' variables
    let all: String = (0..10).map(|i| format!("PAX_Entry{i}_Open\n")).collect();
    let mut v = bus("own", &all);
    v.set_var("PAX_Entry9_Open", 1.0);
    let (e, _) = Humans::doors_open(&v, 10, 0);
    assert_eq!(
        e,
        [false, false, false, false, false, false, false, false, false, true]
    );
    Humans::write_door_requests(
        &mut v,
        &DoorWants {
            entry_req: req.clone(),
            ..Default::default()
        },
    );
    assert_eq!(
        (v.var("PAX_Entry9_Req"), v.var("PAX_Entry7_Req")),
        (Some(1.0), Some(0.0))
    );

    // an older bus knows eight: the ninth and tenth go with the eighth
    let eight: String = (0..8).map(|i| format!("PAX_Entry{i}_Open\n")).collect();
    let mut v = bus("eight", &eight);
    v.set_var("PAX_Entry7_Open", 1.0);
    let (e, _) = Humans::doors_open(&v, 10, 0);
    assert_eq!(
        e,
        [false, false, false, false, false, false, false, true, true, true]
    );
    Humans::write_door_requests(
        &mut v,
        &DoorWants {
            entry_req: req.clone(),
            ..Default::default()
        },
    );
    assert_eq!(
        (v.var("PAX_Entry9_Req"), v.var("PAX_Entry7_Req")),
        (Some(0.0), Some(1.0))
    );
}

/// #721: a `[passpos]` that names a variable of its own is offered to passengers only
/// while the script holds that variable off 0, and one that names an occupancy variable
/// has it set while a rider is on it - a tip-up seat follows the person on it, a second
/// seating layout comes and goes with a setvar.
#[test]
fn places_follow_their_own_variables() {
    let dir = std::env::temp_dir().join(format!("omsi-place-vars-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("test.bus"),
        "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n\n[passengercabin]\ncabin.cfg\n\n[paths]\npaths.cfg\n",
    )
    .unwrap();
    std::fs::write(dir.join("model.cfg"), "").unwrap();
    std::fs::write(dir.join("vars.txt"), "layout_long\nfold_seat_down\n").unwrap();
    std::fs::write(dir.join("main.osc"), "{init}\n{end}\n").unwrap();
    std::fs::write(
        dir.join("paths.cfg"),
        "[pathpnt]\n0\n4\n0.5\n\n[pathpnt]\n0\n-3\n0.5\n\n[pathlink]\n0\n1\n",
    )
    .unwrap();
    // a seat of the long layout, a tip-up seat, a plain standing place, and a place whose
    // variable the script does not have
    std::fs::write(
        dir.join("cabin.cfg"),
        "[entry]\n0\n\n[passpos]\n-0.5\n2\n0.9\n0.45\n0\nlayout_long\n\n[passpos]\n0.5\n1\n0.9\n0.45\n0\nlayout_any\nfold_seat_down\n\n\
         [passpos]\n0\n0\n0.5\n0\n0\n\n[passpos]\n0\n-1\n0.5\n0\n0\nno_such_var\n",
    )
    .unwrap();
    let ty = std::sync::Arc::new(omsi_sim::VehicleType::load(&dir, &dir.join("test.bus")).unwrap());
    let def = omsi_vehicle::Vehicle::load(&dir.join("test.bus")).unwrap();
    let cabin = Cabin::load_train(&[(&def, Vec3::ZERO, f32::INFINITY)]).expect("cabin");
    std::fs::remove_dir_all(&dir).ok();
    let mut v = VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()));
    v.set_var("layout_long", 0.0);
    let off = places_off(&v, &cabin);
    assert_eq!(
        off,
        [true, false, false, false],
        "only the place whose variable is 0 is off"
    );
    let mut h = Humans::new(Path::new("/nonexistent"));
    for _ in 0..40 {
        let k = h
            .reserve_place(BusId::Player, &cabin.seats, &off)
            .expect("a free place");
        assert_ne!(k, 0, "nobody takes a place that is switched off");
        h.free_seat(BusId::Player, k);
    }
    assert_eq!(
        h.reserve_place(BusId::Player, &cabin.seats, &[true; 4]),
        None,
        "all off: nobody gets on"
    );
    // a free seat comes first (places 0 and 1 are seats, 2 and 3 standing places)
    for k in 0..4 {
        h.free_seat(BusId::Player, k);
    }
    let mut stood = 0;
    for _ in 0..2000 {
        let k = h
            .reserve_place(BusId::Player, &cabin.seats, &[false; 4])
            .expect("a free place");
        stood += (k >= 2) as u32;
        h.free_seat(BusId::Player, k);
    }
    assert!(
        (40..=160).contains(&stood),
        "about 5% of 2000 stand beside free seats: {stood}"
    );
    // seats full: the next stands, and with the standing places full as well nobody gets on
    let all: Vec<usize> = (0..4)
        .map(|_| {
            h.reserve_place(BusId::Player, &cabin.seats, &[false; 4])
                .unwrap()
        })
        .collect();
    let mut sorted = all.clone();
    sorted.sort();
    assert_eq!(sorted, [0, 1, 2, 3]);
    assert_eq!(
        h.reserve_place(BusId::Player, &cabin.seats, &[false; 4]),
        None,
        "all taken"
    );
    for k in 0..4 {
        h.free_seat(BusId::Player, k);
    }
    // seats taken, standing places free: always standing
    h.reserve_place(BusId::Player, &cabin.seats, &[false, false, true, true]);
    h.reserve_place(BusId::Player, &cabin.seats, &[false, false, true, true]);
    let k = h
        .reserve_place(BusId::Player, &cabin.seats, &[false; 4])
        .unwrap();
    assert!(k >= 2, "no seat free: a standing place");
    for k in 0..4 {
        h.free_seat(BusId::Player, k);
    }
    v.set_var("layout_long", 1.0);
    assert_eq!(places_off(&v, &cabin), [false; 4]);
    // somebody sits on the tip-up seat (place 1) of this bus, somebody on place 1 of another
    let bn = BusNow {
        id: BusId::Player,
        cabin: Arc::new(cabin),
        pos: DVec3::ZERO,
        rot: Mat4::IDENTITY,
        heading: 0.0,
        speed: 0.0,
        entry_open: vec![false],
        exit_open: Vec::new(),
        walk_open: None,
        interior: 0.0,
        air: CabinAir::default(),
        half: DVec2::new(1.25, 6.0),
        centre: DVec2::ZERO,
        accel: DVec2::ZERO,
        trailers: Vec::new(),
        terminus: None,
        takes: Takes::Terminus,
        next_stop: None,
        places_off: Vec::new(),
        served: None,
    };
    let taken = places_taken(&bn, &[(BusId::Player, 1), (BusId::Ai(2), 2)]);
    assert_eq!(taken, [("fold_seat_down".to_string(), true)]);
    Humans::write_door_requests(
        &mut v,
        &DoorWants {
            places: taken,
            ..Default::default()
        },
    );
    assert_eq!(v.var("fold_seat_down"), Some(1.0));
    Humans::write_door_requests(
        &mut v,
        &DoorWants {
            places: places_taken(&bn, &[]),
            ..Default::default()
        },
    );
    assert_eq!(
        v.var("fold_seat_down"),
        Some(0.0),
        "up again once they have got up"
    );
}

/// A timetable bus boarding at a stop the passengers' stops do not have (#1593): its
/// riders get off at its timetable's stop instead of waiting at a shut door for good.
#[test]
fn riders_get_off_at_a_timetable_stop_the_nearby_stops_do_not_have() {
    let dir = std::env::temp_dir().join(format!("omsi-served-stop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.bus"), "[boundingbox]\n2.5\n12\n3\n0\n0\n1.5\n\n[passengercabin]\ncabin.cfg\n\n[paths]\npaths.cfg\n").unwrap();
    std::fs::write(dir.join("paths.cfg"), "[pathpnt]\n1.1\n4\n0.4\n\n[pathpnt]\n0\n4\n0.5\n\n[pathpnt]\n0\n-3\n0.5\n\n[pathpnt]\n1.1\n-3\n0.4\n\n[pathlink]\n0\n1\n\n[pathlink]\n1\n2\n\n[pathlink]\n2\n3\n").unwrap();
    std::fs::write(
        dir.join("cabin.cfg"),
        "[entry]\n0\n\n[exit]\n3\n\n[passpos]\n-0.5\n0\n0.9\n0.45\n0\n",
    )
    .unwrap();
    let def = omsi_vehicle::Vehicle::load(&dir.join("test.bus")).unwrap();
    let cabin = Arc::new(Cabin::load_train(&[(&def, Vec3::ZERO, f32::INFINITY)]).expect("cabin"));
    std::fs::remove_dir_all(&dir).ok();
    let bus = |served| BusNow {
        id: BusId::Ai(5),
        cabin: cabin.clone(),
        pos: DVec3::new(-198.0, 2359.0, 0.0),
        rot: Mat4::IDENTITY,
        heading: 0.0,
        speed: 0.0,
        entry_open: vec![true],
        exit_open: vec![true],
        walk_open: None,
        interior: 0.0,
        air: CabinAir::default(),
        half: DVec2::new(1.25, 6.0),
        centre: DVec2::ZERO,
        accel: DVec2::ZERO,
        trailers: Vec::new(),
        terminus: None,
        takes: Takes::Terminus,
        next_stop: None,
        places_off: Vec::new(),
        served,
    };
    let mut h = Humans::new(Path::new("/nonexistent"));
    let reg = h.register_buses(&[bus(Some(496193))], 0.0);
    assert_eq!(reg[&BusId::Ai(5)].next, Some(496193));
    let reg = h.register_buses(&[bus(None)], 0.0);
    assert_eq!(
        reg[&BusId::Ai(5)].next,
        None,
        "no stop near and none served: nowhere to get off"
    );
}

/// #720: `PAX_Entry<n>_Busy` / `PAX_Exit<n>_Busy` tell a door script that somebody stands
/// in that doorway - on the threshold or in the opening, not in the queue outside a shut
/// door, the aisle or the deck above - and go with the frame like the requests.
#[test]
fn a_doorway_is_busy_while_somebody_stands_in_it() {
    let dir = std::env::temp_dir().join(format!("omsi-doorway-busy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.bus"), "[boundingbox]\n2.5\n12\n3\n0\n0\n1.5\n\n[passengercabin]\ncabin.cfg\n\n[paths]\npaths.cfg\n").unwrap();
    // the front door's threshold on the right (x 1.1), an aisle point beside it, the
    // rear door's threshold at y -3
    std::fs::write(dir.join("paths.cfg"), "[pathpnt]\n1.1\n4\n0.4\n\n[pathpnt]\n0\n4\n0.5\n\n[pathpnt]\n0\n-3\n0.5\n\n[pathpnt]\n1.1\n-3\n0.4\n\n[pathlink]\n0\n1\n\n[pathlink]\n1\n2\n\n[pathlink]\n2\n3\n").unwrap();
    std::fs::write(
        dir.join("cabin.cfg"),
        "[entry]\n0\n\n[exit]\n3\n\n[passpos]\n-0.5\n0\n0.9\n0.45\n0\n",
    )
    .unwrap();
    let def = omsi_vehicle::Vehicle::load(&dir.join("test.bus")).unwrap();
    let cabin = Cabin::load_train(&[(&def, Vec3::ZERO, f32::INFINITY)]).expect("cabin");
    let door = &cabin.entries[0];
    assert!(
        door.in_doorway(Vec3::new(1.1, 4.0, 0.4)),
        "on the threshold"
    );
    assert!(door.in_doorway(Vec3::new(1.35, 4.1, 0.0)), "in the opening");
    assert!(
        !door.in_doorway(Vec3::new(1.8, 4.0, 0.0)),
        "waiting outside the shut door"
    );
    // (the bus is 2.5 m wide: walking past along the line 0.5 m out from its side)
    assert!(
        !door.in_doorway(Vec3::new(1.25 + 0.5, 4.0, 0.0)),
        "walking past the door"
    );
    assert!(
        !door.in_doorway(Vec3::new(1.25 + 0.5, 4.0, 0.4)),
        "walking past the door"
    );
    assert!(!door.in_doorway(Vec3::new(0.0, 4.0, 0.5)), "in the aisle");
    assert!(
        !door.in_doorway(Vec3::new(1.1, 4.0, 2.4)),
        "on the deck above"
    );
    assert!(
        !door.in_doorway(Vec3::new(1.1, 2.5, 0.4)),
        "beside the door"
    );
    assert!(cabin.exits[0].in_doorway(Vec3::new(1.2, -3.0, 0.4)));
    assert!(!cabin.exits[0].in_doorway(Vec3::new(1.2, 4.0, 0.4)));
    // a bus standing at (100, 200) heading east: somebody stepping in from the kerb (in
    // the world) and a rider at the rear door (in its frame)
    let rot = Mat4::from_rotation_z(-90f32.to_radians());
    let bn = BusNow {
        id: BusId::Ai(3),
        cabin: Arc::new(cabin),
        pos: DVec3::new(100.0, 200.0, 0.0),
        rot,
        heading: 90.0,
        speed: 0.0,
        entry_open: vec![true],
        exit_open: vec![true],
        walk_open: None,
        interior: 0.0,
        air: CabinAir::default(),
        half: DVec2::new(1.25, 6.0),
        centre: DVec2::ZERO,
        accel: DVec2::ZERO,
        trailers: Vec::new(),
        terminus: None,
        takes: Takes::Terminus,
        next_stop: None,
        places_off: Vec::new(),
        served: None,
    };
    let step_in = bn.world(Vec3::new(1.35, 4.0, 0.0));
    assert_eq!(
        doorways_taken(&bn, &[(None, step_in)]),
        (vec![true], vec![false])
    );
    let rear = DVec3::new(1.1, -3.0, 0.4);
    assert_eq!(
        doorways_taken(&bn, &[(Some(BusId::Ai(3)), rear)]),
        (vec![false], vec![true])
    );
    // a rider of another bus, somebody waiting further out on the pavement: nobody here
    let waiting = bn.world(Vec3::new(2.6, 4.0, 0.0));
    assert_eq!(
        doorways_taken(&bn, &[(Some(BusId::Player), rear), (None, waiting)]),
        (vec![false], vec![false])
    );
    // into the script's variables, a door past the eighth with the eighth's
    std::fs::write(
        dir.join("vars.bus"),
        "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
    )
    .unwrap();
    std::fs::write(dir.join("model.cfg"), "").unwrap();
    std::fs::write(dir.join("vars.txt"), "door_0\n").unwrap();
    std::fs::write(dir.join("main.osc"), "{init}\n{end}\n").unwrap();
    let ty = std::sync::Arc::new(omsi_sim::VehicleType::load(&dir, &dir.join("vars.bus")).unwrap());
    std::fs::remove_dir_all(&dir).ok();
    let mut v = VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()));
    let mut entry_busy = vec![false; 10];
    entry_busy[9] = true;
    Humans::write_door_requests(
        &mut v,
        &DoorWants {
            entry_busy,
            exit_busy: vec![false, true],
            ..Default::default()
        },
    );
    assert_eq!(
        (v.var("PAX_Entry7_Busy"), v.var("PAX_Entry0_Busy")),
        (Some(1.0), Some(0.0))
    );
    assert_eq!(
        (v.var("PAX_Exit1_Busy"), v.var("PAX_Exit0_Busy")),
        (Some(1.0), Some(0.0))
    );
    // nobody there in the next frame: gone after the scripts' frame, as the requests
    v.update(0.02);
    assert_eq!(
        (v.var("PAX_Entry7_Busy"), v.var("PAX_Exit1_Busy")),
        (Some(0.0), Some(0.0))
    );
}
