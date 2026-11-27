//! The world's light rules, per frame: the headlights and spots of the vehicles from their
//! script variables (`[light_enh_2]`, `[spotlight]` + `Spot_Select`, `[interiorlight]`), the
//! lighting of the daylight and the weather, the particles as the renderer draws them, and
//! the airstream a vehicle's glass meets. Reading the textures and the per-frame state is in
//! `implementation/lights.rs`.

use glam::{DVec3, Vec3};
use omsi_render::{Corona, LightMode, Lighting, PointLight};
use omsi_sim::{Daylight, VehicleInstance};

/// A headlight's strength for the enhanced renderer, its beam's at full (`headlamp` in
/// `enhanced.wgsl`): the road from the bumper out to some 12 m lit about as brightly as a
/// street under a lamp, and less beyond.
const HEADLIGHT_INTENSITY: f32 = 1800.0;

/// A `[spotlight]` declaring this range or more is a full beam (the stock buses' 500, the
/// Grand Paris-Moulon Citaro's 450), less a low beam (the stock 100, that Citaro's 200).
const FULL_BEAM_RANGE: f32 = 300.0;

/// A headlight's strength in the classic picture, as a map lamp's (`PointLight::intensity`).
/// Measured against OMSI 2 from above, the stock NL202 at night in Spandau: its pool is
/// some 18 of 255 over the unlit cobbles 1.5 - 2.5 m ahead, 12 at 4 - 5 m and nearly gone
/// at 7 m; this gives 20, 12 and 5 there. (At 1 it was 60, 40 and 15.)
const VANILLA_HEADLIGHT_INTENSITY: f32 = 0.2;

/// Lighting parameters for the renderer from the daylight model.
pub fn lighting_from(d: &Daylight, fog_range: f32) -> Lighting {
    // fog density from the weather's visibility range (an object at `range` is ~90% fogged)
    let density = (2.3 / fog_range.max(50.0)).max(0.00005);
    Lighting {
        sun_dir: d.sun_dir,
        sun_intensity: 1.0,
        sun_color: d.sun_color,
        secondary: d.secondary,
        ambient: d.ambient,
        fog_color: d.sky,
        fog_density: density,
        sky_color: d.sky,
        night: d.night,
        night_maps: Some(if d.lamps_on || d.night >= 0.5 {
            1.0
        } else {
            0.0
        }),
        sun_azimuth: d.azimuth_rad,
        sky_weights: d.sky_weights,
        envir_tint: d.envir_tint,
        moon_dir: d.moon_dir,
        moon_illum: d.moon_illum,
        day_of_year: d.day_of_year,
        latitude: d.latitude,
        day_seed: d.day_seed,
        star_frame: d.star_frame,
        star_brightness: d.star_brightness(),
        ..Default::default()
    }
}

/// What the weather does to the light: an overcast sky takes the sun away and turns
/// everything grey and flat, rain darkens it further, fog and snow take their colour
/// from the sky. `cloud_density` 0..1 (1 = closed cover), `precip` 0..1, `snow` 0..1.
pub fn apply_weather(
    l: &mut Lighting,
    cloud_density: f32,
    precip_kind: i32,
    precip: f32,
    snow: f32,
) {
    let o = cloud_density.clamp(0.0, 1.0);
    let overcast = (o - 0.45).max(0.0) / 0.55; // cumulus cover keeps the sun; a closed sky loses it
    let grey = |c: Vec3, k: f32| -> Vec3 {
        let lum = c.dot(Vec3::new(0.3, 0.59, 0.11));
        c.lerp(Vec3::splat(lum), k)
    };
    // the sun: dimmed and whitened under cloud, gone under overcast
    l.sun_intensity *= 1.0 - 0.85 * overcast;
    l.sun_color = grey(l.sun_color, 0.6 * o);
    // the sky light is what is left, greyer and a little darker
    let sky_lum = l.sky_color.dot(Vec3::new(0.3, 0.59, 0.11));
    let cloud_sky = Vec3::splat(sky_lum * 0.82)
        .lerp(Vec3::new(0.62, 0.65, 0.70) * sky_lum.max(0.25) * 1.3, 0.5);
    l.sky_color = l.sky_color.lerp(cloud_sky, overcast * 0.9);
    l.secondary = grey(l.secondary, o * 0.7) * (1.0 - 0.15 * overcast);
    l.ambient = grey(l.ambient, o * 0.7) * (1.0 + 0.25 * overcast);
    l.fog_color = l.fog_color.lerp(l.sky_color, overcast);
    // rain and snow take some light and thicken the air
    // heavy rain by day is dark: the sun is gone, the sky light drops by a third and
    // everything, the bus included, sits in the same grey. The gloom also brings the
    // night factor up a little, so interior lights and nightmaps start to show - by
    // day in a downpour you do want the saloon lights on.
    let rain = if precip_kind != 0 {
        precip.clamp(0.0, 1.0)
    } else {
        0.0
    };
    l.sun_intensity *= 1.0 - 0.75 * rain;
    l.ambient *= 1.0 - 0.28 * rain;
    l.secondary *= 1.0 - 0.32 * rain;
    l.sky_color *= 1.0 - 0.22 * rain;
    l.fog_color *= 1.0 - 0.15 * rain;
    if rain > 0.0 {
        l.fog_density = l.fog_density.max(2.3 / (2500.0 - 1800.0 * rain));
    }
    // the enhanced renderer builds its own sky from these
    l.overcast = overcast;
    l.rain = rain;
    l.snowfall = if precip_kind == 2 { rain } else { 0.0 };
    let gloom = (overcast * 0.5 + rain * 0.5).clamp(0.0, 1.0);
    l.night = l.night.max(0.45 * gloom);
    // (night maps on with the street lamps, or where the weather makes it that dark: a
    // clear dusk showed the lit windows and signs at a fraction, a rainy one at nearly
    // full, #276)
    if l.night >= 0.5 {
        l.night_maps = Some(1.0);
    }
    if snow > 0.0 {
        // snow on the ground throws light back up
        l.ambient *= 1.0 + 0.35 * snow;
        l.secondary *= 1.0 + 0.2 * snow;
        // (the snowy grey only as bright as the daylight: a fixed 0.86 made a winter night
        // in fog a pale grey dusk in the classic picture and in every mirror)
        let day = 1.0 - 0.93 * l.night.clamp(0.0, 1.0);
        l.fog_color = l
            .fog_color
            .lerp(Vec3::new(0.86, 0.88, 0.92) * day, 0.4 * snow);
    }
    l.snow = snow;
}

/// Coronas and point lights of one vehicle in its current state.
pub fn vehicle_lights(
    v: &VehicleInstance,
    coronas: &mut Vec<Corona>,
    lights: &mut Vec<PointLight>,
    night: f32,
) {
    let ty = &v.ty;
    // def index → loaded mesh index (animation transform)
    let value_of = |name: &str| -> f32 {
        let t = name.trim();
        if let Ok(x) = t.parse::<f32>() {
            return x;
        }
        v.var(t).unwrap_or(0.0)
    };
    let mesh_xf = |def_index: usize| -> glam::Mat4 {
        match ty.meshes.iter().position(|m| m.def_index == def_index) {
            Some(i) => v.mesh_local_transform(i),
            None => v.body_rotation(),
        }
    };
    coronas.extend(crate::world::scene::model_lights_faded(
        &ty.model,
        &mesh_xf,
        v.position,
        &value_of,
        &v.light_fade,
    ));
    // An articulated vehicle is one visual bus, but its rear section has its own
    // `[light_enh_2]`/corona declarations and animated meshes.  The old collector only
    // visited the leading section, which made rear lamps, destination lights and section-
    // local headlights appear dead even though the coupled part was rendered correctly.
    for t in &v.trailers {
        let part_mesh_xf = |def_index: usize| -> glam::Mat4 {
            match t.ty.meshes.iter().position(|m| m.def_index == def_index) {
                Some(i) => t.mesh_local_transform(i),
                None => t.body_rotation(),
            }
        };
        coronas.extend(crate::world::scene::model_lights_faded(
            &t.ty.model,
            &part_mesh_xf,
            t.position,
            &value_of,
            &t.light_fade,
        ));
    }
    let body = v.body_rotation();
    // headlights: the spotlight selected by Spot_Select
    // (OMSI_SPOT_SELECT=n: that spotlight on, for checking the headlights in a picture)
    let forced = omsi_cfg::flags::SPOT_SELECT
        .text()
        .and_then(|s| s.trim().parse::<f32>().ok());
    if let Some(sel) = forced.or_else(|| v.var("Spot_Select")) {
        if sel >= 0.0 {
            if let Some(sp) = ty.model.spotlights.get(sel as usize) {
                let vals = sp.values;
                let d = body
                    .transform_vector3(Vec3::new(vals[3], vals[4], vals[5]))
                    .normalize_or_zero();
                // D3D's spot is only a direction: the stock NL202 puts it 3.8 m behind its
                // nose, where the real one lit the dashboard and the windscreen from inside.
                // It shines from the vehicle's front (or rear) face at its own height. The face
                // is where the model's lamps ([light_enh], [light_enh_2]) reach furthest: the
                // AA-FR Agora's spot sits 4 m behind its headlamps, and moved at most 1.5 m it
                // still shone from inside the bus onto its own front. (Within the bounding box;
                // without lamps, the box alone, by at most 1.5 m: a box grown by mirrors, a
                // coupling or a mod's odd bounds threw the light well ahead of the bus.)
                let mut apex = Vec3::new(vals[0], vals[1], vals[2]);
                let dl = Vec3::new(vals[3], vals[4], vals[5]).normalize_or_zero();
                let lamps: Vec<[f32; 3]> = ty
                    .model
                    .meshes
                    .iter()
                    .flat_map(|m| {
                        m.light_enh
                            .iter()
                            .map(|l| l.pos)
                            .chain(m.light_enh_2.iter().map(|l| l.pos))
                    })
                    .collect();
                let nose = lamps.iter().map(|l| l[1]).reduce(f32::max);
                let tail = lamps.iter().map(|l| l[1]).reduce(f32::min);
                let bb = ty
                    .def
                    .bounding_box
                    .map(|bb| (bb[4] + bb[1] * 0.5, bb[4] - bb[1] * 0.5));
                if dl.y > 0.3 {
                    let face = match (nose.filter(|n| *n > apex.y), bb) {
                        (Some(n), Some((front, _))) => Some(n.min(front)),
                        (Some(n), None) => Some(n),
                        (None, Some((front, _))) => Some(front.min(apex.y + 1.5)),
                        (None, None) => None,
                    };
                    if let Some(face) = face.filter(|f| *f > apex.y) {
                        apex.y = face + 0.05;
                    } else if let Some(n) = nose.filter(|n| *n + 0.05 < apex.y) {
                        // (a spot ahead of the lamps, the Grand Paris-Moulon Citaro's 55 cm: back onto them)
                        apex.y = n + 0.05;
                    }
                } else if dl.y < -0.3 {
                    let face = match (tail.filter(|t| *t < apex.y), bb) {
                        (Some(t), Some((_, rear))) => Some(t.max(rear)),
                        (Some(t), None) => Some(t),
                        (None, Some((_, rear))) => Some(rear.max(apex.y - 1.5)),
                        (None, None) => None,
                    };
                    if let Some(face) = face.filter(|f| *f < apex.y) {
                        apex.y = face - 0.05;
                    } else if let Some(t) = tail.filter(|t| *t - 0.05 > apex.y) {
                        apex.y = t - 0.05;
                    }
                }
                // One spot on the vehicle's axis threw one narrow beam: it is split into one
                // per headlamp on that face (within 35 cm of it), as far apart as the mean of
                // their distances from the axis (the outermost are indicators and position
                // lamps). The Agora's headlamps are 1.8 to 2.4 m apart.
                let half_width = ty
                    .def
                    .bounding_box
                    .map_or(1.25, |bb| (bb[0] * 0.5).min(1.25));
                let on_face: Vec<f32> = lamps
                    .iter()
                    .filter(|l| (dl.y > 0.3 || dl.y < -0.3) && (l[1] - apex.y).abs() < 0.35)
                    .map(|l| (l[0] - apex.x).abs())
                    .collect();
                let spread =
                    (on_face.iter().sum::<f32>() / on_face.len().max(1) as f32).min(half_width);
                let right = body.transform_vector3(Vec3::X).normalize_or_zero();
                let apex = body.transform_point3(apex);
                let sides: &[f32] = if spread > 0.1 { &[-1.0, 1.0] } else { &[0.0] };
                for side in sides {
                    let at = v.position + (apex + right * spread * side).as_dvec3();
                    push_spot(lights, at, d, &vals, 1.0 / sides.len() as f32, night);
                }
            }
        }
    }
    // [spotlight_2] (openOMSI): every spot its variable switches on, the leading vehicle's
    // and its sections', where it is declared - a pair mirrored across the vehicle's axis
    // unless its flag keeps the one lamp - and as bright as a [spotlight] of its colour,
    // shared between the pair, times the variable (0..1)
    let parts = std::iter::once((&ty.model, v.position, body)).chain(
        v.trailers
            .iter()
            .map(|t| (&t.ty.model, t.position, t.body_rotation())),
    );
    for (model, origin, rot) in parts {
        for sp in &model.spotlights_2 {
            for (pos, dir, share) in spotlight_2_lamps(sp, value_of(sp.variable.as_str())) {
                let at = origin + rot.transform_point3(pos).as_dvec3();
                let d = rot.transform_vector3(dir).normalize_or_zero();
                push_spot(lights, at, d, &sp.values, share, night);
            }
        }
    }
    // [interiorlight]s light only the meshes listing them and the passengers (per-instance
    // term, see MeshProps::interior); they do not shine on the outside world.
    let _ = &ty.model.interior_lights;
}

/// A `[spotlight_2]`'s lamps in its vehicle's frame - position, direction and share of a
/// spot's light - with its variable at `k`: none when that is off, the lamp and its twin
/// mirrored across the axis unless the flag keeps the one.
pub fn spotlight_2_lamps(sp: &omsi_model::Spotlight2, k: f32) -> Vec<(Vec3, Vec3, f32)> {
    let k = k.clamp(0.0, 1.0);
    if k <= 0.0 {
        return Vec::new();
    }
    let v = sp.values;
    let sides: &[f32] = if sp.mirrored { &[1.0, -1.0] } else { &[1.0] };
    sides
        .iter()
        .map(|s| {
            (
                Vec3::new(v[0] * s, v[1], v[2]),
                Vec3::new(v[3] * s, v[4], v[5]),
                k / sides.len() as f32,
            )
        })
        .collect()
}

/// The lights of one headlamp at `at` shining along `d`, with a `[spotlight]`'s numbers
/// (`vals`: colour 6-8, range 9, inner and outer cone 10 and 11) and `share` of its light.
pub fn push_spot(
    lights: &mut Vec<PointLight>,
    at: DVec3,
    d: Vec3,
    vals: &[f32; 12],
    share: f32,
    night: f32,
) {
    let color = [vals[6] / 255.0, vals[7] / 255.0, vals[8] / 255.0];
    // inner and outer cone as full angles (values 10 and 11)
    let (inner, outer) = (vals[10], vals[11]);
    let half = |deg: f32| (deg.clamp(1.0, 179.0) * 0.5).to_radians().cos();
    let cone = [half(inner.min(outer)), half(outer)];
    // vanilla: the spot, lit as the classic picture lights a lamp, only inside
    // its cone (three point lights along its axis stood in for it before: they
    // shone every way, on the bus's own body and saloon)
    lights.push(PointLight {
        position: at,
        radius: spot_reach(vals[9], 45.0),
        color,
        intensity: VANILLA_HEADLIGHT_INTENSITY * share * (0.3 + 0.7 * night),
        direction: d,
        cone,
        mode: LightMode::Vanilla,
        ..Default::default()
    });
    // enhanced: falling off with the square of the distance from a one-metre core. A lamp
    // declared to reach less far than a low beam is a weaker one: the light that still
    // reaches the low beam's last lit metres at its own range is the square of the ranges
    // smaller (a work light of range 1 shone as bright as a headlamp, #1541)
    let reach = spot_reach(vals[9], 60.0);
    let weaker = (reach / 60.0).min(1.0).powi(2);
    lights.push(PointLight {
        position: at,
        radius: reach,
        color,
        intensity: HEADLIGHT_INTENSITY * share * weaker,
        direction: d,
        cone,
        core: 1.0,
        // (a lamp pointing steeply down - a `[spotlight_2]` over a door - keeps its cone: the
        // road lamp's profile is for one aimed along the road)
        beam: if d.normalize_or_zero().z.abs() >= 0.5 {
            0.0
        } else if vals[9] >= FULL_BEAM_RANGE {
            -1.0
        } else {
            1.0
        },
        housed: false,
        mode: LightMode::Enhanced,
    });
}

/// How far a `[spotlight]` reaches in the picture, from its declared range (value 9):
/// up to `low` metres as declared, as before, and a range past the stock low beam's 100 in
/// proportion to it (`low` metres for 100), at most five times `low`. The stock buses' full beam declares 500 against the low beam's 100; both
/// cut at the same 45 m (60 in Enhanced), the full beam lit no further than the low beam
/// (#941). (The light's core grows with it - a fixed share of the reach - so the full beam
/// is also brighter ahead, as one is.)
/// (A short range is taken as declared, down to half a metre: from 10 m on, a lamp of range
/// 1 lit everything round it like a headlamp, #1541.)
pub fn spot_reach(range: f32, low: f32) -> f32 {
    range
        .clamp(0.5, low)
        .max(range * low / 100.0)
        .min(low * 5.0)
}

/// How far OMSI moves every `[smoke]` and `[particle_emitter]` puff towards the eye in
/// depth (m): each particle gets 0.1, which is taken off the view depth it
/// projects the puff's depth at.
const SMOKE_Z_OFFSET: f32 = 0.1;

/// The particles of a particle set as the renderer draws them: smoke blended over the scene,
/// and the glowing ones (`--PS_emissive--`: sparks, rockets, a flame) as coronas.
pub fn particle_sprites(
    set: &omsi_sim::particles::ParticleSet,
    smoke: &mut Vec<omsi_render::SmokeParticle>,
    coronas: &mut Vec<Corona>,
) {
    for (p, def) in set.particles() {
        let alpha = p.alpha();
        if alpha <= 0.002 {
            continue;
        }
        if def.emissive {
            coronas.push(Corona {
                position: p.pos,
                size: (p.size() * 0.5).max(0.02),
                color: p.color,
                brightness: alpha,
                direction: Vec3::ZERO,
                cone_cos: -1.0,
                z_offset: 0.0,
                ..Default::default()
            });
        } else {
            smoke.push(omsi_render::SmokeParticle {
                position: p.pos,
                size: p.size() * 0.5,
                color: p.color,
                alpha,
                spin: p.spin,
                z_offset: SMOKE_Z_OFFSET,
                // (to fade into, where OMSI lets the road cut it off)
                ground: p.ground.is_finite().then_some(p.ground),
                spray: false,
            });
        }
    }
}

/// A vehicle's velocity (m/s, world) from its heading and speed: the airstream its glass
/// meets (`Lighting::glass_wind`).
pub fn vehicle_velocity(v: &omsi_sim::VehicleInstance) -> glam::Vec3 {
    let h = v.heading.to_radians();
    glam::Vec3::new(h.sin() as f32, h.cos() as f32, 0.0) * v.motion().speed
}
