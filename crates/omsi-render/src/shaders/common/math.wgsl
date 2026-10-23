// Small maths several shaders share.

const PI: f32 = 3.14159265;

// Whether every component is a finite number, tested on the bits: Metal's fast maths takes
// `x != x` and comparisons with a NaN as it likes (a NaN colour happened to come out as
// something on the Mac), Direct3D 12 keeps the NaN and draws it black.
fn all_finite(v: vec3<f32>) -> bool {
    let e = bitcast<vec3<u32>>(v) & vec3<u32>(0x7f800000u);
    return all(e != vec3<u32>(0x7f800000u));
}

// A colour read back from a target as black when it is not finite, else kept between 0
// and `hi`.
fn finite_clamped(c: vec3<f32>, hi: f32) -> vec3<f32> {
    return select(vec3<f32>(0.0), clamp(c, vec3<f32>(0.0), vec3<f32>(hi)), all_finite(c));
}
