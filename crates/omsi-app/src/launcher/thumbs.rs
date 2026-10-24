//! Small pictures for the rows of a library (Maps: each map's own picture): read and made small
//! one after another on a thread of their own, only for the rows on screen, and kept on the GPU
//! for the rest of the run, so a long library opens as fast as a short one.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};

/// A thumbnail's size in pixels (twice the row's picture, for sharp edges on a scaled window).
pub const W: u32 = 168;
pub const H: u32 = 104;

type Job = (String, Box<dyn FnOnce() -> Option<image::RgbaImage> + Send>);

#[derive(Default)]
pub struct Thumbs {
    /// The textures made (None: there was no picture to make one of).
    kept: HashMap<String, Option<usize>>,
    /// Asked for and not yet made.
    asked: HashSet<String>,
    jobs: Option<Sender<Job>>,
    done: Option<Receiver<(String, Option<image::RgbaImage>)>>,
    /// Made and still to go to the GPU.
    pending: Vec<(String, Option<image::RgbaImage>)>,
}

/// `img` cut to the thumbnail's shape (its middle) and made that small.
pub fn make(img: &image::RgbaImage) -> image::RgbaImage {
    let d = image::DynamicImage::ImageRgba8(img.clone());
    d.resize_to_fill(W, H, image::imageops::FilterType::Triangle)
        .to_rgba8()
}

impl Thumbs {
    /// The texture of `key`, once made.
    pub fn get(&self, key: &str) -> Option<usize> {
        self.kept.get(key).copied().flatten()
    }

    /// Make the thumbnail of `key` with `read` (on the thread), unless it is made or asked for.
    pub fn want(
        &mut self,
        key: &str,
        read: impl FnOnce() -> Option<image::RgbaImage> + Send + 'static,
    ) {
        if self.kept.contains_key(key) || self.asked.contains(key) {
            return;
        }
        if self.jobs.is_none() {
            let (jtx, jrx) = channel::<Job>();
            let (dtx, drx) = channel();
            let spawned = std::thread::Builder::new()
                .name("launcher-thumbs".into())
                .spawn(move || {
                    for (key, read) in jrx {
                        if dtx.send((key, read().map(|i| make(&i)))).is_err() {
                            break;
                        }
                    }
                });
            if spawned.is_err() {
                return;
            }
            self.jobs = Some(jtx);
            self.done = Some(drx);
        }
        if let Some(tx) = &self.jobs {
            if tx.send((key.to_string(), Box::new(read))).is_ok() {
                self.asked.insert(key.to_string());
            }
        }
    }

    /// Take in what the thread made; true when something came (the page draws again).
    pub fn update(&mut self) -> bool {
        let Some(rx) = &self.done else { return false };
        let before = self.pending.len();
        while let Ok(r) = rx.try_recv() {
            self.asked.remove(&r.0);
            self.pending.push(r);
        }
        self.pending.len() > before
    }

    pub fn upload(&mut self, gpu: &mut omsi_ui::Gpu, device: &wgpu::Device, queue: &wgpu::Queue) {
        for (key, img) in self.pending.drain(..) {
            let tex = img.map(|i| gpu.add_image(device, queue, i.width(), i.height(), i.as_raw()));
            self.kept.insert(key, tex);
        }
    }

    /// Ask again for `key` when what was made of it was nothing (the picture it reads was not
    /// there yet and is now).
    pub fn retry(&mut self, key: &str) {
        if matches!(self.kept.get(key), Some(None)) {
            self.kept.remove(key);
        }
    }

    /// The device went (or was given to the game): the textures with it, made again when asked.
    pub fn forget_textures(&mut self) {
        self.kept.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_cut_to_the_thumbnails_shape() {
        let t = make(&image::RgbaImage::new(1600, 400));
        assert_eq!(t.dimensions(), (W, H));
        let t = make(&image::RgbaImage::new(300, 900));
        assert_eq!(t.dimensions(), (W, H));
    }

    #[test]
    fn a_thumbnail_is_made_once() {
        let mut t = Thumbs::default();
        t.want("a", || Some(image::RgbaImage::new(10, 10)));
        t.want("a", || panic!("asked twice"));
        let start = std::time::Instant::now();
        while !t.update() && start.elapsed().as_secs() < 5 {
            std::thread::yield_now();
        }
        assert_eq!(t.pending.len(), 1);
        assert!(t.asked.is_empty());
    }
}
