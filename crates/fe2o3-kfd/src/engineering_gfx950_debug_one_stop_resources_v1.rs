//! Same-owned nine-allocation joins, before any publication; no caller ranges.
use super::super::super::super::{CONTROL, KERNARG, RING, SIGNAL};
use super::super::super::Backend;
use super::{E, Inner, contract};
use crate::engineering_gfx950_profile::{CWSR_BYTES, PAGE_BYTES, RING_BYTES};
use crate::memory::MemoryBackend;
use sha2::{Digest, Sha256};

pub(super) fn check(base: &Inner) -> Result<(), E> {
    let r = base.cold.resources.get();
    let c = &r.context;
    let requested = [
        RING_BYTES, PAGE_BYTES, PAGE_BYTES, 65536, PAGE_BYTES, CWSR_BYTES, 272,
    ];
    if c.internal.len() != 7
        || c.internal.capacity() != 7
        || c.handles.len() != 9
        || c.mmap_offsets.len() != 6
        || !c.buffers.is_empty()
        || c.kernels.len() != 1
        || c.runtime.is_some()
        || c.event.is_none()
        || c.queue_id.is_none()
        || c.doorbell.is_none()
        || c.performance.is_some()
        || c.ordered_batch_poisoned
        || c.total_bytes != base.geometry.projected_native_bytes()
        || c.queue_epoch != 0
        || c.next_buffer != 1
        || c.next_kernel != 2
        || base.queue_call.as_ref().map(|q| q.queue_id) != c.queue_id
    {
        return Err(E::Contract("one-stop exact native resource roster"));
    }
    let kernel = c
        .kernels
        .get(&1)
        .ok_or(E::Contract("one-stop kernel custody"))?;
    let trap = r
        .trap
        .as_ref()
        .ok_or(E::Contract("one-stop trap custody"))?;
    for (i, a) in c.internal.iter().chain([trap, &kernel.code]).enumerate() {
        let expected = if i < 7 {
            requested[i]
        } else if i == 7 {
            trap.requested
        } else {
            kernel.code.requested
        };
        let backing = expected
            .checked_add(4095)
            .map(|v| v & !4095)
            .ok_or(E::Contract("one-stop allocation rounding"))?;
        if a.requested != expected
            || a.backing != backing
            || a.va == 0
            || !a.va.is_multiple_of(4096)
            || a.handle == 0
            || !c.handles.contains(&a.handle)
            || a.mmap_offset
                .is_some_and(|offset| !c.mmap_offsets.contains(&offset))
            || a.va.checked_add(a.backing as u64).is_none()
            || Backend::mapping_address(&a.mapping) != a.va
            || Backend::reservation_address(&a.reservation) != a.va
        {
            return Err(E::Contract("one-stop owned allocation identity"));
        }
        for b in c.internal.iter().chain([trap, &kernel.code]).take(i) {
            if a.handle == b.handle
                || !(a.va + a.backing as u64 <= b.va || b.va + b.backing as u64 <= a.va)
            {
                return Err(E::Contract("one-stop nonaliasing actual allocations"));
            }
        }
    }
    let closure = contract::validate_object(&kernel.object)?;
    if closure.selected_kernel() != &kernel.inspected
        || closure.resources() != kernel.resources
        || kernel.metadata.object_sha256 != contract::OBJECT_SHA256
        || base.cold.facts.artifact_sha256() != contract::OBJECT_SHA256
        || base.cold.facts.artifact_bytes() != contract::OBJECT_BYTES
        || closure.identity_inputs().closure_sha256() != base.closure
        || closure
            .selected_binding()
            .descriptor_address()
            .checked_sub(closure.envelope().plan().image_start())
            != Some(kernel.descriptor_offset)
    {
        return Err(E::Contract("one-stop retained object/source relation"));
    }
    let trap_match = Backend::with_bytes(&trap.mapping, trap.backing, |bytes| {
        let logical = base.cold.facts.trap_bytes();
        logical <= bytes.len()
            && <[u8; 32]>::from(Sha256::digest(&bytes[..logical])) == base.cold.facts.trap_sha256()
            && bytes[logical..].iter().all(|v| *v == 0)
    });
    if !trap_match {
        return Err(E::Contract("one-stop retained trap bytes"));
    }
    // Existing helper checks the actual 8-XCC headers against the actual event.
    super::super::native::check_owned_cwsr_headers(&base.cold)?;
    Ok(())
}
pub(super) fn check_prepublication(base: &mut Inner) -> Result<(), E> {
    check(base)?;
    let c = &mut base.cold.resources.get_mut().context;
    if c.ring.write() != 0
        || c.ring.last_read() != 0
        || c.completed_write != 0
        || c.last_observed_read != 0
    {
        return Err(E::Contract("one-stop original empty model frontier"));
    }
    let counters = Backend::observe_aql_counters(&mut c.internal[CONTROL].mapping, PAGE_BYTES)
        .map_err(|e| E::Native(format!("{e:?}")))?;
    let exception = Backend::observe_i64_acquire(&mut c.internal[CONTROL].mapping, PAGE_BYTES, 256)
        .map_err(|e| E::Native(format!("{e:?}")))?;
    let signal = Backend::observe_completion_signal_state_acquire(
        &mut c.internal[SIGNAL].mapping,
        PAGE_BYTES,
        0,
    )
    .map_err(|e| E::Native(format!("{e:?}")))?;
    if counters != (0, 0)
        || exception != 0
        || signal
            != (
                fe2o3_aql::AMD_SIGNAL_KIND_USER_V1,
                fe2o3_aql::AMD_SIGNAL_VALUE_PENDING_V1,
            )
    {
        return Err(E::Contract("one-stop original native frontier/signal"));
    }
    let empty = Backend::with_bytes(&c.internal[RING].mapping, RING_BYTES, |bytes| {
        bytes.chunks_exact(64).all(|slot| {
            slot[..2] == fe2o3_aql::AQL_INVALID_PACKET_HEADER_V1.to_le_bytes()
                && slot[2..].iter().all(|v| *v == 0)
        })
    });
    if !empty {
        return Err(E::Contract("one-stop all-invalid original ring"));
    }
    Backend::with_bytes(&c.internal[contract::OUTPUT].mapping, 4096, |bytes| {
        contract::check_output(bytes, false)
    })?;
    Backend::with_bytes(&c.internal[KERNARG].mapping, 65536, |bytes| {
        contract::check_kernarg(&bytes[..264], c.internal[contract::OUTPUT].va + 8)?;
        if bytes[264..].iter().any(|v| *v != 0) {
            return Err(E::Contract("one-stop kernarg backing tail"));
        }
        Ok(())
    })
}
pub(super) fn completion(
    counters: (u64, u64),
    signal: (i64, i64),
    exception: i64,
) -> Result<bool, E> {
    if counters.0 != 1
        || counters.1 > 1
        || exception != 0
        || signal.0 != fe2o3_aql::AMD_SIGNAL_KIND_USER_V1
    {
        return Err(E::Contract(
            "one-stop actual completion frontier/exception/kind",
        ));
    }
    match signal.1 {
        fe2o3_aql::AMD_SIGNAL_VALUE_PENDING_V1 => Ok(false),
        0 if counters == (1, 1) => Ok(true),
        0 => Err(E::Contract(
            "one-stop completed signal without read frontier one",
        )),
        _ => Err(E::Contract("one-stop unexpected completion value")),
    }
}
