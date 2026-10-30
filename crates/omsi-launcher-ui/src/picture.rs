//! The picture a map ships in its folder (`picture.jpg` and the like): found through the
//! content roots, read within bounds and put on the GPU, for the Maps page and the game's
//! loading screen.

/// The picture in a map's folder (`picture.jpg` and the like, as a map ships it), for the
/// Maps page: read and made smaller on a thread, then put on the GPU.
#[derive(Default)]
pub struct MapPicture {
    /// The map file it is of.
    file: String,
    reading: Option<std::sync::mpsc::Receiver<Option<image::RgbaImage>>>,
    pending: Option<image::RgbaImage>,
    texture: Option<usize>,
    /// Width over height, while the texture shows this map's picture.
    aspect: Option<f32>,
}

impl MapPicture {
    /// The picture of the map `file` asked for: read on a thread when it is another map's,
    /// and taken once read.
    pub fn want(&mut self, root: &std::path::Path, file: &str) {
        if self.file != file {
            self.file = file.to_string();
            self.aspect = None;
            self.pending = None;
            // (found through the content roots as the map list found it: a mod's map, or one
            // read in place from an archive, has its own picture)
            let dir = Self::folder(root, file);
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let img = dir
                    .and_then(|d| Self::find(&d))
                    .and_then(|f| Self::read(&f));
                let _ = tx.send(img);
            });
            self.reading = Some(rx);
        }
        if let Some(rx) = self.reading.as_ref() {
            match rx.try_recv() {
                Ok(img) => {
                    self.pending = img;
                    self.reading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.reading = None,
            }
        }
    }

    /// The folder of the map `file`, found through the content roots as the map list found it.
    pub fn folder(root: &std::path::Path, file: &str) -> Option<std::path::PathBuf> {
        let global = omsi_cfg::find_in_roots(file)
            .map(|(_, p)| p)
            .unwrap_or_else(|| omsi_cfg::resolve_path(root, file));
        global.parent().map(|d| d.to_path_buf())
    }

    /// The picture in `dir`: a file named "picture" (any case) the image reader knows.
    pub fn find(dir: &std::path::Path) -> Option<std::path::PathBuf> {
        let mut files: Vec<_> = omsi_cfg::vfs::list_dir(dir)?
            .into_iter()
            .filter(|(_, is_dir)| !is_dir)
            .map(|(name, _)| dir.join(name))
            .filter(|p| {
                p.file_stem()
                    .is_some_and(|s| s.eq_ignore_ascii_case("picture"))
                    && p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                        ["jpg", "jpeg", "png", "bmp"].contains(&x.to_ascii_lowercase().as_str())
                    })
            })
            .collect();
        files.sort();
        files.into_iter().next()
    }

    /// The picture decoded within bounds (a map's file is anyone's: no more than 8192 pixels a
    /// side and 256 MB decoded, and a file on disk over 64 MB is not read at all), then made to
    /// fit 1280 x 1280 (the page shows it a few hundred points wide; a texture of that size goes
    /// on any card).
    pub fn read(file: &std::path::Path) -> Option<image::RgbaImage> {
        if std::fs::metadata(file).is_ok_and(|m| m.len() > 64 << 20) {
            return None;
        }
        let bytes = omsi_cfg::vfs::read(file).ok()?;
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(256 << 20);
        reader.limits(limits);
        let img = reader.decode().ok()?;
        let img = if img.width() > 1280 || img.height() > 1280 {
            img.resize(1280, 1280, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        Some(img.to_rgba8())
    }

    /// The picture read put on the GPU (in the texture this map's picture had, if any).
    pub fn upload(&mut self, gpu: &mut omsi_ui::Gpu, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Some(img) = self.pending.take() else {
            return;
        };
        let (w, h) = img.dimensions();
        match self.texture {
            Some(id) => gpu.update_image(device, queue, id, w, h, img.as_raw()),
            None => self.texture = Some(gpu.add_image(device, queue, w, h, img.as_raw())),
        }
        self.aspect = Some(w as f32 / h.max(1) as f32);
    }

    /// Whether the picture is still being read, or read and not yet on the GPU.
    pub fn busy(&self) -> bool {
        self.reading.is_some() || self.pending.is_some()
    }

    /// The texture and its width over height, while it shows this map's picture.
    pub fn shown(&self) -> Option<(usize, f32)> {
        Some((self.texture?, self.aspect?))
    }
}
