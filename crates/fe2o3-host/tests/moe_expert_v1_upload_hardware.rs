use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::{
    GeneratedMoeExpertV1HostAdapterV1, MoeRoutingOutputCandidateV1, ObservedContext,
    check_host_observed_moe_routing_output_v1, deny_moe_expert_execution_v1,
    upload_checked_moe_routing_expert_bridge_v1,
};

#[test]
#[ignore = "requires an available gfx942:xnack- HIP device; no kernel is dispatched"]
fn gfx942_routing_bridge_upload_readback_and_denial_are_exact()
-> Result<(), Box<dyn std::error::Error>> {
    let context = GpuContext::new(0)?;
    let stream = context.default_stream();
    let observed = ObservedContext::observe(&context)?;

    let activation_tiles = DeviceBuffer::from_host(&stream, &[0_u16; 1024])?;
    let expert_weights = DeviceBuffer::from_host(&stream, &[0_u16; 1024])?;
    let mut expert_offsets = DeviceBuffer::zeroed(&stream, 5)?;
    let mut inverse_routing = DeviceBuffer::zeroed(&stream, 16)?;
    let route_weights = DeviceBuffer::from_host(&stream, &[0.5_f32; 16])?;
    let mut expert_output = DeviceBuffer::zeroed(&stream, 1024)?;
    let mut compact_output = DeviceBuffer::zeroed(&stream, 256)?;
    let mut combined_output = DeviceBuffer::zeroed(&stream, 128)?;

    let top2 = [0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3];
    let route_slots = [0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15];
    let permutation = [0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15];
    let offsets = [0, 4, 8, 12, 16];
    let checked = check_host_observed_moe_routing_output_v1(MoeRoutingOutputCandidateV1::new(
        top2,
        [4; 4],
        [4; 4],
        offsets,
        route_slots,
        permutation,
        route_slots,
    ))?;
    let routing_bridge = upload_checked_moe_routing_expert_bridge_v1(
        &stream,
        &mut expert_offsets,
        &mut inverse_routing,
        checked,
    )?;
    assert!(routing_bridge.upload_completed());
    assert_eq!(routing_bridge.expert_offsets(), offsets);
    assert_eq!(routing_bridge.inverse(), route_slots);
    assert_eq!(routing_bridge.offsets_region_byte_range(), 0..20);
    assert_eq!(routing_bridge.inverse_region_byte_range(), 0..64);
    assert_eq!(routing_bridge.context_identity(), context.identity());
    assert_eq!(routing_bridge.stream_identity(), stream.identity());
    assert_ne!(
        routing_bridge.offsets_allocation_identity(),
        routing_bridge.inverse_allocation_identity()
    );
    assert!(!routing_bridge.producer_is_authenticated());
    assert!(!routing_bridge.grants_copy_authority());
    assert!(!routing_bridge.grants_load_authority());
    assert!(!routing_bridge.grants_dispatch_authority());

    let binding = GeneratedMoeExpertV1HostAdapterV1::prepare(
        &observed,
        activation_tiles.view(..)?,
        expert_weights.view(..)?,
        routing_bridge,
        route_weights.view(..)?,
        expert_output.view_mut(..)?,
        compact_output.view_mut(..)?,
        combined_output.view_mut(..)?,
    )?;
    assert_eq!(binding.compact_pack_plan().accepted_routes(), 16);
    assert_eq!(binding.compact_pack_plan().defined_tail_elements(), 0);
    assert!(!binding.compact_pack_plan().grants_copy_authority());
    assert!(binding.has_host_snapshot_internal_consistency_witness());
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
    assert_eq!(inverse_routing.to_host_vec(&stream)?, route_slots);
    Ok(())
}
