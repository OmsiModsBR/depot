//! The public methods for meshes, textures and materials: adding, updating, freeing and
//! recycling them.

use crate::assets::{
    fit_image, fit_size, make_meshes, mesh_page_bytes, next_gen, snow_texture_flag, texture_bytes,
    upload_texture, AlphaMode, BindKey, GpuTexture, Material, MaterialExtra, MaterialMaps,
    MaterialUniform, MeshPage, PbrMaps, PreparedMesh, PreparedTexture, TexAddressing,
    MESH_PAGE_INDEX_BYTES, MESH_PAGE_VERTEX_BYTES,
};
use crate::gpu::buffer_init;
use crate::renderer::Renderer;
use crate::scene::{MaterialId, MeshId, Scene, TextureId, Vertex};
use glam::Vec3;
use omsi_geometry::MeshData;

impl Renderer {
    /// Overwrite the vertices of a mesh (skinned humans); the vertex count must not grow.
    pub fn update_mesh(
        &self,
        scene: &mut Scene,
        id: MeshId,
        positions: &[Vec3],
        normals: &[Vec3],
        uvs: &[glam::Vec2],
    ) {
        let verts: Vec<Vertex> = positions
            .iter()
            .zip(normals)
            .zip(uvs)
            .map(|((p, n), uv)| Vertex {
                pos: p.to_array(),
                normal: n.to_array(),
                uv: uv.to_array(),
            })
            .collect();
        let bytes: &[u8] = bytemuck::cast_slice(&verts);
        let m = &mut scene.meshes[id];
        if (m.vertex_bytes as usize) < bytes.len() {
            return;
        }
        {
            // (a newer pose of the same mesh replaces one still waiting)
            let mut pending = self.assets.pending_meshes.borrow_mut();
            match pending.iter_mut().find(|(mid, _)| *mid == id) {
                Some(e) => e.1 = bytes.to_vec(),
                None => pending.push((id, bytes.to_vec())),
            }
        }
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in positions {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        if !positions.is_empty() {
            m.bounds_center = (lo + hi) * 0.5;
            m.bounds_radius = (hi - m.bounds_center).length();
            Self::mesh_bounds_changed(scene, id);
        }
    }

    pub fn add_mesh(&self, scene: &mut Scene, data: &MeshData) -> MeshId {
        let (vb, ib) = mesh_page_bytes(data);
        let mesh = if vb > MESH_PAGE_VERTEX_BYTES / 4
            || ib > MESH_PAGE_INDEX_BYTES / 4
            || !self.assets.mesh_pages
        {
            make_meshes(&self.device, &self.queue, &[data], true)
                .pop()
                .expect("one mesh")
        } else {
            let found = scene
                .mesh_pages
                .iter_mut()
                .enumerate()
                .find_map(|(k, p)| p.take(vb, ib).map(|at| (k, at)));
            let (k, at) = found.unwrap_or_else(|| {
                let mut page =
                    MeshPage::new(&self.device, MESH_PAGE_VERTEX_BYTES, MESH_PAGE_INDEX_BYTES);
                let at = page
                    .take(vb, ib)
                    .expect("a new page holds a mesh a quarter its size");
                scene.mesh_pages.push(page);
                (scene.mesh_pages.len() - 1, at)
            });
            scene.mesh_pages[k].place(&self.queue, data, k as u32, at)
        };
        scene.meshes.push(mesh);
        scene.meshes.len() - 1
    }

    /// Whether this renderer's adapter draws meshes from shared pages (it can draw with a
    /// base vertex); [`prepare_meshes`] is told it.
    pub fn mesh_pages(&self) -> bool {
        self.assets.mesh_pages
    }

    /// Put a mesh made on another thread ([`prepare_mesh`]) into the scene.
    pub fn add_prepared_mesh(&self, scene: &mut Scene, mesh: PreparedMesh) -> MeshId {
        scene.meshes.push(mesh.0);
        scene.meshes.len() - 1
    }

    /// Bytes of all textures of the scene on the GPU (render targets included).
    /// Bytes the meshes' vertex and index buffers take on the GPU (the freed ones' shared
    /// placeholder not counted).
    pub fn mesh_bytes(&self, scene: &Scene) -> u64 {
        scene.mesh_page_bytes()
    }

    /// Put a texture made on another thread ([`prepare_texture`]) into the scene.
    pub fn add_prepared_texture(&self, scene: &mut Scene, texture: PreparedTexture) -> TextureId {
        scene.textures.push(texture.0);
        scene.textures.len() - 1
    }

    pub fn add_texture(
        &self,
        scene: &mut Scene,
        img: &omsi_texture::Image,
        mipmaps: bool,
    ) -> TextureId {
        let fitted = fit_image(img, self.device.limits().max_texture_dimension_2d);
        let img = fitted.as_ref().unwrap_or(img);
        let t = if mipmaps && img.width > 1 && img.height > 1 {
            self.upload_texture_gpu_mips(img)
        } else {
            upload_texture(&self.device, &self.queue, img, false)
        };
        scene.textures.push(t);
        scene.textures.len() - 1
    }

    pub fn add_blank_texture(&self, scene: &mut Scene, width: u32, height: u32) -> TextureId {
        let (width, height) = fit_size(
            width.max(1),
            height.max(1),
            self.device.limits().max_texture_dimension_2d,
        );
        // A newly allocated GPU texture has undefined contents. Script displays may be
        // sampled before their first `STUnlock`, so initialise them as transparent rather
        // than briefly showing arbitrary solid pixels on new or AI vehicles.
        let image = omsi_texture::Image {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
            has_alpha: true,
        };
        let texture = upload_texture(&self.device, &self.queue, &image, false);
        scene.textures.push(texture);
        scene.textures.len() - 1
    }

    /// The PBR set found beside diffuse texture `diffuse` (`omsi_texture::pbr`): its maps up
    /// (as data, not colours) and known to the materials made with that texture from now on.
    pub fn add_pbr_maps(
        &self,
        scene: &mut Scene,
        diffuse: TextureId,
        set: &omsi_texture::pbr::PbrImages,
    ) {
        // stored so that the sRGB textures give back the bytes as they are: a normal map's
        // 128 read as 0.22 through the sRGB curve bent every normal
        let lut: Vec<u8> = (0..256)
            .map(|v| {
                let l = v as f32 / 255.0;
                let s = if l <= 0.003_130_8 {
                    l * 12.92
                } else {
                    1.055 * l.powf(1.0 / 2.4) - 0.055
                };
                (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8
            })
            .collect();
        let mut up = |img: &omsi_texture::Image| {
            let data = omsi_texture::Image {
                width: img.width,
                height: img.height,
                rgba: img.rgba.iter().map(|b| lut[*b as usize]).collect(),
                has_alpha: false,
            };
            self.add_texture(scene, &data, true)
        };
        let normal = set.normal.as_ref().map(&mut up);
        let orm = set.orm.as_ref().map(&mut up);
        scene.pbr_maps.insert(
            diffuse,
            PbrMaps {
                normal,
                orm,
                flags: set.flags,
            },
        );
    }

    /// The device takes BC1-3 (DXT) textures.
    pub fn supports_bc(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    }

    /// Upload a texture prepared by `omsi_texture::gpu` (blocks with their levels, or RGBA
    /// whose chain is made here).
    pub fn add_texture_data(
        &self,
        scene: &mut Scene,
        data: &omsi_texture::TextureData,
    ) -> TextureId {
        let t = self.upload_texture_data(data);
        scene.textures.push(t);
        scene.textures.len() - 1
    }

    pub fn texture_bytes(&self, scene: &Scene) -> u64 {
        scene.textures.iter().map(|t| t.bytes).sum()
    }

    /// Bytes of one texture (0 for a freed slot).
    pub fn texture_size_bytes(&self, scene: &Scene, id: TextureId) -> u64 {
        scene.textures.get(id).map(|t| t.bytes).unwrap_or(0)
    }

    /// The view of a texture of the scene (to draw into it with another pipeline).
    pub fn texture_view(&self, scene: &Scene, id: TextureId) -> Option<wgpu::TextureView> {
        scene.textures.get(id).map(|t| t.view.clone())
    }

    pub fn upload_speed_mb_s(&self) -> f64 {
        let n = 1024u32;
        let size = wgpu::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("upload check"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let data = vec![0u8; (n * n * 4) as usize];
        let rounds = 4;
        let t = std::time::Instant::now();
        for _ in 0..rounds {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(n * 4),
                    rows_per_image: Some(n),
                },
                size,
            );
        }
        let secs = t.elapsed().as_secs_f64().max(1e-6);
        self.queue.submit([]);
        texture.destroy();
        (data.len() * rounds) as f64 / 1e6 / secs
    }

    /// Replace the pixels of a texture (same size as when created, no mipmaps regenerated).
    pub fn update_texture(&self, scene: &Scene, id: TextureId, img: &omsi_texture::Image) {
        let Some(gt) = scene.textures.get(id) else {
            return;
        };
        // (a picture larger than the chip takes went up halved, see `fit_image`; one larger
        // than its texture is not written - a device error each frame)
        let fitted = fit_image(img, self.device.limits().max_texture_dimension_2d);
        let img = fitted.as_ref().unwrap_or(img);
        if img.width > gt.size.0
            || img.height > gt.size.1
            || img.rgba.len() < (img.width * img.height * 4) as usize
        {
            return;
        }
        let t = &gt.texture;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: t,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(img.width * 4),
                rows_per_image: Some(img.height),
            },
            wgpu::Extent3d {
                width: img.width,
                height: img.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Replace the top pixels of a dynamic texture and rebuild its mip chain. Returns true
    /// when the texture had to be recreated (and its materials therefore need rebinding).
    /// This is used for OMSI script textures after `STFilter`.
    pub fn update_texture_mips(
        &self,
        scene: &mut Scene,
        id: TextureId,
        img: &omsi_texture::Image,
    ) -> bool {
        let Some(old) = scene.textures.get(id) else {
            return false;
        };
        let fitted = fit_image(img, self.device.limits().max_texture_dimension_2d);
        let img = fitted.as_ref().unwrap_or(img);
        let levels = (32 - img.width.max(img.height).leading_zeros()).max(1);
        if old.size != (img.width, img.height) || old.texture.mip_level_count() != levels {
            scene.textures[id] = self.upload_texture_gpu_mips(img);
            return true;
        }
        self.update_texture(scene, id, img);
        if levels > 1 {
            self.generate_mip_chain(&scene.textures[id].texture, levels);
        }
        false
    }

    /// Put `data` into texture slot `id` in place of what is there (a picture uploaded as
    /// RGBA to spare a frame, compressed since), and point the materials using it at it.
    pub fn replace_texture(
        &self,
        scene: &mut Scene,
        id: TextureId,
        data: &omsi_texture::TextureData,
    ) {
        if id >= scene.textures.len() {
            return;
        }
        scene.textures[id] = self.upload_texture_data(data);
    }

    /// Mip levels and size of a texture: (width, height, levels).
    pub fn texture_levels(&self, scene: &Scene, id: TextureId) -> Option<(u32, u32, u32)> {
        let t = scene.textures.get(id)?;
        (t.bytes > 0).then(|| (t.size.0, t.size.1, t.texture.mip_level_count()))
    }

    /// Let go of the `n` finest mip levels of texture `id` (a texture far away while the
    /// textures are over their memory budget): the coarser levels are copied into a smaller
    /// texture on the GPU, which takes the slot. False when the texture cannot shrink so far
    /// (a block format needs sides that are multiples of four). The materials using it
    /// need [`Renderer::rebind_textures`] afterwards.
    pub fn drop_top_levels(&self, scene: &mut Scene, id: TextureId, n: u32) -> bool {
        let Some(t) = scene.textures.get(id) else {
            return false;
        };
        let (w, h) = t.size;
        let levels = t.texture.mip_level_count();
        let format = t.texture.format();
        if n == 0 || n >= levels || !t.texture.usage().contains(wgpu::TextureUsages::COPY_SRC) {
            return false;
        }
        let (nw, nh) = ((w >> n).max(1), (h >> n).max(1));
        let (bw, bh) = format.block_dimensions();
        if nw % bw != 0 || nh % bh != 0 {
            return false;
        }
        let new_levels = levels - n;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: nw,
                height: nh,
                depth_or_array_layers: 1,
            },
            mip_level_count: new_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("drop levels"),
            });
        for l in 0..new_levels {
            let (lw, lh) = ((nw >> l).max(1), (nh >> l).max(1));
            let size = wgpu::Extent3d {
                width: lw.div_ceil(bw) * bw,
                height: lh.div_ceil(bh) * bh,
                depth_or_array_layers: 1,
            };
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &t.texture,
                    mip_level: l + n,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: l,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                size,
            );
        }
        self.queue.submit([encoder.finish()]);
        scene.textures[id] =
            GpuTexture::new(texture, (nw, nh), texture_bytes(format, nw, nh, new_levels));
        true
    }

    /// Rebuild the bind groups of the materials that sample any of `ids` (after
    /// [`Renderer::replace_texture`]). Returns how many were rebuilt.
    pub fn rebind_textures(&self, scene: &mut Scene, ids: &[TextureId]) -> usize {
        if ids.is_empty() {
            return 0;
        }
        let textures = &scene.textures;
        let pbr_maps = &scene.pbr_maps;
        let set: std::collections::HashSet<TextureId> = ids.iter().copied().collect();
        let mut n = 0;
        for m in scene.materials.iter_mut() {
            let uses = [
                m.texture,
                m.nightmap,
                m.lightmap,
                m.envmap.map(|e| e.0),
                m.transmap.map(|t| t.0),
                m.env_mask,
                m.bump.map(|b| b.0),
            ]
            .iter()
            .flatten()
            .any(|t| set.contains(t));
            if !uses {
                continue;
            }
            m.bind_group = self.material_bind_group(
                textures,
                MaterialMaps {
                    texture: m.texture,
                    transmap: m.transmap,
                    nightmap: m.nightmap,
                    lightmap: m.lightmap,
                    envmap: m.envmap,
                    env_mask: m.env_mask,
                    bump: m.bump,
                    pbr: m.texture.and_then(|t| pbr_maps.get(&t)).copied(),
                },
                m.address,
                &m.buf,
            );
            n += 1;
        }
        n
    }

    /// `[matl_noZwrite]` for a material that is already in the scene.
    pub fn set_no_z_write(&self, scene: &mut Scene, id: MaterialId, on: bool) {
        if let Some(m) = scene.materials.get_mut(id) {
            m.no_z_write = on;
        }
    }

    pub fn add_material(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        alpha: AlphaMode,
        color: [f32; 4],
        unlit: bool,
    ) -> MaterialId {
        self.add_material_ex(scene, texture, alpha, color, unlit, None)
    }

    /// Material with an optional transparency map `(texture, use_alpha_channel)`.
    pub fn add_material_ex(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        alpha: AlphaMode,
        color: [f32; 4],
        unlit: bool,
        transmap: Option<(TextureId, bool)>,
    ) -> MaterialId {
        self.add_material_full(
            scene, texture, alpha, color, unlit, transmap, None, None, None, None, [0.0; 3],
        )
    }

    /// Material with transparency map and `[matl_nightmap]` self-illumination texture.
    pub fn add_material_night(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        alpha: AlphaMode,
        color: [f32; 4],
        unlit: bool,
        transmap: Option<(TextureId, bool)>,
        nightmap: Option<TextureId>,
    ) -> MaterialId {
        self.add_material_full(
            scene, texture, alpha, color, unlit, transmap, None, nightmap, None, None, [0.0; 3],
        )
    }

    /// Material with a `[matl_lightmap]` texture whose strength comes from the per-slot
    /// instance value (illuminated displays).
    pub fn add_material_lit(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        alpha: AlphaMode,
        color: [f32; 4],
        unlit: bool,
        transmap: Option<(TextureId, bool)>,
        nightmap: Option<TextureId>,
        lightmap: Option<TextureId>,
    ) -> MaterialId {
        self.add_material_full(
            scene, texture, alpha, color, unlit, transmap, None, nightmap, lightmap, None, [0.0; 3],
        )
    }

    /// Full vehicle material: also a `[matl_envmap]` sphere map with its strength factor
    /// (masked by the diffuse alpha channel like the original).
    pub fn add_material_env(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        alpha: AlphaMode,
        color: [f32; 4],
        unlit: bool,
        transmap: Option<(TextureId, bool)>,
        nightmap: Option<TextureId>,
        lightmap: Option<TextureId>,
        envmap: Option<(TextureId, f32)>,
    ) -> MaterialId {
        self.add_material_full(
            scene, texture, alpha, color, unlit, transmap, None, nightmap, lightmap, envmap,
            [0.0; 3],
        )
    }

    /// Like `add_material_env` with an emissive colour (`[matl_allcolor]`).
    pub fn add_material_all(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        alpha: AlphaMode,
        color: [f32; 4],
        unlit: bool,
        transmap: Option<(TextureId, bool)>,
        nightmap: Option<TextureId>,
        lightmap: Option<TextureId>,
        envmap: Option<(TextureId, f32)>,
        emissive: [f32; 3],
    ) -> MaterialId {
        self.add_material_full(
            scene, texture, alpha, color, unlit, transmap, None, nightmap, lightmap, envmap,
            emissive,
        )
    }

    /// Like `add_material_all` with the rest of the material manager's settings
    /// (reflection mask, depth handling, specular term).
    #[allow(clippy::too_many_arguments)]
    pub fn add_material_extra(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        alpha: AlphaMode,
        color: [f32; 4],
        unlit: bool,
        transmap: Option<(TextureId, bool)>,
        nightmap: Option<TextureId>,
        lightmap: Option<TextureId>,
        envmap: Option<(TextureId, f32)>,
        emissive: [f32; 3],
        extra: MaterialExtra,
    ) -> MaterialId {
        self.add_material_inner(
            scene,
            texture,
            alpha,
            color,
            unlit,
            transmap,
            None,
            nightmap,
            lightmap,
            envmap,
            emissive,
            extra.moisture,
            extra,
        )
    }

    /// Swap the material of one slot of an instance (material variants).
    pub fn set_material(
        &self,
        scene: &mut Scene,
        instance: usize,
        slot: usize,
        material: MaterialId,
    ) {
        if let Some(m) = scene.instances[instance].materials.get_mut(slot) {
            *m = material;
        }
    }

    /// Make a copy of a material with a different diffuse texture. Used by scenery CTC and
    /// `[texchanges]` selectors: the slot's alpha, lighting, reflection, and depth settings
    /// stay as they were, while the replacement texture may bring its own PBR maps.
    pub fn add_material_retextured(
        &self,
        scene: &mut Scene,
        base: MaterialId,
        texture: Option<TextureId>,
    ) -> Option<MaterialId> {
        let (
            alpha,
            color,
            unlit,
            no_z_write,
            writes_depth,
            no_z_check,
            z_bias,
            nightmap,
            lightmap,
            envmap,
            env_mask,
            bump,
            emissive,
            transmap,
            address,
            mut uniform,
        ) = {
            let src = scene.materials.get(base)?;
            (
                src.alpha,
                src.color,
                src.unlit,
                src.no_z_write,
                src.writes_depth,
                src.no_z_check,
                src.z_bias,
                src.nightmap,
                src.lightmap,
                src.envmap,
                src.env_mask,
                src.bump,
                src.emissive,
                src.transmap,
                src.address,
                src.uniform,
            )
        };
        uniform.pbr = texture
            .and_then(|id| scene.pbr_maps.get(&id))
            .map(|maps| maps.flags)
            .unwrap_or([0.0; 4]);
        if uniform.ambient[3] < 1.5 {
            uniform.ambient[3] = if texture.is_none() {
                -1.0
            } else {
                snow_texture_flag(scene, texture)
            };
        }
        let slot = |t: Option<TextureId>| {
            t.and_then(|t| scene.textures.get(t).map(|g| (t, g.gen)))
                .unwrap_or((usize::MAX, 0))
        };
        let key = BindKey {
            textures: [
                slot(texture),
                slot(transmap.map(|t| t.0)),
                slot(nightmap),
                slot(lightmap),
                slot(envmap.map(|e| e.0)),
                slot(env_mask),
                slot(bump.map(|b| b.0)),
            ],
            address,
            uniform: bytemuck::cast(uniform),
        };
        let look = scene.look(&key);
        let (bind_group, buf) = match scene.bind_groups.get(&key) {
            Some((bg, b)) => (bg.clone(), b.clone()),
            None => {
                let buf = buffer_init(
                    &self.device,
                    &self.queue,
                    None,
                    bytemuck::bytes_of(&uniform),
                    wgpu::BufferUsages::UNIFORM,
                );
                let bind_group = self.material_bind_group(
                    &scene.textures,
                    MaterialMaps {
                        texture,
                        transmap,
                        nightmap,
                        lightmap,
                        envmap,
                        env_mask,
                        bump,
                        pbr: texture.and_then(|id| scene.pbr_maps.get(&id)).copied(),
                    },
                    address,
                    &buf,
                );
                scene
                    .bind_groups
                    .insert(key, (bind_group.clone(), buf.clone()));
                (bind_group, buf)
            }
        };
        scene.materials.push(Material {
            texture,
            alpha,
            color,
            unlit,
            no_z_write,
            writes_depth,
            no_z_check,
            z_bias,
            nightmap,
            lightmap,
            envmap,
            env_mask,
            bump,
            emissive,
            transmap,
            address,
            uniform,
            buf,
            bind_group,
            look,
        });
        Some(scene.materials.len() - 1)
    }

    /// Terrain material: uv is tile space, the ground texture repeats `repeats` times per
    /// tile, its detail texture `detail` times, and the optional mask (alpha 0 = cut) is
    /// sampled in tile space.
    /// `nightmap`: the tile's `.map.LM.bmp` (street lamp light pools), added at night.
    /// `moisture`: 1 when this layer's `<texture>.cfg` sidecar carries `[moisture]` or
    /// `[puddles]` (the map's base ground layer wets in the rain just like a painted one).
    #[allow(clippy::too_many_arguments)]
    pub fn add_terrain_material(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        mask: Option<TextureId>,
        detail: Option<(TextureId, f32)>,
        repeats: f32,
        nightmap: Option<TextureId>,
        moisture: f32,
    ) -> MaterialId {
        self.add_material_wet(
            scene,
            texture,
            if mask.is_some() {
                AlphaMode::Test
            } else {
                AlphaMode::Opaque
            },
            [1.0; 4],
            false,
            mask.map(|m| (m, true)),
            Some((detail.map(|d| d.1).unwrap_or(0.0), repeats)),
            nightmap,
            detail.map(|d| d.0),
            None,
            [0.0; 3],
            moisture,
        )
    }

    /// A painted ground layer: the tile mesh drawn again with one of the map's other
    /// `[groundtex]` textures, blended in wherever the layer's painting mask (the alpha
    /// DDS the editor's brush writes to `texture/map/tile_x_y.map.<n>.dds`) says so.
    #[allow(clippy::too_many_arguments)]
    pub fn add_terrain_layer_material(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        mask: TextureId,
        detail: Option<(TextureId, f32)>,
        repeats: f32,
        nightmap: Option<TextureId>,
        moisture: f32,
    ) -> MaterialId {
        // The brush mask includes fully transparent road cutouts. Those fragments must
        // never write biased terrain depth over the splines drawn in the next phase:
        // they contribute no colour, but would reject the road and expose the sky.
        // The C++ handler likewise draws painted terrain with depth writes disabled.
        self.add_material_inner(
            scene,
            texture,
            AlphaMode::Blend,
            [1.0; 4],
            false,
            Some((mask, true)),
            Some((detail.map(|d| d.1).unwrap_or(0.0), repeats)),
            nightmap,
            detail.map(|d| d.0),
            None,
            [0.0; 3],
            moisture,
            MaterialExtra {
                no_z_write: true,
                ..MaterialExtra::default()
            },
        )
    }

    /// `moisture`: 1 when the `<texture>.cfg` sidecar marks this surface as one that
    /// darkens and starts to mirror the sky while the rain is on it.
    #[allow(clippy::too_many_arguments)]
    pub fn add_material_wet(
        &self,
        scene: &mut Scene,
        texture: Option<TextureId>,
        alpha: AlphaMode,
        color: [f32; 4],
        unlit: bool,
        transmap: Option<(TextureId, bool)>,
        terrain: Option<(f32, f32)>,
        nightmap: Option<TextureId>,
        lightmap: Option<TextureId>,
        envmap: Option<(TextureId, f32)>,
        emissive: [f32; 3],
        moisture: f32,
    ) -> MaterialId {
        self.add_material_inner(
            scene,
            texture,
            alpha,
            color,
            unlit,
            transmap,
            terrain,
            nightmap,
            lightmap,
            envmap,
            emissive,
            moisture,
            MaterialExtra::default(),
        )
    }

    /// Release a mesh's buffers; drawing it afterwards draws nothing.
    pub fn free_mesh(&self, scene: &mut Scene, id: MeshId) {
        if id >= scene.meshes.len() {
            return;
        }
        let (vertex_buf, index_buf) = {
            let f = self.freed(scene);
            (f.vertex_buf.clone(), f.index_buf.clone())
        };
        if let Some(page) = scene.mesh_pages.get_mut(scene.meshes[id].page as usize) {
            page.give(&scene.meshes[id]);
        }
        let m = &mut scene.meshes[id];
        m.vertex_buf = vertex_buf;
        m.index_buf = index_buf;
        m.page = u32::MAX;
        m.base_vertex = 0;
        m.first_index = 0;
        m.vertex_offset = 0;
        m.vertex_bytes = 0;
        m.index_bytes = 0;
        m.ranges.clear();
        m.bounds_center = Vec3::ZERO;
        m.bounds_radius = 0.0;
        Self::mesh_slot_replaced(scene, id);
    }

    /// Release a texture (a material still using it keeps it alive until it is freed too).
    pub fn free_texture(&self, scene: &mut Scene, id: TextureId) {
        scene.snow_textures.remove(&id);
        // (its PBR maps go with it: the slot is taken by another texture next)
        if let Some(m) = scene.pbr_maps.remove(&id) {
            for t in [m.normal, m.orm].into_iter().flatten() {
                self.free_texture(scene, t);
            }
        }
        if let Some(t) = scene.textures.get_mut(id) {
            *t = GpuTexture {
                texture: self.assets.white_texture.texture.clone(),
                view: self.assets.white_texture.view.clone(),
                size: (1, 1),
                bytes: 0,
                gen: next_gen(),
            };
        }
    }

    /// Release a material's bind group (and with it the textures it held).
    pub fn free_material(&self, scene: &mut Scene, id: MaterialId) {
        if id >= scene.materials.len() {
            return;
        }
        let (bind_group, buf) = {
            let f = self.freed(scene);
            (f.bind_group.clone(), f.buf.clone())
        };
        let look = scene.intern_look(0);
        scene.materials[id] = Material {
            texture: None,
            alpha: AlphaMode::Opaque,
            color: [1.0; 4],
            unlit: false,
            no_z_write: false,
            writes_depth: false,
            no_z_check: false,
            z_bias: 0,
            nightmap: None,
            lightmap: None,
            envmap: None,
            env_mask: None,
            bump: None,
            emissive: [0.0; 3],
            transmap: None,
            address: TexAddressing::Wrap,
            uniform: <MaterialUniform as bytemuck::Zeroable>::zeroed(),
            buf,
            bind_group,
            look,
        };
    }

    pub fn recycle_mesh(&self, scene: &mut Scene, new: MeshId, into: MeshId) -> MeshId {
        if new + 1 != scene.meshes.len() || into >= new {
            return new;
        }
        let m = scene.meshes.pop().unwrap();
        scene.meshes[into] = m;
        Self::mesh_slot_replaced(scene, into);
        into
    }

    pub fn recycle_texture(&self, scene: &mut Scene, new: TextureId, into: TextureId) -> TextureId {
        if new + 1 != scene.textures.len() || into >= new {
            return new;
        }
        let t = scene.textures.pop().unwrap();
        scene.textures[into] = t;
        into
    }

    pub fn recycle_material(
        &self,
        scene: &mut Scene,
        new: MaterialId,
        into: MaterialId,
    ) -> MaterialId {
        if new + 1 != scene.materials.len() || into >= new {
            return new;
        }
        let m = scene.materials.pop().unwrap();
        scene.materials[into] = m;
        into
    }
}
