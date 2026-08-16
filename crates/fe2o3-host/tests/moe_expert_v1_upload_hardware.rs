use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::{
    GeneratedMoeExpertV1HostAdapterV1, ObservedContext, deny_moe_expert_execution_v1,
    upload_moe_expert_offsets_v1,
};

#[test]
#[ignore = "requires an available gfx942:xnack- HIP device; no kernel is dispatched"]
fn gfx942_offset_upload_readback_and_denial_are_exact() -> Result<(), Box<dyn std::error::Error>> {
    let context = GpuContext::new(0)?;
    let stream = context.default_stream();
    let observed = ObservedContext::observe(&context)?;

    let activation_tiles = DeviceBuffer::from_host(&stream, &[0_u16; 1024])?;
    let expert_weights = DeviceBuffer::from_host(&stream, &[0_u16; 1024])?;
    let mut expert_offsets = DeviceBuffer::zeroed(&stream, 5)?;
    let inverse_routing = DeviceBuffer::from_host(&stream, &(0_u32..16).collect::<Vec<_>>())?;
    let route_weights = DeviceBuffer::from_host(&stream, &[0.5_f32; 16])?;
    let mut expert_output = DeviceBuffer::zeroed(&stream, 1024)?;
    let mut compact_output = DeviceBuffer::zeroed(&stream, 256)?;
    let mut combined_output = DeviceBuffer::zeroed(&stream, 128)?;

    // These arrays describe a coherent full-capacity identity permutation, but
    // the host slice deliberately does not inspect or prove that relationship.
    let offsets = [0, 4, 8, 12, 16];
    let offsets_upload = upload_moe_expert_offsets_v1(&stream, &mut expert_offsets, offsets)?;
    assert!(offsets_upload.upload_completed());
    assert_eq!(offsets_upload.expert_offsets(), offsets);
    assert_eq!(offsets_upload.region_byte_range(), 0..20);
    assert_eq!(offsets_upload.context_identity(), context.identity());
    assert_eq!(offsets_upload.stream_identity(), stream.identity());
    assert!(!offsets_upload.grants_copy_authority());

    let binding = GeneratedMoeExpertV1HostAdapterV1::prepare(
        &observed,
        activation_tiles.view(..)?,
        expert_weights.view(..)?,
        offsets_upload,
        inverse_routing.view(..)?,
        route_weights.view(..)?,
        expert_output.view_mut(..)?,
        compact_output.view_mut(..)?,
        combined_output.view_mut(..)?,
    )?;
    assert_eq!(binding.compact_pack_plan().accepted_routes(), 16);
    assert_eq!(binding.compact_pack_plan().defined_tail_elements(), 0);
    assert!(!binding.compact_pack_plan().grants_copy_authority());
    assert!(!binding.has_routing_consistency_witness());
    assert!(!binding.grants_artifact_authority());
    assert!(!binding.grants_launch_authority());

    let denied = deny_moe_expert_execution_v1(binding);
    assert!(denied.reason().contains("authority"));
    assert!(!denied.grants_artifact_authority());
    assert!(!denied.grants_copy_authority());
    assert!(!denied.grants_load_authority());
    assert!(!denied.grants_dispatch_authority());
    drop(denied);

    assert_eq!(expert_offsets.to_host_vec(&stream)?, offsets);
    Ok(())
}
