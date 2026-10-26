//! What is installed: the OMSI 2 folder checks, the .NET 6 runtimes and OmsiLaunch itself.
//! Everything here only reads; nothing is executed and no file is written.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// How the files of the installed OmsiLaunch compare with its own `release-manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "files", rename_all = "snake_case")]
pub enum Integrity {
    /// Every plugin file listed in the manifest is present with the listed hash.
    Intact,
    /// The package has no manifest beside the program (presence is all that can be said).
    NoManifest,
    /// The manifest cannot be read.
    Unreadable,
    /// These files are missing or differ from the manifest.
    Damaged(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OmsiLaunchStatus {
    pub installed: bool,
    pub exe: Option<PathBuf>,
    pub windowed_exe: Option<PathBuf>,
    /// `product_version` of the manifest, when there is one.
    pub version: Option<String>,
    pub control_protocol: Option<String>,
    pub plugin_present: bool,
    pub integrity: Integrity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderChecks {
    /// The folder is not marked read-only (a real write test is the plan's job).
    pub writable: bool,
    /// Under `Program Files`, where a normal user cannot write.
    pub in_program_files: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DotnetStatus {
    pub x64: bool,
    pub x86: bool,
}

/// Everything Setup and Diagnostics show about the original engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineStatus {
    pub root: Option<PathBuf>,
    pub folder: Option<FolderChecks>,
    pub dotnet: DotnetStatus,
    pub omsilaunch: OmsiLaunchStatus,
}

impl EngineStatus {
    /// Ready to be installed or used: a folder was found, it is not under Program Files and
    /// both runtimes are there.
    pub fn requirements_met(&self) -> bool {
        self.root.is_some()
            && self
                .folder
                .as_ref()
                .map(|f| !f.in_program_files)
                .unwrap_or(false)
            && self.dotnet.x64
            && self.dotnet.x86
    }
}

pub fn in_program_files(p: &Path) -> bool {
    // split on both separators so a Windows path is read the same on every platform
    p.to_string_lossy().split(['\\', '/']).any(|c| {
        let c = c.to_ascii_lowercase();
        c == "program files" || c == "program files (x86)"
    })
}

pub fn folder_checks(root: &Path) -> FolderChecks {
    FolderChecks {
        writable: std::fs::metadata(root)
            .map(|m| !m.permissions().readonly())
            .unwrap_or(false),
        in_program_files: in_program_files(root),
    }
}

fn has_net6(base: &Path, shared: &str) -> bool {
    std::fs::read_dir(base.join("dotnet").join("shared").join(shared))
        .map(|rd| {
            rd.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with("6.")
                    && n[2..]
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// .NET 6 runtimes under the two program folders: the x64 controller needs the Desktop
/// runtime, the x86 plugin inside OMSI 2 needs the base runtime (the x86 Desktop bundle has it).
pub fn dotnet_in(program_files_x64: &Path, program_files_x86: &Path) -> DotnetStatus {
    DotnetStatus {
        x64: has_net6(program_files_x64, "Microsoft.WindowsDesktop.App"),
        x86: has_net6(program_files_x86, "Microsoft.NETCore.App"),
    }
}

/// The real machine's program folders (empty paths elsewhere, so nothing is found).
pub fn dotnet_here() -> DotnetStatus {
    let x64 = std::env::var_os("ProgramW6432")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .map(PathBuf::from)
        .unwrap_or_default();
    let x86 = std::env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_default();
    if x64.as_os_str().is_empty() && x86.as_os_str().is_empty() {
        return DotnetStatus {
            x64: false,
            x86: false,
        };
    }
    dotnet_in(&x64, &x86)
}

fn file_sha256(p: &Path) -> Option<String> {
    let mut f = std::fs::File::open(p).ok()?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h).ok()?;
    Some(format!("{:X}", h.finalize()))
}

/// Reads the installed OmsiLaunch from the OMSI 2 folder without running it.
pub fn omsilaunch_in(root: &Path) -> OmsiLaunchStatus {
    let exe = root.join("OmsiLaunch.exe");
    let w = root.join("OmsiLaunchW.exe");
    let installed = exe.is_file() && w.is_file();
    let plugin_present = root.join("plugins").join("OmsiLaunch.Plugin.opl").is_file();
    let mut st = OmsiLaunchStatus {
        installed,
        exe: exe.is_file().then_some(exe),
        windowed_exe: w.is_file().then_some(w),
        version: None,
        control_protocol: None,
        plugin_present,
        integrity: Integrity::NoManifest,
    };
    let mpath = root.join("release-manifest.json");
    let Ok(text) = std::fs::read_to_string(&mpath) else {
        return st;
    };
    let Ok(m) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) else {
        st.integrity = Integrity::Unreadable;
        return st;
    };
    st.version = m
        .get("product_version")
        .and_then(Value::as_str)
        .map(str::to_string);
    st.control_protocol = m
        .get("control_protocol")
        .and_then(Value::as_str)
        .map(str::to_string);
    let Some(files) = m.get("files").and_then(Value::as_array) else {
        st.integrity = Integrity::Unreadable;
        return st;
    };
    let mut bad = Vec::new();
    for f in files {
        let (Some(path), Some(want)) = (
            f.get("path").and_then(Value::as_str),
            f.get("sha256").and_then(Value::as_str),
        ) else {
            continue;
        };
        // only the permanent plugin closure matters at run time (like OmsiLaunch itself)
        if !path.starts_with("plugins/") || path.contains("..") {
            continue;
        }
        match file_sha256(&root.join(path)) {
            Some(got) if got.eq_ignore_ascii_case(want) => {}
            _ => bad.push(path.to_string()),
        }
    }
    st.integrity = if bad.is_empty() {
        Integrity::Intact
    } else {
        Integrity::Damaged(bad)
    };
    st
}

/// The full picture for a (possibly absent) OMSI 2 folder.
pub fn status_for(root: Option<&Path>, dotnet: DotnetStatus) -> EngineStatus {
    EngineStatus {
        root: root.map(Path::to_path_buf),
        folder: root.map(folder_checks),
        dotnet,
        omsilaunch: match root {
            Some(r) => omsilaunch_in(r),
            None => OmsiLaunchStatus {
                installed: false,
                exe: None,
                windowed_exe: None,
                version: None,
                control_protocol: None,
                plugin_present: false,
                integrity: Integrity::NoManifest,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("depot-oe-det-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn install(root: &Path, plugin_bytes: &[u8], manifest_hash: &str) {
        std::fs::create_dir_all(root.join("plugins")).unwrap();
        std::fs::write(root.join("OmsiLaunch.exe"), b"x").unwrap();
        std::fs::write(root.join("OmsiLaunchW.exe"), b"x").unwrap();
        std::fs::write(
            root.join("plugins").join("OmsiLaunch.Plugin.opl"),
            plugin_bytes,
        )
        .unwrap();
        std::fs::write(
            root.join("release-manifest.json"),
            format!(
                "\u{feff}{{\"product_version\":\"0.1.0-beta4\",\"control_protocol\":\"0.1\",\"files\":[\
                 {{\"path\":\"OmsiLaunch.exe\",\"bytes\":1,\"sha256\":\"00\"}},\
                 {{\"path\":\"plugins/OmsiLaunch.Plugin.opl\",\"bytes\":1,\"sha256\":\"{manifest_hash}\"}}]}}"
            ),
        )
        .unwrap();
    }

    #[test]
    fn nothing_installed_is_reported_as_such() {
        let d = tmp("none");
        let s = omsilaunch_in(&d);
        assert!(!s.installed && s.version.is_none());
        assert_eq!(s.integrity, Integrity::NoManifest);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn manifest_hash_decides_integrity() {
        let d = tmp("hash");
        let good = format!("{:X}", Sha256::digest(b"plugin"));
        install(&d, b"plugin", &good.to_ascii_lowercase());
        let s = omsilaunch_in(&d);
        assert!(s.installed && s.plugin_present);
        assert_eq!(s.version.as_deref(), Some("0.1.0-beta4"));
        assert_eq!(s.integrity, Integrity::Intact);
        install(&d, b"changed", &good);
        assert_eq!(
            omsilaunch_in(&d).integrity,
            Integrity::Damaged(vec!["plugins/OmsiLaunch.Plugin.opl".into()])
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn broken_manifest_is_unreadable_not_a_panic() {
        let d = tmp("broken");
        install(&d, b"p", "00");
        std::fs::write(d.join("release-manifest.json"), "{").unwrap();
        assert_eq!(omsilaunch_in(&d).integrity, Integrity::Unreadable);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn program_files_is_recognised_in_any_case() {
        assert!(in_program_files(Path::new(
            r"C:\Program Files (x86)\Steam\OMSI 2"
        )));
        assert!(in_program_files(Path::new("c:/PROGRAM FILES/OMSI 2")));
        assert!(!in_program_files(Path::new(r"D:\Games\OMSI 2")));
    }

    #[test]
    fn dotnet_six_is_found_per_architecture() {
        let a = tmp("net-a");
        let b = tmp("net-b");
        assert_eq!(
            dotnet_in(&a, &b),
            DotnetStatus {
                x64: false,
                x86: false
            }
        );
        std::fs::create_dir_all(a.join("dotnet/shared/Microsoft.WindowsDesktop.App/6.0.36"))
            .unwrap();
        std::fs::create_dir_all(b.join("dotnet/shared/Microsoft.NETCore.App/8.0.1")).unwrap();
        assert_eq!(
            dotnet_in(&a, &b),
            DotnetStatus {
                x64: true,
                x86: false
            }
        );
        std::fs::create_dir_all(b.join("dotnet/shared/Microsoft.NETCore.App/6.0.36")).unwrap();
        assert_eq!(
            dotnet_in(&a, &b),
            DotnetStatus {
                x64: true,
                x86: true
            }
        );
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn requirements_need_folder_outside_program_files_and_both_runtimes() {
        let d = tmp("req");
        let both = DotnetStatus {
            x64: true,
            x86: true,
        };
        assert!(status_for(Some(&d), both.clone()).requirements_met());
        assert!(!status_for(None, both).requirements_met());
        assert!(!status_for(
            Some(&d),
            DotnetStatus {
                x64: true,
                x86: false
            }
        )
        .requirements_met());
        let _ = std::fs::remove_dir_all(&d);
    }
}
