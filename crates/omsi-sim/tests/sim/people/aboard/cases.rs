use crate::support::scratch::ScratchPack;
use omsi_content::Human;
use omsi_sim::testing::people::payload_by_section;

/// A rigid bus: everybody aboard weighs on its one section, their `[mass]` in tonnes.
#[test]
fn everybody_aboard_a_rigid_bus_weighs_on_it() {
    let kg = payload_by_section(&[], [(4.0, 0.075), (-3.0, 0.08), (0.0, 0.045)]);
    assert_eq!(kg.len(), 1);
    assert!((kg[0] - 200.0).abs() < 1e-3, "{kg:?}");
}

/// An articulated bus: each person weighs on the section they are in, the one behind every
/// joint they stand behind.
#[test]
fn each_person_weighs_on_the_section_they_are_in() {
    // joints at y -4.3 (front | middle) and -12 (middle | rear)
    let joints = [-4.3, -12.0];
    let kg = payload_by_section(
        &joints,
        [
            (3.0, 0.08),   // front
            (-4.2, 0.07),  // front, just ahead of the first joint
            (-4.4, 0.06),  // middle, just behind it
            (-8.0, 0.075), // middle
            (-13.0, 0.09), // rear
        ],
    );
    assert_eq!(kg.len(), 3);
    for (got, want) in kg.iter().zip([150.0, 135.0, 90.0]) {
        assert!((got - want).abs() < 1e-3, "{kg:?}");
    }
}

/// Nobody aboard weighs nothing, and a person without a mass or with a broken place adds
/// nothing either.
#[test]
fn nobody_aboard_weighs_nothing() {
    assert_eq!(payload_by_section(&[-4.3], []), vec![0.0, 0.0]);
    let kg = payload_by_section(
        &[-4.3],
        [(f32::NAN, 0.08), (1.0, f32::NAN), (1.0, 0.0), (1.0, -0.1)],
    );
    assert_eq!(kg, vec![0.0, 0.0]);
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
