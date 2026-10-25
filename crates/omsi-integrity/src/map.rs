use crate::image_size::{image_size, next_pow2};
use crate::report::{Finding, Report, Severity};
use omsi_cfg::{resolve_path, vfs};
use omsi_map::{GlobalCfg, Tile};
use omsi_model::{MaterialDef, Model};
use omsi_scenery::{SceneryObject, Spline};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Longest path Windows takes without the `\\?\` prefix, the terminating NUL not counted.
const MAX_PATH: usize = 259;

/// Check the map whose `global.cfg` is in `map_dir` (a folder of `root\Maps`), with `root`
/// the OMSI 2 folder the map's content lives in. A caller that wants mod folders searched
/// first adds them as content roots (`omsi_cfg::add_content_root`) before.
pub fn check_map(root: &Path, map_dir: &Path) -> Report {
    let mut c = Checker::new(root);
    c.report.map = map_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    c.run(map_dir);
    c.finish()
}

/// What one kind of finding collects: who names the file, and how many do.
struct Pending {
    severity: Severity,
    message: String,
    detail: Option<String>,
    first_by: String,
    count: usize,
}

pub(crate) struct Checker<'a> {
    pub(crate) root: &'a Path,
    pub(crate) report: Report,
    pending: BTreeMap<(&'static str, String), Pending>,
    seen_sco: HashSet<PathBuf>,
    seen_sli: HashSet<PathBuf>,
    seen_mesh: HashSet<(PathBuf, String)>,
    mesh_cache: HashMap<PathBuf, Option<Vec<omsi_o3d::Material>>>,
    seen_texture: HashSet<PathBuf>,
    seen_long: HashSet<PathBuf>,
    pub(crate) sound_seen: HashSet<PathBuf>,
}

impl<'a> Checker<'a> {
    pub(crate) fn new(root: &'a Path) -> Self {
        Checker {
            root,
            report: Report::default(),
            pending: BTreeMap::new(),
            seen_sco: HashSet::new(),
            seen_sli: HashSet::new(),
            seen_mesh: HashSet::new(),
            mesh_cache: HashMap::new(),
            seen_texture: HashSet::new(),
            seen_long: HashSet::new(),
            sound_seen: HashSet::new(),
        }
    }

    pub(crate) fn rel(&self, p: &Path) -> String {
        p.strip_prefix(self.root).unwrap_or(p).display().to_string()
    }

    pub(crate) fn add(
        &mut self,
        severity: Severity,
        code: &'static str,
        target: String,
        by: &str,
        message: String,
        detail: Option<String>,
    ) {
        let e = self.pending.entry((code, target)).or_insert(Pending {
            severity,
            message,
            detail,
            first_by: by.to_string(),
            count: 0,
        });
        e.count += 1;
    }

    pub(crate) fn finish(mut self) -> Report {
        for ((code, file), p) in std::mem::take(&mut self.pending) {
            self.report.findings.push(Finding {
                severity: p.severity,
                code,
                file,
                message: p.message,
                detail: p.detail,
                referenced_by: (!p.first_by.is_empty()).then_some((p.first_by, p.count)),
            });
        }
        self.report
            .findings
            .sort_by(|a, b| (a.severity, a.code, &a.file).cmp(&(b.severity, b.code, &b.file)));
        self.report
    }

    fn run(&mut self, map_dir: &Path) {
        let global_path = resolve_path(map_dir, "global.cfg");
        let global = match GlobalCfg::load(&global_path) {
            Ok(g) => g,
            Err(e) => {
                let file = self.rel(&global_path);
                self.add(
                    Severity::Error,
                    "map-unreadable",
                    file,
                    "",
                    format!("global.cfg cannot be read: {e}"),
                    None,
                );
                return;
            }
        };
        let by = self.rel(&global_path);
        for g in &global.ground_textures {
            for name in [&g.texture, &g.detail_texture] {
                self.ground_texture(name, &by);
            }
        }
        for t in &global.tiles {
            let path = resolve_path(map_dir, &t.file);
            let tile_name = self.rel(&path);
            if !vfs::is_file(&path) {
                // the game treats a listed tile it cannot find as empty
                self.add(
                    Severity::Warning,
                    "tile-missing",
                    tile_name,
                    &by,
                    "A tile listed in global.cfg is missing; the game leaves that square empty."
                        .into(),
                    None,
                );
                continue;
            }
            let tile = match Tile::load(&path) {
                Ok(t) => t,
                Err(e) => {
                    self.add(
                        Severity::Error,
                        "tile-unreadable",
                        tile_name,
                        &by,
                        format!("The tile cannot be read: {e}"),
                        None,
                    );
                    continue;
                }
            };
            self.report.tiles += 1;
            for o in tile.objects.iter().chain(&tile.attach_objects) {
                self.report.objects += 1;
                self.object(&o.file, &tile_name);
            }
            for a in &tile.spline_attachments {
                self.report.objects += 1;
                self.object(&a.file, &tile_name);
            }
            for s in &tile.splines {
                self.report.splines += 1;
                self.spline(&s.file, &tile_name);
            }
        }
    }

    /// A ground texture is named from the installation folder (`Texture\...`).
    fn ground_texture(&mut self, name: &str, by: &str) {
        if name.trim().is_empty() {
            return;
        }
        let texture_dir = resolve_path(self.root, "Texture");
        match omsi_texture::find_texture(name, &[self.root, &texture_dir]) {
            Some(p) => self.texture(&p, by),
            None => self.add(
                Severity::Warning,
                "texture-missing",
                name.to_string(),
                by,
                "A ground texture is missing; that ground shows white.".into(),
                None,
            ),
        }
    }

    fn object(&mut self, rel: &str, by: &str) {
        if rel.trim().is_empty() {
            return;
        }
        let path = resolve_path(self.root, rel);
        if !vfs::is_file(&path) {
            self.add(
                Severity::Error,
                "object-missing",
                rel.to_string(),
                by,
                "A scenery object the map places is missing; it is not drawn.".into(),
                None,
            );
            return;
        }
        if !self.seen_sco.insert(path.clone()) {
            return;
        }
        self.long_path(&path, by);
        let file = self.rel(&path);
        let sco = match SceneryObject::load(&path) {
            Ok(s) => s,
            Err(e) => {
                self.add(
                    Severity::Error,
                    "object-unreadable",
                    file,
                    by,
                    format!("The object file cannot be read: {e}"),
                    None,
                );
                return;
            }
        };
        let Some(sco_dir) = path.parent() else { return };
        let (model, model_dir) = match &sco.model_file {
            Some(m) => {
                let mp = resolve_path(sco_dir, m);
                match Model::load(&mp) {
                    Ok(model) => (model, mp.parent().unwrap_or(sco_dir).to_path_buf()),
                    Err(_) => {
                        self.add(
                            Severity::Error,
                            "model-missing",
                            self.rel(&mp),
                            &file,
                            "The model.cfg an object names is missing or unreadable; the object is not drawn."
                                .into(),
                            None,
                        );
                        return;
                    }
                }
            }
            None => (sco.model.clone(), sco_dir.to_path_buf()),
        };
        let mut dirs = texture_dirs(self.root, &model_dir);
        if sco.model_file.is_some() {
            let own = resolve_path(sco_dir, "texture");
            dirs.retain(|d| *d != own);
            dirs.insert(0, own);
        }
        self.model_meshes(&model, &model_dir, &dirs, &file);
    }

    /// Every mesh a model names, with their textures.
    pub(crate) fn model_meshes(
        &mut self,
        model: &Model,
        model_dir: &Path,
        dirs: &[PathBuf],
        file: &str,
    ) {
        for md in &model.meshes {
            if md.file.trim().is_empty() {
                continue;
            }
            let mut mesh_path = resolve_path(&resolve_path(model_dir, "model"), &md.file);
            if !vfs::is_file(&mesh_path) {
                mesh_path = resolve_path(model_dir, &md.file);
            }
            if !vfs::is_file(&mesh_path) {
                self.add(
                    Severity::Error,
                    "mesh-missing",
                    self.rel(&mesh_path),
                    file,
                    "A mesh of the object is missing; that part is not drawn.".into(),
                    None,
                );
                continue;
            }
            self.mesh(&mesh_path, dirs, &md.materials, file);
        }
    }

    fn mesh(&mut self, path: &Path, dirs: &[PathBuf], overrides: &[MaterialDef], by: &str) {
        // (a mesh shared by two objects is looked at once per texture lookup context: the
        // folders its textures are searched in, and the material commands that name more)
        let context = format!("{dirs:?}{overrides:?}");
        if !self.seen_mesh.insert((path.to_path_buf(), context)) {
            return;
        }
        self.long_path(path, by);
        if !self.mesh_cache.contains_key(path) {
            let loaded = match omsi_o3d::load_mesh(path) {
                Ok(m) => Some(m.materials),
                Err(e) => {
                    self.add(
                        Severity::Error,
                        "mesh-unreadable",
                        self.rel(path),
                        by,
                        format!("The mesh cannot be read: {e}"),
                        None,
                    );
                    None
                }
            };
            self.mesh_cache.insert(path.to_path_buf(), loaded);
        }
        let Some(materials) = self.mesh_cache.get(path).cloned().flatten() else {
            return;
        };
        let mesh_name = self.rel(path);
        let dir_refs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();
        for (slot, m) in materials.iter().enumerate() {
            let slot_overrides: Vec<&MaterialDef> = overrides
                .iter()
                .filter(|o| !o.item && override_slot(&materials, o) == Some(slot))
                .collect();
            // a slot fed by [useTextTexture] / [useScriptTexture] gets a generated picture:
            // the name in the mesh is a placeholder
            let generated = slot_overrides
                .iter()
                .any(|o| o.use_text_texture.is_some() || o.use_script_texture.is_some());
            let mut names: Vec<&str> = Vec::new();
            if !generated {
                names.push(&m.texture);
            }
            for o in &slot_overrides {
                names.extend(o.nightmap.as_deref());
                names.extend(o.transmap.as_deref());
                names.extend(o.envmap.as_ref().map(|e| e.0.as_str()));
                names.extend(o.envmap_mask.as_deref());
                names.extend(o.bumpmap.as_ref().map(|b| b.0.as_str()));
            }
            for name in names {
                let name = name.trim();
                // `null.bmp` stands for no texture, `\S:n` for a picture a script draws
                if is_null_texture(name) || name.starts_with("\\S:") {
                    continue;
                }
                match omsi_texture::find_texture(name, &dir_refs) {
                    Some(p) => self.texture(&p, &mesh_name),
                    None => self.add(
                        Severity::Warning,
                        "texture-missing",
                        name.to_string(),
                        &mesh_name,
                        "A texture a mesh names is missing; the surface shows white.".into(),
                        None,
                    ),
                }
            }
        }
    }

    fn spline(&mut self, rel: &str, by: &str) {
        if rel.trim().is_empty() {
            return;
        }
        let path = resolve_path(self.root, rel);
        if !vfs::is_file(&path) {
            self.add(
                Severity::Error,
                "spline-missing",
                rel.to_string(),
                by,
                "A spline profile the map uses is missing; those splines are not drawn.".into(),
                None,
            );
            return;
        }
        if !self.seen_sli.insert(path.clone()) {
            return;
        }
        self.long_path(&path, by);
        let file = self.rel(&path);
        let sli = match Spline::load(&path) {
            Ok(s) => s,
            Err(e) => {
                self.add(
                    Severity::Error,
                    "spline-unreadable",
                    file,
                    by,
                    format!("The spline file cannot be read: {e}"),
                    None,
                );
                return;
            }
        };
        let Some(dir) = path.parent() else { return };
        let dirs = texture_dirs(self.root, dir);
        let dir_refs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();
        for t in &sli.textures {
            let name = t.file.trim();
            if name.is_empty() {
                continue;
            }
            match omsi_texture::find_texture(name, &dir_refs) {
                Some(p) => self.texture(&p, &file),
                None => self.add(
                    Severity::Warning,
                    "texture-missing",
                    name.to_string(),
                    &file,
                    "A texture a spline names is missing; the road or rail shows white.".into(),
                    None,
                ),
            }
        }
    }

    pub(crate) fn texture(&mut self, path: &Path, by: &str) {
        if !self.seen_texture.insert(path.to_path_buf()) {
            return;
        }
        self.report.textures += 1;
        self.long_path(path, by);
        let file = self.rel(path);
        match image_size(path) {
            Some((w, h)) if !(w.is_power_of_two() && h.is_power_of_two()) => {
                let (pw, ph) = (next_pow2(w), next_pow2(h));
                let (lw, lh) = (pw.min(prev_pow2(w)), ph.min(prev_pow2(h)));
                let ratio = (pw as f64 * ph as f64) / (w as f64 * h as f64);
                self.add(
                    Severity::NotOptimized,
                    "texture-npot",
                    file,
                    by,
                    format!("{w}x{h}: not a power of two, takes {pw}x{ph} in video memory."),
                    Some(format!(
                        "OMSI 2 uses DirectX 9, which enlarges a texture whose size is not a \
                         power of two to the next one. This {w}x{h} file takes {pw}x{ph} in \
                         PC/GPU memory ({ratio:.1}x the pixels). Saving it as {pw}x{ph} \
                         costs the same memory and loads without conversion; {lw}x{lh} uses less."
                    )),
                );
            }
            Some(_) => {}
            None => self.add(
                Severity::Warning,
                "texture-unreadable",
                file,
                by,
                "The picture is not a bmp, tga, dds, png or jpg the game can read.".into(),
                None,
            ),
        }
    }

    fn long_path(&mut self, path: &Path, by: &str) {
        if path.to_string_lossy().chars().count() > MAX_PATH && self.seen_long.insert(path.into()) {
            self.add(
                Severity::Warning,
                "path-too-long",
                self.rel(path),
                by,
                "The full path is longer than 259 characters.".into(),
                Some(
                    "Windows refuses paths over 259 characters in most programs, including \
                     OMSI 2; installed in a deeper folder, this file may not load."
                        .into(),
                ),
            );
        }
    }
}

fn prev_pow2(n: u32) -> u32 {
    if n <= 1 {
        1
    } else {
        1 << (31 - n.leading_zeros())
    }
}

/// Where the game looks for a texture of content in `content_dir`: its `texture` folder,
/// the folder itself, the `texture` next to a vehicle's `model` folder, then `Texture` of
/// the installation.
pub(crate) fn texture_dirs(root: &Path, content_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        resolve_path(content_dir, "texture"),
        content_dir.to_path_buf(),
    ];
    if let Some(parent) = content_dir.parent() {
        dirs.push(resolve_path(parent, "texture"));
    }
    dirs.push(resolve_path(root, "Texture"));
    dirs
}

/// A texture name that stands for "no texture": exporters write `null.bmp` into slots that
/// have none.
pub(crate) fn is_null_texture(name: &str) -> bool {
    let n = name.trim();
    n.is_empty()
        || Path::new(&n.replace('\\', "/"))
            .file_stem()
            .is_some_and(|s| s.eq_ignore_ascii_case("null"))
}

/// The material slot a `[matl]` block refers to: its texture name, and which occurrence of
/// that name when the mesh uses it more than once.
fn override_slot(materials: &[omsi_o3d::Material], o: &MaterialDef) -> Option<usize> {
    let mut nth = 0;
    for (slot, m) in materials.iter().enumerate() {
        if m.texture.eq_ignore_ascii_case(&o.texture) {
            if nth == o.index.max(0) as usize {
                return Some(slot);
            }
            nth += 1;
        }
    }
    None
}
