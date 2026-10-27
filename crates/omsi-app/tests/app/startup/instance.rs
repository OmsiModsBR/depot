use openomsi_game::testing::startup::{instance_descriptor, RESOURCE_MEMORY_PERCENT};

/// Resources past the card's budget are refused as an "out of memory" the game gets
/// over, before the driver runs out (#48); losing the device stays the driver's call.
#[test]
fn every_instance_refuses_resources_past_the_memory_budget() {
    for backends in [
        None,
        Some(wgpu::Backends::VULKAN),
        Some(wgpu::Backends::DX12),
    ] {
        let d = instance_descriptor(backends);
        assert_eq!(
            d.memory_budget_thresholds.for_resource_creation,
            Some(RESOURCE_MEMORY_PERCENT)
        );
        assert_eq!(d.memory_budget_thresholds.for_device_loss, None);
    }
}

#[test]
fn the_backends_asked_for_are_kept() {
    assert_eq!(
        instance_descriptor(Some(wgpu::Backends::GL)).backends,
        wgpu::Backends::GL
    );
    assert_eq!(
        instance_descriptor(None).backends,
        wgpu::InstanceDescriptor::new_without_display_handle().backends
    );
}
