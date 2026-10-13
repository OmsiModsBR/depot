//! The frame's lights on the renderer: the light grid and the lamps' shadows, the coronas and the smoke uploaded for the frame.

use crate::gpu::GpuArray;
use crate::lighting::{
    drawn_by, gpu_light, smoke_sprite, Corona, GpuCorona, GpuPointLight, LampShadow, LAMP_SHADOWS,
    LAMP_SHADOW_REACH, LIGHT_CELL, LIGHT_CELL_CAP, LIGHT_GRID_SIDE,
};
use crate::renderer::Renderer;
use crate::scene::{point_in_vehicle_box, Scene};
use glam::{DVec3, Vec3};

impl Renderer {
    /// Upload this frame's coronas.
    /// `cab`: the box of the vehicle the camera is in, whose own flares Omsi.exe draws
    /// after the vehicle (0x6f0a2c -> 0x6f07bc), the others before it (0x6f0400/0x6f0418).
    pub(crate) fn prepare_coronas(
        &self,
        scene: &mut Scene,
        night: f32,
        cab: Option<&(DVec3, f64, [f32; 6])>,
    ) {
        let ro = scene.render_origin;
        // (its lamps sit on the skin of its box: half a metre round it is the vehicle's)
        let cab = cab.map(|&(o, h, mut bb)| {
            for v in &mut bb[..3] {
                *v += 1.0;
            }
            (o, h, bb)
        });
        let late = |c: &Corona| {
            cab.as_ref()
                .is_some_and(|b| point_in_vehicle_box(c.position, b))
        };
        let mut order: Vec<(bool, &Corona)> = scene
            .coronas
            .iter()
            .filter(|c| c.brightness > 0.001)
            .map(|c| (late(c), c))
            .collect();
        order.sort_by_key(|(l, c)| (*l, c.texture));
        let mut runs: Vec<(u16, u32, u32, bool)> = Vec::new();
        for (k, &(l, c)) in order.iter().enumerate() {
            match runs.last_mut() {
                Some(r) if r.0 == c.texture && r.3 == l => r.2 += 1,
                _ => runs.push((c.texture, k as u32, 1, l)),
            }
        }
        let order: Vec<&Corona> = order.into_iter().map(|(_, c)| c).collect();
        if omsi_cfg::env::var_os("OMSI_DEBUG_CONES").is_some() {
            log::info!(
                "coronas: {} in {} runs {:?}, {} beams",
                order.len(),
                runs.len(),
                runs,
                order.iter().filter(|c| c.beam).count()
            );
        }
        scene.corona_runs = runs;
        let data: Vec<GpuCorona> = order
            .into_iter()
            .map(|c| {
                let p = (c.position - ro).as_vec3();
                // (a raindrop's streak keeps its own level; a snowflake, -3, is scaled as a
                // light's corona is, as it always was in the classic picture)
                let b = if (c.cone_cos < -1.5 && c.cone_cos > -2.5) || c.beam || c.halo {
                    c.brightness
                } else {
                    // the original: ((1 - ambient)^2 + 0.8) 0.6 times the light's
                    // brightness (the shader takes the viewing angle and clamps it to 1)
                    c.brightness * (night * night + 0.8) * 0.6
                };
                GpuCorona {
                    pos: p.to_array(),
                    size: c.size,
                    color: [c.color[0], c.color[1], c.color[2], b],
                    dir: [c.direction.x, c.direction.y, c.direction.z, c.cone_cos],
                    up: [c.up.x, c.up.y, c.up.z, c.rotating as f32],
                    extra: [
                        c.inner_cos,
                        if c.beam || c.halo {
                            c.beam_width
                        } else {
                            c.z_offset
                        },
                        c.flags as f32,
                        if c.beam {
                            1.0
                        } else if c.halo {
                            2.0
                        } else {
                            0.0
                        },
                    ],
                }
            })
            .collect();
        scene.corona_count = data.len() as u32;
        if data.is_empty() {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&data);
        match &scene.corona_buf {
            Some(b) if b.size() as usize >= bytes.len() => self.queue.write_buffer(b, 0, bytes),
            _ => {
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("coronas"),
                    size: (bytes.len() * 2) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.queue.write_buffer(&b, 0, bytes);
                scene.corona_buf = Some(b);
            }
        }
    }

    /// Upload this frame's smoke particles, farthest first (they are blended over each other).
    pub(crate) fn prepare_smoke(&self, scene: &mut Scene, eye: DVec3) {
        let ro = scene.render_origin;
        let mut order: Vec<(f64, GpuCorona)> = scene
            .smoke
            .iter()
            .filter_map(|p| smoke_sprite(p, ro).map(|g| (-(p.position - eye).length_squared(), g)))
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        let data: Vec<GpuCorona> = order.into_iter().map(|(_, g)| g).collect();
        scene.smoke_count = data.len() as u32;
        if data.is_empty() {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&data);
        match &scene.smoke_buf {
            Some(b) if b.size() as usize >= bytes.len() => self.queue.write_buffer(b, 0, bytes),
            _ => {
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("smoke"),
                    size: (bytes.len() * 2) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.queue.write_buffer(&b, 0, bytes);
                scene.smoke_buf = Some(b);
            }
        }
    }
}

impl Renderer {
    /// Upload this frame's point lights into the light grid around the camera. Returns the
    /// grid origin (render-origin relative). `enhanced`: the frame is drawn by the enhanced
    /// path, which takes a headlight as one spot light where the vanilla path takes three
    /// points (`LightMode`). Its mirrors (the vanilla shader) and its picture share the
    /// enhanced list, so that it is not sent twice a frame: the vanilla shader passes a spot
    /// by, whose radius it reads as 0, and the mirrors show no headlight pools. The three
    /// stand-in points stay out of it, or a street full of cars would fill the grid cells'
    /// sixteen places before the street lamps got theirs.
    pub(crate) fn prepare_lights(
        &self,
        scene: &mut Scene,
        cam_rel: Vec3,
        enhanced: bool,
        lamp_shadows: bool,
    ) -> ([f32; 4], Vec<LampShadow>) {
        // the street lamps that get a shadow map: the few lighting the camera's
        // surroundings most (by their strength over the distance)
        let mut chosen: Vec<(f32, LampShadow, [i64; 3])> = Vec::new();
        let ro = scene.render_origin;
        let side = LIGHT_GRID_SIDE;
        let half = side as f32 * LIGHT_CELL * 0.5;
        // snapped to whole cells, so that the grid stays the same while nothing moves
        let origin = [
            ((cam_rel.x - half) / LIGHT_CELL).floor() * LIGHT_CELL,
            ((cam_rel.y - half) / LIGHT_CELL).floor() * LIGHT_CELL,
        ];
        let mut gpu_lights: Vec<GpuPointLight> =
            Vec::with_capacity(scene.interior_lights.len() + scene.lights.len().max(1));
        // the vehicles' interior lamps first, at the indices their meshes name (they are in
        // no grid cell: only those meshes look them up)
        for l in &scene.interior_lights {
            gpu_lights.push(gpu_light(l, (l.position - ro).as_vec3()));
        }
        let mut grid = vec![u32::MAX; side * side * LIGHT_CELL_CAP];
        for l in &scene.lights {
            if !drawn_by(l, enhanced) {
                continue;
            }
            let p = (l.position - ro).as_vec3();
            let x0 = ((p.x - l.radius - origin[0]) / LIGHT_CELL).floor();
            let x1 = ((p.x + l.radius - origin[0]) / LIGHT_CELL).floor();
            let y0 = ((p.y - l.radius - origin[1]) / LIGHT_CELL).floor();
            let y1 = ((p.y + l.radius - origin[1]) / LIGHT_CELL).floor();
            if x1 < 0.0 || y1 < 0.0 || x0 >= side as f32 || y0 >= side as f32 {
                continue;
            }
            let idx = gpu_lights.len() as u32;
            gpu_lights.push(gpu_light(l, p));
            if lamp_shadows && enhanced && l.housed && l.intensity > 0.0 {
                let d = (p - cam_rel).length();
                if d < l.radius + LAMP_SHADOW_REACH {
                    let mut score =
                        l.intensity * (l.color[0] + l.color[1] + l.color[2]) * l.core * l.core
                            / (d * d + 25.0);
                    // (a lamp that had a map keeps it until another is clearly stronger: the
                    // set changing with every metre the camera moved switched shadows on and
                    // off between lamps that light the view about alike, #1613)
                    let key = l.position.to_array().map(|c| (c * 100.0).round() as i64);
                    if scene.lamp_shadow_last.contains(&key) {
                        score *= 1.6;
                    }
                    chosen.push((
                        score,
                        LampShadow {
                            index: idx,
                            position: p,
                            range: l.radius,
                        },
                        key,
                    ));
                }
            }
            for y in (y0.max(0.0) as usize)..=(y1.min(side as f32 - 1.0) as usize) {
                for x in (x0.max(0.0) as usize)..=(x1.min(side as f32 - 1.0) as usize) {
                    let base = (y * side + x) * LIGHT_CELL_CAP;
                    if let Some(slot) = grid[base..base + LIGHT_CELL_CAP]
                        .iter()
                        .position(|v| *v == u32::MAX)
                    {
                        grid[base + slot] = idx;
                    } else if omsi_cfg::env::var_os("OMSI_DEBUG_LIGHT_GRID").is_some() {
                        log::info!("light grid: cell ({x}, {y}) full, light at ({:.1}, {:.1}) radius {:.0} {} left out", p.x, p.y, l.radius, if l.direction.length_squared() > 0.5 { "spot" } else { "point" });
                    }
                }
            }
        }
        if gpu_lights.is_empty() {
            gpu_lights.push(GpuPointLight {
                pos: [0.0; 4],
                color: [0.0; 4],
                dir: [0.0; 4],
                extra: [0.0; 4],
            });
        }
        let lbytes: &[u8] = bytemuck::cast_slice(&gpu_lights);
        let gbytes: &[u8] = bytemuck::cast_slice(&grid);
        let mut rebuilt = false;
        match &scene.light_buf {
            Some(b) if b.size() >= lbytes.len() as u64 => {
                if scene.last_lights != lbytes {
                    b.write(&self.queue, 0, lbytes);
                }
            }
            _ => {
                let cap = (lbytes.len() * 2).max(64 * std::mem::size_of::<GpuPointLight>());
                let b = GpuArray::new(&self.device, "lights", cap as u64, 16, false);
                b.write(&self.queue, 0, lbytes);
                scene.light_buf = Some(b);
                rebuilt = true;
            }
        }
        match &scene.grid_buf {
            Some(b) => {
                if scene.last_grid != grid {
                    b.write(&self.queue, 0, gbytes);
                }
            }
            None => {
                let b = GpuArray::new(&self.device, "light grid", gbytes.len() as u64, 4, false);
                b.write(&self.queue, 0, gbytes);
                scene.grid_buf = Some(b);
                rebuilt = true;
            }
        }
        scene.last_lights.clear();
        scene.last_lights.extend_from_slice(lbytes);
        scene.last_grid = grid;
        if rebuilt {
            self.rebuild_camera_bind_group(scene);
        }
        chosen.sort_by(|a, b| b.0.total_cmp(&a.0));
        chosen.truncate(LAMP_SHADOWS);
        scene.lamp_shadow_last = chosen.iter().map(|c| c.2).collect();
        let lamps = chosen.into_iter().map(|c| c.1).collect();
        ([origin[0], origin[1], LIGHT_CELL, side as f32], lamps)
    }
}
