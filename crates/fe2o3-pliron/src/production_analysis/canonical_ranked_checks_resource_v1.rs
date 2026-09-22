//! Two existing resource domains, without changing their units or admission.
use super::*;
use pliron::{builtin::ops::FuncOp, context::Context};

pub(super) fn checked_add(left: usize, right: usize) -> Result<usize, Failure> {
    left.checked_add(right).ok_or(Resource::Arithmetic.into())
}

pub(super) fn reserve_rows<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, Failure> {
    let payload = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(1)?;
    budget.reserve_storage(checked_add(size_of::<Vec<T>>(), payload)?)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        rows.capacity()
            .checked_sub(count)
            .and_then(|n| n.checked_mul(size_of::<T>()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(rows)
}

#[derive(Clone, Copy)]
enum QueryFailure {
    Resource(Resource),
    Invalid(usize),
}
impl QueryFailure {
    fn error(self) -> Failure {
        match self {
            Self::Resource(e) => Failure::Resource(e),
            Self::Invalid(function) => Failure::InvalidQuery { function },
        }
    }
}

pub(super) struct Guard {
    slot: usize,
    ledger: Ledger,
    floor: usize,
    first: Cell<Option<QueryFailure>>,
}
impl Guard {
    pub(super) fn new(budget: &Budget<'_>) -> Self {
        Self {
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            first: Cell::new(None),
        }
    }
    fn fail(&self, error: QueryFailure) -> Failure {
        if self.first.get().is_none() {
            self.first.set(Some(error));
        }
        self.first.get().expect("first error installed").error()
    }
    fn check(&self, budget: &Budget<'_>) -> Result<(), Failure> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(self.fail(QueryFailure::Resource(Resource::Accounting)));
        }
        self.first.get().map_or(Ok(()), |error| Err(error.error()))
    }
    pub(super) fn query(&self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        self.check(budget)?;
        budget
            .charge_work(1)
            .map_err(|e| self.fail(QueryFailure::Resource(e)))
    }
    pub(super) fn invalid(&self, ordinal: usize) -> Failure {
        self.fail(QueryFailure::Invalid(ordinal))
    }
    pub(super) fn callback<'w, T>(
        &self,
        budget: &mut Budget<'w>,
        run: impl FnOnce(&mut Budget<'w>) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let returned = catch_unwind(AssertUnwindSafe(|| run(budget)));
        let postcheck = self.check(budget).and_then(|()| {
            if budget.storage() != self.floor {
                Err(self.fail(QueryFailure::Resource(Resource::Accounting)))
            } else {
                Ok(())
            }
        });
        if let Err(error) = postcheck {
            // Rejected callback owners and payloads die while the native graph,
            // reports and their full floor still live. No aliased budget probe.
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(returned))) {
                drop(payload);
            }
            return Err(error);
        }
        match returned {
            Ok(value) => value,
            Err(payload) => {
                drop(payload);
                Err(Failure::Panicked)
            }
        }
    }
}

pub(super) fn protected<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result<T, Failure>,
) -> Result<T, Failure> {
    budget.charge_work(2)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    if ledger != budget.work_ledger_identity_v1()
        || slot != std::ptr::from_ref(&*budget) as usize
        || budget.storage() < floor
    {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    // A panic payload may own caller state. Destroy it before restoring credit,
    // including when its own destructor panics; the enclosing owner retains its
    // established unwind contract instead of silently refunding during unwind.
    let value = match result {
        Ok(value) => value,
        Err(payload) => {
            drop(payload);
            Err(Failure::Panicked)
        }
    };
    budget.release_storage(budget.storage() - floor)?;
    value
}

fn snapshot(
    bound: Bound,
    observation: InvocationObservationV1,
) -> CanonicalRankedPolicyResourceObservationV1 {
    CanonicalRankedPolicyResourceObservationV1 {
        work: bound.work_upper_bound(),
        retained: bound.retained_storage_upper_bound(),
        peak: bound.peak_storage_upper_bound(),
        first_denial: observation.first_denial.map(|d| (d.phase, d.resource)),
        caught_panic: observation.caught_panic,
    }
}

pub(super) struct AnalysisState {
    contract: Contract,
    limits: Limits,
    first_denial: Option<(Phase, &'static str)>,
    caught_panic: bool,
    pub(super) last: Option<CanonicalRankedPolicyHistoryV1>,
}
impl AnalysisState {
    pub(super) fn new(limits: Limits) -> Self {
        Self {
            contract: Contract::new(limits),
            limits,
            first_denial: None,
            caught_panic: false,
            last: None,
        }
    }
    pub(super) fn observation(&self) -> CanonicalRankedPolicyResourceObservationV1 {
        let mut out = snapshot(
            self.contract.cumulative(),
            InvocationObservationV1::default(),
        );
        out.first_denial = self.first_denial;
        out.caught_panic = self.caught_panic;
        out
    }
    fn denial(&mut self, error: Limit) -> Failure {
        self.first_denial
            .get_or_insert((error.phase, error.resource));
        error.into()
    }
    pub(super) fn invoke(
        &mut self,
        ordinal: usize,
        context: &Context,
        function: &FuncOp,
    ) -> Result<ProductionPlironPreloweringOutcomeV1, Failure> {
        let floor = self.contract.cumulative();
        let local = self
            .contract
            .remaining(Phase::PipelineVerification)
            .map_err(|error| self.denial(error))?;
        let mut receipt =
            InvocationReceiptV1::new(floor, self.limits).map_err(|error| self.denial(error))?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            require_production_pliron_checks_with_observation_v1(
                context,
                function,
                local,
                &mut receipt,
            )
        }));
        let observed = receipt.snapshot();
        self.last = Some(CanonicalRankedPolicyHistoryV1 {
            function: ordinal,
            floor: snapshot(floor, InvocationObservationV1::default()),
            invocation: snapshot(observed.current, observed),
        });
        if let Some(error) = observed.first_denial {
            self.first_denial
                .get_or_insert((error.phase, error.resource));
        }
        self.caught_panic |= observed.caught_panic || result.is_err();
        // Accepted prefixes commit ONCE on success/error/panic, before inspecting
        // the outcome. The outcome's bound is only an independent equality join.
        self.contract
            .admit_retained(Phase::PipelineVerification, observed.current)
            .map_err(|error| self.denial(error))?;
        let outcome = match result {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(cause)) => {
                return Err(Failure::Analysis {
                    function: ordinal,
                    cause,
                });
            }
            Err(payload) => {
                drop(payload);
                return Err(Failure::Panicked);
            }
        };
        let complete = receipt
            .complete()
            .map_err(|_| Failure::InvocationAccounting)?;
        if outcome.resource_upper_bound != complete {
            return Err(Failure::InvocationAccounting);
        }
        Ok(outcome)
    }
    pub(super) fn release_reports(&mut self) -> Result<(), Failure> {
        let retained = self.contract.cumulative().retained_storage_upper_bound();
        self.contract
            .admit_replacement(Phase::PipelineVerification, retained, Bound::default())
            .map_err(|error| self.denial(error))
    }
}

#[cfg(test)]
#[path = "canonical_ranked_checks_resources_v1_tests.rs"]
mod tests;
