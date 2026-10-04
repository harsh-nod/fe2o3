//! Private phase-scoped accounting; a receipt is not compiler or proof authority.

use std::cell::Cell;

use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisResourceLimitV1 as Limit, ProductionAnalysisResourceLimitsV1 as Limits,
    ProductionAnalysisResourcePhaseV1 as Phase, ProductionAnalysisResourceUpperBoundV1 as Bound,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct InvocationObservationV1 {
    pub(crate) committed: Bound,
    pub(crate) current: Bound,
    pub(crate) first_denial: Option<Limit>,
    pub(crate) caught_panic: bool,
}

pub(crate) struct InvocationReceiptV1<'budget> {
    state: Cell<InvocationObservationV1>,
    floor: Bound,
    limits: Limits,
    admission: Option<&'budget dyn Fn(Phase, Bound) -> Result<(), Limit>>,
}

pub(crate) struct InvocationPhaseV1<'r> {
    receipt: &'r InvocationReceiptV1<'r>,
    anchor: Bound,
    replaced: usize,
    owner: Phase,
    observed: Cell<bool>,
    closed: bool,
}

pub(crate) struct InvocationObserverV1<'p, 'r> {
    phase: &'p InvocationPhaseV1<'r>,
    // Complete phase-relative prefix, excluding the frozen anchor and floor.
    project: &'p dyn Fn(Bound) -> Result<Bound, Limit>,
}

// The caller supplies a frozen producer prefix; this cell tracks only real
// sequential admissions made inside that producer, never manager snapshots.
pub(crate) type AdditionalObservationV1<'o, 'p, 'r> =
    Option<(&'o InvocationObserverV1<'p, 'r>, &'o Cell<Bound>)>;

pub(crate) fn observe_additional_admission_v1(
    observation: AdditionalObservationV1<'_, '_, '_>,
    limits: Limits,
    phase: Phase,
    local: Result<Bound, Limit>,
) -> Result<Bound, Limit> {
    let Some((observer, admitted)) = observation else {
        return local.and_then(|bound| limits.require(phase, bound));
    };
    let previous = admitted.get();
    observer.with_projection(
        &|bound| previous.checked_then_retain(bound, phase),
        |nested| {
            let bound = nested.require(limits, phase, local)?;
            let complete = previous
                .checked_then_retain(bound, phase)
                .map_err(|error| observer.deny(error))?;
            admitted.set(complete);
            Ok(bound)
        },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InvocationReceiptFailureV1 {
    Denied(Limit),
    CaughtPanic,
}

impl<'budget> InvocationReceiptV1<'budget> {
    pub(crate) fn new(floor: Bound, limits: Limits) -> Result<Self, Limit> {
        limits.require(Phase::PipelineVerification, floor)?;
        Ok(Self {
            state: Cell::new(InvocationObservationV1::default()),
            floor,
            limits,
            admission: None,
        })
    }

    /// The hook admits each complete invocation prefix before the producer runs.
    /// It may debit an enclosing ledger, but cannot replace observed history.
    pub(crate) fn with_admission(
        floor: Bound,
        limits: Limits,
        admission: &'budget dyn Fn(Phase, Bound) -> Result<(), Limit>,
    ) -> Result<Self, Limit> {
        let mut receipt = Self::new(floor, limits)?;
        receipt.admission = Some(admission);
        Ok(receipt)
    }

    pub(crate) fn snapshot(&self) -> InvocationObservationV1 {
        self.state.get()
    }

    fn deny(&self, error: Limit) -> Limit {
        let mut state = self.state.get();
        state.first_denial.get_or_insert(error);
        self.state.set(state);
        error
    }

    fn watch<T>(&self, result: Result<T, Limit>) -> Result<T, Limit> {
        result.map_err(|error| self.deny(error))
    }

    pub(crate) fn caught(&self) {
        let mut state = self.state.get();
        state.caught_panic = true;
        self.state.set(state);
    }

    pub(crate) fn observe_unwind<T>(&mut self, run: impl FnOnce(&mut Self) -> T) -> T {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(self))) {
            Ok(value) => value,
            Err(payload) => {
                self.caught();
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(crate) fn phase(
        &mut self,
        owner: Phase,
        replaced: usize,
    ) -> Result<InvocationPhaseV1<'_>, Limit> {
        // A replacement's local peak must include the outgoing owner's live
        // overlap; the caller drops that owner before committing its successor.
        let anchor = self.state.get().committed;
        if replaced > anchor.retained_storage_upper_bound() {
            return Err(self.deny(Limit {
                phase: owner,
                resource: "observed replacement storage",
            }));
        }
        Ok(InvocationPhaseV1 {
            receipt: self,
            anchor,
            replaced,
            owner,
            observed: Cell::new(false),
            closed: false,
        })
    }

    pub(crate) fn complete(&self) -> Result<Bound, InvocationReceiptFailureV1> {
        let state = self.state.get();
        if let Some(error) = state.first_denial {
            return Err(InvocationReceiptFailureV1::Denied(error));
        }
        if state.caught_panic {
            return Err(InvocationReceiptFailureV1::CaughtPanic);
        }
        Ok(state.committed)
    }

    // The caller binds the reservation to this exact owner. No retained credit
    // is released until its destructor returns; work and historical peak stay.
    pub(crate) fn drop_owner<T>(
        &mut self,
        owner: Phase,
        value: T,
        retained: usize,
    ) -> Result<Bound, Limit> {
        let state = self.state.get();
        let released = self.watch(state.committed.checked_then_replace_retained(
            retained,
            Bound::default(),
            owner,
        ));
        let panic = InvocationPanicGuardV1(self);
        drop(value);
        let released = released?;
        let mut state = self.state.get();
        state.committed = released;
        state.current = released;
        self.state.set(state);
        drop(panic);
        Ok(released)
    }
}

impl<'r> InvocationPhaseV1<'r> {
    pub(crate) fn observer<'p>(
        &'p self,
        project: &'p dyn Fn(Bound) -> Result<Bound, Limit>,
    ) -> InvocationObserverV1<'p, 'r> {
        InvocationObserverV1 {
            phase: self,
            project,
        }
    }

    fn observe(&self, leaf: Phase, local: Bound) -> Result<(), Limit> {
        let absolute = self
            .receipt
            .watch(
                self.anchor
                    .checked_then_replace_retained(self.replaced, local, leaf),
            )?;
        let mut state = self.receipt.state.get();
        let peak = state
            .current
            .peak_storage_upper_bound()
            .max(absolute.peak_storage_upper_bound());
        let held = self.receipt.watch(Bound::checked_phase(
            leaf,
            state
                .current
                .work_upper_bound()
                .max(absolute.work_upper_bound()),
            peak,
            0,
        ))?;
        self.receipt.watch(
            self.receipt
                .floor
                .checked_then_retain(held, leaf)
                .and_then(|total| self.receipt.limits.require(leaf, total)),
        )?;
        if let Some(admission) = self.receipt.admission {
            self.receipt.watch(admission(leaf, held))?;
        }
        state.current = held;
        self.receipt.state.set(state);
        self.observed.set(true);
        Ok(())
    }

    pub(crate) fn commit(mut self, output: Bound) -> Result<Bound, Limit> {
        let output = self
            .receipt
            .watch(
                self.anchor
                    .checked_then_replace_retained(self.replaced, output, self.owner),
            )?;
        let mut state = self.receipt.state.get();
        if !self.observed.get()
            || output.work_upper_bound() > state.current.work_upper_bound()
            || output.peak_storage_upper_bound() > state.current.peak_storage_upper_bound()
            || output.retained_storage_upper_bound() > state.current.retained_storage_upper_bound()
        {
            return Err(self.receipt.deny(Limit {
                phase: self.owner,
                resource: "unobserved resource transfer",
            }));
        }
        let settled = self.receipt.watch(Bound::checked_phase(
            self.owner,
            state.current.work_upper_bound(),
            output.retained_storage_upper_bound(),
            state.current.peak_storage_upper_bound() - output.retained_storage_upper_bound(),
        ))?;
        state.current = settled;
        state.committed = settled;
        self.receipt.state.set(state);
        self.closed = true;
        Ok(settled)
    }
}

impl Drop for InvocationPhaseV1<'_> {
    fn drop(&mut self) {
        if !self.closed {
            let mut state = self.receipt.state.get();
            state.committed = state.current;
            state.caught_panic |= std::thread::panicking();
            self.receipt.state.set(state);
        }
    }
}

impl InvocationObserverV1<'_, '_> {
    pub(crate) fn with_projection<T>(
        &self,
        project: &dyn Fn(Bound) -> Result<Bound, Limit>,
        execute: impl FnOnce(&InvocationObserverV1<'_, '_>) -> T,
    ) -> T {
        let _panic = InvocationPanicGuardV1(self.phase.receipt);
        let composed = |local| (self.project)(project(local)?);
        execute(&InvocationObserverV1 {
            phase: self.phase,
            project: &composed,
        })
    }

    pub(crate) fn require(
        &self,
        limits: Limits,
        leaf: Phase,
        local: Result<Bound, Limit>,
    ) -> Result<Bound, Limit> {
        let _panic = InvocationPanicGuardV1(self.phase.receipt);
        let bound = self
            .phase
            .receipt
            .watch(local.and_then(|bound| limits.require(leaf, bound)))?;
        let projected = self.phase.receipt.watch((self.project)(bound))?;
        self.phase.observe(leaf, projected)?;
        Ok(bound)
    }

    pub(crate) fn deny(&self, error: Limit) -> Limit {
        self.phase.receipt.deny(error)
    }

    pub(crate) fn caught(&self) {
        self.phase.receipt.caught();
    }
}

pub(crate) fn require_observed_v1(
    limits: Limits,
    phase: Phase,
    bound: Result<Bound, Limit>,
    observer: Option<&InvocationObserverV1<'_, '_>>,
) -> Result<Bound, Limit> {
    match observer {
        Some(observer) => observer.require(limits, phase, bound),
        None => bound.and_then(|bound| limits.require(phase, bound)),
    }
}

pub(crate) fn observe_resource_preflight_v1<T>(
    observer: Option<&InvocationObserverV1<'_, '_>>,
    run: impl FnOnce(Option<&InvocationObserverV1<'_, '_>>) -> Result<T, Limit>,
) -> Result<T, Limit> {
    let result = match observer {
        None => run(None),
        Some(observer) => observer.with_projection(&Ok, |nested| run(Some(nested))),
    };
    if let (Some(observer), Err(error)) = (observer, &result) {
        observer.deny(*error);
    }
    result
}

struct InvocationPanicGuardV1<'a>(&'a InvocationReceiptV1<'a>);

impl Drop for InvocationPanicGuardV1<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.caught();
        }
    }
}

#[cfg(test)]
#[path = "invocation_receipt_v1_tests.rs"]
mod tests;
