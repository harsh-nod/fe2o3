//! Real local operations driven only by the fixed sticky cursor.
use super::super::super::{CONTROL, CWSR, EOP, KERNARG, RING, SIGNAL, initialize_cwsr};
use super::super::{Backend, Gfx950DebugColdOwnerV1, Gfx950DebugQueueGeometryV1, explain};
use super::{Gfx950DebugLocalErrorV1 as E, Gfx950DebugLocalStepV1 as S};
use crate::engineering_gfx950_profile::{
    CONTEXT_BYTES_PER_XCC, CONTROL_STACK_BYTES, CWSR_BYTES, PAGE_BYTES, RING_BYTES, admit_doorbell,
};
use crate::engineering_gfx950_profile::{DEBUG_BYTES_TOTAL, XCC_COUNT};
use crate::memory::MemoryBackend;
use crate::queue_linux::{
    LinuxDestroyedQueueExceptionEventV1, LinuxDoorbellSliceV1, LinuxQueueExceptionEventV1,
};
use fe2o3_kfd_uapi::{
    KFD_IOC_QUEUE_TYPE_COMPUTE_AQL, KfdAllocMemoryFlags, KfdIoctlCreateQueueArgs,
    KfdIoctlDestroyQueueArgs,
};

pub(super) fn execute(
    cold: &mut Gfx950DebugColdOwnerV1,
    geometry: Gfx950DebugQueueGeometryV1,
    closure: [u8; 32],
    queue_call: &mut Option<KfdIoctlCreateQueueArgs>,
    event_destroyed: &mut Option<LinuxDestroyedQueueExceptionEventV1>,
    step: S,
) -> Result<(), E> {
    if cold.resources.get().context.backend.opener_pid() != std::process::id() {
        return Err(E::ProcessChanged);
    }
    // Full profile/currentness, never the optional operational shortcut.
    cold.resources.get_mut().context.check_currentness(true)?;
    match step {
        S::Revalidate => {
            let actual =
                super::super::execution::inspect(cold).map_err(|e| E::Native(explain(e)))?;
            if actual != (geometry, closure) || cold.resources.get_mut().finish()? != cold.facts {
                return Err(E::Contract("empty debug preparation changed"));
            }
            cold.resources
                .get_mut()
                .context
                .internal
                .try_reserve_exact(6)
                .map_err(|_| E::Contract("six retained resource slots"))?;
        }
        S::RegisterRuntime => {
            let r = cold.resources.get_mut();
            r.metadata
                .as_mut()
                .ok_or(E::Contract("metadata custody"))?
                .activate_no_queue(
                    r.trap.as_ref().ok_or(E::Contract("trap custody"))?,
                    &mut r.context,
                )
                .map_err(|e| E::Native(explain(e)))?;
        }
        S::CreateEvent => {
            let c = &mut cold.resources.get_mut().context;
            if c.event.is_some() || c.queue_id.is_some() || c.runtime.is_some() {
                return Err(E::Contract("foreign local queue/event/runtime"));
            }
            c.event = Some(
                LinuxQueueExceptionEventV1::create(c.backend.kfd_fd(), c.backend.opener_pid())
                    .map_err(|e| E::Native(explain(e)))?,
            );
        }
        S::AllocateRing => allocate(
            cold,
            RING,
            RING_BYTES,
            KfdAllocMemoryFlags::USERPTR_EXECUTABLE,
            |bytes| crate::queue::submit::initialize_invalid_ring(bytes).map_err(explain),
        )?,
        S::AllocateControl => allocate(
            cold,
            CONTROL,
            PAGE_BYTES,
            KfdAllocMemoryFlags::USERPTR_QUEUE_CONTROL,
            |bytes| crate::queue::submit::initialize_amd_aql_control(bytes).map_err(explain),
        )?,
        S::InitializeControl => {
            Backend::initialize_engineering_error_payload(
                &mut cold.resources.get_mut().context.internal[CONTROL].mapping,
            )
            .map_err(|e| E::Native(explain(e)))?;
        }
        S::AllocateSignal => allocate(
            cold,
            SIGNAL,
            PAGE_BYTES,
            KfdAllocMemoryFlags::KERNARG,
            |_| Ok(()),
        )?,
        S::InitializeSignal => {
            Backend::initialize_engineering_signal(
                &mut cold.resources.get_mut().context.internal[SIGNAL].mapping,
            )
            .map_err(|e| E::Native(explain(e)))?;
        }
        S::AllocateKernarg => allocate(cold, KERNARG, 65536, KfdAllocMemoryFlags::KERNARG, |_| {
            Ok(())
        })?,
        S::AllocateEop => allocate(
            cold,
            EOP,
            PAGE_BYTES,
            KfdAllocMemoryFlags::EXECUTABLE,
            |_| Ok(()),
        )?,
        S::AllocateCwsr => {
            let c = &cold.resources.get().context;
            let payload = c.internal[CONTROL]
                .va
                .checked_add(256)
                .ok_or(E::Contract("error payload extent"))?;
            let event_id = c
                .event
                .as_ref()
                .ok_or(E::Contract("event custody"))?
                .event_id_observation();
            allocate(
                cold,
                CWSR,
                CWSR_BYTES,
                KfdAllocMemoryFlags::USERPTR_EXECUTABLE,
                |bytes| initialize_cwsr(bytes, payload, event_id),
            )?;
        }
        S::CreateQueue => {
            check_resources(cold, geometry)?;
            check_owned_cwsr_headers(cold)?;
            inspect_empty(cold)?;
            let c = &mut cold.resources.get_mut().context;
            if queue_call.is_some() || c.queue_id.is_some() {
                return Err(E::Contract("queue already attempted"));
            }
            let expected = KfdIoctlCreateQueueArgs {
                ring_base_address: c.internal[RING].va,
                write_pointer_address: c.internal[CONTROL]
                    .va
                    .checked_add(0x38)
                    .ok_or(E::Contract("write pointer extent"))?,
                read_pointer_address: c.internal[CONTROL]
                    .va
                    .checked_add(0x80)
                    .ok_or(E::Contract("read pointer extent"))?,
                doorbell_offset: u64::MAX,
                ring_size: RING_BYTES as u32,
                gpu_id: c.backend.gpu_id(),
                queue_type: KFD_IOC_QUEUE_TYPE_COMPUTE_AQL,
                queue_percentage: 100,
                queue_priority: 0,
                queue_id: u32::MAX,
                eop_buffer_address: c.internal[EOP].va,
                eop_buffer_size: PAGE_BYTES as u64,
                ctx_save_restore_address: c.internal[CWSR].va,
                ctx_save_restore_size: CONTEXT_BYTES_PER_XCC as u32,
                ctl_stack_size: CONTROL_STACK_BYTES,
                sdma_engine_id: 0,
                pad: 0,
            };
            // Retain the actual in/out record even when ioctl returns an error.
            *queue_call = Some(expected);
            let returned = queue_call.as_mut().ok_or(E::Phase)?;
            crate::queue_linux::create_queue(c.backend.kfd_fd(), returned)
                .map_err(|e| E::Native(explain(e)))?;
            checked_create_output(expected, *returned)?;
            c.queue_id = Some(returned.queue_id);
        }
        S::MapDoorbell => {
            let returned = queue_call.as_ref().ok_or(E::Phase)?;
            let c = &mut cold.resources.get_mut().context;
            if c.queue_id != Some(returned.queue_id) || c.doorbell.is_some() {
                return Err(E::Contract("doorbell queue custody"));
            }
            let plan = admit_doorbell(
                returned.queue_id,
                returned.doorbell_offset,
                c.backend.gpu_id(),
            )
            .map_err(E::Contract)?;
            c.doorbell = Some(
                LinuxDoorbellSliceV1::map_gfx950(c.backend.kfd_fd(), plan, c.backend.opener_pid())
                    .map_err(|e| E::Native(explain(e)))?,
            );
        }
        S::InspectEmpty => {
            check_resources(cold, geometry)?;
            inspect_empty(cold)?;
        }
        S::DestroyQueue => {
            check_resources(cold, geometry)?;
            inspect_empty(cold)?;
            let c = &mut cold.resources.get_mut().context;
            let id = c.queue_id.ok_or(E::Contract("missing owned empty queue"))?;
            if queue_call.as_ref().map(|r| r.queue_id) != Some(id) {
                return Err(E::Contract("queue identity changed"));
            }
            let expected = KfdIoctlDestroyQueueArgs::new(id);
            let mut returned = expected;
            crate::queue_linux::destroy_queue(c.backend.kfd_fd(), &mut returned)
                .map_err(|e| E::Native(explain(e)))?;
            if returned != expected {
                return Err(E::Contract("DESTROY_QUEUE output"));
            }
            c.queue_id = None;
        }
        S::DestroyEvent => {
            let c = &mut cold.resources.get_mut().context;
            if c.queue_id.is_some() || event_destroyed.is_some() {
                return Err(E::Phase);
            }
            *event_destroyed = Some(
                c.event
                    .as_mut()
                    .ok_or(E::Contract("event custody"))?
                    .destroy_empty_debug_in_place(c.backend.kfd_fd(), c.backend.opener_pid())
                    .map_err(|e| E::Native(explain(e)))?,
            );
        }
        S::WithdrawMetadata | S::DisableRuntime | S::ClearTrap => {
            let r = cold.resources.get_mut();
            if r.context.queue_id.is_some() || event_destroyed.is_none() {
                return Err(E::Phase);
            }
            let metadata = r.metadata.as_mut().ok_or(E::Contract("metadata custody"))?;
            match step {
                S::WithdrawMetadata => metadata.withdraw_empty_queue(&mut r.context),
                S::DisableRuntime => metadata.disable_empty_runtime(&mut r.context),
                S::ClearTrap => metadata.clear_empty_trap(&mut r.context),
                _ => unreachable!(),
            }
            .map_err(|e| E::Native(explain(e)))?;
        }
        S::UnmapDoorbell => {
            cold.resources
                .get_mut()
                .context
                .doorbell
                .as_mut()
                .ok_or(E::Contract("doorbell custody"))?
                .release_empty_debug_in_place()
                .map_err(|e| E::Native(explain(e)))?;
        }
        S::RetireAllocation { ordinal, operation } => {
            super::retirement::retire_allocation(cold, ordinal, operation)?
        }
        S::Reconcile => {
            let r = cold.resources.get_mut();
            let c = &mut r.context;
            if c.queue_id.is_some()
                || c.runtime.is_some()
                || event_destroyed.is_none()
                || !c.handles.is_empty()
                || !c.mmap_offsets.is_empty()
                || c.total_bytes != 0
                || !c.buffers.is_empty()
                || c.internal.len() != 6
                || c.kernels.len() != 1
                || r.trap.is_none()
                || !r
                    .metadata
                    .as_ref()
                    .is_some_and(|m| m.empty_local_trap_cleared())
            {
                return Err(E::Contract("local teardown final custody/accounting"));
            }
            // All eight allocation retirements and doorbell/event operations
            // were completed in-place. Only now discard inert wrapper records.
            c.internal.clear();
            c.kernels.clear();
            r.trap = None;
            c.event = None;
            c.doorbell = None;
        }
    }
    cold.resources.get_mut().context.check_currentness(true)?;
    Ok(())
}

fn allocate(
    cold: &mut Gfx950DebugColdOwnerV1,
    ordinal: usize,
    bytes: usize,
    flags: KfdAllocMemoryFlags,
    init: impl FnOnce(&mut [u8]) -> Result<(), String>,
) -> Result<(), E> {
    let c = &mut cold.resources.get_mut().context;
    if c.internal.len() != ordinal || ordinal >= 6 || c.internal.capacity() < 6 {
        return Err(E::Contract("fixed resource roster/order"));
    }
    let resource = c.allocate_resource(bytes, flags, init)?;
    // Capacity was reserved before native registration. No intervening
    // fallible operation may strand a successful returned allocation.
    c.internal.push(resource);
    Ok(())
}

fn check_resources(
    cold: &Gfx950DebugColdOwnerV1,
    geometry: Gfx950DebugQueueGeometryV1,
) -> Result<(), E> {
    let c = &cold.resources.get().context;
    let sizes = [
        RING_BYTES, PAGE_BYTES, PAGE_BYTES, 65536, PAGE_BYTES, CWSR_BYTES,
    ];
    if c.internal.len() != sizes.len()
        || c.runtime.is_some()
        || c.event.is_none()
        || !c.buffers.is_empty()
        || c.handles.len() != 8
        || c.mmap_offsets.len() != 5
        || c.total_bytes
            != cold
                .facts
                .mapped_backing_bytes()
                .checked_add(geometry.queue_backing_bytes())
                .ok_or(E::Contract("fixed geometry sum"))?
        || c.total_bytes > geometry.projected_native_bytes()
    {
        return Err(E::Contract("fixed resource cardinality/geometry"));
    }
    for (i, (a, expected)) in c.internal.iter().zip(sizes).enumerate() {
        if a.requested != expected
            || a.backing != expected
            || a.va == 0
            || !a.va.is_multiple_of(PAGE_BYTES as u64)
            || Backend::mapping_address(&a.mapping) != a.va
            || !c.handles.contains(&a.handle)
            || a.va.checked_add(a.backing as u64).is_none()
        {
            return Err(E::Contract("fixed allocation identity"));
        }
        for b in c.internal.iter().take(i) {
            if a.handle == b.handle
                || !(a.va + a.backing as u64 <= b.va || b.va + b.backing as u64 <= a.va)
            {
                return Err(E::Contract("aliased queue backing"));
            }
        }
    }
    Ok(())
}

pub(super) fn check_owned_cwsr_headers(cold: &Gfx950DebugColdOwnerV1) -> Result<(), E> {
    let c = &cold.resources.get().context;
    let payload = c.internal[CONTROL]
        .va
        .checked_add(256)
        .ok_or(E::Contract("CWSR payload extent"))?;
    let event = c
        .event
        .as_ref()
        .ok_or(E::Contract("CWSR event custody"))?
        .event_id_observation();
    let valid = Backend::with_bytes(&c.internal[CWSR].mapping, CWSR_BYTES, |bytes| {
        (0..XCC_COUNT).all(|xcc| {
            let base = xcc * CONTEXT_BYTES_PER_XCC;
            let h = &bytes[base + 16..base + 40];
            cwsr_header_matches(xcc, h, payload, event)
        })
    });
    if valid {
        Ok(())
    } else {
        Err(E::Contract("actual CWSR/event header join"))
    }
}

fn cwsr_header_matches(xcc: usize, h: &[u8], payload: u64, event: u32) -> bool {
    if xcc >= XCC_COUNT || h.len() != 24 || payload == 0 || !payload.is_multiple_of(8) || event == 0
    {
        return false;
    }
    let offset = ((XCC_COUNT - xcc) * CONTEXT_BYTES_PER_XCC) as u32;
    h[..4] == offset.to_le_bytes()
        && h[4..8] == DEBUG_BYTES_TOTAL.to_le_bytes()
        && h[8..16] == payload.to_le_bytes()
        && h[16..20] == event.to_le_bytes()
        && h[20..24] == [0; 4]
}

fn inspect_empty(cold: &mut Gfx950DebugColdOwnerV1) -> Result<(), E> {
    let c = &mut cold.resources.get_mut().context;
    if c.internal.len() != 6
        || c.ring.write() != 0
        || c.ring.last_read() != 0
        || c.completed_write != 0
        || c.last_observed_read != 0
        || c.queue_epoch != 0
        || c.performance.is_some()
        || c.ordered_batch_poisoned
    {
        return Err(E::Contract("nonempty local queue model"));
    }
    let counters = Backend::observe_aql_counters(&mut c.internal[CONTROL].mapping, PAGE_BYTES)
        .map_err(|e| E::Native(explain(e)))?;
    let exception = Backend::observe_i64_acquire(&mut c.internal[CONTROL].mapping, PAGE_BYTES, 256)
        .map_err(|e| E::Native(explain(e)))?;
    // The unused completion signal stays Pending, not falsely Completed.
    let signal =
        Backend::observe_completion_signal_acquire(&mut c.internal[SIGNAL].mapping, PAGE_BYTES, 0)
            .map_err(|e| E::Native(explain(e)))?;
    if counters != (0, 0)
        || exception != 0
        || signal != fe2o3_aql::AqlCompletionObservationV1::Pending
    {
        return Err(E::Contract("nonempty native queue observation"));
    }
    let empty = Backend::with_bytes(&c.internal[RING].mapping, RING_BYTES, |bytes| {
        bytes.chunks_exact(64).all(|slot| {
            slot[..2] == fe2o3_aql::AQL_INVALID_PACKET_HEADER_V1.to_le_bytes()
                && slot[2..].iter().all(|v| *v == 0)
        })
    });
    if !empty {
        return Err(E::Contract("packet publication absent invariant"));
    }
    Ok(())
}

fn checked_create_output(
    expected: KfdIoctlCreateQueueArgs,
    observed: KfdIoctlCreateQueueArgs,
) -> Result<(), E> {
    admit_doorbell(observed.queue_id, observed.doorbell_offset, expected.gpu_id)
        .map_err(E::Contract)?;
    let mut immutable = observed;
    immutable.queue_id = expected.queue_id;
    immutable.doorbell_offset = expected.doorbell_offset;
    if immutable != expected {
        return Err(E::Contract("CREATE_QUEUE changed immutable inputs"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "engineering_gfx950_debug_empty_native_v1_tests.rs"]
mod tests;
