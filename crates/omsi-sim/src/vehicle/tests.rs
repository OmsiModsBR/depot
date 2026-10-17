use glam::Quat;
use omsi_model::Model;
use omsi_script::Program;
use omsi_vehicle::Vehicle;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use super::physics_bridge::script_speed;
use super::props::PropsPlan;
use super::trailer::body_rotation;
use super::visuals::wheel_ground;
use super::winding::winding_pack;
use super::*;

pub(super) fn coupling_test_type(boogies: Option<f32>) -> Arc<VehicleType> {
    Arc::new(VehicleType {
        def: Vehicle {
            boogies,
            coupling_front: Some(omsi_vehicle::vehicle::Coupling {
                pos: [0.0, 5.0, 0.3],
            }),
            coupling_back: Some(omsi_vehicle::vehicle::Coupling {
                pos: [0.0, -5.0, 0.3],
            }),
            axles: [-3.0, 3.0]
                .map(|long| omsi_vehicle::Axle {
                    long,
                    ..Default::default()
                })
                .to_vec(),
            ..Default::default()
        },
        model: Model::default(),
        model_dir: PathBuf::new(),
        program: Arc::new(Program::default()),
        meshes: Vec::new(),
        paint_schemes: Vec::new(),
        texchanges: Vec::new(),
        wheel_meshes: Vec::new(),
        suspension_axles: Vec::new(),
        missing_packs: Vec::new(),
        mesh_bounds: Vec::new(),
        // Deliberately includes a protruding coupler beyond the declared joint.
        mesh_boxes: vec![(Vec3::new(-1.0, -6.2, 0.0), Vec3::new(1.0, 6.2, 3.0))],
    })
}

#[test]
fn bogie_orientation_is_independent_of_consist_flags() {
    for bogies in [None, Some(5.0), Some(-5.0), Some(0.0)] {
        let ty = coupling_test_type(bogies);
        let intrinsic = bogies.is_some_and(|b| b < 0.0);
        let mut v = VehicleInstance::new(ty.clone(), VehicleHost::new(Default::default()));
        v.heading = 37.0;
        v.pitch = 5.0;
        v.bank = 2.0;
        let expected = body_rotation(37.0, 5.0, 2.0, false);
        let forward = expected.transform_vector3(Vec3::Y);
        assert!(
            (v.body_rotation().transform_vector3(Vec3::Y).dot(forward)
                - if intrinsic { -1.0 } else { 1.0 })
            .abs()
                < 1e-5
        );
        for reversed in [false, true] {
            let mut t =
                TrailerPart::new_ex(ty.clone(), &ty, false, reversed, &ty.program, 2, 0);
            t.heading = v.heading;
            t.pitch = v.pitch;
            t.bank = v.bank;
            let sign = if intrinsic ^ reversed { -1.0 } else { 1.0 };
            assert!(
                (t.body_rotation().transform_vector3(Vec3::Y).dot(forward) - sign).abs() < 1e-5
            );
            assert!(
                (t.body_rotation().transform_vector3(Vec3::Z)
                    - expected.transform_vector3(Vec3::Z))
                .length()
                    < 1e-5
            );
            assert!(
                (t.body_heading() - (37.0 + if sign < 0.0 { 180.0 } else { 0.0 })).abs() < 1e-5
            );
        }
    }
}

#[test]
fn simple_collision_uses_the_body_frame_for_current_and_previous_pose() {
    for bogies in [None, Some(5.0), Some(-5.0)] {
        for already_inside in [false, true] {
            let mut ty = coupling_test_type(bogies);
            let def = &mut Arc::get_mut(&mut ty).unwrap().def;
            def.bounding_box = Some([2.0, 2.0, 2.0, 0.0, 3.0, 1.0]);
            def.rolling_resistance = 0.0;
            let mut v = VehicleInstance::new(ty, VehicleHost::new(Default::default()));
            v.collision = Some(Arc::new(crate::collision::CollisionWorld::default()));
            let center = if bogies == Some(-5.0) { -3.0 } else { 3.0 };
            let y = center + if already_inside { 0.0 } else { 1.15 };
            v.dynamic_boxes.push(crate::collision::Obb::point(
                DVec3::new(0.0, y, 1.0),
                0.025,
            ));
            v.set_speed(2.0);
            v.step_physics(0.1);
            if already_inside {
                assert!((v.position.y - 0.2).abs() < 1e-6);
                assert_eq!(v.motion().speed, 2.0);
            } else {
                assert_eq!(v.position, DVec3::ZERO, "{bogies:?}");
                assert_eq!(v.motion().speed, 0.0);
                let sign = if bogies == Some(-5.0) { -1.0 } else { 1.0 };
                assert!((v.host.coll_pos[1] as f64 - sign * (y - 0.2)).abs() < 1e-5);
            }
        }
    }
}

#[test]
fn non_bogie_rail_markers_keep_asymmetric_body_end_placement() {
    for marker in 0..2 {
        let mut ty = coupling_test_type(None);
        let ty_mut = Arc::get_mut(&mut ty).unwrap();
        if marker == 0 {
            ty_mut.def.rail_body_osc = Some([0.0; 7]);
        } else {
            ty_mut.def.contact_shoes.push([0.0; 6]);
        }
        ty_mut.mesh_boxes = vec![(
            Vec3::new(-1.0, -7.0, 0.0),
            Vec3::new(1.0, 5.0, 3.0),
        )];
        // The declared joint is inset from the body's rear end.
        ty_mut.def.coupling_back.as_mut().unwrap().pos[1] = -4.4;
        for lead_reversed in [false, true] {
            for reversed in [false, true] {
                let (back, front) = coupling_points(&ty, lead_reversed, &ty, reversed);
                assert_eq!(back.y, if lead_reversed { 5.0 } else { -7.0 });
                assert_eq!(front.y, if reversed { -7.0 } else { 5.0 });
                let (position, heading) = coupling_placement(
                    DVec3::ZERO,
                    0.0,
                    lead_reversed,
                    back.y,
                    reversed,
                    front.y,
                );
                let expected_distance = if lead_reversed { 5.0 } else { 7.0 }
                    + if reversed { 7.0 } else { 5.0 };
                assert!((position.y + expected_distance).abs() < 1e-6);
                assert_eq!(heading, if reversed { 180.0 } else { 0.0 });
            }
        }
    }
}

#[test]
fn consecutive_reversed_parts_keep_absolute_orientation() {
    for bogies in [None, Some(5.0), Some(-5.0)] {
        let ty = coupling_test_type(bogies);
        let mut v = VehicleInstance::new(ty.clone(), VehicleHost::new(Default::default()));
        v.heading = 23.0;
        for reversed in [true, true, false] {
            v.attach_trailer_ex(ty.clone(), reversed);
        }
        for _ in 0..30 {
            v.update_trailers(0.0);
        }
        let h = v.heading.to_radians();
        let forward = DVec3::new(h.sin(), h.cos(), 0.0);
        for (i, t) in v.trailers.iter().enumerate() {
            assert!(
                (t.position + forward * (10.0 * (i + 1) as f64)).length() < 1e-4
            );
            let reversed = (i < 2) ^ (bogies == Some(-5.0));
            assert!(
                (t.body_heading() - (v.heading + if reversed { 180.0 } else { 0.0 })).abs()
                    < 1e-4
            );
        }
    }
}

#[test]
fn bogie_couplings_agree_with_spawn_placement_and_following() {
    for lead_bogies in [None, Some(5.0), Some(-5.0)] {
        for car_bogies in [None, Some(5.0), Some(-5.0)] {
            for lead_reversed in [false, true] {
                for reversed in [false, true] {
                    let lead = coupling_test_type(lead_bogies);
                    let car = coupling_test_type(car_bogies);
                    let (back, front) = coupling_points(&lead, lead_reversed, &car, reversed);
                    let heading = 23.0;
                    let (position, model_heading) = coupling_placement(
                        DVec3::ZERO,
                        heading,
                        body_reversed(&lead.def, lead_reversed),
                        back.y,
                        body_reversed(&car.def, reversed),
                        front.y,
                    );
                    let mut v = VehicleInstance::new(
                        lead.clone(),
                        VehicleHost::new(Default::default()),
                    );
                    v.heading = heading;
                    let lead_rot = body_rotation(
                        heading,
                        0.0,
                        0.0,
                        body_reversed(&lead.def, lead_reversed),
                    );
                    let mut t = TrailerPart::new_ex(
                        car.clone(),
                        &lead,
                        lead_reversed,
                        reversed,
                        &lead.program,
                        2,
                        0,
                    );
                    for _ in 0..30 {
                        t.update(&mut v, 0.0, Some((DVec3::ZERO, lead_rot, heading)));
                    }
                    assert!(
                        (t.position - position).length() < 1e-4,
                        "{lead_bogies:?}/{car_bogies:?}, {lead_reversed}/{reversed}"
                    );
                    assert!((t.body_heading() - model_heading).abs() < 1e-4);
                    if lead_bogies.is_some() && car_bogies.is_some() {
                        assert!((position.length() - 10.0).abs() < 1e-4);
                    }
                    let joint = lead_rot.transform_point3(back).as_dvec3();
                    assert!(
                        (joint
                            - t.position
                            - t.body_rotation().transform_point3(front).as_dvec3())
                        .length()
                            < 1e-4
                    );
                }
            }
        }
    }
}

#[test]
fn coupling_defaults_and_non_bogie_rail_bounds_are_preserved() {
    // a coupling the file does not declare is at the model origin, as OMSI keeps it
    let mut ty = coupling_test_type(None);
    let def = &mut Arc::get_mut(&mut ty).unwrap().def;
    def.coupling_front = None;
    def.coupling_back = None;
    assert_eq!(coupling_points(&ty, false, &ty, false), (Vec3::ZERO, Vec3::ZERO));
    // an AI copy's type couples where it always has
    for bogies in [None, Some(5.0), Some(-5.0)] {
        let mut ty = coupling_test_type(bogies);
        let def = &mut Arc::get_mut(&mut ty).unwrap().def;
        def.coupling_front = None;
        def.coupling_back = None;
        super::vehicle_type::ai_couplings(def);
        let (back, front) = coupling_points(&ty, false, &ty, false);
        let sign = if bogies == Some(-5.0) { -1.0 } else { 1.0 };
        let z = if bogies.is_some() { 0.0 } else { 0.3 };
        assert_eq!(back, Vec3::new(0.0, -4.0 * sign, z));
        assert_eq!(front, Vec3::new(0.0, 4.0 * sign, z));
    }
    let mut ty = coupling_test_type(None);
    Arc::get_mut(&mut ty).unwrap().def.rail_body_osc = Some([0.0; 7]);
    let (back, front) = coupling_points(&ty, false, &ty, false);
    assert_eq!((back.y, front.y), (-6.2, 6.2));
}

#[test]
fn inset_rail_joints_and_unit_couplers_keep_their_declared_distances() {
    // Two three-part units: the outer ends have longer couplers than the
    // internal joints. Mesh bounds extending past a joint must not stretch it.
    for bogies in [5.0, -5.0] {
        for half_length in [6.6, 7.8] {
            let part = |front: f32, back: f32| {
                let mut ty = coupling_test_type(Some(bogies));
                let ty_mut = Arc::get_mut(&mut ty).unwrap();
                ty_mut.def.coupling_front.as_mut().unwrap().pos[1] = front;
                ty_mut.def.coupling_back.as_mut().unwrap().pos[1] = back;
                ty_mut.mesh_boxes = vec![(
                    Vec3::new(-1.0, -half_length, 0.0),
                    Vec3::new(1.0, half_length, 3.0),
                )];
                ty
            };
            let lead = part(6.3, -5.7);
            let middle = part(5.7, -5.7);
            let tail = part(5.7, -6.3);
            let mut v = VehicleInstance::new(
                lead.clone(),
                VehicleHost::new(Default::default()),
            );
            for ty in [&middle, &tail, &lead, &middle, &tail] {
                v.attach_trailer_ex(ty.clone(), false);
            }
            for heading in [0.0_f64, 180.0] {
                v.heading = heading;
                for t in &mut v.trailers {
                    t.realign();
                }
                for _ in 0..30 {
                    v.update_trailers(0.0);
                }
                let h = heading.to_radians();
                let forward = DVec3::new(h.sin(), h.cos(), 0.0);
                let mut previous = v.position;
                for (t, distance) in v.trailers.iter().zip([11.4, 11.4, 12.6, 11.4, 11.4]) {
                    assert!((previous - t.position - forward * distance).length() < 1e-4);
                    previous = t.position;
                }
            }
        }
    }
}

#[test]
fn asymmetric_rail_ends_distinguish_declared_joints_from_body_bounds() {
    for bogies in [None, Some(5.0), Some(-5.0)] {
        let part = |front: f32, back: f32, min_y: f32, max_y: f32| {
            let mut ty = coupling_test_type(bogies);
            let ty_mut = Arc::get_mut(&mut ty).unwrap();
            ty_mut.def.rail_body_osc = Some([0.0; 7]);
            ty_mut.def.coupling_front.as_mut().unwrap().pos[1] = front;
            ty_mut.def.coupling_back.as_mut().unwrap().pos[1] = back;
            ty_mut.mesh_boxes = vec![(
                Vec3::new(-1.0, min_y, 0.0),
                Vec3::new(1.0, max_y, 3.0),
            )];
            ty
        };
        // The first joint is inside the lead body; the tail's joints extend
        // beyond its body. Bogie-defined cars must retain these declared
        // distances even when they produce an overlap or a visible gap.
        let lead = part(15.0, -9.0, -12.0, 15.0);
        let middle = part(13.0, -13.0, -13.0, 13.0);
        let tail = part(14.0, -16.0, -15.0, 12.0);
        for middle_reversed in [false, true] {
            for tail_reversed in [false, true] {
                let mut v = VehicleInstance::new(
                    lead.clone(),
                    VehicleHost::new(Default::default()),
                );
                v.attach_trailer_ex(middle.clone(), middle_reversed);
                v.attach_trailer_ex(tail.clone(), tail_reversed);
                // Without bogies, rail cars still meet at their mesh bounds.
                let distances = if bogies.is_some() {
                    [22.0, if tail_reversed { 29.0 } else { 27.0 }]
                } else {
                    [25.0, if tail_reversed { 28.0 } else { 25.0 }]
                };
                for heading in [37.0_f64, 180.0] {
                    v.heading = heading;
                    for t in &mut v.trailers {
                        t.realign();
                    }
                    let h = heading.to_radians();
                    let forward = DVec3::new(h.sin(), h.cos(), 0.0);
                    for _ in 0..30 {
                        v.update_trailers(0.0);
                        let mut previous = v.position;
                        for (i, (previous_ty, previous_reversed, ty, reversed)) in [
                            (&lead, false, &middle, middle_reversed),
                            (&middle, middle_reversed, &tail, tail_reversed),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let expected = previous - forward * distances[i];
                            let (back, front) = coupling_points(
                                previous_ty,
                                previous_reversed,
                                ty,
                                reversed,
                            );
                            let (spawn_position, spawn_heading) = coupling_placement(
                                previous,
                                heading,
                                body_reversed(&previous_ty.def, previous_reversed),
                                back.y,
                                body_reversed(&ty.def, reversed),
                                front.y,
                            );
                            let t = &v.trailers[i];
                            assert!((spawn_position - expected).length() < 1e-4);
                            assert!((t.position - expected).length() < 1e-4);
                            assert!((t.body_heading() - spawn_heading).abs() < 1e-4);
                            previous = expected;
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn rail_coupling_distance_does_not_displace_cars_off_track() {
    for bogies in [None, Some(5.0), Some(-5.0)] {
        let mut lead = coupling_test_type(bogies);
        let mut car = coupling_test_type(bogies);
        Arc::get_mut(&mut lead)
            .unwrap()
            .def
            .coupling_back
            .as_mut()
            .unwrap()
            .pos = [0.6, -5.0, 0.8];
        Arc::get_mut(&mut car)
            .unwrap()
            .def
            .coupling_front
            .as_mut()
            .unwrap()
            .pos = [-0.2, 5.0, 3.6];
        let mut v = VehicleInstance::new(lead, VehicleHost::new(Default::default()));
        v.position.z = 2.0;
        v.attach_trailer_ex(car, false);
        for _ in 0..30 {
            v.update_trailers(0.0);
        }
        let offset = v.trailers[0].position - v.position;
        assert!((offset.y + 10.0).abs() < 1e-5);
        if bogies.is_some() {
            assert!(offset.x.abs() < 1e-5 && offset.z.abs() < 1e-5);
        } else {
            // Road sections still meet at their full three-dimensional joint.
            assert!((offset.x - 0.8).abs() < 1e-5);
            assert!((offset.z + 2.8).abs() < 1e-5);
        }
    }
}

/// The front section pitched 4° nose up (nothing under the rear axle: it hangs level
/// with the coupling). A puller's joint pitches at the ball, which stays the coupling
/// point. A pusher's (`[coupling_pitch_offset]`) pitches `d` ahead of it on the front
/// section: the ball rides a link of length `d` that pitches with the rear section and
/// swings up off the coupling point by about d·sin(beta) - more for a longer link - while
/// the turn about the ball (alpha) is the puller's.
#[test]
fn a_pushers_joint_pitches_ahead_of_the_ball() {
    let joint = |d: Option<f32>, heading: f64| {
        let mut car = coupling_test_type(None);
        Arc::get_mut(&mut car).unwrap().def.coupling_pitch_offset = d;
        let mut v = VehicleInstance::new(coupling_test_type(None), VehicleHost::new(Default::default()));
        v.pitch = 4.0;
        v.attach_trailer_ex(car, false);
        v.update_trailers(0.0);
        v.heading = heading;
        for _ in 0..30 {
            v.update_trailers(0.0);
        }
        let t = &v.trailers[0];
        let c = t.coupling_point(v.position, v.body_rotation());
        let ball = t.position + t.body_rotation().transform_point3(t.couplings().1).as_dvec3();
        let hinge = t.pitch_hinge(v.position, v.body_rotation());
        let alpha = ((v.heading - t.heading + 540.0) % 360.0) - 180.0;
        (c, ball, hinge, t.pitch as f64, alpha)
    };
    // a puller's: hinge and ball are the coupling point, the rear section stays level
    let (c, ball, hinge, pitch, _) = joint(None, 0.0);
    assert!((ball - c).length() < 1e-9 && (hinge - c).length() < 1e-9, "{ball:?} {hinge:?} {c:?}");
    assert!(pitch.abs() < 1e-6, "{pitch}");
    for d in [0.4f32, 0.785, 1.2] {
        let (c, ball, hinge, pitch, _) = joint(Some(d), 0.0);
        let d = d as f64;
        // the hinge stands d ahead of the coupling, along the front section
        assert!(((hinge - c).length() - d).abs() < 1e-6, "{hinge:?} {c:?}");
        assert!(hinge.z > c.z && hinge.y > c.y, "{hinge:?} {c:?}");
        // the link keeps its length, at the rear section's pitch
        assert!(((ball - hinge).length() - d).abs() < 1e-6, "{ball:?} {hinge:?}");
        let fall = (hinge.z - ball.z) / d;
        assert!((fall - pitch.to_radians().sin()).abs() < 1e-6, "{fall} {pitch}");
        // the rear section pitches less than the front; the ball has swung up off the
        // coupling point by about d·sin(beta)
        let beta = 4.0 - pitch;
        assert!(pitch > 0.0 && beta > 0.0, "{pitch}");
        let up = ball.z - c.z;
        assert!(up > 0.0 && (up - d * beta.to_radians().sin()).abs() < 0.002 * d, "d {d}: up {up}, beta {beta}");
    }
    // the turn about the ball is the puller's
    let (_, _, _, _, a0) = joint(None, 20.0);
    let (_, _, _, _, a1) = joint(Some(0.785), 20.0);
    assert!(a0.abs() > 1.0 && (a0 - a1).abs() < 1e-6, "{a0} {a1}");
}

fn synthetic_rail_consist() -> VehicleInstance {
    let ty = coupling_test_type(Some(-5.0));
    let mut v = VehicleInstance::new(ty.clone(), VehicleHost::new(Default::default()));
    for reversed in [false, true, true] {
        v.attach_trailer_ex(ty.clone(), reversed);
    }
    v
}

#[test]
fn negative_bogie_consist_has_outward_cabs_and_declared_spacing() {
    let mut v = synthetic_rail_consist();
    for heading in [0.0_f64, 180.0] {
        v.heading = heading;
        for t in &mut v.trailers {
            t.realign();
        }
        for _ in 0..30 {
            v.update_trailers(0.0);
        }
        let h = heading.to_radians();
        let forward = Vec3::new(h.sin() as f32, h.cos() as f32, 0.0);
        // Model the cab pointing towards the body's negative longitudinal end.
        let front_cab = v.body_rotation().transform_vector3(Vec3::NEG_Y);
        let rear_cab = v
            .trailers
            .last()
            .unwrap()
            .body_rotation()
            .transform_vector3(Vec3::NEG_Y);
        assert!(
            front_cab.dot(forward) > 0.99,
            "front cab faces inward: {front_cab:?}"
        );
        assert!(
            rear_cab.dot(forward) < -0.99,
            "rear cab faces inward: {rear_cab:?}"
        );
        let mut previous = v.position;
        for t in &v.trailers {
            let offset = previous - t.position;
            assert!(
                (offset.dot(forward.as_dvec3()) - 10.0).abs() < 1e-4,
                "car spacing {offset:?}"
            );
            assert!(
                offset.cross(forward.as_dvec3()).length() < 1e-4,
                "car off track: {offset:?}"
            );
            previous = t.position;
        }
    }
}

#[test]
fn rail_joints_stay_closed_on_curved_graded_track() {
    let mut v = synthetic_rail_consist();
    for direction in [1.0_f64, -1.0] {
        v.heading = if direction > 0.0 { 0.0 } else { 180.0 };
        v.pitch = 0.03_f32.atan().to_degrees();
        for t in &mut v.trailers {
            t.realign();
        }
        let track = |d: f64| {
            let a = d / 150.0;
            Some(DVec3::new(
                150.0 * (1.0 - a.cos()) * direction,
                -150.0 * a.sin() * direction,
                -d * 0.03,
            ))
        };
        for _ in 0..60 {
            v.retrail(1.0 / 30.0, &track);
            v.update_trailers(0.0);
            let (mut origin, mut rotation) = (v.position, v.body_rotation());
            for t in &v.trailers {
                let (back, front) = t.couplings();
                let lead_joint = origin + rotation.transform_point3(back).as_dvec3();
                let own_joint =
                    t.position + t.body_rotation().transform_point3(front).as_dvec3();
                assert!((lead_joint - own_joint).length() < 1e-4);
                assert!(t.position.is_finite() && t.body_rotation().is_finite());
                let rail = t.track.expect("rail contact");
                assert!(rail.z < 0.0, "contact left the descending track");
                assert!((t.axle_z.unwrap() - rail.z).abs() < 1e-4);
                origin = t.position;
                rotation = t.body_rotation();
            }
        }
    }
}

#[test]
fn a_borrowed_part_is_judged_with_its_own_pack() {
    // the same machine from its own pack, whichever bus borrows it (#977)
    let a = winding_pack(Path::new("/omsi/Vehicles/Citelis/model/body.o3d"));
    let b = winding_pack(Path::new("/omsi/vehicles/Atron_AFR4/model/afr4.o3d"));
    let c = winding_pack(Path::new("/omsi/Vehicles/MAN_SD200/model/../../Atron_AFR4/model/afr4.o3d"));
    assert_eq!(a, "citelis");
    assert_eq!(b, "atron_afr4");
    assert_eq!(winding_pack(Path::new("/omsi/Vehicles/Atron_AFR4/model/sub/afr4.o3d")), b);
    assert_eq!(c, b);
    let elsewhere = winding_pack(Path::new("/omsi/Sceneryobjects/x/model/y.o3d"));
    assert_eq!(elsewhere, winding_pack(Path::new("/omsi/Sceneryobjects/x/model/z.o3d")));
    assert_ne!(elsewhere, winding_pack(Path::new("/omsi/Sceneryobjects/w/model/y.o3d")));
}

fn restored_display_vehicle() -> VehicleInstance {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "omsi_restore_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("display.osc");
    std::fs::write(
        &script,
        r#"
{frame}
(M.L.Matrix_frame)
(M.L.line_draw)
{end}
{macro:line_draw}
(L.$.line_request) (S.$.line_display)
{end}
{macro:Matrix_frame}
(L.L.power) (L.L.IBIS_Linie_Complex) (L.L.Matrix_Nr_Last) = ! &&
(L.L.power) (L.L.IBIS_TerminusIndex) (L.L.Matrix_TerminusIndex_Last) = ! && ||
{if}
0 (M.V.STNewTex)
0 (M.V.STLock)
0 255 255 120 0 (M.V.STSetColor)
0 0 0 4 2 (M.V.STDrawRect)
0 (M.V.STUnlock)
(L.L.IBIS_Linie_Complex) (S.L.Matrix_Nr_Last)
(L.L.IBIS_TerminusIndex) (S.L.Matrix_TerminusIndex_Last)
{endif}
{end}
"#,
    )
    .unwrap();
    let vars = dir.join("vars.txt");
    std::fs::write(&vars, "power\nIBIS_Linie_Complex\nIBIS_TerminusIndex\nMatrix_Nr_Last\nMatrix_TerminusIndex_Last\n").unwrap();
    let strings = dir.join("strings.txt");
    std::fs::write(&strings, "line_request\nline_display\n").unwrap();
    let program = omsi_script::compile(&omsi_script::CompileInput {
        scripts: vec![script],
        varlists: vec![vars],
        stringvarlists: vec![strings],
        ..Default::default()
    });
    assert!(program.errors.is_empty(), "{:?}", program.errors);
    let mut program = program;
    program.declare_str_var("destination");
    let mut model = Model::default();
    model.script_textures = vec![(4, 2)];
    model.text_textures.push(omsi_model::TextTexture {
        variable: "destination".into(),
        width: 4,
        height: 2,
        ..Default::default()
    });
    model.text_textures.push(omsi_model::TextTexture {
        variable: "line_display".into(),
        width: 900,
        height: 100,
        ..Default::default()
    });
    let ty = Arc::new(VehicleType {
        def: Default::default(),
        model,
        model_dir: dir.clone(),
        program: Arc::new(program),
        meshes: Vec::new(),
        paint_schemes: Vec::new(),
        texchanges: Vec::new(),
        wheel_meshes: Vec::new(),
        suspension_axles: Vec::new(),
        missing_packs: Vec::new(),
        mesh_bounds: Vec::new(),
        mesh_boxes: Vec::new(),
    });
    std::fs::remove_dir_all(dir).unwrap();
    VehicleInstance::new(ty, VehicleHost::new(Default::default()))
}

#[test]
fn restore_refreshes_unchanged_bitmap_without_retyping_or_powering_on() {
    for power in [0.0, 1.0] {
        let mut original = restored_display_vehicle();
        original.set_var("power", power);
        original.set_var("IBIS_Linie_Complex", 10900.0);
        original.set_var("IBIS_TerminusIndex", 9.0);
        original.update(0.02);
        let vars: Vec<_> = original
            .ty
            .program
            .var_names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), original.state.vars[i]))
            .collect();
        let mut resumed =
            VehicleInstance::new(original.ty.clone(), VehicleHost::new(Default::default()));
        resumed.restore_script_state(
            &vars,
            &[("destination".into(), "  Manual destination  ".into())],
        );
        assert_eq!(resumed.var("power"), Some(power));
        assert_eq!(resumed.var("IBIS_Linie_Complex"), Some(10900.0));
        assert_eq!(resumed.var("IBIS_TerminusIndex"), Some(9.0));
        assert_eq!(resumed.str_var("destination"), "  Manual destination  ");
        assert!(resumed.host.script_textures[0].rgba.iter().all(|p| *p == 0));
        resumed.update(0.02);
        assert_eq!(
            resumed.host.script_textures[0].rgba,
            original.host.script_textures[0].rgba
        );
        assert_eq!(
            resumed.host.script_textures[0].rgba.iter().any(|p| *p != 0),
            power > 0.0
        );
    }
}

#[test]
fn string_only_restore_preserves_and_reuploads_main_and_articulated_displays() {
    for power in [0.0, 1.0] {
        let mut v = restored_display_vehicle();
        v.set_var("power", power);
        v.attach_trailer(v.ty.clone());
        for def in &v.ty.model.text_textures {
            v.text_textures
                .push(crate::texttex::TextTextureState::new(def.clone(), None));
            v.trailers[0]
                .text_textures
                .push(crate::texttex::TextTextureState::new(def.clone(), None));
        }
        let strings = [
            ("line_request".into(), "X9                            ".into()),
            ("line_display".into(), "X9                            ".into()),
            ("destination".into(), "  Manual destination  ".into()),
        ];
        // Repeat with already uploaded, unchanged text: a newly bound texture still
        // needs its image, even when no numeric variables were saved.
        for _ in 0..2 {
            v.restore_script_state(&[], &strings);
            v.update(0.02);
            assert_eq!(v.var("power"), Some(power));
            assert_eq!(v.str_var("destination"), "  Manual destination  ");
            assert_eq!(v.str_var("line_request"), "X9                            ");
            assert_eq!(v.str_var("line_display"), "X9                            ");
            assert_eq!(v.update_text_textures(), vec![0, 1]);
            let mut part = v.trailers.pop().unwrap();
            assert_eq!(part.update_text_textures(&v), vec![0, 1]);
            assert_eq!(
                part.text_textures[1].last_text.as_deref(),
                Some("X9                            ")
            );
            v.trailers.push(part);
            for t in &mut v.text_textures {
                t.pending.take();
            }
            assert!(v.update_text_textures().is_empty());
        }
    }
}

#[test]
fn script_speed_reports_tiny_resting_motion_as_stopped() {
    assert_eq!(script_speed(0.000251), 0.0);
    assert_eq!(script_speed(-0.000251), 0.0);
    assert_eq!(script_speed(0.02), 0.02);
}

/// A shadow blob at the model's z = 0 is laid onto the plane through the wheels: 15 cm
/// up with the body sagging, and following a pitch; one axle gives a level plane.
#[test]
fn shadow_blob_lies_on_the_wheels_plane() {
    let wheels = [
        Vec3::new(-1.0, 3.2, 0.13),
        Vec3::new(1.0, 3.2, 0.13),
        Vec3::new(-1.0, -2.6, 0.17),
        Vec3::new(1.0, -2.6, 0.17),
    ];
    let p = fit_plane(&wheels);
    for w in wheels {
        assert!(
            (p[0] + p[1] * w.x + p[2] * w.y - w.z).abs() < 1e-4,
            "{p:?} at {w:?}"
        );
    }
    let m = onto_plane(p);
    for corner in [Vec3::new(1.4, 5.7, 0.0), Vec3::new(-1.4, -5.7, 0.0)] {
        let q = m.transform_point3(corner);
        assert!((q.truncate() - corner.truncate()).length() < 1e-5);
        assert!(
            (q.z - (p[0] + p[2] * corner.y) - SHADOW_LIFT).abs() < 1e-4,
            "{q:?}"
        );
    }
    let axle = fit_plane(&[Vec3::new(-1.0, -0.4, 0.2), Vec3::new(1.0, -0.4, 0.1)]);
    assert!(
        (axle[0] - 0.15).abs() < 1e-5 && axle[1] == 0.0 && axle[2] == 0.0,
        "{axle:?}"
    );
}

/// Volvo Wright's dashboard rear-close trigger falls back to its explicit external-close
/// path when the handbrake guard only produced the button sound.
#[test]
fn volvo_wright_rear_close_fallback_moves_a_partly_open_door() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/Volvo_Wright_Family/AVBWS1.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("AVBWS1"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    for (name, value) in [
        ("elec_real_main", 1.0),
        ("elec_busbar_main", 1.0),
        ("elec_busbar_avail", 1.0),
        ("cockpit_button_ignition", 1.0),
        ("bremse_feststell_sw", 0.0),
        ("cockpit_button_smallhb", 0.0),
        ("door_2", 0.5),
        ("door_3", 0.5),
        ("doorTarget_23", 1.0),
    ] {
        assert!(v.set_var(name, value), "missing {name}");
    }
    assert!(v.trigger("bus_dooraftclose"));
    assert_eq!(v.var("doorTarget_23"), Some(0.0));
}

#[test]
fn volvo_wright_rear_toggle_falls_back_to_external_close() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/Volvo_Wright_Family/AVBWS1.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("AVBWS1"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    for (name, value) in [
        ("elec_real_main", 1.0),
        ("elec_busbar_main", 1.0),
        ("elec_busbar_avail", 1.0),
        ("cockpit_button_ignition", 1.0),
        ("bremse_feststell_sw", 0.0),
        ("cockpit_button_smallhb", 0.0),
        ("door_2", 0.5),
        ("door_3", 0.5),
        ("doorTarget_23", 1.0),
    ] {
        assert!(v.set_var(name, value), "missing {name}");
    }
    assert!(v.trigger("bus_dooraft"));
    assert_eq!(v.var("doorTarget_23"), Some(0.0));
    assert_eq!(v.var("bdoor_embtn_cls"), Some(1.0));
}

#[test]
fn volvo_wright_rear_toggle_closes_open_leaves_even_with_zero_target() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/Volvo_Wright_Family/AVBWS1.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("AVBWS1"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    for (name, value) in [
        ("elec_real_main", 1.0),
        ("elec_busbar_main", 1.0),
        ("elec_busbar_avail", 1.0),
        ("cockpit_button_ignition", 1.0),
        ("bremse_feststell_sw", 0.0),
        ("cockpit_button_smallhb", 0.0),
        ("door_2", 0.5),
        ("door_3", 0.5),
        ("doorTarget_23", 0.0),
    ] {
        assert!(v.set_var(name, value), "missing {name}");
    }
    assert!(v.trigger("bus_dooraft"));
    assert_eq!(v.var("doorTarget_23"), Some(0.0));
}

#[test]
fn volvo_wright_rear_toggle_reopens_during_forced_close() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/Volvo_Wright_Family/AVBWS1.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("AVBWS1"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    for (name, value) in [
        ("elec_real_main", 1.0),
        ("elec_busbar_main", 1.0),
        ("elec_busbar_avail", 1.0),
        ("cockpit_button_ignition", 1.0),
        ("bremse_feststell_sw", 1.0),
        ("cockpit_button_smallhb", 0.0),
        ("door_2", 0.5),
        ("door_3", 0.5),
        ("doorTarget_23", 0.0),
        ("bdoor_embtn_cls", 1.0),
    ] {
        assert!(v.set_var(name, value), "missing {name}");
    }
    assert!(v.trigger("bus_dooraft"));
    assert_eq!(v.var("doorTarget_23"), Some(1.0));
    assert_eq!(v.var("bdoor_embtn_cls"), Some(0.0));
}

#[test]
fn volvo_wright_rear_toggle_keeps_close_target_through_frames() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/Volvo_Wright_Family/AVBWS1.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("AVBWS1"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    for (name, value) in [
        ("elec_real_main", 1.0),
        ("elec_busbar_main", 1.0),
        ("elec_busbar_avail", 1.0),
        ("cockpit_button_ignition", 1.0),
        ("bremse_feststell_sw", 0.0),
        ("cockpit_button_smallhb", 0.0),
        ("bremse_p_Tank04", 800000.0),
        ("door_2", 1.0),
        ("door_3", 1.0),
        ("doorTarget_23", 1.0),
        ("bdoor_sound_played", 1.0),
    ] {
        assert!(v.set_var(name, value), "missing {name}");
    }
    assert!(v.trigger("bus_dooraft"));
    v.update(1.0 / 30.0);
    assert_eq!(v.var("backdoor_buzzer"), Some(1.0));
    for _ in 1..120 {
        v.update(1.0 / 30.0);
    }
    assert_eq!(v.var("doorTarget_23"), Some(0.0));
    assert!(v.var("door_2").unwrap_or(1.0) < 0.1);
    assert!(v.var("door_3").unwrap_or(1.0) < 0.1);
    assert!(v.host.fired_triggers.iter().any(|name| name == "ev_doortriggerclose_2"));
}

#[test]
fn volvo_wright_family_rear_toggle_closes_every_bus_variant() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let dir = root.join("Vehicles/Volvo_Wright_Family");
    if !dir.is_dir() {
        eprintln!("skipped: no {}", dir.display());
        return;
    }
    let mut buses: Vec<_> = std::fs::read_dir(&dir)
        .expect("Volvo Wright directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("bus")))
        .collect();
    buses.sort();
    assert!(buses.len() >= 20, "unexpectedly few Volvo Wright buses: {}", buses.len());
    for bus in buses {
        let name = bus.file_name().unwrap().to_string_lossy().into_owned();
        let ty = Arc::new(VehicleType::load(&root, &bus).unwrap_or_else(|e| panic!("{name}: {e}")));
        let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
        for (var, value) in [
            ("elec_real_main", 1.0),
            ("elec_busbar_main", 1.0),
            ("elec_busbar_avail", 1.0),
            ("cockpit_button_ignition", 1.0),
            ("bremse_feststell_sw", 0.0),
            ("cockpit_button_smallhb", 0.0),
            ("door_2", 0.5),
            ("door_3", 0.5),
            ("doorTarget_23", 1.0),
        ] {
            if v.var(var).is_some() {
                assert!(v.set_var(var, value), "{name}: missing {var}");
            }
        }
        assert!(v.trigger("bus_dooraft"), "{name}: missing bus_dooraft");
        let target = v.var("doorTarget_23").unwrap_or(0.0);
        let request = v.var("door_back_close_request").unwrap_or(0.0);
        assert!(target == 0.0 || request > 0.0, "{name}: rear door stayed open (target={target}, request={request})");
    }
}

/// The wheel of a body without a rigid body stands on the road the drawn faces put
/// under it, not on the deck of a bridge over that road (or a canopy above it): the
/// plain sampler knows only x and y and gives the highest face there, which laid the
/// `[isshadow]` blob up on the deck. Where the faces put nothing near the wheel - and
/// where there is no face probe at all - the plain sampler still answers.
#[test]
fn a_wheels_ground_is_the_road_under_it_not_the_highest_face() {
    let (road, deck) = (12.0f64, 17.0f64);
    // the faces: the deck above the wheel, the road under it
    let faces = |_x: f64, _y: f64, top: f64| crate::physics::GroundProbe {
        below: [road, deck].into_iter().filter(|h| *h <= top).fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b)))),
        above: None,
    };
    let faces: &dyn crate::physics::Ground = &faces;
    // the plain sampler: the highest face at (x, y), which is the deck
    let plain = |_x: f64, _y: f64| Some(deck);
    let plain: &(dyn Fn(f64, f64) -> Option<f64> + Send + Sync) = &plain;
    let wheel = DVec3::new(100.0, 200.0, road);
    assert_eq!(wheel_ground(Some(faces), Some(plain), wheel), Some(road));
    // a wheel standing on the deck itself gets the deck
    assert_eq!(wheel_ground(Some(faces), Some(plain), DVec3::new(100.0, 200.0, deck)), Some(deck));
    // nothing drawn within a step of the wheel: the plain sampler, as before
    let empty = |_x: f64, _y: f64, _top: f64| crate::physics::GroundProbe { below: None, above: Some(deck) };
    let empty: &dyn crate::physics::Ground = &empty;
    assert_eq!(wheel_ground(Some(empty), Some(plain), wheel), Some(deck));
    // no face probe at all (a rail or air lane): the plain sampler
    assert_eq!(wheel_ground(None, Some(plain), wheel), Some(deck));
}

/// #95: `GetHeightAbovePoint` (OMSI's name, kept for compatibility) measures the height of
/// the point over what lies *under* it, not up to a roof over it. The plain sampler knows
/// only x and y and gives the highest face there: a bus stopped under a canopy read the
/// canopy, its lift sensor got a large negative height and the lift and ramp could not be
/// lowered. Only faces at or below the point count.
#[test]
fn the_ground_probe_reads_the_ground_under_a_roof() {
    let (road, kerb, roof) = (0.0f64, 0.15f64, 4.5f64);
    let mut v = VehicleInstance::new(coupling_test_type(None), VehicleHost::new(crate::SimClock::default()));
    v.position = DVec3::new(100.0, 200.0, road);
    // a kerb right of x = 1 m (world), a canopy over everything
    let faces = move |x: f64, _y: f64, top: f64| {
        let kerb = if x > 101.0 { kerb } else { road };
        crate::physics::GroundProbe {
            below: [road, kerb, roof].into_iter().filter(|h| *h <= top).reduce(f64::max),
            above: (roof > top).then_some(roof),
        }
    };
    v.contact = Some(Arc::new(faces));
    v.ground = Some(Arc::new(move |_x: f64, _y: f64| Some(roof)));
    v.update_ground_probe();
    let probe = v.host.ground_probe.clone().expect("probe");
    assert!((probe(0.0, 0.0, 0.3) - 0.3).abs() < 1e-4, "lift sensor read {}", probe(0.0, 0.0, 0.3));
    assert!((probe(0.0, 0.0, 0.0)).abs() < 1e-4);
    // over the kerb: the kerb; below its top: what lies under the point, the road
    assert!((probe(1.5, 0.0, 0.25) - 0.10).abs() < 1e-4, "kerb read {}", probe(1.5, 0.0, 0.25));
    assert!((probe(1.5, 0.0, 0.10) - 0.10).abs() < 1e-4, "under the kerb read {}", probe(1.5, 0.0, 0.10));
    // a point up on the canopy itself stands on it
    assert!((probe(0.0, 0.0, 4.6) - 0.1).abs() < 1e-4);
    // no face probe (a vehicle with only the plain sampler): as before
    v.contact = None;
    v.ground = Some(Arc::new(move |_x: f64, _y: f64| Some(road)));
    v.update_ground_probe();
    let probe = v.host.ground_probe.clone().expect("probe");
    assert!((probe(0.0, 0.0, 0.3) - 0.3).abs() < 1e-4);
}

/// The resolved property plan gives what `compute_mesh_props` gives, for a stock bus
/// with its variables set to changing values, and after an engine variable joins.
#[test]
fn props_plan_matches_compute_mesh_props() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_NL_NG/MAN_EN92_main.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("EN92"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    let compare = |v: &VehicleInstance| {
        let mut plan = PropsPlan::default();
        plan.refresh(&v.ty, &v.var_index);
        let mut got = vec![MeshProps::default(); 3];
        plan.apply(&v.state.vars, &mut got);
        let want = compute_mesh_props(&v.ty, &|n| v.var(n));
        assert_eq!(got.len(), want.len());
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            assert_eq!(
                (
                    g.visible,
                    &g.slot_alpha,
                    &g.slot_light,
                    &g.slot_night,
                    &g.slot_uv,
                    g.interior
                ),
                (
                    w.visible,
                    &w.slot_alpha,
                    &w.slot_light,
                    &w.slot_night,
                    &w.slot_uv,
                    w.interior
                ),
                "mesh {i} {}",
                v.ty.model.meshes[v.ty.meshes[i].def_index].file
            );
        }
    };
    for round in 0..4 {
        for (k, x) in v.state.vars.iter_mut().enumerate() {
            *x = ((k * 7 + round * 13) % 5) as f32 * 0.37 - 0.2;
        }
        compare(&v);
    }
    // Dirt_Norm is not declared by the scripts; the engine adds it later
    v.set_engine_var("Dirt_Norm", 0.63);
    compare(&v);
}

/// A `[matl_lightmap]` whose variable the bus does not have is always on (OMSI's
/// index -1); one on a variable at 0.3 is off.
#[test]
fn a_lightmap_on_an_unknown_variable_is_on() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_NL_NG/MAN_EN92_main.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let light = |name: Option<&str>, value: f32| {
        let mut ty = VehicleType::load(&root, &bus).expect("EN92");
        let (i, slot, var) = ty
            .meshes
            .iter()
            .enumerate()
            .find_map(|(i, vm)| {
                ty.model.meshes[vm.def_index].materials.iter().find_map(|m| {
                    let (_, v) = m.lightmaps.first()?;
                    Some((i, override_slot(&vm.materials, m)?, v.clone()))
                })
            })
            .expect("a lightmap");
        let di = ty.meshes[i].def_index;
        for m in ty.model.meshes[di].materials.iter_mut() {
            if let Some(l) = m.lightmaps.first_mut() {
                l.1 = name.unwrap_or(&var).to_string();
                m.lightmaps.truncate(1);
            }
        }
        let mut v = VehicleInstance::new(Arc::new(ty), VehicleHost::new(crate::SimClock::default()));
        for x in v.state.vars.iter_mut() {
            *x = value;
        }
        let mut plan = PropsPlan::default();
        plan.refresh(&v.ty, &v.var_index);
        let mut got = vec![MeshProps::default(); 3];
        plan.apply(&v.state.vars, &mut got);
        let want = compute_mesh_props(&v.ty, &|n| v.var(n));
        assert_eq!(got[i].slot_light[slot], want[i].slot_light[slot]);
        got[i].slot_light[slot]
    };
    assert_eq!(light(Some("no_such_var"), 0.3), 1.0);
    assert_eq!(light(None, 0.3), 0.0);
}

/// The articulated GN92 turning right: the angle goes to `articulation_0_alpha` (the
/// jackknife protection's), and the joint's last dummy - the bone the bellows' rear end
/// hangs on - turns with it onto the rear section's line.
#[test]
fn articulation_alpha_turns_the_joint_onto_the_rear_section() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_NL_NG/MAN_GN92_main.bus");
    let trail = root.join("Vehicles/MAN_NL_NG/MAN_GN92_trail.bus");
    if !bus.exists() || !trail.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("GN92"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    v.attach_trailer_ex(
        Arc::new(VehicleType::load(&root, &trail).expect("GN92 trail")),
        false,
    );
    v.heading = 10.0;
    v.update_visuals(0.02);
    assert!(v.var("articulation_0_alpha").unwrap().abs() < 1e-3);
    // the front section turns right (clockwise) by 20 degrees about its rear coupling's
    // neighbourhood: the rear section still points the old way
    v.heading = 30.0;
    v.update_visuals(0.02);
    let t = &v.trailers[0];
    let alpha = v.var("articulation_0_alpha").unwrap();
    let beta = v.var("articulation_0_beta").unwrap();
    let geometric = ((v.heading - t.heading + 540.0) % 360.0) - 180.0;
    assert!(geometric > 5.0, "{geometric}");
    assert!(
        (alpha.abs() - geometric as f32).abs() < 1e-3,
        "alpha {alpha} for {geometric}"
    );
    assert!(beta.abs() < 1.0, "beta {beta}");
    // where the rear dummy points, in the front section's frame, against where the rear
    // section lies: (−sin a, cos a) forwards for the angle a
    let d =
        v.ty.meshes
            .iter()
            .position(|m| {
                v.ty.model.meshes[m.def_index]
                    .file
                    .to_ascii_lowercase()
                    .ends_with("gelenk_d.o3d")
            })
            .expect("GN_Dum_Gelenk_D");
    let back = v.mesh_transforms[d].transform_vector3(-Vec3::Y);
    let a = (geometric as f32).to_radians();
    let want = Vec3::new(a.sin(), -a.cos(), 0.0);
    assert!(
        (back - want).length() < 0.02,
        "dummy D points {back:?}, the rear section {want:?} (alpha {alpha})"
    );
    // the bellows ([smoothskin] on the dummies) bend with it: the rear ring swings
    // towards the rear section's side, the front ring stays
    let skinned = v.skinned_meshes();
    assert!(!skinned.is_empty(), "no skinned mesh on the GN92");
    for i in skinned {
        let rest = v.ty.meshes[i].data.positions.clone();
        let (pos, nrm) = v.skinned(i).expect("skinned vertices");
        assert_eq!((pos.len(), nrm.len()), (rest.len(), rest.len()));
        let mean_dx = |f: &dyn Fn(f32) -> bool| {
            let d: Vec<f32> = pos
                .iter()
                .zip(&rest)
                .filter(|(_, r)| f(r.y))
                .map(|(p, r)| p.x - r.x)
                .collect();
            d.iter().sum::<f32>() / d.len().max(1) as f32
        };
        let (front, rear) = (mean_dx(&|y| y > -3.8), mean_dx(&|y| y < -4.5));
        assert!(
            front.abs() < 0.05,
            "{}: front ring moved {front}",
            v.ty.model.meshes[v.ty.meshes[i].def_index].file
        );
        assert!(
            rear > 0.1,
            "{}: rear ring moved {rear}",
            v.ty.model.meshes[v.ty.meshes[i].def_index].file
        );
    }
    // straight: the modelled shape
    let mut s =
        VehicleInstance::new(v.ty.clone(), VehicleHost::new(crate::SimClock::default()));
    s.attach_trailer_ex(v.trailers[0].ty.clone(), false);
    s.update_visuals(0.02);
    let i = s.skinned_meshes()[0];
    let rest = s.ty.meshes[i].data.positions.clone();
    let (pos, _) = s.skinned(i).unwrap();
    let worst = pos
        .iter()
        .zip(&rest)
        .map(|(p, r)| (*p - *r).length())
        .fold(0.0f32, f32::max);
    assert!(worst < 1e-3, "straight bellows off by {worst}");
}

#[test]
fn restored_odometer_survives_engine_updates_and_continues_driving() {
    for driven in [0.0, 2.75] {
        let mut v = VehicleInstance::new(coupling_test_type(None), VehicleHost::new(Default::default()));
        v.driven_km = driven;
        v.set_odometer_km(75556.639);
        assert_eq!(v.odometer_km(), 75556.639);
        v.update_engine_vars(0.02);
        assert_eq!(v.odometer_km(), 75556.639);
        assert_eq!(v.var("kmcounter_km"), Some(75556.0));
        assert!((v.var("kmcounter_m").unwrap() - 639.0).abs() < 0.001);
        v.driven_km += 0.5;
        v.update_engine_vars(0.02);
        assert!((v.odometer_km() - 75557.139).abs() < 1e-8);
        assert_eq!(v.var("kmcounter_km"), Some(75557.0));
        assert!((v.var("kmcounter_m").unwrap() - 139.0).abs() < 0.001);
    }
    // A restored zero is deliberate, rather than the sentinel for a random start.
    let mut v = VehicleInstance::new(coupling_test_type(None), VehicleHost::new(Default::default()));
    v.set_odometer_km(0.0);
    v.update_engine_vars(0.02);
    assert_eq!(v.odometer_km(), 0.0);
}

/// A bus without `[kmcounter_init]` starts with OMSI's defaults (in service since
/// 1980, 60000 km a year, +-20 %), not at 0 km; reversing takes the counter back.
#[test]
fn odometer_starts_at_the_default_service_life() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_SD200/MAN_SD77.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let mut ty = VehicleType::load(&root, &bus).expect("SD77");
    ty.def.km_counter_init = None;
    let mut v = VehicleInstance::new(Arc::new(ty), VehicleHost::new(crate::SimClock::default()));
    v.host.clock.year = 2011;
    v.host.clock.day_of_year = 0;
    v.update_engine_vars(0.02);
    let base = v.host.km_base;
    assert!((31.0 * 60000.0 * 0.8 + 8.0..=31.0 * 60000.0 * 1.2 + 18.0).contains(&base), "{base}");
    v.driven_km = -0.5;
    v.update_engine_vars(0.02);
    let km = v.var("kmcounter_km").unwrap() as f64 + v.var("kmcounter_m").unwrap() as f64 / 1000.0;
    assert!((km - (base - 0.5)).abs() < 1.0, "{km} for {base}");
}

/// The GN92's joint stops at `[coupling_front_character]`'s 52.5 degrees: the front
/// section swinging round 90 degrees drags the rear section's axle with it.
#[test]
fn articulation_stops_at_the_coupling_max_alpha() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_NL_NG/MAN_GN92_main.bus");
    let trail = root.join("Vehicles/MAN_NL_NG/MAN_GN92_trail.bus");
    if !bus.exists() || !trail.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("GN92"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    v.attach_trailer_ex(
        Arc::new(VehicleType::load(&root, &trail).expect("GN92 trail")),
        false,
    );
    v.heading = 10.0;
    v.update_visuals(0.02);
    for h in [100.0, -80.0] {
        v.heading = h;
        v.update_visuals(0.02);
        let alpha = v.var("articulation_0_alpha").unwrap();
        assert!((alpha.abs() - 52.5).abs() < 1e-3, "alpha {alpha} at heading {h}");
    }
}

/// The stock rattle (`klappern.osc`) as loud at 144 frames a second as at OMSI's 30.
#[test]
fn a_jolt_rattles_alike_at_any_frame_rate() {
    fn rattle(fps: f32) -> f32 {
        let dt = 1.0 / fps;
        let (mut frames, mut last, mut vol, mut peak) = (super::OmsiFrames::default(), 0.0f32, 0.0f32, 0.0f32);
        for i in 0..(fps as usize) {
            let t = i as f32 * dt;
            // a 6 Hz pitching after a bump, 0.5 m/s² along the bus
            let a = Vec3::new(0.0, 0.5 * (t * 6.0 * std::f32::consts::TAU).sin() * (-t * 3.0).exp(), 0.0);
            let a = frames.push(a, dt);
            let m = (a.x * a.x + a.y * a.y + 0.01 * a.z * a.z).sqrt();
            vol = ((m - last) * 1.0).max(vol * (-dt).exp()).min(1.0);
            last = m;
            peak = peak.max(vol);
        }
        peak
    }
    let (omsi, fast) = (rattle(30.0), rattle(144.0));
    // (taken frame by frame, 144 a second rattled at 0.3 of OMSI's)
    assert!(omsi > 0.2 && omsi < 0.9, "{omsi}");
    assert!(fast > 0.6 * omsi && fast < 1.4 * omsi, "30 fps {omsi}, 144 fps {fast}");
}

/// `A_Trans_*` are the body's acceleration without gravity, as in OMSI: 0 for a bus
/// standing still, on the level or on a grade, and the braking's deceleration alone.
#[test]
fn scripts_acceleration_leaves_gravity_out() {
    let level = super::scripts_acceleration(Vec3::new(0.0, 0.0, 9.81), Quat::IDENTITY);
    assert!(level.length() < 1e-4, "{level}");
    // standing nose up on a 10 % grade: the accelerometer reads gravity's share along it
    let rot = Quat::from_rotation_x(0.1f32.atan());
    let reading = rot.inverse().mul_vec3(Vec3::new(0.0, 0.0, 9.81));
    let grade = super::scripts_acceleration(reading, rot);
    assert!(grade.length() < 1e-4, "{grade}");
    // braking at 3 m/s² on the level
    let braking = super::scripts_acceleration(Vec3::new(0.0, -3.0, 9.81), Quat::IDENTITY);
    assert!((braking - Vec3::new(0.0, -3.0, 0.0)).length() < 1e-4, "{braking}");
}

/// A rear section turns about its own `[rot_pnt_long]` line: the stock GN92's is its
/// axle; one set ahead of the axle (a steered rear axle, #322) is where it turns.
#[test]
fn rear_section_turns_about_its_rot_pnt_long() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_NL_NG/MAN_GN92_main.bus");
    let trail = root.join("Vehicles/MAN_NL_NG/MAN_GN92_trail.bus");
    if !bus.exists() || !trail.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("GN92"));
    let mut tt = VehicleType::load(&root, &trail).expect("GN92 trail");
    let stock = TrailerPart::new(Arc::new(VehicleType::load(&root, &trail).unwrap()), &ty, &ty.program, 2);
    assert!((stock.pivot_length() - (4.169 + 0.387)).abs() < 1e-3, "{}", stock.pivot_length());
    tt.def.rot_pnt_long = 1.0;
    let steered = TrailerPart::new(Arc::new(tt), &ty, &ty.program, 2);
    assert!((steered.pivot_length() - (4.169 - 1.0)).abs() < 1e-3, "{}", steered.pivot_length());
}

/// The rear section of an articulated bus on a viaduct stays on the deck: one frame with
/// no deck under its axle (a gap at a joint) does not drop it onto the road below, and
/// one that had sunk under the deck finds it again (#135).
#[test]
fn rear_section_stays_on_a_viaduct_deck() {
    use std::sync::atomic::{AtomicU8, Ordering};
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_NL_NG/MAN_GN92_main.bus");
    let trail = root.join("Vehicles/MAN_NL_NG/MAN_GN92_trail.bus");
    if !bus.exists() || !trail.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("GN92"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    v.attach_trailer_ex(Arc::new(VehicleType::load(&root, &trail).expect("GN92 trail")), false);
    // 0: a deck at 10 m over a road at 0; 1: a gap in the deck
    let mode = Arc::new(AtomicU8::new(0));
    let m = mode.clone();
    let ground = move |_x: f64, _y: f64, top: f64| {
        let deck = m.load(Ordering::Relaxed) == 0;
        if deck && top >= 10.0 {
            crate::physics::GroundProbe { below: Some(10.0), above: None }
        } else if deck {
            crate::physics::GroundProbe { below: Some(0.0), above: Some(10.0) }
        } else {
            crate::physics::GroundProbe { below: Some(0.0), above: None }
        }
    };
    v.contact = Some(Arc::new(ground));
    v.position = DVec3::new(0.0, 0.0, 10.0);
    for _ in 0..50 {
        v.update_visuals(0.02);
    }
    let on_deck = v.trailers[0].position.z;
    assert!((on_deck - 10.0).abs() < 0.5, "rear section at {on_deck}");
    mode.store(1, Ordering::Relaxed);
    v.update_visuals(0.02);
    assert!(v.trailers[0].position.z > 9.0, "dropped through the gap to {}", v.trailers[0].position.z);
    // sunk under the deck: it comes back up
    mode.store(0, Ordering::Relaxed);
    v.trailers[0].position.z = 0.2;
    v.trailers[0].axle_z = Some(0.0);
    for _ in 0..5 {
        v.update_visuals(0.02);
    }
    assert!(v.trailers[0].position.z > 9.0, "stayed under the deck at {}", v.trailers[0].position.z);
}

/// #901: the rear section's wheels take the road under them - a kerb-high step under
/// its left wheel pushes that wheel up into its arch and leaves the right one.
#[test]
fn rear_section_wheels_spring_on_their_own() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_NL_NG/MAN_GN92_main.bus");
    let trail = root.join("Vehicles/MAN_NL_NG/MAN_GN92_trail.bus");
    if !bus.exists() || !trail.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("GN92"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    v.attach_trailer_ex(Arc::new(VehicleType::load(&root, &trail).expect("GN92 trail")), false);
    let t = &v.trailers[0];
    let axle = t.first_axle;
    assert!(t.ty.suspension_axles.contains(&axle), "the GN92's rear axle is drawn sprung");
    // a step 6 cm high under the left wheels behind the joint
    let ground = move |x: f64, y: f64, _top: f64| crate::physics::GroundProbe { below: Some(if x < 0.0 && y < -4.0 { 0.06 } else { 0.0 }), above: None };
    v.contact = Some(Arc::new(ground));
    v.position = DVec3::new(0.0, 0.0, 0.0);
    for _ in 0..50 {
        v.update_visuals(0.02);
    }
    let l = -v.var(&format!("Axle_Suspension_{axle}_L")).unwrap();
    let r = -v.var(&format!("Axle_Suspension_{axle}_R")).unwrap();
    assert!(l - r > 0.04, "left wheel up {l:.3}, right {r:.3}");
}

/// A timetable duty and a random traffic car load their bus with `VehicleType::load_ai`,
/// which lets the vertices of every mesh go to save memory - except a `[smoothskin]`
/// mesh (the bellows) has to keep its own, or there is nothing left to bend it from and
/// every AI articulated bus (and the rear-section trailer of a `.zug` train) drove with
/// its bellows frozen in the rest pose through every corner.
#[test]
fn ai_loaded_bellows_keep_their_skin_and_still_bend() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_NL_NG/MAN_GN92_main.bus");
    let trail = root.join("Vehicles/MAN_NL_NG/MAN_GN92_trail.bus");
    if !bus.exists() || !trail.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let ai = VehicleType::load_ai(&root, &bus).expect("GN92 (AI)");
    let skinned: Vec<usize> = (0..ai.meshes.len())
        .filter(|&i| !ai.meshes[i].skin.is_empty())
        .collect();
    assert!(
        !skinned.is_empty(),
        "the GN92 has no [smoothskin] mesh to check"
    );
    for &i in &skinned {
        assert!(
            !ai.meshes[i].data.positions.is_empty(),
            "{}: an AI copy dropped the bellows' vertices",
            ai.model.meshes[ai.meshes[i].def_index].file
        );
        assert!(
            !ai.mesh_dropped(i),
            "{}: an AI copy still marks the bellows as needing a re-read",
            ai.model.meshes[ai.meshes[i].def_index].file
        );
    }
    // every other mesh still lets its vertices go, exactly as before this fix
    assert!(
        (0..ai.meshes.len()).any(|i| ai.mesh_dropped(i)),
        "the AI type keeps meshes it should have let go"
    );

    let ty = Arc::new(ai);
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    v.attach_trailer_ex(
        Arc::new(VehicleType::load_ai(&root, &trail).expect("GN92 trail (AI)")),
        false,
    );
    v.heading = 10.0;
    v.update_visuals(0.02);
    v.heading = 30.0;
    v.update_visuals(0.02);
    let i = skinned[0];
    let rest = v.ty.meshes[i].data.positions.clone();
    let (pos, nrm) = v.skinned(i).expect("skinned vertices of an AI copy");
    assert_eq!((pos.len(), nrm.len()), (rest.len(), rest.len()));
    let moved = pos
        .iter()
        .zip(&rest)
        .map(|(p, r)| (*p - *r).length())
        .fold(0.0f32, f32::max);
    assert!(
        moved > 0.1,
        "the AI copy's bellows did not bend with the joint (moved {moved})"
    );
}

/// The same check on the O530G Facelift / Citaro LE mod pack (`OMSI_CONTENT`, or the
/// user's overlay by default): a third-party model with its own bone names
/// (`Armature_Gelenk_A-D` on `18m_main\bellows_out.o3d` / `bellows_in.o3d`, against the
/// stock GN92's `Gelenk_A-D` on its own files) still bends, both loaded normally and
/// loaded as an AI copy - the fix is generic over `[setbone]` names, not tied to MAN's.
#[test]
fn o530g_mod_bellows_bend_and_survive_an_ai_load() {
    let content = omsi_cfg::env::var_os("OMSI_CONTENT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(
                "/Users/savva/OMSI 2 Source Code/depot/target/release",
            )
        });
    let bus = content.join("Vehicles/MB_O530_Facelift/MB_O530GFL EL 3D Main.bus");
    let trail = content.join("Vehicles/MB_O530_Facelift/MB_O530GFL EL 3D Trail.bus");
    if !bus.exists() || !trail.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    // loaded fully (the player's own bus): bends like the GN92
    let ty = Arc::new(VehicleType::load(&content, &bus).expect("O530G"));
    let skinned = ty
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.skin.is_empty())
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    assert!(
        !skinned.is_empty(),
        "the O530G has no [smoothskin] mesh to check (bellows_out/bellows_in)"
    );
    let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
    v.attach_trailer_ex(
        Arc::new(VehicleType::load(&content, &trail).expect("O530G trail")),
        false,
    );
    v.heading = 10.0;
    v.update_visuals(0.02);
    v.heading = 30.0;
    v.update_visuals(0.02);
    for &i in &skinned {
        let rest = v.ty.meshes[i].data.positions.clone();
        let (pos, nrm) = v.skinned(i).expect("skinned vertices");
        assert_eq!((pos.len(), nrm.len()), (rest.len(), rest.len()));
        let moved = pos
            .iter()
            .zip(&rest)
            .map(|(p, r)| (*p - *r).length())
            .fold(0.0f32, f32::max);
        assert!(
            moved > 0.1,
            "{}: did not bend with the joint (moved {moved})",
            v.ty.model.meshes[v.ty.meshes[i].def_index].file
        );
    }
    // loaded as an AI copy (the timetable's or a random traffic bus): same bones kept
    let ai = VehicleType::load_ai(&content, &bus).expect("O530G (AI)");
    for &i in &skinned {
        assert!(
            !ai.meshes[i].data.positions.is_empty(),
            "an AI copy dropped the O530G's bellows vertices"
        );
    }
}

/// `OMSI_DEBUG_WHEELS` reports the wheels of an AI type, whose vertices were let go after
/// loading, exactly as it reports those of a fully loaded one.
#[test]
fn wheel_pivot_report_reads_dropped_meshes() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_F90/AI_MAN_F90_Wechselbruecke.bus");
    if !bus.exists() {
        eprintln!("skipped: no {}", bus.display());
        return;
    }
    let report = |ty: VehicleType| {
        VehicleInstance::new(Arc::new(ty), VehicleHost::new(crate::SimClock::default()))
            .wheel_pivot_report()
    };
    let ai = VehicleType::load_ai(&root, &bus).expect("F90");
    assert!(
        (0..ai.meshes.len()).any(|i| ai.mesh_dropped(i)),
        "the AI type keeps its vertices"
    );
    let full = report(VehicleType::load(&root, &bus).expect("F90"));
    assert!(!full.is_empty(), "the F90 has no animated wheels");
    assert_eq!(report(ai), full);
}
