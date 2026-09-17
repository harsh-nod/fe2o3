//! One bounded publication attempt between two complete 128-invocation groups.
//!
//! This is a visibility primitive, not a progress guarantee. Every consumer
//! first replaces its own flag with REQUEST, so even an initially READY flag
//! cannot authorize a payload read. Production admission additionally requires
//! exclusive payload custody, unique per-cell roles and the exact effect order.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{DisjointSlice, Index1D, thread};

pub const PUBLICATION_NOT_READY: u32 = 0;
pub const PUBLICATION_PUBLISHED: u32 = 1;
pub const PUBLICATION_READY: u32 = 2;
pub const PUBLICATION_INVALID: u32 = 3;

const CELLS: usize = 128;
const INVOCATIONS: usize = 256;
const REQUEST: u32 = 1;
const READY: u32 = 2;

/// Plain result data. A status value carries no permission to access memory.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_publication_attempt_f32_v1"]
pub struct PublicationAttemptF32 {
    pub status: u32,
    pub value: f32,
}

/// Attempts one publication per cell, with producer group zero and consumer one.
///
/// The admitted kernel must use 128 invocations per group. Both allocations
/// must have exactly 128 initialized elements and remain valid for the launch.
/// The payload is consumed; production compilation rejects all other accesses,
/// aliases and escapes of either root. Existing atomic memory eligibility and
/// host allocation-lifetime requirements remain in force.
///
/// Consumers make one attempt and may return `PUBLICATION_NOT_READY` even when
/// their producer ran. No wait, residency assumption or forward-progress claim
/// is made. Invalid shape or launch roles perform no memory effects. Every
/// result other than `PUBLICATION_READY` has positive zero as its value.
// The source proof needs this role CFG before ordinary helper lowering, not
// merely eventual LLVM inlining of an exclusive payload owner.
#[rustc_force_inline]
pub fn publish_once_128(
    payload: DisjointSlice<f32, Index1D>,
    flags: &[AtomicU32],
    producer_value: f32,
) -> PublicationAttemptF32 {
    if payload.len() != CELLS || flags.len() != CELLS {
        return PublicationAttemptF32 {
            status: PUBLICATION_INVALID,
            value: 0.0,
        };
    }
    if thread::launch_extent_1d() != INVOCATIONS {
        return PublicationAttemptF32 {
            status: PUBLICATION_INVALID,
            value: 0.0,
        };
    }
    let global = thread::index_1d().get();
    if global >= INVOCATIONS {
        return PublicationAttemptF32 {
            status: PUBLICATION_INVALID,
            value: 0.0,
        };
    }
    let cell = global % CELLS;
    if cell >= payload.len() || cell >= flags.len() {
        return PublicationAttemptF32 {
            status: PUBLICATION_INVALID,
            value: 0.0,
        };
    }
    if global < CELLS {
        publish_cell_128(payload, flags, cell, producer_value)
    } else {
        request_and_try_read_cell_128(payload, flags, cell)
    }
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_static_publication_128_publish_f32_v1"]
fn publish_cell_128(
    payload: DisjointSlice<f32, Index1D>,
    flags: &[AtomicU32],
    cell: usize,
    value: f32,
) -> PublicationAttemptF32 {
    // SAFETY: the sole public caller checks the actual payload and flag bounds.
    // Admitted source custody gives this cell one producer and excludes aliases;
    // the release below publishes this write to a successful consumer acquire.
    unsafe { payload.ptr.add(cell).write(value) };
    flags[cell].store(READY, Ordering::Release);
    PublicationAttemptF32 {
        status: PUBLICATION_PUBLISHED,
        value: 0.0,
    }
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_static_publication_128_try_read_f32_v1"]
fn request_and_try_read_cell_128(
    payload: DisjointSlice<f32, Index1D>,
    flags: &[AtomicU32],
    cell: usize,
) -> PublicationAttemptF32 {
    flags[cell].store(REQUEST, Ordering::Release);
    let observed = flags[cell].load(Ordering::Acquire);
    if observed == READY {
        // SAFETY: the caller proves the actual cell bounds. Write-read coherence
        // excludes READY values preceding this invocation's REQUEST; the unique
        // current producer's release therefore publishes the ordinary payload.
        let value = unsafe { payload.ptr.add(cell).read() };
        PublicationAttemptF32 {
            status: PUBLICATION_READY,
            value,
        }
    } else {
        PublicationAttemptF32 {
            status: PUBLICATION_NOT_READY,
            value: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_writes_only_its_cell_then_marks_ready() {
        let mut values = [0.0_f32; CELLS];
        let flags = [const { AtomicU32::new(0) }; CELLS];
        // SAFETY: this local array is exclusively used through the consumed slice.
        let payload = unsafe { DisjointSlice::from_raw_parts(values.as_mut_ptr(), CELLS) };
        let result = publish_cell_128(payload, &flags, 17, 4.5);
        assert_eq!(result.status, PUBLICATION_PUBLISHED);
        assert_eq!(result.value.to_bits(), 0);
        for (cell, value) in values.iter().enumerate() {
            assert_eq!(*value, if cell == 17 { 4.5 } else { 0.0 });
            assert_eq!(
                flags[cell].load(Ordering::Relaxed),
                if cell == 17 { READY } else { 0 }
            );
        }
    }

    #[test]
    fn consumer_does_not_accept_any_initial_ready_marker() {
        for initial in [0, REQUEST, READY, u32::MAX] {
            let mut values = [f32::NAN; CELLS];
            let flags = [const { AtomicU32::new(0) }; CELLS];
            flags[23].store(initial, Ordering::Relaxed);
            // SAFETY: this local array remains live with no concurrent accesses.
            let payload = unsafe { DisjointSlice::from_raw_parts(values.as_mut_ptr(), CELLS) };
            let result = request_and_try_read_cell_128(payload, &flags, 23);
            assert_eq!(result.status, PUBLICATION_NOT_READY);
            assert_eq!(result.value.to_bits(), 0);
            assert_eq!(flags[23].load(Ordering::Relaxed), REQUEST);
            assert!(values.iter().all(|value| value.is_nan()));
        }
    }

    #[test]
    fn invalid_shape_has_no_effects_before_device_index_query() {
        let mut values = [3.0_f32; CELLS - 1];
        let flags = [const { AtomicU32::new(READY) }; CELLS];
        // SAFETY: the array is live and exclusively leased for this call.
        let payload = unsafe { DisjointSlice::from_raw_parts(values.as_mut_ptr(), CELLS - 1) };
        let result = publish_once_128(payload, &flags, 9.0);
        assert_eq!(result.status, PUBLICATION_INVALID);
        assert_eq!(result.value.to_bits(), 0);
        assert!(values.iter().all(|value| *value == 3.0));
        assert!(
            flags
                .iter()
                .all(|flag| flag.load(Ordering::Relaxed) == READY)
        );
    }
}
