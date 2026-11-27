//! Reading the start menu's choices from the installation and the mods: the maps,
//! the vehicles, the situations and the weather files. The state and its rules are in
//! `domain/menu.rs`.

use super::super::domain::menu::Menu;
use std::path::{Path, PathBuf};

impl Menu {
    pub fn new(root: &Path, default_map: &str) -> Menu {
        // the folders of every content root: the installation, the installed mods and the
        // archives read in place
        let merged = |rel: &str| {
            let mut dirs = omsi_cfg::read_dir_merged(rel);
            if dirs.is_empty() {
                dirs = omsi_cfg::vfs::read_dir_paths(&root.join(rel));
            }
            dirs.retain(|d| omsi_cfg::vfs::is_dir(d));
            dirs.sort_by_key(|d| d.file_name().map(|n| n.to_ascii_lowercase()));
            dirs
        };
        let mut maps = Vec::new();
        for d in merged("maps") {
            let cfg = omsi_cfg::resolve_path(&d, "global.cfg");
            if !omsi_cfg::vfs::is_file(&cfg) {
                continue;
            }
            let name = omsi_map::GlobalCfg::load(&cfg)
                .map(|g| g.name.clone())
                .ok()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| d.file_name().unwrap().to_string_lossy().into_owned());
            maps.push((
                name,
                format!(
                    "maps/{}/global.cfg",
                    d.file_name().unwrap().to_string_lossy()
                ),
            ));
        }
        // every content root's vehicle folders (installed mods too), and of their files only
        // what OMSI offers: those with a [friendlyname] - never an articulated bus's rear
        // section, which comes with its front
        let mut vehicles = Vec::new();
        let mut vehicle_meta = Vec::new();
        for d in merged("Vehicles") {
            // the folder's files over all roots: a repaint installed as a mod brings only
            // textures and must neither hide the installation's bus nor add one of its own
            let folder = d.file_name().unwrap().to_string_lossy().to_string();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut files: Vec<PathBuf> = Vec::new();
            for dd in omsi_cfg::mirrored_dirs(&d) {
                for f in omsi_cfg::vfs::read_dir_paths(&dd) {
                    if seen.insert(
                        f.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .to_ascii_lowercase(),
                    ) {
                        files.push(f);
                    }
                }
            }
            files.sort_by_key(|f| {
                f.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_ascii_lowercase()
            });
            for (f, v) in omsi_vehicle::vehicle::offered_vehicles(&files) {
                let rel = format!(
                    "Vehicles/{}/{}",
                    folder,
                    f.file_name().unwrap().to_string_lossy()
                );
                let stem = f.file_stem().unwrap().to_string_lossy().to_string();
                let name = Some(
                    format!("{} {}", v.manufacturer, v.type_name)
                        .trim()
                        .to_string(),
                )
                .filter(|n| !n.is_empty())
                .unwrap_or(stem);
                vehicle_meta.push((
                    v.manufacturer.trim().to_string(),
                    Some(v.type_name.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .unwrap_or_else(|| name.clone()),
                ));
                vehicles.push((name, rel));
            }
        }
        log::info!("menu: {} vehicles offered", vehicles.len());
        let mut situations = vec![("-".to_string(), String::new())];
        // the installation's and the content folder's (where the game saves them)
        let mut files: Vec<PathBuf> = omsi_cfg::read_dir_merged("Situations");
        if files.is_empty() {
            files = std::fs::read_dir(root.join("Situations"))
                .map(|rd| rd.flatten().map(|e| e.path()).collect())
                .unwrap_or_default();
        }
        files.retain(|p| {
            p.extension()
                .map(|e| e.eq_ignore_ascii_case("osn"))
                .unwrap_or(false)
        });
        files.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
        {
            for f in files {
                let name = omsi_content::Situation::load(&f)
                    .map(|s| s.name)
                    .ok()
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| f.file_stem().unwrap().to_string_lossy().into_owned());
                situations.push((
                    name,
                    format!("Situations/{}", f.file_name().unwrap().to_string_lossy()),
                ));
            }
        }
        let mut weathers = vec![("default".to_string(), String::new())];
        if let Ok(rd) = std::fs::read_dir(root.join("Weather")) {
            let mut files: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .map(|e| e.eq_ignore_ascii_case("owt"))
                        .unwrap_or(false)
                })
                .collect();
            files.sort();
            for f in files {
                let name = f
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .trim_start_matches('#')
                    .to_string();
                weathers.push((
                    name,
                    format!("Weather/{}", f.file_name().unwrap().to_string_lossy()),
                ));
            }
        }
        let map = maps
            .iter()
            .position(|m| m.1.eq_ignore_ascii_case(default_map))
            .unwrap_or(0);
        let vehicle = vehicles
            .iter()
            .position(|v| v.1.to_ascii_lowercase().contains("sd80"))
            .unwrap_or(0);
        Menu {
            maps,
            vehicles,
            vehicle_meta,
            map,
            vehicle,
            hour: 9,
            traffic: 30,
            passengers: true,
            schedule: true,
            row: 0,
            start: false,
            situations,
            situation: 0,
            weathers,
            weather: 0,
            day: 150,
        }
    }
}
