//! The closed forms of `docs/physics/` that the conformance tests compare the engine with.
//! Units are SI; `y` runs forward along the body.

/// The static share of a weight over springs (`K-1` and `D-1` of the model documents):
/// the sag `z` of the body and its pitch `theta` (nose down positive), so that the
/// compression at `y` is `z - theta * y`.
///
/// `axles` are `(y, k)` with `k` the spring rate of the whole axle in N/m, `weight` is `m g`
/// in N and `reference` the `y` the weight is shared about: `0` for Classic, `[cog]` y for
/// Depot. Valid while every spring is pressed and below its largest force.
pub fn static_share(axles: &[(f64, f64)], weight: f64, reference: f64) -> (f64, f64) {
    let s0: f64 = axles.iter().map(|a| a.1).sum();
    let s1: f64 = axles.iter().map(|a| a.1 * a.0).sum();
    let s2: f64 = axles.iter().map(|a| a.1 * a.0 * a.0).sum();
    let moment = weight * reference;
    let d = s0 * s2 - s1 * s1;
    (
        (weight * s2 - s1 * moment) / d,
        (s1 * weight - s0 * moment) / d,
    )
}

/// The compression of a spring at `y` for a share `(z, theta)`.
pub fn compression(share: (f64, f64), y: f64) -> f64 {
    share.0 - share.1 * y
}

/// What a section of a train must satisfy at rest. Its wheels carry `wheels` (N, summed) at
/// `wheels_moment` (N m about the model origin). The struts turn the body about `reference`
/// (`0` for Classic, `cog_y` for Depot, `K-1` and `D-1`); the joints' loads turn it about
/// its centre of gravity `cog_y`. `up_front` is the load its front joint carries up at
/// `front_y`, `up_back` the load it pushes up on the follower at `back_y` (so down on it).
///
/// Returns the residual force and the residual moment (both 0 at rest).
#[allow(clippy::too_many_arguments)]
pub fn section_residual(
    wheels: f64,
    wheels_moment: f64,
    weight: f64,
    reference: f64,
    cog_y: f64,
    up_front: f64,
    front_y: f64,
    up_back: f64,
    back_y: f64,
) -> (f64, f64) {
    let force = wheels + up_front - weight - up_back;
    let moment = wheels_moment - wheels * reference + up_front * (front_y - cog_y)
        - up_back * (back_y - cog_y);
    (force, moment)
}
