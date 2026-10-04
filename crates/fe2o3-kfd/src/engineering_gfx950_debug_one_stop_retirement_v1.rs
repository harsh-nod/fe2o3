//! Distinct completed-dispatch retirement. Never delegates to EmptyQueue::close.
use super::super::Gfx950DebugAllocationRetirementV1 as Operation;
use super::{E, Inner};
use crate::memory::MemoryBackend;
pub(super) fn retire(base: &mut Inner, ordinal: u8, operation: Operation) -> Result<(), E> {
    // Preserve the existing exact eight-allocation operation bodies/order;
    // the separately retained ninth output is last. Nothing is removed early.
    if ordinal < 8 {
        return super::super::retirement::retire_allocation(&mut base.cold, ordinal, operation);
    }
    if ordinal != 8 {
        return Err(E::Contract("one-stop nine-allocation ordinal"));
    }
    let c = &mut base.cold.resources.get_mut().context;
    let a = c
        .internal
        .get_mut(super::contract::OUTPUT)
        .ok_or(E::Contract("one-stop output retirement custody"))?;
    match operation {
        Operation::UnmapGpu => {
            let result = c.backend.unmap_gpu(a.handle, 0);
            result.result.map_err(|e| E::Native(format!("{e:?}")))?;
            if result.value != 1 {
                return Err(E::Contract("one-stop output GPU unmap count"));
            }
        }
        Operation::UnmapCpu => c
            .backend
            .unmap_cpu(&mut a.mapping)
            .map_err(|e| E::Native(format!("{e:?}")))?,
        Operation::FreeGpuHandle => c
            .backend
            .free(a.handle)
            .map_err(|e| E::Native(format!("{e:?}")))?,
        Operation::ReleaseVa => c
            .backend
            .release_va_reservation(&mut a.reservation)
            .map_err(|e| E::Native(format!("{e:?}")))?,
        Operation::ReconcileAccounting => {
            let remaining = c
                .total_bytes
                .checked_sub(a.backing as u64)
                .ok_or(E::Contract("one-stop output accounting"))?;
            if !c.handles.contains(&a.handle)
                || a.mmap_offset.is_some_and(|o| !c.mmap_offsets.contains(&o))
            {
                return Err(E::Contract("one-stop output retirement identity"));
            }
            c.handles.remove(&a.handle);
            if let Some(offset) = a.mmap_offset {
                c.mmap_offsets.remove(&offset);
            }
            c.total_bytes = remaining;
        }
    }
    Ok(())
}
pub(super) fn reconcile(base: &mut Inner, completed: bool) -> Result<(), E> {
    let r = base.cold.resources.get_mut();
    let c = &mut r.context;
    if !completed
        || c.queue_id.is_some()
        || c.runtime.is_some()
        || base.event_destroyed.is_none()
        || !c.handles.is_empty()
        || !c.mmap_offsets.is_empty()
        || c.total_bytes != 0
        || !c.buffers.is_empty()
        || c.internal.len() != 7
        || c.kernels.len() != 1
        || r.trap.is_none()
        || c.ring.write() != 1
        || c.completed_write != 1
        || c.last_observed_read != 1
        || c.queue_epoch != 0
        || !r
            .metadata
            .as_ref()
            .is_some_and(|m| m.empty_local_trap_cleared())
    {
        return Err(E::Contract(
            "one-stop completed nine-allocation terminal accounting",
        ));
    }
    // Every original allocation operation has already returned success. These
    // wrapper drops perform no native retirement or permission transition.
    c.internal.clear();
    c.kernels.clear();
    r.trap = None;
    c.event = None;
    c.doorbell = None;
    Ok(())
}
