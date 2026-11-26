//! Tests moved out of `src/sky/atmosphere.rs`.

use omsi_render::testing::internals::sky::atmosphere::*;
use omsi_render::testing::internals::*;

fn lum(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

fn sun_at(deg: f32) -> Vec3 {
    let a = deg.to_radians();
    Vec3::new(0.0, -a.cos(), a.sin())
}

#[test]
fn noon_light_is_white_and_the_shade_realistic() {
    let s = SkyState::compute(&SkyInput {
        sun_dir: sun_at(60.0),
        ..Default::default()
    });
    let global = s.sun * 0.866 + s.sky_horizontal;
    // the camera is balanced for this light
    assert!(
        (global.x / global.y - 1.0).abs() < 0.02 && (global.z / global.y - 1.0).abs() < 0.02,
        "{global:?}"
    );
    // about 11 units in all; the clear sky gives a tenth to a quarter of it
    assert!((lum(global) - 11.0).abs() < 2.5, "{}", lum(global));
    let share = lum(s.sky_horizontal) / lum(global);
    assert!((0.09..0.25).contains(&share), "sky share {share}");
    // the sun is a little warmer than the sky, the sky bluer than the sun
    assert!(s.sun.x > s.sun.z && s.sky_horizontal.z > s.sky_horizontal.x);
    // SH: a surface facing up gets the sky plus a little from nowhere below
    let up = sh_irradiance(&s.sh, Vec3::Z);
    assert!(
        (lum(up) / lum(s.sky_horizontal) - 1.0).abs() < 0.1,
        "{up:?} vs {:?}",
        s.sky_horizontal
    );
    // a wall gets half the ground's bounce and some of the sky and the houses across the
    // street, the ground's underside only the bounce
    let wall = sh_irradiance(&s.sh, Vec3::X);
    let down = sh_irradiance(&s.sh, -Vec3::Z);
    assert!(
        lum(wall) > 0.6 * lum(down) && lum(down) > 0.0,
        "wall {wall:?} down {down:?}"
    );
    // the shade is bluish, but not as blue as the sky
    let shade_blue = up.z / up.x;
    assert!(
        shade_blue > 1.05 && shade_blue < s.sky_horizontal.z / s.sky_horizontal.x,
        "{up:?}"
    );
}

#[test]
fn low_sun_is_golden_and_twilight_blue() {
    let low = SkyState::compute(&SkyInput {
        sun_dir: sun_at(9.6),
        ..Default::default()
    });
    assert!(
        low.sun.x > low.sun.y * 1.15 && low.sun.y > low.sun.z * 1.2,
        "{:?}",
        low.sun
    );
    let dusk = SkyState::compute(&SkyInput {
        sun_dir: sun_at(-4.0),
        ..Default::default()
    });
    assert!(lum(dusk.sun) < 1e-3, "{:?}", dusk.sun);
    assert!(
        dusk.sky_horizontal.z > dusk.sky_horizontal.x,
        "{:?}",
        dusk.sky_horizontal
    );
    let night = SkyState::compute(&SkyInput {
        sun_dir: sun_at(-30.0),
        ..Default::default()
    });
    // a moonless city night: the glow of its own lamps, a few hundredths of a lux, warm
    assert!(
        lum(night.sky_horizontal) < 1e-4 && lum(night.sky_horizontal) > 2e-6,
        "{:?}",
        night.sky_horizontal
    );
    assert!(
        night.sky_horizontal.x > night.sky_horizontal.z,
        "{:?}",
        night.sky_horizontal
    );
    // exposure rises into the night, but not all the way
    assert!(night.exposure > low.exposure * 100.0);
    // a white wall under the street lamps comes out clearly below white, one lit by
    // the night sky alone dark but not black
    let lamp = 0.9 / std::f32::consts::PI * ARTIFICIAL * night.exposure;
    let sky = 0.3 / std::f32::consts::PI * lum(night.sky_horizontal) * night.exposure;
    assert!(
        (0.1..0.6).contains(&lamp) && (0.0004..0.03).contains(&sky),
        "lamp {lamp} sky {sky}"
    );
}

#[test]
fn the_moon_lights_the_night_and_clouds_throw_back_the_city() {
    let dark = SkyInput {
        sun_dir: sun_at(-30.0),
        ..Default::default()
    };
    let base = SkyState::compute(&dark);
    // a full moon 40 degrees up: several times the light of the glow alone, coming from
    // the moon's side, and its disc in the sky
    let moon_dir = Vec3::new(0.3, 0.7, 0.64).normalize();
    let full = SkyState::compute(&SkyInput {
        moon_dir,
        moon_illum: 1.0,
        ..dark
    });
    assert!(
        lum(full.sky_horizontal) > 3.0 * lum(base.sky_horizontal),
        "{:?} vs {:?}",
        full.sky_horizontal,
        base.sky_horizontal
    );
    assert!(lum(full.moon_disc) > 1e-5, "{:?}", full.moon_disc);
    // (its direct light, which the moon's shadow takes away, stronger than all the
    // night sky's light round it)
    assert!(
        lum(full.moon_light) > lum(base.sky_horizontal),
        "{:?}",
        full.moon_light
    );
    // a half moon gives far less than half the light
    let half = SkyState::compute(&SkyInput {
        moon_dir,
        moon_illum: 0.5,
        ..dark
    });
    assert!(lum(half.moon_disc) < 0.3 * lum(full.moon_disc));
    // an overcast city night is brighter than a clear one: the deck throws the lamps back
    let deck = SkyState::compute(&SkyInput {
        overcast: 1.0,
        sun_visibility: 0.0,
        ..dark
    });
    assert!(
        lum(deck.sky_horizontal) > 2.0 * lum(base.sky_horizontal),
        "{:?}",
        deck.sky_horizontal
    );
}

#[test]
fn clouds_stay_lit_after_the_sun_has_set_for_the_street() {
    // two degrees below the horizon: no sun in the street, but the cumulus layer and
    // above all the high layer still in a red sun
    let st = SkyState::compute(&SkyInput {
        sun_dir: sun_at(-2.0),
        ..Default::default()
    });
    assert!(lum(st.sun_disc) < 1e-4, "{:?}", st.sun_disc);
    assert!(
        lum(st.sun_at[3]) > 0.01 && lum(st.sun_at[3]) > lum(st.sun_at[0]),
        "{:?}",
        st.sun_at
    );
    assert!(st.sun_at[3].x > 2.0 * st.sun_at[3].z, "{:?}", st.sun_at[3]);
    // at noon the clouds get about what the street gets, a little more
    let noon = SkyState::compute(&SkyInput {
        sun_dir: sun_at(60.0),
        ..Default::default()
    });
    let r = lum(noon.sun_at[0]) / lum(noon.sun_disc);
    assert!((1.0..1.15).contains(&r), "{r}");
}

#[test]
fn humid_haze_whitens_the_sky_and_reddens_the_sunset() {
    let clean = SkyState::compute(&SkyInput {
        sun_dir: sun_at(45.0),
        haze: 0.6,
        angstrom: 1.5,
        ..Default::default()
    });
    let humid = SkyState::compute(&SkyInput {
        sun_dir: sun_at(45.0),
        haze: 2.5,
        angstrom: 0.6,
        ..Default::default()
    });
    let blue = |st: &SkyState| st.sky_horizontal.z / st.sky_horizontal.x;
    assert!(
        blue(&clean) > blue(&humid) * 1.15,
        "{} {}",
        blue(&clean),
        blue(&humid)
    );
    let low_clean = SkyState::compute(&SkyInput {
        sun_dir: sun_at(4.0),
        haze: 0.6,
        angstrom: 1.5,
        ..Default::default()
    });
    let low_humid = SkyState::compute(&SkyInput {
        sun_dir: sun_at(4.0),
        haze: 2.5,
        angstrom: 0.6,
        ..Default::default()
    });
    // (raw, before the camera's white balance follows it part of the way)
    let red = |st: &SkyState| st.sun_disc.x / st.sun_disc.z.max(1e-9);
    assert!(
        red(&low_humid) > red(&low_clean),
        "{} {}",
        red(&low_humid),
        red(&low_clean)
    );
}

#[test]
fn overcast_is_grey_and_darker() {
    let clear = SkyState::compute(&SkyInput {
        sun_dir: sun_at(40.0),
        ..Default::default()
    });
    let grey = SkyState::compute(&SkyInput {
        sun_dir: sun_at(40.0),
        sun_visibility: 0.0,
        overcast: 1.0,
        ..Default::default()
    });
    let g = grey.sky_horizontal;
    assert!((g.z / g.x) < 1.15, "{g:?}");
    let clear_global = lum(clear.sun * sun_at(40.0).z + clear.sky_horizontal);
    assert!(
        (0.2..0.5).contains(&(lum(g) / clear_global)),
        "{} of {}",
        lum(g),
        clear_global
    );
    assert!(grey.exposure > clear.exposure * 1.5);
}

/// `cargo test -p omsi-render sky_report -- --nocapture --ignored`: the sky as the
/// screen shows it (sRGB after the exposure, before the metering) for tuning.
#[test]
#[ignore]
fn sky_report() {
    let srgb = |c: f32| omsi_texture::srgb::from_linear(c.clamp(0.0, 1.0)) * 255.0;
    for (sun_el, oc) in [
        (60.0f32, 0.0f32),
        (22.0, 0.0),
        (8.0, 0.0),
        (1.0, 0.0),
        (-4.0, 0.0),
        (-20.0, 0.0),
        (40.0, 1.0),
    ] {
        let input = SkyInput {
            sun_dir: sun_at(sun_el),
            overcast: oc,
            sun_visibility: 1.0 - oc,
            ..Default::default()
        };
        let st = SkyState::compute(&input);
        let e = st.exposure;
        println!(
            "sun {sun_el:5.1}° overcast {oc}: sun {:?} sky_h {:?} exposure {e:.3} (log2 {:.2})",
            st.sun,
            st.sky_horizontal,
            e.log2()
        );
        let sun_az = input.sun_dir.y.atan2(input.sun_dir.x);
        for rel_az in [0.0f32, 90.0, 180.0] {
            let mut line = format!("  az {rel_az:5.0}:");
            for el in [2.0f32, 8.0, 20.0, 45.0, 89.0] {
                let a = sun_az + rel_az.to_radians();
                let d = Vec3::new(
                    el.to_radians().cos() * a.cos(),
                    el.to_radians().cos() * a.sin(),
                    el.to_radians().sin(),
                );
                // the table cell nearest to d
                let az = {
                    let (s2, d2) = (
                        glam::Vec2::new(input.sun_dir.x, input.sun_dir.y).normalize(),
                        glam::Vec2::new(d.x, d.y).normalize(),
                    );
                    s2.dot(d2).clamp(-1.0, 1.0).acos()
                };
                let col = ((az / std::f32::consts::PI) * SKY_LUT_W as f32)
                    .floor()
                    .min(SKY_LUT_W as f32 - 1.0) as usize;
                let row = (lut_row(el.to_radians()) * SKY_LUT_H as f32)
                    .floor()
                    .min(SKY_LUT_H as f32 - 1.0) as usize;
                let l = st.lut[row * SKY_LUT_W as usize + col];
                let c = Vec3::new(l[0], l[1], l[2]) * st.lut_scale * e;
                line += &format!(
                    "  {el:2.0}°({:3.0},{:3.0},{:3.0})",
                    srgb(c.x),
                    srgb(c.y),
                    srgb(c.z)
                );
            }
            println!("{line}");
        }
        let n = |v: Vec3| format!("({:3.0},{:3.0},{:3.0})", srgb(v.x), srgb(v.y), srgb(v.z));
        // a grey card (albedo 0.18) facing up, facing the sun's side, facing away
        let up = (st.sun * input.sun_dir.z.max(0.0) + sh_irradiance(&st.sh, Vec3::Z)) * 0.18
            / std::f32::consts::PI
            * e;
        let shade = sh_irradiance(&st.sh, Vec3::Z) * 0.18 / std::f32::consts::PI * e;
        let wall_away = sh_irradiance(
            &st.sh,
            -Vec3::new(input.sun_dir.x, input.sun_dir.y, 0.0).normalize(),
        ) * 0.18
            / std::f32::consts::PI
            * e;
        println!(
            "  grey card: sunlit up {}  shaded up {}  wall facing away {}",
            n(up),
            n(shade),
            n(wall_away)
        );
    }
}

#[test]
fn half_floats() {
    for v in [0.0f32, 1.0, -2.5, 0.5, 65504.0, 1e-3, 3.140625] {
        let b = f16_bits(v);
        let sign = if b & 0x8000 != 0 { -1.0 } else { 1.0 };
        let e = ((b >> 10) & 0x1f) as i32;
        let m = (b & 0x3ff) as f32;
        let back = if e == 0 {
            sign * m / 1024.0 * 2f32.powi(-14)
        } else {
            sign * (1.0 + m / 1024.0) * 2f32.powi(e - 15)
        };
        assert!((back - v).abs() <= v.abs() * 1e-3 + 1e-6, "{v} -> {back}");
    }
    assert_eq!(f16_bits(1e9), 0x7c00);
}
