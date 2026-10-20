//! The game's interface as this renderer takes it: `omsi-ui`'s vertex lists, layers and draws
//! (the menus, the Alt bar, the loading screen), and the HUD's pictures over the scene. The
//! device side is `d3d9::ui`; this part is plain data and the rules both renderers share.

use std::ops::Range;

/// Floats per interface vertex: position (3), extent (2), width (2), uv (2), colour (4, sRGB
/// with straight alpha) and mode (2: x 0 pixels / 1 world, y 1 = times the texture), as
/// `omsi-ui` lays them out.
pub const UI_VERTEX_FLOATS: usize = 15;

/// How one group of draws is seen (`omsi_ui::Layer`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiLayer {
    /// Column-major.
    pub view_proj: [[f32; 4]; 4],
    /// Target pixels (x, y, w, h) the world is drawn into; pixel vertices are laid out in it.
    pub viewport: [f32; 4],
    /// Everything outside this box (x0, y0, x1, y1) with rounded corners fades out.
    pub clip: [f32; 4],
    pub radius: f32,
    pub opacity: f32,
    /// Metres per pixel per metre of depth; 0 for a flat layer.
    pub px_scale: f32,
}

const IDENTITY: [[f32; 4]; 4] = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

impl UiLayer {
    /// Pixels only, clipped to `clip`.
    pub fn flat(clip: [f32; 4], radius: f32, opacity: f32) -> Self {
        Self {
            view_proj: IDENTITY,
            viewport: [0.0; 4],
            clip,
            radius,
            opacity,
            px_scale: 0.0,
        }
    }

    fn is_flat(&self) -> bool {
        self.px_scale == 0.0 && self.view_proj == IDENTITY
    }

    /// The box the layer's pixel vertices are laid out in (the shader's), in a `w` by `h`
    /// target.
    pub fn layout_box(&self, w: u32, h: u32) -> [f32; 4] {
        if self.viewport[2] > 0.0 {
            self.viewport
        } else {
            [0.0, 0.0, w as f32, h as f32]
        }
    }

    /// The rasteriser's viewport (x, y, w, h), inside the target. A flat layer covers the
    /// whole target, its pixels scaled to it from its layout box (an interface laid out in
    /// logical pixels).
    pub fn viewport_in(&self, w: u32, h: u32) -> [u32; 4] {
        let (wf, hf) = (w.max(1) as f32, h.max(1) as f32);
        let vp = if self.is_flat() {
            [0.0, 0.0, wf, hf]
        } else {
            self.layout_box(w, h)
        };
        let x = vp[0].clamp(0.0, wf - 1.0);
        let y = vp[1].clamp(0.0, hf - 1.0);
        let vw = vp[2].min(wf - x).max(1.0);
        let vh = vp[3].min(hf - y).max(1.0);
        [x as u32, y as u32, vw as u32, vh as u32]
    }

    /// The scissor box (x0, y0, x1, y1) of the layer's clip in the target; None when empty.
    pub fn scissor_in(&self, w: u32, h: u32) -> Option<[u32; 4]> {
        let (wf, hf) = (w as f32, h as f32);
        let x0 = self.clip[0].floor().clamp(0.0, wf) as u32;
        let y0 = self.clip[1].floor().clamp(0.0, hf) as u32;
        let x1 = self.clip[2].ceil().clamp(0.0, wf) as u32;
        let y1 = self.clip[3].ceil().clamp(0.0, hf) as u32;
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1, y1])
    }
}

/// Vertices `range` of buffer `buffer`, seen as layer `layer`, with picture `texture`
/// (0 = the atlas).
#[derive(Debug, Clone, PartialEq)]
pub struct UiDraw {
    pub buffer: usize,
    pub range: Range<u32>,
    pub layer: usize,
    pub texture: usize,
}

/// One frame of an interface: drawn over what the frame holds, in order.
#[derive(Debug, Clone, Copy)]
pub struct UiFrame<'a> {
    /// The target's size the layers were laid out for.
    pub size: (u32, u32),
    /// Cleared to this first (linear light, as wgpu takes a clear colour) when given.
    pub clear: Option<[f32; 4]>,
    pub layers: &'a [UiLayer],
    pub draws: &'a [UiDraw],
}

/// A HUD picture over the scene: scene texture `texture` in the pixel box `rect` (x0, y0,
/// x1, y1 of the window).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiOverlay {
    pub texture: usize,
    pub rect: [f32; 4],
    /// The picture holds premultiplied alpha (drawn by `omsi-ui`).
    pub premultiplied: bool,
    /// The picture lies on its side (u and v swapped).
    pub transposed: bool,
}

/// The rect on whole pixels, as the default renderer snaps it: halves up the same way on
/// both sides, at least a pixel wide where it has width.
pub fn snap_rect(r: [f32; 4]) -> [f32; 4] {
    let snap = |v: f32| (v + 0.5).floor();
    let (x0, y0) = (snap(r[0]), snap(r[1]));
    let x1 = if r[2] > r[0] {
        snap(r[2]).max(x0 + 1.0)
    } else {
        snap(r[2])
    };
    let y1 = if r[3] > r[1] {
        snap(r[3]).max(y0 + 1.0)
    } else {
        snap(r[3])
    };
    [x0, y0, x1, y1]
}

/// The six interface vertices (two triangles, pixel mode, white, textured) of an overlay.
pub fn overlay_vertices(o: &UiOverlay) -> [[f32; UI_VERTEX_FLOATS]; 6] {
    let r = snap_rect(o.rect);
    let corner = |cx: f32, cy: f32| {
        let (u, v) = if o.transposed { (cy, cx) } else { (cx, cy) };
        [
            r[0] + (r[2] - r[0]) * cx,
            r[1] + (r[3] - r[1]) * cy,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            u,
            v,
            1.0,
            1.0,
            1.0,
            1.0,
            0.0,
            1.0,
        ]
    };
    [
        corner(0.0, 0.0),
        corner(1.0, 0.0),
        corner(1.0, 1.0),
        corner(0.0, 0.0),
        corner(1.0, 1.0),
        corner(0.0, 1.0),
    ]
}

/// The atlas's changed `region` (x, y, w, h) lies in a `size` square of `rgba` bytes.
pub fn atlas_region_fits(size: u32, rgba_len: usize, region: [u32; 4]) -> bool {
    let [x, y, w, h] = region;
    let square = (size as usize)
        .checked_mul(size as usize)
        .and_then(|n| n.checked_mul(4));
    square == Some(rgba_len)
        && x.checked_add(w).is_some_and(|e| e <= size)
        && y.checked_add(h).is_some_and(|e| e <= size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_layer_covers_the_target_and_lays_out_in_its_own_box() {
        let mut l = UiLayer::flat([0.0, 0.0, 100.0, 50.0], 0.0, 1.0);
        l.viewport = [0.0, 0.0, 800.0, 450.0];
        assert_eq!(l.viewport_in(1600, 900), [0, 0, 1600, 900]);
        assert_eq!(l.layout_box(1600, 900), [0.0, 0.0, 800.0, 450.0]);
        let world = UiLayer {
            px_scale: 0.01,
            viewport: [100.0, 50.0, 4000.0, 300.0],
            ..l
        };
        assert_eq!(world.viewport_in(1600, 900), [100, 50, 1500, 300]);
    }

    #[test]
    fn the_scissor_is_the_clip_on_whole_pixels_inside_the_target() {
        let l = UiLayer::flat([-5.0, 10.2, 99.5, 2000.0], 0.0, 1.0);
        assert_eq!(l.scissor_in(100, 100), Some([0, 10, 100, 100]));
        assert_eq!(
            UiLayer::flat([5.0, 5.0, 5.0, 9.0], 0.0, 1.0).scissor_in(10, 10),
            None
        );
    }

    #[test]
    fn overlays_snap_and_turn_their_pictures() {
        assert_eq!(snap_rect([0.4, 0.5, 0.6, 10.49]), [0.0, 1.0, 1.0, 10.0]);
        let v = overlay_vertices(&UiOverlay {
            texture: 1,
            rect: [10.0, 20.0, 30.0, 60.0],
            premultiplied: false,
            transposed: true,
        });
        // the second corner: right, top; on its side its uv is (0, 1)
        assert_eq!(&v[1][..2], &[30.0, 20.0]);
        assert_eq!(&v[1][7..9], &[0.0, 1.0]);
        assert_eq!(v[1][14], 1.0);
    }

    #[test]
    fn atlas_regions_are_checked() {
        assert!(atlas_region_fits(4, 64, [0, 0, 4, 4]));
        assert!(!atlas_region_fits(4, 64, [1, 0, 4, 4]));
        assert!(!atlas_region_fits(4, 60, [0, 0, 1, 1]));
        assert!(!atlas_region_fits(4, 64, [u32::MAX, 0, 2, 1]));
    }
}
