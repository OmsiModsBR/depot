//! OmsiLaunch's `OL_E_*` codes as plain sentences and what the player can do about them.
//! The sentences are the keys of the launcher's text catalog (translated there).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fix {
    None,
    /// Open Microsoft's .NET 6 download page.
    InstallDotnet,
    /// Another session holds the OMSI 2 folder: close it, then try again.
    CloseOtherSession,
    /// A session left files changed: restore them.
    RecoverSession,
    /// The OmsiLaunch files are damaged or do not match: install them again.
    ReinstallOmsiLaunch,
    /// The folder needs write access (not under Program Files).
    MoveOmsiFolder,
}

pub fn explain(code: &str) -> (&'static str, Fix) {
    use Fix::*;
    match code {
        "OL_E_INSTALLATION_BUSY" | "OL_E_SESSION_ALREADY_ACTIVE" => (
            "Another OMSI 2 session started through OmsiLaunch is still running. Close it and try again.",
            CloseOtherSession,
        ),
        "OL_E_INSTALLATION_NOT_WRITABLE" => (
            "The OMSI 2 folder cannot be written to by this user. Move OMSI 2 out of Program Files or allow write access to the folder.",
            MoveOmsiFolder,
        ),
        "OL_E_INSTALLATION_NOT_FOUND" => ("The OMSI 2 folder was not found. Choose it under Setup.", None),
        "OL_E_UNSUPPORTED_BUILD" => ("This OMSI 2 program file is not one OmsiLaunch can start.", None),
        "OL_E_UNSUPPORTED_OPERATING_SYSTEM" | "OL_E_UNSUPPORTED_OS_ARCHITECTURE" => {
            ("Starting the original OMSI 2 needs 64-bit Windows 10 or later.", None)
        }
        "OL_E_MAP_NOT_FOUND" | "OL_E_SITUATION_MAP_NOT_FOUND" => ("The map was not found in the OMSI 2 folder.", None),
        "OL_E_SITUATION_NOT_FOUND" | "OL_E_SITUATION_LOAD_FAILED" => ("The saved situation could not be found or loaded.", None),
        "OL_E_ENTRYPOINT_REQUIRED" | "OL_E_ENTRYPOINT_NOT_FOUND" | "OL_E_ENTRYPOINT_NOT_PRESENTED" => {
            ("The start point is not available for this map. Choose another one.", None)
        }
        "OL_E_PERMANENT_PLUGIN_HASH_MISMATCH" | "OL_E_PERMANENT_PLUGIN_MISSING" | "OL_E_PERMANENT_PLUGIN_MANIFEST_INCOMPLETE" | "OL_E_RELEASE_MANIFEST_INVALID" | "OL_E_RUNTIME_ARTIFACT_MISSING" | "OL_E_RUNTIME_INSTALLATION_INCOMPLETE" => (
            "The OmsiLaunch files in the OMSI 2 folder are damaged or do not belong together. Install OmsiLaunch again.",
            ReinstallOmsiLaunch,
        ),
        "OL_E_PLUGIN_NOT_LOADED" | "OL_E_STARTUP_TIMEOUT" => (
            "OMSI 2 did not finish starting. Check that the .NET 6 runtimes (x64 and x86) are installed.",
            InstallDotnet,
        ),
        "OL_E_PLUGIN_PROTOCOL_MISMATCH" | "OL_E_RUNTIME_PROTOCOL_MISMATCH" => (
            "The OmsiLaunch files in the OMSI 2 folder are from different versions. Install OmsiLaunch again.",
            ReinstallOmsiLaunch,
        ),
        "OL_E_UNKNOWN_SETTING" | "OL_E_SETTING_NOT_WRITABLE" | "OL_E_INVALID_SETTING_VALUE" => (
            "A saved OMSI 2 setting is not accepted. Open Settings and check the values.",
            None,
        ),
        "OL_E_CAPABILITY_UNAVAILABLE" | "OL_E_CAPABILITY_UNRESOLVED" => ("This start option is not available in OMSI 2 mode.", None),
        "OL_E_WORLD_START_FAILED" | "OL_E_SESSION_START_FAILED" | "OL_E_START_SESSION" | "OL_E_PROCESS_START_FAILED" | "OL_E_PROCESS_EXITED_EARLY" => {
            ("OMSI 2 could not be started or closed before the map loaded.", None)
        }
        "OL_E_RESTORE_FAILED" | "OL_E_RECOVERY_BACKUP_CORRUPT" | "OL_E_RECOVERY_JOURNAL_MISSING" | "OL_E_RECOVERY_JOURNAL_REMOVE_FAILED" | "OL_E_RECOVERY_ABSENT_OWNERSHIP_MISMATCH" | "OL_E_RECOVERY_ABSENT_OWNERSHIP_UNVERIFIED" | "OL_E_RESTORE_DEFERRED" => (
            "OMSI 2 files changed for the session could not be restored. See the OmsiLaunch log.",
            RecoverSession,
        ),
        "OL_E_MODULE_CONFLICT" => ("A module conflicts with another plug-in in the OMSI 2 folder.", None),
        "OL_E_MODULE_NOT_FOUND" | "OL_E_MODULE_MANIFEST_INVALID" | "OL_E_MODULE_INTEGRITY" | "OL_E_MODULE_LOAD_FAILED" => {
            ("A module could not be loaded. Install it again or switch it off.", None)
        }
        "OL_E_MODULE_SETTING_INVALID" => ("A module setting is not accepted. Open Settings > Modules and check the values.", None),
        "OL_E_PLAN_NOT_RUNNABLE" => ("OmsiLaunch cannot start this session.", None),
        _ => ("OmsiLaunch reported a problem. See its log for details.", None),
    }
}

/// The `OL_E_...` token in a text (the first one), as OmsiLaunch itself extracts it.
pub fn extract_code(text: &str) -> Option<String> {
    let at = text.find("OL_E_")?;
    let code: String = text[at..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    Some(code)
}

/// Exit codes of the two shims, before OmsiLaunch's own program runs.
pub fn shim_problem(code: i32) -> Option<(&'static str, Fix)> {
    match code {
        100..=106 => Some((
            "OmsiLaunch could not start: the .NET 6 desktop runtime (x64) is missing or the OmsiLaunch files are incomplete.",
            Fix::InstallDotnet,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_codes_have_sentences_and_fixes() {
        assert_eq!(explain("OL_E_INSTALLATION_BUSY").1, Fix::CloseOtherSession);
        assert_eq!(
            explain("OL_E_PERMANENT_PLUGIN_HASH_MISMATCH").1,
            Fix::ReinstallOmsiLaunch
        );
        assert_eq!(explain("OL_E_RESTORE_FAILED").1, Fix::RecoverSession);
        assert_eq!(explain("OL_E_SOMETHING_NEW").1, Fix::None);
        assert!(explain("OL_E_SOMETHING_NEW").0.contains("OmsiLaunch"));
    }

    #[test]
    fn code_is_taken_from_a_message() {
        assert_eq!(
            extract_code("failed: OL_E_MAP_NOT_FOUND (maps\\X)").as_deref(),
            Some("OL_E_MAP_NOT_FOUND")
        );
        assert_eq!(extract_code("nothing here"), None);
        assert!(shim_problem(102).is_some() && shim_problem(7).is_none());
    }

    #[test]
    fn no_sentence_talks_about_testing_status() {
        for c in [
            "OL_E_MAP_NOT_FOUND",
            "OL_E_CAPABILITY_UNAVAILABLE",
            "OL_E_STARTUP_TIMEOUT",
            "OL_E_X",
        ] {
            let t = explain(c).0.to_ascii_lowercase();
            for w in [
                "beta",
                "experimental",
                "validated",
                "unsupported by testing",
            ] {
                assert!(!t.contains(w), "{c}: {t}");
            }
        }
    }
}
