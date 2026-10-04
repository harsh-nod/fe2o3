//! Separate one-shot packet-capable sibling. EmptyQueue remains packet-incapable.
//! No diagnostic bytes can construct this owner or authorize its unsafe publisher.
use super::super::{
    Gfx950DebugColdOwnerV1, Gfx950DebugColdPreparationFactsV1, Gfx950DebugQueueGeometryV1,
};
use super::{
    Gfx950DebugLocalErrorV1 as E, Gfx950DebugLocalPhaseV1 as EmptyPhase,
    Gfx950DebugLocalStepV1 as EmptyStep, Inner,
};
use crate::queue_linux::ProcessGlobalKfdDebugReservationV1;
use core::fmt;
use std::time::{Duration, Instant};

#[path = "engineering_gfx950_debug_one_stop_checkpoint_v1.rs"]
mod checkpoint;
#[path = "engineering_gfx950_debug_one_stop_contract_v1.rs"]
mod contract;
#[path = "engineering_gfx950_debug_one_stop_cursor_v1.rs"]
mod cursor;
#[path = "engineering_gfx950_debug_one_stop_native_v1.rs"]
mod native;
#[path = "engineering_gfx950_debug_one_stop_resources_v1.rs"]
mod resources;
#[path = "engineering_gfx950_debug_one_stop_retirement_v1.rs"]
mod retirement;
use Gfx950DebugOneStopStepV1 as S;
use cursor::Cursor;
pub use cursor::{Gfx950DebugOneStopPhaseV1, Gfx950DebugOneStopStepV1};

struct OneStopInner {
    // Cold remains first; its existing Drop retains every allocation and FD on
    // native ambiguity before the process reservation is poisoned.
    base: Inner,
    cursor: Cursor,
    packet: Option<fe2o3_aql::AqlPreparedKernelDispatchV1>,
    checkpoint: Option<checkpoint::Checkpoint>,
    image_sha256: [u8; 32],
    deadline: Option<Instant>,
    completed: bool,
    descriptors_closed: bool,
}
impl OneStopInner {
    fn step(&mut self) -> Result<(), E> {
        let Self {
            base,
            cursor,
            packet,
            checkpoint,
            image_sha256,
            deadline,
            completed,
            ..
        } = self;
        cursor.run(std::process::id(), |step| {
            native::execute(
                base,
                packet,
                checkpoint,
                image_sha256,
                deadline,
                completed,
                step,
            )
        })
    }
    fn until(&mut self, phase: Gfx950DebugOneStopPhaseV1) -> Result<(), E> {
        for _ in 0..64 {
            if self.cursor.phase() == phase {
                return Ok(());
            }
            self.step()?;
        }
        if self.cursor.phase() == phase {
            Ok(())
        } else {
            Err(E::Phase)
        }
    }
    fn failure(self: Box<Self>, error: E) -> Gfx950DebugOneStopFailureV1 {
        Gfx950DebugOneStopFailureV1 {
            retained: self,
            error,
        }
    }
}
/// Retained, process-local native custody. Drop is NOT teardown. There is no
/// retry, packet/address accessor, state-deserialization or cast to EmptyQueue.
pub struct Gfx950DebugOneStopFailureV1 {
    retained: Box<OneStopInner>,
    error: E,
}
impl Gfx950DebugOneStopFailureV1 {
    pub fn phase(&self) -> Gfx950DebugOneStopPhaseV1 {
        self.retained.cursor.phase()
    }
    pub fn reason(&self) -> &E {
        &self.error
    }
    /// Conservative effect possibility, never evidence of actual execution.
    pub fn packet_publication_possible(&self) -> bool {
        self.retained.cursor.publication_possible()
    }
    /// Actual local descriptor Drop completed, even if the deadline then refused.
    pub fn local_descriptors_closed(&self) -> bool {
        self.retained.descriptors_closed
    }
}
impl fmt::Debug for Gfx950DebugOneStopFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gfx950DebugOneStopFailureV1")
            .field("phase", &self.phase())
            .field("reason", &self.error)
            .field(
                "packet_publication_possible",
                &self.packet_publication_possible(),
            )
            .field("local_descriptors_closed", &self.local_descriptors_closed())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for Gfx950DebugOneStopFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "one-stop target {:?}: {:?}; descriptor_drop_completed={}, any unresolved custody retained",
            self.phase(),
            self.error,
            self.local_descriptors_closed()
        )
    }
}
impl std::error::Error for Gfx950DebugOneStopFailureV1 {}

/// Actual fixed queue/resources/packet preparation, not evidence of an attached
/// debugger, runtime ACK, TTMP readback, sampling exclusion or permission to run.
///
/// No conversion from EmptyQueue or a report exists.
/// ```compile_fail
/// use fe2o3_kfd::Gfx950DebugEmptyQueueV1;
/// fn no_cast(v: Gfx950DebugEmptyQueueV1) { v.publish_at_owned_checkpoint_once(); }
/// ```
pub struct Gfx950DebugOneStopPreparedV1 {
    inner: Box<OneStopInner>,
}
/// One possible submitted packet. There is no empty-queue close, resubmission,
/// mutation, snapshot-reading or queue-rollover method.
/// ~~~compile_fail
/// use fe2o3_kfd::Gfx950DebugOneStopInFlightV1;
/// fn no_empty_close(v: Gfx950DebugOneStopInFlightV1) { v.close_empty(); }
/// ~~~
/// ~~~compile_fail
/// use fe2o3_kfd::Gfx950DebugOneStopInFlightV1;
/// fn require_send<T: Send>() {}
/// require_send::<Gfx950DebugOneStopInFlightV1>();
/// ~~~
/// ~~~compile_fail
/// use fe2o3_kfd::Gfx950DebugOneStopInFlightV1;
/// fn duplicate(v: Gfx950DebugOneStopInFlightV1) { let _ = v.clone(); }
/// ~~~
pub struct Gfx950DebugOneStopInFlightV1 {
    inner: Box<OneStopInner>,
}
/// Historical actual acquire-completion + local native teardown. This does NOT
/// prove a GPU debug stop, a same-client ACK, a physical sample, whole-family
/// cleanup, or release of any external supervisor obligation.
#[derive(Debug)]
pub struct Gfx950DebugOneStopLocalCompleteV1 {
    facts: Gfx950DebugColdPreparationFactsV1,
}
impl Gfx950DebugOneStopLocalCompleteV1 {
    pub fn preparation_facts(&self) -> Gfx950DebugColdPreparationFactsV1 {
        self.facts
    }
}
pub(in super::super) fn begin(
    cold: Gfx950DebugColdOwnerV1,
    geometry: Gfx950DebugQueueGeometryV1,
    closure: [u8; 32],
) -> Result<Gfx950DebugOneStopPreparedV1, Gfx950DebugOneStopFailureV1> {
    // The only Box allocation is BEFORE Revalidate/RegisterRuntime, and this
    // same box moves through prepared, in-flight, failure and terminal custody.
    let mut inner = Box::new(OneStopInner {
        base: Inner {
            cold,
            geometry,
            closure,
            cursor: super::cursor::Cursor::new(std::process::id()),
            queue_call: None,
            event_destroyed: None,
        },
        cursor: Cursor::new(std::process::id()),
        packet: None,
        checkpoint: None,
        image_sha256: [0; 32],
        deadline: None,
        completed: false,
        descriptors_closed: false,
    });
    if let Err(error) = inner.until(Gfx950DebugOneStopPhaseV1::Ready(S::OwnedCheckpoint)) {
        return Err(inner.failure(error));
    }
    Ok(Gfx950DebugOneStopPreparedV1 { inner })
}
impl Gfx950DebugOneStopPreparedV1 {
    /// Reach the fixed source-bound host rendezvous, then consume the actual
    /// prepared packet exactly once. No Boolean, JSON, caller address or script
    /// supplies permission. The checkpoint does NOT enforce debugger facts.
    ///
    /// # Safety
    ///
    /// The caller must retain the original isolated disposable-process and
    /// no-foreign-runtime/injection/concurrent-native-actor contract. Before the
    /// first publication effect, a separately qualified SAME native debugger
    /// client must stop this exact owned process at the fixed checkpoint, join
    /// its actual source/executable/PC and retained object/queue/packet/metadata
    /// lifetimes, prove actual attach-before-runtime, LoadedSuccess, the sole
    /// successful event_processed ACK, reviewed trap/CWSR/TTMP setup and live
    /// sampling exclusion, then consume its exact pre-resume gate once. Merely
    /// calling this function or observing checkpoint bytes proves none of this.
    ///
    /// That client/supervisor must remain in control through the actual unique
    /// DEBUG_TRAP stop, separately qualified completion-only resume and local
    /// retirement. No next-MI checkpoint exception is granted. The supervisor
    /// must bound blocked native calls and terminate/reap the owned family on
    /// any ambiguity. This native path is not yet qualified by CPU/static tests.
    ///
    /// ```compile_fail
    /// use fe2o3_kfd::Gfx950DebugOneStopPreparedV1;
    /// fn no_safe_release(v: Gfx950DebugOneStopPreparedV1) {
    ///     let _ = v.publish_at_owned_checkpoint_once();
    /// }
    /// ```
    pub unsafe fn publish_at_owned_checkpoint_once(
        mut self,
    ) -> Result<Gfx950DebugOneStopInFlightV1, Gfx950DebugOneStopFailureV1> {
        // One original inclusive deadline through checkpoint, publication,
        // completion and local teardown. It is never restarted after a stop.
        self.inner.deadline = Instant::now().checked_add(Duration::from_secs(60));
        let result = (|| {
            if self.inner.deadline.is_none() {
                return Err(E::Contract("one-stop deadline overflow"));
            }
            self.inner
                .until(Gfx950DebugOneStopPhaseV1::Ready(S::WriteBody))?;
            let packet = self.inner.packet.take().ok_or(E::Phase)?;
            packet.publish_with(&mut native::Publication {
                inner: &mut self.inner,
            })?;
            self.inner.step()?;
            if self.inner.cursor.phase() != Gfx950DebugOneStopPhaseV1::Ready(S::ObserveCompletion) {
                return Err(E::Phase);
            }
            native::check_deadline(self.inner.deadline)?;
            Ok(())
        })();
        match result {
            Ok(()) => Ok(Gfx950DebugOneStopInFlightV1 { inner: self.inner }),
            Err(error) => Err(self.inner.failure(error)),
        }
    }
}
impl Gfx950DebugOneStopInFlightV1 {
    /// Acquire actual completion, require write/read 1/1 and exact output, then
    /// perform the distinct nine-allocation local retirement. No native retry.
    pub fn complete_and_close(
        mut self,
    ) -> Result<Gfx950DebugOneStopLocalCompleteV1, Gfx950DebugOneStopFailureV1> {
        if let Err(error) = self
            .inner
            .until(Gfx950DebugOneStopPhaseV1::LocalBackingRetired)
        {
            return Err(self.inner.failure(error));
        }
        let witness = DebugOneStopTeardownWitnessV1 { inner: self.inner };
        match super::super::retention::release_after_local_one_stop_teardown(witness) {
            Ok(facts) => Ok(Gfx950DebugOneStopLocalCompleteV1 { facts }),
            Err((witness, error)) => Err(witness.inner.failure(error)),
        }
    }
}
/// Constructor is private to the completed target path; never minted from a
/// claimed phase, address, JSON, debugger observation or empty-queue witness.
pub(crate) struct DebugOneStopTeardownWitnessV1 {
    inner: Box<OneStopInner>,
}
impl DebugOneStopTeardownWitnessV1 {
    pub(crate) fn reservation_mut(&mut self) -> &mut ProcessGlobalKfdDebugReservationV1 {
        &mut self.inner.base.cold._reservation
    }
    pub(in super::super) fn check_terminal(&self) -> Result<(), E> {
        if self.inner.cursor.phase() != Gfx950DebugOneStopPhaseV1::LocalBackingRetired
            || !self.inner.completed
            || !self.inner.cursor.publication_possible()
        {
            return Err(E::Phase);
        }
        native::check_deadline(self.inner.deadline)
    }
    pub(in super::super) fn cold_mut_after_terminal(&mut self) -> &mut Gfx950DebugColdOwnerV1 {
        &mut self.inner.base.cold
    }
    pub(in super::super) fn mark_descriptors_closed(&mut self) {
        self.inner.descriptors_closed = true;
    }
}
#[cfg(test)]
#[path = "engineering_gfx950_debug_one_stop_owner_v1_tests.rs"]
mod tests;
