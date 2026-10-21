//! The Scene background (Settings > Interface > Background): behind the launcher's pages, a
//! blurred picture, darkened by the dimming, under panels one sees through. On Maps it is the
//! chosen map's picture, on Vehicles the chosen bus as its preview shows it, on the other pages
//! the player's latest screenshot; none of those, the plain dark.
//!
//! A picture is blurred once, on a thread of its own (shrunk first: the blur of a small picture
//! drawn large is soft and cheap), and kept for the rest of the run.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// The blurred picture's width (it is drawn across the window: what the blur leaves is no finer).
const WIDTH: u32 = 320;
/// The blur's strength, in pixels of that width.
const SIGMA: f32 = 7.0;
/// How many pictures are kept blurred.
const KEPT: usize = 24;

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
    /// Pictures blurred (None: there was none to blur).
    kept: HashMap<Want, Option<image::RgbaImage>>,
    order: Vec<Want>,
    reading: Vec<(Want, Receiver<Option<image::RgbaImage>>)>,
    /// What the texture holds, and what goes to it next.
    shown: Option<Want>,
    pending: Option<(Want, image::RgbaImage)>,
    texture: Option<usize>,
    aspect: f32,
    /// The picture fades in (0 to 1) when it changes.
    pub fade: f32,
}

/// `img` made small and blurred.
pub fn blur(img: &image::DynamicImage) -> image::RgbaImage {
    let h = (img.height() as u64 * WIDTH as u64 / img.width().max(1) as u64).clamp(1, WIDTH as u64 * 2) as u32;
    let small = img.resize_exact(WIDTH, h, image::imageops::FilterType::Triangle).to_rgba8();
    image::imageops::blur(&small, SIGMA)
}

/// The picture of a map: its own "picture" file, else the latest screenshot of it, else any.
fn map_picture(root: &std::path::Path, file: &str) -> Option<image::RgbaImage> {
    let global = omsi_cfg::find_in_roots(file).map(|(_, p)| p).unwrap_or_else(|| omsi_cfg::resolve_path(root, file));
    let own = global.parent().and_then(super::MapPicture::find).and_then(|f| super::MapPicture::read(&f));
    if own.is_some() {
        return own;
    }
    let folder = crate::screenshot_name::folder_of(file).unwrap_or_default();
    let shot = omsi_launcher_lib::screenshots::for_map(&folder).pop().or_else(omsi_launcher_lib::screenshots::latest);
    shot.and_then(|p| super::MapPicture::read(&p))
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
        let job: Box<dyn FnOnce() -> Option<image::RgbaImage> + Send> = match &want {
            Want::Map { root, file } => {
                let (root, file) = (root.clone(), file.clone());
                Box::new(move || map_picture(&root, &file).map(|i| blur(&image::DynamicImage::ImageRgba8(i))))
            }
            Want::Latest => Box::new(|| omsi_launcher_lib::screenshots::latest().and_then(|p| super::MapPicture::read(&p)).map(|i| blur(&image::DynamicImage::ImageRgba8(i)))),
            Want::Bus(_) => return,
        };
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new().name("launcher-backdrop".into()).spawn(move || {
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
        let Some(img) = image::RgbaImage::from_raw(w, h, rgba.to_vec()) else { return };
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new().name("launcher-backdrop".into()).spawn(move || {
            let _ = tx.send(Some(blur(&image::DynamicImage::ImageRgba8(img))));
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
        let Some(want) = self.want.clone() else { return };
        if self.shown.as_ref() == Some(&want) || self.pending.as_ref().is_some_and(|p| p.0 == want) {
            return;
        }
        match self.kept.get(&want) {
            Some(Some(img)) => self.pending = Some((want, img.clone())),
            // (nothing to show for it: the plain dark)
            Some(None) => {
                self.shown = None;
                self.pending = None;
            }
            // (still being read: the last picture stays meanwhile)
            None => {}
        }
    }

    fn keep(&mut self, w: Want, img: Option<image::RgbaImage>) {
        self.order.retain(|k| *k != w);
        self.order.push(w.clone());
        self.kept.insert(w, img);
        while self.order.len() > KEPT {
            let old = self.order.remove(0);
            self.kept.remove(&old);
        }
    }

    pub fn upload(&mut self, gpu: &mut omsi_ui::Gpu, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Some((want, img)) = self.pending.take() else { return };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_blurred_small_keeping_its_shape() {
        let mut img = image::RgbaImage::new(1600, 900);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = if x % 2 == 0 { image::Rgba([255, 255, 255, 255]) } else { image::Rgba([0, 0, 0, 255]) };
        }
        let b = blur(&image::DynamicImage::ImageRgba8(img));
        assert_eq!(b.dimensions(), (WIDTH, 180));
        // (the stripes are gone: one grey)
        let p = b.get_pixel(160, 90).0;
        assert!((100..=155).contains(&p[0]), "{p:?}");
    }

    #[test]
    fn what_was_blurred_is_kept_and_shown_without_reading_again() {
        let mut b = Backdrop::default();
        b.keep(Want::Latest, Some(image::RgbaImage::new(4, 2)));
        b.want(Want::Latest);
        assert!(b.reading.is_empty());
        b.update();
        assert!(b.pending.is_some());
        // (a bus waits for its picture: nothing is read for it)
        b.want(Want::Bus(7));
        assert!(b.reading.is_empty());
        b.offer_bus(7, 2, 1, &[0; 8]);
        assert_eq!(b.reading.len(), 1);
        b.offer_bus(7, 2, 1, &[0; 8]);
        assert_eq!(b.reading.len(), 1);
    }

    #[test]
    fn only_the_latest_pictures_are_kept() {
        let mut b = Backdrop::default();
        for t in 0..KEPT as u64 + 5 {
            b.keep(Want::Bus(t), None);
        }
        assert_eq!(b.kept.len(), KEPT);
        assert!(!b.kept.contains_key(&Want::Bus(0)));
        assert!(b.kept.contains_key(&Want::Bus(KEPT as u64 + 4)));
    }
}
