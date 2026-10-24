//! What the people aboard a vehicle weigh on it: each person's `[mass]` on the section of
//! the vehicle they are in.

/// The mass (kg) the people aboard a vehicle put on each of its sections, the front one
/// first: `joints` are the y of the joint in front of each section behind the first one (in
/// the frame of the vehicle unfolded straight, front first), `aboard` each person's y in that
/// frame and their `.hum` `[mass]` (t). A person is in the section behind every joint they
/// stand behind; the front section takes everybody in front of the first joint.
pub fn payload_by_section(
    joints: &[f32],
    aboard: impl IntoIterator<Item = (f32, f32)>,
) -> Vec<f32> {
    let mut kg = vec![0.0; joints.len() + 1];
    for (y, tonnes) in aboard {
        if !(y.is_finite() && tonnes.is_finite() && tonnes > 0.0) {
            continue;
        }
        let section = joints.iter().take_while(|&&joint| y < joint).count();
        kg[section] += tonnes * 1000.0;
    }
    kg
}
