//! The physics of a bus in the log (`OMSI_DEBUG_PHYSICS`): one line with the player's pose,
//! speed, wheel contacts and crashes, and each coupled part against the one in front.

/// `OMSI_DEBUG_PHYSICS`: one line with the player's pose, speed, wheel contacts and crashes.
pub(crate) fn log_physics(v: &omsi_sim::VehicleInstance, t: f32) {
    let wheels = v
        .rigid()
        .map(|rb| {
            rb.wheels()
                .iter()
                .map(|w| {
                    format!(
                        "{:+.3}@{:+.3}{}{}{}",
                        w.compression(),
                        w.ground_z() - v.position.z,
                        if w.on_ground() { "" } else { "!" },
                        if w.step_force().abs() > 500.0 {
                            format!(" step {:+.1} kN", w.step_force() / 1000.0)
                        } else {
                            String::new()
                        },
                        if w.walls().is_empty() {
                            String::new()
                        } else {
                            format!(" against {} face(s)", w.walls().len())
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    // the drift of the body: how far its travel departs from where it points (deg), and the
    // coupled parts' angles to it
    let slip = v
        .rigid()
        .filter(|rb| rb.velocity().truncate().length() > 1.0)
        .map(|rb| {
            let d = (rb.velocity().x as f64)
                .atan2(rb.velocity().y as f64)
                .to_degrees()
                - v.heading;
            let d = omsi_geometry::util::wrap_deg(d);
            let d = if d.abs() > 90.0 {
                (d + 360.0).rem_euclid(360.0) - 180.0
            } else {
                d
            };
            format!(" slip {d:+.2}")
        })
        .unwrap_or_default();
    let joints = (0..2)
        .filter_map(|i| {
            v.var(&format!("articulation_{i}_alpha"))
                .map(|a| format!(" joint{i} {:+.2}", a))
        })
        .collect::<String>();
    let parts = coupled_parts(v);
    log::info!(
        "physics t={t:5.2} pos ({:.2}, {:.2}, {:.3}) hdg {:.1}{slip}{joints}{parts} pitch {:+.2} bank {:+.2} v {:+.2} km/h wheels [{wheels}] crashes {} last {:.1} kJ gear {:?} n {:?} M_Wheel {:?} brake {:?} pedal {:?} abs {:?} rpm {:?}",
        v.position.x,
        v.position.y,
        v.position.z,
        v.heading,
        v.pitch,
        v.bank,
        v.motion().velocity_kmh(),
        v.crashes,
        v.last_impact / 1000.0,
        v.var("antrieb_getr_aktugang").or(v.var("gear")),
        v.var("engine_n"),
        v.var("M_Wheel"),
        v.var("Axle_Brakeforce_1_L"),
        v.var("Brake"),
        v.var("bremse_ABS_eingriff"),
        v.var("Wheel_RotationSpeed_1_L"),
    );
}

/// Each coupled part against the one in front of it, for `log_physics`: the joint's alpha and
/// beta as the animations read them, the part's roll against the one in front (the joint lets
/// none), how far apart the two hold the coupling (m), and the part's own pitch and bank.
fn coupled_parts(v: &omsi_sim::VehicleInstance) -> String {
    let mut lead = (v.body_rotation(), v.heading, v.position);
    let mut out = String::new();
    for (i, t) in v.trailers.iter().enumerate() {
        let rot = t.body_rotation();
        let (alpha, beta) = omsi_sim::vehicle::joint_angles(
            lead.0,
            lead.1,
            rot,
            t.heading,
            t.ty.def.pitch_offset() > 0.0,
        );
        let (fwd, up) = (
            rot.transform_vector3(glam::Vec3::Y),
            rot.transform_vector3(glam::Vec3::Z),
        );
        let roll = up
            .cross(lead.0.transform_vector3(glam::Vec3::Z))
            .dot(fwd)
            .clamp(-1.0, 1.0)
            .asin()
            .to_degrees();
        let (_, front) = t.couplings();
        let gap = (t.position + rot.transform_point3(front).as_dvec3()
            - t.coupling_point(lead.2, lead.0))
        .length();
        let (pitch, bank) = t
            .rigid()
            .map(|rb| {
                let (_, p, b) = rb.heading_pitch_bank();
                (p, b)
            })
            .unwrap_or((f32::NAN, f32::NAN));
        out += &format!(
            " part{i} (alpha {alpha:+.2} beta {beta:+.2} roll {roll:+.2} gap {gap:.3} pitch {pitch:+.2} bank {bank:+.2})"
        );
        lead = (rot, t.heading, t.position);
    }
    out
}
