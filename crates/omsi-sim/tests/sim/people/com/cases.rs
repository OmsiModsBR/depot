use crate::support::scratch::ScratchPack;
use omsi_sim::people::{BodyCom, HumanType};

/// The `[links]` of a stock-like man (right hip, knee, waist, shoulder, elbow, neck, hand,
/// finger), as OMSI's `.hum` files give them.
const LINKS: [f32; 22] = [
    0.09, 0.0, 0.92, 0.09, -0.03, 0.53, 0.02, 1.17, 0.18, -0.05, 1.43, 0.44, -0.04, 1.41, -0.02,
    1.55, 0.69, -0.03, 1.43, 0.9, -0.03, 1.43,
];

/// A man `height` m tall without a mesh, loaded as the game loads one, his `[links]` scaled
/// from the 1.77 m ones.
fn person(height: f32) -> HumanType {
    let pack = ScratchPack::new("body-com");
    let k = height / 1.77;
    let links: Vec<String> = LINKS.iter().map(|x| (x * k).to_string()).collect();
    pack.write("model.cfg", "");
    let hum = pack.write(
        "man.hum",
        format!(
            "[model]\nmodel.cfg\n\n[humangeom]\n0.04\n{height}\n\n[seatheight]\n{}\n\n[links]\n{}\n",
            0.83 * k,
            links.join("\n")
        ),
    );
    HumanType::load(&hum).unwrap()
}

/// Standing, the centre of mass lies over the feet a little above the hips: 0.55 to 0.58
/// of the body's height, as the anthropometric tables put it for an adult.
#[test]
fn standing_the_centre_of_mass_is_over_the_feet_at_over_half_the_height() {
    for height in [1.6, 1.77, 1.95] {
        let com = person(height).com.standing;
        let share = com.z / height;
        assert!(
            (0.55..=0.58).contains(&share),
            "{height} m: {com} ({share})"
        );
        assert!(
            com.x.abs() < 0.01 && com.y.abs() < 0.08,
            "{height} m: {com}"
        );
    }
}

/// Seated, the centre of mass lies ahead of the hips (the thighs reach forward) and over
/// them (the upper body weighs most): about a tenth of a metre ahead and over the seat, as
/// the seated body is put.
#[test]
fn seated_the_centre_of_mass_is_ahead_of_and_over_the_hips() {
    let man = person(1.77);
    let com = man.com.seated;
    // the hips: `seat_lift` over the seat, which is `[seatheight]` over the point
    let hips = man.def.seat_height + man.rig.seat_lift;
    assert!(com.x.abs() < 0.01, "{com}");
    assert!((0.05..0.2).contains(&com.y), "{com}");
    assert!((0.08..0.25).contains(&(com.z - hips)), "{com} {hips}");
}

/// A taller body has its centre of mass higher, in proportion.
#[test]
fn the_centre_of_mass_scales_with_the_body() {
    let (small, tall) = (person(1.6).com, person(1.95).com);
    let k = 1.95 / 1.6;
    assert!(
        (tall.standing.z / small.standing.z - k).abs() < 0.03,
        "{small:?} {tall:?}"
    );
}

/// Broken `[links]` (no numbers at all) give a centre of mass that is a number - the
/// body's as broken as it is drawn, not a repaired one -, never one that brings the game down.
#[test]
fn broken_links_give_a_centre_of_mass_that_is_a_number() {
    let pack = ScratchPack::new("body-com-broken");
    pack.write("model.cfg", "");
    let hum = pack.write("man.hum", "[model]\nmodel.cfg\n\n[links]\nx\nnan\n");
    let com: BodyCom = HumanType::load(&hum).unwrap().com;
    assert!(
        com.standing.is_finite() && com.seated.is_finite(),
        "{com:?}"
    );
}

/// A person's centre of mass turns with them: seated facing the bus's right (90°), what lies
/// ahead of their hips lies to the right of their place; standing it is over the place.
#[test]
fn the_centre_of_mass_turns_with_the_person() {
    let com = person(1.77).com;
    let place = glam::Vec3::new(1.0, 2.0, 0.4);
    let ahead = com.seated.y;
    let seated = com.at(place, 90.0, true);
    assert!((seated.x - (place.x + ahead)).abs() < 1e-4, "{seated}");
    assert!((seated.y - place.y).abs() < 1e-4, "{seated}");
    assert!(
        (seated.z - (place.z + com.seated.z)).abs() < 1e-4,
        "{seated}"
    );
    let facing_forward = com.at(place, 0.0, true);
    assert!(
        (facing_forward.y - (place.y + ahead)).abs() < 1e-4,
        "{facing_forward}"
    );
    let standing = com.at(place, 37.0, false);
    assert!(
        (standing.z - (place.z + com.standing.z)).abs() < 1e-4,
        "{standing}"
    );
}
