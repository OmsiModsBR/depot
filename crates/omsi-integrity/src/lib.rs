//! Integrity check of a map (later vehicles too), run on the user's request.
//!
//! It follows what the map names - `global.cfg` -> tiles -> objects and splines -> models and
//! meshes -> textures - and reports what is missing, unreadable or wasteful. It only reads
//! what the files declare; nothing here is taken from any other checker.
//!
//! Three kinds of finding, in the order a player should care:
//! * [`Severity::Error`]: something the map needs is missing, so the game leaves a piece out.
//! * [`Severity::Warning`]: the map works but may not on every system (a path too long for
//!   Windows, a tile listed but absent, a texture that is missing and shows white).
//! * [`Severity::NotOptimized`]: it works as it is but costs more than it should; the
//!   finding says why (a texture that is not a power of two is enlarged by DirectX 9).

mod image_size;
mod map;
mod report;

pub use image_size::{image_size, next_pow2};
pub use map::check_map;
pub use report::{Finding, Report, Severity};
