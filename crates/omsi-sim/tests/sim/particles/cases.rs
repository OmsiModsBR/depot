use glam::{DVec3, Mat4};
use omsi_model::{ParticleSystemDef, PsValue};
use omsi_sim::testing::particles::*;

fn smoke(freq: f32) -> ParticleSystemDef {
    let p: Vec<String> = [
        "0",
        "-5",
        "0.4",
        "0",
        "-1",
        "0",
        "2",
        "0.2",
        &freq.to_string(),
        "2",
        "0.95",
        "-0.2",
        "0.5",
        "3",
        "0.8",
        "0",
        "0.6",
        "0.6",
        "0.6",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    ParticleSystemDef::from_smoke(&p)
}

#[test]
fn exhaust_puffs_rise_grow_and_fade() {
    let mut s = ParticleSet::new(vec![smoke(20.0)], 7);
    for _ in 0..30 {
        s.update(
            1.0 / 30.0,
            DVec3::new(100.0, 200.0, 10.0),
            Mat4::IDENTITY,
            &|_| 0.0,
        );
    }
    let ps: Vec<&Particle> = s.particles().map(|(p, _)| p).collect();
    assert!(
        ps.len() >= 18 && ps.len() <= 21,
        "{} particles after a second at 20/s",
        ps.len()
    );
    let oldest = ps.iter().max_by(|a, b| a.age.total_cmp(&b.age)).unwrap();
    assert!(
        oldest.pos.y < 195.0,
        "blown out backwards: {:?}",
        oldest.pos
    );
    assert!(oldest.size() > 2.5, "grown to {}", oldest.size());
    assert!(oldest.alpha() < 0.5, "faded to {}", oldest.alpha());
    assert!(
        oldest.vel.length() < 2.0,
        "slowed to {}",
        oldest.vel.length()
    );
}

/// One puff of `def` set off on the first frame (its frequency read from `f`), then
/// `frames` more of `dt`.
fn one_puff(mut def: ParticleSystemDef, dt: f32, frames: usize) -> Particle {
    def.freq.0 = PsValue::Var("f".into());
    let mut s = ParticleSet::new(vec![def], 11);
    s.update(dt, DVec3::ZERO, Mat4::IDENTITY, &|_| 1.01 / dt);
    for _ in 0..frames {
        s.update(dt, DVec3::ZERO, Mat4::IDENTITY, &|_| 0.0);
    }
    let ps: Vec<&Particle> = s.particles().map(|(p, _)| p).collect();
    assert_eq!(ps.len(), 1);
    ps[0].clone()
}

/// OMSI brakes a particle by its factor every twentieth of a second (it keeps
/// 20 ln(brake) and takes e to that times dt), whatever the frame rate.
#[test]
fn a_brake_factor_is_per_twentieth_of_a_second() {
    let mut def = smoke(0.0);
    def.brake = (PsValue::Const(0.5), PsValue::Const(0.0));
    def.gravity = (PsValue::Const(0.0), PsValue::Const(0.0));
    def.velocity = (PsValue::Const(2.0), PsValue::Const(0.0));
    let at_60 = one_puff(def.clone(), 1.0 / 60.0, 30).vel.length();
    let at_30 = one_puff(def, 1.0 / 30.0, 15).vel.length();
    let want = 2.0 * 0.5f32.powi(10);
    assert!(
        (at_60 - want).abs() < want * 0.01 && (at_30 - want).abs() < want * 0.01,
        "half a second at 60 fps {at_60}, at 30 fps {at_30}, want {want}"
    );
}

/// A puff remembers the height of the ground under where it was set off: the plane its
/// owner stands on (a vehicle's tilted with the road), or the owner's own z = 0.
#[test]
fn a_puff_knows_the_ground_it_was_set_off_over() {
    let asked = std::cell::Cell::new(0);
    let plane = || {
        asked.set(asked.get() + 1);
        [-0.1, 0.0, 0.02]
    };
    let rot = Mat4::from_rotation_z(1.0);
    let mut s = ParticleSet::new(vec![smoke(31.0)], 5);
    s.update_over(
        1.0 / 30.0,
        DVec3::new(100.0, 200.0, 10.0),
        rot,
        &plane,
        &|_| 0.0,
    );
    let p = s.particles().next().unwrap().0;
    // the emitter at (0, -5, 0.4): the plane 0.2 m under the origin there
    assert!((p.ground - 9.8).abs() < 1e-4, "ground {}", p.ground);
    assert!(
        p.pos.z > p.ground,
        "set off {} over the ground {}",
        p.pos.z,
        p.ground
    );
    assert_eq!(asked.get(), 1);
    // (nothing set off: the plane is not asked for)
    let mut quiet = ParticleSet::new(vec![smoke(0.0)], 5);
    quiet.update_over(1.0 / 30.0, DVec3::ZERO, rot, &plane, &|_| 0.0);
    assert_eq!(asked.get(), 1);
    // a scenery object's: its own z = 0
    let mut o = ParticleSet::new(vec![smoke(31.0)], 5);
    o.update(
        1.0 / 30.0,
        DVec3::new(0.0, 0.0, 33.0),
        Mat4::IDENTITY,
        &|_| 0.0,
    );
    assert_eq!(o.particles().next().unwrap().0.ground, 33.0);
}

/// Every puff's picture is turned its own way (a random angle).
#[test]
fn every_puff_is_turned_its_own_way() {
    let mut s = ParticleSet::new(vec![smoke(60.0)], 9);
    for _ in 0..30 {
        s.update(1.0 / 30.0, DVec3::ZERO, Mat4::IDENTITY, &|_| 0.0);
    }
    let spins: Vec<f32> = s.particles().map(|(p, _)| p.spin).collect();
    assert!(spins.len() > 40);
    assert!(
        spins
            .iter()
            .all(|a| (0.0..std::f32::consts::TAU).contains(a)),
        "{spins:?}"
    );
    let (lo, hi) = spins
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), a| (lo.min(*a), hi.max(*a)));
    assert!(lo < 1.0 && hi > 5.0, "spins {lo}..{hi}");
}

#[test]
fn a_variable_frequency_of_zero_sends_nothing_and_the_cap_holds() {
    let mut p = smoke(0.0);
    p.freq.0 = PsValue::Var("auspuff_freq".into());
    let mut s = ParticleSet::new(vec![p], 3);
    for _ in 0..30 {
        s.update(1.0 / 30.0, DVec3::ZERO, Mat4::IDENTITY, &|n| {
            if n == "auspuff_freq" {
                0.0
            } else {
                1.0
            }
        });
    }
    assert_eq!(s.particles().count(), 0);
    for _ in 0..60 {
        s.update(1.0 / 30.0, DVec3::ZERO, Mat4::IDENTITY, &|_| 500.0);
    }
    assert!(s.particles().count() <= MAX_PER_EMITTER);
}
