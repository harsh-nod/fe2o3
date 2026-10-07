//! Keep recycled dispatch data outside the model loan until ledger settlement.

use super::*;
use crate::queue::dispatch_binding::ReturnedDispatchDataLeaseV1;
use crate::queue::dispatch_binding::control_release::{
    ReturningControlCleanupCustodyV1, ReturningControlModeV1,
};
use crate::queue::dispatch_binding::pristine_abort::PristineControlReleaseV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

type DetachEnvelopeV1 =
    Result<((), Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1>;

pub(in crate::queue) struct RecycledDetachLedgerV1<'a> {
    pub(in crate::queue) generation: &'a mut Option<u64>,
    pub(in crate::queue) count: &'a mut usize,
    pub(in crate::queue) identities: &'a mut Vec<Gfx942FixedDispatchStorageIdentityV1>,
    pub(in crate::queue) next: &'a mut Option<usize>,
}

impl RecycledDetachLedgerV1<'_> {
    fn is_empty(&self) -> bool {
        self.generation.is_none()
            && *self.count == 0
            && self.identities.is_empty()
            && self.next.is_none()
    }
}

pub(in crate::queue) trait RecycledDetachContextV1 {
    type Memory: PristineControlReleaseV1;

    fn require_detachable(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn require_completed(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn dispatch(&mut self) -> &mut Option<DispatchResourceOwnerV1>;
    fn ledger(&mut self) -> RecycledDetachLedgerV1<'_>;
    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Self::Memory),
    ) -> DetachEnvelopeV1;
    fn retain(&mut self, root: RecycledDetachCustodyV1);
    fn poison(&mut self, panicked: bool);

    #[cfg(test)]
    fn output_capacities(&self, count: usize) -> (usize, usize) {
        (count, count)
    }
}

pub(in crate::queue) struct RecycledDetachCustodyV1 {
    pub(in crate::queue) cleanup: Option<ReturningControlCleanupCustodyV1>,
    returned: Option<ReturnedDispatchDataV1>,
    remaining: std::vec::IntoIter<ReturnedDispatchDataLeaseV1>,
    pub(in crate::queue) data: Vec<Gfx942FixedDispatchDataV1>,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
}

impl RecycledDetachCustodyV1 {
    fn new() -> Self {
        Self {
            cleanup: None,
            returned: None,
            remaining: Vec::new().into_iter(),
            data: Vec::new(),
            identities: Vec::new(),
        }
    }

    fn reserve_output(
        &mut self,
        count: usize,
        capacities: (usize, usize),
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let error = || Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
            operation: "detached dispatch output",
        };
        self.data
            .try_reserve_exact(capacities.0)
            .map_err(|_| error())?;
        self.identities
            .try_reserve_exact(capacities.1)
            .map_err(|_| error())?;
        if self.data.capacity() < count || self.identities.capacity() < count {
            return Err(error());
        }
        Ok(())
    }

    fn convert_completed(
        &mut self,
        generation: u64,
        count: usize,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let cleanup = self
            .cleanup
            .as_mut()
            .filter(|root| root.is_complete())
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "recycled dispatch cleanup was incomplete",
            ))?;
        self.returned = Some(cleanup.take_completed()?);
        let returned = self.returned.as_ref().expect("rooted returned data");
        if returned.generation() != generation || returned.data().len() != count {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "recycled dispatch return shape changed",
            ));
        }
        self.remaining = self
            .returned
            .take()
            .expect("validated returned data")
            .into_data()
            .into_iter();
        // Conversion only unwraps typed authorities; both pushes have reserved capacity.
        for returned in self.remaining.by_ref() {
            let data = returned.into_data();
            self.identities.push(data.storage_identity());
            self.data.push(data);
        }
        Ok(())
    }
}

pub(in crate::queue) struct SettledRecycledDetachV1 {
    pub(in crate::queue) result:
        std::thread::Result<Result<Gfx942DetachedFixedDispatchV1, ComputeAqlQueueSessionErrorV1>>,
    pub(in crate::queue) transport: bool,
}

impl SettledRecycledDetachV1 {
    pub(super) fn into_result(
        self,
    ) -> Result<Gfx942DetachedFixedDispatchV1, ComputeAqlQueueSessionErrorV1> {
        match self.result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }
}

pub(in crate::queue) fn settle_recycled_detach_v1(
    context: &mut impl RecycledDetachContextV1,
) -> SettledRecycledDetachV1 {
    let mut original = None;
    let mut root = RecycledDetachCustodyV1::new();
    let mut lower = None;
    let mut entered = false;
    let mut terminal_on_error = false;
    let result = catch_unwind(AssertUnwindSafe(|| {
        context.require_detachable()?;
        if !context.ledger().is_empty() {
            terminal_on_error = true;
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data ledger was not empty",
            ));
        }
        context.require_completed()?;
        let dispatch = context
            .dispatch()
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        terminal_on_error = true;
        let (generation, count) = dispatch.recycled_data_shape_v1()?;
        if generation == 0 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch generation was zero",
            ));
        }
        terminal_on_error = false;
        let capacities = (count, count);
        #[cfg(test)]
        let capacities = context.output_capacities(capacities.0);
        root.reserve_output(count, capacities)?;
        terminal_on_error = true;
        context.check_currentness()?;
        original = context.dispatch().take();
        let envelope = context.with_memory_custody(|memory| {
            entered = true;
            root.cleanup = Some(ReturningControlCleanupCustodyV1::new(
                original.take().expect("opened loan retains its dispatch"),
                ReturningControlModeV1::AfterRecycle,
            ));
            lower = Some(catch_unwind(AssertUnwindSafe(|| {
                root.cleanup
                    .as_mut()
                    .expect("rooted recycled dispatch")
                    .release_in_place(memory)
            })));
        });
        let ((), retake) = match envelope {
            Ok(envelope) => envelope,
            Err(error) => {
                terminal_on_error = entered;
                return Err(error);
            }
        };
        retake?;
        match lower.take() {
            Some(Ok(result)) => result?,
            Some(Err(payload)) => resume_unwind(payload),
            None => {
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "recycled dispatch callback did not execute",
                ));
            }
        }
        root.convert_completed(generation, count)?;
        let ledger = context.ledger();
        // No fallible operation follows the first ledger write.
        *ledger.identities = core::mem::take(&mut root.identities);
        *ledger.count = count;
        *ledger.generation = Some(generation);
        *ledger.next = None;
        Ok(Gfx942DetachedFixedDispatchV1 {
            generation,
            data: core::mem::take(&mut root.data),
        })
    }));
    let mut result = match lower {
        Some(Err(payload)) => {
            // Preserve the lower panic even when retake or poisoning also panics.
            core::mem::forget(result);
            Err(payload)
        }
        _ => result,
    };
    if let Some(dispatch) = original {
        *context.dispatch() = Some(dispatch);
    }
    let transport = result.is_err() || terminal_on_error && !matches!(result, Ok(Ok(_)));
    if transport {
        context.retain(root);
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| context.poison(result.is_err()))) {
            if result.is_err() {
                core::mem::forget(payload);
            } else {
                result = Err(payload);
            }
        }
    }
    SettledRecycledDetachV1 { result, transport }
}

impl RecycledDetachContextV1 for ComputeAqlQueueSessionV1 {
    type Memory = SharedGttMemorySessionV1;

    fn require_detachable(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() || !self.unpublished_dispatch.is_clear()
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        Ok(())
    }

    fn require_completed(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.completion_owner
            .ensure_releasable()
            .map_err(Into::into)
    }

    fn dispatch(&mut self) -> &mut Option<DispatchResourceOwnerV1> {
        &mut self.dispatch
    }

    fn ledger(&mut self) -> RecycledDetachLedgerV1<'_> {
        RecycledDetachLedgerV1 {
            generation: &mut self.detached_dispatch_generation,
            count: &mut self.detached_data_count,
            identities: &mut self.detached_data_identities,
            next: &mut self.detached_next_insertion_index,
        }
    }

    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session
            .check_queue_currentness()
            .map_err(Into::into)
    }

    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Self::Memory),
    ) -> DetachEnvelopeV1 {
        self.with_live_queue_memory_model_custody(operation)
    }

    fn retain(&mut self, root: RecycledDetachCustodyV1) {
        core::mem::forget(root);
    }

    fn poison(&mut self, panicked: bool) {
        self.poison_terminal();
        if panicked {
            poison_process_global_after_dispatch_terminal_v1();
        }
    }
}

impl ComputeAqlQueueSessionV1 {
    pub(super) fn detach_recycled_settled_v1(&mut self) -> SettledRecycledDetachV1 {
        settle_recycled_detach_v1(self)
    }
}
