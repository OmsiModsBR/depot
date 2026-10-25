use crate::map::{is_null_texture, Checker};
use crate::report::{Report, Severity};
use omsi_cfg::{resolve_path, vfs, CfgFile};
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
                        self.paints(&model, &dir, &model_dir, &dirs);
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

impl Checker<'_> {
    /// The textures of the paint schemes, found as the vehicle loader finds them: the
    /// `[CTC]` folders are named from the vehicle's folder, every content root's copy of a
    /// folder is read (a `.cti` of the same name in a root of higher priority wins), then the
    /// model's own paint items follow. A paint's texture is searched in the paint's folder
    /// first.
    fn paints(&mut self, model: &Model, vehicle_dir: &Path, _model_dir: &Path, dirs: &[PathBuf]) {
        let mut item_dir = vehicle_dir.to_path_buf();
        for (i, c) in model.ctc.iter().enumerate() {
            let folder = resolve_path(vehicle_dir, &c.path);
            if i == 0 {
                item_dir = folder.clone();
            }
            let mut seen = std::collections::HashSet::new();
            for d in omsi_cfg::mirrored_dirs(&folder) {
                let mut ctis: Vec<PathBuf> = vfs::read_dir_paths(&d)
                    .into_iter()
                    .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cti")))
                    .collect();
                ctis.sort();
                for cti in ctis {
                    let name = cti
                        .file_name()
                        .map(|n| n.to_string_lossy().to_ascii_lowercase())
                        .unwrap_or_default();
                    if !seen.insert(name) {
                        continue;
                    }
                    let by = self.rel(&cti);
                    let Ok(cfg) = CfgFile::read(&cti) else {
                        continue;
                    };
                    let mut r = cfg.reader();
                    while let Some(k) = r.next_keyword() {
                        if k == "item" {
                            let _name = r.str();
                            let _ctc = r.str();
                            let texture = r.str().to_string();
                            self.paint_texture(&texture, &d, dirs, &by);
                        }
                    }
                }
            }
        }
        let by = self.rel(&model.path);
        for it in &model.items {
            self.paint_texture(&it.texture, &item_dir, dirs, &by);
        }
    }

    fn paint_texture(&mut self, name: &str, folder: &Path, dirs: &[PathBuf], by: &str) {
        let name = name.trim();
        if is_null_texture(name) {
            return;
        }
        let mut look: Vec<&Path> = vec![folder];
        look.extend(dirs.iter().map(|d| d.as_path()));
        match omsi_texture::find_texture(name, &look) {
            Some(p) => self.texture(&p, by),
            None => self.add(
                Severity::Warning,
                "texture-missing",
                name.to_string(),
                by,
                "A paint scheme names a texture that is missing; that paint shows white.".into(),
                None,
            ),
        }
    }
}
