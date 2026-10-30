//! The user's settings: graphics and gameplay switches, kept as `key=value` lines in
//! `~/.depot/settings.cfg` (the launcher writes the same file). Anything missing
//! keeps its default, so an old file never breaks a new build.

use std::path::PathBuf;

/// Where the game keeps its own files (`~/.depot`): settings beside the content, the
/// seats, the missing content list, the LAN status files.
pub(crate) fn data_dir() -> Option<PathBuf> {
    omsi_cfg::paths::data_dir()
}

/// Version of the settings file (`version=`); files without it are version 1.
pub const SETTINGS_VERSION: u32 = 3;

/// The items of the information bar the `custom` layout can switch one by one (settings
/// keys `info_item_<name>`).
pub const INFO_ITEM_KEYS: [&str; 9] = [
    "info_item_time",
    "info_item_speed",
    "info_item_trip",
    "info_item_odometer",
    "info_item_temps",
    "info_item_tank",
    "info_item_pax",
    "info_item_line",
    "info_item_next",
];

/// Where `key` (`info_item_<name>`) stands among the items of the information bar.
pub fn info_item_index(key: &str) -> Option<usize> {
    INFO_ITEM_KEYS.iter().position(|k| *k == key)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Samples per pixel: 1, 2, 4 or 8.
    pub msaa: u32,
    /// Anisotropic filtering 1..16.
    pub anisotropy: u16,
    pub ssao: bool,
    pub shadows: bool,
    pub shadow_size: u32,
    /// Draw the models' `[isshadow]` shadow meshes: OMSI's flat blob under a vehicle,
    /// standing in for the sky light the body keeps off the road. Off, only the sun shadow
    /// map shades under a vehicle (the blob is a fake OMSI draws whatever the depth).
    pub shadow_blobs: bool,
    /// The route navigator in the lower right corner.
    pub navigator: bool,
    /// How much of the interface's backgrounds shows - the navigator's, the menu's, the
    /// timetable's, the notes' plates - 0.2 .. 1, the texts staying solid: 0.85 (the default)
    /// as designed (`ui::backdrop`). `navigator_opacity` in older files: it was the
    /// navigator's alone.
    pub ui_opacity: f32,
    /// Which corner the navigator sits in: `bottom-left` (default), `bottom-right`,
    /// `top-left` or `top-right`.
    pub navigator_corner: String,
    /// How passengers board: `auto` - they pay at the cash desk and take the ticket
    /// themselves; `pay` - they wait at the desk for the driver to sell the ticket (the
    /// ticket key or the printer); `walk` - they just walk into the saloon (a flat-fare
    /// or ticket-machine service).
    pub boarding: String,
    /// Procedural detail (fractal) texturing of the ground and large walls when close.
    pub detail_textures: bool,
    /// Passengers pay the exact fare (no change to give at the cash desk).
    pub exact_fare: bool,
    /// Enhanced graphics: the physically based renderer (its own lighting, sky, exposure).
    pub enhanced: bool,
    /// The graphics: `vanilla` (as OMSI 2 draws it: no sun shadows, no ambient occlusion,
    /// no detail grain, no snow cover or rain drops of our own), `vanilla_plus` (the same
    /// renderer with those extras, the default) or `enhanced` (`enhanced` follows it).
    pub graphics: String,
    /// Start a PC OpenXR headset session when the game starts (Windows only).
    pub vr: bool,
    /// Fraction of the OpenXR runtime's recommended eye resolution.
    pub vr_scale: f32,
    /// Optional VR head pose smoothing time in milliseconds; zero uses raw tracking.
    pub vr_head_smoothing_ms: f32,
    /// Total bus mirror redraws per second in VR; zero freezes them,
    /// and -1 redraws every mirror once per game frame.
    pub vr_mirror_rate: f32,
    /// Copy the left eye to the desktop while VR is active.
    pub vr_desktop_mirror: bool,
    pub triple: omsi_render::TripleScreen,
    pub triple_span: bool,
    pub triple_hud_center: bool,
    pub fullscreen: bool,
    pub vsync: bool,
    /// Master volume 0..1.
    pub volume: f32,
    /// Control preset: "simple", "wasd", "arrows" or "omsi".
    pub drive_keys: String,
    /// Anti-aliasing of the enhanced picture after tone mapping: `fxaa` (default) or `off`.
    pub post_aa: String,
    /// OMSI's maintenance condition (`[wear_lifespan]`): 0 infinite (no wear), 1 very bad,
    /// 2 bad, 3 normal, 4 good - the player's bus's `wearlifespan` 1.5e6, 0.01, 0.1, 1, 10
    ///; AI vehicles never wear.
    pub maintenance: u8,
    /// `[AIUnschedFactor]`: the share of the random traffic (percent of the map's density).
    pub ai_unsched_factor: f32,
    /// `[AIMaxCountScheduled]`: timetable vehicles on the road at once, 0 to 1000 (0 = none).
    pub ai_max_scheduled: u32,
    /// `[AIMaxCountRandom]`'s first line: vehicles without a timetable (cars) on the road at
    /// once, 0 to 1000.
    pub ai_max_random: u32,
    /// `[AIPriorityScheduled]`: the least important timetable line (1 to 4) that still runs:
    /// 1 only the most important lines, 4 all of them.
    pub ai_priority_scheduled: u8,
    /// `[AIMaxCountParked]`: the share of the parking spaces with a car, 0 to 100 percent.
    pub ai_max_parked: i32,
    /// `[AIMaxCountRandom]`'s second line: OMSI makes this many people at the start and
    /// no more - everybody waiting, walking and riding comes out
    /// of them, and at most half walk the pavements.
    pub ai_max_humans: u32,
    /// `[no_collision_vehToVeh]` off: the player's bus collides with the traffic.
    pub collision_vehicles: bool,
    /// `[no_collision]` off: the player's bus collides with the map's solid objects.
    pub collision_objects: bool,
    /// `[no_collision_pedastrians]` off: people are knocked down.
    pub collision_pedestrians: bool,
    /// `[driverview_moving]`: the driver's head moves with the bus (braking, bends, bumps).
    pub head_movement: bool,
    /// The interior camera glides between viewpoints (OMSI's `[driverview_smooth]`).
    pub driverview_smooth: bool,
    /// The driver's hands on the wheel in the cab view (the rest of the figure left out).
    pub hands_in_cab: bool,
    /// OMSI's `[altView]` (options.cfg): the right mouse button turns the view and
    /// Shift+right zooms. Off, the right button zooms and the middle one turns the view
    /// - OMSI's default, but on here by default: a
    /// laptop's touchpad has no middle button to look round with, and right-drag looking
    /// is what openOMSI always did.
    pub alt_view: bool,
    /// Precision zoom: the both-drag zoom follows the FOV-multiplier curve
    /// instead of OMSI's linear way over 500 px. Off (OMSI's way).
    pub precision_zoom: bool,
    /// The 3D picture drawn at this fraction of the window's size and scaled up (0.5..1),
    /// 0 = automatic (full size unless the window has more pixels than a 2560x1080 screen,
    /// as a Retina window does). The HUD is always drawn at full size.
    pub render_scale: f32,
    /// Language of the texts the game shows about the cockpit: `ENG` (default), `DEU` or
    /// `FRA` - OMSI's own language file codes.
    pub language: String,
    /// What passengers say: `all`, `tickets` (only what they ask for) or `off`.
    pub pax_voices: String,
    /// OMSI 2's route arrows over the road (as well as or instead of the navigator).
    pub nav_arrows: bool,
    /// The other (AI) vehicles as dots on the navigator's and the city map.
    pub nav_ai: bool,
    /// The navigator and the city map follow the walker (a walker icon) and the free camera (F4: a camera icon) in place of the bus; off, the bus.
    pub nav_follow_view: bool,
    /// The map's own surface (water, green, rivers, names) under the roads of the navigator and the city map, when the map ships one.
    pub nav_surface: bool,
    /// The light look of the map (light ground, white roads) instead of the dark one.
    pub nav_light: bool,
    /// The terrain's contour lines on the city map, when the map's package carries them.
    pub nav_contours: bool,
    /// How the city map is turned: "map" (as the map is drawn, openOMSI's way), "north" (real north up, where the map
    /// says where it is) or "heading" (the way the vehicle faces up).
    pub nav_orientation: String,
    /// How the minimap looks at the map: `perspective` (tilted ahead of the vehicle) or `top`.
    pub nav_view: String,
    /// The driver may get up from the seat and walk about (Ctrl+Shift+G).
    pub get_up: bool,
    /// Uncompressed texture files are compressed on loading where that leaves the picture
    /// close (DXT files always stay compressed on a GPU that takes them).
    pub texture_compression: bool,
    /// Texture memory the scenery may take (MB) before far textures lose their finest mip
    /// levels, like OMSI's `[texmemlimit]`; 0 = automatic (a share of the machine's memory).
    pub texture_memory: u32,
    /// OMSI's automatic clutch (`AutoClutch`, on unless `[no_automaticClutch]`): the
    /// manual-gearbox scripts work the clutch themselves while it is on.
    pub auto_clutch: bool,
    /// Manual gate buttons return the gearbox to neutral when released.
    pub momentary_gears: bool,
    /// The automated manual: a gear lever's gates are worked by the engine speed (#713).
    pub auto_shift: bool,
    /// The original's `performance_minObjSize`: objects smaller on the screen than this are
    /// not drawn (its presets say 0.013; 0.020 for slow machines, smaller keeps more).
    pub min_obj_size: f32,
    /// The original's `performance_maxObjDist` (m): objects farther away are not drawn
    /// (0 = no limit). `auto` (-1) takes `view_distance` when the file sets one, else 900 m
    /// (the original's high presets).
    pub max_obj_dist: f32,
    /// Frames a second at most (the original's `[maxFPS]`); 0 = no limit. The frame waits
    /// asleep, so a limit also saves power and heat, and the CPU time for the rest.
    pub max_fps: u32,
    /// The chat of a LAN session (V shows and hides it, / types); off leaves it out altogether.
    pub chat: bool,
    /// The chat's own size on top of the interface's (`ui_scale`): 0.5 .. 3, also set with
    /// Ctrl + the mouse wheel over the chat.
    pub chat_size: f32,
    /// The name of what the cursor points at, shown next to the cursor.
    pub tooltips: bool,
    /// The other players' names above their buses.
    pub name_tags: bool,
    /// The driver sits in the player's bus, turning the wheel, seen from outside and the
    /// passengers' seats and in the mirrors (`driver`), never in the driver's own view.
    pub driver: bool,
    /// The frame rate in the HUD.
    pub show_fps: bool,
    /// The notes in the top left corner: why the bus does not move, the change due, what a
    /// service did (off, none are shown).
    pub notes: bool,
    /// The information bar along the top (Shift+Y, the menu, the touch panel's button): as
    /// it was left, not switched on again every session (#1164).
    pub info_bar: bool,
    /// What the information bar shows: `full` (all of it), `compact` (time, speed, line, next
    /// stop) or `essential` (speed, limit, delay).
    pub info_bar_mode: String,
    /// Where it sits: `top` or `bottom`.
    pub info_bar_pos: String,
    /// The frame rate (and its 1 % low) as the bar's last part, not only in the corner.
    pub info_bar_fps: bool,
    /// Which items the `custom` layout shows, in the order of [`INFO_ITEM_KEYS`].
    pub info_items: [bool; 9],
    /// Minutes of play between the automatic saves of the last situation (`laststn.osn`); 0 switches them off.
    pub autosave_minutes: u32,
    /// The interface's accent colour (`omsi_ui::theme::Accent`, by name), in the launcher and
    /// every menu of the game.
    pub ui_accent: String,
    /// The launcher's corners, "rounded" or "square" (the game's menus keep theirs).
    pub ui_corners: String,
    /// The size of the game's interface over the picture - texts, the menu, the timetable,
    /// the navigator - on top of the screen's own scale and the window's height (see
    /// `ui::size_factor`): 0.5 .. 2 (1 = as designed).
    pub ui_scale: f32,
    /// The interface grows with a window taller than 1080 lines (`ui::size_factor`); off,
    /// it keeps its size on any window, as before.
    pub ui_scale_window: bool,
    /// Clouds in the sky (volumetric with enhanced graphics, OMSI's cloud layer without).
    pub clouds: bool,
    /// Windy trees: the trees' foliage bends and sways in the weather's wind and its gusts
    /// (above the trunk only; with no wind nothing moves).
    pub windy_trees: bool,
    /// "DirectX9 (D3D9Ex)" only: falling snow, drawn the way OMSI 2 draws it (camera-facing
    /// flakes of its snowflake picture). Off by default: the DirectX9 picture has no falling
    /// snow without it.
    pub dx9_snow: bool,
    /// How many people wait and ride, against the map's own numbers (OMSI's `AIPassFactor`,
    /// 1 = 100 %).
    pub pax_density: f32,
    /// Volume of the AI vehicles and of the scenery's sounds (OMSI's `sound_ai`,
    /// `sound_scenery`), 0..1.
    pub vol_ai: f32,
    pub vol_scenery: f32,
    /// Edge of the mirrors' pictures in pixels (OMSI's `performance_reflTexSize`, 2^n); 0 disables mirror rendering.
    pub mirror_size: u32,
    /// How often the mirrors are drawn (OMSI's `performance_realreflexions`): `off` once
    /// for each bus and then still, `eco` 15 times a second at most, `full` 30 (the default).
    pub mirror_refresh: String,
    /// Copies of the main side mirrors in the corners of the window while in the cab: 0 off, 1 the right one, 2 the left, 3 both.
    pub mirror_hud: u8,
    /// OMSI's `sound_doppler`: approaching sounds higher, receding ones lower.
    pub doppler: bool,
    /// Outside sounds heard from a cab (other vehicles', the bus's own exterior ones) go
    /// through a low-pass, as the game did before it mixed as OMSI 2 does. Off by default:
    /// OMSI 2 has no such filter.
    pub muffle_outside: bool,
    /// How fast the clock runs (1 real time .. 30); in LAN play the host's decides.
    pub time_speed: f64,
    /// The game's date and time follow this device's clock (the host's in LAN play); the
    /// time cannot be changed while it is on.
    pub time_sync: bool,
    /// The weather follows the METAR report of the airport nearest the map (refreshed every
    /// few minutes); the weather cannot be changed while it is on.
    pub metar_sync: bool,
    /// The airport (ICAO) whose report the METAR sync follows; empty: the one nearest the map.
    pub metar_station: String,
    /// Which meshes cast sun shadows: "all" solid ones, or "omsi" - only those the models
    /// mark `[shadow]`, as OMSI 2's shadows do.
    pub shadow_casters: String,
    /// Dead zone round the centre of a set-up game controller's axes (0..0.3).
    pub ctrl_deadzone: f32,
    /// Use a gamepad's right stick for automatic camera movement.
    pub right_stick_look: bool,
    /// How long (ms) a gamepad stick's steering takes to settle where the stick points
    /// (0..300; 0 = the stick as it reads).
    pub pad_steer_smooth: f32,
    /// A gamepad stick steers as a wheel's axis: the wheel where the stick points, as far
    /// at any speed - for the cheap wheels the system calls an Xbox controller (#1653).
    pub pad_steer_linear: bool,
    /// With a wheel steering, Left/Right switch the interior cameras as without one, instead
    /// of turning the head (OMSI's glance), #1345.
    pub arrows_switch_cams: bool,
    /// Game controllers switched off, by name (`|` between them).
    pub ctrl_off: String,
    /// Keyboard steering at OMSI's steady pace (`KeyboardAxes::linear`).
    pub steering_linear: bool,
    /// The wheel stays where the keys left it (`KeyboardAxes::old_steering`).
    pub old_steering: bool,
    /// OMSI's `[redSteerSpd]`: the steering keys slower at speed (`KeyboardAxes::red_steer_spd`).
    pub red_steer_spd: bool,
    /// The materials' reflection maps (`RenderOptions::reflections`).
    pub reflections: bool,
    /// How bright an LED panel's dots burn (`Lighting::led_glow`): 0 (off) .. 15, 16 levels.
    pub led_glow: u8,
    /// How much of the mip chain an LED panel is held at (`Lighting::led_mips`): its own
    /// picture and its `\S:n` mask are sampled at the level their screen footprint asks
    /// for, never coarser than this. 0 point-samples them (the sharpest dots, and the
    /// worst shimmer); 1.3 (the default) leaves a matrix's dots a couple of pixels across
    /// when the panel is small; 4 is near the calm of the full chain.
    pub led_mips: f32,
    /// Enhanced graphics: how much brighter the night is shown, in exposure steps (0 .. 3).
    pub night_brightness: f32,
    /// Mouse steering: how far the wheel turns for the same hand movement (1 = OMSI's: the
    /// window's width is the full lock).
    pub mouse_sens: f32,
    /// The graphics interface: `auto` (Vulkan, else DirectX 12, else OpenGL), `vulkan`,
    /// `dx12` or `gl` (see `startup::graphics_instance`).
    pub graphics_api: String,
    /// Default motor polarity for wheels without a saved per-device direction.
    pub ff_invert: bool,
    /// Force feedback and rumble at all (off: the controller neither pushes nor shakes).
    pub ff_enabled: bool,
    /// The constant trembling of the tarmac under the wheels (0 = none .. 4 = strong).
    pub ff_road_vib: f32,
    /// The engine's buzz in the wheel, whatever gear the bus is in (0 = none .. 4 =
    /// strong).
    pub ff_engine_vib: f32,
    /// How long (s) a jolt's or a script's vibration eases away once it stops (0: it stops
    /// where it stands; 1.5 = a long easing-off).
    pub ff_fade: f32,
    /// OMSI's held brake on the keyboard (see `KeyboardAxes::pedal_hold`); `brake_hold` in
    /// the file - the old `pedal_hold` (off unless set, and holding the throttle as well)
    /// is left behind.
    pub brake_hold: bool,
    /// Mouse steering as it was left last time (O or the menu switches it; the game
    /// starts with it as the player had it).
    pub mouse_steering: bool,
    /// A right click ends the mouse steering, as in OMSI (the right button
    /// held while the mouse steers fires `toggel_mouse_ctrl`). Off by default: looking
    /// round with the right button dragged switched it off every time.
    pub mouse_right_off: bool,
    /// The mouse steering's wheel and pedals ease after the cursor (~60 ms); off, they follow
    /// it at once, as in OMSI (#1092).
    pub mouse_smooth: bool,
    /// How fast the view turns for the mouse's way while looking round (1 = OMSI's
    /// field of view / 78.75 degrees per pixel in the views of the bus; #859).
    pub look_sens: f32,
    /// How long the view takes to come round to where the mouse (or the stick, or the
    /// arrow keys) turned it (milliseconds; 0 = off, the view goes there at once, as it
    /// always did). A little of this takes the edge off a mouse that reports in jumps and
    /// off a hand that shakes: the view glides to the angle instead of jumping to it.
    pub look_smoothing_ms: f32,
    /// The bus's script may cancel the indicator after a turn (#451: off, an indicator set
    /// stays on until the player turns it off).
    pub blinker_cancel: bool,
    /// The steering wheel's own rotation, lock to lock (degrees; a G29 turns 900).
    pub wheel_range: f32,
    /// How far the wheel is turned, lock to lock, for the bus's full lock (degrees); 0 = the
    /// whole of the wheel's rotation, as OMSI.
    pub wheel_lock: f32,
    /// Field of view of the views from the bus (degrees; 0 = the bus's own cameras).
    pub fov: f32,
    /// Vertical field of view of the outside view, F3 (degrees; 0 = automatic, from the shape of
    /// the window: 60, held to 16:9's width on wider screens).
    pub outside_fov: f32,
    /// The outside camera is pulled in in front of what stands between it and the bus
    /// (off: it goes through everything, as in OMSI).
    pub camera_collision: bool,
    /// The driver's view turns a little into the steering (off: it stays fixed to the bus,
    /// as in OMSI, which has no such thing).
    pub steer_look: bool,
    /// Maximum automatic driver's-view yaw at full steering lock (degrees).
    pub steer_look_angle: f32,
    /// Steering-view smoothing time constant (seconds).
    pub steer_look_response: f32,
    /// How much the view sways on its own when nothing is done to it: a head at rest breathes
    /// and shifts its weight, and most of that is seen while the bus waits at a stop. 0 is not
    /// at all (as OMSI, and the default); 1 is the whole of the sway `head_idle` makes.
    pub head_idle: f32,
    /// How fast that sway moves, against the pace it is designed at (1: as designed; below 1
    /// slower, above it quicker).
    pub head_idle_pace: f32,
    /// How strongly the analog throttle and brake pedals act: the response curve's
    /// strength (1 = linear, below 1 softer at the start, above 1 stronger).
    pub pedal_throttle: f32,
    pub pedal_brake: f32,
    /// The driver's eye moved from the bus's own camera (m, bus frame: right, forward, up).
    pub seat: [f32; 3],
    /// The driver's default head pitch (degrees; positive looks up).
    pub seat_pitch_deg: f32,
    /// Head tracking through opentrack's UDP output (TrackIR, webcams, phones), and its port.
    pub head_tracking: bool,
    pub head_tracking_port: u16,
    /// Where the head-tracking UDP port listens (`head_tracking_bind`, written by hand): empty
    /// is this computer only (127.0.0.1); an address, or `any` for every interface (opentrack
    /// on another machine). See `headtrack::bind_addr`.
    pub head_tracking_bind: String,
    /// Axes of the tracker turned the other way (`yaw,pitch,roll`): trackers disagree.
    pub head_tracking_invert: String,
    /// Head-tracking rotation sensitivity for yaw, pitch and roll.
    pub head_tracking_yaw_sens: f32,
    pub head_tracking_pitch_sens: f32,
    pub head_tracking_roll_sens: f32,
    /// Head-tracking translation sensitivity for X/Y/Z (percentage, 100 = 1:1).
    pub head_tracking_x_sens: f32,
    pub head_tracking_y_sens: f32,
    pub head_tracking_z_sens: f32,
    /// Explicit per-axis inversion for native TrackIR/head tracking rotation.
    pub head_tracking_invert_yaw: bool,
    pub head_tracking_invert_pitch: bool,
    pub head_tracking_invert_roll: bool,
    /// Explicit per-axis inversion for native TrackIR/head tracking translation.
    pub head_tracking_invert_x: bool,
    pub head_tracking_invert_y: bool,
    pub head_tracking_invert_z: bool,
    /// Positional voice in multiplayer through GreenTeaSpeak's openOMSI plugin (`voice`).
    pub voice_chat: bool,
}

/// A pedal's last few per cent of travel are its end: a wheel's pedal on the floor reads
/// 0.93..0.99, and the scripts ask for the ends exactly - the LiAZ/PAZ gearboxes put a gear
/// in only at `(L.L.clutch) 1 =` and part the engine from the wheels only above 0.95, so a
/// clutch held down to the floor still dragged and the engine died at every stop.
pub fn pedal_ends(v: f32) -> f32 {
    if v >= 0.96 {
        1.0
    } else if v <= 0.02 {
        0.0
    } else {
        v
    }
}

/// A pedal as the settings shape it: `v` 0..1 through the response curve of `strength`.
pub fn pedal_curve(v: f32, strength: f32) -> f32 {
    let g = strength.clamp(0.25, 4.0);
    v.clamp(0.0, 1.0).powf(1.0 / g)
}

impl Default for Settings {
    fn default() -> Self {
        if crate::platform::MOBILE {
            // a phone's graphics chip and battery: 2x MSAA (cheap on a tiled GPU), no
            // ambient occlusion, a smaller shadow map and mirrors, a shorter view
            return Self {
                msaa: 2,
                anisotropy: 4,
                ssao: false,
                shadow_size: 1024,
                mirror_size: 128,
                max_fps: 60,
                max_obj_dist: 900.0,
                pax_density: 0.7,
                navigator_corner: "top-center".into(),
                ..Self::desktop()
            };
        }
        Self::desktop()
    }
}

impl Settings {
    /// HUD coordinates occupy the centre panel by default; world picking always
    /// keeps the full window's physical coordinates.
    pub fn hud_viewport(&self, size: (u32, u32)) -> [f32; 4] {
        if self.triple.enabled && self.triple_hud_center && !self.vr_requested() && size.0 >= 3 {
            // (the centre panel, as the renderer draws it)
            let w = omsi_render::panel_width(size.0);
            [w as f32, 0.0, w as f32, size.1 as f32]
        } else {
            [0.0, 0.0, size.0 as f32, size.1 as f32]
        }
    }
    /// The defaults of a computer.
    fn desktop() -> Self {
        Self {
            msaa: 4,
            anisotropy: 8,
            ssao: true,
            shadows: true,
            shadow_size: 2048,
            shadow_blobs: true,
            navigator: true,
            ui_opacity: 0.85,
            notes: true,
            ui_scale: 1.0,
            ui_scale_window: true,
            navigator_corner: "bottom-left".into(),
            boarding: "auto".into(),
            detail_textures: true,
            exact_fare: true,
            enhanced: false,
            graphics: "vanilla_plus".into(),
            triple: Default::default(),
            triple_span: true,
            triple_hud_center: true,
            vr: false,
            vr_scale: 0.65,
            vr_head_smoothing_ms: 0.0,
            vr_mirror_rate: 16.0,
            vr_desktop_mirror: true,
            fullscreen: false,
            vsync: true,
            volume: 0.6,
            drive_keys: "simple".into(),
            post_aa: "fxaa".into(),
            render_scale: 0.0,
            language: "ENG".into(),
            pax_voices: "all".into(),
            nav_arrows: false,
            nav_ai: true,
            nav_follow_view: false,
            nav_surface: true,
            nav_light: false,
            nav_contours: false,
            nav_orientation: "map".into(),
            nav_view: "perspective".into(),
            get_up: false,
            texture_compression: true,
            texture_memory: 0,
            auto_clutch: true,
            momentary_gears: false,
            auto_shift: false,
            min_obj_size: 0.013,
            max_obj_dist: -1.0,
            max_fps: 0,
            chat: true,
            chat_size: 1.0,
            tooltips: true,
            name_tags: true,
            show_fps: false,
            clouds: true,
            windy_trees: true,
            dx9_snow: false,
            pax_density: 1.0,
            vol_ai: 1.0,
            vol_scenery: 1.0,
            mirror_size: 256,
            mirror_hud: 0,
            mirror_refresh: "full".into(),
            doppler: true,
            muffle_outside: false,
            driver: true,
            maintenance: 0,
            ai_unsched_factor: 1.0,
            ai_max_scheduled: 1000,
            ai_max_random: 1000,
            ai_priority_scheduled: 4,
            ai_max_parked: 100,
            ai_max_humans: 200,
            collision_vehicles: true,
            collision_objects: true,
            collision_pedestrians: true,
            head_movement: true,
            driverview_smooth: true,
            hands_in_cab: false,
            alt_view: true,
            precision_zoom: false,
            time_speed: 1.0,
            time_sync: false,
            metar_sync: false,
            metar_station: String::new(),
            shadow_casters: "all".into(),
            ctrl_deadzone: 0.0,
            right_stick_look: true,
            pad_steer_smooth: 120.0,
            pad_steer_linear: false,
            arrows_switch_cams: false,
            ctrl_off: String::new(),
            steering_linear: true,
            old_steering: false,
            red_steer_spd: false,
            reflections: true,
            led_glow: 6,
            led_mips: 1.3,
            night_brightness: 0.0,
            mouse_sens: 1.0,
            graphics_api: "auto".into(),
            ff_invert: false,
            ff_enabled: true,
            ff_road_vib: 1.0,
            ff_engine_vib: 1.0,
            ff_fade: 0.28,
            brake_hold: true,
            mouse_steering: false,
            mouse_right_off: false,
            mouse_smooth: true,
            look_sens: 1.0,
            look_smoothing_ms: 0.0,
            blinker_cancel: true,
            wheel_range: 900.0,
            wheel_lock: 0.0,
            fov: 0.0,
            outside_fov: 0.0,
            camera_collision: true,
            steer_look: false,
            steer_look_angle: 30.0,
            steer_look_response: 0.25,
            head_idle: 0.0,
            head_idle_pace: 1.0,
            pedal_throttle: 1.0,
            pedal_brake: 1.0,
            seat: [0.0; 3],
            seat_pitch_deg: 0.0,
            head_tracking: false,
            head_tracking_port: 4242,
            head_tracking_bind: String::new(),
            head_tracking_invert: String::new(),
            head_tracking_yaw_sens: 100.0,
            head_tracking_pitch_sens: 100.0,
            head_tracking_roll_sens: 100.0,
            head_tracking_x_sens: 100.0,
            head_tracking_y_sens: 100.0,
            head_tracking_z_sens: 100.0,
            head_tracking_invert_yaw: false,
            head_tracking_invert_pitch: false,
            head_tracking_invert_roll: false,
            head_tracking_invert_x: false,
            head_tracking_invert_y: false,
            head_tracking_invert_z: false,
            voice_chat: true,
            info_bar: false,
            info_bar_mode: "full".into(),
            info_bar_pos: "top".into(),
            info_bar_fps: false,
            info_items: [true; 9],
            autosave_minutes: 5,
            ui_accent: "Amber".into(),
            ui_corners: "rounded".into(),
        }
    }
}

impl Settings {
    /// The launcher setting, with the old environment switch kept for existing VR runs.
    pub fn vr_requested(&self) -> bool {
        cfg!(windows) && (self.vr || omsi_cfg::flags::OPENXR.is_set())
    }

    /// `~/.depot/settings.cfg` (or `%USERPROFILE%` on Windows).
    pub fn path() -> Option<PathBuf> {
        omsi_cfg::paths::settings()
    }

    /// The window's size in pixels the settings ask for (`resolution=WxH`, #904); None:
    /// automatic (fitted to the screen, the whole of it under gamescope).
    pub fn resolution() -> Option<(u32, u32)> {
        let text = std::fs::read_to_string(Self::path()?).ok()?;
        resolution_of(&text)
    }

    /// The accent the settings choose, for every menu drawn from now on; the game's menus keep
    /// their rounded corners (on Android the launcher, square or not, ran in this same process).
    pub fn apply_accent(&self) {
        omsi_ui::theme::set_accent(
            omsi_ui::theme::Accent::named(&self.ui_accent).unwrap_or_default(),
        );
        omsi_ui::theme::set_square(false);
    }

    pub fn load() -> Settings {
        let Some(p) = Self::path() else {
            return Settings::default();
        };
        let mut text = std::fs::read_to_string(&p).unwrap_or_default();
        // OMSI_GRAPHICS=vanilla|vanilla_plus|enhanced: another renderer for one run
        if let Some(g) = omsi_cfg::flags::GRAPHICS.text() {
            text.push_str(&format!("\ngraphics={g}\n"));
        }
        let mut s = Self::from_text(&text);
        // "DirectX9 (D3D9Ex)" draws the picture as OMSI 2 does and no more: Vanilla, whatever
        // the file says (it keeps the choice for the other graphics APIs)
        s = s.locked_for_api();
        // OMSI_SAFE_GPU=<n>: the game was started again after its graphics device was lost
        // (see `App::restart_after_device_loss`): lighter on the card each time
        if let Some(n) = omsi_cfg::flags::SAFE_GPU.number::<u32>().filter(|n| *n > 0) {
            s.apply_safe_gpu(n);
        }
        log::info!("settings from {}: msaa {} af {} ssao {} shadows {} ({}) navigator {} graphics {} post aa {} vsync {} render scale {} boarding {} min object size {} max object distance {} max fps {}", p.display(), s.msaa, s.anisotropy, s.ssao, s.shadows, s.shadow_size, s.navigator, s.graphics, s.post_aa, s.vsync, s.render_scale_text(), s.boarding, s.min_obj_size, s.object_distance(), s.max_fps);
        s
    }

    /// The settings a `settings.cfg` text describes (anything missing keeps its default).
    pub fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        let mut version = 0u32;
        let mut graphics: Option<String> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            let b =
                |v: &str| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "on" | "yes");
            match k.as_str() {
                "version" => version = v.parse().unwrap_or(0),
                "msaa" => s.msaa = v.parse().unwrap_or(s.msaa),
                "anisotropy" | "af" => s.anisotropy = v.parse().unwrap_or(s.anisotropy),
                "ssao" | "ambient_occlusion" => s.ssao = b(v),
                "shadows" => s.shadows = b(v),
                "shadow_size" => s.shadow_size = v.parse().unwrap_or(s.shadow_size),
                "shadow_blobs" => s.shadow_blobs = b(v),
                "navigator" => s.navigator = b(v),
                "ui_opacity" | "navigator_opacity" => {
                    s.ui_opacity = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.2, 1.0))
                        .unwrap_or(s.ui_opacity)
                }
                "navigator_corner" => s.navigator_corner = v.to_ascii_lowercase(),
                "boarding" => s.boarding = v.to_ascii_lowercase(),
                "detail_textures" | "fractal" => s.detail_textures = b(v),
                "exact_fare" => s.exact_fare = b(v),
                "enhanced" => s.enhanced = b(v),
                "graphics" | "renderer" => graphics = Some(graphics_mode(v).to_string()),
                "vr" => s.vr = b(v),
                "vr_scale" => {
                    s.vr_scale = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.5, 1.0))
                        .unwrap_or(s.vr_scale)
                }
                "vr_head_smoothing_ms" => {
                    s.vr_head_smoothing_ms = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 30.0))
                        .unwrap_or(s.vr_head_smoothing_ms)
                }
                "vr_mirror_rate" => {
                    s.vr_mirror_rate = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(-1.0, 360.0))
                        .unwrap_or(s.vr_mirror_rate)
                }
                "vr_desktop_mirror" => s.vr_desktop_mirror = b(v),
                "triple_screen" => s.triple.enabled = b(v),
                "triple_span" => s.triple_span = b(v),
                "triple_hud_center" => s.triple_hud_center = b(v),
                "triple_fov_deg" => {
                    s.triple.fov_deg = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| if x < 20.0 { 0.0 } else { x.min(120.0) })
                        .unwrap_or(s.triple.fov_deg)
                }
                "triple_width_mm" => {
                    s.triple.width_mm = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(200.0, 2000.0))
                        .unwrap_or(s.triple.width_mm)
                }
                "triple_distance_mm" => {
                    s.triple.distance_mm = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(200.0, 3000.0))
                        .unwrap_or(s.triple.distance_mm)
                }
                "triple_bezel_mm" => {
                    s.triple.bezel_mm = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 100.0))
                        .unwrap_or(s.triple.bezel_mm)
                }
                "triple_left_angle_deg" => {
                    s.triple.left_angle_deg = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 90.0))
                        .unwrap_or(s.triple.left_angle_deg)
                }
                "triple_right_angle_deg" => {
                    s.triple.right_angle_deg = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 90.0))
                        .unwrap_or(s.triple.right_angle_deg)
                }
                "triple_eye_height_mm" => {
                    s.triple.eye_height_mm = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(-500.0, 500.0))
                        .unwrap_or(s.triple.eye_height_mm)
                }
                "fullscreen" => s.fullscreen = b(v),
                "vsync" => s.vsync = b(v),
                "volume" => {
                    s.volume = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .unwrap_or(s.volume)
                }
                // "auto", a fraction (0.75) or a percentage (75)
                "render_scale" => {
                    s.render_scale = if v.eq_ignore_ascii_case("auto") {
                        0.0
                    } else {
                        match v.trim_end_matches('%').parse::<f32>() {
                            Ok(x) if x > 1.5 => (x / 100.0).clamp(0.5, 1.0),
                            Ok(x) if x > 0.0 => x.clamp(0.5, 1.0),
                            Ok(_) => 0.0,
                            Err(_) => s.render_scale,
                        }
                    }
                }
                "language" | "lang" => s.language = crate::describe::language_code(v),
                "pax_voices" => {
                    s.pax_voices = match v.to_ascii_lowercase().as_str() {
                        "tickets" => "tickets".into(),
                        "off" | "0" | "none" => "off".into(),
                        _ => "all".into(),
                    }
                }
                "nav_arrows" => s.nav_arrows = b(v),
                "nav_ai" => s.nav_ai = b(v),
                "nav_follow_view" => s.nav_follow_view = b(v),
                "nav_surface" => s.nav_surface = b(v),
                "nav_light" => s.nav_light = b(v),
                "nav_contours" => s.nav_contours = b(v),
                "nav_orientation" => {
                    s.nav_orientation = if matches!(v, "north" | "heading") {
                        v.to_string()
                    } else {
                        "map".to_string()
                    }
                }
                "nav_view" => {
                    s.nav_view = if v == "top" {
                        "top".to_string()
                    } else {
                        "perspective".to_string()
                    }
                }
                "get_up" => s.get_up = b(v),
                "texture_compression" => s.texture_compression = b(v),
                "auto_clutch" | "automatic_clutch" => s.auto_clutch = b(v),
                "momentary_gears" | "gear_buttons_hold" => s.momentary_gears = b(v),
                "auto_shift" => s.auto_shift = b(v),
                "min_obj_size" | "performance_minobjsize" => {
                    s.min_obj_size = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 0.2))
                        .unwrap_or(s.min_obj_size)
                }
                "max_obj_dist" | "performance_maxobjdist" => {
                    s.max_obj_dist = if v.eq_ignore_ascii_case("off") {
                        0.0
                    } else if v.eq_ignore_ascii_case("auto") {
                        -1.0
                    } else {
                        v.parse::<f32>()
                            .map(|x| x.max(0.0))
                            .unwrap_or(s.max_obj_dist)
                    }
                }
                "max_fps" | "maxfps" => {
                    s.max_fps = v
                        .parse::<f32>()
                        .map(|x| x.max(0.0) as u32)
                        .unwrap_or(s.max_fps);
                    // a phone given the PC OMSI's 30 by the settings import: 60
                    if cfg!(target_os = "android") && s.max_fps == 30 {
                        s.max_fps = 60;
                    }
                }
                "chat" => s.chat = b(v),
                "tooltips" | "mouseover" => s.tooltips = b(v),
                "name_tags" | "nametags" => s.name_tags = b(v),
                "driver" => s.driver = b(v),
                "show_fps" | "fps" => s.show_fps = b(v),
                "clouds" => s.clouds = b(v),
                "windy_trees" => s.windy_trees = b(v),
                "dx9_snow" => s.dx9_snow = b(v),
                "pax_density" | "aipassfactor" => {
                    s.pax_density = v
                        .trim_end_matches('%')
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| if x > 5.0 { x / 100.0 } else { x })
                        .map(|x| x.clamp(0.0, 2.0))
                        .unwrap_or(s.pax_density)
                }
                "vol_ai" => {
                    s.vol_ai = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 1.0))
                        .unwrap_or(s.vol_ai)
                }
                "vol_scenery" => {
                    s.vol_scenery = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 1.0))
                        .unwrap_or(s.vol_scenery)
                }
                "doppler" | "sound_doppler" => s.doppler = b(v),
                "muffle_outside" => s.muffle_outside = b(v),
                "mirror_hud" => {
                    s.mirror_hud = v.parse::<u8>().map(|x| x.min(3)).unwrap_or(s.mirror_hud)
                }
                "mirror_size" => {
                    s.mirror_size = v
                        .parse::<u32>()
                        .map(|x| {
                            if x == 0 {
                                0
                            } else {
                                x.clamp(64, 2048).next_power_of_two()
                            }
                        })
                        .unwrap_or(s.mirror_size)
                }
                "mirror_refresh" | "performance_realreflexions" => {
                    s.mirror_refresh = match v.trim().to_ascii_lowercase().as_str() {
                        "off" | "none" => "off",
                        "eco" | "economy" => "eco",
                        _ => "full",
                    }
                    .into()
                }
                "texture_memory" | "texmemlimit" => {
                    s.texture_memory = v
                        .parse::<f32>()
                        .map(|x| x.max(0.0) as u32)
                        .unwrap_or(s.texture_memory)
                }
                "maintenance" | "wear_lifespan" => {
                    s.maintenance = v.parse::<u8>().map(|x| x.min(4)).unwrap_or(s.maintenance)
                }
                "ai_unsched_factor" | "aiunschedfactor" => {
                    s.ai_unsched_factor = v
                        .trim_end_matches('%')
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| (x / 100.0).clamp(0.01, 3.0))
                        .unwrap_or(s.ai_unsched_factor)
                }
                "ai_max_scheduled" | "aimaxcountscheduled" => {
                    s.ai_max_scheduled = v.parse().unwrap_or(s.ai_max_scheduled)
                }
                "ai_max_random" | "aimaxcountrandom" => {
                    s.ai_max_random = v
                        .parse::<u32>()
                        .map(|x| x.min(1000))
                        .unwrap_or(s.ai_max_random)
                }
                "ai_priority_scheduled" | "aipriorityscheduled" => {
                    s.ai_priority_scheduled = v
                        .parse::<u8>()
                        .map(|x| x.clamp(1, 4))
                        .unwrap_or(s.ai_priority_scheduled)
                }
                "ai_max_humans" | "aimaxhumans" => {
                    s.ai_max_humans = v
                        .parse::<u32>()
                        .map(|x| x.min(1000))
                        .unwrap_or(s.ai_max_humans)
                }
                "ai_max_parked" | "aimaxcountparked" => {
                    s.ai_max_parked = v
                        .parse::<i32>()
                        .map(|x| x.max(-1))
                        .unwrap_or(s.ai_max_parked)
                }
                "collision_vehicles" => s.collision_vehicles = b(v),
                "collision_objects" => s.collision_objects = b(v),
                "collision_pedestrians" => s.collision_pedestrians = b(v),
                "head_movement" | "driverview_moving" => s.head_movement = b(v),
                "driverview_smooth" => s.driverview_smooth = b(v),
                "hands_in_cab" => s.hands_in_cab = b(v),
                "alt_view" => s.alt_view = b(v),
                "precision_zoom" => s.precision_zoom = b(v),
                "time_speed" => {
                    s.time_speed = v
                        .trim_start_matches(['x', 'X'])
                        .parse::<f64>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(1.0, 30.0))
                        .unwrap_or(s.time_speed)
                }
                "time_sync" | "real_time_sync" => s.time_sync = b(v),
                "metar_sync" => s.metar_sync = b(v),
                "metar_station" => {
                    s.metar_station = v
                        .trim()
                        .chars()
                        .filter(|c| c.is_ascii_alphabetic())
                        .take(4)
                        .collect::<String>()
                        .to_ascii_uppercase()
                }
                "ctrl_deadzone" => {
                    s.ctrl_deadzone = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 0.3))
                        .unwrap_or(s.ctrl_deadzone)
                }
                "right_stick_look" => s.right_stick_look = b(v),
                "pad_steer_linear" => s.pad_steer_linear = b(v),
                "arrows_switch_cams" => s.arrows_switch_cams = b(v),
                "pad_steer_smooth" => {
                    s.pad_steer_smooth = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 300.0))
                        .unwrap_or(s.pad_steer_smooth)
                }
                "reflections" | "envmap" => s.reflections = b(v),
                "led_glow" => {
                    s.led_glow = v
                        .trim()
                        .parse::<i32>()
                        .map(|x| x.clamp(0, 15) as u8)
                        .unwrap_or(s.led_glow)
                }
                "night_brightness" => {
                    s.night_brightness = v
                        .trim()
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 3.0))
                        .unwrap_or(s.night_brightness)
                }
                "led_mips" => {
                    s.led_mips = v
                        .trim()
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 4.0))
                        .unwrap_or(s.led_mips)
                }
                "graphics_api" => {
                    s.graphics_api = match v.trim().to_ascii_lowercase().as_str() {
                        x @ ("vulkan" | "dx12" | "gl") => x.to_string(),
                        "d3d9" | "d3d9ex" | "dx9" if cfg!(windows) => "d3d9".to_string(),
                        _ => "auto".to_string(),
                    }
                }
                "ctrl_off" => s.ctrl_off = v.trim().to_string(),
                "steering_linear" => s.steering_linear = b(v),
                "old_steering" => s.old_steering = b(v),
                "red_steer_spd" => s.red_steer_spd = b(v),
                "ff_invert" => s.ff_invert = b(v),
                "ff_enabled" => s.ff_enabled = b(v),
                "ff_road_vib" => {
                    s.ff_road_vib = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 4.0))
                        .unwrap_or(s.ff_road_vib)
                }
                "ff_engine_vib" => {
                    s.ff_engine_vib = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 4.0))
                        .unwrap_or(s.ff_engine_vib)
                }
                "ff_fade" => {
                    s.ff_fade = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 1.5))
                        .unwrap_or(s.ff_fade)
                }
                "brake_hold" => s.brake_hold = b(v),
                "mouse_steering" => s.mouse_steering = b(v),
                "mouse_right_off" => s.mouse_right_off = b(v),
                "mouse_smooth" => s.mouse_smooth = b(v),
                "blinker_cancel" => s.blinker_cancel = b(v),
                "look_sens" => {
                    s.look_sens = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.1, 2.0))
                        .unwrap_or(s.look_sens)
                }
                "look_smoothing_ms" => {
                    s.look_smoothing_ms = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 200.0))
                        .unwrap_or(s.look_smoothing_ms)
                }
                "wheel_range" => {
                    s.wheel_range = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(90.0, 2880.0))
                        .unwrap_or(s.wheel_range)
                }
                "wheel_lock" => {
                    s.wheel_lock = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| if x < 45.0 { 0.0 } else { x.min(2880.0) })
                        .unwrap_or(s.wheel_lock)
                }
                "camera_collision" => s.camera_collision = b(v),
                "steer_look" => s.steer_look = b(v),
                "steer_look_angle" => {
                    s.steer_look_angle = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 60.0))
                        .unwrap_or(s.steer_look_angle)
                }
                "steer_look_response" => {
                    s.steer_look_response = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.05, 1.0))
                        .unwrap_or(s.steer_look_response)
                }
                "head_idle" => {
                    s.head_idle = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.0, 1.0))
                        .unwrap_or(s.head_idle)
                }
                "head_idle_pace" => {
                    s.head_idle_pace = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.5, 2.0))
                        .unwrap_or(s.head_idle_pace)
                }
                "head_tracking" => s.head_tracking = b(v),
                "head_tracking_invert" => s.head_tracking_invert = v.to_ascii_lowercase(),
                "head_tracking_yaw_sens" => {
                    s.head_tracking_yaw_sens = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| (if x <= 2.0 { x * 100.0 } else { x }).clamp(0.0, 100.0))
                        .unwrap_or(s.head_tracking_yaw_sens)
                }
                "head_tracking_pitch_sens" => {
                    s.head_tracking_pitch_sens = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| (if x <= 2.0 { x * 100.0 } else { x }).clamp(0.0, 100.0))
                        .unwrap_or(s.head_tracking_pitch_sens)
                }
                "head_tracking_roll_sens" => {
                    s.head_tracking_roll_sens = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| (if x <= 2.0 { x * 100.0 } else { x }).clamp(0.0, 100.0))
                        .unwrap_or(s.head_tracking_roll_sens)
                }
                "head_tracking_x_sens" => {
                    s.head_tracking_x_sens = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| (if x <= 2.0 { x * 100.0 } else { x }).clamp(0.0, 100.0))
                        .unwrap_or(s.head_tracking_x_sens)
                }
                "head_tracking_y_sens" => {
                    s.head_tracking_y_sens = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| (if x <= 2.0 { x * 100.0 } else { x }).clamp(0.0, 100.0))
                        .unwrap_or(s.head_tracking_y_sens)
                }
                "head_tracking_z_sens" => {
                    s.head_tracking_z_sens = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| (if x <= 2.0 { x * 100.0 } else { x }).clamp(0.0, 100.0))
                        .unwrap_or(s.head_tracking_z_sens)
                }
                "head_tracking_invert_yaw" => s.head_tracking_invert_yaw = b(v),
                "head_tracking_invert_pitch" => s.head_tracking_invert_pitch = b(v),
                "head_tracking_invert_roll" => s.head_tracking_invert_roll = b(v),
                "head_tracking_invert_x" => s.head_tracking_invert_x = b(v),
                "head_tracking_invert_y" => s.head_tracking_invert_y = b(v),
                "head_tracking_invert_z" => s.head_tracking_invert_z = b(v),
                "voice_chat" => s.voice_chat = b(v),
                "head_tracking_port" => {
                    s.head_tracking_port = v
                        .parse::<u16>()
                        .ok()
                        .filter(|p| *p > 0)
                        .unwrap_or(s.head_tracking_port)
                }
                "head_tracking_bind" => s.head_tracking_bind = v.trim().to_string(),
                "pedal_throttle" => {
                    s.pedal_throttle = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.25, 4.0))
                        .unwrap_or(s.pedal_throttle)
                }
                "pedal_brake" => {
                    s.pedal_brake = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.25, 4.0))
                        .unwrap_or(s.pedal_brake)
                }
                "seat_x" | "seat_y" | "seat_z" => {
                    let k = (k.as_bytes()[5] - b'x') as usize;
                    s.seat[k] = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(-1.5, 1.5))
                        .unwrap_or(0.0);
                }
                "seat_pitch_deg" => {
                    s.seat_pitch_deg = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(-45.0, 45.0))
                        .unwrap_or(s.seat_pitch_deg)
                }
                "outside_fov" => {
                    s.outside_fov = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| if x < 20.0 { 0.0 } else { x.min(120.0) })
                        .unwrap_or(s.outside_fov)
                }
                "fov" => {
                    s.fov = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| if x < 20.0 { 0.0 } else { x.min(120.0) })
                        .unwrap_or(s.fov)
                }
                "mouse_sens" => {
                    s.mouse_sens = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.1, 3.0))
                        .unwrap_or(s.mouse_sens)
                }
                "ui_scale_window" => s.ui_scale_window = b(v),
                "notes" => s.notes = b(v),
                "info_bar" => s.info_bar = b(v),
                "info_bar_mode" => {
                    s.info_bar_mode = if matches!(v, "compact" | "essential" | "custom") {
                        v.to_string()
                    } else {
                        "full".to_string()
                    }
                }
                "info_bar_pos" => {
                    s.info_bar_pos = if v == "bottom" { "bottom" } else { "top" }.to_string()
                }
                "info_bar_fps" => s.info_bar_fps = b(v),
                k if info_item_index(k).is_some() => {
                    if let Some(i) = info_item_index(k) {
                        s.info_items[i] = b(v);
                    }
                }
                "autosave_minutes" => {
                    s.autosave_minutes = v
                        .parse::<u32>()
                        .map(|x| x.min(60))
                        .unwrap_or(s.autosave_minutes)
                }
                "ui_accent" => {
                    s.ui_accent = omsi_ui::theme::Accent::named(v)
                        .unwrap_or_default()
                        .name()
                        .into()
                }
                "ui_corners" => {
                    s.ui_corners = if v.eq_ignore_ascii_case("square") {
                        "square"
                    } else {
                        "rounded"
                    }
                    .into()
                }
                "chat_size" => {
                    s.chat_size = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.5, 3.0))
                        .unwrap_or(s.chat_size)
                }
                "ui_scale" => {
                    s.ui_scale = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(|x| x.clamp(0.5, 2.0))
                        .unwrap_or(s.ui_scale)
                }
                "shadow_casters" => {
                    s.shadow_casters = if v.eq_ignore_ascii_case("omsi") {
                        "omsi".into()
                    } else {
                        "all".into()
                    }
                }
                "post_aa" => {
                    s.post_aa = if matches!(
                        v.to_ascii_lowercase().as_str(),
                        "off" | "0" | "none" | "false"
                    ) {
                        "off".into()
                    } else {
                        "fxaa".into()
                    }
                }
                "drive_keys" => {
                    s.drive_keys = match v.to_ascii_lowercase().as_str() {
                        "wasd" | "arrows" | "omsi" | "simple" => v.to_ascii_lowercase(),
                        _ => s.drive_keys,
                    }
                }
                _ => {}
            }
        }
        // `graphics` decides; a file without it (older builds) says only `enhanced`, and
        // its vanilla renderer is what is now called Vanilla+
        s.graphics = graphics.unwrap_or_else(|| {
            if s.enhanced {
                "enhanced"
            } else {
                "vanilla_plus"
            }
            .to_string()
        });
        s.enhanced = s.graphics == "enhanced" || s.graphics == "enhanced_plus";
        // Enhanced+ is Enhanced with ray tracing: its shadows, ambient occlusion and
        // reflections are traced and belong to it - they cannot be switched off apart
        if s.ray_tracing() {
            s.shadows = true;
            s.ssao = true;
            s.reflections = true;
        }
        if s.classic() {
            s.shadows = false;
            s.ssao = false;
            s.detail_textures = false;
        }
        // Files older than version 2 say `boarding=pay` because that was the launcher's
        // default, not because anybody chose it: passengers then stood at the cash desk
        // waiting for a driver who did not know he had to sell them a ticket.
        if version < 2 && s.boarding == "pay" {
            log::info!("settings: boarding=pay from an old settings file taken as auto (choose pay again in the launcher to keep it)");
            s.boarding = "auto".into();
        }
        // Before version 3 the count options meant something else: no timetable limit was 0
        // and every parking space was 0 (-1 none, other values a count); they now follow the
        // original's sliders (0 is none, 1000 and 100 percent the most).
        if version < 3 {
            if s.ai_max_scheduled == 0 {
                s.ai_max_scheduled = 1000;
            }
            s.ai_max_parked = if s.ai_max_parked == -1 { 0 } else { 100 };
        }
        s.ai_max_scheduled = s.ai_max_scheduled.min(1000);
        s.ai_max_parked = s.ai_max_parked.clamp(0, 100);
        s
    }

    /// Enhanced+ graphics: Enhanced with ray-traced sun shadows, ambient occlusion and
    /// reflections (and the graded look that goes with them).
    pub fn ray_tracing(&self) -> bool {
        self.graphics == "enhanced_plus"
    }

    /// Windy trees, unless `OMSI_WINDY_TREES=0`/`1` says otherwise (A/B renders).
    pub fn windy_trees(&self) -> bool {
        match omsi_cfg::flags::WINDY_TREES.text() {
            Some(v) => v.trim() != "0",
            None => self.windy_trees,
        }
    }

    /// The graphics backend these settings choose for the game window (see `Backend::of`).
    pub(crate) fn backend(&self) -> crate::output::Backend {
        crate::output::Backend::of(self)
    }

    /// Whether the falling snow of "DirectX9 (D3D9Ex)" is drawn: its own option, and only
    /// under that API (the other renderers draw their own snowfall).
    pub fn dx9_snowfall(&self) -> bool {
        self.dx9_snow && self.d3d9_selected()
    }

    /// Whether a clear sky with no weather chosen may get a few fair-weather clouds (when
    /// clouds are on): an addition OMSI 2 does not make, so not under "DirectX9 (D3D9Ex)".
    pub fn fair_weather_clouds(&self) -> bool {
        self.backend() != crate::output::Backend::D3d9
    }

    /// These settings as the chosen graphics API takes them: Vanilla under "DirectX9 (D3D9Ex)".
    pub fn locked_for_api(mut self) -> Self {
        if self.backend() == crate::output::Backend::D3d9 {
            self.lock_vanilla();
        }
        self
    }

    /// Vanilla graphics, whatever was set: the "same as OMSI 2" mode of "DirectX9 (D3D9Ex)"
    /// has no graphics improvements to choose.
    pub fn lock_vanilla(&mut self) {
        self.graphics = "vanilla".into();
        self.enhanced = false;
        // (what a Vanilla file is loaded without, see `from_text`)
        self.shadows = false;
        self.ssao = false;
        self.detail_textures = false;
        // (the other limits of the table in `api_limits`: what DirectX9 does not draw yet)
        self.render_scale = 0.0;
        self.post_aa = "off".into();
        // (flat mirrors are drawn, in the sizes OMSI 2 offers and on or off: no economical
        // refresh, no picture beyond 512; the corner copies of the mirrors are ours too)
        if let Some(l) = crate::api_limits::d3d9("mirror_size") {
            self.mirror_size = l.fit(&self.mirror_size.to_string()).parse().unwrap_or(0);
        }
        if let Some(l) = crate::api_limits::d3d9("mirror_refresh") {
            self.mirror_refresh = l.fit(&self.mirror_refresh);
        }
        self.mirror_hud = 0;
        self.windy_trees = false;
        // (OMSI 2's sky: the weather's own clouds, none added to a clear sky, see
        // `fair_weather_clouds`)
        self.clouds = true;
        // (the sun's own light only: Enhanced's brighter night is an improvement)
        self.night_brightness = 0.0;
    }

    /// Vanilla graphics: the picture as OMSI 2 draws it.
    pub fn classic(&self) -> bool {
        self.graphics == "vanilla"
    }

    /// How far objects are drawn (m, 0 = no limit): `max_obj_dist`, or when that is `auto`
    /// the visible distance the launcher sets, else the original's 900 m.
    pub fn object_distance(&self) -> f32 {
        if self.max_obj_dist >= 0.0 {
            self.max_obj_dist
        } else {
            view_distance().map(|v| v as f32).unwrap_or(900.0)
        }
    }

    /// `auto` or the fraction, as the file and the log write it.
    pub fn render_scale_text(&self) -> String {
        if self.render_scale > 0.0 {
            format!("{}", self.render_scale)
        } else {
            "auto".into()
        }
    }

    /// Lighter graphics after the graphics device was lost `n` times this session: no
    /// multisampling, no SSAO, smaller shadow and mirror maps, fewer textures kept; a second
    /// loss also a smaller picture and no shadows.
    pub fn apply_safe_gpu(&mut self, n: u32) {
        self.msaa = 1;
        self.ssao = false;
        self.shadow_size = self.shadow_size.min(2048);
        self.mirror_size = self.mirror_size.min(256);
        let budget = if self.texture_memory > 0 {
            self.texture_memory
        } else {
            1200
        };
        self.texture_memory = (budget * 2 / 3).max(400);
        if n >= 2 {
            self.shadows = false;
            self.shadow_size = 1024;
            self.render_scale = if self.render_scale > 0.0 {
                self.render_scale.min(0.75)
            } else {
                0.75
            };
            self.texture_memory = self.texture_memory.min(700);
            self.mirror_size = self.mirror_size.min(128);
        }
        log::warn!("safer graphics after a lost graphics device ({n}): msaa 1, SSAO off, shadows {} ({}), textures {} MB, render scale {}", self.shadows, self.shadow_size, self.texture_memory, self.render_scale_text());
    }

    pub fn render_options(&self) -> omsi_render::RenderOptions {
        omsi_render::RenderOptions {
            msaa: self.msaa,
            anisotropy: self.anisotropy,
            shadow_size: self.shadow_size,
            ssao: self.ssao,
            render_scale: self.render_scale,
            compress_textures: self.texture_compression,
            fxaa: self.post_aa != "off",
            min_obj_size: self.min_obj_size,
            max_obj_dist: self.object_distance(),
            omsi_shadow_casters: self.shadow_casters == "omsi",
            shadow_blobs: self.shadow_blobs,
            reflections: self.reflections,
            no_enhanced: !matches!(graphics_mode(&self.graphics), "enhanced" | "enhanced_plus"),
            ray_tracing: self.ray_tracing()
                || crate::startup::ENHANCED_PLUS.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

/// `vanilla`, `vanilla_plus`, `enhanced` or `enhanced_plus` from the ways a file may spell them.
pub fn graphics_mode(v: &str) -> &'static str {
    match v
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "enhanced_plus" | "enhanced+" | "enhancedplus" | "enhanced_+" | "2" => "enhanced_plus",
        "enhanced" | "1" => "enhanced",
        "vanilla" | "classic" | "original" | "omsi" | "omsi2" | "omsi_2" => "vanilla",
        _ => "vanilla_plus",
    }
}

/// `view_distance=<metres>` of the settings file: how far around the camera the map's tiles
/// are kept loaded (OMSI's "visible distance"). None when the file does not say.
pub fn view_distance() -> Option<f64> {
    let text = std::fs::read_to_string(Settings::path()?).ok()?;
    text.lines()
        .filter_map(|l| l.trim().split_once('='))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("view_distance"))
        .and_then(|(_, v)| v.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
}

/// The file as the game writes it and as the launcher writes it, read by the other side
/// (`settings/round_trip.rs`).

impl Settings {
    /// The player's bus's `wearlifespan` for the maintenance condition (OMSI's table).
    pub fn wear_lifespan(&self) -> f32 {
        [1.5e6, 0.01, 0.1, 1.0, 10.0][self.maintenance.min(4) as usize]
    }
}

/// The player's own turn of a bus's mirrors (degrees yaw, pitch per `[add_camera_reflexion]`),
/// kept per `.bus` file in `~/.depot/mirrors.cfg` as `<bus file>|<mirror>=<yaw>,<pitch>`.
pub fn mirror_offsets(bus: &std::path::Path) -> Vec<[f32; 2]> {
    let key = bus.to_string_lossy().to_ascii_lowercase();
    let Some(p) = omsi_cfg::paths::mirrors() else {
        return Vec::new();
    };
    let text = std::fs::read_to_string(p).unwrap_or_default();
    let mut out: Vec<[f32; 2]> = Vec::new();
    for line in text.lines() {
        let Some((k, v)) = line.rsplit_once('=') else {
            continue;
        };
        let Some((file, i)) = k.rsplit_once('|') else {
            continue;
        };
        let (Ok(i), Some((y, p))) = (i.trim().parse::<usize>(), v.split_once(',')) else {
            continue;
        };
        if file.trim().to_ascii_lowercase() != key || i > 64 {
            continue;
        }
        if out.len() <= i {
            out.resize(i + 1, [0.0; 2]);
        }
        out[i] = [
            y.trim().parse().unwrap_or(0.0),
            p.trim().parse().unwrap_or(0.0),
        ];
    }
    out
}

/// A bus's mirror shifts (bus frame, m: across, along, up), kept in the same file as the
/// turns on lines `<bus>|s<n>=x,y,z` (a build that knows only turns skips them).
pub fn mirror_shifts(bus: &std::path::Path) -> Vec<[f32; 3]> {
    let key = bus.to_string_lossy().to_ascii_lowercase();
    let Some(p) = omsi_cfg::paths::mirrors() else {
        return Vec::new();
    };
    let text = std::fs::read_to_string(p).unwrap_or_default();
    let mut out: Vec<[f32; 3]> = Vec::new();
    for line in text.lines() {
        let Some((k, v)) = line.rsplit_once('=') else {
            continue;
        };
        let Some((file, i)) = k.rsplit_once('|') else {
            continue;
        };
        let Some(i) = i
            .trim()
            .strip_prefix('s')
            .and_then(|n| n.parse::<usize>().ok())
        else {
            continue;
        };
        let xyz: Vec<f32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if file.trim().to_ascii_lowercase() != key || i > 64 || xyz.len() != 3 {
            continue;
        }
        if out.len() <= i {
            out.resize(i + 1, [0.0; 3]);
        }
        out[i] = [xyz[0], xyz[1], xyz[2]];
    }
    out
}

/// A bus's mirror field-of-view changes (degrees added to the mirror camera's), on lines
/// `<bus>|f<n>=degrees` of the same file.
pub fn mirror_fovs(bus: &std::path::Path) -> Vec<f32> {
    let key = bus.to_string_lossy().to_ascii_lowercase();
    let Some(p) = omsi_cfg::paths::mirrors() else {
        return Vec::new();
    };
    let text = std::fs::read_to_string(p).unwrap_or_default();
    let mut out: Vec<f32> = Vec::new();
    for line in text.lines() {
        let Some((k, v)) = line.rsplit_once('=') else {
            continue;
        };
        let Some((file, i)) = k.rsplit_once('|') else {
            continue;
        };
        let Some(i) = i
            .trim()
            .strip_prefix('f')
            .and_then(|n| n.parse::<usize>().ok())
        else {
            continue;
        };
        let Ok(d) = v.trim().parse::<f32>() else {
            continue;
        };
        if file.trim().to_ascii_lowercase() != key || i > 64 {
            continue;
        }
        if out.len() <= i {
            out.resize(i + 1, 0.0);
        }
        out[i] = d.clamp(-60.0, 60.0);
    }
    out
}

/// Keep a bus's mirror turns, shifts and field-of-view changes (see [`mirror_offsets`],
/// [`mirror_shifts`], [`mirror_fovs`]).
pub fn save_mirror_state(
    bus: &std::path::Path,
    offsets: &[[f32; 2]],
    shifts: &[[f32; 3]],
    fovs: &[f32],
) {
    let key = bus.to_string_lossy().to_ascii_lowercase();
    let Some(p) = omsi_cfg::paths::mirrors() else {
        return;
    };
    let text = std::fs::read_to_string(&p).unwrap_or_default();
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| {
            l.rsplit_once('=')
                .and_then(|(k, _)| k.rsplit_once('|'))
                .is_none_or(|(f, _)| f.trim().to_ascii_lowercase() != key)
        })
        .map(str::to_string)
        .collect();
    for (i, o) in offsets.iter().enumerate() {
        if o[0].abs() > 0.01 || o[1].abs() > 0.01 {
            lines.push(format!(
                "{}|{i}={:.1},{:.1}",
                bus.to_string_lossy(),
                o[0],
                o[1]
            ));
        }
    }
    for (i, s) in shifts.iter().enumerate() {
        if s.iter().any(|v| v.abs() > 0.002) {
            lines.push(format!(
                "{}|s{i}={:.3},{:.3},{:.3}",
                bus.to_string_lossy(),
                s[0],
                s[1],
                s[2]
            ));
        }
    }
    for (i, d) in fovs.iter().enumerate() {
        if d.abs() > 0.05 {
            lines.push(format!("{}|f{i}={:.1}", bus.to_string_lossy(), d));
        }
    }
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&p, lines.join("\n") + "\n");
}
/// `resolution=` (or `window_size=`) of a settings text, the last one written.
pub fn resolution_of(text: &str) -> Option<(u32, u32)> {
    let v = text
        .lines()
        .filter_map(|l| l.split_once('='))
        .filter(|(k, _)| {
            matches!(
                k.trim().to_ascii_lowercase().as_str(),
                "resolution" | "window_size"
            )
        })
        .last()?
        .1;
    let (w, h) = omsi_launcher_lib::resolution_text(v)
        .split_once('x')
        .map(|(w, h)| (w.parse().ok(), h.parse().ok()))?;
    Some((w?, h?))
}

/// The driver's seat per bus (`~/.depot/seats.cfg`, a line `bus|x|y|z|pitch` each, the
/// bus by its `.bus` file relative to the installation, lower case). The seat set in the
/// menu was one for every bus: fitted to a Solaris, every other bus put the driver
/// somewhere else than its `.bus` file does (#1355). A bus that has none takes the
/// settings' own seat, which is where a seat set before this lives on.
pub mod bus_seats {
    use std::path::PathBuf;

    fn path() -> Option<PathBuf> {
        omsi_cfg::paths::seats()
    }

    fn key(bus: &str) -> String {
        bus.trim().replace('\\', "/").to_ascii_lowercase()
    }

    fn read() -> Vec<(String, [f32; 3], f32)> {
        let Some(t) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
            return Vec::new();
        };
        t.lines().filter_map(parse).collect()
    }

    pub fn parse(line: &str) -> Option<(String, [f32; 3], f32)> {
        let mut f = line.rsplitn(5, '|');
        let pitch = f.next()?.trim().parse().ok()?;
        let z = f.next()?.trim().parse().ok()?;
        let y = f.next()?.trim().parse().ok()?;
        let x = f.next()?.trim().parse().ok()?;
        let bus = f.next()?.to_string();
        (!bus.is_empty()).then_some((bus, [x, y, z], pitch))
    }

    /// The seat kept for `bus` (its `.bus` path), if one is.
    pub fn of(bus: &str) -> Option<([f32; 3], f32)> {
        let k = key(bus);
        read().into_iter().find(|e| e.0 == k).map(|e| (e.1, e.2))
    }

    /// Keep `seat` and `pitch` for `bus`.
    pub fn remember(bus: &str, seat: [f32; 3], pitch: f32) {
        let Some(p) = path() else { return };
        let k = key(bus);
        let mut all = read();
        all.retain(|e| e.0 != k);
        all.push((k, seat, pitch));
        let text: String = all
            .iter()
            .map(|(b, s, pt)| format!("{b}|{}|{}|{}|{pt}\n", s[0], s[1], s[2]))
            .collect();
        let _ = std::fs::write(p, text);
    }
}
