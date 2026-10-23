// What several shaders share with the Rust side: the camera uniform (`CameraUniform` in
// scene/camera.rs, field for field; a test checks the offsets), the point light as the
// scene's light array holds it (`GpuPointLight`) and the light grid's cell size
// (`LIGHT_CELL_CAP`). Every shader that binds the camera includes the whole struct, so
// no field is read one vec4 too early.

struct Camera {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    world_origin: vec4<f32>, // world-space origin removed from render coordinates
    sun_dir: vec4<f32>,      // xyz direction to the sun, w intensity
    ambient: vec4<f32>,      // rgb ambient (envir light C)
    fog: vec4<f32>,          // rgb, density
    sun_color: vec4<f32>,    // rgb (envir light A), w night factor 0..1 (nightmap strength)
    sky_color: vec4<f32>,    // rgb secondary light from above (envir light B), w 1 = vanilla (as OMSI 2), 0 = Vanilla+
    light_grid: vec4<f32>,   // x,y grid origin (render-origin relative), z cell size, w cells per side
    sky: vec4<f32>,          // x sun azimuth (rad), y day weight, z twilight weight, w night weight
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    clouds: vec4<f32>,       // x density 0..1, yz texture offset (wind drift)
    light_view_proj: mat4x4<f32>,
    light_view_proj_far: mat4x4<f32>,
    shadow: vec4<f32>,       // x enabled, y texel size, z near cascade half range
    post: vec4<f32>,         // x enhanced, y time, zw sun ndc
    inside_a: vec4<f32>,     // player vehicle box: origin xyz, sin(heading)
    inside_b: vec4<f32>,     // cos(heading), half extents xyz
    inside_c: vec4<f32>,     // box centre offset xyz, w = 1 when there is a box
    flags: vec4<f32>,        // x detail texturing, y enhanced graphics, z never set (see fs_main's end), w close cascade half range
    light_view_proj_close: mat4x4<f32>,
    wind: vec4<f32>,         // the player's vehicle's velocity (m/s, world): the airstream on its glass
    // Enhanced: the street lamps' shadow maps (the tiles under the far map), and the
    // lights they belong to (-1: none)
    lamp_view_proj: array<mat4x4<f32>, 4>,
    lamp_shadow: vec4<f32>,
    // Windy trees: xy the weather's wind (m/s, world; 0 with the setting off), zw how far
    // the air has carried the gusts since the start (m, modulo PATTERN_PERIOD)
    tree_wind: vec4<f32>,
};

struct PointLight {
    pos: vec4<f32>,   // xyz relative to the render origin, w radius (0: an enhanced-only light)
    color: vec4<f32>, // rgb, w intensity
    dir: vec4<f32>,   // spot direction, w cosine of the outer cone (< -1.5: a point light)
    extra: vec4<f32>, // enhanced path: cosine of the inner cone, core radius, beam gain, radius
};

// per light grid cell CELL_CAP light indices, 0xffffffff = empty
const CELL_CAP: u32 = 32u;
