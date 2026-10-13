//! A road vehicle: its type (definition + compiled scripts + model) and instances.

use crate::particles::ParticleSet;
use crate::anim::MeshAnimator;
use crate::host::VehicleHost;
use crate::physics::{Controls, Dynamics, KinematicModel, VehicleMotion};
use glam::{DVec3, Mat4, Vec3};
use hashbrown::HashMap;
use omsi_script::{State, Vm};
use std::sync::Arc;

mod ai;
mod articulation;
mod builtins;
mod motion;
mod physics_bridge;
mod pose;
mod props;
mod radio;
mod scripts;
mod skin;
mod trailer;
mod vehicle_type;
mod visuals;
mod weather;
mod winding;

use props::PropsPlan;
pub use ai::AiFrame;
pub use builtins::{builtin_str_vars, builtin_vars, PAX_DOORS};
pub use props::{compute_mesh_props, override_slot, MeshProps};
pub use skin::skin_vertices;
pub use trailer::{body_heading, body_reversed, coupling_offsets, coupling_placement, coupling_points, TrailerPart};
pub use vehicle_type::{load_paint_schemes, PaintScheme, SkinBone, VehicleMesh, VehicleType};
pub use visuals::{fit_plane, onto_plane, SHADOW_LIFT};
pub use weather::{relative_humidity, road_grip};

use physics_bridge::OmsiFrames;
#[cfg(test)]
use physics_bridge::scripts_acceleration;

pub struct VehicleInstance {
    /// `A_Trans_*` taken over OMSI's frames (see [`OmsiFrames`]).
    a_trans: OmsiFrames,
    pub ty: Arc<VehicleType>,
    pub state: State,
    pub vm: Vm,
    pub host: VehicleHost,
    /// World position of the vehicle origin and heading (degrees, clockwise from north).
    pub position: DVec3,
    pub heading: f64,
    pub pitch: f32,
    /// The driver's seat's travel and speed, and the body's turning last frame (see
    /// `update_driver_seat`).
    seat: (f32, f32, Vec3),
    pub bank: f32,
    pub animators: Vec<MeshAnimator>,
    /// Current mesh-space transforms, parallel to `ty.meshes`.
    pub mesh_transforms: Vec<Mat4>,
    pub mesh_props: Vec<MeshProps>,
    /// What moves the vehicle: its rigid body or the kinematic model.
    pub(crate) dynamics: Dynamics,
    /// The motion as the physics left it this frame (see `motion`).
    pub(crate) motion: VehicleMotion,
    /// The driver's inputs (see `set_controls`).
    controls: Controls,
    /// `[texttexture]` states, parallel to `ty.model.text_textures`.
    pub text_textures: Vec<crate::texttex::TextTextureState>,
    pub html_textures: Vec<crate::htmltex::HtmlTexture>,
    /// Ground height query (world x, y → z), set by the world.
    /// Ground height sampler (shared: the script host probes the same one).
    pub ground: Option<std::sync::Arc<dyn Fn(f64, f64) -> Option<f64> + Send + Sync>>,
    /// What the wheels stand on: the highest face at or below a height and the lowest one
    /// above it. Without it the wheels stand on `ground`.
    pub contact: Option<std::sync::Arc<dyn crate::rigid::Ground>>,
    /// Vehicles coupled behind this one.
    pub trailers: Vec<TrailerPart>,
    /// How many coupled parts the lead body's rest loads were worked out for (`step_bodies`).
    pub(crate) rest_loads_for: usize,
    /// The vehicle's `[smoke]` particle systems (see `crate::particles`).
    pub particles: ParticleSet,
    /// How far each `[light_enh_2]` of the model (in order) has come on: its fading variable
    /// followed with the light's `timeconst` (63 % of the way in that time when switched on,
    /// down to 27 % when switched off, as the stock files document it).
    pub light_fade: Vec<f32>,
    /// The meshes' transforms in the modelled pose, for `[smoothskin]` (made when needed).
    skin_rest: Vec<Mat4>,
    /// Static obstacles; the vehicle's `[boundingbox]` is kept out of them.
    pub collision: Option<Arc<crate::collision::CollisionWorld>>,
    /// Moving obstacles (AI vehicles) for this frame, set by the app.
    pub dynamic_boxes: Vec<crate::collision::Obb>,
    /// A collision happened this frame: the `{trigger:collision}` block runs after physics.
    collided: bool,
    /// Energy of the last crash (J), for whoever wants to report it; cleared by the reader.
    pub last_crash: f32,
    /// Faces the wheels cannot climb stop the vehicle (see `RigidBody::wheel_walls`); the
    /// player's bus follows the setting for collisions with objects.
    pub wheel_walls: bool,
    /// The springs as at full air whatever the scripts' `Axle_Springfactor_*` say (the
    /// launcher's vehicle viewer shows the bus on full springs, not knelt or leaning).
    pub springs_full: bool,
    /// What the radio plays, cut to what a text display shows (see `show_radio_text`):
    /// `None` leaves the scripts' own texts alone, an empty text is a radio that is off.
    pub radio_text: Option<String>,
    /// The frequency the station is on where the bus is (`94.6 MHz`), where it is known.
    pub radio_frequency: Option<String>,
    /// Crashes so far and the energy of the latest (J), kept for logs and the HUD.
    pub crashes: u32,
    pub last_impact: f32,
    /// Map ids of the `[crashmode_pole]` posts this vehicle has knocked over, and those
    /// knocked this frame with the direction they fall in (the scene lays them down).
    pub knocked: Vec<i64>,
    pub knocked_now: Vec<(i64, DVec3)>,
    /// Moving obstacles (AI cars, by their collision id) in contact last frame: one crash per
    /// contact, not one per frame of it.
    touching: Vec<i64>,
    /// Per axle, the static spring compression and how far the tyre radius exceeds the
    /// unloaded hub height (m): the simple model has no springs, but sits as low.
    rest_sag: Vec<(f32, f32)>,
    /// How far an AI copy has to stand higher for its modelled tyres to touch the road.
    ai_lift: f32,
    /// How dirty the body is (0..1). OMSI reports it to the model as `Dirt_Norm`, where
    /// it scales the alpha of the dirt overlay; the bus wash clears it.
    pub dirt: f32,
    /// The AI odometer at the last `update_ai` (the wheels roll the difference).
    ai_odometer: f32,
    /// Kilometres driven this session (the odometer's `kmcounter_*`).
    driven_km: f64,
    /// The odometer's start was set from `[kmcounter_init]` (see `update_engine_vars`).
    km_started: bool,
    /// The cabin air as the engine keeps it (°C, g/m³), and the value last written, so that
    /// a bus whose own scripts heat or cool the cabin keeps what they write.
    cabin_air: Option<(f32, f32, f32)>,
    /// An AI vehicle nobody can see (out of the view and away from the mirrors) leaves its
    /// mesh animations and material values as they are: the traffic clears this while it is
    /// out of sight, and the time it missed is made up at once when it comes back. The body,
    /// the coupled parts and the scripts still run every frame.
    pub ai_visuals: bool,
    ai_visuals_missed: f32,
    var_index: HashMap<String, omsi_script::VarId>,
    /// Where each mesh's material properties come from, resolved against `var_index`
    /// (rebuilt when the vehicle gains engine variables).
    props_plan: PropsPlan,
    // cached variable ids for the physics interface
    v_velocity: Option<omsi_script::VarId>,
    v_velocity_ground: Option<omsi_script::VarId>,
    v_n_wheel: Option<omsi_script::VarId>,
    v_m_wheel: Option<omsi_script::VarId>,
    v_throttle: Option<omsi_script::VarId>,
    v_brake: Option<omsi_script::VarId>,
    /// `Brakeforce`: the scripts' brake force for the whole vehicle (N), shared out over its
    /// wheels besides each wheel's own `Axle_Brakeforce_*`.
    v_brakeforce: Option<omsi_script::VarId>,
    v_clutch: Option<omsi_script::VarId>,
    /// `PAX_Entry<n>_Req` and `PAX_Exit<n>_Req` ([`PAX_DOORS`]), and their `_Busy`: set by
    /// the passengers every frame and cleared after the scripts' frame (see
    /// `clear_pax_requests`).
    v_pax_req: Vec<omsi_script::VarId>,
    v_accel: [Option<omsi_script::VarId>; 3],
    v_wheels: Vec<[[Option<omsi_script::VarId>; 5]; 2]>,
    v_springfactor: Vec<[Option<omsi_script::VarId>; 2]>,
}

impl VehicleInstance {
    /// The fleet number the scripts know as `number` (empty for a vehicle without one).
    pub fn number(&self) -> String {
        self.ty
            .program
            .str_var("number")
            .and_then(|i| self.state.str_vars.get(i as usize))
            .cloned()
            .unwrap_or_default()
    }

    pub fn new(ty: Arc<VehicleType>, host: VehicleHost) -> VehicleInstance {
        let program = ty.program.clone();
        let mut state = State::new(&program);
        let mut vm = Vm::new();
        let mut host = host;
        host.script_textures = ty
            .model
            .script_textures
            .iter()
            .map(|(w, h)| crate::scripttex::ScriptTexture::new(*w, *h))
            .collect();
        host.content_dir = ty.def.dir().to_path_buf();
        let html_textures: Vec<crate::htmltex::HtmlTexture> = ty
            .model
            .html_textures
            .iter()
            .map(|d| {
                let dirs = [ty.model_dir.as_path(), ty.def.dir()];
                let html = crate::htmltex::load_page(&dirs, &d.path);
                crate::htmltex::HtmlTexture::new(d.script_index, d.width, d.height, &html)
                    .with_asset_dirs(crate::htmltex::asset_dirs(&dirs, &d.path))
            })
            .collect();
        host.number_var = program.str_var("number");
        // The vehicle dialog has already chosen these. They must exist before {init}: many
        // mods branch on the fleet number to choose equipment, textures or script state.
        if let (Some(i), Some(number)) = (program.str_var("number"), host.initial_number.as_ref()) {
            state.str_vars[i as usize] = number.clone();
        }
        if let (Some(i), Some(ident)) = (program.str_var("ident"), host.initial_ident.as_ref()) {
            state.str_vars[i as usize] = ident.clone();
        }
        // defaults every bus expects before {init}
        let mut var_index = HashMap::new();
        for (i, n) in program.var_names.iter().enumerate() {
            var_index.insert(n.to_ascii_lowercase(), i as omsi_script::VarId);
        }
        // engine-provided defaults
        if let Some(i) = var_index.get("envir_brightness") {
            state.vars[*i as usize] = 1.0;
        }
        // no ticket printed yet
        if let Some(i) = var_index.get("giventicket") {
            state.vars[*i as usize] = -1.0;
        }
        // `wearlifespan` (varlist_roadvehicle.txt, written by the engine from the
        // difficulty settings): every random part lifetime the scripts draw at {init} is
        // multiplied by it - the rear-door automatic, bulbs, the matrix display. Left at 0
        // the SD200's rear door was worn out on its first closing and reopened by itself,
        // with the opening sound, from then on; the matrix "spinnt" for the same reason.
        if let Some(i) = var_index.get("wearlifespan") {
            state.vars[*i as usize] = host.wear_lifespan.max(0.01);
        }
        // These are engine-owned visual inputs, not script variables.  They still need
        // to exist before {init}: material/prop setup can read them while the scene is
        // created, which otherwise makes stock buses (notably GN92) start with the dirt
        // layer at full opacity until the first simulation tick repairs it.
        for (name, value) in [
            ("Dirt_Norm", 0.0),
            ("DirtRate", 0.0),
            ("PrecipRate", 0.0),
            ("StreetCond", 0.0),
        ] {
            let key = name.to_ascii_lowercase();
            let id = match var_index.get(&key).copied() {
                Some(id) => id,
                None => {
                    let id = state.vars.len() as omsi_script::VarId;
                    state.vars.push(value);
                    var_index.insert(key, id);
                    id
                }
            };
            state.vars[id as usize] = value;
        }
        // `Axle_Springfactor_*` (varlist_roadvehicle.txt): the engine's spring scale, which
        // air-suspension scripts overwrite with their bellows pressure; a steel-sprung bus
        // never writes it and must not ride on springs of zero
        for n in program
            .var_names
            .iter()
            .filter(|n| n.to_ascii_lowercase().starts_with("axle_springfactor_"))
        {
            if let Some(i) = var_index.get(&n.to_ascii_lowercase()) {
                state.vars[*i as usize] = 1.0;
            }
        }
        // `$.yard` is the depot the bus runs from, the [name] of its .hof, known before
        // {init}: the matrix displays build their line list and palette paths from it
        // (`Linienlisten\<yard>_ANX.jpg`, `palettes\<yard>.bmp`), the roller blind its
        // line list, and some IBIS scripts branch on it.
        if let (Some(i), Some(h)) = (program.str_var("yard"), host.hof.as_ref()) {
            state.str_vars[i as usize] = h.name.clone();
        }
        if let Some(scheme) = host.paint_scheme {
            let scheme = scheme.filter(|i| *i < ty.paint_schemes.len());
            let mut put = |name: &str, v: f32| {
                if let Some(id) = var_index.get(&name.to_ascii_lowercase()) {
                    state.vars[*id as usize] = v;
                }
            };
            put("Colorscheme", scheme.map(|i| i as f32).unwrap_or(-1.0));
            if let Some(i) = scheme {
                for (var, v) in &ty.paint_schemes[i].set_vars {
                    put(var, *v);
                }
            }
        }
        if let Some(i) = var_index.get("schedule_active") {
            state.vars[*i as usize] = host.schedule_active;
        }
        vm.run_init(&program, &mut state, &mut host);
        let mut animators: Vec<MeshAnimator> = ty
            .meshes
            .iter()
            .map(|m| MeshAnimator::new(&ty.model.meshes[m.def_index], m.pivot, |n| program.var(n)))
            .collect();
        crate::anim::link_parents(
            &mut animators,
            &ty.meshes
                .iter()
                .map(|m| &ty.model.meshes[m.def_index])
                .collect::<Vec<_>>(),
        );
        let n = ty.meshes.len();
        let motion = VehicleMotion::at_rest(&ty.def);
        let kinematic = KinematicModel::from_definition(&ty.def);
        let v = |name: &str| program.var(name);
        let v_wheels = (0..motion.wheels.len())
            .map(|a| {
                let side = |s: &str| {
                    [
                        v(&format!("Wheel_Rotation_{a}_{s}")),
                        v(&format!("Wheel_RotationSpeed_{a}_{s}")),
                        v(&format!("Axle_Steering_{a}_{s}")),
                        v(&format!("Axle_Suspension_{a}_{s}")),
                        v(&format!("Axle_Brakeforce_{a}_{s}")),
                    ]
                };
                [side("L"), side("R")]
            })
            .collect();
        let v_springfactor = (0..motion.wheels.len())
            .map(|a| {
                [
                    v(&format!("Axle_Springfactor_{a}_L")),
                    v(&format!("Axle_Springfactor_{a}_R")),
                ]
            })
            .collect();
        let rest_sag: Vec<(f32, f32)> = {
            let rb = crate::rigid::RigidBody::from_definition(&ty.def, &ty.hub_heights(0));
            (0..motion.wheels.len())
                .map(|a| {
                    let w = &rb.wheels[a * 2];
                    (
                        w.rest_compression().min(crate::rigid::BUMP),
                        w.radius - w.attach.z,
                    )
                })
                .collect()
        };
        // An AI copy has no rigid body: it stands on the contact plane of its wheels
        // (`ai_motion::AiBody`), and what the modelled tyre reaches below its hub less than
        // the model thinks lifts it. An axle the model does not move with `Axle_Suspension`
        // got the spring sag taken off its hub height (`VehicleType::hub_heights`), which an
        // AI copy without springs must not stand on.
        let ai_lift = {
            let offs: Vec<f32> = rest_sag
                .iter()
                .enumerate()
                .map(|(a, (comp, off))| {
                    if ty.suspension_axles.contains(&a) {
                        *off
                    } else {
                        off - comp
                    }
                })
                .collect();
            offs.iter().sum::<f32>() / offs.len().max(1) as f32
        };
        VehicleInstance {
            a_trans: OmsiFrames::default(),
            particles: ParticleSet::new(ty.model.particle_systems(), std::ptr::addr_of!(host) as u64 ^ 0x9e37_79b9),
            light_fade: Vec::new(),
            v_springfactor,
            rest_sag,
            ai_lift,
            contact: None,
            knocked: Vec::new(),
            knocked_now: Vec::new(),
            touching: Vec::new(),
            v_velocity: v("Velocity"),
            v_velocity_ground: v("Velocity_Ground"),
            v_n_wheel: v("n_Wheel"),
            v_m_wheel: v("M_Wheel"),
            v_throttle: v("Throttle").or_else(|| v("throttle_pedal")),
            v_brake: v("Brake").or_else(|| v("brake_pedal")),
            v_brakeforce: v("Brakeforce"),
            v_clutch: v("Clutch").or_else(|| v("clutch_pedal")),
            v_pax_req: (0..PAX_DOORS)
                .flat_map(|i| [format!("PAX_Entry{i}_Req"), format!("PAX_Exit{i}_Req"), format!("PAX_Entry{i}_Busy"), format!("PAX_Exit{i}_Busy")])
                .filter_map(|n| v(&n))
                .collect(),
            v_accel: [v("A_Trans_X"), v("A_Trans_Y"), v("A_Trans_Z")],
            v_wheels,
            ty,
            state,
            vm,
            host,
            position: DVec3::ZERO,
            heading: 0.0,
            pitch: 0.0,
            bank: 0.0,
            animators,
            mesh_transforms: vec![Mat4::IDENTITY; n],
            mesh_props: vec![MeshProps::default(); n],
            dynamics: Dynamics::Kinematic(kinematic),
            motion,
            controls: Controls::default(),
            text_textures: Vec::new(),
            html_textures,
            ground: None,
            trailers: Vec::new(),
            rest_loads_for: 0,
            skin_rest: Vec::new(),
            collision: None,
            dynamic_boxes: Vec::new(),
            collided: false,
            last_crash: 0.0,
            wheel_walls: true,
            springs_full: false,
            radio_text: None,
            radio_frequency: None,
            crashes: 0,
            last_impact: 0.0,
            dirt: 0.0,
            ai_odometer: 0.0,
            seat: (0.0, 0.0, Vec3::ZERO),
            driven_km: 0.0,
            km_started: false,
            cabin_air: None,
            ai_visuals: true,
            ai_visuals_missed: 0.0,
            var_index,
            props_plan: PropsPlan::default(),
        }
    }

    /// Restore a situation without running scripts against an incomplete host. Nothing is
    /// ticked here: the caller attaches the timetable callbacks (`PlayerDuty::resume`) and
    /// then runs the frames, the first of which rebuilds the displays from the restored
    /// state. Returns how many numeric and string variables the program knew.
    pub fn restore_script_state(
        &mut self,
        vars: &[(String, f32)],
        strings: &[(String, String)],
    ) -> (usize, usize) {
        let mut numeric = 0;
        let mut textual = 0;
        for (name, value) in vars {
            numeric += usize::from(self.set_var(name, *value));
            if name.eq_ignore_ascii_case("Dirt_Norm") {
                self.dirt = value.clamp(0.0, 1.0);
            }
        }
        for (name, value) in strings {
            if let Some(i) = self.ty.program.str_var(name) {
                self.state.str_vars[i as usize] = value.clone();
                textual += 1;
            }
        }
        // Text images are per instance. An unchanged string still needs an upload when
        // a snapshot is applied to a vehicle whose previous images were already synced.
        for t in &mut self.text_textures {
            t.last_text = None;
        }
        for part in &mut self.trailers {
            for t in &mut part.text_textures {
                t.last_text = None;
            }
        }
        // The stock bitmap matrices - Script/Matrix.osc of the MAN_SD200/SD202 and
        // Matrix_D.osc / VMatrix*.osc of the MAN_NL_NG (EN92, GN92) - redraw their
        // script texture only when the IBIS line or terminus differs from
        // Matrix_Nr_Last / Matrix_TerminusIndex_Last. The texture is not in an .osn, so
        // the saved "last" values would leave this fresh instance's matrix blank: they
        // are reset to force one redraw. Text/roller displays keep all their saved
        // state; no destination or power trigger is fired here.
        if !self.host.script_textures.is_empty()
            && self.ty.program.macro_block("Matrix_frame").is_some()
        {
            self.set_var("Matrix_Nr_Last", -1.0);
            self.set_var("Matrix_TerminusIndex_Last", -1.0);
        }
        self.update_visuals(0.0);
        (numeric, textual)
    }

    /// `Driver_Seat_VertTransl`: the sprung driver's seat, as OMSI moves it:
    /// the body's vertical acceleration and its roll and pitch at the seat's place (the
    /// driver's camera) push a spring (3000) and damper (2000) on 150 kg; ±10 cm at most.
    fn update_driver_seat(&mut self, dt: f32) {
        let Some(id) = self.ty.program.var("Driver_Seat_VertTransl") else { return };
        if dt <= 0.0 {
            return;
        }
        let seat = self.ty.def.cameras_driver.first().map(|c| c.pos).unwrap_or([0.0; 3]);
        let (acc_z, omega) = match self.rigid() {
            Some(rb) => (rb.accel_body.z - 9.81, rb.omega),
            None => (0.0, Vec3::ZERO),
        };
        let d_omega = omega - self.seat.2;
        self.seat.2 = omega;
        // (the rolls' and pitches' change moves the seat by its lever: y forward rolls
        // about x, x across pitches about y)
        let push = -acc_z * dt + d_omega.y * seat[0] - d_omega.x * seat[1];
        let k = (1000.0 / (15.0 * dt * 1000.0)).min(1.0);
        let (mut x, mut v) = (self.seat.0, self.seat.1);
        v += push + ((-x * 3000.0 - v * 2000.0) / 150.0) * k * dt;
        x += v * dt;
        if x.abs() > 0.1 {
            x = x.clamp(-0.1, 0.1);
            v = 0.0;
        }
        self.seat = (x, v, omega);
        self.state.vars[id as usize] = x;
    }

    /// Run one simulation frame: physics, scripts, then animations.
    pub fn update(&mut self, dt: f32) {
        self.host.clock.advance(dt);
        self.step_physics(dt);
        self.update_ground_probe();
        if std::mem::take(&mut self.collided) {
            // OMSI runs the vehicle's `collision` block on a crash (it damages the bus); the
            // energy is that crash's, for as many reads as the block makes
            self.trigger("collision");
            self.host.coll_energy = 0.0;
        }
        self.update_dirt(dt);
        // (signed, as OMSI adds it: reversing takes it back)
        self.driven_km += (self.motion.velocity_kmh() as f64 / 3600.0) * dt as f64;
        self.update_engine_vars(dt);
        let p = self.ty.program.clone();
        self.vm.run_frame(&p, &mut self.state, &mut self.host);
        self.show_radio_text();
        self.clear_pax_requests();
        self.update_visuals(dt);
    }

    /// The passengers' door requests are pulses: OMSI clears all eight of each kind
    /// (here all [`PAX_DOORS`]) after the vehicle's scripts ran and the
    /// passengers set them again every frame. Kept, a timetable bus that drove out of the
    /// passengers' reach kept its last request, and its automatic door never shut.
    fn clear_pax_requests(&mut self) {
        for &id in &self.v_pax_req {
            self.state.vars[id as usize] = 0.0;
        }
    }

    /// Run the scripts' frame once with no time passing (no physics, no clock): what a
    /// switch just pressed makes of the variables the scripts derive from it.
    pub fn update_scripts_only(&mut self, _dt: f32) {
        let p = self.ty.program.clone();
        let gap = self.host.clock.timegap;
        self.host.clock.timegap = 0.0;
        self.vm.run_frame(&p, &mut self.state, &mut self.host);
        self.host.clock.timegap = gap;
    }

    /// Let the scripts measure the ground under a point of the vehicle
    /// (`GetHeightAbovePoint`): the probe captures this frame's pose and ground sampler.
    /// The answer is how high the point stands *over* the ground, positive with room below:
    /// the Solaris' kneeling sensor stops lowering the body "as soon as it touches the
    /// kerb" when it reads under 5 cm and lets the level control vent the right-hand
    /// bellows only above that, and the NL/NG lift may drop by what it reads (up to
    /// 0.3 m). Read the other way round (ground minus point) the sensor sat at 0, the
    /// right-hand bellows of an Urbino never let their air out and the bus leant 1.7° to
    /// the left all the way.
    fn update_ground_probe(&mut self) {
        if self.ground.is_none() {
            return;
        }
        let rot = self.body_rotation();
        let pos = self.position;
        let g = self.ground.clone().unwrap();
        self.host.ground_probe = Some(std::sync::Arc::new(move |x, y, z| {
            let w = pos + rot.transform_point3(glam::Vec3::new(x, y, z)).as_dvec3();
            match g(w.x, w.y) {
                Some(h) => (w.z - h) as f32,
                None => 0.0,
            }
        }));
    }

    /// Has the bus's script finished leaving its stop (doors shut, stop brake off)? True
    /// for vehicles whose scripts do not take part in that handshake.
    pub fn station_released(&self) -> bool {
        self.var("AI_Scheduled_AtStation")
            .map(|v| v > -0.5)
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests;
