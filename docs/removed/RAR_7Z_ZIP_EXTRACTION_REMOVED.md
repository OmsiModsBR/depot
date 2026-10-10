# Suporte a RAR/7z e descompactação automática de ZIP removidos

Data: 2026-10-10. Decisão de Leo (OmsiModsBR Team): "remover suporte a RAR ou 7Z no momento, e também a descompactação automática de ZIP. ZIPs serão aceitos somente em Archives para execução direta." Nenhum commit feito junto com esta alteração.
Vem depois de [MULTIPLAYER_DISABLED.md](MULTIPLAYER_DISABLED.md); o diff abaixo é só o desta etapa (também em `rar-7z-zip-extraction-removal.patch`).

## O que mudou

- **Instalador de mods (`omsi-launcher-core/src/install.rs`)**: só aceita **pasta** ou **`.zip`**. `.rar` e `.7z` são recusados com mensagem clara. O `.zip` nunca é descompactado: é sempre colocado como está em `<conteúdo>/Archives` (modo "in place"), que o jogo já monta. Pastas continuam sendo copiadas como antes. Removidos `unpack_archive`, `enum Source` e o ramo de extração; `ArchiveKind` tem só `Zip`.
- **Dependências**: removidos `unrar-rs` e `sevenz-rust2` do `Cargo.toml` do launcher-core, e do `Cargo.lock` também `reedsolomon-rs` (transitiva do `unrar-rs`, usada só para volumes de recuperação RAR) e as dependências que só eles traziam (aes, cbc, sha2, lzma-rust2 etc.). O `Cargo.lock` só teve remoções.
- **UI**: o seletor de modo da página Mods (Auto/Extrair/Em Archives) saiu; o seletor de arquivo e a pasta de entrada (inbox) filtram só `.zip`; textos explicam que o ZIP é usado em Archives sem descompactar. `InstallMode` continua no código por compatibilidade (Auto/Extract não mudam mais nada); a chave antiga do modo em `app.yml` fica ignorada.
- **Docs**: README, `docs/USER_GUIDE.md`, `docs/ANDROID.md`.
- **Mantido**: a descompactação do atualizador do próprio jogo (não é conteúdo de mod).

## Efeito na segurança

- **Resolve o item D1 (licença GPL-3.0)** da auditoria: `unrar-rs` e `reedsolomon-rs` não estão mais no `Cargo.lock`. Projeto MIT sem código GPL ligado.
- **Reduz a superfície de DoS de extração** (itens L5–L7 e o 7z de D5): não há mais extração de RAR/7z/ZIP de mod no instalador, logo nada de zip-bomb, caminhos hostis na extração ou árvore recursiva vinda de arquivo.
- **Não resolve** os achados do montador de ZIP em si (`omsi-cfg/src/vfs.rs`, P1/P2): como o ZIP agora vai direto para `Archives/`, que é montado no boot, vale priorizá-los (nome de entrada longo/profundo, pânico em fronteira de caractere).

## Verificação

`cargo test -p omsi-launcher-core` e `-p omsi-cfg` passam; `omsi-app` compila. Testes do instalador reescritos para o novo comportamento (zip vai para Archives sem extrair; rar/7z recusados). Suíte `omsi-app` e build completo: pendentes para quem commitar (usar `--ignore-rust-version` com rustc < 1.97.1).

## Pendências

- Traduções antigas com texto de RAR/7z/modos podem ter sobrado.
- Remover de vez `InstallMode`/`mod_mode` numa limpeza futura.
- Se RAR/7z voltarem, usar crate com licença compatível e limites de tamanho/profundidade.

## Cópia das alterações (diff desta etapa)

Para restaurar: salvar o bloco como `rar-7z-zip-extraction-removal.patch` e aplicar `git apply -R`.

```diff
diff --git a/README.md b/README.md
index 7441128..832f1b6 100644
--- a/README.md
+++ b/README.md
@@ -91,7 +91,7 @@ brings no game content of its own; it plays the original's maps, buses and mods.
    `…\Steam\steamapps\common\OMSI 2`.
 4. **Drive:** pick a bus, a map and a duty on the **Drive** page and press **Start the duty**.
 
-**Mods** are installed on the **Mods** page (a folder or a `.zip`, or dropped on the window) or
+**Mods** are installed on the **Mods** page (a folder, or a `.zip` that is used in place from `Archives` and never unpacked; or dropped on the window) or
 by putting them into the `Mods` folder next to the game; the OMSI 2 folder is never written
 to.
 
diff --git a/crates/omsi-app/src/android.rs b/crates/omsi-app/src/android.rs
index 3cae680..5374144 100644
--- a/crates/omsi-app/src/android.rs
+++ b/crates/omsi-app/src/android.rs
@@ -304,7 +304,7 @@ const README: &str = "Project Depot\n\
 Put a complete copy of OMSI 2 (the folder with Omsi.exe, maps and Vehicles in it) here as\n\
 \"OMSI 2\", e.g. depot/OMSI 2, and choose it in the launcher under Setup.\n\
 Mods: copy them into depot/Mods (they are installed when the launcher opens), or install\n\
-a folder or a .zip, .7z or .rar from the launcher's Mods page. Screenshots are written to depot/Screenshots.\n\
+a folder or a .zip from the launcher's Mods page. Screenshots are written to depot/Screenshots.\n\
 The folders here hold a .nomedia file so that the gallery leaves the game's textures alone:\n\
 they are not photos - deleting them breaks buses and maps.\n";
 
diff --git a/crates/omsi-app/src/launcher/mobile.rs b/crates/omsi-app/src/launcher/mobile.rs
index 401a583..e8116df 100644
--- a/crates/omsi-app/src/launcher/mobile.rs
+++ b/crates/omsi-app/src/launcher/mobile.rs
@@ -48,7 +48,7 @@ pub enum Purpose {
     Root,
     /// A mod as a folder.
     ModFolder,
-    /// A mod archive (.zip, .7z or .rar).
+    /// A mod archive (.zip: used in place, never unpacked).
     ModZip,
 }
 
@@ -113,7 +113,7 @@ impl Browser {
                         continue;
                     }
                     let Ok(m) = e.metadata() else { continue };
-                    let archive = [".zip", ".7z", ".rar"].iter().any(|ext| name.to_ascii_lowercase().ends_with(ext));
+                    let archive = name.to_ascii_lowercase().ends_with(".zip");
                     if m.is_dir() || (archive && self.purpose == Purpose::ModZip) {
                         self.entries.push((name, m.is_dir(), m.len()));
                     }
@@ -136,7 +136,7 @@ impl Browser {
         match self.purpose {
             Purpose::Root => "Choose the OMSI 2 folder",
             Purpose::ModFolder => "Choose the mod folder",
-            Purpose::ModZip => "Choose a mod archive (.zip, .7z, .rar)",
+            Purpose::ModZip => "Choose a mod archive (.zip)",
         }
     }
 }
@@ -299,7 +299,7 @@ impl Launcher {
                 }
             }
             Purpose::ModZip => {
-                self.ui.text_in("Tap a .zip, .7z or .rar to install it", Rect::new(foot.x, foot.y, foot.w, foot.h), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
+                self.ui.text_in("Tap a .zip to install it", Rect::new(foot.x, foot.y, foot.w, foot.h), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
             }
         }
         // the folder's contents
diff --git a/crates/omsi-app/src/launcher/pages.rs b/crates/omsi-app/src/launcher/pages.rs
index 4bc0960..e1729f8 100644
--- a/crates/omsi-app/src/launcher/pages.rs
+++ b/crates/omsi-app/src/launcher/pages.rs
@@ -2325,7 +2325,7 @@ pub fn mods(l: &mut Launcher, area: Rect) {
     if !l.state.mods_asked {
         l.state.load_mods();
     }
-    let body = l.page_title(area, "Mods", "A bus, a map, scenery, a whole OMSI folder - as a folder or a .zip, .7z or .rar. The original OMSI 2 folder is never written to.");
+    let body = l.page_title(area, "Mods", "A bus, a map, scenery, a whole OMSI folder - as a folder, or a .zip that the game reads in place from Archives. The original OMSI 2 folder is never written to.");
     let cols = 3;
     let cw = (body.w - GAP * 2.0 * (cols as f32 - 1.0)) / cols as f32;
     let colr = |k: usize| Rect::new(body.x + k as f32 * (cw + GAP * 2.0), body.y, cw, body.h);
@@ -2350,13 +2350,6 @@ pub fn mods(l: &mut Launcher, area: Rect) {
         }
     }
     y += 52.0;
-    l.ui.label(Rect::new(inner.x, y, inner.w, 20.0), "Archive install mode");
-    y += 22.0;
-    let mut m = l.state.mod_mode;
-    if l.ui.segmented("mod-mode", Rect::new(inner.x, y, inner.w, 34.0), &mut m, &["Auto", "Unpacked", "Used in place"]) {
-        l.state.mod_mode = m;
-    }
-    y += 44.0;
     let drop = Rect::new(inner.x, y, inner.w, 110.0);
     let hot = l.pages.drop_hover;
     let t = l.ui.anim(id_of("drop"), if hot { 1.0 } else { 0.0 }, 0.1);
@@ -2378,16 +2371,15 @@ pub fn mods(l: &mut Launcher, area: Rect) {
         l.ui.p().circle(p, 1.3, ACCENT.alpha(0.35 + 0.5 * t));
     }
     l.ui.icon("upload", Vec2::new(drop.center().x, drop.y + 38.0), 30.0, ACCENT.alpha(0.6 + 0.4 * t));
-    l.ui.text_in("…or drop a mod folder or .zip, .7z or .rar onto this window", Rect::new(drop.x, drop.y + 62.0, drop.w, 30.0), 12.5, Weight::Medium, TEXT_SOFT, Align::Center);
+    l.ui.text_in("…or drop a mod folder or .zip onto this window", Rect::new(drop.x, drop.y + 62.0, drop.w, 30.0), 12.5, Weight::Medium, TEXT_SOFT, Align::Center);
     y += 122.0;
     if !l.state.mod_path.is_empty() {
         let p = l.state.mod_path.clone();
         y += l.ui.paragraph(&p, Vec2::new(inner.x, y), inner.w, 11.5, Weight::Regular, TEXT_FAINT);
         match l.state.mod_info.clone() {
             Some(Ok(i)) if i.is_archive => {
-                let fit = if i.fits { format!("fits ({} free)", fmt_bytes(i.free_bytes)) } else { format!("does not fit: needs {}, {} free", fmt_bytes(i.needed_bytes), fmt_bytes(i.free_bytes)) };
                 let place = if i.in_place_ok { "can be used in place".to_string() } else { i.in_place.clone() };
-                y += l.ui.paragraph(&format!("{} archive, {} files, {} unpacked - {fit}; {place}", fmt_bytes(i.archive_bytes), i.files, fmt_bytes(i.unpacked_bytes)), Vec2::new(inner.x, y), inner.w, 12.0, Weight::Regular, if i.fits { TEXT_DIM } else { WARN });
+                y += l.ui.paragraph(&format!("{} archive, {} files - {place}", fmt_bytes(i.archive_bytes), i.files), Vec2::new(inner.x, y), inner.w, 12.0, Weight::Regular, if i.in_place_ok { TEXT_DIM } else { WARN });
             }
             Some(Err(e)) => {
                 y += l.ui.paragraph(&e, Vec2::new(inner.x, y), inner.w, 12.0, Weight::Regular, DANGER);
diff --git a/crates/omsi-app/src/launcher/state.rs b/crates/omsi-app/src/launcher/state.rs
index 2349b5e..c8aa885 100644
--- a/crates/omsi-app/src/launcher/state.rs
+++ b/crates/omsi-app/src/launcher/state.rs
@@ -238,7 +238,6 @@ pub struct State {
     pub mods_asked: bool,
     pub mod_info: Option<Result<core::install::SourceInfo, String>>,
     pub mod_path: String,
-    pub mod_mode: usize,
     pub ibis: Option<(String, Result<core::IbisInfo, String>)>,
     pub cmdline: String,
     pub join: (bool, String),
@@ -309,7 +308,6 @@ impl State {
             mods_asked: false,
             mod_info: None,
             mod_path: String::new(),
-            mod_mode: 0,
             ibis: None,
             cmdline: String::new(),
             join: (true, String::new()),
@@ -601,7 +599,8 @@ impl State {
     }
 
     pub fn install(&mut self, path: String) {
-        let mode = ["auto", "extract", "inplace"][self.mod_mode.min(2)].to_string();
+        // (a .zip is only ever used in place, from Archives; a folder is copied)
+        let mode = "inplace".to_string();
         self.mod_path = path.clone();
         let p2 = path.clone();
         self.spawn(move || Msg::ModInfo(core::inspect_mod(std::path::Path::new(&p2)).map_err(|e| format!("{e:#}"))));
@@ -1071,12 +1070,6 @@ impl State {
             }
             Msg::Mods(Err(e)) => self.set_status(e, true),
             Msg::ModInfo(r) => {
-                if let Ok(i) = &r {
-                    // a big archive that does not fit is used in place
-                    if !i.fits && i.in_place_ok && self.mod_mode == 1 {
-                        self.mod_mode = 2;
-                    }
-                }
                 self.mod_info = Some(r);
             }
             Msg::Installed(Ok(j)) => {
diff --git a/crates/omsi-app/src/lib.rs b/crates/omsi-app/src/lib.rs
index fdfbd5e..36aa35b 100644
--- a/crates/omsi-app/src/lib.rs
+++ b/crates/omsi-app/src/lib.rs
@@ -443,7 +443,6 @@ pub(crate) fn make_app(mut args: Args, server_cfg: Option<server::ServerCfg>) ->
     let clock_note = args.clock_moved.clone();
     // (as the last session left it, #1164)
     let info_bar = settings.info_bar;
-    let is_server = args.server.is_some();
     let mut app = App {
         args,
         instance: graphics_instance(),
diff --git a/crates/omsi-launcher-core/Cargo.toml b/crates/omsi-launcher-core/Cargo.toml
index eca665a..b8dbc29 100644
--- a/crates/omsi-launcher-core/Cargo.toml
+++ b/crates/omsi-launcher-core/Cargo.toml
@@ -22,8 +22,6 @@ serde_json.workspace = true
 anyhow.workspace = true
 rayon.workspace = true
 zip.workspace = true
-sevenz-rust2 = { version = "0.22", default-features = false, features = ["util", "deflate"] }
-unrar-rs = { version = "0.10", default-features = false, features = ["crypto-rust"] }
 omsi-cfg = { path = "../omsi-cfg" }
 omsi-map = { path = "../omsi-map" }
 omsi-vehicle = { path = "../omsi-vehicle" }
diff --git a/crates/omsi-launcher-core/src/install.rs b/crates/omsi-launcher-core/src/install.rs
index 99dee96..8e5016f 100644
--- a/crates/omsi-launcher-core/src/install.rs
+++ b/crates/omsi-launcher-core/src/install.rs
@@ -1,6 +1,9 @@
 //! Installing mods as background jobs.
 //!
-//! A mod is a folder or a `.zip`. Installing it happens in four steps, each of which can be
+//! A mod is a folder or a `.zip`. **A `.zip` is never unpacked** (and `.rar` / `.7z` are not
+//! supported): it is put into `<content>/Archives/` as it is, and the game mounts every
+//! archive there as a content root (`omsi_cfg::vfs`), as the launcher's own lists do. A folder
+//! is copied into the content folder. Installing happens in four steps, each of which can be
 //! followed on the Mods page and cancelled:
 //!
 //! 1. **Plan** - from the archive's central directory (or the folder listing) alone, work
@@ -8,14 +11,18 @@
 //!    merged into the same folders of the content folder, a lone bus / map / object folder is
 //!    put under the right folder, and everything else (read-mes, screenshots) is left out.
 //!    A pack that only adds paints or textures to a bus that is not installed is kept aside
-//!    in `Mods/waiting` and installed by itself once the bus is there.
-//! 2. **Check** - the unpacked size of the planned files plus a margin must fit on the disk
-//!    of the content folder; otherwise the job stops with the numbers before anything is
+//!    in `Mods/waiting` and installed by itself once the bus is there. A `.zip` whose folders
+//!    are not where the game looks for them (a lone bus folder, say) is refused: it would
+//!    have to be unpacked.
+//! 2. **Check** - for a folder, the size of the planned files plus a margin must fit on the
+//!    disk of the content folder; otherwise the job stops with the numbers before anything is
 //!    written.
-//! 3. **Unpack / copy** into `<content>/.install-staging/<pid>-<job>`, on the same volume
+//! 3. **Copy** a folder into `<content>/.install-staging/<pid>-<job>`, on the same volume
 //!    as the content folder, with the free space watched while it runs. The files of a
 //!    folder in the Mods inbox are hard-linked there instead (no space, no time), so the
-//!    staging folder never holds the only copy of anything.
+//!    staging folder never holds the only copy of anything. A `.zip` is placed in `Archives`
+//!    instead - hard-linked when it is on the same disk, moved when it came through the Mods
+//!    inbox, else copied through the staging folder after the same free-space check.
 //! 4. **Move** into place: a new folder is renamed into the content folder in one step,
 //!    files for an existing folder are renamed over it one by one. Only then are the
 //!    linked inbox files removed from the inbox.
@@ -27,14 +34,8 @@
 //! Every path inside a source goes through `safe_rel` before it is planned, so no entry
 //! (`..\..\x` in a zip made on Windows included) can be written outside the staging folder.
 //!
-//! **In place** (`InstallMode::InPlace`): a `.zip` laid out like OMSI 2 is not unpacked at
-//! all. It is put into `<content>/Archives/` - hard-linked when it is on the same disk
-//! (no space, no time), moved when it came through the Mods inbox, else copied through the
-//! staging folder after the same free-space check - and the game mounts every archive
-//! there as a content root (`omsi_cfg::vfs`), as the launcher's own lists do. The plan is
-//! still made, for the report and to check that the archive's folders are where the game
-//! looks for them. `InstallMode::Auto` (the page's default) unpacks what fits on the disk
-//! and uses an archive in place when its unpacked size does not.
+//! `InstallMode` is kept for compatibility with the CLI and the page, but no longer changes
+//! anything: a `.zip` is always used in place and a folder is always copied.
 
 use anyhow::{anyhow, Context, Result};
 use serde::Serialize;
@@ -59,12 +60,13 @@ pub const ARCHIVES: &str = "Archives";
 #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
 #[serde(rename_all = "lowercase")]
 pub enum InstallMode {
-    /// Unpack (or copy) the files into the content folder.
+    /// Copy a folder's files into the content folder (the only way for a folder).
     #[default]
     Extract,
-    /// Put the archive into `<content>/Archives` and let the game read it as a content root.
+    /// Put the archive into `<content>/Archives` and let the game read it as a content root
+    /// (the only way for a `.zip`).
     InPlace,
-    /// Unpack when the unpacked files fit on the disk, else use the archive in place.
+    /// Kept for compatibility: the same as the source's only way (a `.zip` in place, a folder copied).
     Auto,
 }
 
@@ -724,8 +726,6 @@ fn kind_label(kind: &str) -> &'static str {
 #[derive(Clone, Copy, PartialEq, Eq)]
 enum ArchiveKind {
     Zip,
-    SevenZip,
-    Rar,
 }
 
 fn archive_kind(path: &Path) -> Option<ArchiveKind> {
@@ -734,113 +734,10 @@ fn archive_kind(path: &Path) -> Option<ArchiveKind> {
     }
     match path.extension()?.to_string_lossy().to_ascii_lowercase().as_str() {
         "zip" => Some(ArchiveKind::Zip),
-        "7z" => Some(ArchiveKind::SevenZip),
-        "rar" => Some(ArchiveKind::Rar),
         _ => None,
     }
 }
 
-/// Expand formats the game cannot mount into the install job's own staging area. Header
-/// sizes are checked against the content volume before any data is written; the later
-/// install step hard-links these staged files into their final layout.
-fn unpack_archive(job: &Job, content: &Path, src: &Path, dest: &Path, kind: ArchiveKind) -> Result<()> {
-    let (files, bytes) = match kind {
-        ArchiveKind::SevenZip => {
-            let reader = sevenz_rust2::ArchiveReader::open(src, sevenz_rust2::Password::empty())
-                .with_context(|| format!("{} is not a readable 7z archive", src.display()))?;
-            let entries = &reader.archive().files;
-            let files = entries.iter().filter(|e| e.has_stream && !e.is_directory).count() as u64;
-            let bytes = entries.iter().filter(|e| e.has_stream && !e.is_directory).fold(0u64, |sum, e| sum.saturating_add(e.size));
-            (files, bytes)
-        }
-        ArchiveKind::Rar => {
-            let archive = unrar_rs::RarArchive::open(std::fs::File::open(src)?)
-                .with_context(|| format!("{} is not a readable RAR archive", src.display()))?;
-            let members: Vec<_> = archive.entries().collect();
-            if members.iter().any(|m| m.unpacked_size.is_none() && !m.is_directory) {
-                return Err(anyhow!("{} is a multi-volume RAR archive; add all parts and start with the first .rar file", src.display()));
-            }
-            let files = members.iter().filter(|m| !m.is_directory && !m.is_symlink && !m.is_hardlink).count() as u64;
-            let bytes = members.iter().filter(|m| !m.is_directory && !m.is_symlink && !m.is_hardlink).fold(0u64, |sum, m| sum.saturating_add(m.unpacked_size.unwrap_or(0)));
-            (files, bytes)
-        }
-        ArchiveKind::Zip => unreachable!(),
-    };
-    let free = free_space(content).unwrap_or(u64::MAX);
-    let needed = bytes.saturating_add(MIN_MARGIN);
-    job.set(|p| {
-        p.files_total = files;
-        p.bytes_total = bytes;
-        p.free_bytes = free;
-        p.needed_bytes = needed;
-    });
-    if needed > free {
-        return Err(anyhow!("not enough disk space to unpack {}: {} plus {} kept free needs {}, but only {} is free on {}", src.display(), gb(bytes), gb(MIN_MARGIN), gb(needed), gb(free), content.display()));
-    }
-    if job.cancelled() {
-        return Err(anyhow!(Cancelled));
-    }
-    std::fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
-    match kind {
-        ArchiveKind::SevenZip => {
-            let mut done = 0u64;
-            sevenz_rust2::decompress_file_with_extract_fn(src, dest, |entry, reader, target| {
-                if job.cancelled() {
-                    return Err(sevenz_rust2::Error::Io(std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled"), "install cancelled".into()));
-                }
-                let wrote = sevenz_rust2::default_entry_extract_fn(entry, reader, target)?;
-                if wrote && entry.has_stream && !entry.is_directory {
-                    done += 1;
-                    job.files_done.store(done, Ordering::Relaxed);
-                    if free_space(content).is_some_and(|left| left < ABORT_BELOW) {
-                        return Err(sevenz_rust2::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, "free disk space fell below 1 GB"), "install stopped".into()));
-                    }
-                }
-                Ok(wrote)
-            }).with_context(|| format!("unpacking {} (the archive may be damaged or encrypted)", src.display()))?;
-        }
-        ArchiveKind::Rar => {
-            let mut archive = unrar_rs::RarArchive::open(std::fs::File::open(src)?)
-                .with_context(|| format!("opening {}", src.display()))?;
-            let members: Vec<_> = archive.entries().collect();
-            for (index, member) in members.iter().enumerate() {
-                if job.cancelled() {
-                    return Err(anyhow!(Cancelled));
-                }
-                if member.is_directory || member.is_symlink || member.is_hardlink {
-                    continue;
-                }
-                let Some(rel) = safe_rel(&member.name) else {
-                    job.set(|p| p.warnings.push(format!("unsafe RAR path refused: {}", member.name)));
-                    continue;
-                };
-                let out = dest.join(&rel);
-                if let Some(parent) = out.parent() {
-                    std::fs::create_dir_all(parent)?;
-                }
-                let mut file = std::fs::File::create(&out)?;
-                let n = archive.by_index(index)?.copy_to(&mut file)
-                    .with_context(|| format!("unpacking {} from {}", rel, src.display()))?;
-                job.files_done.fetch_add(1, Ordering::Relaxed);
-                job.bytes_done.fetch_add(n, Ordering::Relaxed);
-                check_space(content)?;
-            }
-        }
-        ArchiveKind::Zip => unreachable!(),
-    }
-    if job.cancelled() {
-        return Err(anyhow!(Cancelled));
-    }
-    Ok(())
-}
-
-enum Source {
-    /// A folder's files as they are on disk, by `Entry::index` (the name in `Entry::rel`
-    /// may differ: `\` read as a separator).
-    Folder(Vec<PathBuf>),
-    Zip(zip::ZipArchive<std::io::BufReader<std::fs::File>>),
-}
-
 /// The files of a folder, with their paths on disk, and how many were left out because
 /// their names cannot be installed safely.
 fn list_folder(dir: &Path, job: &Job) -> Result<(Vec<Entry>, Vec<PathBuf>, usize)> {
@@ -948,25 +845,15 @@ fn run(job: &Job, content: &Path, root: Option<&Path>) -> Result<()> {
     let src = job.source.clone();
     let archive = archive_kind(&src);
     let is_zip = archive == Some(ArchiveKind::Zip);
-    let staged_archive = matches!(archive, Some(ArchiveKind::SevenZip | ArchiveKind::Rar));
     if archive.is_none() && !src.is_dir() {
-        return Err(anyhow!("{} is neither a folder nor a supported mod archive (.zip, .7z, .rar)", src.display()));
+        return Err(anyhow!("{} is neither a folder nor a .zip: only folders and .zip archives (used in place, never unpacked) are supported; .rar and .7z are not", src.display()));
     }
     if src.starts_with(content) && !job.from_inbox && !src.starts_with(content.join("Mods")) {
         return Err(anyhow!("{} is already inside the content folder", src.display()));
     }
     // (the name becomes a folder name: `...zip` has the stem `..`)
     let source_name = if archive.is_some() { src.file_stem() } else { src.file_name() }.map(|s| s.to_string_lossy().to_string()).filter(|n| safe_rel(n).as_deref() == Some(n.as_str())).unwrap_or_else(|| "mod".into());
-    let (mut source, entries) = if staged_archive {
-        let unpacked = staging_dir(content, job.id).join("source");
-        job.state("unpacking", "checking the archive and unpacking it into staging");
-        unpack_archive(job, content, &src, &unpacked, archive.unwrap())?;
-        let (entries, paths, refused) = list_folder(&unpacked, job)?;
-        if refused > 0 {
-            job.set(|p| p.warnings.push(format!("{refused} file(s) whose names cannot be installed safely were left out")));
-        }
-        (Source::Folder(paths), entries)
-    } else if is_zip {
+    let (paths, entries) = if is_zip {
         job.state("planning", "reading the archive's table of contents");
         let f = std::fs::File::open(&src).with_context(|| format!("opening {}", src.display()))?;
         let mut z = zip::ZipArchive::new(std::io::BufReader::with_capacity(ZIP_BUFFER, f)).with_context(|| format!("{} is not a readable zip archive", src.display()))?;
@@ -974,14 +861,16 @@ fn run(job: &Job, content: &Path, root: Option<&Path>) -> Result<()> {
         if !problems.is_empty() {
             return Err(anyhow!("{}: {}", src.display(), problems.join("; ")));
         }
-        (Source::Zip(z), entries)
+        // (a zip is never read past its table of contents: it is placed in Archives as it is)
+        drop(z);
+        (Vec::new(), entries)
     } else {
         job.state("planning", "listing the folder");
         let (entries, paths, refused) = list_folder(&src, job)?;
         if refused > 0 {
             job.set(|p| p.warnings.push(format!("{refused} file(s) whose names cannot be installed safely (a '..', '\\' or ':' in them) were left out")));
         }
-        (Source::Folder(paths), entries)
+        (paths, entries)
     };
     if entries.is_empty() {
         return Err(anyhow!("{} is empty", src.display()));
@@ -1005,7 +894,7 @@ fn run(job: &Job, content: &Path, root: Option<&Path>) -> Result<()> {
     let total_bytes: u64 = plan.maps.iter().map(|m| m.bytes).sum();
     // an inbox folder's files are hard-linked into the staging folder (no space, and the
     // originals stay until the install is in place); everything else is written anew
-    let moving = (job.from_inbox && !is_zip) || staged_archive;
+    let moving = job.from_inbox && !is_zip;
     let needed = if moving { 0 } else { total_bytes + (total_bytes / 20).max(MIN_MARGIN) };
     let free = free_space(content).unwrap_or(u64::MAX);
     job.set(|p| {
@@ -1014,30 +903,12 @@ fn run(job: &Job, content: &Path, root: Option<&Path>) -> Result<()> {
         p.free_bytes = free;
         p.needed_bytes = needed;
     });
-    // unpack, or use the archive in place?
-    let layout = if is_zip { in_place_layout(&src, &plan) } else { Err(anyhow!("only a .zip archive can be used in place; this archive must be unpacked")) };
-    let mode = if staged_archive {
-        InstallMode::Extract
-    } else { match job.mode {
-        InstallMode::Auto if is_zip && needed > free => match &layout {
-            Ok(()) => {
-                let line = format!("{} unpacks to {}, which does not fit ({} free): using the archive in place instead", source_name, gb(total_bytes), gb(free));
-                job.set(|p| p.report.push(line.clone()));
-                InstallMode::InPlace
-            }
-            Err(e) => {
-                let line = format!("the archive cannot be used in place either: {e:#}");
-                job.set(|p| p.warnings.push(line.clone()));
-                InstallMode::Extract
-            }
-        },
-        InstallMode::Auto => InstallMode::Extract,
-        m => m,
-    }};
+    // a .zip is only ever used in place (the game mounts it from Archives); a folder is copied
+    let mode = if is_zip { InstallMode::InPlace } else { InstallMode::Extract };
+    let layout = if is_zip { in_place_layout(&src, &plan) } else { Ok(()) };
     job.set(|p| p.mode = mode);
     if mode == InstallMode::InPlace {
         layout?;
-        drop(source);
         return place_archive(job, content, &src, &plan);
     }
     if needed > free {
@@ -1055,7 +926,7 @@ fn run(job: &Job, content: &Path, root: Option<&Path>) -> Result<()> {
     // 3. unpack / copy into the staging folder
     let staging = staging_dir(content, job.id);
     std::fs::create_dir_all(&staging).with_context(|| format!("creating {}", staging.display()))?;
-    job.state(if is_zip { "unpacking" } else if moving { "moving" } else { "copying" }, format!("{} files, {}", total_files, gb(total_bytes)));
+    job.state(if moving { "moving" } else { "copying" }, format!("{} files, {}", total_files, gb(total_bytes)));
     let mut buf = vec![0u8; 1 << 20];
     let mut since_check = 0u64;
     for (mi, m) in plan.maps.iter().enumerate() {
@@ -1075,51 +946,27 @@ fn run(job: &Job, content: &Path, root: Option<&Path>) -> Result<()> {
                 std::fs::remove_file(&out).with_context(|| format!("replacing {}", out.display()))?;
             }
             let e = &entries[*idx];
-            match &mut source {
-                Source::Zip(z) => {
-                    let mut f = z.by_index(e.index).with_context(|| format!("reading {} from the archive", e.rel))?;
-                    let mut w = std::fs::File::create(&out).with_context(|| format!("writing {}", out.display()))?;
-                    loop {
-                        let n = f.read(&mut buf).with_context(|| format!("unpacking {} (the archive may be damaged)", e.rel))?;
-                        if n == 0 {
-                            break;
-                        }
-                        w.write_all(&buf[..n]).with_context(|| format!("writing {}", out.display()))?;
-                        job.bytes_done.fetch_add(n as u64, Ordering::Relaxed);
-                        since_check += n as u64;
-                        if since_check > 64 << 20 {
-                            since_check = 0;
-                            if job.cancelled() {
-                                return Err(anyhow!(Cancelled));
-                            }
-                            check_space(content)?;
-                        }
+            let from = &paths[e.index];
+            if moving && std::fs::hard_link(from, &out).is_ok() {
+                job.linked.lock().unwrap().push(from.clone());
+                job.bytes_done.fetch_add(e.size, Ordering::Relaxed);
+            } else {
+                let mut r = std::fs::File::open(from).with_context(|| format!("reading {}", from.display()))?;
+                let mut w = std::fs::File::create(&out).with_context(|| format!("writing {}", out.display()))?;
+                loop {
+                    let n = r.read(&mut buf)?;
+                    if n == 0 {
+                        break;
                     }
-                }
-                Source::Folder(paths) => {
-                    let from = &paths[e.index];
-                    if moving && std::fs::hard_link(from, &out).is_ok() {
-                        job.linked.lock().unwrap().push(from.clone());
-                        job.bytes_done.fetch_add(e.size, Ordering::Relaxed);
-                    } else {
-                        let mut r = std::fs::File::open(from).with_context(|| format!("reading {}", from.display()))?;
-                        let mut w = std::fs::File::create(&out).with_context(|| format!("writing {}", out.display()))?;
-                        loop {
-                            let n = r.read(&mut buf)?;
-                            if n == 0 {
-                                break;
-                            }
-                            w.write_all(&buf[..n]).with_context(|| format!("writing {}", out.display()))?;
-                            job.bytes_done.fetch_add(n as u64, Ordering::Relaxed);
-                            since_check += n as u64;
-                            if since_check > 64 << 20 {
-                                since_check = 0;
-                                if job.cancelled() {
-                                    return Err(anyhow!(Cancelled));
-                                }
-                                check_space(content)?;
-                            }
+                    w.write_all(&buf[..n]).with_context(|| format!("writing {}", out.display()))?;
+                    job.bytes_done.fetch_add(n as u64, Ordering::Relaxed);
+                    since_check += n as u64;
+                    if since_check > 64 << 20 {
+                        since_check = 0;
+                        if job.cancelled() {
+                            return Err(anyhow!(Cancelled));
                         }
+                        check_space(content)?;
                     }
                 }
             }
@@ -1130,7 +977,6 @@ fn run(job: &Job, content: &Path, root: Option<&Path>) -> Result<()> {
             }
         }
     }
-    drop(source);
     if job.cancelled() {
         return Err(anyhow!(Cancelled));
     }
@@ -1293,7 +1139,7 @@ fn in_place_layout(src: &Path, plan: &Plan) -> Result<()> {
     if wrong.is_empty() {
         Ok(())
     } else {
-        Err(anyhow!("the archive is not laid out like an OMSI 2 folder, so it has to be unpacked: {}", wrong.join("; ")))
+        Err(anyhow!("the archive is not laid out like an OMSI 2 folder, and a .zip is only ever used in place from Archives (never unpacked): {}", wrong.join("; ")))
     }
 }
 
@@ -1424,34 +1270,10 @@ pub fn inspect(content: &Path, root: Option<&Path>, src: &Path) -> Result<Source
     let is_zip = archive == Some(ArchiveKind::Zip);
     let free = free_space(content).unwrap_or(u64::MAX);
     if archive.is_none() {
-        return Ok(SourceInfo { free_bytes: free, fits: true, in_place: "only a .zip archive can be used in place".into(), suggested: InstallMode::Extract, ..Default::default() });
+        return Ok(SourceInfo { free_bytes: free, fits: true, in_place: "only a .zip archive can be used (in place, from Archives); .rar and .7z are not supported".into(), suggested: InstallMode::Extract, ..Default::default() });
     }
     let job = Job { id: 0, source: src.to_path_buf(), mode: InstallMode::Auto, from_inbox: false, cancel: AtomicBool::new(false), progress: Mutex::new(Progress::default()), bytes_done: AtomicU64::new(0), files_done: AtomicU64::new(0), linked: Mutex::new(Vec::new()), stall_at: u64::MAX };
     let archive_bytes = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
-    if !is_zip {
-        let (files, unpacked) = match archive.unwrap() {
-            ArchiveKind::SevenZip => {
-                let a = sevenz_rust2::ArchiveReader::open(src, sevenz_rust2::Password::empty()).with_context(|| format!("{} is not a readable 7z archive", src.display()))?;
-                let entries = &a.archive().files;
-                (entries.iter().filter(|e| e.has_stream && !e.is_directory).count() as u64,
-                    entries.iter().filter(|e| e.has_stream && !e.is_directory).fold(0u64, |n, e| n.saturating_add(e.size)))
-            }
-            ArchiveKind::Rar => {
-                let a = unrar_rs::RarArchive::open(std::fs::File::open(src)?)
-                    .with_context(|| format!("{} is not a readable RAR archive", src.display()))?;
-                let members: Vec<_> = a.entries().collect();
-                let regular = members.iter().filter(|m| !m.is_directory && !m.is_symlink && !m.is_hardlink);
-                let entries: Vec<_> = regular.collect();
-                if entries.iter().any(|m| m.unpacked_size.is_none()) {
-                    return Err(anyhow!("multi-volume RAR archive; add all parts and start with the first .rar file"));
-                }
-                (entries.len() as u64, entries.iter().fold(0u64, |n, m| n.saturating_add(m.unpacked_size.unwrap_or(0))))
-            }
-            ArchiveKind::Zip => unreachable!(),
-        };
-        let needed = unpacked.saturating_add(MIN_MARGIN);
-        return Ok(SourceInfo { is_archive: true, is_zip: false, files, unpacked_bytes: unpacked, archive_bytes, needed_bytes: needed, free_bytes: free, fits: needed <= free, in_place: "7z and RAR archives must be unpacked".into(), in_place_ok: false, suggested: InstallMode::Extract });
-    }
     let f = std::fs::File::open(src).with_context(|| format!("opening {}", src.display()))?;
     let mut z = zip::ZipArchive::new(std::io::BufReader::with_capacity(ZIP_BUFFER, f)).with_context(|| format!("{} is not a readable zip archive", src.display()))?;
     let (entries, _) = list_zip(&mut z, &job)?;
@@ -1611,21 +1433,29 @@ mod tests {
     }
 
     #[test]
-    fn install_zip_moves_into_place_and_cleans_up() {
+    fn a_zip_is_never_unpacked() {
         let dir = tmp("zip");
         let content = dir.join("content");
         omsi_cfg::ensure_content_layout(&content).unwrap();
+        // a lone bus folder in a zip would have to be unpacked: it is refused instead
         let zip = dir.join("Foo Bus.zip");
         write_zip(&zip, &[("Foo/foo.bus", 100), ("Foo/Model/model.cfg", 2000), ("Foo/Texture/a.dds", 300_000), ("readme.txt", 10)]);
-        let p = run_blocking(content.clone(), None, zip.clone(), InstallMode::Extract, None, false);
-        assert_eq!(p.state, "done", "{p:?}");
-        assert_eq!(p.installed, vec!["Vehicles/Foo".to_string()]);
-        assert_eq!(std::fs::metadata(content.join("Vehicles/Foo/Texture/a.dds")).unwrap().len(), 300_000);
-        assert!(!content.join(STAGING).exists(), "staging removed");
-        // again: merged over the existing folder
-        let p = run_blocking(content.clone(), None, zip.clone(), InstallMode::Extract, None, false);
-        assert_eq!(p.state, "done", "{p:?}");
-        assert!(p.report.iter().any(|l| l.contains("3 existing file(s) replaced")), "{:?}", p.report);
+        for mode in [InstallMode::Extract, InstallMode::Auto, InstallMode::InPlace] {
+            let p = run_blocking(content.clone(), None, zip.clone(), mode, None, false);
+            assert_eq!(p.state, "failed", "{p:?}");
+            assert!(p.message.contains("never unpacked"), "{}", p.message);
+            assert!(!content.join("Vehicles/Foo").exists(), "nothing unpacked");
+            assert!(!content.join(ARCHIVES).join("Foo Bus.zip").exists());
+            assert!(!content.join(STAGING).exists(), "staging removed");
+        }
+        // .rar and .7z are not supported at all
+        for name in ["Pack.rar", "Pack.7z"] {
+            let src = dir.join(name);
+            std::fs::write(&src, b"not an archive the launcher reads").unwrap();
+            let p = run_blocking(content.clone(), None, src, InstallMode::Auto, None, false);
+            assert_eq!(p.state, "failed", "{p:?}");
+            assert!(p.message.contains("only folders and .zip"), "{}", p.message);
+        }
         let _ = std::fs::remove_dir_all(&dir);
     }
 
@@ -1652,16 +1482,16 @@ mod tests {
         // again: already there
         let p = run_blocking(content.clone(), None, zip.clone(), InstallMode::InPlace, None, false);
         assert_eq!(p.state, "done", "{p:?}");
-        // a lone bus folder cannot be read in place: explicit in-place fails, auto unpacks
+        // a lone bus folder cannot be read in place, and a zip is never unpacked: refused,
+        // whatever the mode asked for
         let lone = dir.join("Foo Bus.zip");
         write_zip(&lone, &[("Foo/foo.bus", 100), ("Foo/Texture/a.dds", 3000)]);
-        let p = run_blocking(content.clone(), None, lone.clone(), InstallMode::InPlace, None, false);
-        assert_eq!(p.state, "failed", "{p:?}");
-        assert!(p.message.contains("unpacked"), "{}", p.message);
-        let p = run_blocking(content.clone(), None, lone.clone(), InstallMode::Auto, None, false);
-        assert_eq!(p.state, "done", "{p:?}");
-        assert_eq!(p.mode, InstallMode::Extract);
-        assert!(content.join("Vehicles/Foo/foo.bus").exists());
+        for mode in [InstallMode::InPlace, InstallMode::Auto] {
+            let p = run_blocking(content.clone(), None, lone.clone(), mode, None, false);
+            assert_eq!(p.state, "failed", "{p:?}");
+            assert!(p.message.contains("never unpacked"), "{}", p.message);
+        }
+        assert!(!content.join("Vehicles/Foo/foo.bus").exists());
         // what the page is told before
         let info = inspect(&content, None, &zip).unwrap();
         assert!(info.is_zip && info.in_place_ok && info.unpacked_bytes == 2100, "{info:?}");
@@ -1675,9 +1505,13 @@ mod tests {
         let dir = tmp("plugins");
         let content = dir.join("content");
         omsi_cfg::ensure_content_layout(&content).unwrap();
-        let zip = dir.join("Pack.zip");
-        write_zip(&zip, &[("Vehicles/Foo/foo.bus", 10), ("Plugins/evil.dll", 100), ("Plugins/evil.opl", 10)]);
-        let p = run_blocking(content.clone(), None, zip, InstallMode::Extract, None, false);
+        let pack = dir.join("Pack");
+        for (name, size) in [("Vehicles/Foo/foo.bus", 10), ("Plugins/evil.dll", 100), ("Plugins/evil.opl", 10)] {
+            let f = pack.join(name);
+            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
+            std::fs::write(&f, vec![1u8; size]).unwrap();
+        }
+        let p = run_blocking(content.clone(), None, pack, InstallMode::Extract, None, false);
         assert_eq!(p.state, "done", "{p:?}");
         assert!(content.join("Vehicles/Foo/foo.bus").exists());
         assert!(!content.join("Plugins/evil.dll").exists(), "never where the game loads plugins");
@@ -1704,15 +1538,17 @@ mod tests {
         let dir = tmp("cancel");
         let content = dir.join("content");
         omsi_cfg::ensure_content_layout(&content).unwrap();
-        let zip = dir.join("Big.zip");
-        let files: Vec<(String, usize)> = (0..4).map(|i| (format!("Vehicles/Big/Texture/t{i}.dds"), 100)).collect();
-        let refs: Vec<(&str, usize)> = files.iter().map(|(n, s)| (n.as_str(), *s)).collect();
-        let mut with_bus = refs.clone();
-        with_bus.push(("Vehicles/Big/big.bus", 10));
-        write_zip(&zip, &with_bus);
+        let big = dir.join("Big");
+        let mut names: Vec<String> = (0..4).map(|i| format!("Vehicles/Big/Texture/t{i}.dds")).collect();
+        names.push("Vehicles/Big/big.bus".to_string());
+        for name in &names {
+            let f = big.join(name);
+            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
+            std::fs::write(&f, [1u8; 100]).unwrap();
+        }
         // Pause after a file is staged: a fast disk must not finish before the test
         // requests cancellation, and cancellation must clean up actual partial files.
-        let job = start_inner(content.clone(), None, zip, InstallMode::Extract, false, 1);
+        let job = start_inner(content.clone(), None, big, InstallMode::Extract, false, 1);
         wait_for(&job, 1);
         assert_eq!(job.snapshot().files_done, 1);
         assert!(staging_dir(&content, job.id).exists());
@@ -1868,28 +1704,28 @@ mod tests {
         let dir = tmp("zipslip");
         let content = dir.join("content");
         omsi_cfg::ensure_content_layout(&content).unwrap();
-        // files are unpacked into dir/content/.install-staging/<pid>-<job>/0: four levels up
-        // is `dir`, so this name would land in `dir/evil.txt`
+        // a name that climbs out of the archive is refused before anything is placed
         let zip = dir.join("Evil Map.zip");
         write_zip(&zip, &[("global.cfg", 10), ("..\\..\\..\\..\\evil.txt", 10), ("tile_0_0.map", 10)]);
-        let p = run_blocking(content.clone(), None, zip, InstallMode::Extract, None, false);
+        let p = run_blocking(content.clone(), None, zip, InstallMode::InPlace, None, false);
         assert_eq!(p.state, "failed", "{p:?}");
         assert!(p.message.contains("refused"), "{}", p.message);
-        assert!(!dir.join("evil.txt").exists(), "nothing written outside the staging folder");
-        assert!(!content.join("maps/Evil Map").exists());
+        assert!(!dir.join("evil.txt").exists());
+        assert!(!content.join(ARCHIVES).join("Evil Map.zip").exists());
         assert!(!content.join(STAGING).exists());
         // the same inside a bus folder
         let zip = dir.join("Evil Bus.zip");
         write_zip(&zip, &[("Vehicles/Foo/foo.bus", 10), ("Vehicles/Foo/..\\..\\..\\..\\evil.txt", 10)]);
-        let p = run_blocking(content.clone(), None, zip, InstallMode::Extract, None, false);
+        let p = run_blocking(content.clone(), None, zip, InstallMode::InPlace, None, false);
         assert_eq!(p.state, "failed", "{p:?}");
         assert!(!dir.join("evil.txt").exists());
-        // a zip made on Windows with `\` separators still installs
+        // a zip made on Windows with `\` separators is used in place as it is
         let zip = dir.join("Win Bus.zip");
         write_zip(&zip, &[("Vehicles\\Bar\\bar.bus", 10), ("Vehicles\\Bar\\Texture\\a.dds", 100)]);
-        let p = run_blocking(content.clone(), None, zip, InstallMode::Extract, None, false);
+        let p = run_blocking(content.clone(), None, zip, InstallMode::InPlace, None, false);
         assert_eq!(p.state, "done", "{p:?}");
-        assert_eq!(std::fs::metadata(content.join("Vehicles/Bar/Texture/a.dds")).unwrap().len(), 100);
+        assert!(content.join(ARCHIVES).join("Win Bus.zip").exists());
+        assert!(!content.join("Vehicles/Bar").exists(), "nothing unpacked");
         let _ = std::fs::remove_dir_all(&dir);
     }
 
diff --git a/crates/omsi-launcher-core/src/lib.rs b/crates/omsi-launcher-core/src/lib.rs
index 44ebc16..0ba41e8 100644
--- a/crates/omsi-launcher-core/src/lib.rs
+++ b/crates/omsi-launcher-core/src/lib.rs
@@ -440,10 +440,9 @@ pub struct ModsStatus {
     pub jobs: Vec<install::Progress>,
 }
 
-/// Start installing the mod at `src` (a folder, .zip, .7z or .rar) into the content folder in the
-/// background; `mode` is "auto" (unpack, or use a .zip in place when it does not fit; .7z
-/// and .rar are always unpacked),
-/// "extract" or "inplace".
+/// Start installing the mod at `src` (a folder, or a .zip) into the content folder in the
+/// background. A folder is copied; a .zip is never unpacked, only put into `Archives` and read
+/// in place (`mode` is kept for compatibility and ignored: .rar and .7z are not supported).
 pub fn start_install(src: &Path, mode: &str) -> Result<install::Progress> {
     let content = content_dir().ok_or_else(|| anyhow!("no game binary configured, so no content folder"))?;
     if !src.exists() {
@@ -454,7 +453,7 @@ pub fn start_install(src: &Path, mode: &str) -> Result<install::Progress> {
     Ok(job.snapshot())
 }
 
-/// How big the mod at `src` is unpacked, what is free, and whether it can be used in place.
+/// What the mod at `src` holds, what is free, and whether a .zip can be used in place.
 pub fn inspect_mod(src: &Path) -> Result<install::SourceInfo> {
     let content = content_dir().ok_or_else(|| anyhow!("no game binary configured, so no content folder"))?;
     install::inspect(&content, root().ok().as_deref(), src)
@@ -475,7 +474,7 @@ fn inbox_entries(content: &Path) -> Vec<PathBuf> {
         .filter(|p| {
             let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
             !(name.starts_with('.') || name.eq_ignore_ascii_case("installed") || name.eq_ignore_ascii_case(install::WAITING) || name.eq_ignore_ascii_case(install::PLUGINS_HELD) || name.eq_ignore_ascii_case(install::UNINSTALLED) || name.eq_ignore_ascii_case("README.txt"))
-                && (p.is_dir() || p.extension().map(|x| ["zip", "7z", "rar"].iter().any(|ext| x.eq_ignore_ascii_case(ext))).unwrap_or(false))
+                && (p.is_dir() || p.extension().map(|x| x.eq_ignore_ascii_case("zip")).unwrap_or(false))
         })
         .collect();
     v.sort();
@@ -2772,7 +2771,7 @@ pub fn pick_mod(zip: bool) -> Option<PathBuf> {
     #[cfg(not(target_os = "android"))]
     {
         if zip {
-            rfd::FileDialog::new().set_title("Choose a mod archive").add_filter("Mod archive", &["zip", "7z", "rar"]).pick_file()
+            rfd::FileDialog::new().set_title("Choose a mod archive").add_filter("Mod archive", &["zip"]).pick_file()
         } else {
             rfd::FileDialog::new().set_title("Choose the mod folder").pick_folder()
         }
diff --git a/docs/ANDROID.md b/docs/ANDROID.md
index a446a69..f852f25 100644
--- a/docs/ANDROID.md
+++ b/docs/ANDROID.md
@@ -58,7 +58,7 @@ gives, so a mod bus is driven by them as by the keyboard.
    cable, from a PC or a USB stick). Project Depot finds it by itself in `depot/`,
    `Download/` or the top of the storage; anywhere else choose it in the launcher under
    **Setup → Browse → Use this folder → Save**.
-4. **Mods**: copy mod folders or .zip, .7z and .rar files into `depot/Mods` (installed when the launcher
+4. **Mods**: copy mod folders or .zip files into `depot/Mods` (installed when the launcher
    opens), or install them from the launcher's **Mods** page (Choose a folder / Choose an
    archive); .zip archives can also lie in `depot/Archives` and are used in place. Maps and buses
    work exactly as on the computer.
diff --git a/docs/USER_GUIDE.md b/docs/USER_GUIDE.md
index 439c32d..a6adbc6 100644
--- a/docs/USER_GUIDE.md
+++ b/docs/USER_GUIDE.md
@@ -226,7 +226,7 @@ again only when something changes; drag on it to turn the bus, scroll to zoom. I
   it save its run (SIGTERM, up to 8 s, and only a stuck game is killed) and, for a LAN
   session, the code to copy, who is playing and the chat.
 * **Mods** - installing mods and archives (see *Mods and the content folder*); a folder or
-  .zip, .7z or .rar dropped on the window is installed.
+  .zip dropped on the window is installed (a .zip is never unpacked: it goes to `Archives`).
 * **Timetable** - a map's lines, their tours and trips. Changes stay while you move between
   lines and are saved together (*Save all*); **New line** makes a line, **Repeat** turns a tour
   into a whole day of them (every *n* minutes up to a last departure).
@@ -464,26 +464,26 @@ vehicle folder and `HOFs/` contain the same file name, the vehicle's own copy ta
 priority (the launcher's depot list shows the shared ones after the bus's own).
 
 Installing a mod: the launcher's **Mods** page opens the system's folder / file picker
-(Finder, Explorer, GTK) for a mod folder or a `.zip`, `.7z` or `.rar` archive and sorts it
+(Finder, Explorer, GTK) for a mod folder or a `.zip` archive and sorts it
 into place (OMSI-style folders anywhere inside are merged; a lone bus, map, object or
 spline folder is recognised by its `.bus` / `global.cfg` / `.sco` / `.sli` files and put
 under the right folder), or drop it into `Mods/` next to the binary and open the page.
-`openomsi-launcher --cli install '{"path":"/path/to/mod.7z"}'` and `--cli mods` do the same
-from a shell. An installation is a background job: the archive's table of contents becomes
-a plan, the disk is checked for room, everything is unpacked into a staging folder on the
-content volume and moved into place in one step, and it can be cancelled and cleaned up at
-any point. A repaint for a bus that is not installed is kept aside and installed when the
+`openomsi-launcher --cli install '{"path":"/path/to/mod.zip"}'` and `--cli mods` do the same
+from a shell. **`.rar` and `.7z` archives are not supported** (unpack them with your own tool
+and install the folder), and **a `.zip` is never unpacked by the launcher**. An installation is
+a background job: a folder's listing (or a `.zip`'s table of contents) becomes a plan, the disk
+is checked for room, a folder is copied into a staging folder on the content volume and moved
+into place in one step, and it can be cancelled and cleaned up at any point. A repaint for a bus that is not installed is kept aside and installed when the
 bus arrives.
 
-Archives can also be **used in place**: a `.zip` laid out like OMSI 2 is put into the
+A `.zip` is **used in place**, and only that way: a `.zip` laid out like OMSI 2 is put into the
 content folder's `Archives/` (hard-linked when it is on the same disk, moved from the
 `Mods/` inbox, else copied after a free-space check) and read by the game without
 unpacking (`omsi_cfg::vfs` mounts every archive there, as well as `--content-zip` and
-`OMSI_CONTENT_ZIP`). The Mods page offers it ("use the archive in place"), and its default
-unpacks what fits on the disk and uses an archive in place when its unpacked size does not;
-`.7z` and `.rar` archives are always unpacked.
-`--cli install '{"path":…,"mode":"inplace"}'` (or `extract` / `auto`) and
-`--cli modinfo '{"path":…}'` do the same from a shell. The launcher's lists see the maps
+`OMSI_CONTENT_ZIP`). A `.zip` whose folders are not where the game looks for them (a lone bus
+folder, say) is refused; unpack it yourself and install the folder. The `mode` of
+`--cli install '{"path":…,"mode":…}'` is accepted but no longer changes anything, and
+`--cli modinfo '{"path":…}'` shows what the launcher would do. The launcher's lists see the maps
 and buses inside the archives.
 
 ## Season and weather
```
