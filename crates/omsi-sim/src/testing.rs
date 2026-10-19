//! What the tests in `tests/` reach of the crate's insides, built only with the
//! `test-support` feature (the crate's own dev-dependency turns it on; the game builds
//! without it).
//!
//! One module per module of the crate, each re-exporting what that module has public, plus
//! the items the tests need that the crate keeps to itself.

pub mod ai {
    pub use crate::ai::*;

    /// The body's state as a test reads it.
    pub trait AiBodyInternals {
        fn yaw_rate(&self) -> f32;
        fn rear(&self) -> glam::DVec2;
        fn wheels(&self) -> &[crate::physics::AiWheelSpec];
        fn contact_z(&self) -> &[f64];
    }

    impl AiBodyInternals for AiBody {
        fn yaw_rate(&self) -> f32 {
            self.yaw_rate
        }
        fn rear(&self) -> glam::DVec2 {
            self.rear
        }
        fn wheels(&self) -> &[crate::physics::AiWheelSpec] {
            &self.wheels
        }
        fn contact_z(&self) -> &[f64] {
            &self.contact_z
        }
    }
}
pub mod anim {
    pub use crate::anim::*;
}
pub mod clock {
    pub use crate::clock::*;
}
pub mod collision {
    pub use crate::collision::*;

    pub const PART_CELL: f64 = crate::collision::PART_CELL;

    /// A mesh's grid of parts as a test reads it.
    pub trait MeshShapeInternals {
        fn near(&self, c: glam::DVec2, r: f64) -> Vec<u32>;
        fn grid(&self) -> &hashbrown::HashMap<(i32, i32), Vec<u32>>;
    }

    impl MeshShapeInternals for MeshShape {
        fn near(&self, c: glam::DVec2, r: f64) -> Vec<u32> {
            MeshShape::near(self, c, r)
        }
        fn grid(&self) -> &hashbrown::HashMap<(i32, i32), Vec<u32>> {
            &self.grid
        }
    }
}
pub mod crowd {
    pub use crate::crowd::*;
}
pub mod daylight {
    pub use crate::daylight::*;

    pub fn default_envir_values() -> omsi_content::Envir {
        crate::daylight::default_envir_values()
    }
}
pub mod host {
    pub use crate::host::*;

    pub fn arg_i32(v: f32) -> i32 {
        crate::host::arg_i32(v)
    }
    pub fn arg_idx(v: f32) -> usize {
        crate::host::arg_idx(v)
    }
}
pub mod human {
    pub use crate::human::*;

    use glam::{DVec3, Vec2};

    pub const SIDE: [f32; 2] = crate::human::SIDE;

    pub fn bone_id_by_name(name: &str) -> Option<i32> {
        crate::human::bone_id_by_name(name)
    }

    /// How a rig is measured from a human's files.
    pub trait RigInternals {
        fn measure(def: &omsi_content::Human, j: &Joints, meshes: &[HumanMesh]) -> Rig;
    }

    impl RigInternals for Rig {
        fn measure(def: &omsi_content::Human, j: &Joints, meshes: &[HumanMesh]) -> Rig {
            Rig::measure(def, j, meshes)
        }
    }

    /// A foot as a test reads it: where it stands (floor frame), its heading (degrees)
    /// and whether it is planted.
    #[derive(Debug, Clone, Copy)]
    pub struct Foot {
        pub pos: DVec3,
        pub yaw: f64,
        pub planted: bool,
    }

    /// A pose's state as a test reads it.
    pub trait PoseInternals {
        fn feet(&self) -> [Foot; 2];
        fn head(&self) -> Vec2;
        fn drop(&self) -> f32;
        fn shuffles(&self) -> u32;
        fn rest_target(
            &self,
            rig: &Rig,
            side: usize,
            origin: DVec3,
            heading: f64,
            walking: bool,
        ) -> (DVec3, f64);
    }

    impl PoseInternals for Pose {
        fn feet(&self) -> [Foot; 2] {
            self.feet.map(|f| Foot {
                pos: f.pos,
                yaw: f.yaw,
                planted: f.planted,
            })
        }
        fn head(&self) -> Vec2 {
            self.head
        }
        fn drop(&self) -> f32 {
            self.drop
        }
        fn shuffles(&self) -> u32 {
            self.shuffles
        }
        fn rest_target(
            &self,
            rig: &Rig,
            side: usize,
            origin: DVec3,
            heading: f64,
            walking: bool,
        ) -> (DVec3, f64) {
            Pose::rest_target(self, rig, side, origin, heading, walking)
        }
    }
}
pub mod human_omsi {
    pub use crate::human_omsi::*;

    pub const THIGH_CURVE: [(f32, f32); 4] = crate::human_omsi::THIGH_CURVE;
    pub const KNEE_CURVE: [(f32, f32); 4] = crate::human_omsi::KNEE_CURVE;

    pub fn curve(c: &[(f32, f32)], x: f32) -> f32 {
        crate::human_omsi::curve(c, x)
    }
}
pub mod ibis {
    pub use crate::ibis::*;

    pub fn layouts(t: &Target, room: usize) -> Vec<String> {
        crate::ibis::layouts(t, room)
    }
}
pub mod input {
    pub use crate::input::*;
}
pub mod particles {
    pub use crate::particles::*;
}
pub mod physics {
    pub use crate::physics::*;

    /// A body's insides as a test reads them.
    pub trait RigidBodyInternals {
        fn inv_mass_at(&self, r: glam::Vec3, n: glam::Vec3) -> f32;
        fn wheel_axle(&self) -> &[usize];
        fn rot_pnt_long(&self) -> f32;
    }

    impl RigidBodyInternals for RigidBody {
        fn inv_mass_at(&self, r: glam::Vec3, n: glam::Vec3) -> f32 {
            RigidBody::inv_mass_at(self, r, n)
        }
        fn wheel_axle(&self) -> &[usize] {
            &self.wheel_axle
        }
        fn rot_pnt_long(&self) -> f32 {
            self.rot_pnt_long
        }
    }

    pub mod domain {
        pub use crate::physics::domain::*;

        pub mod params {
            pub use crate::physics::domain::params::*;
        }
    }
    pub mod implementation {
        pub use crate::physics::implementation::*;

        pub mod body {
            pub use crate::physics::implementation::body::*;
        }
        pub mod ground {
            pub use crate::physics::implementation::ground::*;
        }
        pub mod spec {
            pub use crate::physics::implementation::spec::*;
        }
        pub mod train {
            pub use crate::physics::implementation::train::*;
        }
        pub mod tyre {
            pub use crate::physics::implementation::tyre::*;
        }
    }
}
pub mod scenery {
    pub use crate::scenery::*;
}
pub mod scripttex {
    pub use crate::scripttex::*;
}
pub mod startup {
    pub use crate::startup::*;

    use crate::VehicleInstance;

    pub const PRESS: f32 = crate::startup::PRESS;
    pub const MAX_CRANK: f32 = crate::startup::MAX_CRANK;

    pub fn starter_engaged(v: &VehicleInstance) -> bool {
        crate::startup::starter_engaged(v)
    }
    pub fn engine_caught(v: &VehicleInstance) -> bool {
        crate::startup::engine_caught(v)
    }

    /// The start-up's state as a test reads it.
    pub trait StartUpInternals {
        /// Whether it is checking that the engine runs, the last step.
        fn verifying(&self) -> bool;
    }

    impl StartUpInternals for StartUp {
        fn verifying(&self) -> bool {
            self.step == crate::startup::Step::Verify
        }
    }
}
pub mod texttex {
    pub use crate::texttex::*;

    use omsi_content::font::Font;

    pub fn family_of(name: &str) -> String {
        crate::texttex::family_of(name)
    }

    /// What a test sets on a font library.
    pub trait FontLibraryInternals {
        /// Takes these fonts as the library's index, as if read from the fonts folder.
        fn set_index(&mut self, fonts: Vec<Font>);
    }

    impl FontLibraryInternals for FontLibrary {
        fn set_index(&mut self, fonts: Vec<Font>) {
            self.index = Some(fonts);
        }
    }
}
pub mod traffic {
    pub use crate::traffic::*;

    /// A driver's choice of the way on, as a test calls it.
    pub trait AiStateInternals {
        fn choose_after(&mut self, net: &Network, lane: usize) -> Option<usize>;
    }

    impl AiStateInternals for AiState {
        fn choose_after(&mut self, net: &Network, lane: usize) -> Option<usize> {
            AiState::choose_after(self, net, lane)
        }
    }
}
pub mod vehicle {
    pub use crate::vehicle::*;

    use glam::{DVec3, Mat4};

    /// A vehicle's steps and state as a test calls and reads them.
    pub trait VehicleInstanceInternals {
        fn step_physics(&mut self, dt: f32);
        fn update_ground_probe(&mut self);
        fn update_visuals(&mut self, dt: f32);
        fn update_trailers(&mut self, dt: f32);
        fn update_engine_vars(&mut self, dt: f32);
        fn var_index(&self) -> &hashbrown::HashMap<String, omsi_script::VarId>;
        fn driven_km(&self) -> f64;
        fn set_driven_km(&mut self, km: f64);
    }

    impl VehicleInstanceInternals for VehicleInstance {
        fn step_physics(&mut self, dt: f32) {
            VehicleInstance::step_physics(self, dt)
        }
        fn update_ground_probe(&mut self) {
            VehicleInstance::update_ground_probe(self)
        }
        fn update_visuals(&mut self, dt: f32) {
            VehicleInstance::update_visuals(self, dt)
        }
        fn update_trailers(&mut self, dt: f32) {
            VehicleInstance::update_trailers(self, dt)
        }
        fn update_engine_vars(&mut self, dt: f32) {
            VehicleInstance::update_engine_vars(self, dt)
        }
        fn var_index(&self) -> &hashbrown::HashMap<String, omsi_script::VarId> {
            &self.var_index
        }
        fn driven_km(&self) -> f64 {
            self.driven_km
        }
        fn set_driven_km(&mut self, km: f64) {
            self.driven_km = km;
        }
    }

    /// A coupled part's step and state as a test calls and reads them.
    pub trait TrailerPartInternals {
        fn update(&mut self, main: &mut VehicleInstance, dt: f32, lead: Option<(DVec3, Mat4, f64)>);
        fn axle_long(&self) -> f32;
        fn first_axle(&self) -> usize;
        fn pitch(&self) -> f32;
        fn set_pitch(&mut self, pitch: f32);
        fn set_bank(&mut self, bank: f32);
        fn axle_z(&self) -> Option<f64>;
        fn set_axle_z(&mut self, z: Option<f64>);
        fn track(&self) -> Option<DVec3>;
    }

    impl TrailerPartInternals for TrailerPart {
        fn update(
            &mut self,
            main: &mut VehicleInstance,
            dt: f32,
            lead: Option<(DVec3, Mat4, f64)>,
        ) {
            TrailerPart::update(self, main, dt, lead)
        }
        fn axle_long(&self) -> f32 {
            self.axle_long
        }
        fn first_axle(&self) -> usize {
            self.first_axle
        }
        fn pitch(&self) -> f32 {
            self.pitch
        }
        fn set_pitch(&mut self, pitch: f32) {
            self.pitch = pitch;
        }
        fn set_bank(&mut self, bank: f32) {
            self.bank = bank;
        }
        fn axle_z(&self) -> Option<f64> {
            self.axle_z
        }
        fn set_axle_z(&mut self, z: Option<f64>) {
            self.axle_z = z;
        }
        fn track(&self) -> Option<DVec3> {
            self.track
        }
    }

    pub mod articulation {
        pub use crate::vehicle::articulation::*;
    }
    pub mod physics_bridge {
        pub use crate::vehicle::physics_bridge::*;
    }
    pub mod props {
        pub use crate::vehicle::props::*;
    }
    pub mod radio {
        pub use crate::vehicle::radio::*;
    }
    pub mod trailer {
        pub use crate::vehicle::trailer::*;
    }
    pub mod vehicle_type {
        pub use crate::vehicle::vehicle_type::*;
    }
    pub mod visuals {
        pub use crate::vehicle::visuals::*;
    }
    pub mod weather {
        pub use crate::vehicle::weather::*;
    }
    pub mod winding {
        pub use crate::vehicle::winding::*;
    }
}
pub mod vehicle_api {
    pub use crate::vehicle_api::*;
}
