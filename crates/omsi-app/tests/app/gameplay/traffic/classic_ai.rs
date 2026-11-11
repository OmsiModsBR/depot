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
