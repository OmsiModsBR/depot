use glam::DVec3;
use glam::Mat4;
use glam::Vec3;
use openomsi_game::testing::puddles::*;

fn puddle_coverage(x: f64, y: f64, wet_road: f32) -> f32 {
    water_at(x, y, wet_road).puddle
}

/// A car tyre at the origin going north (+y) at `speed`, its outside to the east.
fn tyre(speed: f32) -> Tyre {
    Tyre {
        key: 1,
        contact: DVec3::new(0.0, 0.0, 33.0),
        travel: Vec3::Y,
        out: Vec3::X,
        speed,
        heavy: 1.0,
        own_spray: false,
    }
}

fn puddle(_: f64, _: f64) -> Water {
    Water {
        puddle: 1.0,
        depth: 1.0,
        wet: 1.0,
    }
}

fn wet_road(_: f64, _: f64) -> Water {
    Water {
        puddle: 0.0,
        depth: 0.0,
        wet: 1.0,
    }
}

/// Puffs thrown by `tyres` over `secs` (the tyre standing where it is, the camera beside it).
fn run(tyres: &[Tyre], secs: f32, water: &dyn Fn(f64, f64) -> Water) -> Spray {
    let mut s = Spray::new();
    for _ in 0..(secs * 30.0) as usize {
        s.update(
            1.0 / 30.0,
            tyres,
            DVec3::new(5.0, 0.0, 34.0),
            Vec3::ZERO,
            water,
        );
    }
    s
}

#[test]
fn coverage_is_zero_off_the_moisture_mask_and_grows_with_wetness() {
    assert_eq!(
        puddle_coverage(1000.0, 2000.0, 0.0),
        0.0,
        "no [moisture] under the wheel: never a puddle"
    );
    // scan a stretch of road for a spot the mask calls a puddle at full wetness, and
    // check it grows monotonically as the road dries back out from there
    let mut best = None;
    for i in 0..400 {
        let x = i as f64 * 1.3;
        let c = puddle_coverage(x, 500.0, 1.0);
        if c > 0.4 {
            best = Some((x, c));
            break;
        }
    }
    let (x, full) =
        best.expect("some spot along 520 m of road should read as a puddle at full wetness");
    let half = puddle_coverage(x, 500.0, 0.5);
    let dry = puddle_coverage(x, 500.0, 0.05);
    assert!(
        full >= half && half >= dry && full > dry,
        "a puddle should shrink as the road dries: {full} (wet) vs {half} (half) vs {dry} (drier)"
    );
}

#[test]
fn a_road_wet_through_has_puddles_in_patches() {
    // water stands in the low spots; the road between them is wet asphalt, not a mirror
    let covered = |wet: f32| {
        (0..4000)
            .map(|i| puddle_coverage(i as f64 * 0.77, i as f64 * 1.9, wet))
            .sum::<f32>()
            / 4000.0
    };
    let (soaked, half) = (covered(1.0), covered(0.5));
    assert!(
        (0.15..0.5).contains(&soaked),
        "puddles cover {soaked} of a soaked road"
    );
    assert!(
        half < soaked * 0.5,
        "puddles cover {half} of a half wet road, {soaked} of a soaked one"
    );
}

#[test]
fn the_mask_repeats_with_the_shaders_pattern_period() {
    // the shader sees the map coordinate modulo 1000 m: Spandau's 892 km and the same
    // spot a period on must read the same water
    for (x, y) in [(891_700.3, 4_193_450.8), (12.5, 640.0), (-37.0, 999.9)] {
        let a = water_at(x, y, 1.0);
        let b = water_at(x + 1000.0, y - 3000.0, 1.0);
        assert!(
            (a.puddle - b.puddle).abs() < 1e-3 && (a.depth - b.depth).abs() < 1e-3,
            "{x}, {y}: {a:?} vs {b:?}"
        );
    }
    // and the hash is the shader's integer one: cells a period apart are the same cell
    assert_eq!(hash_cell(3.0, 5.0, 220.0), hash_cell(223.0, -215.0, 220.0));
    assert!((0.0..=1.0).contains(&hash_cell(17.0, 4.0, 900.0)));
}

#[test]
fn spray_grows_with_the_square_of_the_speed() {
    let walk = run(&[tyre(1.4)], 1.0, &puddle).len();
    let slow = run(&[tyre(7.0)], 1.0, &puddle).len();
    let city = run(&[tyre(13.9)], 1.0, &puddle).len();
    assert!(walk == 0, "a tyre at walking pace throws nothing: {walk}");
    assert!(
        city > 40,
        "a tyre at 50 km/h in a puddle throws a cloud: {city} puffs"
    );
    // twice the speed, about four times the spray (the fan's drops land meanwhile)
    assert!(
        city as f32 > 2.8 * slow as f32,
        "50 km/h: {city} puffs, 25 km/h: {slow}"
    );
}

/// A tyre rolling on along its `travel` for `secs`, the camera beside it.
fn run_moving(mut t: Tyre, secs: f32) -> (Spray, Tyre) {
    let mut s = Spray::new();
    let dt = 1.0 / 30.0;
    for _ in 0..(secs * 30.0) as usize {
        t.contact += (t.travel * t.speed * dt).as_dvec3();
        s.update(
            dt,
            &[t],
            t.contact + DVec3::new(5.0, 0.0, 1.0),
            Vec3::ZERO,
            &puddle,
        );
    }
    (s, t)
}

#[test]
fn the_spray_goes_backwards_from_the_tyre() {
    for travel in [Vec3::Y, -Vec3::Y, Vec3::new(0.6, 0.8, 0.0)] {
        let mut t = tyre(13.9);
        t.travel = travel;
        t.out = Vec3::new(travel.y, -travel.x, 0.0);
        let (s, t) = run_moving(t, 0.6);
        assert!(s.len() > 30);
        let n = s.len() as f32;
        // against the vehicle, which goes on at 13.9 m/s
        let rel = s.puffs.iter().map(|p| p.vel - travel * 13.9).sum::<Vec3>() / n;
        let (along, side) = (rel.dot(travel), rel.dot(t.out));
        assert!(
            along < -8.0,
            "going {travel}, the spray falls back from the vehicle: mean relative velocity {rel:?}"
        );
        assert!(
            along.abs() > 4.0 * side.abs(),
            "back, not out to the side: {along} along, {side} sideways"
        );
        // and all of it is behind the tyre that threw it, a fan just outside its track
        for p in &s.puffs {
            let d = (p.pos - t.contact).as_vec3();
            assert!(
                d.dot(travel) < 0.0,
                "a puff {:.2} m ahead of the tyre",
                d.dot(travel)
            );
            assert!(
                d.dot(t.out) > -1.0,
                "a puff {:.2} m inside the track",
                -d.dot(t.out)
            );
        }
        let mean_out = s
            .puffs
            .iter()
            .map(|p| (p.pos - t.contact).as_vec3().dot(t.out))
            .sum::<f32>()
            / n;
        assert!(
            mean_out > 0.0,
            "it spreads outwards rather than in: {mean_out}"
        );
    }
}

#[test]
fn a_wet_road_gives_a_thin_haze_and_a_puddle_a_cloud() {
    let haze = run(&[tyre(16.0)], 2.0, &wet_road).len();
    let cloud = run(&[tyre(16.0)], 2.0, &puddle).len();
    assert!(haze > 0, "a road wet through hazes at 58 km/h");
    assert!(
        cloud > 5 * haze,
        "a puddle throws far more: {cloud} puffs vs {haze}"
    );
    // only at speed on a mere film, and not at all on a damp road
    assert_eq!(
        run(&[tyre(5.0)], 2.0, &wet_road).len(),
        0,
        "a film at 18 km/h throws nothing"
    );
    let damp = |_: f64, _: f64| Water {
        puddle: 0.0,
        depth: 0.0,
        wet: 0.4,
    };
    assert_eq!(
        run(&[tyre(16.0)], 2.0, &damp).len(),
        0,
        "a damp road throws nothing"
    );
    // a vehicle spraying through its own [smoke] keeps only what the puddles add
    let mut own = tyre(16.0);
    own.own_spray = true;
    assert_eq!(
        run(&[own], 2.0, &wet_road).len(),
        0,
        "no second haze over its own [smoke] spray"
    );
    assert!(
        run(&[own], 2.0, &puddle).len() as f32 > 0.8 * cloud as f32,
        "but the puddles' spray all the same"
    );
}

#[test]
fn a_vehicle_facing_east_throws_to_the_west() {
    // a body turned as `VehicleInstance::body_rotation` turns one with heading 90°
    let (fwd, right) = level_axes(Mat4::from_rotation_z((-90f32).to_radians()));
    assert!(
        (fwd - Vec3::X).length() < 1e-5 && (right + Vec3::Y).length() < 1e-5,
        "{fwd} {right}"
    );
    // nose up on a hill, the travel stays level
    let (fwd, _) = level_axes(Mat4::from_rotation_x(0.2));
    assert!((fwd - Vec3::Y).length() < 1e-5, "{fwd}");
}

#[test]
fn a_bus_throws_more_than_a_car() {
    let mut bus = tyre(13.9);
    bus.heavy = heaviness(11_000.0);
    let car = run(&[tyre(13.9)], 1.0, &puddle).len();
    let bus = run(&[bus], 1.0, &puddle).len();
    assert!(
        bus as f32 > 1.6 * car as f32,
        "bus tyre {bus} puffs, car tyre {car}"
    );
    assert_eq!(heaviness(1100.0), 1.0);
}

#[test]
fn far_tyres_throw_less_and_the_farthest_none() {
    let at = |d: f64| {
        let mut t = tyre(13.9);
        t.contact.x = d;
        let mut s = Spray::new();
        for _ in 0..30 {
            s.update(
                1.0 / 30.0,
                &[t],
                DVec3::new(0.0, 0.0, 34.0),
                Vec3::ZERO,
                &puddle,
            );
        }
        s.len()
    };
    let (near, mid, far) = (at(5.0), at(80.0), at(130.0));
    assert_eq!(far, 0, "nothing beyond {SPAWN_RANGE} m");
    assert!(
        mid > 0 && (mid as f32) < 0.6 * near as f32,
        "at 80 m {mid} puffs, at 5 m {near}"
    );
}

#[test]
fn the_puffs_stay_under_their_cap() {
    // a whole jam of lorries at motorway speed through deep water
    let tyres: Vec<Tyre> = (0..120)
        .map(|i| {
            let mut t = tyre(25.0);
            t.key = i;
            t.heavy = 2.0;
            t.contact.x = (i % 12) as f64 * 3.0;
            t.contact.y = (i / 12) as f64 * 6.0;
            t
        })
        .collect();
    let s = run(&tyres, 4.0, &puddle);
    assert!(s.len() <= MAX_PUFFS, "{} puffs", s.len());
    assert!(
        s.len() > MAX_PUFFS / 2,
        "the cap is used: {} puffs",
        s.len()
    );
}

#[test]
fn the_mist_lies_on_the_road_not_in_it() {
    let s = run(&[tyre(20.0)], 3.0, &puddle);
    let mut sprites = Vec::new();
    s.sprites(DVec3::new(30.0, 0.0, 35.0), &mut sprites);
    assert!(!sprites.is_empty());
    for p in &s.puffs {
        assert!(
            p.pos.z >= 33.0 - 1e-6,
            "a puff under the road at {:?}",
            p.pos
        );
        if p.kind == Kind::Mist {
            assert!(
                p.pos.z - 33.0 >= (GROUND_HOLD * p.size()) as f64 - 1e-4,
                "a mist puff sunk into the road: {:?} size {}",
                p.pos,
                p.size()
            );
            assert!(
                p.pos.z - 33.0 < 3.5,
                "a mist puff floating away at {:?}",
                p.pos
            );
        }
    }
    for sp in &sprites {
        assert!(sp.alpha > 0.0 && sp.alpha <= 1.0 && sp.size < 1.6, "{sp:?}");
    }
}

#[test]
fn no_spray_in_the_cab_the_camera_sits_in() {
    let mut s = Spray::new();
    // a bus 12 m long facing north round the origin, its rear wheels 3 m from its tail
    s.bodies = vec![Body {
        origin: DVec3::new(0.0, 0.0, 33.0),
        heading: 0.0,
        bb: [2.5, 12.0, 3.2, 0.0, 0.0, 1.6],
    }];
    let mut t = tyre(13.9);
    t.contact = DVec3::new(1.1, -3.0, 33.0);
    for _ in 0..45 {
        s.update(
            1.0 / 30.0,
            &[t],
            DVec3::new(0.0, 4.0, 35.0),
            Vec3::ZERO,
            &puddle,
        );
    }
    let mut cab = Vec::new();
    s.sprites(DVec3::new(-0.5, 4.5, 35.0), &mut cab);
    for p in &cab {
        let outside = s.bodies[0].outside(p.position);
        assert!(
            outside > 0.6 * p.size,
            "a puff {outside:.2} m outside the cab, {:.2} m wide, seen from the driver's seat",
            p.size * 2.0
        );
    }
    // the same spray from outside: what trails behind the bus is seen
    let mut street = Vec::new();
    s.sprites(DVec3::new(6.0, -20.0, 34.5), &mut street);
    assert!(
        street.len() > cab.len() && !cab.is_empty(),
        "{} puffs from the street, {} from the cab",
        street.len(),
        cab.len()
    );
}

#[test]
fn a_tyre_that_leaves_the_puddle_stops_throwing() {
    let mut s = Spray::new();
    let t = [tyre(13.9)];
    for _ in 0..10 {
        s.update(
            1.0 / 30.0,
            &t,
            DVec3::new(5.0, 0.0, 34.0),
            Vec3::ZERO,
            &puddle,
        );
    }
    assert_eq!(s.tyres_in_puddle, 1);
    let n = s.len();
    s.update(
        1.0 / 30.0,
        &t,
        DVec3::new(5.0, 0.0, 34.0),
        Vec3::ZERO,
        &|_, _| Water::default(),
    );
    assert_eq!(s.tyres_wet, 0);
    assert!(s.len() <= n, "nothing new off the water");
    // and a vehicle gone from the list leaves no debt behind
    s.update(
        1.0 / 30.0,
        &[],
        DVec3::new(5.0, 0.0, 34.0),
        Vec3::ZERO,
        &puddle,
    );
    assert!(s.tyres.is_empty());
}
