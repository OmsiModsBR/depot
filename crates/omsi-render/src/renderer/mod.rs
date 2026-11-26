//! The renderer: its state, opening it, building everything it draws with, and the frame:
//! preparing it and recording its passes, ending by handing it to an `output` destination.

pub mod assets;
pub mod build;
pub mod camera_uniform;
pub mod encode;
pub mod features;
pub mod frame;
pub mod lights;
pub mod lists;
pub mod mirror;
pub mod open;
pub mod options;
pub mod output;
pub mod passes;
pub mod prepare;
pub mod reflections;
pub mod sky;
pub mod state;
pub mod targets;
pub mod upload;

#[cfg(feature = "test-support")]
pub use build::mode_for;
#[cfg(any(test, feature = "test-support"))]
pub use build::sky_own_text;
#[cfg(feature = "test-support")]
pub use build::{corona_own_text, snow_own_text};
#[cfg(feature = "test-support")]
pub use build::{scene_own_text, scene_shader_for};
#[cfg(feature = "test-support")]
pub use features::{BuiltPasses, FrameInputs, FrameSwitches, ViewKind};
pub use mirror::{AssetMirror, MirrorEvent, MirrorTexture};
pub use options::{RenderOptions, AUTO_SCALE_PIXELS};
pub use state::{OverlayHook, Renderer, LAMP_CODE_STRIDE, MAX_LAMPS_PER_MESH, SHADOW_SIZE};

pub use mirror::MirrorSlot;
pub use options::scene_scale_for;
