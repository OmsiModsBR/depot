//! The shapes the interface is drawn with: the plates, solid colours, rounded rectangles,
//! their corners and shadows, and the headset pointer, each rendered once into a texture.

use super::*;
use crate::hud::interface::output::{Renderer, Scene, TextureId};

impl TextCache {
    /// A small white circle, centred on the point that receives the click.
    pub(super) fn vr_pointer(&mut self, r: &Renderer, scene: &mut Scene) -> TextureId {
        let key = ("\u{0}vr_pointer_dot".to_string(), 0, [0, 0, 0, 0]);
        if let Some(label) = self.labels.get_mut(&key) {
            label.used = self.frame;
            return label.tex;
        }
        const W: usize = 32;
        const H: usize = 32;
        let mut rgba = vec![0u8; W * H * 4];
        for y in 0..H {
            for x in 0..W {
                let dx = x as f32 + 0.5 - W as f32 * 0.5;
                let dy = y as f32 + 0.5 - H as f32 * 0.5;
                let radius = (dx * dx + dy * dy).sqrt();
                let alpha = (16.0 - radius).clamp(0.0, 1.0);
                let white = radius < 13.0;
                let color = if white {
                    [255, 255, 255, (alpha * 255.0) as u8]
                } else {
                    [0, 0, 0, (alpha * 220.0) as u8]
                };
                rgba[(y * W + x) * 4..(y * W + x + 1) * 4].copy_from_slice(&color);
            }
        }
        let image = omsi_texture::Image {
            width: W as u32,
            height: H as u32,
            rgba,
            has_alpha: true,
        };
        let tex = r.add_texture(scene, &image, false);
        self.labels.insert(
            key,
            Label {
                tex,
                w: W as u32,
                h: H as u32,
                used: self.frame,
            },
        );
        tex
    }

    /// A plate of one colour: 0 the chat's dark translucent input box, 3 .. 9 as said below.
    pub(super) fn plate(&mut self, r: &Renderer, scene: &mut Scene, kind: u8) -> TextureId {
        let mut rgba = match kind {
            // an opaque panel as designed (translucent dark panels show the sky through
            // them), unless the opacity setting asks for less
            3 => vec![22, 22, 22, 255],
            // the chosen line: the interface's accent
            4 => accent().to_vec(),
            // a line the mouse is over, the next stop
            5 => vec![255, 255, 255, 28],
            // a hairline between groups (the card's border colour)
            9 => vec![255, 255, 255, 15],
            // the picture dimmed behind the menu
            6 => vec![0, 0, 0, 120],
            // a hairline between the information bar's groups
            10 => vec![255, 255, 255, 46],
            // behind a readout that warns (a tank nearly empty)
            16 => vec![238, 106, 95, 41],
            // the speed limit's red frame and white sign
            11 => vec![208, 48, 48, 255],
            12 => vec![235, 235, 235, 255],
            // the frame rate's dot: keeps up, slow, struggling
            13 => vec![95, 207, 138, 255],
            14 => vec![232, 171, 58, 255],
            15 => vec![238, 106, 95, 255],
            // behind a note or the tooltip (the old HUD's 40 %)
            7 => vec![0, 0, 0, 102],
            _ => vec![10, 12, 16, 150],
        };
        // the backgrounds - the panels, the chat's box, the plates - as the opacity setting
        // has them (`backdrop`); the accent, the lines and the dimming stay as they are
        if matches!(kind, 0 | 3 | 7) {
            rgba[3] = (rgba[3] as f32 * self.backdrop).round().clamp(0.0, 255.0) as u8;
        }
        let key = (
            "\u{0}plate".to_string(),
            kind as u32,
            [rgba[0], rgba[1], rgba[2], rgba[3]],
        );
        if let Some(l) = self.labels.get_mut(&key) {
            l.used = self.frame;
            return l.tex;
        }
        let img = omsi_texture::Image {
            width: 1,
            height: 1,
            rgba,
            has_alpha: true,
        };
        let tex = r.add_texture(scene, &img, false);
        self.labels.insert(
            key,
            Label {
                tex,
                w: 1,
                h: 1,
                used: self.frame,
            },
        );
        tex
    }

    /// A plate of one RGBA colour, any size (stretched).
    fn solid(&mut self, r: &Renderer, scene: &mut Scene, rgba: [u8; 4]) -> TextureId {
        let key = ("\u{0}solid".to_string(), 0, rgba);
        if let Some(l) = self.labels.get_mut(&key) {
            l.used = self.frame;
            return l.tex;
        }
        let img = omsi_texture::Image {
            width: 1,
            height: 1,
            rgba: rgba.to_vec(),
            has_alpha: true,
        };
        let tex = r.add_texture(scene, &img, false);
        self.labels.insert(
            key,
            Label {
                tex,
                w: 1,
                h: 1,
                used: u64::MAX / 2,
            },
        );
        tex
    }

    /// One anti-aliased quarter disc of `rad` pixels: the rounded corner `idx` (0 top left,
    /// 1 top right, 2 bottom left, 3 bottom right) of a rounded plate.
    fn corner(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        rad: u32,
        idx: u32,
        rgba: [u8; 4],
    ) -> TextureId {
        let key = ("\u{0}corner".to_string(), (rad << 8) | idx, rgba);
        if let Some(l) = self.labels.get_mut(&key) {
            l.used = self.frame;
            return l.tex;
        }
        let n = rad as usize;
        let (cx, cy) = (
            if idx & 1 == 0 { rad as f32 } else { 0.0 },
            if idx & 2 == 0 { rad as f32 } else { 0.0 },
        );
        let mut data = vec![0u8; n * n * 4];
        for py in 0..n {
            for px in 0..n {
                let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                let cover = (rad as f32 - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
                let o = (py * n + px) * 4;
                data[o..o + 3].copy_from_slice(&rgba[..3]);
                data[o + 3] = (rgba[3] as f32 * cover).round() as u8;
            }
        }
        let img = omsi_texture::Image {
            width: rad,
            height: rad,
            rgba: data,
            has_alpha: true,
        };
        let tex = r.add_texture(scene, &img, false);
        self.labels.insert(
            key,
            Label {
                tex,
                w: rad,
                h: rad,
                used: u64::MAX / 2,
            },
        );
        tex
    }

    /// A texture made once for `key` (kept while it is used, like a text).
    fn cached(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        key: (String, u32, [u8; 4]),
        size: (u32, u32),
        build: impl FnOnce() -> Vec<u8>,
    ) -> TextureId {
        if let Some(l) = self.labels.get_mut(&key) {
            l.used = self.frame;
            return l.tex;
        }
        let img = omsi_texture::Image {
            width: size.0,
            height: size.1,
            rgba: build(),
            has_alpha: true,
        };
        let tex = r.add_texture(scene, &img, false);
        self.labels.insert(
            key,
            Label {
                tex,
                w: size.0,
                h: size.1,
                used: self.frame,
            },
        );
        tex
    }

    /// A whole rounded plate of `w` x `h` pixels in one texture, anti-aliased.
    fn rrect(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        w: u32,
        h: u32,
        rad: f32,
        rgba: [u8; 4],
    ) -> TextureId {
        let key = (format!("\u{0}rr{w}x{h}r{}", rad as u32), 0, rgba);
        self.cached(r, scene, key, (w, h), || {
            let mut data = vec![0u8; (w * h * 4) as usize];
            let ri = rad as u32;
            for py in 0..h {
                let band_y = py >= ri && py + ri < h;
                for px in 0..w {
                    let cover = if band_y || (px >= ri && px + ri < w) {
                        1.0
                    } else {
                        let d = rr_dist(
                            px as f32 + 0.5,
                            py as f32 + 0.5,
                            0.0,
                            0.0,
                            w as f32,
                            h as f32,
                            rad,
                        );
                        (0.5 - d).clamp(0.0, 1.0)
                    };
                    let o = ((py * w + px) * 4) as usize;
                    data[o..o + 3].copy_from_slice(&rgba[..3]);
                    data[o + 3] = (rgba[3] as f32 * cover).round() as u8;
                }
            }
            data
        })
    }

    /// A soft shadow under the rounded rectangle `rect`: it fades out over `spread` pixels
    /// and lies `dy` lower.
    pub(super) fn shadow(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        rect: [f32; 4],
        radius: f32,
        spread: f32,
        dy: f32,
        alpha: u8,
    ) {
        let [x0, y0, x1, y1] = [
            rect[0].round(),
            rect[1].round(),
            rect[2].round(),
            rect[3].round(),
        ];
        let (w, h) = ((x1 - x0).max(0.0) as u32, (y1 - y0).max(0.0) as u32);
        let m = spread.round().max(1.0) as u32;
        let (tw, th) = (w + 2 * m, h + 2 * m);
        if w < 2 || h < 2 || tw as u64 * th as u64 > 4_000_000 {
            return;
        }
        let rad = radius
            .round()
            .min((w as f32 * 0.5).floor())
            .min((h as f32 * 0.5).floor())
            .max(0.0);
        let key = (
            format!("\u{0}shadow{w}x{h}r{}m{m}", rad as u32),
            0,
            [0, 0, 0, alpha],
        );
        let tex = self.cached(r, scene, key, (tw, th), || {
            let mut data = vec![0u8; (tw * th * 4) as usize];
            for py in 0..th {
                for px in 0..tw {
                    let d = rr_dist(
                        px as f32 + 0.5,
                        py as f32 + 0.5,
                        m as f32,
                        m as f32,
                        w as f32,
                        h as f32,
                        rad,
                    );
                    let k = (1.0 - d / m as f32).clamp(0.0, 1.0);
                    data[((py * tw + px) * 4 + 3) as usize] = (alpha as f32 * k * k).round() as u8;
                }
            }
            data
        });
        scene.overlays.push((
            tex,
            [
                x0 - m as f32,
                y0 - m as f32 + dy,
                x1 + m as f32,
                y1 + m as f32 + dy,
            ],
        ));
    }

    /// A rectangle with rounded corners of `radius` pixels in `rgba`, on whole pixels so
    /// that translucent colours show no seams: one texture for a plate of some size, else
    /// (a thin bar, a thumb that grows and shrinks) three bands and four corner discs.
    pub(super) fn rounded(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        rect: [f32; 4],
        radius: f32,
        rgba: [u8; 4],
    ) {
        let [x0, y0, x1, y1] = [
            rect[0].round(),
            rect[1].round(),
            rect[2].round(),
            rect[3].round(),
        ];
        let (w, h) = (x1 - x0, y1 - y0);
        if w < 1.0 || h < 1.0 {
            return;
        }
        let rad = radius
            .round()
            .min((w * 0.5).floor())
            .min((h * 0.5).floor())
            .max(0.0);
        if rad < 1.0 {
            let solid = self.solid(r, scene, rgba);
            scene.overlays.push((solid, [x0, y0, x1, y1]));
            return;
        }
        if w.min(h) >= 12.0 && w * h <= 4_000_000.0 {
            let tex = self.rrect(r, scene, w as u32, h as u32, rad, rgba);
            scene.overlays.push((tex, [x0, y0, x1, y1]));
            return;
        }
        let solid = self.solid(r, scene, rgba);
        let ri = rad as u32;
        // the middle band over the whole height, the side bands between the corners
        if x1 - rad > x0 + rad {
            scene.overlays.push((solid, [x0 + rad, y0, x1 - rad, y1]));
        }
        if h > rad * 2.0 {
            scene
                .overlays
                .push((solid, [x0, y0 + rad, x0 + rad, y1 - rad]));
            scene
                .overlays
                .push((solid, [x1 - rad, y0 + rad, x1, y1 - rad]));
        }
        let tl = self.corner(r, scene, ri, 0, rgba);
        let tr = self.corner(r, scene, ri, 1, rgba);
        let bl = self.corner(r, scene, ri, 2, rgba);
        let br = self.corner(r, scene, ri, 3, rgba);
        scene.overlays.push((tl, [x0, y0, x0 + rad, y0 + rad]));
        scene.overlays.push((tr, [x1 - rad, y0, x1, y0 + rad]));
        scene.overlays.push((bl, [x0, y1 - rad, x0 + rad, y1]));
        scene.overlays.push((br, [x1 - rad, y1 - rad, x1, y1]));
    }
}
