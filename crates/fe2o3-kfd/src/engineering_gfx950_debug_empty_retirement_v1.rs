//! In-place retirement keeps every unresolved Allocation inside original custody.
use super::super::{Gfx950DebugColdOwnerV1, explain};
use super::{Gfx950DebugAllocationRetirementV1 as Operation, Gfx950DebugLocalErrorV1 as E};
use crate::memory::MemoryBackend;

pub(super) fn retire_allocation(
    cold: &mut Gfx950DebugColdOwnerV1,
    ordinal: u8,
    operation: Operation,
) -> Result<(), E> {
    let r = cold.resources.get_mut();
    let c = &mut r.context;
    let allocation = match ordinal {
        0..=5 => c.internal.get_mut(usize::from(ordinal)),
        6 => r.trap.as_mut(),
        7 => c.kernels.get_mut(&1).map(|kernel| &mut kernel.code),
        _ => None,
    }
    .ok_or(E::Contract("fixed allocation retirement ordinal"))?;
    // Each earlier operation is represented by the enclosing sticky cursor;
    // no removed Allocation or inferred handle may be supplied by a caller.
    match operation {
        Operation::UnmapGpu => {
            let result = c.backend.unmap_gpu(allocation.handle, 0);
            result.result.map_err(|e| E::Native(explain(e)))?;
            if result.value != 1 {
                return Err(E::Contract("selected GPU unmap count"));
            }
        }
        Operation::UnmapCpu => c
            .backend
            .unmap_cpu(&mut allocation.mapping)
            .map_err(|e| E::Native(explain(e)))?,
        Operation::FreeGpuHandle => c
            .backend
            .free(allocation.handle)
            .map_err(|e| E::Native(explain(e)))?,
        Operation::ReleaseVa => c
            .backend
            .release_va_reservation(&mut allocation.reservation)
            .map_err(|e| E::Native(explain(e)))?,
        Operation::ReconcileAccounting => {
            // Validate all arithmetic/membership BEFORE mutating bookkeeping.
            let remaining = c
                .total_bytes
                .checked_sub(allocation.backing as u64)
                .ok_or(E::Contract("allocation retirement accounting"))?;
            if !c.handles.contains(&allocation.handle)
                || allocation
                    .mmap_offset
                    .is_some_and(|offset| !c.mmap_offsets.contains(&offset))
            {
                return Err(E::Contract("allocation retirement identity"));
            }
            c.handles.remove(&allocation.handle);
            if let Some(offset) = allocation.mmap_offset {
                c.mmap_offsets.remove(&offset);
            }
            c.total_bytes = remaining;
        }
    }
    Ok(())
}
