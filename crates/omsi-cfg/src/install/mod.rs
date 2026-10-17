//! Where things are installed: the original OMSI 2, openOMSI's content folder and the
//! user's data folder.

mod home;
mod layout;
mod original;
pub mod paths;
pub mod search;

pub use home::{drop_unusable_home, migrate_legacy_data_dir};
pub use layout::{
    content_folder_of, ensure_content_layout, is_programs_folder, is_writable, CONTENT_FOLDERS,
    CONTENT_MARKER, LEGACY_CONTENT_MARKER,
};
pub use original::{missing_original_essentials, original_keyboard_cfg, ORIGINAL_ESSENTIALS};
