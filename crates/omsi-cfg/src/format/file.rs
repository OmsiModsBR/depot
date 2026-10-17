//! A text file loaded and split into lines.

use std::path::{Path, PathBuf};

use crate::content::vfs;
use crate::text::{decode_text, split_lines};

#[derive(Debug, thiserror::Error)]
pub enum CfgError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// A loaded text file, split into lines.
#[derive(Debug, Clone)]
pub struct CfgFile {
    pub path: PathBuf,
    pub lines: Vec<String>,
}

impl CfgFile {
    pub fn read(path: impl AsRef<Path>) -> Result<Self, CfgError> {
        let path = path.as_ref();
        let bytes = vfs::read(path).map_err(|e| CfgError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        Ok(Self::from_bytes(path, &bytes))
    }

    pub fn from_bytes(path: impl AsRef<Path>, bytes: &[u8]) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            lines: split_lines(&decode_text(bytes)),
        }
    }

    pub fn from_str(path: impl AsRef<Path>, text: &str) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            lines: split_lines(text),
        }
    }

    /// Directory the file lives in (for resolving relative paths in it).
    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16() {
        let mut b = vec![0xFF, 0xFE];
        for c in "[name]\r\nGrundorf".encode_utf16() {
            b.extend_from_slice(&c.to_le_bytes());
        }
        let f = CfgFile::from_bytes("g.cfg", &b);
        assert_eq!(f.lines, vec!["[name]", "Grundorf"]);
    }
}
