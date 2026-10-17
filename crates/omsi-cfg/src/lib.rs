//! OMSI's text formats.
//!
//! Almost every OMSI content file (`.bus`, `.sco`, `.sli`, `.cfg`, `.hof`, `.map`, …) is a list
//! of lines in which a *keyword* line such as `[mesh]` is followed by a fixed number of parameter
//! lines.  Everything else is free text and ignored.  The rules, read off the loaders of
//! OMSI 2 (every one compares the line it read with its keyword literals by plain
//! string equality) and checked against the stock and installed content, are:
//!
//! * a keyword is recognised only when the whole line is `[name]`: an indented keyword
//!   (the stock files' help texts, the F90 lorry's disabled second rear axle, the EN92's
//!   disabled fourth door-slam sound) is free text, and so is one followed by spaces;
//! * the name is spelled as the original spells it (`[matl_noZwrite]`, not `[matl_nozWrite]`);
//!   names the original does not know are compared without regard to case;
//! * two loaders differ: `.hof` cuts trailing tabs, spaces and quotes off every line
//!   (spreadsheet exports), `ailists.cfg` lower-cases its lines ([`KeywordRule`]);
//! * the main content loaders skip `-<DISABLED>-` … `-<ENABLED>-` between blocks
//!   ([`CfgReader::disabled_blocks`]);
//! * parameters are read verbatim, one per line, empty lines included.
//!
//! Text is read in the code page it was written in: a byte-order mark says so, otherwise
//! [`codepage::detect`] tells UTF-8, Windows-1251 (Russian mods), 1250 (Polish, Czech) and
//! the stock content's Windows-1252 apart.
//!
//! The crate is organised by domain:
//!
//! * [`text`]: code pages, lines and numbers;
//! * `format`: keyword files and their reader ([`CfgFile`], [`CfgReader`], [`keyword_of`]);
//! * `content`: the content roots, the archives mounted among them ([`vfs`]) and the
//!   lookup of a content file's path ([`resolve_path`]);
//! * `install`: the original OMSI 2 installation, openOMSI's content folder and the user's
//!   data folder ([`paths`]);
//! * `fs`: [`write_atomic`];
//! * [`env`](mod@env), [`flags`] and [`limits`]: the environment, the `OMSI_*` variables and the
//!   ceilings of the parsers.

mod content;
pub mod env;
pub mod flags;
mod format;
mod fs;
mod install;
pub mod limits;
pub mod text;

pub use content::vfs::{self, add_content_zip, mount_zip};
pub use content::{
    add_content_root, add_content_root_before, add_content_root_first, content_changed,
    content_dirs, content_generation, content_roots, find_in_roots, is_in_content_roots,
    is_sandbox, mark_sandbox, mirrored_dirs, missing_vehicle_pack, read_dir_merged,
    remove_content_root, resolve_path, windows_components,
};
pub use format::{keyword_of, keyword_with, CfgError, CfgFile, CfgReader, Entry, KeywordRule};
pub use fs::write_atomic;
pub use install::paths;
pub use install::search::{self as install_search, find_original_install};
pub use install::{
    content_folder_of, drop_unusable_home, ensure_content_layout, is_programs_folder, is_writable,
    migrate_legacy_data_dir, missing_original_essentials, original_keyboard_cfg, CONTENT_FOLDERS,
    CONTENT_MARKER, LEGACY_CONTENT_MARKER, ORIGINAL_ESSENTIALS,
};
pub use text::number::{parse_f32, parse_f64, parse_i32, parse_i64};
pub use text::{codepage, decode_text, encode_text, encode_utf16_bom, number, split_lines};
