//! The content OMSI reads: the content roots searched in order of priority, the archives
//! mounted among them, and the way a content file's path is found the way Windows finds it.

mod listing;
mod packages;
mod resolve;
mod roots;
pub mod vfs;

pub use listing::{content_changed, content_generation, windows_components};
pub use packages::missing_vehicle_pack;
pub(crate) use resolve::resolve_existing;
pub use resolve::{content_dirs, read_dir_merged, resolve_path};
pub use roots::{
    add_content_root, add_content_root_before, add_content_root_first, content_roots,
    find_in_roots, is_in_content_roots, is_sandbox, mark_sandbox, mirrored_dirs,
    remove_content_root,
};
