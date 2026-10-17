//! Writing files so that a reader never sees half of one.

use std::path::{Path, PathBuf};

/// Write `bytes` to `path` through a temporary file beside it (`<name>.tmp`) that is then
/// renamed over `path`: a crash or a full disk leaves the old file or the new one, never a
/// cut one. The temporary file is removed when the write fails. The folder must exist.
pub fn write_atomic(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    let path = path.as_ref();
    let tmp = tmp_path(path);
    let result = std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_and_leaves_no_temporary() {
        let dir =
            std::env::temp_dir().join(format!("omsi-cfg-write-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.cfg");
        write_atomic(&p, "a=1\n").unwrap();
        write_atomic(&p, "a=2\n").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "a=2\n");
        assert!(!dir.join("settings.cfg.tmp").exists());
        // no folder: an error, and nothing left behind
        let q = dir.join("missing").join("x.json");
        assert!(write_atomic(&q, b"{}").is_err());
        assert!(!dir.join("missing").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
