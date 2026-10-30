//! The content on disk: the mods to install, what it takes and the Free up space card.

use super::super::domain::{msg::*, squeeze::*, state::*};
use super::super::interface::output::core;
use super::text::*;

impl State {
    pub fn load_mods(&mut self) {
        self.mods_asked = true;
        self.spawn(|| Msg::Mods(core::mods_status().map_err(|e| format!("{e:#}"))));
    }

    /// Reads what the OMSI 2 folder and the content folder take on disk (Setup's Content
    /// card), in the background.
    pub fn load_usage(&mut self) {
        self.usage_asked = true;
        self.usage = None;
        self.usage_gen += 1;
        let gen = self.usage_gen;
        let root = std::path::PathBuf::from(&self.config.root);
        self.spawn(move || {
            let mut folders = Vec::new();
            if !root.as_os_str().is_empty() {
                folders.push(("OMSI 2 folder".to_string(), root));
            }
            if let Some(c) = core::content_dir() {
                folders.push(("Depot content folder".to_string(), c));
            }
            Msg::Usage(gen, core::disk::scan(&folders))
        });
    }

    /// Reads the folders of the Free up space card (the Depot's content and the original
    /// OMSI 2): their disks, the files that may be compressed, the estimate and what is
    /// compressed now - in the background.
    pub fn load_squeeze(&mut self) {
        self.squeeze.asked = true;
        self.squeeze.gen += 1;
        let gen = self.squeeze.gen;
        let root = std::path::PathBuf::from(&self.config.root);
        self.spawn(move || {
            let (content, root) = Squeeze::dirs(&root);
            let mut folders = Vec::new();
            if let Some(c) = &content {
                let skip: Vec<_> = root.iter().cloned().collect();
                folders.push(core::compress::inspect(Squeeze::DEPOT, c, &skip));
            }
            if let Some(r) = &root {
                // (the Depot's folder often sits inside the original's: not counted twice)
                let skip: Vec<_> = content.iter().cloned().collect();
                folders.push(core::compress::inspect(Squeeze::ORIGINAL, r, &skip));
            }
            Msg::Squeeze(gen, std::sync::Arc::new(folders))
        });
    }

    /// Forgets what was read of the folders (they were changed under Setup) and reads again;
    /// a reading still on its way is not taken.
    pub fn reset_squeeze(&mut self) {
        self.squeeze.folders = None;
        self.squeeze.chosen_set = false;
        self.squeeze.confirm = None;
        self.squeeze.hdd_ok = false;
        self.load_squeeze();
    }

    /// Compresses the ticked folders (or, with `undo`, stores their files as they were), in
    /// the background; `squeeze.job` tells how far it is and cancels it.
    pub fn start_squeeze(&mut self, undo: bool) {
        if self.squeeze.job.is_some() {
            return;
        }
        let Some(folders) = self.squeeze.folders.clone() else {
            return;
        };
        // (the original folder under Setup may have changed since it was read; the content
        // folder's change comes through `reset_squeeze` when Setup saves)
        let root = std::path::Path::new(&self.config.root);
        if folders
            .iter()
            .any(|f| f.name == Squeeze::ORIGINAL && !core::compress::same_path(&f.dir, root))
        {
            self.reset_squeeze();
            return;
        }
        self.squeeze.hdd_ok = false;
        let chosen = self.squeeze.chosen;
        let job = core::compress::Job::new();
        self.squeeze.job = Some((job.clone(), undo));
        self.squeeze.done = None;
        self.spawn(move || {
            let mut files = Vec::new();
            for f in folders.iter().filter(|f| f.disk.is_ok()) {
                if chosen[Squeeze::index(f.name)] {
                    if undo {
                        files.extend(core::compress::compressed_of(&f.files));
                    } else {
                        files.extend(f.files.iter().cloned());
                    }
                }
            }
            core::log_to_file(&format!(
                "{} {} files",
                if undo {
                    "undoing compression of"
                } else {
                    "compressing"
                },
                files.len()
            ));
            use std::sync::atomic::Ordering::Relaxed;
            // (a panic ends the job with its message, so that the card does not wait for ever)
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                core::compress::run(&files, undo, &job)
            }))
            .unwrap_or_else(|_| Err("the compression stopped unexpectedly".into()));
            Msg::SqueezeDone(match r {
                Err(e) => Squeezed::Error(e),
                // (a cancel that came after the last batch changed nothing)
                Ok(())
                    if job.cancel.load(Relaxed)
                        && job.done.load(Relaxed) < job.total.load(Relaxed) =>
                {
                    Squeezed::Cancelled
                }
                Ok(()) => Squeezed::Done(job.failed.load(Relaxed)),
            })
        });
    }
}

impl State {
    pub fn install(&mut self, path: String) {
        // (a .zip only: loose files are copied by hand, into the OMSI 2 folder or the content
        // folder - the Mods page says where)
        if !is_zip(&path) {
            self.set_status("The launcher installs a .zip only. Copy loose files yourself: the Mods page shows where.", true);
            return;
        }
        // (a .zip is only ever used in place, from Archives)
        let mode = "inplace".to_string();
        self.mod_path = path.clone();
        let p2 = path.clone();
        self.spawn(move || {
            Msg::ModInfo(core::inspect_mod(std::path::Path::new(&p2)).map_err(|e| format!("{e:#}")))
        });
        self.spawn(move || {
            Msg::Installed(
                core::start_install(std::path::Path::new(&path), &mode)
                    .map_err(|e| format!("{e:#}")),
            )
        });
    }
}
