fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3<f32>(0.0031308));
}
fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    return select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
}
// A light's colour as the Enhanced passes add it (lighting/model.rs `linear_light_colour`):
// the content's sRGB numbers decoded, at the luminance they had.
fn light_colour(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    let lin = srgb_decode(x);
    let w = vec3<f32>(0.2126, 0.7152, 0.0722);
    let after = dot(lin, w);
    if (after <= 1e-6) {
        return lin;
    }
    return lin * (dot(x, w) / after);
}
