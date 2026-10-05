//! Grouped source argument for one finite-join invocation.
//!
//! This experimental type has no public constructor, compiler-issued instance,
//! generated host binder, or GPU admission yet. Generic by-value layout support
//! must not treat its pointer fields as inert scalar kernel arguments.

use core::{fmt, marker::PhantomData, sync::atomic::AtomicU32};

use super::{
    FiniteJoinWorkerResult, INPUT_ELEMENTS, INVALID, PAYLOAD_ELEMENTS, STATE_WORDS, run_leader,
};
use crate::thread;

/// An inseparable input, payload and state binding for the fixed finite join.
///
/// The value is not copyable and exposes neither raw roots nor a caller-selected
/// epoch. A checked implementation must issue it only for disjoint initialized
/// allocations, fresh state `[1, 3, 0, 0, 0, 0]`, and exactly two WG128 groups
/// using eligible system-atomic memory. Initialization must precede dispatch.
/// No such compiler/runtime issuer is implemented yet.
///
/// The device value does not replace the host's allocation lease. That lease
/// must retain all storage until every invocation is quiescent, including on
/// failure. A worker returning from [`Self::run`] is not dispatch completion.
#[must_use = "a finite-join argument is consumed by its worker entry"]
#[repr(C)]
pub struct FiniteJoinDispatch128<'dispatch> {
    input: *const [f32; INPUT_ELEMENTS],
    payload: *mut [f32; PAYLOAD_ELEMENTS],
    state: *const [AtomicU32; STATE_WORDS],
    borrows: PhantomData<(
        &'dispatch [f32; INPUT_ELEMENTS],
        &'dispatch mut [f32; PAYLOAD_ELEMENTS],
        &'dispatch mut [AtomicU32; STATE_WORDS],
    )>,
}

impl FiniteJoinDispatch128<'_> {
    /// Consumes this invocation's binding and runs the bounded worker protocol.
    ///
    /// Only workgroup leaders access payloads. The result records local work;
    /// it grants no permission to inspect outputs or reclaim shared storage.
    pub fn run(self) -> FiniteJoinWorkerResult {
        let extent = thread::launch_extent_1d();
        if extent != 256 {
            return FiniteJoinWorkerResult {
                error: INVALID,
                ..Default::default()
            };
        }
        self.run_at(thread::index_1d().get(), extent)
    }

    fn run_at(self, global: usize, extent: usize) -> FiniteJoinWorkerResult {
        if extent != 256 || global >= extent {
            return FiniteJoinWorkerResult {
                error: INVALID,
                ..Default::default()
            };
        }
        if global & 127 != 0 {
            return FiniteJoinWorkerResult::default();
        }
        // The private grouped representation carries all three roots. Issuing
        // it requires the fresh-dispatch, alias and lifetime obligations above;
        // matching an epoch word alone cannot establish those obligations.
        // SAFETY: no safe constructor can issue or replace these roots. The
        // invocation contract limits execution to the two distinct leaders and
        // preserves input immutability and unique task ownership of payloads.
        unsafe {
            run_leader(
                self.input.cast::<f32>(),
                self.payload.cast::<f32>(),
                &*self.state,
                1,
                (global / 128) as u32,
            )
        }
    }
}

impl fmt::Debug for FiniteJoinDispatch128<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FiniteJoinDispatch128")
            .field("input_elements", &INPUT_ELEMENTS)
            .field("payload_elements", &PAYLOAD_ELEMENTS)
            .field("state_words", &STATE_WORDS)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "finite_join_dispatch_tests.rs"]
mod tests;
