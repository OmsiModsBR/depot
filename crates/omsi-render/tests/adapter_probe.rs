//! Diagnostic for issue #324: where the renderer stops on a real adapter. Each step prints a
//! marker before it runs, so a crash (stack overflow aborts the process) shows the last stage
//! reached. `OMSI_PROBE_BACKEND=dx12|vulkan|gl` picks the interface; run with `--ignored
//! --nocapture` on a host with a graphics adapter.

fn backends() -> wgpu::Backends {
    match std::env::var("OMSI_PROBE_BACKEND")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "dx12" => wgpu::Backends::DX12,
        "vulkan" => wgpu::Backends::VULKAN,
        "gl" => wgpu::Backends::GL,
        _ => wgpu::Backends::all(),
    }
}

#[test]
#[ignore = "diagnostic: requires a graphics adapter; see issue #324"]
fn renderer_starts_on_the_selected_backend() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = backends();
    eprintln!("probe: backends {:?}", descriptor.backends);
    let instance = wgpu::Instance::new(descriptor);
    eprintln!("probe: instance created");
    for a in pollster::block_on(instance.enumerate_adapters(backends())) {
        let i = a.get_info();
        eprintln!(
            "probe: adapter {:?} {:?} {:?} driver {:?} {:?}",
            i.name, i.backend, i.device_type, i.driver, i.driver_info
        );
    }
    eprintln!("probe: requesting adapter and device (Renderer::new_with)");
    let renderer = pollster::block_on(omsi_render::Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        omsi_render::RenderOptions {
            msaa: 1,
            ssao: false,
            shadow_size: 1024,
            fxaa: false,
            render_scale: 1.0,
            ..Default::default()
        },
    ));
    eprintln!("probe: renderer created: {}", renderer.is_ok());
    renderer.expect("renderer");
}
