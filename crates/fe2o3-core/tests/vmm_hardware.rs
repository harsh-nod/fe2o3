use fe2o3_core::{
    AllocationKind, Error, GpuContext, PeerAccessObservationError, VmmAccess, VmmUnmappedAllocation,
};
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

#[test]
fn opt_in_gfx942_second_device_access_and_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("FE2O3_ALLOW_GPU_SMOKE").as_deref() != Ok("1") {
        eprintln!("SKIP: set FE2O3_ALLOW_GPU_SMOKE=1 to exercise live VMM hardware");
        return Ok(());
    }

    let source = GpuContext::new(0)?;
    let destination = match GpuContext::new(1) {
        Ok(context) => context,
        Err(Error::NoDevice { count, .. }) => {
            eprintln!("SKIP: gfx942 multi-device VMM requires two devices; HIP reported {count}");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    let source_target = source.observe_target()?;
    let destination_target = destination.observe_target()?;
    if source_target.target_id().processor() != "gfx942"
        || destination_target.target_id().processor() != "gfx942"
    {
        eprintln!(
            "SKIP: expected two gfx942 devices, observed {} and {}",
            source_target.target_id(),
            destination_target.target_id()
        );
        return Ok(());
    }

    let source_topology = source.observe_memory_topology()?;
    let destination_topology = destination.observe_memory_topology()?;
    assert!(source_topology.is_for_context(&source));
    assert!(destination_topology.is_for_context(&destination));
    assert_eq!(source_topology.physical_device().ordinal(), 0);
    assert_eq!(destination_topology.physical_device().ordinal(), 1);
    assert_ne!(
        source_topology.physical_device().uuid(),
        destination_topology.physical_device().uuid(),
        "two HIP ordinals must not authenticate as the same physical device"
    );
    assert_ne!(
        source_topology.physical_device().pci_address(),
        destination_topology.physical_device().pci_address(),
        "two HIP ordinals must not report the same PCI function"
    );

    if !source_topology.capabilities().virtual_memory_management()
        || !destination_topology
            .capabilities()
            .virtual_memory_management()
    {
        eprintln!("SKIP: both gfx942 devices must explicitly report VMM support");
        return Ok(());
    }

    let _peer_capability = match source.observe_peer_access(&destination) {
        Ok(capability) => capability,
        Err(PeerAccessObservationError::Unavailable { direction }) => {
            eprintln!(
                "SKIP: HIP reports no peer path from device {} to device {}",
                direction.source_device(),
                direction.destination_device()
            );
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    let mut accessible = VmmUnmappedAllocation::create(&source, source_topology, 1)?
        .map()?
        .grant_access(source_topology, VmmAccess::ReadWrite)?;
    let reservation = accessible.reservation_identity();
    let physical = accessible.physical_identity();

    let grant = accessible.grant_access(destination_topology, VmmAccess::ReadWrite)?;
    assert_eq!(grant.reservation(), reservation);
    assert_eq!(grant.destination(), destination_topology.physical_device());
    assert_eq!(grant.access(), VmmAccess::ReadWrite);
    assert_eq!(
        accessible.access_for(destination_topology.physical_device()),
        Some(VmmAccess::ReadWrite)
    );

    let query = accessible.query_access(destination_topology)?;
    assert_eq!(query.reservation(), reservation);
    assert_eq!(query.destination(), destination_topology.physical_device());
    assert_eq!(query.access(), VmmAccess::ReadWrite);

    let cleanup = accessible.reclaim()?;
    assert_eq!(cleanup.reservation(), reservation);
    assert_eq!(cleanup.physical(), physical);

    let follow_up = VmmUnmappedAllocation::create(&source, source_topology, 1)?;
    let follow_up_reservation = follow_up.reservation_identity();
    let follow_up_physical = follow_up.physical_identity();
    assert_ne!(follow_up_reservation, reservation);
    assert_ne!(follow_up_physical, physical);
    let follow_up_cleanup = follow_up.reclaim()?;
    assert_eq!(follow_up_cleanup.reservation(), follow_up_reservation);
    assert_eq!(follow_up_cleanup.physical(), follow_up_physical);

    Ok(())
}
