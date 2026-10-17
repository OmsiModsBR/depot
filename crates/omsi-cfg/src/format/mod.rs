//! OMSI's keyword files: a file read into lines, the keyword lines in it and the reader
//! that walks blocks of parameters the way the original's loaders do.

mod file;
mod keyword;
mod keywords;
mod reader;

pub use file::{CfgError, CfgFile};
pub use keyword::{keyword_of, keyword_with, KeywordRule};
pub use reader::{CfgReader, Entry};
