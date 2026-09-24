//! Packet-incapable LOCAL native ownership. Not debugger acceptance or execution.
use super::{
    Gfx950DebugColdOwnerV1, Gfx950DebugColdPreparationFactsV1, Gfx950DebugQueueGeometryV1,
};
use crate::memory::MemoryBackend;
use crate::queue_linux::{LinuxDestroyedQueueExceptionEventV1, ProcessGlobalKfdDebugReservationV1};
use core::fmt;
use fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs;

#[path = "engineering_gfx950_debug_empty_cursor_v1.rs"]
mod cursor;
#[path = "engineering_gfx950_debug_empty_native_v1.rs"]
mod native;
#[path = "engineering_gfx950_debug_one_stop_owner_v1.rs"]
pub(super) mod one_stop;
#[path = "engineering_gfx950_debug_empty_retirement_v1.rs"]
mod retirement;
use cursor::Cursor;
pub use cursor::{
    Gfx950DebugAllocationRetirementV1, Gfx950DebugLocalPhaseV1, Gfx950DebugLocalStepV1,
};

/// Exact local refusal; native errors do not mean native effects were absent.
#[derive(Debug)]
pub enum Gfx950DebugLocalErrorV1 {
    ProcessChanged,
    Phase,
    Contract(&'static str),
    Native(String),
}
impl From<String> for Gfx950DebugLocalErrorV1 {
    fn from(value: String) -> Self {
        Self::Native(value)
    }
}

struct Inner {
    // This owner already retains all native storage on Drop, before its gate drops.
    cold: Gfx950DebugColdOwnerV1,
    geometry: Gfx950DebugQueueGeometryV1,
    closure: [u8; 32],
    cursor: Cursor,
    // Keep even malformed/ambiguous returned native records privately for custody.
    queue_call: Option<KfdIoctlCreateQueueArgs>,
    event_destroyed: Option<LinuxDestroyedQueueExceptionEventV1>,
}
impl Inner {
    fn step(&mut self) -> Result<(), Gfx950DebugLocalErrorV1> {
        let Self {
            cold,
            geometry,
            closure,
            cursor,
            queue_call,
            event_destroyed,
        } = self;
        cursor.run(std::process::id(), |step| {
            native::execute(cold, *geometry, *closure, queue_call, event_destroyed, step)
        })
    }
    fn until(&mut self, phase: Gfx950DebugLocalPhaseV1) -> Result<(), Gfx950DebugLocalErrorV1> {
        // The private fixed cursor has 61 steps; callers cannot supply a plan.
        for _ in 0..61 {
            if self.cursor.phase() == phase {
                return Ok(());
            }
            self.step()?;
        }
        if self.cursor.phase() == phase {
            Ok(())
        } else {
            Err(Gfx950DebugLocalErrorV1::Phase)
        }
    }
    fn failure(self: Box<Self>, error: Gfx950DebugLocalErrorV1) -> Gfx950DebugLocalFailureV1 {
        Gfx950DebugLocalFailureV1 {
            retained: self,
            error,
        }
    }
}

/// Retains all remaining custody after refusal. No retry, cleanup, address or
/// queue accessor exists. Dropping this is not cleanup; the disposable process
/// supervisor must terminate/reap the family after any ambiguous native result.
pub struct Gfx950DebugLocalFailureV1 {
    retained: Box<Inner>,
    error: Gfx950DebugLocalErrorV1,
}
impl Gfx950DebugLocalFailureV1 {
    pub fn phase(&self) -> Gfx950DebugLocalPhaseV1 {
        self.retained.cursor.phase()
    }
    pub fn reason(&self) -> &Gfx950DebugLocalErrorV1 {
        &self.error
    }
}
impl fmt::Debug for Gfx950DebugLocalFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gfx950DebugLocalFailureV1")
            .field("phase", &self.phase())
            .field("reason", &self.error)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Gfx950DebugLocalFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "local empty queue {:?}: {:?}; remaining custody retained",
            self.phase(),
            self.error
        )
    }
}
impl std::error::Error for Gfx950DebugLocalFailureV1 {}

/// Mode3 returned and actual metadata was locally published. This says neither
/// attached debugger nor LoadedSuccess/ACK/no-sampling. No packet can be emitted.
/// A later create consumes this same actual cold/resource custody.
///
/// A plain numeric record cannot construct this owner.
/// ```compile_fail
/// use fe2o3_kfd::Gfx950DebugRuntimeEnableReturnedV1;
/// fn fabricate() { let _ = Gfx950DebugRuntimeEnableReturnedV1 { queue_id: 0 }; }
/// ```
/// ```compile_fail
/// use fe2o3_kfd::Gfx950DebugRuntimeEnableReturnedV1;
/// fn duplicate(v: Gfx950DebugRuntimeEnableReturnedV1) { let _ = v.clone(); }
/// ```
pub struct Gfx950DebugRuntimeEnableReturnedV1 {
    inner: Box<Inner>,
}

/// Exactly one native queue, with zero publications by construction. No dispatch,
/// doorbell store, update, rollover, source admission or GPU read capability.
///
/// The existing plain/gfx942/noqueue owners cannot be promoted.
/// ```compile_fail
/// use fe2o3_kfd::Gfx950DebugEmptyQueueV1;
/// fn dispatch(v: Gfx950DebugEmptyQueueV1) { v.dispatch(); }
/// ```
/// ```compile_fail
/// use fe2o3_kfd::Gfx950DebugEmptyQueueV1;
/// fn notify(v: Gfx950DebugEmptyQueueV1) { v.doorbell_store(0); }
/// ```
/// ```compile_fail
/// use fe2o3_kfd::Gfx950DebugEmptyQueueV1;
/// fn fake_close(v: Gfx950DebugEmptyQueueV1) { v.complete_teardown(true); }
/// ```
/// ```compile_fail
/// use fe2o3_kfd::Gfx950DebugEmptyQueueV1;
/// fn require_send<T: Send>() {}
/// require_send::<Gfx950DebugEmptyQueueV1>();
/// ```
/// ```compile_fail
/// use fe2o3_kfd::Gfx950DebugMetadataNoQueueOwnerV1;
/// fn promote(v: Gfx950DebugMetadataNoQueueOwnerV1) { v.create_empty_queue(); }
/// ```
pub struct Gfx950DebugEmptyQueueV1 {
    inner: Box<Inner>,
}

/// Every local native teardown and accounting step returned success; descriptors
/// have been dropped last. Not same-client ACK, external family cleanup or a
/// reusable runtime/dispatch token.
#[derive(Debug)]
pub struct Gfx950DebugLocalReleaseCompleteV1 {
    facts: Gfx950DebugColdPreparationFactsV1,
}
impl Gfx950DebugLocalReleaseCompleteV1 {
    pub fn preparation_facts(&self) -> Gfx950DebugColdPreparationFactsV1 {
        self.facts
    }
}

pub(super) fn begin(
    cold: Gfx950DebugColdOwnerV1,
    geometry: Gfx950DebugQueueGeometryV1,
    closure: [u8; 32],
) -> Result<Gfx950DebugRuntimeEnableReturnedV1, Gfx950DebugLocalFailureV1> {
    let pid = cold.resources.get().context.backend.opener_pid();
    // One owner allocation, before the first Revalidate/RegisterRuntime step.
    // The original cold retention is already armed; no later failure reboxes it.
    let mut inner = Box::new(Inner {
        cold,
        geometry,
        closure,
        cursor: Cursor::new(pid),
        queue_call: None,
        event_destroyed: None,
    });
    if let Err(error) = inner.until(Gfx950DebugLocalPhaseV1::Ready(
        Gfx950DebugLocalStepV1::CreateEvent,
    )) {
        return Err(inner.failure(error));
    }
    Ok(Gfx950DebugRuntimeEnableReturnedV1 { inner })
}
impl Gfx950DebugRuntimeEnableReturnedV1 {
    pub fn preparation_facts(&self) -> Gfx950DebugColdPreparationFactsV1 {
        self.inner.cold.facts
    }
    pub fn queue_geometry(&self) -> Gfx950DebugQueueGeometryV1 {
        self.inner.geometry
    }
    /// Create only an empty native queue; this may perform allocations/ioctls.
    /// The unsafe isolated-process invariant established at begin must still hold.
    pub fn create_empty_queue(
        mut self,
    ) -> Result<Gfx950DebugEmptyQueueV1, Gfx950DebugLocalFailureV1> {
        if let Err(error) = self.inner.until(Gfx950DebugLocalPhaseV1::Ready(
            Gfx950DebugLocalStepV1::DestroyQueue,
        )) {
            return Err(self.inner.failure(error));
        }
        Ok(Gfx950DebugEmptyQueueV1 { inner: self.inner })
    }
}
impl Gfx950DebugEmptyQueueV1 {
    pub fn queue_geometry(&self) -> Gfx950DebugQueueGeometryV1 {
        self.inner.geometry
    }
    /// Fixed explicit teardown. Runtime disable can block on an attached
    /// debugger; the external disposable-process supervisor owns the deadline.
    /// Unknown/error results consume custody into a terminal retained failure.
    pub fn close_empty(
        mut self,
    ) -> Result<Gfx950DebugLocalReleaseCompleteV1, Gfx950DebugLocalFailureV1> {
        if let Err(error) = self
            .inner
            .until(Gfx950DebugLocalPhaseV1::LocalBackingRetired)
        {
            return Err(self.inner.failure(error));
        }
        // Sole production witness constructor, after all real fixed operations.
        let witness = DebugLocalTeardownWitnessV1 { inner: self.inner };
        match super::retention::release_after_local_empty_teardown(witness) {
            Ok(facts) => Ok(Gfx950DebugLocalReleaseCompleteV1 { facts }),
            Err((witness, error)) => Err(witness.inner.failure(error)),
        }
    }
}

/// Crate-visible solely for the queue gate's typed terminal hook. No public
/// constructor, parser, numeric identity or synthetic production mint exists.
pub(crate) struct DebugLocalTeardownWitnessV1 {
    inner: Box<Inner>,
}
impl DebugLocalTeardownWitnessV1 {
    pub(crate) fn reservation_mut(&mut self) -> &mut ProcessGlobalKfdDebugReservationV1 {
        &mut self.inner.cold._reservation
    }
    pub(super) fn into_retired_cold(self) -> Gfx950DebugColdOwnerV1 {
        // Called only after the existing typed terminal gate has succeeded.
        // Move the original cold custody out of the one allocation, then let
        // the specialized retention hook close descriptors in its old order.
        let Inner { cold, .. } = *self.inner;
        cold
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_debug_empty_layout_v1_tests.rs"]
mod layout_tests;
