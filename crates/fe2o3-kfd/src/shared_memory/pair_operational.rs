//! Paired fail-closed custody around the narrower retained-queue observations.

use super::*;

pub(super) fn validate_direction(
    source_vm: VmKeyV1,
    source_gpu: u32,
    peer_gpu: u32,
    route: crate::topology::Gfx942XgmiRouteV1,
    queue: QueueKeyV1,
) -> Result<(), MemorySessionError> {
    if queue.vm != source_vm
        || source_gpu != route.source_gpu_id()
        || peer_gpu != route.destination_gpu_id()
        || queue.id.0 == 0
        || queue.generation.0 == 0
    {
        return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
    }
    Ok(())
}

pub(super) fn validate_resource<B, R, P, S>(
    engine: &SharedMemoryEngine<B>,
    vm: VmKeyV1,
    authority: &SharedGttQueueResourceAuthorityV1<R, P, S>,
) -> Result<(), MemorySessionError>
where
    B: MemoryBackend,
    R: SharedGttQueueResourceRoleV1,
    P: GttProfileV1,
    S: GpuMappedGttStateV1,
{
    if engine.mapped_resource_facts_v1(&authority.token, vm)? != authority.facts {
        return Err(MemorySessionError::InvalidAllocationAuthority);
    }
    Ok(())
}

pub(super) fn check<B: MemoryBackend>(
    source: &mut SharedMemoryEngine<B>,
    peer: &mut SharedMemoryEngine<B>,
) -> Result<(), MemorySessionError> {
    pair_currentness::with_terminal_pair(&mut source.phase, &mut peer.phase, || {
        if source.backend.opener_pid() != std::process::id() {
            return Err(MemorySessionError::ProcessChanged);
        }
        source.backend.check_operational_currentness()?;
        if peer.backend.opener_pid() != std::process::id() {
            return Err(MemorySessionError::ProcessChanged);
        }
        peer.backend.check_operational_currentness()
    })
}
