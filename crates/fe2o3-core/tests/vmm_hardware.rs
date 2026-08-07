use fe2o3_core::{AllocationKind, GpuContext, VmmAccess, VmmUnmappedAllocation};
use std::sync::Arc;

#[test]
#[ignore = "requires a HIP device with virtual-memory-management support"]
fn vmm_reserve_map_access_and_reclaim() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(GpuContext::new(0)?);
    let topology = context.observe_memory_topology()?;
    assert!(topology.capabilities().virtual_memory_management());

    let unmapped = VmmUnmappedAllocation::create(&context, topology, 1)?;
    let layout = unmapped.layout();
    assert_eq!(layout.requested_byte_len(), 1);
    assert!(layout.granularity() > 0);
    assert_eq!(layout.byte_len() % layout.granularity(), 0);
    assert_eq!(
        unmapped.reservation_identity().kind(),
        AllocationKind::VmmVirtualRange
    );
    assert_eq!(
        unmapped.physical_identity().kind(),
        AllocationKind::VmmPhysical
    );

    let accessible = unmapped
        .map()?
        .grant_access(topology, VmmAccess::ReadWrite)?;
    assert_eq!(
        accessible.access_for(topology.physical_device()),
        Some(VmmAccess::ReadWrite)
    );
    assert_eq!(
        accessible.query_access(topology)?.access(),
        VmmAccess::ReadWrite
    );
    assert!(!unsafe { accessible.raw_pointer() }.is_null());

    let reservation = accessible.reservation_identity();
    let physical = accessible.physical_identity();
    let receipt = accessible.reclaim()?;
    assert_eq!(receipt.reservation(), reservation);
    assert_eq!(receipt.physical(), physical);
    Ok(())
}
