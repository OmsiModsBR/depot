//! What the people aboard a vehicle weigh on it: each person's `[mass]` on the section of
//! the vehicle they are in, centred where they are.

use crate::physics::Payload;
use glam::Vec3;

/// What the people aboard a vehicle put on each of its sections, the front one first: their
/// mass (kg) and its centre in the section's own frame (m from the section's model origin).
/// `sections` are, for each section behind the first one, the y of the joint in front of it
/// and where its model origin lies (both in the frame of the vehicle unfolded straight, front
/// first, whose origin is the front section's); `aboard` each person's place in that frame and
/// their `.hum` `[mass]` (t). A person is in the section behind every joint they stand behind;
/// the front section takes everybody in front of the first joint.
pub fn payload_by_section(
    sections: &[(f32, Vec3)],
    aboard: impl IntoIterator<Item = (Vec3, f32)>,
) -> Vec<Payload> {
    // per section: the mass and its first moment about the section's origin
    let mut sums = vec![(0.0f32, Vec3::ZERO); sections.len() + 1];
    for (at, tonnes) in aboard {
        if !(at.is_finite() && tonnes.is_finite() && tonnes > 0.0) {
            continue;
        }
        let section = sections
            .iter()
            .take_while(|(joint, _)| at.y < *joint)
            .count();
        let origin = section.checked_sub(1).map_or(Vec3::ZERO, |s| sections[s].1);
        let kg = tonnes * 1000.0;
        sums[section].0 += kg;
        sums[section].1 += (at - origin) * kg;
    }
    sums.into_iter()
        .map(|(kg, moment)| Payload {
            kg,
            centre: if kg > 0.0 { moment / kg } else { Vec3::ZERO },
        })
        .collect()
}
