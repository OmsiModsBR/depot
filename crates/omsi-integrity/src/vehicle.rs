use crate::map::Checker;
use crate::report::{Report, Severity};
use omsi_cfg::{resolve_path, vfs};
use omsi_model::Model;
use omsi_vehicle::sound::SoundCfg;
use omsi_vehicle::vehicle::Vehicle;
use std::path::{Path, PathBuf};

/// Check one vehicle file (`.bus`, `.ovh`, ...): its model with meshes and textures, scripts
/// and variable lists, sound set and the other files it names. `root` is the OMSI 2 folder.
pub fn check_vehicle(root: &Path, vehicle_file: &Path) -> Report {
    let mut c = Checker::new(root);
    c.report.map = vehicle_file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    c.vehicle(vehicle_file);
    c.finish()
}

impl Checker<'_> {
    fn vehicle(&mut self, path: &Path) {
        let file = self.rel(path);
        let v = match Vehicle::load(path) {
            Ok(v) => v,
            Err(e) => {
                self.add(
                    Severity::Error,
                    "vehicle-unreadable",
                    file,
                    "",
                    format!("The vehicle file cannot be read: {e}"),
                    None,
                );
                return;
            }
        };
        self.report.objects += 1;
        let dir = v.dir().to_path_buf();

        match &v.model {
            Some(m) => {
                let mp = resolve_path(&dir, m);
                match Model::load(&mp) {
                    Ok(model) => {
                        let model_dir = mp.parent().unwrap_or(&dir).to_path_buf();
                        // (the folders the vehicle loader searches, in its order)
                        let dirs = vec![
                            resolve_path(&dir, "Texture"),
                            resolve_path(&model_dir, "Texture"),
                            model_dir.clone(),
                            resolve_path(self.root, "Texture"),
                        ];
                        // a mesh the model.cfg's folder lacks may lie beside the vehicle,
                        // in its `model` folder, one folder above the model.cfg, or in the
                        // installation
                        let bases: Vec<PathBuf> = [
                            Some(dir.clone()),
                            Some(resolve_path(&dir, "model")),
                            model_dir.parent().map(Path::to_path_buf),
                            Some(self.root.to_path_buf()),
                        ]
                        .into_iter()
                        .flatten()
                        .collect();
                        self.model_meshes(&model, &model_dir, &dirs, &bases, &file);
                    }
                    Err(_) => self.add(
                        Severity::Error,
                        "model-missing",
                        self.rel(&mp),
                        &file,
                        "The model.cfg the vehicle names is missing or unreadable; it has no body."
                            .into(),
                        None,
                    ),
                }
            }
            None => self.add(
                Severity::Error,
                "model-missing",
                file.clone(),
                "",
                "The vehicle names no model.cfg; it has no body.".into(),
                None,
            ),
        }

        let s = &v.scripts;
        for p in s
            .scripts
            .iter()
            .chain(&s.varlists)
            .chain(&s.stringvarlists)
            .chain(&s.constfiles)
        {
            if !vfs::is_file(p) {
                self.add(
                    Severity::Error,
                    "script-missing",
                    self.rel(p),
                    &file,
                    "A script or variable list of the vehicle is missing; its functions do not work."
                        .into(),
                    None,
                );
            }
        }

        for (name, kind, severity) in [
            (&v.sound, "sound", Severity::Warning),
            (&v.sound_ai, "sound", Severity::Warning),
            (&v.paths, "paths", Severity::Warning),
            (&v.passenger_cabin, "passenger-cabin", Severity::Warning),
        ] {
            if let Some(name) = name.as_deref().filter(|n| !n.trim().is_empty()) {
                let p = resolve_path(&dir, name);
                if !vfs::is_file(&p) {
                    let code: &'static str = match kind {
                        "sound" => "sound-missing",
                        "paths" => "paths-missing",
                        _ => "cabin-missing",
                    };
                    self.add(
                        severity,
                        code,
                        self.rel(&p),
                        &file,
                        "A file the vehicle names is missing.".into(),
                        None,
                    );
                } else if kind == "sound" {
                    self.sound_set(&p);
                }
            }
        }
    }

    fn sound_set(&mut self, cfg_path: &Path) {
        if !self.sound_seen.insert(cfg_path.to_path_buf()) {
            return;
        }
        let by = self.rel(cfg_path);
        let Ok(cfg) = SoundCfg::load(cfg_path) else {
            self.add(
                Severity::Warning,
                "sound-unreadable",
                by,
                "",
                "The sound set cannot be read; the vehicle is silent.".into(),
                None,
            );
            return;
        };
        let dir = cfg_path.parent().unwrap_or(Path::new(""));
        for def in &cfg.sounds {
            let name = def.file.trim();
            // `[sound] N` has no fixed file: a script names one when it plays
            if name.is_empty() || name.parse::<i32>().is_ok() {
                continue;
            }
            let p = resolve_path(dir, name);
            if !vfs::is_file(&p) {
                self.add(
                    Severity::Warning,
                    "sound-missing",
                    self.rel(&p),
                    &by,
                    "A sound file of the vehicle is missing; that sound is not played.".into(),
                    None,
                );
            }
        }
    }
}
