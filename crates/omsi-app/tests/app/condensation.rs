use openomsi_game::testing::condensation::*;

fn run(i: CabinInputs, minutes: f32) -> CabinAir {
    let mut c = CabinAir::new();
    for _ in 0..(minutes * 60.0) as usize {
        c.step(1.0, &i);
    }
    c
}

fn inputs() -> CabinInputs {
    CabinInputs {
        outside: 7.0,
        dew_point: 6.0,
        speed: 10.0,
        engine: true,
        people: 31,
        doors_open: 0,
        precip: true,
        volume: 70.0,
        skin: 150.0,
        vehicle: 1,
        position: glam::DVec3::ZERO,
    }
}

fn milky(c: &CabinAir, k: usize) -> f32 {
    1.0 - (-c.appearance()[k]).exp()
}

#[test]
fn the_dew_point_comes_from_the_weathers_humidity() {
    for t in [-10.0, 0.0, 7.0, 25.0] {
        assert!((dew_point(t, saturation(t)) - t).abs() < 0.05, "{t}");
    }
    // OMSI's own "Starker Schneefall": -6 °C, dew point -7.96 - as the loaded weather
    // has it, about 3 g/m³, which is not a dew point of 3 °C (nor saturated air)
    let abs = omsi_content::weather::absolute_humidity(-6.0, -7.96);
    assert!(
        (dew_point(-6.0, abs) + 7.96).abs() < 0.5,
        "{}",
        dew_point(-6.0, abs)
    );
    let abs = omsi_content::weather::absolute_humidity(7.0, 6.0);
    assert!(
        (dew_point(7.0, abs) - 6.0).abs() < 0.3,
        "{}",
        dew_point(7.0, abs)
    );
}

#[test]
fn a_full_bus_on_a_wet_autumn_evening_fogs_up() {
    let c = run(inputs(), 15.0);
    assert!(milky(&c, 1) > 0.5, "{c:?}");
}

#[test]
fn an_empty_bus_stays_clear() {
    let c = run(
        CabinInputs {
            people: 1,
            precip: false,
            dew_point: -2.0,
            ..inputs()
        },
        15.0,
    );
    assert!(milky(&c, 1) < 0.2, "{c:?}");
}

#[test]
fn the_heating_warms_the_cabin_in_some_minutes() {
    let c = run(
        CabinInputs {
            people: 1,
            ..inputs()
        },
        5.0,
    );
    assert!(c.temp < 17.0, "{c:?}");
    let c = run(
        CabinInputs {
            people: 1,
            ..inputs()
        },
        25.0,
    );
    assert!((c.temp - HEATED).abs() < 2.0, "{c:?}");
}

#[test]
fn open_doors_and_dry_air_clear_the_glass() {
    let mut c = run(inputs(), 15.0);
    let fogged = c.film[1];
    let dry = CabinInputs {
        people: 1,
        precip: false,
        dew_point: -10.0,
        doors_open: 2,
        ..inputs()
    };
    for _ in 0..600 {
        c.step(1.0, &dry);
    }
    assert!(c.film[1] < fogged * 0.5, "{fogged} -> {}", c.film[1]);
}

#[test]
fn the_defroster_clears_the_windscreen() {
    let c = run(inputs(), 15.0);
    assert!(
        c.defrost > 0.5 && c.defrosted_glass_clear(&inputs()),
        "{c:?}"
    );
}

#[test]
fn another_bus_starts_afresh() {
    let mut c = run(inputs(), 15.0);
    assert!(c.film[1] > 1.0);
    c.step(
        1.0,
        &CabinInputs {
            vehicle: 2,
            ..inputs()
        },
    );
    assert!(c.film[1] < 0.1 && c.temp < 8.0, "{c:?}");
    let mut c = run(inputs(), 15.0);
    c.step(
        1.0,
        &CabinInputs {
            position: glam::DVec3::new(500.0, 0.0, 0.0),
            ..inputs()
        },
    );
    assert!(c.film[1] < 0.1, "{c:?}");
}

#[test]
fn a_door_that_is_entry_and_exit_counts_once() {
    let d = |x: f32, open| (glam::Vec3::new(x, 5.0, 0.0), glam::DVec3::ZERO, 1.0, open);
    assert_eq!(
        open_doors(&[d(1.0, true), d(1.3, true), d(1.0, false), d(1.2, true)]),
        1
    );
    assert_eq!(open_doors(&[d(1.0, true), d(-1.0, false)]), 1);
    let far = (
        glam::Vec3::new(1.0, -4.0, 0.0),
        glam::DVec3::ZERO,
        1.0,
        true,
    );
    assert_eq!(open_doors(&[d(1.0, true), far]), 2);
}
