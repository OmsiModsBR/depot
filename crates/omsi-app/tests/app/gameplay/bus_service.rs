use omsi_sim::traffic::{AiState, LaneKind, Network};
use omsi_sim::VehicleInstance;
/// A bus with a long wait at its stop switches its engine off after six seconds and on
/// twenty seconds before it leaves; a short stop keeps it running (#1404).
#[test]
fn the_engine_rests_through_a_long_wait() {
    let mut b = testing::bus_service::BusService::new(Vec::new());
    b.phase = testing::bus_service::Phase::Waiting;
    b.arrived_at = 1000.0;
    b.leave_at = 1300.0;
    assert!(b.engine_running(1003.0), "just arrived");
    assert!(!b.engine_running(1010.0), "resting");
    assert!(b.engine_running(1285.0), "about to leave");
    b.leave_at = 1040.0;
    assert!(b.engine_running(1010.0), "a short stop");
    b.leave_at = 1300.0;
    b.phase = testing::bus_service::Phase::Running;
    assert!(b.engine_running(1010.0), "driving");
}

#[test]
fn scheduled_ai_exposes_timetable_during_init_and_frame_ai() {
    let template = testing::script_test_vehicle(
        "{init}\n(L.L.schedule_active) (S.L.init_active)\n(M.V.GetTTBusstopCount) (S.L.init_count)\n(M.V.GetTTLineString) (S.$.init_line)\n{end}\n{frame_ai}\n(L.L.schedule_active) (S.L.frame_active)\n(M.V.GetTTBusstopIndex) (S.L.frame_index)\n{end}\n",
        "schedule_active\ninit_active\ninit_count\nframe_active\nframe_index\n",
        "init_line\n",
    );
    let stop = Stop::from_tuple((0, 10.0, 0.0, 120.0, 7, 0.0));
    let timetable = AiTimetable {
        line: "042".into(),
        terminus: "terminus".into(),
        stops: vec![
            (7, "First".into(), 100.0, 120.0),
            (8, "Last".into(), 200.0, 220.0),
        ],
    };
    let mut host = omsi_sim::VehicleHost::new(Default::default());
    timetable.install(&mut host, Some(&stop));
    let mut vehicle = VehicleInstance::new(template.ty.clone(), host);
    assert_eq!(vehicle.var("init_active"), Some(1.0));
    assert_eq!(vehicle.var("init_count"), Some(2.0));
    assert_eq!(vehicle.str_var("init_line"), "042");
    vehicle.set_var("schedule_active", 0.0);
    vehicle.update_ai(0.01, &Default::default());
    assert_eq!(vehicle.var("frame_active"), Some(1.0));
    assert_eq!(vehicle.var("frame_index"), Some(0.0));
    // Ordinary traffic has no assigned timetable; it must remain inactive.
    let mut ordinary = VehicleInstance::new(
        template.ty.clone(),
        omsi_sim::VehicleHost::new(Default::default()),
    );
    ordinary.update_ai(0.01, &Default::default());
    assert_eq!(ordinary.var("init_active"), Some(0.0));
    assert_eq!(ordinary.var("frame_active"), Some(0.0));
}

#[test]
fn timetable_tracks_repeated_stops_streaming_and_trip_changes() {
    let stop = |id, depart| Stop::from_tuple((0, 10.0, 0.0, depart, id, 0.0));
    let first = stop(7, 120.0);
    let second = stop(8, 220.0);
    let repeated = stop(7, 320.0);
    let timetable = AiTimetable {
        line: "42".into(),
        terminus: "terminal".into(),
        stops: vec![
            (7, "A".into(), 100.0, 120.0),
            (8, "B".into(), 200.0, 220.0),
            (7, "A".into(), 300.0, 320.0),
        ],
    };
    let mut v = testing::script_test_vehicle("{frame_ai}\n{end}\n", "schedule_active\n", "");
    v.host.hof = Some(std::sync::Arc::new(omsi_vehicle::Hof {
        termini: vec![omsi_vehicle::hof::Terminus {
            texture_id: "terminal".into(),
            ..Default::default()
        }],
        ..Default::default()
    }));
    timetable.install(&mut v.host, Some(&first));
    assert_eq!(v.host.tt_terminus_index, 0);
    let mut service = BusService::new(vec![first, second, repeated]);
    service.phase = Phase::Boarding;
    service.feed_timetable(&mut v, 110.0);
    assert_eq!(v.host.tt_delay, -10.0);
    service.stops.pop_front();
    service.phase = Phase::Running;
    service.delay = 5.0;
    service.feed_timetable(&mut v, 210.0);
    assert_eq!(v.host.tt_busstop_index, 1);
    assert_eq!(v.host.tt_delay, 10.0);
    service.stops.pop_front();
    service.feed_timetable(&mut v, 310.0);
    assert_eq!(v.host.tt_busstop_index, 2);
    // Exhausting the loaded geometry does not exhaust the script's timetable.
    service.stops.clear();
    service.route_open = true;
    service.feed_timetable(&mut v, 315.0);
    assert_eq!(v.host.tt_stops.len(), 3);
    assert_eq!(v.host.tt_busstop_index, 2);
    assert_eq!(v.host.schedule_active, 1.0);
    let next = AiTimetable {
        line: "43".into(),
        terminus: "missing".into(),
        stops: vec![(9, "C".into(), 400.0, 420.0)],
    };
    next.install(&mut v.host, Some(&stop(9, 420.0)));
    assert_eq!(v.host.tt_line, "43");
    assert_eq!(v.host.tt_stops.len(), 1);
    assert_eq!(v.host.tt_busstop_index, 0);
    assert_eq!(v.host.tt_terminus_index, -1);
    assert_eq!(v.host.tt_delay, 0.0);
}

use openomsi_game::testing;
use openomsi_game::testing::bus_service::*;

/// Somebody coming from another stop than the one the bus serves does not keep it there
/// (#767); somebody on the way out does, wherever.
#[test]
fn a_hold_counts_at_the_stop_served() {
    let stop = Stop {
        ri: 0,
        s: 0.0,
        bay: 0.0,
        depart: 0.0,
        id: 42,
        side: 0.0,
    };
    let mut s = BusService::new(vec![stop]);
    s.phase = Phase::Boarding;
    s.boarding = 0.0;
    s.hold(Some(41), 2.5);
    assert_eq!(s.boarding, 0.0);
    s.hold(Some(42), 2.5);
    assert_eq!(s.boarding, 2.5);
    s.boarding = 0.0;
    s.hold(None, 2.5);
    assert_eq!(s.boarding, 2.5);
}

#[test]
fn an_early_bus_waits_for_its_time() {
    // five minutes early: until 20 s before its departure (a train: 2 min)
    assert_eq!(early_wait(400.0, 100.0, false, false), 280.0);
    assert_eq!(early_wait(400.0, 100.0, false, true), 180.0);
    // late, or nearly on time: off at once
    assert_eq!(early_wait(400.0, 390.0, false, false), 0.0);
    assert_eq!(early_wait(400.0, 500.0, false, false), 0.0);
    // a layover: to the departure itself
    assert_eq!(early_wait(400.0, 100.0, true, false), 300.0);
}

#[test]
fn standing_time() {
    let mut s = BusService::new(vec![]);
    assert_eq!(s.standing_for(0.0), 0.0);
    s.phase = Phase::Waiting;
    s.leave_at = 100.0;
    assert!((s.standing_for(40.0) - 62.0).abs() < 1e-3);
    s.phase = Phase::TripDone;
    assert!(s.standing_for(0.0) > 100.0);
}

#[test]
fn only_the_ends_of_the_trip_and_an_early_bus_stop_for_nobody() {
    let stop = |id: i64, depart: f64| Stop::from_tuple((0, 0.0, 0.0, depart, id, 0.0));
    let mut s = BusService::new(vec![stop(1, 100.0), stop(2, 200.0), stop(3, 300.0)]);
    s.last_stop = Some(3);
    // on time at a stop in the middle: only if somebody wants it
    assert!(!s.must_serve(&stop(2, 200.0), 150.0));
    // over two minutes early: it stops and waits
    assert!(s.must_serve(&stop(2, 200.0), 70.0));
    // the trip's terminus, and the last stop it knows of with the route complete
    assert!(s.must_serve(&stop(3, 300.0), 300.0));
    s.stops = vec![stop(2, 200.0)].into();
    assert!(s.must_serve(&stop(2, 200.0), 200.0));
    s.route_open = true;
    assert!(!s.must_serve(&stop(2, 200.0), 200.0));
    // its first stop, where it stands out its layover
    s.layover = true;
    assert!(s.must_serve(&stop(2, 200.0), 200.0));
    s.layover = false;
    // `[profile_otherstopping]` 1/4 (always) and 3 (when early)
    s.always = vec![2];
    assert!(s.must_serve(&stop(2, 200.0), 200.0));
    s.always.clear();
    s.serve_early = vec![2];
    assert!(!s.must_serve(&stop(2, 200.0), 190.0));
    assert!(s.must_serve(&stop(2, 200.0), 170.0));
}

/// One timetable bus on a straight street, standing at its first stop (the stop point is
/// where the bus rests), with a second stop ahead so that the service does not end the
/// trip when it pulls out. Nothing is simulated but the service itself: no physics, no
/// passengers, a script that answers nothing (so the doors count as shut at once).
struct Rig {
    net: Network,
    st: AiState,
    vehicle: VehicleInstance,
    svc: BusService,
    now: f64,
    id: u64,
}

impl Rig {
    fn at_first_stop(depart: f64, now: f64, id: u64) -> Rig {
        let lane = omsi_sim::traffic::LaneBuilder::arc(
            glam::DVec3::ZERO,
            0.0,
            300.0,
            0.0,
            0.0,
            LaneKind::Street,
            2.5,
        );
        let net = Network {
            lanes: vec![lane],
            crossings: vec![Vec::new()],
            ..Default::default()
        };
        let mut st = AiState::new(0, 100.0, id);
        st.route = vec![0];
        let stop = |id, s, depart| Stop {
            ri: 0,
            s,
            bay: 0.0,
            depart,
            id,
            side: 0.0,
        };
        let svc = BusService::new(vec![stop(1, 100.0, depart), stop(2, 250.0, depart + 300.0)]);
        let vehicle = testing::script_test_vehicle("{frame_ai}\n{end}\n", "", "");
        Rig {
            net,
            st,
            vehicle,
            svc,
            now,
            id,
        }
    }

    /// One frame of `dt` seconds; whether `holds` keeps the doors open (somebody at them).
    fn frame(&mut self, dt: f32, hold: bool) {
        self.now += dt as f64;
        if hold {
            self.svc.hold(None, 2.5);
        }
        let way = [(0usize, 0.0f32)];
        let ctx = Ctx {
            net: &self.net,
            way: &way,
            day_time: self.now,
            dt,
            id: self.id,
            stopped: 0.0,
            passing: false,
            kerb_swerve: None,
            wanted: Some(true),
            debug: false,
        };
        self.svc.step(&mut self.st, &mut self.vehicle, &ctx);
    }

    /// Frames of 0.1 s until the bus has left its first stop; the seconds that took, or
    /// None when it was still there after `limit` seconds.
    fn seconds_to_leave(&mut self, limit: f32, hold: bool) -> Option<f32> {
        let mut t = 0.0;
        while t < limit {
            self.frame(0.1, hold);
            t += 0.1;
            if self.svc.phase == Phase::Running && self.svc.stops.len() == 1 {
                return Some(t);
            }
        }
        None
    }
}

/// #172: one stop, one bus, boarding finished, nobody at the doors and nothing ahead: the
/// bus pulls out. A late bus stands no longer than the doors' time and the closing.
#[test]
fn a_bus_leaves_its_stop_when_boarding_is_over() {
    for id in 1..=10u64 {
        // due at 08:00:00, there at 08:00:30 (late): off as soon as the doors are shut
        let mut rig = Rig::at_first_stop(28_800.0, 28_830.0, id);
        let left = rig
            .seconds_to_leave(120.0, false)
            .unwrap_or_else(|| panic!("bus {id} never left ({:?})", rig.svc.phase));
        assert!(
            left <= boarding_time(id) + CLOSE_MAX + 1.0,
            "bus {id} stood {left:.1} s"
        );
        assert!(
            left >= boarding_time(id),
            "bus {id} left with its doors open {left:.1} s"
        );
    }
}

/// #172: an early bus waits for its time and not a second longer: it leaves when its
/// departure is reached (a bus that is more than `EARLY_LEAVE` early stands until then).
#[test]
fn an_early_bus_leaves_when_its_time_comes_and_not_later() {
    let mut rig = Rig::at_first_stop(28_800.0, 28_700.0, 3);
    let left = rig.seconds_to_leave(300.0, false).expect("never left");
    let wait = early_wait(28_800.0, 28_700.0, false, false) as f32;
    assert!(
        left >= wait && left <= wait + 2.0 + CLOSE_MAX,
        "left after {left:.1} s, expected about {wait:.0} s"
    );
}

/// #172: somebody at the doors who cannot get on or off (an exit that never opens, a
/// door that stays shut) must not keep a timetable bus at its stop for good: the people
/// hold the doors every frame (`hold`), so without a limit the bus never leaves.
/// Until the cause of the reports is seen on a real map, this limit is the safety net;
/// what the original does with such a case is not known (#49).
#[test]
fn nobody_keeps_a_bus_at_its_stop_for_good() {
    let mut rig = Rig::at_first_stop(28_800.0, 28_800.0, 3);
    let left = rig.seconds_to_leave(600.0, true);
    assert!(
        left.is_some(),
        "still {:?} after ten minutes of somebody at the doors",
        rig.svc.phase
    );
}

/// The limit does not cut short a crowd: people getting on one after another keep the
/// doors open until just before `MAX_HELD` past the normal dwell, and
/// not much longer.
#[test]
fn a_queue_of_people_still_holds_the_doors() {
    let mut rig = Rig::at_first_stop(28_800.0, 28_800.0, 3);
    let base = boarding_time(3);
    let left = rig.seconds_to_leave(base + MAX_HELD - 5.0, true);
    assert!(
        left.is_none(),
        "left after {left:?} with people at the doors"
    );
    assert_eq!(rig.svc.phase, Phase::Boarding);
    let left = rig
        .seconds_to_leave(20.0 + CLOSE_MAX, true)
        .expect("never left");
    assert!(left < 20.0 + CLOSE_MAX);
}

/// At a higher time speed the day clock runs faster than the frames: the limit is in the
/// frames' seconds, so a normal dwell is never cut short by it.
#[test]
fn the_hold_limit_does_not_follow_the_day_clock() {
    let mut rig = Rig::at_first_stop(28_800.0, 28_800.0, 3);
    for _ in 0..(boarding_time(3) * 10.0) as usize - 5 {
        // (x30: a frame of 0.1 s is 3 s of the day)
        rig.now += 2.9;
        rig.frame(0.1, true);
    }
    assert!(!rig.svc.held_out);
    assert_eq!(rig.svc.phase, Phase::Boarding);
}

/// The limit is per stop: after being held out at one stop the bus serves the next
/// one normally.
#[test]
fn a_bus_held_out_of_one_stop_boards_normally_at_the_next() {
    let mut rig = Rig::at_first_stop(28_800.0, 28_800.0, 3);
    rig.seconds_to_leave(600.0, true).expect("never left");
    assert!(rig.svc.held_out);
    rig.svc.restart(
        vec![
            Stop {
                ri: 0,
                s: 100.0,
                bay: 0.0,
                depart: rig.now,
                id: 5,
                side: 0.0,
            },
            Stop {
                ri: 0,
                s: 250.0,
                bay: 0.0,
                depart: rig.now + 300.0,
                id: 6,
                side: 0.0,
            },
        ],
        false,
    );
    rig.st.s = 100.0;
    rig.st.speed = 0.0;
    for _ in 0..50 {
        rig.frame(0.1, false);
    }
    assert_eq!(rig.svc.phase, Phase::Boarding);
    assert!(!rig.svc.held_out);
    rig.svc.hold(None, 2.5);
    assert!(rig.svc.boarding >= 2.4, "a hold at the new stop is ignored");
}

/// Steps `s` once every half second from `from` up to `to`, as the traffic does, and
/// returns the first time `stop_when` holds.
fn run_until(
    s: &mut BusService,
    st: &mut AiState,
    v: &mut VehicleInstance,
    net: &Network,
    from: f64,
    to: f64,
    stop_when: impl Fn(&BusService) -> bool,
) -> Option<f64> {
    let mut t = from;
    while t < to {
        let ctx = Ctx {
            net,
            way: &[],
            day_time: t,
            dt: 0.5,
            id: 1,
            stopped: 0.0,
            passing: false,
            kerb_swerve: None,
            wanted: None,
            debug: false,
        };
        s.step(st, v, &ctx);
        if stop_when(s) {
            return Some(t);
        }
        t += 0.5;
    }
    None
}

/// A timetable bus that reaches the last stop of its trip early stands there until
/// the next trip of its tour is due, and leaves on it no sooner than its departure.
#[test]
fn a_bus_early_at_the_end_of_a_trip_waits_for_the_next_trips_departure() {
    use omsi_sim::traffic::LaneBuilder;
    let net = Network {
        lanes: vec![LaneBuilder::arc(
            glam::DVec3::ZERO,
            0.0,
            200.0,
            0.0,
            0.0,
            LaneKind::Street,
            3.0,
        )],
        ..Default::default()
    };
    let mut st = AiState::new(0, 10.0, 1);
    st.route = vec![0];
    let mut v = testing::script_test_vehicle("{frame}\n{end}\n", "", "");
    // the last stop of the first trip, due at 1000 s; the bus is there at 500
    let stop = |id: i64, s: f32, depart: f64| Stop::from_tuple((0, s, 0.0, depart, id, 0.0));
    let mut b = BusService::new(vec![stop(1, 10.3, 1000.0)]);
    b.last_stop = Some(1);
    let done = run_until(&mut b, &mut st, &mut v, &net, 500.0, 2000.0, |b| {
        b.trip_done()
    })
    .expect("the trip ends");
    assert!(
        done >= 1000.0 - EARLY_LEAVE,
        "the first trip is over at {done}"
    );
    // the tour's next trip leaves the same place at 1300 s: the bus takes it on in place
    b.restart(vec![stop(2, 10.3, 1300.0), stop(3, 150.0, 1500.0)], true);
    let left = run_until(&mut b, &mut st, &mut v, &net, done, 3000.0, |b| {
        b.stops.len() == 1
    })
    .expect("it sets off");
    assert!(
        left >= 1300.0,
        "left at {left} for a trip that leaves at 1300"
    );
    assert!(
        left < 1310.0,
        "left at {left}: it waits for no more than the doors take to close"
    );
}

#[test]
fn station_side_comes_from_the_stop_it_boards_at() {
    let stop = |side: f32| Stop::from_tuple((0, 0.0, 0.0, 0.0, 1, side));
    let mut s = BusService::new(vec![stop(1.0)]);
    // off a stop: nothing to open
    s.phase = Phase::Running;
    assert_eq!(s.at_station_side(), 0.0);
    // boarding: the front stop's side
    s.phase = Phase::Boarding;
    assert_eq!(s.at_station_side(), 1.0);
    // waiting to pull out (doors shut): the side is not asked for any more
    s.phase = Phase::Waiting;
    assert_eq!(s.at_station_side(), 0.0);
    // an empty queue answers 0, not a panic
    s.phase = Phase::Boarding;
    s.stops.clear();
    assert_eq!(s.at_station_side(), 0.0);
}
