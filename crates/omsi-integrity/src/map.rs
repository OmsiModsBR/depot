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
    seen_texture: HashMap<PathBuf, String>,
    seen_long: HashSet<PathBuf>,
    pub(crate) sound_seen: HashSet<PathBuf>,
    /// The `[texchanges]` masters of the object being looked at: their mesh texture names are
    /// placeholders, the pictures are the entries.
    masters: Vec<omsi_model::TexChangeMaster>,
}

impl<'a> Checker<'a> {
    pub(crate) fn new(root: &'a Path) -> Self {
        // (the folder listings the loaders keep are dropped: files may have changed since an
        // earlier check)
        omsi_cfg::content_changed();
        Checker {
            root,
            report: Report::default(),
            pending: BTreeMap::new(),
            seen_sco: HashSet::new(),
            seen_sli: HashSet::new(),
            seen_mesh: HashSet::new(),
            mesh_cache: HashMap::new(),
            seen_texture: HashMap::new(),
            seen_long: HashSet::new(),
            sound_seen: HashSet::new(),
            masters: Vec::new(),
        }
    }

    /// Where a missing texture was first looked for, as the finding's identity: two objects
    /// that lack different files of the same name are two findings, and a comparison with a
    /// baseline tells them apart.
    pub(crate) fn missing_target(&self, dirs: &[&Path], name: &str) -> String {
        match dirs.first() {
            Some(d) => self.rel(&d.join(name.replace('\\', "/"))),
            None => name.to_string(),
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
                known: false,
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
        if global.ground_textures.is_empty() {
            // (the game falls back to the installation's `Texture\gras.bmp`)
            self.ground_texture("", &by, true);
        }
        for (i, g) in global.ground_textures.iter().enumerate() {
            self.ground_texture(&g.texture, &by, i == 0);
            self.ground_texture(&g.detail_texture, &by, false);
        }
        // (the game looks for chrono folders at any depth below `Chrono`)
        let mut chrono_dirs = Vec::new();
        collect_dirs(&resolve_path(map_dir, "Chrono"), 4, &mut chrono_dirs);
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
            self.tile_content(&tile, &tile_name, true);
            // the patches of every chrono folder (active or not: the check is of the map)
            for base in &chrono_dirs {
                let name = t.file.rsplit(['/', '\\']).next().unwrap_or(&t.file);
                let patch_path = resolve_path(base, name);
                if !vfs::is_file(&patch_path) {
                    continue;
                }
                let patch_name = self.rel(&patch_path);
                match Tile::load(&patch_path) {
                    Ok(patch) => self.tile_content(&patch, &patch_name, false),
                    Err(e) => self.add(
                        Severity::Error,
                        "tile-unreadable",
                        patch_name,
                        &by,
                        format!("The chrono patch cannot be read: {e}"),
                        None,
                    ),
                }
            }
        }
    }

    /// The objects, splines and replacements a tile (or a chrono patch of one) names.
    fn tile_content(&mut self, tile: &Tile, tile_name: &str, count: bool) {
        for o in tile.objects.iter().chain(&tile.attach_objects) {
            self.report.objects += count as usize;
            self.object(&o.file, tile_name);
        }
        for a in &tile.spline_attachments {
            self.report.objects += count as usize;
            self.object(&a.file, tile_name);
        }
        for s in &tile.splines {
            self.report.splines += count as usize;
            self.spline(&s.file, tile_name);
        }
        // what a chrono patch changes an object or spline to
        for c in &tile.chrono_changes {
            if let Some(f) = &c.new_type {
                if c.is_spline {
                    self.spline(f, tile_name);
                } else {
                    self.object(f, tile_name);
                }
            }
        }
    }

    /// A ground texture is named from the installation folder (`Texture\...`) and looked
    /// for there only; the first ground, when it names none, is the installation's
    /// `Texture\gras.bmp`.
    fn ground_texture(&mut self, name: &str, by: &str, first: bool) {
        let name = match (name.trim(), first) {
            ("", true) => "Texture\\gras.bmp",
            ("", false) => return,
            (n, _) => n,
        };
        match omsi_texture::find_texture_uncached(name, &[self.root]) {
            Some(p) => self.texture(&p, by),
            None => self.add(
                Severity::Warning,
                "texture-missing",
                self.missing_target(&[self.root], name),
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
        self.model_meshes(&model, &model_dir, &dirs, &[], &file);
    }

    /// Every mesh a model names, with their textures.
    pub(crate) fn model_meshes(
        &mut self,
        model: &Model,
        model_dir: &Path,
        dirs: &[PathBuf],
        fallback_bases: &[PathBuf],
        file: &str,
    ) {
        // (a vehicle names them from its own folder, which may not be the model.cfg's)
        self.masters = omsi_model::load_texchanges(model_dir, &model.texchanges);
        for b in fallback_bases {
            if self.masters.is_empty() {
                self.masters = omsi_model::load_texchanges(b, &model.texchanges);
            }
        }
        for md in &model.meshes {
            if md.file.trim().is_empty() {
                continue;
            }
            let mut mesh_path = resolve_path(&resolve_path(model_dir, "model"), &md.file);
            if !vfs::is_file(&mesh_path) {
                mesh_path = resolve_path(model_dir, &md.file);
            }
            if !vfs::is_file(&mesh_path) {
                if let Some(p) = fallback_bases.iter().find_map(|b| find_beside(b, &md.file)) {
                    mesh_path = p;
                }
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
        self.masters.clear();
    }

    fn mesh(&mut self, path: &Path, dirs: &[PathBuf], overrides: &[MaterialDef], by: &str) {
        // (a mesh shared by two objects is looked at once per texture lookup context: the
        // folders its textures are searched in, and the material commands that name more)
        let masters: Vec<&str> = self.masters.iter().map(|m| m.texture.as_str()).collect();
        let context = format!("{dirs:?}{overrides:?}{masters:?}");
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
            let mut entries: Vec<(String, PathBuf)> = Vec::new();
            // a texture a [texchanges] master replaces is a placeholder name; its entries are
            // the pictures
            let master = self
                .masters
                .iter()
                .find(|x| x.texture.trim().eq_ignore_ascii_case(m.texture.trim()));
            if let Some(x) = master {
                for e in &x.entries {
                    entries.push((e.clone(), x.dir.clone()));
                }
            } else if !generated {
                names.push(&m.texture);
            }
            for o in &slot_overrides {
                names.extend(o.nightmap.as_deref());
                names.extend(o.transmap.as_deref());
                names.extend(o.envmap.as_ref().map(|e| e.0.as_str()));
                names.extend(o.envmap_mask.as_deref());
                names.extend(o.bumpmap.as_ref().map(|b| b.0.as_str()));
            }
            for (name, dir) in &entries {
                let name = name.trim();
                if is_null_texture(name) {
                    continue;
                }
                let mut where_ = vec![dir.as_path()];
                where_.extend(dir_refs.iter().copied());
                match omsi_texture::find_texture_uncached(name, &where_) {
                    Some(p) => self.texture(&p, &mesh_name),
                    None => self.add(
                        Severity::Warning,
                        "texture-missing",
                        name.to_string(),
                        &mesh_name,
                        "A texture a texture-change list names is missing.".into(),
                        None,
                    ),
                }
            }
            for name in names {
                let name = name.trim();
                // `null.bmp` stands for no texture, `\S:n` for a picture a script draws
                if is_null_texture(name) || name.starts_with("\\S:") || is_reflexion(name) {
                    continue;
                }
                match omsi_texture::find_texture_uncached(name, &dir_refs) {
                    Some(p) => self.texture(&p, &mesh_name),
                    None => self.add(
                        Severity::Warning,
                        "texture-missing",
                        self.missing_target(&dir_refs, name),
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
            match omsi_texture::find_texture_uncached(name, &dir_refs) {
                Some(p) => self.texture(&p, &file),
                None => self.add(
                    Severity::Warning,
                    "texture-missing",
                    self.missing_target(&dir_refs, name),
                    &file,
                    "A texture a spline names is missing; the road or rail shows white.".into(),
                    None,
                ),
            }
        }
    }

    pub(crate) fn texture(&mut self, path: &Path, by: &str) {
        if let Some(file) = self.seen_texture.get(path) {
            // (checked already: each finding about it counts one more file that names it)
            for code in ["texture-npot", "texture-unreadable", "path-too-long"] {
                if let Some(p) = self.pending.get_mut(&(code, file.clone())) {
                    p.count += 1;
                }
            }
            return;
        }
        let file = self.rel(path);
        self.seen_texture.insert(path.to_path_buf(), file.clone());
        self.report.textures += 1;
        self.long_path(path, by);
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
        if path.to_string_lossy().encode_utf16().count() > MAX_PATH
            && self.seen_long.insert(path.into())
        {
            self.add(
                Severity::Warning,
                "path-too-long",
                self.rel(path),
                by,
                "The full path is longer than 259 characters.".into(),
                Some(
                    "Windows refuses paths over 259 characters in most programs, including \
                     OMSI 2. The length is measured with the folder the installation is in now; \
                     in a deeper folder, this file may not load."
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

/// A mirror or camera monitor: the game draws what a reflection camera sees into a texture
/// named `reflexion<N>`, so no file of that name is needed.
fn is_reflexion(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    n.strip_prefix("reflexion")
        .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
}

/// A file named from another folder, found in `base`: as written, or without its first
/// folder (a path that names its own folder again, given from inside it).
pub(crate) fn find_beside(base: &Path, file: &str) -> Option<PathBuf> {
    let p = resolve_path(base, file);
    if vfs::exists(&p) {
        return Some(p);
    }
    let trimmed = file.trim_start_matches(['\\', '/']);
    let (_, rest) = trimmed.split_once(['\\', '/'])?;
    let p = resolve_path(base, rest);
    vfs::exists(&p).then_some(p)
}

/// The folders below `dir`, to `depth` levels.
fn collect_dirs(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    for (name, is_dir) in vfs::list_dir(dir).unwrap_or_default() {
        if is_dir {
            let sub = dir.join(name);
            out.push(sub.clone());
            collect_dirs(&sub, depth - 1, out);
        }
    }
}
