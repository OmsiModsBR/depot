//! The renderer's tests that draw nothing: shaders, pipeline tables, draw lists, helpers.

use super::*;

#[test]
fn noop_backend_initializes_renderer() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
    let instance = wgpu::Instance::new(descriptor);
    let res = pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 2,
            shadow_size: 1024,
            ..Default::default()
        },
    ));
    assert!(
        res.is_ok(),
        "renderer should initialize on noop backend: {:?}",
        res.err()
    );
}

#[test]
fn declared_transmap_ignores_slot_alpha() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
    let instance = wgpu::Instance::new(descriptor);
    let renderer = pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            shadow_size: 1024,
            ..Default::default()
        },
    ))
    .expect("noop renderer");
    let mut scene = renderer.new_scene();
    let blended = |scene: &mut Scene, transmap: Option<(TextureId, bool)>, extra: MaterialExtra| {
        renderer.add_material_extra(
            scene,
            None,
            AlphaMode::Blend,
            [1.0; 4],
            true,
            transmap,
            None,
            None,
            None,
            [0.0; 3],
            extra,
        )
    };
    // declared, its file missing: no transmap texture bound
    let declared = blended(
        &mut scene,
        None,
        MaterialExtra {
            transmap_declared: true,
            ..Default::default()
        },
    );
    let map = renderer.add_blank_texture(&mut scene, 1, 1);
    let bound = blended(&mut scene, Some((map, true)), MaterialExtra::default());
    // another bit of the same flags, no transmap
    let metal = blended(
        &mut scene,
        None,
        MaterialExtra {
            metal_ok: true,
            ..Default::default()
        },
    );
    let plain = blended(&mut scene, None, MaterialExtra::default());
    assert_eq!(scene.materials[metal].uniform.params2[3], 4.0);
    // a raindrop map is kept on the material and marks its look (bit 8), apart from the rest
    let drops = blended(
        &mut scene,
        None,
        MaterialExtra {
            raindrop_map: Some(map),
            ..Default::default()
        },
    );
    assert_eq!(scene.materials[drops].raindrop_map, Some(map));
    assert!(scene.materials[drops].uses_texture(map));
    assert_eq!(
        scene.materials[drops].uniform.params2[3],
        8.0 + 16.0 * (map as f32 + 1.0)
    );
    let other_map = renderer.add_blank_texture(&mut scene, 1, 1);
    let other = blended(
        &mut scene,
        None,
        MaterialExtra {
            raindrop_map: Some(other_map),
            ..Default::default()
        },
    );
    assert_ne!(scene.materials[drops].look, scene.materials[other].look);
    assert_eq!(scene.materials[plain].raindrop_map, None);
    assert_ne!(scene.materials[drops].look, scene.materials[plain].look);
    assert!(!scene.materials[drops].transmap_declared());
    let flags: Vec<bool> = [declared, bound, metal, plain]
        .iter()
        .map(|&m| scene.materials[m].transmap_declared())
        .collect();
    assert_eq!(flags, [true, true, false, false]);
    let corner = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let data = MeshData {
        positions: corner.repeat(4),
        normals: vec![Vec3::Z; 12],
        uvs: vec![glam::Vec2::ZERO; 12],
        indices: (0..12).collect(),
        ranges: (0..4).map(|s| (s * 3, 3, s)).collect(),
        ..Default::default()
    };
    let mesh = renderer.add_mesh(&mut scene, &data);
    let i = renderer.add_instance(
        &mut scene,
        mesh,
        DVec3::ZERO,
        Mat4::IDENTITY,
        vec![declared, bound, metal, plain],
    );
    renderer.set_params(&mut scene, i, &[0.0, 0.0, 0.35, 0.35], true, &[]);
    assert_eq!(scene.instances[i].slot_alpha, vec![1.0, 1.0, 0.35, 0.35]);
}

#[test]
fn omsi_render_phases_are_monotonic_and_complete() {
    assert_eq!(
        RenderPhase::DRAW_ORDER.map(|phase| phase as usize),
        [0, 1, 2, 3, 4, 5, 6, 7, 8]
    );
    assert_eq!(RenderPhase::default(), RenderPhase::Normal);
}

#[test]
fn metric_lifted_surfaces_keep_shading_class_without_view_space_pull() {
    assert_eq!(surface_instance_code(false, false, false, true, false), 0.9);
    assert_eq!(surface_instance_code(false, false, false, true, true), 1.0);
    assert_eq!(surface_instance_code(false, false, true, false, false), 0.9);
    assert_eq!(surface_instance_code(false, true, false, true, false), 0.75);
}

#[test]
fn spline_blend_sort_ignores_height_and_camera_pitch() {
    let origin = DVec3::new(120.0, 45.0, 0.0);
    let render_origin = DVec3::new(100.0, 40.0, 0.0);
    let level_camera = Vec3::new(3.0, 1.0, 4.0);
    let high_camera = Vec3::new(3.0, 1.0, 80.0);
    assert_eq!(
        horizontal_sort_distance(origin, render_origin, level_camera),
        horizontal_sort_distance(origin + DVec3::Z * 60.0, render_origin, high_camera)
    );
}

#[test]
fn render_scale_auto_keeps_ordinary_windows_sharp() {
    // the default window and a 2560x1080 screen are drawn at full size
    assert_eq!(scene_scale_for(0.0, 1600, 900), 1.0);
    assert_eq!(scene_scale_for(0.0, 2560, 1080), 1.0);
    // Mac/Android cap this at 2.8 million pixels; desktops keep it at full size.
    let s = scene_scale_for(0.0, 3200, 1800);
    if cfg!(target_os = "macos") || cfg!(target_os = "android") {
        assert!((s - 0.697).abs() < 0.01, "{s}");
        assert!((3200.0 * s * 1800.0 * s - AUTO_SCALE_PIXELS).abs() < 1.0);
    } else {
        assert_eq!(s, 1.0);
    }
    // never below half size
    assert_eq!(scene_scale_for(0.0, 16384, 16384), 0.5);
    // what is asked for, within 0.5..1 - on a Mac or a phone a scale over the pixel
    // budget is capped like the automatic one
    let s = scene_scale_for(0.75, 3200, 1800);
    if cfg!(target_os = "macos") || cfg!(target_os = "android") {
        assert!((s - 0.697).abs() < 0.01, "{s}");
        assert!((3200.0 * s * 1800.0 * s - AUTO_SCALE_PIXELS).abs() < 1.0);
    } else {
        assert_eq!(s, 0.75);
    }
    // a small window under the budget keeps what it asked for
    assert_eq!(scene_scale_for(0.3, 1600, 900), 0.5);
    assert_eq!(scene_scale_for(1.4, 1600, 900), 1.0);
}

#[test]
fn surface_depth_coverage_excludes_glass_and_terrain_masks() {
    assert!(surface_depth_coverage(
        RenderPhase::Spline,
        AlphaMode::Blend,
        false,
        false
    ));
    for phase in RenderPhase::DRAW_ORDER {
        if !world_surface_phase(phase) {
            assert!(!surface_depth_coverage(
                phase,
                AlphaMode::Blend,
                false,
                false
            ));
        } else {
            assert!(surface_depth_coverage(
                phase,
                AlphaMode::Blend,
                false,
                false
            ));
        }
    }
    assert!(!surface_depth_coverage(
        RenderPhase::Spline,
        AlphaMode::Blend,
        true,
        false
    ));
    assert!(!surface_depth_coverage(
        RenderPhase::Spline,
        AlphaMode::Blend,
        false,
        true
    ));
    assert!(!surface_depth_coverage(
        RenderPhase::Spline,
        AlphaMode::Opaque,
        false,
        false
    ));
    assert!(!surface_depth_coverage(
        RenderPhase::Spline,
        AlphaMode::Test,
        false,
        false
    ));
    let order = RenderPhase::DRAW_ORDER;
    assert!(
        order
            .iter()
            .position(|p| *p == RenderPhase::OnSurface)
            .unwrap()
            < order
                .iter()
                .position(|p| *p == RenderPhase::BeforeNormal)
                .unwrap()
    );
}

#[test]
fn pipeline_codes_cover_the_table() {
    // every kind x culling x depth bias has its own pipeline, and the opaque and
    // alpha-tested codes sort before the blended ones
    let mut seen = std::collections::HashSet::new();
    for kind in 0..PIPE_KINDS {
        for cull in [false, true] {
            for surface in [false, true] {
                let code = pipe_code(kind, cull, surface);
                assert!((code as usize) < PIPE_KINDS as usize * 4);
                assert!(seen.insert(code));
                assert_eq!(
                    code < pipe_code(PIPE_BLEND, false, false),
                    kind < PIPE_BLEND
                );
            }
        }
    }
    assert_eq!(seen.len(), PIPE_KINDS as usize * 4);
    assert_eq!(pipe_code(PIPE_OPAQUE, false, false), 0);
    assert!(pipe_code(PIPE_ALPHA_TEST, true, true) < pipe_code(PIPE_BLEND_NO_WRITE, false, false));
}

#[test]
fn nearest_by_origin_picks_the_closest_mesh_of_each_object() {
    // A long bus (one origin) has a window 3 m from the camera and another 14 m away;
    // ranking the whole object by its shared origin (here 8 m) instead of its nearest
    // blended mesh is exactly what let a car beside the near window sort as farther
    // away than the bus and disappear behind it.
    let bus = DVec3::new(0.0, 0.0, 0.0);
    let car = DVec3::new(1.0, 0.0, 0.0);
    let by_origin = nearest_by_origin([(bus, 14.0), (bus, 3.0), (car, 8.0)]);
    let bus_dist = by_origin[&origin_key(bus)];
    let car_dist = by_origin[&origin_key(car)];
    assert_eq!(
        bus_dist, 3.0,
        "the object's distance is its nearest mesh, not the first or an average"
    );
    assert_eq!(car_dist, 8.0);
    // the car (8 m) is nearer than the bus's near window (3 m) is far: with the old
    // origin-only distance (bus origin 8.5 m, say) the two could tie or invert
    assert!(car_dist > bus_dist);
    assert_eq!(
        by_origin.len(),
        2,
        "one entry per distinct origin, not per mesh"
    );
}

#[test]
fn origin_lookup_keeps_float_equality_and_large_coordinates() {
    let a = DVec3::new(-0.0, 1_000_000.000_001, 2.0);
    let b = DVec3::new(0.0, a.y, 2.0);
    let c = DVec3::new(0.0, a.y + 0.000_001, 2.0);
    let distances = nearest_by_origin([(a, 8.0), (b, 3.0), (c, 5.0), (DVec3::NAN, 1.0)]);
    assert_eq!(distances.len(), 2);
    assert_eq!(distances[&origin_key(a)], 3.0);
    assert_eq!(distances[&origin_key(c)], 5.0);
}

/// Every shader module parses and validates as the device will see it, translates to
/// Metal and Vulkan SPIR-V, and its uniform structs are laid out as the Rust side writes them.
#[test]
fn shaders_validate_and_match_the_uniforms() {
    use wgpu::naga;
    let modules = [
        ("scene", scene_shader_source(false)),
        ("scene opaque", scene_shader_source(false)),
        ("scene terrain paint", scene_shader_source(false)),
        ("sky", sky_shader_source()),
        ("corona", corona_shader_source()),
        ("fog lamps", fog_lamps_shader_source()),
        ("snow", snow_shader_source()),
        ("post", post_shader_source()),
        (
            "ssao",
            with_fullscreen(include_str!("shaders/post/ssao.wgsl")),
        ),
        ("puddles", puddles::shader_source()),
        (
            "upscale",
            with_fullscreen(include_str!("shaders/output/upscale.wgsl")),
        ),
        (
            "mip",
            with_fullscreen(include_str!("shaders/assets/mip.wgsl")),
        ),
        (
            "xr_ui",
            include_str!("shaders/output/xr_ui.wgsl").to_string(),
        ),
        // Enhanced+: the scene's pass with its reflection targets, the ray tracing
        (
            "scene (Enhanced+)",
            scene_shader_source(false).replace("//RT ", ""),
        ),
        ("ray-traced lighting", rt::lighting_source()),
        ("ray-traced reflections", rt::reflect_source()),
        (
            "texture means",
            include_str!("shaders/effects/rt_avg.wgsl").to_string(),
        ),
    ];
    let sizes: &[(&str, usize)] = &[
        ("Enhanced", std::mem::size_of::<EnhancedUniform>()),
        ("PostParams", std::mem::size_of::<PostUniform>()),
        ("FogLampParams", std::mem::size_of::<FogLampUniform>()),
        ("SnowParams", std::mem::size_of::<SnowUniform>()),
        ("PuddleParams", std::mem::size_of::<puddles::Uniform>()),
        (
            "VehicleReflection",
            std::mem::size_of::<puddles::VehicleUniform>(),
        ),
        ("PointLight", std::mem::size_of::<GpuPointLight>()),
        ("Camera", std::mem::size_of::<CameraUniform>()),
        ("MaterialParams", std::mem::size_of::<MaterialUniform>()),
        ("RtParams", rt::PARAMS_SIZE),
    ];
    let mut checked = std::collections::HashSet::new();
    for (name, src) in &modules {
        let module = naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        // Exercise the painted-ground specialization as well as the default shader.
        let mut constants = naga::back::PipelineConstants::default();
        if name.starts_with("scene ") {
            constants.insert("ALPHA_TEST".into(), 0.0);
        }
        if *name == "scene terrain paint" {
            constants.insert("TERRAIN_PAINT".into(), 1.0);
        }
        let (module, info) =
            naga::back::pipeline_constants::process_overrides(&module, &info, None, &constants)
                .unwrap_or_else(|e| panic!("{name}: overrides: {e:?}"));
        let (module, info) = (module.into_owned(), info.into_owned());
        #[cfg(target_os = "macos")]
        let options = naga::back::msl::Options {
            lang_version: (2, 4),
            ..Default::default()
        };
        #[cfg(target_os = "macos")]
        naga::back::msl::write_string(
            &module,
            &info,
            &options,
            &naga::back::msl::PipelineOptions::default(),
        )
        .unwrap_or_else(|e| panic!("{name}: Metal: {e:?}"));
        for entry in &module.entry_points {
            let pipeline = naga::back::spv::PipelineOptions {
                shader_stage: entry.stage,
                entry_point: entry.name.clone(),
            };
            naga::back::spv::write_vec(&module, &info, &Default::default(), Some(&pipeline))
                .unwrap_or_else(|e| panic!("{name}/{}: Vulkan: {e:?}", entry.name));
        }
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).expect("layout");
        for (ty_name, rust) in sizes {
            if let Some((h, _)) = module
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some(*ty_name))
            {
                assert_eq!(layouter[h].size as usize, *rust, "{name}: {ty_name}");
                checked.insert(*ty_name);
            }
        }
    }
    assert_eq!(checked.len(), sizes.len(), "structs checked: {checked:?}");
}

/// `common/camera.wgsl` lays the camera out field for field as `CameraUniform`, and its
/// light grid holds as many lights per cell as the Rust side fills.
#[test]
fn camera_wgsl_matches_camera_uniform() {
    use wgpu::naga;
    macro_rules! offsets {
        ($($field:ident),* $(,)?) => {
            [$((stringify!($field), std::mem::offset_of!(CameraUniform, $field))),*]
        };
    }
    let rust = offsets!(
        view_proj,
        cam_pos,
        world_origin,
        sun_dir,
        ambient,
        fog,
        sun_color,
        sky_color,
        light_grid,
        sky,
        cam_right,
        cam_up,
        clouds,
        light_view_proj,
        light_view_proj_far,
        shadow,
        post,
        inside_a,
        inside_b,
        inside_c,
        flags,
        light_view_proj_close,
        wind,
        lamp_view_proj,
        lamp_shadow,
        tree_wind,
    );
    let src = include_str!("shaders/common/camera.wgsl");
    let module =
        naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
    let (_, camera) = module
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("Camera"))
        .expect("Camera");
    let naga::TypeInner::Struct { members, span } = &camera.inner else {
        panic!("Camera is not a struct");
    };
    let wgsl: Vec<(&str, usize)> = members
        .iter()
        .map(|m| (m.name.as_deref().unwrap_or(""), m.offset as usize))
        .collect();
    assert_eq!(wgsl, rust.to_vec());
    assert_eq!(*span as usize, std::mem::size_of::<CameraUniform>());
    let cell_cap = module
        .constants
        .iter()
        .find(|(_, c)| c.name.as_deref() == Some("CELL_CAP"))
        .map(|(_, c)| &module.global_expressions[c.init]);
    assert!(
        matches!(
            cell_cap,
            Some(naga::Expression::Literal(naga::Literal::U32(n))) if *n as usize == LIGHT_CELL_CAP
        ),
        "CELL_CAP: {cell_cap:?}"
    );
}

/// The scene module as the OpenGL backend gets it translates to GLSL ES 3.10 and desktop
/// GLSL 4.30 for every entry point, with no `invariant gl_FragCoord` (rejected by AMD's
/// desktop GL and by GLES) and no texture read through two samplers (#617, #610).
#[test]
fn the_scene_shader_translates_to_glsl() {
    use wgpu::naga;
    use wgpu::naga::back::glsl;
    let src = scene_shader_source(true);
    let module =
        naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("validate");
    let constants = [("ALPHA_TEST".into(), 0.0), ("TERRAIN_PAINT".into(), 1.0)]
        .into_iter()
        .collect();
    let (module, info) =
        naga::back::pipeline_constants::process_overrides(&module, &info, None, &constants)
            .expect("overrides");
    for version in [
        glsl::Version::Embedded {
            version: 310,
            is_webgl: false,
        },
        glsl::Version::Desktop(430),
    ] {
        let options = glsl::Options {
            version,
            ..Default::default()
        };
        for entry in &module.entry_points {
            let pipeline = glsl::PipelineOptions {
                shader_stage: entry.stage,
                entry_point: entry.name.clone(),
                multiview: None,
            };
            let mut out = String::new();
            glsl::Writer::new(
                &mut out,
                &module,
                &info,
                &options,
                &pipeline,
                Default::default(),
            )
            .and_then(|mut w| w.write())
            .unwrap_or_else(|e| panic!("{version:?} {}: {e:?}", entry.name));
            assert!(
                !out.contains("invariant gl_FragCoord"),
                "{version:?} {}",
                entry.name
            );
        }
    }
}

/// The corona and smoke module (its smoke branch with the fade into the ground among
/// them) translates to the GLSL of the oldest OpenGL chips the renderer runs on, GLES 3.0
/// and GL 3.3, as to GLES 3.1 and GL 4.3.
#[test]
fn the_corona_shader_translates_to_glsl() {
    use wgpu::naga;
    use wgpu::naga::back::glsl;
    let src = corona_shader_source();
    let module =
        naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("validate");
    let (module, info) = naga::back::pipeline_constants::process_overrides(
        &module,
        &info,
        None,
        &Default::default(),
    )
    .expect("overrides");
    let versions = [
        glsl::Version::Embedded {
            version: 300,
            is_webgl: false,
        },
        glsl::Version::Desktop(330),
        glsl::Version::Embedded {
            version: 310,
            is_webgl: false,
        },
        glsl::Version::Desktop(430),
    ];
    for version in versions {
        let options = glsl::Options {
            version,
            ..Default::default()
        };
        let mut done = 0;
        for entry in module.entry_points.iter().filter(|e| {
            ["vs_main", "fs_main", "fs_smoke", "fs_smoke_enhanced"].contains(&e.name.as_str())
        }) {
            let pipeline = glsl::PipelineOptions {
                shader_stage: entry.stage,
                entry_point: entry.name.clone(),
                multiview: None,
            };
            let mut out = String::new();
            glsl::Writer::new(
                &mut out,
                &module,
                &info,
                &options,
                &pipeline,
                Default::default(),
            )
            .and_then(|mut w| w.write())
            .unwrap_or_else(|e| panic!("{version:?} {}: {e:?}", entry.name));
            done += 1;
        }
        assert_eq!(done, 4, "{version:?}");
    }
}

/// Where the device cannot read storage buffers in a vertex shader, or none at all, the
/// scene module reads its arrays from textures: it validates and translates to the GLSL
/// of such chips (GLES 3.1 / GL 4.3 with storage for the lights, GLES 3.0 / GL 3.3
/// without), and nothing of it is a storage buffer where it must not be (#770, #316).
#[test]
fn the_scene_shader_reads_its_arrays_from_textures_without_vertex_storage() {
    use wgpu::naga;
    use wgpu::naga::back::glsl;
    for (path, versions) in [
        (
            ArrayPath::VertexTextures,
            [
                glsl::Version::Embedded {
                    version: 310,
                    is_webgl: false,
                },
                glsl::Version::Desktop(430),
            ],
        ),
        (
            ArrayPath::NoStorage,
            [
                glsl::Version::Embedded {
                    version: 300,
                    is_webgl: false,
                },
                glsl::Version::Desktop(330),
            ],
        ),
    ] {
        let src = arrays_as_textures(&scene_shader_text(true), path);
        assert!(
            !src.contains("models[")
                && !src.contains("inst_params[")
                && !src.contains("draw_list[")
        );
        assert_eq!(
            src.contains("var<storage"),
            path == ArrayPath::VertexTextures,
            "{path:?}"
        );
        let module = naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{path:?}: {}", e.emit_to_string(&src)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{path:?}: {e:?}"));
        let (module, info) = naga::back::pipeline_constants::process_overrides(
            &module,
            &info,
            None,
            &Default::default(),
        )
        .expect("overrides");
        for version in versions {
            let options = glsl::Options {
                version,
                ..Default::default()
            };
            for entry in &module.entry_points {
                let pipeline = glsl::PipelineOptions {
                    shader_stage: entry.stage,
                    entry_point: entry.name.clone(),
                    multiview: None,
                };
                let mut out = String::new();
                glsl::Writer::new(
                    &mut out,
                    &module,
                    &info,
                    &options,
                    &pipeline,
                    Default::default(),
                )
                .and_then(|mut w| w.write())
                .unwrap_or_else(|e| panic!("{path:?} {version:?} {}: {e:?}", entry.name));
                let vertex = entry.stage == naga::ShaderStage::Vertex;
                if vertex || path == ArrayPath::NoStorage {
                    assert!(
                        !out.contains(" buffer "),
                        "{path:?} {version:?} {}: a storage block",
                        entry.name
                    );
                }
            }
        }
    }
    // the rewrite leaves names that only end alike alone, and nests
    assert_eq!(
        indexing_as_calls(
            "a = my_models[1]; b = models[models[i + 1u] .x];",
            "models",
            "f"
        ),
        "a = my_models[1]; b = f(f(i + 1u) .x);"
    );
}

/// wgpu's OpenGL backend numbers the textures of all a pipeline's groups into sixteen
/// units: the scene's camera and material groups stay within them on every array path
/// there, and what is left out of the camera group for it no pipeline made there reads
/// (#1133, #1154, #1162, #1176).
#[test]
fn the_scene_groups_fit_the_sixteen_texture_units_of_opengl() {
    use wgpu::naga;
    let textures = |e: &[wgpu::BindGroupLayoutEntry]| {
        e.iter()
            .filter(|e| matches!(e.ty, wgpu::BindingType::Texture { .. }))
            .count()
    };
    let material = textures(&material_layout_entries());
    for path in [
        ArrayPath::Storage,
        ArrayPath::VertexTextures,
        ArrayPath::NoStorage,
    ] {
        let sixteen = path != ArrayPath::Storage;
        let n = textures(&camera_layout_entries(path, sixteen)) + material;
        assert!(n <= 16, "{path:?}: {n} textures");
    }
    // (and nothing is left out where storage buffers carry the arrays)
    assert_eq!(
        textures(&camera_layout_entries(ArrayPath::Storage, false)) + material,
        16
    );
    // the entry points that read a texture left out are the enhanced path's, the
    // probe's, the puddles' and the reflection pass's - none made there
    let left_out = |src: &str| -> Vec<String> {
        let module = naga::front::wgsl::parse_str(src).unwrap();
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        let mut out = Vec::new();
        for (i, ep) in module.entry_points.iter().enumerate() {
            let used = info.get_entry_point(i);
            let reads = module.global_variables.iter().any(|(h, g)| {
                !used[h].is_empty()
                    && g.binding.as_ref().is_some_and(|b| {
                        b.group == 0 && ENHANCED_CAMERA_TEXTURES.contains(&b.binding)
                    })
            });
            if reads {
                out.push(ep.name.clone());
            }
        }
        out
    };
    let only_enhanced = |name: &str| {
        name.contains("enhanced")
            || name.contains("probe")
            || name.contains("puddle")
            || name.contains("reflections")
            || name == "fs_sky_cube"
    };
    for path in [ArrayPath::VertexTextures, ArrayPath::NoStorage] {
        let scene = arrays_as_textures(&scene_shader_text(true), path);
        for src in [scene, sky_shader_source(), corona_shader_source()] {
            for name in left_out(&src) {
                assert!(
                    only_enhanced(&name),
                    "{path:?}: {name} reads an enhanced texture"
                );
            }
        }
    }
}

/// An array texture is written in the rest of a row, whole rows and the start of the
/// last row, and never past its end.
#[test]
fn array_texture_writes_cover_the_range_once() {
    let w = ARRAY_TEX_WIDTH;
    assert_eq!(array_tex_spans(0, 10, 4 * w as u64), vec![(0, 0, 10, 1)]);
    assert_eq!(
        array_tex_spans(w as u64 - 2, 5, 4 * w as u64),
        vec![(w - 2, 0, 2, 1), (0, 1, 3, 1)]
    );
    assert_eq!(
        array_tex_spans(5, 3 * w as u64, 4 * w as u64),
        vec![(5, 0, w - 5, 1), (0, 1, w, 2), (0, 3, 5, 1)]
    );
    assert_eq!(
        array_tex_spans(0, 2 * w as u64, 4 * w as u64),
        vec![(0, 0, w, 2)]
    );
    // past the end: cut
    assert_eq!(
        array_tex_spans(2 * w as u64 - 1, 10, 2 * w as u64),
        vec![(w - 1, 1, 1, 1)]
    );
    assert!(array_tex_spans(3 * w as u64, 10, 2 * w as u64).is_empty());
    for (at, n) in [(0u64, 1u64), (7, 9000), (2047, 2049), (4096, 4096)] {
        let covered: u64 = array_tex_spans(at, n, 64 * w as u64)
            .iter()
            .map(|s| s.2 as u64 * s.3 as u64)
            .sum();
        assert_eq!(covered, n);
    }
}

#[test]
fn the_sky_is_recomputed_only_when_it_has_moved_on() {
    let a = atmosphere::SkyInput::default();
    assert!(!sky_input_differs(&a, &a));
    // the sun a hundredth of a degree on, a tint that drifts by a thousandth: the same sky
    let turn = |deg: f32| {
        glam::Quat::from_axis_angle(a.sun_dir.any_orthonormal_vector(), deg.to_radians())
            * a.sun_dir
    };
    assert!(!sky_input_differs(
        &a,
        &atmosphere::SkyInput {
            sun_dir: turn(0.01),
            tint: [Vec3::splat(1.001); 3],
            ..a
        }
    ));
    // a quarter of a degree, some rain, another envir.cfg: a new sky
    assert!(sky_input_differs(
        &a,
        &atmosphere::SkyInput {
            sun_dir: turn(0.25),
            ..a
        }
    ));
    assert!(sky_input_differs(
        &a,
        &atmosphere::SkyInput { rain: 0.3, ..a }
    ));
    assert!(sky_input_differs(
        &a,
        &atmosphere::SkyInput {
            tint: [Vec3::new(1.2, 1.0, 0.9), Vec3::ONE, Vec3::ONE],
            ..a
        }
    ));
}

#[test]
fn each_path_gets_its_own_headlights() {
    let lamp = PointLight {
        radius: 30.0,
        color: [1.0, 0.78, 0.46],
        core: 5.0,
        ..Default::default()
    };
    let stand_in = PointLight {
        radius: 18.0,
        intensity: 0.8,
        mode: LightMode::Vanilla,
        ..Default::default()
    };
    let spot = PointLight {
        radius: 60.0,
        intensity: 20.0,
        direction: Vec3::new(0.0, 2.0, -0.6),
        cone: [0.97, 0.82],
        core: 1.0,
        beam: 24.0,
        mode: LightMode::Enhanced,
        ..Default::default()
    };
    assert!(drawn_by(&lamp, false) && drawn_by(&lamp, true));
    assert!(drawn_by(&stand_in, false) && !drawn_by(&stand_in, true));
    assert!(!drawn_by(&spot, false) && drawn_by(&spot, true));
    assert!(!drawn_by(
        &PointLight {
            intensity: 0.0,
            ..lamp
        },
        true
    ));
    // a map light reads as before in the vanilla shader (position, radius, colour)
    let g = gpu_light(&lamp, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(g.pos, [1.0, 2.0, 3.0, 30.0]);
    assert_eq!(g.color, [1.0, 0.78, 0.46, 1.0]);
    assert_eq!(g.dir[3], -2.0);
    assert_eq!(g.extra[1..], [5.0, 0.0, 30.0]);
    // a spot: no radius for the vanilla shader, the cone and the range for the enhanced
    let g = gpu_light(&spot, Vec3::ZERO);
    assert_eq!(g.pos[3], 0.0);
    assert_eq!(g.extra[3], 60.0);
    assert!(
        (Vec3::from_slice(&g.dir[..3]).length() - 1.0).abs() < 1e-5
            && g.dir[3] == 0.82
            && g.extra[0] == 0.97
    );
    // a headlight's beam gain rides along; a plain lamp has none
    assert_eq!(g.extra[2], 24.0);
    assert_eq!(gpu_light(&lamp, Vec3::ZERO).extra[2], 0.0);
}

#[test]
fn metering_defaults_are_gentle() {
    // the light model's exposure leads: the metering corrects well under a stop either
    // way, so snow stays white and a night stays dark
    let m = meter_tuning();
    assert!(m[0] > 0.0 && m[0] < 0.6, "{m:?}");
    assert!(m[2] <= 2.5 && m[3] <= 2.0, "{m:?}");
    // a snow field metered 1.5 EV over the target is darkened by well under a stop
    let ev = ((m[1] - (m[1] + 1.5)) * m[0]).clamp(-m[2], m[3]);
    assert!(ev < 0.0 && ev >= -0.75, "{ev}");
}

#[test]
fn half_floats_read_back() {
    for v in [0.0f32, 1.0, -2.5, 0.5, 1e-3, -3.46] {
        assert!(
            (half_to_f32(atmosphere::f16_bits(v)) - v).abs() <= v.abs() * 1e-3 + 1e-6,
            "{v}"
        );
    }
}

#[test]
fn vehicle_box_contains_the_driver() {
    // an NL202: 2.5 x 12 x 3 m, the box centre 0.4 m ahead of the origin and 1.5 m up,
    // heading east; the driver sits 4.6 m ahead of the origin, 1.8 m up
    let bus = (
        DVec3::new(100.0, 200.0, 30.0),
        90.0,
        [2.5, 12.0, 3.0, 0.0, 0.4, 1.5],
    );
    assert!(point_in_vehicle_box(DVec3::new(104.6, 200.7, 31.8), &bus));
    // beside the bus, above it, behind it
    assert!(!point_in_vehicle_box(DVec3::new(104.6, 197.0, 31.8), &bus));
    assert!(!point_in_vehicle_box(DVec3::new(104.6, 200.0, 34.0), &bus));
    assert!(!point_in_vehicle_box(DVec3::new(93.0, 200.0, 31.0), &bus));
}

#[test]
fn opaque_and_transmapped_materials_ignore_dynamic_alpha() {
    assert_eq!(
        Renderer::clamp_slot_alpha(0.0, AlphaMode::Opaque, false),
        1.0
    );
    assert_eq!(
        Renderer::clamp_slot_alpha(0.35, AlphaMode::Opaque, false),
        1.0
    );
    assert_eq!(Renderer::clamp_slot_alpha(0.0, AlphaMode::Test, false), 1.0);
    assert_eq!(
        Renderer::clamp_slot_alpha(0.35, AlphaMode::Test, false),
        1.0
    );
    assert_eq!(
        Renderer::clamp_slot_alpha(0.85, AlphaMode::Blend, false),
        0.85
    );
    assert_eq!(Renderer::clamp_slot_alpha(0.0, AlphaMode::Blend, true), 1.0);
    assert_eq!(
        Renderer::clamp_slot_alpha(0.0, AlphaMode::Opaque, true),
        1.0
    );
    assert_eq!(Renderer::clamp_slot_alpha(0.0, AlphaMode::Test, true), 1.0);
}

/// The mirror tests change the process's count of mirroring renderers; they run one at a time.
static MIRROR_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

fn triangle() -> MeshData {
    MeshData {
        positions: vec![
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        normals: vec![-Vec3::Y; 3],
        uvs: vec![glam::Vec2::ZERO; 3],
        indices: vec![0, 1, 2],
        ranges: vec![(0, 3, 0)],
        ..Default::default()
    }
}

/// The renderer goes to other threads (the launcher drops it on one), mirror or not.
#[test]
fn a_renderer_with_a_mirror_is_still_send() {
    fn send<T: Send>() {}
    send::<Renderer>();
    send::<AssetMirror>();
    send::<PlanSink>();
}

/// Without a mirror nothing keeps CPU copies, and turning a mirror off (or dropping its
/// renderer) stops them again.
#[test]
fn without_a_mirror_nothing_is_kept() {
    let _serial = MIRROR_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut renderer = noop_renderer();
    let prepared = crate::prepare_mesh(&renderer.device, &renderer.queue, &triangle());
    assert!(prepared.1.is_none(), "no mirror: no CPU copy");
    assert!(!renderer.mirroring());

    renderer.set_asset_mirror(Some(Box::new(|_| {})));
    assert!(crate::assets::keep_cpu_copies());
    // a second renderer turning its own (absent) mirror off changes nothing for the first
    let mut other = noop_renderer();
    other.set_asset_mirror(None);
    assert!(crate::assets::keep_cpu_copies());
    let kept = crate::prepare_mesh(&renderer.device, &renderer.queue, &triangle());
    assert!(
        kept.1.is_some(),
        "a mirror: the CPU copy waits for the mirror"
    );

    renderer.set_asset_mirror(None);
    assert!(!crate::assets::keep_cpu_copies());
    renderer.set_asset_mirror(Some(Box::new(|_| {})));
    drop(renderer);
    assert!(
        !crate::assets::keep_cpu_copies(),
        "a dropped renderer stops its mirror"
    );
}

/// The asset mirror receives meshes and textures as they go up, and the plan sink receives
/// the window picture's batches with per-draw data the plan's list points at.
#[test]
fn mirror_and_plan_describe_what_is_drawn() {
    use std::sync::{Arc, Mutex};
    let _serial = MIRROR_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut renderer = noop_renderer();

    let events: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = events.clone();
    renderer.set_asset_mirror(Some(Box::new(move |e: MirrorEvent<'_>| {
        seen.lock().unwrap().push(match e {
            MirrorEvent::Mesh {
                id,
                vertices,
                indices,
            } => {
                format!("mesh {id} {} {}", vertices.len(), indices.len())
            }
            MirrorEvent::Texture { id, .. } => format!("texture {id}"),
            MirrorEvent::FreeMesh(id) => format!("free mesh {id}"),
            MirrorEvent::FreeTexture(id) => format!("free texture {id}"),
            MirrorEvent::MeshVertices { id, .. } => format!("pose {id}"),
            MirrorEvent::MoveMesh { from, into } => format!("move mesh {from} {into}"),
            MirrorEvent::MoveTexture { from, into } => format!("move texture {from} {into}"),
            MirrorEvent::LightMapTile { slot, .. } => format!("light map {slot:?}"),
            MirrorEvent::CoronaTexture { id, .. } => format!("corona picture {id}"),
        });
    })));
    let plans: Arc<Mutex<Vec<(FramePlan, usize)>>> = Arc::default();
    let got = plans.clone();
    renderer.set_plan_sink(Some(Box::new(move |plan: &FramePlan, scene: &Scene| {
        got.lock()
            .unwrap()
            .push((plan.clone(), scene.slot_models().len()));
    })));

    let mut scene = renderer.new_scene();
    let data = triangle();
    let mesh = renderer.add_mesh(&mut scene, &data);
    let image = omsi_texture::Image {
        width: 2,
        height: 2,
        rgba: vec![255; 16],
        has_alpha: false,
    };
    let tex = renderer.add_texture(&mut scene, &image, false);
    let mat = renderer.add_material(&mut scene, Some(tex), AlphaMode::Opaque, [1.0; 4], true);
    renderer.add_instance(
        &mut scene,
        mesh,
        DVec3::new(0.0, 10.0, 0.0),
        Mat4::IDENTITY,
        vec![mat],
    );
    {
        let e = events.lock().unwrap();
        assert!(e.contains(&format!("mesh {mesh} 3 3")), "{e:?}");
        assert!(e.contains(&format!("texture {tex}")), "{e:?}");
    }

    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let camera = Camera {
        position: DVec3::new(0.0, 0.0, 0.5),
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 60.0,
        near: 0.1,
        far: 1000.0,
    };
    renderer.render(&mut scene, &view, 64, 64, &camera, &Lighting::default());

    let plans = plans.lock().unwrap();
    assert_eq!(plans.len(), 1, "one window picture, one plan");
    let (plan, slots) = &plans[0];
    assert_eq!((plan.width, plan.height), (64, 64));
    let sky = Lighting::default().sky_color;
    assert_eq!(plan.clear, [sky.x, sky.y, sky.z, 1.0]);
    let b = plan
        .main
        .iter()
        .find(|b| b.mesh == mesh)
        .expect("the triangle is in the main pass");
    assert_eq!(
        (b.kind, b.material, b.first, b.count),
        (PlanKind::Opaque, mat, 0, 3)
    );
    for i in b.instances.clone() {
        assert!(
            (plan.list[i as usize] as usize) < *slots,
            "list entries index the slots"
        );
    }
}

/// A second renderer gets the lights' own pictures as they go up, and the frame's coronas
/// with the runs by picture (kept only while a mirror is set).
#[test]
fn mirror_carries_the_coronas() {
    use std::sync::{Arc, Mutex};
    let _serial = MIRROR_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut renderer = noop_renderer();

    let pictures: Arc<Mutex<Vec<(u16, u32)>>> = Arc::default();
    let seen = pictures.clone();
    renderer.set_asset_mirror(Some(Box::new(move |e: MirrorEvent<'_>| {
        if let MirrorEvent::CoronaTexture { id, image } = e {
            seen.lock().unwrap().push((id, image.width));
        }
    })));
    let frames: Arc<Mutex<Vec<(usize, Vec<(u16, u32, u32, bool)>)>>> = Arc::default();
    let got = frames.clone();
    renderer.set_plan_sink(Some(Box::new(move |_: &FramePlan, scene: &Scene| {
        got.lock().unwrap().push((
            scene.frame_coronas().len(),
            scene.frame_corona_runs().to_vec(),
        ));
    })));

    renderer.set_corona_texture(
        7,
        &omsi_texture::Image {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
            has_alpha: false,
        },
    );
    assert_eq!(*pictures.lock().unwrap(), vec![(7, 2)]);

    let mut scene = renderer.new_scene();
    for (texture, brightness) in [(7, 1.0), (0, 1.0), (7, 0.5), (0, 0.0)] {
        scene.coronas.push(crate::Corona {
            position: DVec3::new(0.0, 10.0, 0.0),
            brightness,
            texture,
            ..Default::default()
        });
    }
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let camera = Camera {
        position: DVec3::new(0.0, 0.0, 0.5),
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 60.0,
        near: 0.1,
        far: 1000.0,
    };
    renderer.render(&mut scene, &view, 64, 64, &camera, &Lighting::default());

    let frames = frames.lock().unwrap();
    assert_eq!(frames.len(), 1);
    // (the one that is not lit is left out; the rest are sorted by picture)
    assert_eq!(frames[0].0, 3);
    assert_eq!(frames[0].1, vec![(0, 0, 1, false), (7, 1, 2, false)]);
}
