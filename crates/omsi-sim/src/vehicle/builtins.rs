//! The variables every road vehicle's scripts have before they declare any.

use std::path::Path;

/// Entries and exits of a vehicle with variables of their own (`PAX_Entry<n>_Open` …
/// `PAX_Exit<n>_Req`): OMSI's eight, and eight more for buses with more doors than
/// that (#719). A cabin's entries and exits past the eighth that the scripts give no
/// variables of their own open with the eighth, as in OMSI, and ask through the eighth's
/// `_Req` (openOMSI's choice: OMSI's request arrays have eight slots and lose them).
pub const PAX_DOORS: usize = 16;

/// Built-in variables every road vehicle has (`program/varlist_roadvehicle.txt` + generated).
pub fn builtin_vars(root: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        match omsi_cfg::CfgFile::read(root.join("program/varlist_roadvehicle.txt")) {
            Ok(f) => f
                .lines
                .iter()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect(),
            Err(_) => Vec::new(),
        };
    for a in 0..8 {
        for side in ["L", "R"] {
            for pre in [
                "Wheel_Rotation_",
                "Wheel_RotationSpeed_",
                "Axle_Steering_",
                "Axle_Suspension_",
                "Axle_Springfactor_",
                "Axle_Brakeforce_",
                "Axle_SurfaceID_",
            ] {
                v.push(format!("{pre}{a}_{side}"));
            }
        }
        v.push(format!("PAX_Entry{a}_Open"));
        v.push(format!("PAX_Entry{a}_Req"));
        v.push(format!("PAX_Exit{a}_Open"));
        v.push(format!("PAX_Exit{a}_Req"));
    }
    for i in 0..6 {
        v.push(format!("Debug_{i}"));
    }
    for i in 0..4 {
        for n in ["alpha", "beta", "gamma"] {
            v.push(format!("articulation_{i}_{n}"));
        }
    }
    // the doors past OMSI's eight (#719), after all of its own variables
    for a in 8..PAX_DOORS {
        v.push(format!("PAX_Entry{a}_Open"));
        v.push(format!("PAX_Entry{a}_Req"));
        v.push(format!("PAX_Exit{a}_Open"));
        v.push(format!("PAX_Exit{a}_Req"));
    }
    // somebody standing in the doorway (openOMSI's, #720: what a door's light barrier
    // sees), after those
    for a in 0..PAX_DOORS {
        v.push(format!("PAX_Entry{a}_Busy"));
        v.push(format!("PAX_Exit{a}_Busy"));
    }
    v
}

pub fn builtin_str_vars(root: &Path) -> Vec<String> {
    match omsi_cfg::CfgFile::read(root.join("program/stringvarlist_roadvehicle.txt")) {
        Ok(f) => f
            .lines
            .iter()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => vec![
            "ident".into(),
            "number".into(),
            "act_route".into(),
            "act_busstop".into(),
            "SetLineTo".into(),
            "yard".into(),
            "file_schedule".into(),
        ],
    }
}
