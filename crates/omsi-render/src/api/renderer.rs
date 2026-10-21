//! The renderer's public methods: opening it, its settings, preparing the scene, and the
//! frame's entry points (the window, a triple screen, a headset's eyes, an image or a texture).
use crate::assets::{fit_size, next_gen, texture_bytes, GpuTexture};
use crate::gpu::{
    fake_poll_loss, gl_backend, guarded_poll, let_go, lost_why, read_buffer_with, target_2d,
    wait_poll, wait_with, GpuWaitError,
};
use crate::lighting::Lighting;
use crate::output::Output;
use crate::renderer::{scene_scale_for, OverlayHook, RenderOptions, Renderer};
use crate::scene::{Camera, Scene, TextureId, TripleScreen};
use glam::Mat4;

use anyhow::anyhow;
use anyhow::Result;

impl Renderer {
    /// Create a renderer with the default options. `surface` is used to pick a compatible
    /// adapter and format.
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        format: Option<wgpu::TextureFormat>,
    ) -> Result<Renderer> {
        Self::new_with(instance, surface, format, RenderOptions::default()).await
    }

    /// Create a renderer with the given graphics settings.
    pub async fn new_with(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        format: Option<wgpu::TextureFormat>,
        options: RenderOptions,
    ) -> Result<Renderer> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: surface, force_fallback_adapter: false, ..Default::default() })
            .await
            .map_err(|e| anyhow!("no graphics adapter that can draw the game was found (Metal, Vulkan, DirectX 12 or OpenGL 3.3 or later); updating the graphics driver often helps: {e}"))?;
        Self::new_on(adapter, surface, format, options).await
    }

    /// The adapters of `instance` that can show `surface`, the ones worth trying first first:
    /// a graphics card of its own, then the processor's graphics, then anything else (a
    /// software renderer last).
    pub fn adapters_for(
        instance: &wgpu::Instance,
        surface: &wgpu::Surface<'_>,
    ) -> Vec<wgpu::Adapter> {
        let mut v: Vec<wgpu::Adapter> =
            pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
                .into_iter()
                .filter(|a| a.is_surface_supported(surface))
                .collect();
        let rank = |a: &wgpu::Adapter| match a.get_info().device_type {
            wgpu::DeviceType::DiscreteGpu => 0,
            wgpu::DeviceType::IntegratedGpu => 1,
            wgpu::DeviceType::VirtualGpu | wgpu::DeviceType::Other => 2,
            wgpu::DeviceType::Cpu => 3,
        };
        v.sort_by_key(rank);
        v
    }

    /// Create a renderer on this adapter.
    pub async fn new_on(
        adapter: wgpu::Adapter,
        surface: Option<&wgpu::Surface<'_>>,
        format: Option<wgpu::TextureFormat>,
        options: RenderOptions,
    ) -> Result<Renderer> {
        Self::open_on(adapter, surface, format, options).await
    }

    /// Whether the card ran out of memory since the last call (the game then keeps fewer
    /// textures, before the driver gives up the device).
    pub fn take_out_of_memory(&self) -> bool {
        self.gpu
            .out_of_memory
            .swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn device_lost(&self) -> Option<String> {
        lost_why(&self.gpu.device_lost)
    }

    /// Wait until the GPU has done `submission` (None: everything submitted so far). A lost
    /// device is not waited for, and one lost during the wait is an error, not wgpu's panic
    /// (#48); either way `device_lost` then says why. (`OMSI_FAKE_GPU_ERROR=lost-poll`: lost
    /// here three seconds in.)
    pub fn wait_gpu(&self, submission: Option<wgpu::SubmissionIndex>) -> Result<(), GpuWaitError> {
        let fake = fake_poll_loss(self.started);
        wait_with(gl_backend(), |timeout| {
            guarded_poll(Some(&self.gpu.device_lost), fake, || {
                self.device.poll(wait_poll(submission.clone(), timeout))
            })
        })
    }

    /// Let the device run what has finished, without waiting; guarded as `wait_gpu`.
    pub fn poll_gpu(&self) -> Result<(), GpuWaitError> {
        let fake = fake_poll_loss(self.started);
        guarded_poll(Some(&self.gpu.device_lost), fake, || {
            self.device.poll(wgpu::PollType::Poll)
        })
        .map(|_| ())
    }

    /// Read `buf` (made with `MAP_READ`, its copy already submitted) back whole; a mapping
    /// refused or a device lost on the way is an error, not wgpu's panic.
    pub fn read_buffer(&self, buf: &wgpu::Buffer) -> Result<Vec<u8>, GpuWaitError> {
        read_buffer_with(buf, || self.wait_gpu(None))
    }

    /// Let the renderer go at the end. Letting the device's queue go waits for the GPU, and
    /// wgpu panics when a card that hangs does not answer in time: the game had ended in
    /// order, and that panic still went to the log and the launcher as its crash. Whether it
    /// went in order.
    pub fn close(self) -> bool {
        let_go(self)
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.gpu.format
    }

    /// Have `hook` draw over the window's picture each frame, after the HUD and before the
    /// picture is delivered (for an interface layer the renderer knows nothing about). It
    /// gets the device, the queue, the frame's command encoder, the picture (in `format()`)
    /// and its size in pixels. `None` takes it away. The window's picture calls it (a triple
    /// screen's once its three panels are in the window, not each panel), not the mirrors' or
    /// the headset's eyes: what the headset shows of it is drawn with `draw_overlay_into`. The
    /// hook runs on the render thread, inside the frame, and must not block.
    pub fn set_overlay_hook(&mut self, hook: Option<OverlayHook>) {
        self.overlay_hook = hook;
    }

    /// Have the overlay hook draw into the render texture `id` (`add_render_texture`),
    /// emptied first, instead of over the window's picture: a picture the scene shows
    /// elsewhere, such as the headset's menu plane. Its pixels hold premultiplied alpha.
    /// Nothing happens without a hook or with no such texture.
    pub fn draw_overlay_into(&mut self, scene: &Scene, id: TextureId) {
        let Some(t) = scene.textures.get(id) else {
            return;
        };
        let (view, size) = (t.view.clone(), t.size);
        self.run_overlay_hook(&view, size, true);
    }

    /// The fraction of a window of this size the 3D picture is drawn at (`render_scale`).
    pub fn scene_scale(&self, width: u32, height: u32) -> f32 {
        let s = scene_scale_for(self.options.render_scale, width, height);
        // (a fixed render scale is the player's choice: the governor works on automatic)
        if self.options.render_scale > 0.0 {
            s
        } else {
            (s * self.dynamic_scale.get()).clamp(0.5, 1.0)
        }
    }

    /// Draw the 3D picture at this fraction of what the render scale says (0.6..1): the
    /// game lowers it while a slow graphics chip cannot keep the frame rate up, and raises
    /// it again when there is room. Steps of a twentieth, so that the few sizes it takes
    /// keep their render targets.
    pub fn set_dynamic_scale(&self, s: f32) {
        // (1, 0.85, 0.7 or 0.55 - the last for a phone's chip that is still too slow: see
        // the game's governor)
        let s = s.clamp(0.55, 1.0);
        let level = [1.0f32, 0.85, 0.7, 0.55]
            .into_iter()
            .min_by(|a, b| (a - s).abs().total_cmp(&(b - s).abs()))
            .unwrap_or(1.0);
        self.dynamic_scale.set(level);
    }

    pub fn dynamic_scale(&self) -> f32 {
        self.dynamic_scale.get()
    }

    /// The size the 3D scene is drawn at for a window of this size.
    pub fn scene_size(&self, width: u32, height: u32) -> (u32, u32) {
        let s = self.scene_scale(width, height);
        if s >= 0.999 {
            return (width, height);
        }
        (
            ((width as f32 * s).round() as u32).max(1),
            ((height as f32 * s).round() as u32).max(1),
        )
    }

    /// Why the graphics device was lost, if it was: nothing can be drawn any more.
    /// Lay the sphere maps of `[matl_envmap]` out by this heading instead of the view
    /// (None: by the view, as Omsi.exe does). A sphere map turns with the view it is drawn
    /// for: in the headset every turn of the player's head turned every reflection of the
    /// bus with it, the two eyes each their own way - the reflections swam about.
    pub fn set_env_heading(&self, heading: Option<f32>) {
        self.env_heading.set(heading);
    }

    /// Upload instance transforms (call after building or changing the scene).
    pub fn prepare(&self, scene: &mut Scene) {
        self.upload_instances(scene);
    }

    /// One step lighter on the graphics card, for a picture the card cannot keep up with at
    /// the smallest render scale: SSAO off. What was switched off, or None when it is off.
    pub fn lighten(&mut self) -> Option<&'static str> {
        if self.options.ssao {
            self.options.ssao = false;
            return Some("ambient occlusion (SSAO) off");
        }
        None
    }

    /// Render the scene into `target` (which must have the renderer's format).
    pub fn render(
        &mut self,
        scene: &mut Scene,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
    ) {
        // (the triple screen's panels are not kept once it is turned off)
        if self.presenter.triple_targets.take().is_some() {
            self.culling.triple_culling = Default::default();
        }
        self.render_inner(
            scene,
            target,
            width,
            height,
            camera,
            lighting,
            Output::Window,
        );
    }

    /// Three independently culled and shaded physical panels, composited into a
    /// spanning window. HUD and menu remain in window pixels and are drawn once, and the
    /// overlay hook draws over the whole window after them.
    pub fn render_triple(
        &mut self,
        scene: &mut Scene,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        rig: &TripleScreen,
    ) {
        self.render_triple_frame(scene, target, width, height, camera, lighting, rig);
    }

    /// Render one OpenXR view using the headset's asymmetric projection matrix.
    /// The matrix uses the same reversed depth range as the desktop camera. The
    /// second eye reuses the first eye's shadow atlas when both share an origin.
    pub fn render_xr_eye(
        &mut self,
        scene: &mut Scene,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        projection: Mat4,
        second_eye: bool,
    ) {
        self.render_inner(
            scene,
            target,
            width,
            height,
            camera,
            lighting,
            Output::XrEye {
                projection,
                second: second_eye,
            },
        );
    }

    /// Composite the shared game menu and a world-positioned pointer into each eye.
    /// Eye-specific positions make the menu fuse into one virtual panel.
    pub fn render_xr_ui(
        &self,
        scene: &Scene,
        eyes: &[wgpu::TextureView; 2],
        desktop_size: (u32, u32),
        eye_size: (u32, u32),
        menu_range: std::ops::Range<usize>,
        menu_transforms: [Mat4; 2],
        cursor_overlay: Option<usize>,
        tooltip_overlay: Option<usize>,
        cursor_transforms: [Option<Mat4>; 2],
        navigator: Option<(TextureId, [Mat4; 2])>,
    ) {
        self.presenter.render_xr_ui(
            &self.device,
            &self.queue,
            &self.sky.sky_sampler,
            scene,
            eyes,
            desktop_size,
            eye_size,
            menu_range,
            menu_transforms,
            cursor_overlay,
            tooltip_overlay,
            cursor_transforms,
            navigator,
        );
    }

    /// Render off-screen and return RGBA8 pixels.
    pub fn render_to_image(
        &mut self,
        scene: &mut Scene,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
    ) -> Result<Vec<u8>> {
        self.render_image(scene, width, height, camera, lighting, None)
    }

    /// Capture all three physical screen projections, including the shared HUD.
    pub fn render_triple_to_image(
        &mut self,
        scene: &mut Scene,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        rig: &TripleScreen,
    ) -> Result<Vec<u8>> {
        self.render_image(scene, width, height, camera, lighting, Some(rig))
    }

    /// A texture the scene can be rendered into (`render_to_texture`), e.g. a rear-view mirror.
    pub fn add_render_texture(&self, scene: &mut Scene, width: u32, height: u32) -> TextureId {
        let (width, height) = fit_size(
            width.max(1),
            height.max(1),
            self.device.limits().max_texture_dimension_2d,
        );
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = target_2d(
            &self.device,
            "render target",
            size,
            1,
            self.gpu.format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let view = texture.create_view(&Default::default());
        // (its depth buffer is the pass's own, see `msaa_targets`)
        let bytes = texture_bytes(self.gpu.format, width, height, 1);
        scene.textures.push(GpuTexture {
            texture,
            view,
            size: (width, height),
            bytes,
            gen: next_gen(),
        });
        scene.textures.len() - 1
    }

    /// Render the scene from `camera` into a texture made by `add_render_texture`.
    /// `aspect`: the projection's width over height (OMSI draws its mirrors 1.6 wide into
    /// square textures, and their meshes show the middle of that).
    pub fn render_to_texture(
        &mut self,
        scene: &mut Scene,
        id: TextureId,
        camera: &Camera,
        lighting: &Lighting,
        aspect: f32,
    ) {
        let Some(t) = scene.textures.get(id) else {
            return;
        };
        let view = t.view.clone();
        let (w, h) = t.size;
        self.texture_aspect = Some(aspect);
        self.render_inner(
            scene,
            &view,
            w,
            h,
            camera,
            lighting,
            Output::Texture { exclude: id },
        );
        self.texture_aspect = None;
    }

    /// Average GPU time per pass so far: (pass, milliseconds, frames measured).
    /// The mirrors' frames (drawn without the overlays) are timed on their own.
    pub fn gpu_pass_times(&self) -> Vec<(String, f64, u32)> {
        let mut out = Vec::new();
        for (k, t) in self.diagnostics.gpu_timers.iter().enumerate() {
            let Some(t) = t else { continue };
            for (label, v) in &t.totals {
                let name = if k == 0 {
                    format!("mirrors: {label}")
                } else {
                    label.to_string()
                };
                out.push((name, v.0 / v.1.max(1) as f64 * 1000.0, v.1));
            }
        }
        out
    }
}

#[cfg(test)]
mod lost_device_tests {
    use super::*;

    fn noop_renderer() -> Renderer {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::NOOP;
        descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
        let instance = wgpu::Instance::new(descriptor);
        pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                shadow_size: 1024,
                ..Default::default()
            },
        ))
        .expect("noop renderer")
    }

    fn readback(r: &Renderer) -> wgpu::Buffer {
        let buf = r.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        r.queue.write_buffer(&buf, 0, &[7; 16]);
        r.queue.submit([]);
        buf
    }

    #[test]
    fn a_working_device_is_waited_for_polled_and_read_back() {
        let r = noop_renderer();
        assert_eq!(r.wait_gpu(None), Ok(()));
        assert_eq!(r.poll_gpu(), Ok(()));
        let buf = readback(&r);
        assert_eq!(r.read_buffer(&buf).map(|d| d.len()), Ok(16));
        assert_eq!(r.device_lost(), None);
        assert!(r.close(), "a working device closes in order");
    }

    #[test]
    fn a_lost_device_is_neither_waited_for_nor_polled_nor_read() {
        let r = noop_renderer();
        let buf = readback(&r);
        crate::gpu::mark_lost(&r.gpu.device_lost, "Unknown: test");
        assert_eq!(r.device_lost().as_deref(), Some("Unknown: test"));
        assert_eq!(r.wait_gpu(None), Err(GpuWaitError::Lost));
        assert_eq!(r.poll_gpu(), Err(GpuWaitError::Lost));
        assert_eq!(r.read_buffer(&buf), Err(GpuWaitError::Lost));
        // (and letting it go after the loss goes in order too)
        assert!(r.close());
    }

    #[test]
    fn a_device_destroyed_on_purpose_is_not_lost() {
        let r = noop_renderer();
        // (`destroy` is the device let go of on purpose: no loss)
        r.device.destroy();
        let _ = r.device.poll(wgpu::PollType::Poll);
        assert_eq!(r.device_lost(), None);
    }
}
