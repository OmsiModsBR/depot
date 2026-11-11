//! The OMSI Classic AI in traffic (`KA-2`): a car moves by its own scripts' drive torque and
//! brakes, the AI driver at its pedals, over the synthetic crossroads of `junction_scenarios`.

use super::junction_scenarios::*;

/// A Classic car whose script turns the throttle into drive torque pulls away and drives on;
/// one whose script hands no drive torque stays where it stands.
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

/// `KA-11`: a loose car beyond the detail radius about the camera goes back to its route.
#[test]
fn a_loose_car_far_from_the_camera_is_cleared() {
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
    w.tick();
    assert!(!w.of(id).is_loose());
}

/// In Project Depot Physics nothing leaves its route.
#[test]
fn a_touched_depot_car_stays_on_its_route() {
    let mut w = World::new(Layout::default(), Vec::new());
    let id = w.car(w.x.approach[NORTH], 0.0, 5.0, 3, false);
    w.t.touched_by_player(vec![id], None);
    w.tick();
    assert!(!w.of(id).is_loose());
    assert_eq!(w.t.loose_count(), 0);
}
