//! The types of the player: the player's bus with its controls and cameras, the driver on
//! the bus's `[drivpos]`, the walk about the bus, a rail vehicle bound to its track. Their
//! procedures are in `implementation/`.

use crate::gameplay::schedule;
use crate::output::camera_arm;
use crate::world::scene;
use glam::{DVec3, Vec3};

/// The vehicle actions of the keys held whose `Inputs/keyboard.cfg` entry has the "held"
/// flag ([`omsi_content::input::KEY_HOLD`], the key list's " *"), by scan code: OMSI
/// gives the vehicle their action again every frame the key stays down, not
/// only when it goes down. The stock roller blind turns while Page Up / Page Down are held
/// (`bus_rollband_up`, `_dn`: its line or destination moves by the frame's time on each
/// call); fired once, it moved by a frame's worth and no more (#1098, #1145).
#[derive(Default)]
pub struct HeldRepeat(pub(crate) hashbrown::HashMap<i32, (Vec<String>, bool)>);

/// Everything about the spawned player vehicle.
pub struct Player {
    /// Which vehicle this is, for as long as the session runs (the people riding in one
    /// the player left know it by this, see `Humans::player_bus_swapped`).
    pub(crate) uid: u64,
    pub(crate) vehicle: omsi_sim::VehicleInstance,
    pub(crate) render: scene::VehicleRender,
    /// Renders of the coupled parts, parallel to `vehicle.trailers`.
    pub(crate) trailer_renders: Vec<scene::VehicleRender>,
    pub(crate) axes: omsi_sim::KeyboardAxes,
    /// A game controller's pedals and steering this frame (they win over the keys).
    pub(crate) analog: crate::input::controllers::Analog,
    /// The interior cameras chosen (OMSI's
    /// `view_interiorcam_minus`/`_plus`): the driver's and the passengers' camera numbers.
    pub(crate) cam_choice: (usize, usize),
    /// (scan code, modifier bits) → action name, from `Inputs/keyboard.cfg` `[vehicles]`.
    pub(crate) bindings: Vec<omsi_content::KeyBinding>,
    pub(crate) sounds: Option<omsi_audio::SoundSet>,
    /// Mesh index currently pressed with the mouse (its `[mouseevent]` gets `_off` on release).
    pub(crate) pressed_mesh: Option<usize>,
    /// Coupled-part mesh currently pressed.  Articulated buses keep the rear controls in
    /// the trailer model, while their mouse events are still handled by the lead vehicle.
    pub(crate) pressed_trailer_mesh: Option<(usize, usize)>,
    /// The camera is one of the bus's interior cameras (the last `sync_transforms`): which
    /// meshes are drawn by `[viewpoint]`, and so which can be clicked or stand in the way.
    pub(crate) view_inside: bool,
    /// The press on `pressed_mesh`: whether the script has a trigger for the click itself,
    /// and how far the mouse has been dragged since (px).
    pub(crate) press_info: (bool, f32),
    /// A control worked only by dragging (the NL/NG driver's door: `cp_Fahrertuer_drag`
    /// and no click trigger) that was clicked: it is dragged over to its other end.
    pub(crate) auto_drag: Option<AutoDrag>,
    /// Running auto-start (Shift+U).
    pub(crate) startup: Option<omsi_sim::startup::StartUp>,
    /// When the running auto-start began (one that has gone on for long is given up by the
    /// next Shift+U).
    pub(crate) startup_at: Option<std::time::Instant>,
    /// The ticket key was pressed this frame (sell the requested ticket).
    pub(crate) give_ticket: bool,
    /// OMSI's `change_give` / `change_take` keys: hand the passenger
    /// at the desk all the change owed at once / take back what lies on the change tray.
    pub(crate) give_change: bool,
    /// The triggers a controller button's door action (`door_<n>`, `doors_all`) fired, by
    /// action: let go, their `_off` is fired (see [`Player::door_key`]).
    pub(crate) door_buttons: hashbrown::HashMap<String, Vec<String>>,
    /// Parts at the end of the train coupled by hand (they can be uncoupled; an articulated
    /// bus's own rear section cannot).
    pub(crate) hand_coupled: usize,
    /// Bound to the rails (see `rail_drive`), and where on them once it is placed.
    pub(crate) rail_bound: bool,
    pub(crate) rail: Option<crate::gameplay::rail_drive::RailDrive>,
    /// The driver camera chosen before the timetable or ticket desk camera took over.
    pub(crate) cam_before_special: Option<usize>,
    /// The actions a key started, by its scan code: its release lets go of the same ones,
    /// whatever modifier is held by then (L pressed, Ctrl held, L let go: the release went
    /// to Ctrl+L and the L action stayed held).
    pub(crate) held_keys: hashbrown::HashMap<i32, Vec<String>>,
    /// The held ones of them that are told every frame (see [`HeldRepeat`]).
    pub(crate) held_repeat: HeldRepeat,
    /// Where the driver's head is thrown by the bus's accelerations (vehicle frame, m):
    /// OMSI's `[driverview_moving]`.
    pub(crate) head: Vec3,
    /// Its speed (m/s) and the body's turning rates of the frame before (see `move_head`).
    pub(crate) head_vel: Vec3,
    pub(crate) head_omega: Vec3,
    /// What a head does when nothing moves it: the slow sway of a standing body, kept apart
    /// from `head` so the bus's own head movement is never fed it (Settings -> idle head sway;
    /// read through `head_offset`).
    pub(crate) head_idle: crate::output::HeadIdle,
    /// How far the driver's view is turned into the steering (degrees of yaw; see `move_head`).
    pub(crate) steer_look: f32,
    /// The driver's seat moved (Settings → seat position; bus frame, m).
    pub(crate) seat: Vec3,
    /// The player's turn of each mirror (yaw, pitch degrees; Ctrl+Alt+arrows in the cab).
    pub(crate) mirror_offsets: Vec<[f32; 2]>,
    /// The player's shift of each mirror (bus frame, m: across, along, up; the mirror editor).
    pub(crate) mirror_shifts: Vec<[f32; 3]>,
    /// Degrees added to each mirror camera's field of view (the mirror editor).
    pub(crate) mirror_fovs: Vec<f32>,
    /// A mirror was turned or shifted and is not saved yet.
    pub(crate) mirrors_dirty: bool,
    pub(crate) take_change: bool,
    /// Keys whose `_toggle` this bus does as `_up`/`_down` (see `action`): turned up last.
    pub(crate) toggled_up: hashbrown::HashSet<String>,
    /// H-pattern actions act as momentary gear buttons when this is enabled - those of a
    /// controller (an H shifter) only: a key stays in its gear as in OMSI (#1440).
    pub(crate) momentary_gears: bool,
    /// The actions now firing come from the keyboard (`key`).
    pub(crate) from_keyboard: bool,
    /// The settings' automated manual (#713): a gear lever's gates are worked by the
    /// engine speed (see [`Player::tick_auto_shift`]).
    pub(crate) auto_shift: bool,
    /// Seconds until the automated manual may shift again; the engine's idle speed as seen.
    pub(crate) auto_shift_wait: f32,
    pub(crate) auto_shift_idle: f32,
    /// L switched the side lights on with the headlights (see
    /// [`Player::headlights_with_side_lights`]).
    pub(crate) side_lights_by_l: bool,
    /// The driver figure at the wheel (see `driver.rs`).
    pub(crate) driver: Option<crate::gameplay::driver::DriverFigure>,
    /// The duty's trip to type into the IBIS once the auto-start has the electrics on:
    /// (line, terminus, the trip's stops, the stop the bus is at: its index and name).
    pub(crate) ibis_duty: Option<(String, String, Vec<String>, (usize, String))>,
    /// A destination picked by hand for the bus's roller blind, kept until the electrics are
    /// on and the auto-start is done (`schedule::turn_roller_blind`).
    pub(crate) blind_pick: Option<schedule::BlindPick>,
    /// The IBIS being typed, with the line and terminus it is typed for.
    pub(crate) ibis_typist: Option<(omsi_sim::ibis::Typist, String, String, Vec<String>)>,
    /// The duty is typed into the IBIS by itself (after Shift+U or `--autostart`): a new
    /// trip of the duty is typed too.
    pub(crate) duty_typed: bool,
    /// The stop a page asked the duty to go on with (`omsi.setNextStop`), for the game to
    /// hand to the duty.
    pub(crate) html_next_stop: Option<usize>,
    /// The IBIS typing looks for its keys on a worker thread (in the window: the trials
    /// would hold a frame up to a second and a half; an offscreen run waits for them).
    pub(crate) ibis_background: bool,
    /// The outside camera's arm (how far out it is swung right now).
    pub(crate) arm: camera_arm::SpringArm,
    /// What `Z`/`X`/`C` last turned on, so a repeat press of the same key turns it back off
    /// (real OMSI's Z/X/C and Shift+numpad 4/6/5 are toggles, not one-shot "set" buttons):
    /// 0 = nothing, 1 = left, 2 = right, 3 = hazard.
    pub(crate) blinker_key_state: u8,
    /// The settings' "Indicators cancel themselves" (`blinker_cancel`): off, the script's
    /// own cancelling after a turn is undone (#451).
    pub(crate) blinker_cancel: bool,
}

/// See `Player::auto_drag`.
#[derive(Debug, Clone)]
pub(crate) struct AutoDrag {
    pub(crate) ev: String,
    pub(crate) var: String,
    pub(crate) target: f32,
    /// The mouse axis (0 x, 1 y) and direction that move it towards the target.
    pub(crate) axis: usize,
    pub(crate) sign: f32,
    pub(crate) last: f32,
    pub(crate) stalled: u32,
    pub(crate) t: f32,
}

/// What one look under the cursor took, for the profiler: seconds in the switch pick of the
/// bus, of its coupled parts, along the rays for what stands in front of a switch, and for
/// the name of a part that is no switch; and how many triangles were tested.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HoverSplit {
    pub pick: f64,
    pub trailer: f64,
    pub blocker: f64,
    pub body: f64,
    pub triangles: u64,
}

/// One look under the cursor: each mesh's posed transform and its inverse (part 0 the bus,
/// then its coupled parts), the rings of rays round the cursor's, and how far along each ring
/// ray the nearest other mesh is once traced. The switch pick of the bus and that of its
/// coupled parts cast the same rays, and up to 25 rays went through every mesh, each time
/// posing it and inverting its transform again.
pub(crate) struct Look<'a> {
    pub(crate) vehicle: &'a omsi_sim::VehicleInstance,
    pub(crate) posed: Vec<Vec<(glam::Mat4, glam::Mat4)>>,
    pub(crate) origin: DVec3,
    pub(crate) dir: Vec3,
    pub(crate) spread: f32,
    pub(crate) inside: bool,
    pub(crate) rings: Vec<Vec<Vec3>>,
    pub(crate) blockers: Vec<Vec<Option<f32>>>,
    pub(crate) split: HoverSplit,
}
