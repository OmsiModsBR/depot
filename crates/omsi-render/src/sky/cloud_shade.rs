//! The clouds' shade map: the sun's light through the cloud layer, for the whole scene.
//!
//! The Enhanced scene used to scale its sun by one number, the light the clouds let through
//! over the camera, so a cloud overhead took the sun - and every shadow - from the whole
//! picture. The map says it per place instead. It is a square of `CLOUD_SHADE_SIZE` metres laid
//! on the base of the cloud layer, centred where the light's way from the camera meets it:
//! each texel holds the share of the light that gets through the layer along the way to the
//! sun (`fs_cloud_shade` in sky_light.wgsl), blurred by the width of the shadow's edge, which
//! grows with the height the shadow falls from (the sun's disc and the light the cloud
//! scatters beside its own shadow). A lit point looks it up along the light's way to the
//! cloud base (`cloud_shade_at` in enhanced.wgsl) - the technique of Unreal's Beer shadow
//! maps and of Unity HDRP's cloud layer shadow.
//!
//! The map is drawn when what it shows has changed (`ShadeState::due`) and slid by the
//! clouds' drift in between, so that a frame costs one fetch per lit pixel and nothing else.

use crate::gpu::{buffer_init, fullscreen_pipeline, sampler_entry, target_2d, uniform_entry};
use crate::sky::CLOUD_ORIGIN_PERIOD;
use glam::{DVec2, DVec3, Vec2, Vec3};

/// Heights (m, over the render origin as the sky shader's `CLOUD_BOTTOM` and `CLOUD_TOP`) of
/// the cloud layer's base and top.
pub const CLOUD_LAYER_BASE: f32 = 1400.0;
pub const CLOUD_LAYER_TOP: f32 = 2800.0;

/// The map's side in texels.
pub const CLOUD_SHADE_TEXELS: u32 = 1024;

/// The map's side in metres (sky_light.wgsl `SHADE_SIZE`): 12 km at a texel of 11.7 m, a
/// cloud's shadow edge being tens of metres wide.
pub const CLOUD_SHADE_SIZE: f64 = 12_000.0;

/// How far the layer's shadow edge spreads for each metre of the height it falls over: the
/// sun's angular radius (0.0047 rad) and the light a cloud scatters beside its own shadow.
const EDGE_SPREAD: f32 = 0.0047 + 0.01;

/// The map is drawn again once the clouds have drifted this far (m) from where it was drawn.
const DRIFT_REDRAW: f32 = 1000.0;

/// A cloud's drift per unit of `Lighting::cloud_offset` (m), as the sky shaders have it.
pub const DRIFT_SCALE: f32 = 2500.0;

/// The longest way (m) the square's centre is put from the camera along the light: the sun
/// low over the horizon would put it out of reach, and the map's border takes over there.
const MAX_SHIFT: f64 = 30_000.0;

/// What the map is drawn from: the light, the clouds and where the camera stands.
#[derive(Clone, Copy, Debug)]
pub struct ShadeInputs {
    /// Unit vector towards the light (world).
    pub light: Vec3,
    /// The weather's cloud cover, 0..1.
    pub cover: f32,
    /// The clouds' drift (`Lighting::cloud_offset`).
    pub offset: Vec2,
    /// Heights (render-origin relative) of the layer's base and top.
    pub base: f32,
    pub top: f32,
    /// The camera in the world, and its height over the render origin.
    pub camera: DVec3,
    pub camera_z: f32,
}

impl ShadeInputs {
    /// Whether there is anything for the map to say: the light over the horizon and clouds.
    pub fn active(&self) -> bool {
        self.light.z > 0.02 && self.cover > 0.001 && self.top > self.base
    }

    /// The map's centre in the clouds' frame (world modulo `CLOUD_ORIGIN_PERIOD`): the light's
    /// way from the camera to the layer's base.
    fn centre(&self) -> DVec2 {
        let way = ((self.base - self.camera_z) as f64 / self.light.z.max(0.02) as f64)
            .clamp(-MAX_SHIFT, MAX_SHIFT);
        DVec2::new(
            (self.camera.x + self.light.x as f64 * way).rem_euclid(CLOUD_ORIGIN_PERIOD),
            (self.camera.y + self.light.y as f64 * way).rem_euclid(CLOUD_ORIGIN_PERIOD),
        )
    }

    /// The sigma (texels) of the blur: the edge's width grows with the height the shadow falls
    /// over (the base over the camera, a hundred metres at least).
    pub fn blur_sigma(&self) -> f32 {
        let radius = (self.base - self.camera_z).max(100.0) * EDGE_SPREAD;
        (radius * 0.5 * CLOUD_SHADE_TEXELS as f32 / CLOUD_SHADE_SIZE as f32).max(0.5)
    }
}

/// What the map on the GPU holds: when it was drawn from, and how to read it now.
#[derive(Default)]
pub struct ShadeState {
    drawn: Option<Drawn>,
}

#[derive(Clone, Copy)]
struct Drawn {
    inputs: ShadeInputs,
    centre: DVec2,
    mean: f32,
}

impl ShadeState {
    /// Whether the map is to be drawn again for `now`: the light has moved on by a quarter of
    /// a degree, the cover by half a per cent, the base by a metre, the centre by an eighth of
    /// the map, or the clouds have drifted a kilometre.
    pub fn due(&self, now: &ShadeInputs) -> bool {
        if !now.active() {
            return false;
        }
        let Some(d) = &self.drawn else {
            return true;
        };
        !d.inputs.active()
            || d.inputs.light.dot(now.light) < 0.999_990_5
            || (d.inputs.cover - now.cover).abs() > 0.005
            || (d.inputs.base - now.base).abs() > 1.0
            || (d.inputs.top - now.top).abs() > 1.0
            || (d.centre - now.centre()).length() > CLOUD_SHADE_SIZE / 8.0
            || ((now.offset - d.inputs.offset) * DRIFT_SCALE).length() > DRIFT_REDRAW
    }

    /// Note the map drawn for `now`; `mean` is the share of the light the clouds let through
    /// on average over the map, which its border blends to.
    pub fn drawn(&mut self, now: &ShadeInputs, mean: f32) {
        self.drawn = Some(Drawn {
            inputs: *now,
            centre: now.centre(),
            mean,
        });
    }

    /// The scene uniform's `cloud_shade` and `cloud_layer` for `now`: the map's centre, the
    /// mean light through the clouds and whether there is a map to read; the layer's base and
    /// top and how far the clouds have drifted since the map was drawn.
    pub fn uniform(&self, now: &ShadeInputs) -> ([f32; 4], [f32; 4]) {
        match &self.drawn {
            Some(d) if now.active() && d.inputs.active() => {
                let drift = (now.offset - d.inputs.offset) * DRIFT_SCALE;
                (
                    [d.centre.x as f32, d.centre.y as f32, d.mean, 1.0],
                    [d.inputs.base, d.inputs.top, drift.x, drift.y],
                )
            }
            _ => ([0.0, 0.0, 1.0, 0.0], [now.base, now.top, 0.0, 0.0]),
        }
    }
}

/// The map and the passes that draw it.
pub struct CloudShade {
    /// The finished map (R16Float), camera binding 21.
    pub view: wgpu::TextureView,
    mid_view: wgpu::TextureView,
    depth_pipeline: wgpu::RenderPipeline,
    depth_bg: wgpu::BindGroup,
    depth_params: wgpu::Buffer,
    blur_pipeline: wgpu::RenderPipeline,
    /// Along x (from the depth pass's picture into `mid_view`) and along y (back).
    blur_bgs: [wgpu::BindGroup; 2],
    blur_params: [wgpu::Buffer; 2],
    pub state: ShadeState,
    /// The map is to be drawn this frame (`CloudShade::set_drawn` was called since the last
    /// `CloudShade::encode`).
    pending: bool,
}

const SHADE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

impl CloudShade {
    /// `sky_shader`: the sky module with `fs_cloud_shade`; `sky_layout`: its second group.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sky_layout: &wgpu::BindGroupLayout,
        sky_shader: &wgpu::ShaderModule,
        camera_buf: &wgpu::Buffer,
        lin_sampler: &wgpu::Sampler,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: CLOUD_SHADE_TEXELS,
            height: CLOUD_SHADE_TEXELS,
            depth_or_array_layers: 1,
        };
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let map = target_2d(device, "cloud shade", size, 1, SHADE_FORMAT, usage);
        let mid = target_2d(device, "cloud shade blur", size, 1, SHADE_FORMAT, usage);
        let view = map.create_view(&Default::default());
        let mid_view = mid.create_view(&Default::default());
        // (the map is read before the first draw as all light: nothing is shaded until a
        // redraw says so, and `cloud_shade.w` keeps the shader from reading it till then)
        // the depth pass: the sky module's clouds, with the camera and its own parameters
        let depth_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cloud shade"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                uniform_entry(15, wgpu::ShaderStages::VERTEX_FRAGMENT),
            ],
        });
        let depth_params = buffer_init(
            device,
            queue,
            Some("cloud shade"),
            bytemuck::cast_slice(&[0.0f32; 4]),
            wgpu::BufferUsages::UNIFORM,
        );
        let depth_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cloud shade"),
            layout: &depth_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: depth_params.as_entire_binding(),
                },
            ],
        });
        let depth_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cloud shade"),
            bind_group_layouts: &[Some(&depth_layout), Some(sky_layout)],
            immediate_size: 0,
        });
        let target = wgpu::ColorTargetState {
            format: SHADE_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        };
        let depth_pipeline = fullscreen_pipeline(
            device,
            "cloud shade",
            &depth_pl,
            sky_shader,
            "vs_probe",
            "fs_cloud_shade",
            target.clone(),
        );
        // the blur
        let blur_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cloud shade blur"),
            source: wgpu::ShaderSource::Wgsl(
                [
                    include_str!("../shaders/common/fullscreen.wgsl"),
                    include_str!("../shaders/sky/cloud_shade_blur.wgsl"),
                ]
                .join("\n")
                .into(),
            ),
        });
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cloud shade blur"),
            entries: &[
                crate::gpu::float_tex(0),
                sampler_entry(1),
                uniform_entry(2, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let blur_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cloud shade blur"),
            bind_group_layouts: &[Some(&blur_layout)],
            immediate_size: 0,
        });
        let blur_pipeline = fullscreen_pipeline(
            device,
            "cloud shade blur",
            &blur_pl,
            &blur_module,
            "vs_blur",
            "fs_blur",
            target,
        );
        let blur_params = [0, 1].map(|_| {
            buffer_init(
                device,
                queue,
                Some("cloud shade blur"),
                bytemuck::cast_slice(&[0.0f32; 4]),
                wgpu::BufferUsages::UNIFORM,
            )
        });
        let blur_bg = |src: &wgpu::TextureView, params: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cloud shade blur"),
                layout: &blur_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(lin_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params.as_entire_binding(),
                    },
                ],
            })
        };
        let blur_bgs = [
            blur_bg(&view, &blur_params[0]),
            blur_bg(&mid_view, &blur_params[1]),
        ];
        Self {
            view,
            mid_view,
            depth_pipeline,
            depth_bg,
            depth_params,
            blur_pipeline,
            blur_bgs,
            blur_params,
            state: ShadeState::default(),
            pending: false,
        }
    }

    /// Note that the map is to be drawn for `inputs` in this frame's encoding, its parameters
    /// going to the GPU now.
    pub fn set_drawn(&mut self, queue: &wgpu::Queue, inputs: &ShadeInputs, mean: f32) {
        self.state.drawn(inputs, mean);
        let centre = inputs.centre();
        queue.write_buffer(
            &self.depth_params,
            0,
            bytemuck::cast_slice(&[centre.x as f32, centre.y as f32, inputs.base, inputs.top]),
        );
        let sigma = inputs.blur_sigma();
        let taps = (sigma * 3.0).ceil().min(24.0);
        let step = 1.0 / CLOUD_SHADE_TEXELS as f32;
        for (k, axis) in [[step, 0.0], [0.0, step]].into_iter().enumerate() {
            queue.write_buffer(
                &self.blur_params[k],
                0,
                bytemuck::cast_slice(&[axis[0], axis[1], sigma, taps]),
            );
        }
        self.pending = true;
    }

    /// Draw the map if `set_drawn` asked for it: the light through the layer, then the blur
    /// along x and along y (the result in `view`).
    pub fn encode(&mut self, sky_bg: Option<&wgpu::BindGroup>, encoder: &mut wgpu::CommandEncoder) {
        if !std::mem::take(&mut self.pending) {
            return;
        }
        let draw = |encoder: &mut wgpu::CommandEncoder,
                    label: &str,
                    target: &wgpu::TextureView,
                    pipeline: &wgpu::RenderPipeline,
                    groups: &[&wgpu::BindGroup]| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            for (slot, bg) in groups.iter().enumerate() {
                pass.set_bind_group(slot as u32, *bg, &[]);
            }
            pass.draw(0..3, 0..1);
        };
        let clear = |encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView| {
            drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cloud shade clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            }));
        };
        // (a scene with no sky textures has no clouds to draw the shade of: all light)
        let Some(sky_bg) = sky_bg else {
            clear(encoder, &self.view);
            return;
        };
        draw(
            encoder,
            "cloud shade",
            &self.view,
            &self.depth_pipeline,
            &[&self.depth_bg, sky_bg],
        );
        draw(
            encoder,
            "cloud shade blur x",
            &self.mid_view,
            &self.blur_pipeline,
            &[&self.blur_bgs[0]],
        );
        draw(
            encoder,
            "cloud shade blur y",
            &self.view,
            &self.blur_pipeline,
            &[&self.blur_bgs[1]],
        );
    }
}
