use fe2o3_core::{
    AllocationKind, GpuContext, ManagedAdviceRequest, ManagedAllocation, ManagedMemoryLocation,
};
use std::sync::Arc;

#[test]
#[ignore = "requires a HIP device with managed-memory support"]
fn managed_allocation_advice_and_prefetch_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(GpuContext::new(0)?);
    let topology = context.observe_memory_topology()?;
    assert!(topology.capabilities().managed_memory());
    assert!(topology.is_for_context(&context));

    let mut allocation = ManagedAllocation::allocate(&context, topology, 4096)?;
    let identity = allocation.identity();
    assert_eq!(identity.kind(), AllocationKind::Managed);
    assert_eq!(identity.byte_len(), 4096);
    assert_eq!(identity.context(), context.identity());
    assert_eq!(identity.physical_device(), topology.physical_device());

    allocation.apply_advice(ManagedAdviceRequest::SetReadMostly)?;
    assert!(allocation.advice_state().read_mostly());
    allocation.apply_advice(ManagedAdviceRequest::UnsetReadMostly)?;
    assert!(!allocation.advice_state().read_mostly());

    let stream = context.default_stream();
    let device = ManagedMemoryLocation::device(topology);
    allocation.prefetch_to_device(&stream, topology)?;
    assert_eq!(allocation.query_last_prefetch_location(device)?.location(), device);

    let host = ManagedMemoryLocation::host();
    allocation.prefetch_to_host(&stream)?;
    assert_eq!(allocation.query_last_prefetch_location(host)?.location(), host);

    assert_eq!(allocation.reclaim()?.identity(), identity);
    Ok(())
}
