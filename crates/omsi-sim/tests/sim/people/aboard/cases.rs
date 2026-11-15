use crate::support::scratch::ScratchPack;
use glam::Vec3;
use omsi_content::Human;
use omsi_sim::physics::Payload;
use omsi_sim::testing::people::payload_by_section;

/// The mass of each section's people (kg).
fn masses(load: &[Payload]) -> Vec<f32> {
    load.iter().map(|p| p.kg).collect()
}

fn at(y: f32) -> Vec3 {
    Vec3::new(0.0, y, 1.0)
}

/// A rigid bus: everybody aboard weighs on its one section, their `[mass]` in tonnes, centred
/// where they are on average, weighted by their mass.
#[test]
fn everybody_aboard_a_rigid_bus_weighs_on_it() {
    let load = payload_by_section(
        &[],
        [
            (Vec3::new(-0.8, 4.0, 1.0), 0.075),
            (Vec3::new(0.8, -3.0, 1.4), 0.08),
            (Vec3::new(0.0, 0.0, 1.0), 0.045),
        ],
    );
    assert_eq!(load.len(), 1);
    assert!((load[0].kg - 200.0).abs() < 1e-3, "{load:?}");
    let want = Vec3::new(
        (-0.8 * 75.0 + 0.8 * 80.0) / 200.0,
        (4.0 * 75.0 - 3.0 * 80.0) / 200.0,
        (75.0 + 1.4 * 80.0 + 45.0) / 200.0,
    );
    assert!((load[0].centre - want).length() < 1e-4, "{load:?} {want}");
    // the spread about that centre: Σ m (r - centre)² along each axis
    let spread = [
        (Vec3::new(-0.8, 4.0, 1.0), 75.0),
        (Vec3::new(0.8, -3.0, 1.4), 80.0),
        (Vec3::new(0.0, 0.0, 1.0), 45.0),
    ]
    .iter()
    .fold(Vec3::ZERO, |s, (r, m)| s + (*r - want) * (*r - want) * *m);
    assert!(
        (load[0].spread - spread).abs().max_element() < 1e-2,
        "{load:?} {spread}"
    );
}

/// An articulated bus: each person weighs on the section they are in, the one behind every
/// joint they stand behind, centred in that section's own frame.
#[test]
fn each_person_weighs_on_the_section_they_are_in() {
    // joints at y -4.3 (front | middle) and -12 (middle | rear); the middle section's origin
    // lies 8 m behind the front one's, the rear one's 16 m
    let sections = [
        (-4.3, Vec3::new(0.0, -8.0, 0.0)),
        (-12.0, Vec3::new(0.0, -16.0, 0.0)),
    ];
    let load = payload_by_section(
        &sections,
        [
            (at(3.0), 0.08),   // front
            (at(-4.2), 0.07),  // front, just ahead of the first joint
            (at(-4.4), 0.06),  // middle, just behind it
            (at(-8.0), 0.075), // middle
            (at(-13.0), 0.09), // rear
        ],
    );
    assert_eq!(load.len(), 3);
    for (got, want) in masses(&load).iter().zip([150.0, 135.0, 90.0]) {
        assert!((got - want).abs() < 1e-3, "{load:?}");
    }
    let middle_y = (3.6 * 60.0 + 0.0 * 75.0) / 135.0;
    for (got, want) in load
        .iter()
        .zip([(3.0 * 80.0 - 4.2 * 70.0) / 150.0, middle_y, 3.0])
    {
        assert!((got.centre.y - want).abs() < 1e-4, "{load:?} {want}");
        assert!((got.centre.z - 1.0).abs() < 1e-4, "{load:?}");
    }
}

/// Nobody aboard weighs nothing, and a person without a mass or with a broken place adds
/// nothing either.
#[test]
fn nobody_aboard_weighs_nothing() {
    let none = Payload::default();
    let sections = [(-4.3, Vec3::new(0.0, -8.0, 0.0))];
    assert_eq!(payload_by_section(&sections, []), vec![none, none]);
    let load = payload_by_section(
        &sections,
        [
            (Vec3::new(0.0, f32::NAN, 1.0), 0.08),
            (Vec3::new(f32::INFINITY, 1.0, 1.0), 0.08),
            (at(1.0), f32::NAN),
            (at(1.0), 0.0),
            (at(1.0), -0.1),
        ],
    );
    assert_eq!(load, vec![none, none]);
}

/// A `.hum` gives its person's mass in tonnes; a file without `[mass]` weighs 1 t, as OMSI
/// reads a missing `[mass]`.
#[test]
fn a_person_weighs_their_files_mass_or_a_tonne_without_one() {
    let pack = ScratchPack::new("hum-mass");
    let with = pack.path().join("with.hum");
    let without = pack.path().join("without.hum");
    std::fs::write(&with, "[model]\nmodel.cfg\n\n[mass]\n0.075\n").unwrap();
    std::fs::write(&without, "[model]\nmodel.cfg\n").unwrap();
    assert_eq!(Human::load(&with).unwrap().mass, 0.075);
    assert_eq!(Human::load(&without).unwrap().mass, 1.0);
}
