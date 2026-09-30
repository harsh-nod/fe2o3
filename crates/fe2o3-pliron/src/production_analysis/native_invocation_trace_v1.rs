//! Non-default scoped native trace diagnostics over the exact current owner.
use super::pliron_analysis_manager::PlironAnalysisManagerV1 as Manager;
use super::pliron_invocation_trace::native_events_v1::{NativeAddressV1, NativeEventKindV1};
use super::pliron_invocation_trace::native_input_v1::{
    NativeTraceGeometryV1, NativeTraceInputV1, NativeTraceObligationsV1, NativeTraceRefusalV1,
};
use super::pliron_invocation_trace::native_resources_v1::{reserve_map, reserve_rows};
use super::pliron_invocation_trace::{
    PlironTraceEventV1 as Event, PlironTraceFailureV1 as TraceFailure,
};
use super::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1 as Census, ProductionAnalysisResourceContractV1 as Contract,
    ProductionAnalysisResourceLimitsV1 as Limits, ProductionAnalysisResourcePhaseV1 as Phase,
    ProductionAnalysisResourceUpperBoundV1 as Bound,
};
use crate::kir_bridge_v1::canonical_trace_v1::{
    NativeCanonicalTraceProjectionV1 as Projection, NativeSubjectV1,
};
use fe2o3_kernel_analysis::CheckedCanonicalRankedViewV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    cell::Cell,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

pub type CanonicalInvocationTraceFailureV1 = super::CanonicalRankedPolicyFailureV1;
type Failure = CanonicalInvocationTraceFailureV1;

#[derive(Debug)]
pub struct CanonicalInvocationTraceErrorV1 {
    pub failure: Failure,
}

impl std::fmt::Display for CanonicalInvocationTraceErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.failure, f)
    }
}

impl std::error::Error for CanonicalInvocationTraceErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalInvocationTraceAttemptV1 {
    Complete {
        invocations: usize,
        events: usize,
    },
    Unsupported {
        block: usize,
        operation: usize,
        reason: NativeTraceRefusalV1,
    },
    UnresolvedControl {
        block: usize,
    },
    ResourceDenied,
    UnavailableGeometry,
}

/// Family coverage for the complete static roster, including unexecuted blocks.
/// `Supported` does not establish finite scalar values or trace completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalNativeFunctionCensusV1 {
    Declaration,
    Supported {
        operations: usize,
    },
    Unsupported {
        block: usize,
        operation: usize,
        reason: NativeTraceRefusalV1,
    },
}

#[derive(Clone, Copy)]
pub enum CanonicalNativeSubjectV1<'g> {
    Operation(&'g ::fe2o3_kernel_ir::Operation),
    Terminator(&'g fe2o3_kernel_ir::Terminator),
}

pub struct CanonicalNativeEventV1<'g> {
    pub block: usize,
    pub operation: usize,
    pub subject: CanonicalNativeSubjectV1<'g>,
    pub kind: Option<NativeEventKindV1>,
    pub address: Option<NativeAddressV1>,
    pub synchronization_address_spaces: Option<u8>,
}

struct RootRow {
    function: usize,
    attempt: CanonicalInvocationTraceAttemptV1,
    geometry: Option<NativeTraceGeometryV1>,
    obligations: Option<NativeTraceObligationsV1>,
    manager: Option<Manager>,
    barrier_divergent: Option<bool>,
}

/// Borrowed diagnostics. Neither completion nor a barrier result grants policy
/// admission, source correspondence, runtime launch truth or fixed-nine coverage.
pub struct CheckedCanonicalInvocationTracesV1<'s, 'g> {
    projection: &'s Projection<'g>,
    rows: &'s [RootRow],
    functions: &'s [CanonicalNativeFunctionCensusV1],
    guard: &'s Guard,
}

impl<'s, 'g> CheckedCanonicalInvocationTracesV1<'s, 'g> {
    pub fn function_census(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalNativeFunctionCensusV1, Failure> {
        self.guard.query(budget)?;
        self.functions
            .get(function)
            .copied()
            .ok_or_else(|| self.guard.invalid(function))
    }
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g Owner, Failure> {
        self.guard.query(budget)?;
        Ok(self.projection.owner())
    }
    pub fn epoch(&self, budget: &mut Budget<'_>) -> Result<u64, Failure> {
        self.guard.query(budget)?;
        self.projection.check_epoch()?;
        Ok(self.projection.epoch())
    }
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.projection.functions().len())
    }
    pub fn root_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.rows.len())
    }
    pub fn operation_count(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self
            .projection
            .functions()
            .get(function)
            .ok_or_else(|| self.guard.invalid(function))?
            .occurrences
            .len())
    }
    pub fn is_declaration(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Failure> {
        self.guard.query(budget)?;
        Ok(self
            .projection
            .functions()
            .get(function)
            .ok_or_else(|| self.guard.invalid(function))?
            .pointer
            .is_none())
    }
    pub fn operation(
        &self,
        function: usize,
        occurrence: usize,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalNativeSubjectV1<'g>, Failure> {
        self.guard.query(budget)?;
        let function = self
            .projection
            .functions()
            .get(function)
            .ok_or_else(|| self.guard.invalid(function))?;
        let occurrence = function
            .occurrences
            .get(occurrence)
            .ok_or_else(|| self.guard.invalid(occurrence))?;
        Ok(subject(occurrence.subject))
    }
    fn root(&self, root: usize, budget: &mut Budget<'_>) -> Result<&RootRow, Failure> {
        self.guard.query(budget)?;
        self.rows.get(root).ok_or_else(|| self.guard.invalid(root))
    }
    pub fn attempt(
        &self,
        root: usize,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalInvocationTraceAttemptV1, Failure> {
        Ok(self.root(root, budget)?.attempt)
    }
    pub fn geometry(
        &self,
        root: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<NativeTraceGeometryV1>, Failure> {
        Ok(self.root(root, budget)?.geometry)
    }
    pub fn remaining_obligations(
        &self,
        root: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<NativeTraceObligationsV1>, Failure> {
        Ok(self.root(root, budget)?.obligations)
    }
    pub fn barrier_divergent(
        &self,
        root: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<bool>, Failure> {
        Ok(self.root(root, budget)?.barrier_divergent)
    }
    pub fn invocation(
        &self,
        root: usize,
        invocation: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&[u64], Failure> {
        let row = self.root(root, budget)?;
        let traces = row
            .manager
            .as_ref()
            .and_then(|v| v.exact_trace().ok())
            .ok_or_else(|| self.guard.invalid(root))?;
        Ok(&traces
            .get(invocation)
            .ok_or_else(|| self.guard.invalid(invocation))?
            .invocation)
    }
    pub fn event_count(
        &self,
        root: usize,
        invocation: usize,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Failure> {
        let row = self.root(root, budget)?;
        let traces = row
            .manager
            .as_ref()
            .and_then(|v| v.exact_trace().ok())
            .ok_or_else(|| self.guard.invalid(root))?;
        Ok(traces
            .get(invocation)
            .ok_or_else(|| self.guard.invalid(invocation))?
            .events
            .len())
    }
    pub fn event(
        &self,
        root: usize,
        invocation: usize,
        event: usize,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalNativeEventV1<'g>, Failure> {
        let row = self.root(root, budget)?;
        let traces = row
            .manager
            .as_ref()
            .and_then(|v| v.exact_trace().ok())
            .ok_or_else(|| self.guard.invalid(root))?;
        let event = traces
            .get(invocation)
            .and_then(|v| v.events.get(event))
            .ok_or_else(|| self.guard.invalid(event))?;
        let (location, occurrence, kind, address, spaces) = match event {
            Event::NativeSubject {
                location,
                occurrence,
                kind,
                address,
            } => (*location, *occurrence, Some(*kind), *address, None),
            Event::NativeBarrier {
                location,
                occurrence,
                address_spaces,
                ..
            }
            | Event::NativeFence {
                location,
                occurrence,
                address_spaces,
            } => (*location, *occurrence, None, None, Some(*address_spaces)),
            _ => return Err(self.guard.invalid(root)),
        };
        let native = &self.projection.functions()[row.function].occurrences[occurrence];
        Ok(CanonicalNativeEventV1 {
            block: location.block,
            operation: location.operation,
            subject: subject(native.subject),
            kind,
            address,
            synchronization_address_spaces: spaces,
        })
    }
}

fn subject(subject: NativeSubjectV1<'_>) -> CanonicalNativeSubjectV1<'_> {
    match subject {
        NativeSubjectV1::Operation(value) => CanonicalNativeSubjectV1::Operation(value),
        NativeSubjectV1::Terminator(value) => CanonicalNativeSubjectV1::Terminator(value),
    }
}

fn attempt(error: TraceFailure) -> CanonicalInvocationTraceAttemptV1 {
    match error {
        TraceFailure::Native {
            block,
            operation,
            reason,
        } => CanonicalInvocationTraceAttemptV1::Unsupported {
            block,
            operation,
            reason,
        },
        TraceFailure::UnresolvedBranch { block } | TraceFailure::CyclicControlFlow { block } => {
            CanonicalInvocationTraceAttemptV1::UnresolvedControl { block }
        }
        TraceFailure::ResourceLimit | TraceFailure::NativeResource(_) => {
            CanonicalInvocationTraceAttemptV1::ResourceDenied
        }
        _ => CanonicalInvocationTraceAttemptV1::UnavailableGeometry,
    }
}

/// One explicit diagnostic consumer. Defaults and policy admission do not call it.
pub fn with_canonical_invocation_traces_v1<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    budget: &mut Budget<'w>,
    callback: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalInvocationTracesV1<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalInvocationTraceErrorV1> {
    protected(budget, |budget| {
        let owner = checked.inventory(budget)?.owner();
        let headers = size_of::<Guard>()
            .checked_add(size_of::<CheckedCanonicalInvocationTracesV1<'_, '_>>())
            .and_then(|v| v.checked_add(size_of::<std::thread::Result<Result<T, Failure>>>()))
            .and_then(|v| v.checked_add(size_of::<std::thread::Result<()>>()))
            .and_then(|v| v.checked_add(size_of::<Contract>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(headers)?;
        let mut projection = Projection::import(owner, budget)?;
        let mut rows = reserve_rows(owner.module().kernels.len(), budget)?;
        let mut functions = reserve_rows(owner.module().functions.len(), budget)?;
        for function in projection.functions() {
            budget.charge_work(1)?;
            let mut status = if function.pointer.is_some() {
                CanonicalNativeFunctionCensusV1::Supported {
                    operations: function.occurrences.len(),
                }
            } else {
                CanonicalNativeFunctionCensusV1::Declaration
            };
            for occurrence in &function.occurrences {
                budget.charge_work(1)?;
                if let Err(reason) =
                    super::pliron_invocation_trace::native_input_v1::classify(occurrence.subject)
                    && matches!(status, CanonicalNativeFunctionCensusV1::Supported { .. })
                {
                    status = CanonicalNativeFunctionCensusV1::Unsupported {
                        block: occurrence.block,
                        operation: occurrence.operation,
                        reason,
                    };
                }
            }
            functions.push(status);
        }
        let mut function_ids = reserve_map(owner.module().functions.len(), budget)?;
        for (ordinal, function) in owner.module().functions.iter().enumerate() {
            budget.charge_work(
                1_usize
                    .checked_add(function.id.as_str().len())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if function_ids.insert(&function.id, ordinal).is_some() {
                return Err(Failure::ExactGraph);
            }
        }
        let mut resources = Contract::new(Limits::production_hard_ceiling());
        for (root, kernel) in owner.module().kernels.iter().enumerate() {
            budget.charge_work(
                1_usize
                    .checked_add(kernel.entry.as_str().len())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let function = *function_ids.get(&kernel.entry).ok_or(Failure::ExactGraph)?;
            let row = projection.with_function(
                function,
                budget,
                |context, native_function, correspondence, budget| {
                    let mut row = RootRow {
                        function,
                        attempt: CanonicalInvocationTraceAttemptV1::UnavailableGeometry,
                        geometry: None,
                        obligations: None,
                        manager: None,
                        barrier_divergent: None,
                    };
                    let input = match NativeTraceInputV1::derive(
                        owner,
                        context,
                        correspondence,
                        root,
                        projection.epoch(),
                        budget,
                    ) {
                        Ok(input) => input,
                        Err(TraceFailure::NativeResource(error)) => return Err(error.into()),
                        Err(error) => {
                            row.attempt = attempt(error);
                            return Ok(row);
                        }
                    };
                    row.geometry = Some(input.geometry);
                    row.obligations = Some(input.obligations);
                    let mut census = Census {
                        blocks: correspondence.blocks.len(),
                        operations: correspondence.occurrences.len(),
                        ..Census::default()
                    };
                    for block in &correspondence.blocks {
                        budget.charge_work(1)?;
                        census.block_arguments = census
                            .block_arguments
                            .checked_add(block.deref(context).get_num_arguments())
                            .ok_or(Resource::Arithmetic)?;
                    }
                    for occurrence in &correspondence.occurrences {
                        budget.charge_work(1)?;
                        let raw = occurrence.pointer.deref(context);
                        census.operands = census
                            .operands
                            .checked_add(raw.get_num_operands())
                            .ok_or(Resource::Arithmetic)?;
                        census.results = census
                            .results
                            .checked_add(raw.get_num_results())
                            .ok_or(Resource::Arithmetic)?;
                        census.successors = census
                            .successors
                            .checked_add(raw.get_num_successors())
                            .ok_or(Resource::Arithmetic)?;
                        census.attributes = census
                            .attributes
                            .checked_add(raw.attributes.0.len())
                            .ok_or(Resource::Arithmetic)?;
                        census.max_operation_arity =
                            census.max_operation_arity.max(raw.get_num_operands());
                    }
                    let mut manager = Manager::new_with_resource_contract(
                        native_function,
                        census,
                        Bound::default(),
                        0,
                        resources.remaining(Phase::InvocationTrace)?,
                    )?;
                    let prepared = manager.prepare_native_exact_trace_v1(
                        context,
                        native_function,
                        &input,
                        budget,
                    );
                    resources
                        .admit_retained(Phase::InvocationTrace, manager.resource_upper_bound())?;
                    row.attempt = match prepared {
                        Err(TraceFailure::NativeResource(error)) => return Err(error.into()),
                        Err(error) => attempt(error),
                        Ok(()) => match manager.exact_trace() {
                            Err(TraceFailure::NativeResource(error)) => return Err(error.into()),
                            Err(error) => attempt(error),
                            Ok(traces) => {
                                let mut events = 0_usize;
                                for trace in traces {
                                    budget.charge_work(1)?;
                                    events = events
                                        .checked_add(trace.events.len())
                                        .ok_or(Resource::Arithmetic)?;
                                }
                                // The existing convergence engine compares real arrivals once
                                // per synchronization occurrence, preserving the entire mask.
                                let floor = budget.storage();
                                let visits = traces
                                    .len()
                                    .checked_mul(events)
                                    .and_then(|v| v.checked_mul(4))
                                    .and_then(|v| v.checked_add(traces.len().checked_mul(2)?))
                                    .and_then(|v| v.checked_add(events.checked_mul(2)?))
                                    .ok_or(Resource::Arithmetic)?;
                                budget.charge_work(visits)?;
                                let scratch = events
                                    .checked_mul(128)
                                    .and_then(|v| v.checked_add(traces.len().checked_mul(128)?))
                                    .and_then(|v| v.checked_add(384))
                                    .ok_or(Resource::Arithmetic)?;
                                budget.reserve_storage(scratch)?;
                                let finding =
                                    super::pliron_barrier::native_barrier_participation_v1(traces);
                                row.barrier_divergent = Some(finding.is_some());
                                drop(finding);
                                budget.release_storage(budget.storage() - floor)?;
                                CanonicalInvocationTraceAttemptV1::Complete {
                                    invocations: traces.len(),
                                    events,
                                }
                            }
                        },
                    };
                    row.manager = Some(manager);
                    Ok(row)
                },
            )?;
            rows.push(row);
        }
        projection.check(budget)?;
        let guard = Guard::new(budget);
        let view = CheckedCanonicalInvocationTracesV1 {
            projection: &projection,
            rows: &rows,
            functions: &functions,
            guard: &guard,
        };
        let result = guard.callback(budget, |budget| callback(&view, budget));
        if let Err(error) = projection.check_epoch() {
            discard(result);
            return Err(error);
        }
        drop(rows);
        drop(functions);
        drop(function_ids);
        drop(projection);
        result
    })
    .map_err(|failure| CanonicalInvocationTraceErrorV1 { failure })
}

#[derive(Clone, Copy)]
enum QueryFailure {
    Resource(Resource),
    Invalid(usize),
}
impl QueryFailure {
    fn error(self) -> Failure {
        match self {
            Self::Resource(error) => Failure::Resource(error),
            Self::Invalid(function) => Failure::InvalidQuery { function },
        }
    }
}

struct Guard {
    slot: usize,
    ledger: Ledger,
    floor: usize,
    first: Cell<Option<QueryFailure>>,
}
impl Guard {
    fn new(budget: &Budget<'_>) -> Self {
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
        self.first.get().unwrap().error()
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
    fn query(&self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        self.check(budget)?;
        budget
            .charge_work(1)
            .map_err(|error| self.fail(QueryFailure::Resource(error)))
    }
    fn invalid(&self, ordinal: usize) -> Failure {
        self.fail(QueryFailure::Invalid(ordinal))
    }
    fn callback<'w, T>(
        &self,
        budget: &mut Budget<'w>,
        run: impl FnOnce(&mut Budget<'w>) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
        let post = self.check(budget).and_then(|()| {
            if budget.storage() == self.floor {
                Ok(())
            } else {
                Err(self.fail(QueryFailure::Resource(Resource::Accounting)))
            }
        });
        if let Err(error) = post {
            discard(result);
            return Err(error);
        }
        match result {
            Ok(result) => result,
            Err(payload) => {
                discard(payload);
                Err(Failure::Panicked)
            }
        }
    }
}

fn discard<T>(value: T) {
    let mut result = catch_unwind(AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = result {
        result = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
}

fn protected<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result<T, Failure>,
) -> Result<T, Failure> {
    budget.charge_work(2)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let valid = |budget: &Budget<'_>| {
        ledger == budget.work_ledger_identity_v1()
            && slot == std::ptr::from_ref(budget) as usize
            && budget.storage() >= floor
    };
    if !valid(budget) {
        discard(result);
        return Err(Resource::Accounting.into());
    }
    let value = match result {
        Ok(value) => value,
        Err(payload) => {
            discard(payload);
            Err(Failure::Panicked)
        }
    };
    if !valid(budget) {
        discard(value);
        return Err(Resource::Accounting.into());
    }
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        discard(value);
        return Err(error.into());
    }
    value
}

#[cfg(test)]
#[path = "native_invocation_trace_consumers_v1_tests.rs"]
mod consumer_tests;
#[cfg(test)]
#[path = "native_invocation_trace_differential_v1_tests.rs"]
mod differential_tests;
#[cfg(test)]
#[path = "native_invocation_trace_hostile_v1_tests.rs"]
mod hostile_tests;
#[cfg(test)]
#[path = "native_invocation_trace_resources_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "native_invocation_trace_v1_tests.rs"]
mod tests;
