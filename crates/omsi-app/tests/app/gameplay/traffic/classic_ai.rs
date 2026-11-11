//! The OMSI Classic AI in traffic (`KA-2`): a car moves by its own scripts' drive torque and
//! brakes, the AI driver at its pedals, over the synthetic crossroads of `junction_scenarios`.

use super::junction_scenarios::*;

/// A Classic car whose script turns the throttle into drive torque pulls away and drives on;
/// one whose script hands no drive torque stays where it stands.
/// Checks: KA-2
#[test]
fn a_classic_car_moves_by_its_own_drive() {
    let mut w = World::new(Layout::default(), Vec::new());
    let engine = w.car_of(
        engine_car_type(),
        PhysicsMode::Classic,
        w.x.approach[NORTH],
        0.0,
        0.0,
        3,
        false,
    );
    let plain = w.car_of(
        car_type(),
        PhysicsMode::Classic,
        w.x.approach[(NORTH + 2) % 4],
        0.0,
        0.0,
        5,
        false,
    );
    let mut throttle = 0.0f32;
    w.run(6.0, |w| {
        throttle = throttle.max(w.of(engine).vehicle.var("Throttle").unwrap_or(0.0));
    });
    assert!(throttle > 0.5, "throttle {throttle}");
    let c = w.of(engine);
    assert!(
        c.state.speed > 3.0 && c.state.s > 5.0,
        "speed {} at {}",
        c.state.speed,
        c.state.s
    );
    let p = w.of(plain);
    assert_eq!((p.state.speed, p.state.s), (0.0, 0.0));
}

/// `KA-6`: a Classic car the player's vehicle touches leaves its route: fully braked on its
/// rigid body, its driver standing where it left the way, until the accident is cleared and it
/// drives on from its route.
/// Checks: KA-6, KA-8
#[test]
fn a_touched_classic_car_is_loose_until_cleared() {
    let mut w = World::new(Layout::default(), Vec::new());
    let id = w.car_of(
        engine_car_type(),
        PhysicsMode::Classic,
        w.x.approach[NORTH],
        0.0,
        0.0,
        3,
        false,
    );
    w.run(4.0, |_| {});
    let s = w.of(id).state.s;
    assert!(w.of(id).state.speed > 1.0);
    w.t.touched_by_player(vec![id], None);
    w.tick();
    assert!(w.of(id).is_loose());
    assert_eq!(w.t.loose_count(), 1);
    w.run(4.0, |_| {});
    let c = w.of(id);
    assert_eq!(c.state.speed, 0.0);
    assert!((c.state.s - s).abs() < 1.0, "{} from {s}", c.state.s);
    assert!(
        c.vehicle.motion().speed.abs() < 0.3,
        "{}",
        c.vehicle.motion().speed
    );
    assert_eq!(c.vehicle.controls().brake, 1.0);
    assert_eq!(w.t.clear_crashes(), 1);
    assert!(!w.of(id).is_loose());
    assert_eq!(w.t.take_cleared(), vec![id]);
    w.run(4.0, |_| {});
    assert!(w.of(id).state.speed > 1.0);
}

/// `KA-6`, `KA-8`: a loose car does not touch the cars on their routes: it meets none of
/// their boxes, and a car it runs through stays on its route; only the player's vehicle
/// releases a car.
/// Checks: KA-6, KA-8
#[test]
fn a_loose_car_passes_through_the_cars_on_their_routes() {
    let mut w = World::new(Layout::default(), Vec::new());
    let classic = |w: &mut World, s: f32, seed: u64| {
        w.car_of(
            engine_car_type(),
            PhysicsMode::Classic,
            w.x.approach[NORTH],
            s,
            0.0,
            seed,
            false,
        )
    };
    let loose = classic(&mut w, 0.0, 3);
    let other = classic(&mut w, 2.0, 5);
    w.t.touched_by_player(vec![loose], None);
    w.tick();
    assert!(w.of(loose).is_loose());
    w.run(2.0, |w| {
        let own = -2 - other as i64;
        assert!(w
            .of(loose)
            .vehicle
            .dynamic_boxes
            .iter()
            .all(|b| b.id != own));
        assert!(!w.of(other).is_loose());
    });
    assert_eq!(w.t.loose_count(), 1);
}

/// `KA-11`: a loose car beyond the detail radius about the camera stays where it lies (it
/// goes only with its tile, and then the player is told they left the accident).
/// Checks: KA-11
#[test]
fn a_loose_car_far_from_the_camera_stays_loose() {
    let mut w = World::new(Layout::default(), Vec::new());
    let id = w.car_of(
        engine_car_type(),
        PhysicsMode::Classic,
        w.x.approach[NORTH],
        0.0,
        0.0,
        3,
        false,
    );
    w.t.touched_by_player(vec![id], None);
    w.tick();
    assert!(w.of(id).is_loose());
    w.t.camera = Some(glam::DVec3::new(5000.0, 5000.0, 0.0));
    w.run(2.0, |_| {});
    assert!(w.of(id).is_loose());
    assert_eq!(w.t.take_cleared(), Vec::<u64>::new());
    assert_eq!(w.t.take_fled(), 0);
}

/// In Project Depot Physics nothing leaves its route.
/// Checks: KA-6
#[test]
fn a_touched_depot_car_stays_on_its_route() {
    let mut w = World::new(Layout::default(), Vec::new());
    let id = w.car(w.x.approach[NORTH], 0.0, 5.0, 3, false);
    w.t.touched_by_player(vec![id], None);
    w.tick();
    assert!(!w.of(id).is_loose());
    assert_eq!(w.t.loose_count(), 0);
}

/// FR-016: a timetable bus put back on its route goes back empty (its riders leave it, by the
/// cleared list the session reads) and waits for the next trip of its timetable.
/// Checks: KA-6
#[test]
fn a_cleared_bus_waits_empty_for_its_next_trip() {
    use openomsi_game::testing::bus_service::{BusService, Phase};
    let mut w = World::new(Layout::default(), Vec::new());
    let id = w.car_of(
        engine_car_type(),
        PhysicsMode::Classic,
        w.x.approach[NORTH],
        0.0,
        0.0,
        3,
        false,
    );
    let car = w.t.cars.iter_mut().find(|c| c.id == id).unwrap();
    car.bus = Some(Box::new(BusService::new(Vec::new())));
    w.t.touched_by_player(vec![id], None);
    w.tick();
    assert!(w.of(id).is_loose());
    assert_eq!(w.t.clear_crashes(), 1);
    let bus = w.of(id).bus.as_ref().unwrap();
    assert_eq!(bus.phase, Phase::TripDone);
    assert!(bus.stops.is_empty());
    assert_eq!(w.t.take_cleared(), vec![id]);
}
