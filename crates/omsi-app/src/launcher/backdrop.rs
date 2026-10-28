//! The Scene background (Settings > Interface > Background): behind the launcher's pages, a
//! blurred picture, darkened by the dimming, under panels one sees through. On Maps it is the
//! chosen map's picture, on Vehicles the chosen bus as its preview shows it, on the other pages
//! the player's latest screenshot; none of those, the plain dark.
//!
//! A picture is blurred once, on a thread of its own (shrunk first: the blur of a small picture
//! drawn large is soft and cheap), and kept for the rest of the run; one that is a screenshot
//! is read again after a game, which may have taken newer ones.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// The blurred picture's width (it is drawn across the window: what the blur leaves is no finer).
pub const WIDTH: u32 = 320;
/// The blur's strength, in pixels of that width.
const SIGMA: f32 = 7.0;
/// How many pictures are kept blurred.
pub const KEPT: usize = 24;

/// What a page wants behind it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Want {
    /// The map's picture (its file as the map list names it, under the OMSI 2 folder), else the
    /// latest screenshot of that map, else the latest of any.
    Map { root: PathBuf, file: String },
    /// The bus (its preview's `tag`), offered with `offer_bus` when its picture comes.
    Bus(u64),
    /// The latest screenshot.
    Latest,
}

#[derive(Default)]
pub struct Backdrop {
    /// What the page asked for in this frame.
    want: Option<Want>,
    /// Pictures blurred (None: there was none to blur), and whether each is a screenshot (or
    /// stands for one: none there was).
    pub kept: HashMap<Want, (Option<image::RgbaImage>, bool)>,
    order: Vec<Want>,
    pub reading: Vec<(Want, Receiver<(Option<image::RgbaImage>, bool)>)>,
    /// A game ran when last asked (see `game_running`).
    playing: bool,
    /// What the texture holds, and what goes to it next.
    pub shown: Option<Want>,
    /// What is shown is a screenshot read again after a game: it goes when the new one comes.
    shown_stale: bool,
    pub pending: Option<(Want, image::RgbaImage)>,
    texture: Option<usize>,
    aspect: f32,
    /// The picture fades in (0 to 1) when it changes.
    pub fade: f32,
}

/// `img` made small and blurred.
pub fn blur(img: &image::DynamicImage) -> image::RgbaImage {
    // (WIDTH wide, keeping the shape; a tall picture no taller than twice that)
    let small = img
        .resize(WIDTH, WIDTH * 2, image::imageops::FilterType::Triangle)
        .to_rgba8();
    image::imageops::blur(&small, SIGMA)
}

/// The picture of a map: its own "picture" file, else the latest screenshot of it, else any;
/// and whether it is (or would have been) a screenshot.
fn map_picture(root: &std::path::Path, file: &str) -> (Option<image::RgbaImage>, bool) {
    let global = omsi_cfg::find_in_roots(file)
        .map(|(_, p)| p)
        .unwrap_or_else(|| omsi_cfg::resolve_path(root, file));
    let own = global
        .parent()
        .and_then(super::MapPicture::find)
        .and_then(|f| super::MapPicture::read(&f));
    if own.is_some() {
        return (own, false);
    }
    let folder = crate::screenshot_name::folder_of(file).unwrap_or_default();
    let shot = omsi_launcher_lib::screenshots::for_map(&folder)
        .pop()
        .or_else(omsi_launcher_lib::screenshots::latest);
    (shot.and_then(|p| super::MapPicture::read(&p)), true)
}

impl Backdrop {
    /// Ask for `want` behind the page in this frame: from the kept ones, else read and blurred
    /// on a thread (a bus waits for `offer_bus`).
    pub fn want(&mut self, want: Want) {
        if self.want.as_ref() != Some(&want) {
            self.want = Some(want.clone());
        }
        if self.kept.contains_key(&want) || self.reading.iter().any(|(w, _)| *w == want) {
            return;
        }
        let job: Box<dyn FnOnce() -> (Option<image::RgbaImage>, bool) + Send> = match &want {
            Want::Map { root, file } => {
                let (root, file) = (root.clone(), file.clone());
                Box::new(move || {
                    let (img, shot) = map_picture(&root, &file);
                    (img.map(|i| blur(&image::DynamicImage::ImageRgba8(i))), shot)
                })
            }
            Want::Latest => Box::new(|| {
                (
                    omsi_launcher_lib::screenshots::latest()
                        .and_then(|p| super::MapPicture::read(&p))
                        .map(|i| blur(&image::DynamicImage::ImageRgba8(i))),
                    true,
                )
            }),
            Want::Bus(_) => return,
        };
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new()
            .name("launcher-backdrop".into())
            .spawn(move || {
                let _ = tx.send(job());
            });
        self.reading.push((want, rx));
    }

    /// The bus `tag`'s picture (`w` x `h`, RGBA), as its preview drew it: blurred once.
    pub fn offer_bus(&mut self, tag: u64, w: u32, h: u32, rgba: &[u8]) {
        let want = Want::Bus(tag);
        if self.kept.contains_key(&want) || self.reading.iter().any(|(k, _)| *k == want) {
            return;
        }
        let Some(img) = image::RgbaImage::from_raw(w, h, rgba.to_vec()) else {
            return;
        };
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new()
            .name("launcher-backdrop".into())
            .spawn(move || {
                let _ = tx.send((Some(blur(&image::DynamicImage::ImageRgba8(img))), false));
            });
        self.reading.push((want, rx));
    }

    /// Collect what the threads finished; the picture wanted goes to the GPU next.
    pub fn update(&mut self) {
        let mut done = Vec::new();
        self.reading.retain(|(w, rx)| match rx.try_recv() {
            Ok(img) => {
                done.push((w.clone(), img));
                false
            }
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => false,
        });
        for (w, img) in done {
            self.keep(w, img);
        }
        let Some(want) = self.want.clone() else {
            return;
        };
        if (self.shown.as_ref() == Some(&want) && !self.shown_stale)
            || self.pending.as_ref().is_some_and(|p| p.0 == want)
        {
            return;
        }
        match self.kept.get(&want) {
            Some((Some(img), _)) => self.pending = Some((want, img.clone())),
            // (nothing to show for it: the plain dark)
            Some((None, _)) => {
                self.shown = None;
                self.pending = None;
            }
            // (still being read: the last picture stays meanwhile)
            None => {}
        }
    }

    /// What the page asked for last, if anything.
    pub fn wanted(&self) -> Option<&Want> {
        self.want.as_ref()
    }

    /// Whether a game runs now: when one has ended, the pictures that were screenshots are read
    /// again (the player may have taken newer ones); the one shown stays until then.
    pub fn game_running(&mut self, on: bool) {
        if std::mem::replace(&mut self.playing, on) && !on {
            let shots: Vec<Want> = self
                .kept
                .iter()
                .filter(|(_, (_, shot))| *shot)
                .map(|(w, _)| w.clone())
                .collect();
            for w in shots {
                self.kept.remove(&w);
                self.order.retain(|k| *k != w);
            }
            // (shown again once read: not the same picture by name)
            if self
                .shown
                .as_ref()
                .is_some_and(|w| !matches!(w, Want::Bus(_)))
            {
                self.shown_stale = true;
            }
        }
    }

    pub fn keep(&mut self, w: Want, img: (Option<image::RgbaImage>, bool)) {
        self.order.retain(|k| *k != w);
        self.order.push(w.clone());
        self.kept.insert(w, img);
        while self.order.len() > KEPT {
            let old = self.order.remove(0);
            self.kept.remove(&old);
        }
    }

    pub fn upload(&mut self, gpu: &mut omsi_ui::Gpu, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Some((want, img)) = self.pending.take() else {
            return;
        };
        let (w, h) = img.dimensions();
        match self.texture {
            Some(id) => gpu.update_image(device, queue, id, w, h, img.as_raw()),
            None => self.texture = Some(gpu.add_image(device, queue, w, h, img.as_raw())),
        }
        self.aspect = w as f32 / h.max(1) as f32;
        // (a picture after the plain dark fades in; one after another replaces it)
        if self.shown.is_none() {
            self.fade = 0.0;
        }
        self.shown = Some(want);
        self.shown_stale = false;
    }

    /// The texture and its width over its height, while it holds a picture.
    pub fn shown(&self) -> Option<(usize, f32)> {
        self.shown.as_ref()?;
        Some((self.texture?, self.aspect))
    }

    /// The device was lost (or given to the game): the texture with it.
    pub fn forget_texture(&mut self) {
        self.texture = None;
        self.shown = None;
        self.pending = None;
    }
}
