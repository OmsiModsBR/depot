//! The per-size target cache: the multisampled colour and depth attachments, the depth
//! target, and letting go of sizes no longer drawn.

use super::ao::AoTargets;
use super::hdr::HdrTargets;
use crate::gpu::DEPTH_FORMAT;
use crate::renderer::Renderer;
use std::collections::HashMap;

impl Renderer {
    /// Before a new size's targets are made: let go of the sizes nobody asked for lately
    /// (dragging the window's edge made a full set per frame, and the old caches, cleared
    /// only past a dozen entries, could hold gigabytes meanwhile), and past a handful of
    /// sizes the least recently used (the window, the scaled picture and the mirrors are
    /// asked for every frame they are drawn).
    pub(crate) fn evict_targets(&mut self) {
        const STALE: std::time::Duration = std::time::Duration::from_millis(250);
        const KEEP: usize = 10;
        let now = std::time::Instant::now();
        let mut uses: Vec<((u32, u32), std::time::Instant)> = self
            .targets
            .target_use
            .iter()
            .map(|(k, t)| (*k, *t))
            .collect();
        uses.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
        let keep: std::collections::HashSet<(u32, u32)> = uses
            .iter()
            .filter(|(_, t)| now.duration_since(*t) < STALE)
            .take(KEEP)
            .map(|(k, _)| *k)
            .collect();
        self.targets.target_use.retain(|k, _| keep.contains(k));
        self.presenter.scale_targets.retain(|k, _| keep.contains(k));
        self.targets.msaa_targets.retain(|k, _| keep.contains(k));
        self.targets.hdr_targets.retain(|k, _| keep.contains(k));
    }

    /// The multisampled colour and depth attachments for a target of this size.
    pub(crate) fn msaa_targets(
        &mut self,
        w: u32,
        h: u32,
    ) -> (wgpu::TextureView, wgpu::TextureView) {
        self.targets
            .target_use
            .insert((w, h), std::time::Instant::now());
        if let Some(t) = self.targets.msaa_targets.get(&(w, h)) {
            return t.clone();
        }
        self.evict_targets();
        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let color = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("msaa colour"),
            size,
            mip_level_count: 1,
            sample_count: self.options.msaa,
            dimension: wgpu::TextureDimension::D2,
            format: self.gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("msaa depth"),
            size,
            mip_level_count: 1,
            sample_count: self.options.msaa,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let t = (
            color.create_view(&Default::default()),
            depth.create_view(&Default::default()),
        );
        self.targets.msaa_targets.insert((w, h), t.clone());
        t
    }

    #[allow(dead_code)]
    pub(crate) fn ensure_depth(&mut self, w: u32, h: u32) {
        if let Some((_, _, dw, dh)) = &self.targets.depth {
            if *dw == w && *dh == h {
                return;
            }
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.targets.depth = Some((tex, view, w, h));
    }
}

impl Renderer {
    /// The smaller colour target of the render scale for this size, with its bind group.
    pub(crate) fn scale_target(&mut self, w: u32, h: u32) -> (wgpu::TextureView, wgpu::BindGroup) {
        self.targets
            .target_use
            .insert((w, h), std::time::Instant::now());
        if let Some(t) = self.presenter.scale_targets.get(&(w, h)) {
            return t.clone();
        }
        self.evict_targets();
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scaled scene"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let bg = self.picture_group(&view);
        self.presenter
            .scale_targets
            .insert((w, h), (view.clone(), bg.clone()));
        (view, bg)
    }
}

/// The render targets per size, and when each size was last asked for.
pub(crate) struct TargetCache {
    pub(crate) depth: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    /// Multisampled colour and depth targets per size (the window, each mirror).
    pub(crate) msaa_targets: HashMap<(u32, u32), (wgpu::TextureView, wgpu::TextureView)>,
    /// Screen-space ambient occlusion: depth prepass and AO textures of the window's size.
    pub(crate) ao: Option<AoTargets>,
    /// Enhanced path: the HDR targets per size, the post pipelines and their resources.
    pub(crate) hdr_targets: HashMap<(u32, u32), HdrTargets>,
    /// Full-resolution current scene before rain films, in its original colour format.
    pub(crate) glass_picture: Option<wgpu::TextureView>,
    /// When each size of the size-keyed targets (scale, MSAA, HDR) was last asked for.
    pub(crate) target_use: HashMap<(u32, u32), std::time::Instant>,
}

impl TargetCache {
    /// No targets yet: they are made at the first size asked for.
    pub(crate) fn new() -> Self {
        Self {
            glass_picture: None,
            target_use: HashMap::new(),
            hdr_targets: HashMap::new(),
            ao: None,
            depth: None,
            msaa_targets: HashMap::new(),
        }
    }
}
