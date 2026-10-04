//! Inert retained lexical-liveness construction for the Shared preparation path.
//! No admission, queries, generic resource callback, ordinary route or refunds.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_mir_model::semantic_mir_v1 as model;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Snapshot {
    budget_slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    counter_slot: usize,
    owned: usize,
    storage: usize,
    work: usize,
    peak: usize,
    denied_work: bool,
    denied_storage: bool,
}
fn snapshot(budget: &Budget<'_>, owned: &usize) -> Snapshot {
    Snapshot {
        budget_slot: budget as *const Budget<'_> as usize,
        ledger: budget.work_ledger_identity_v1(),
        counter_slot: owned as *const usize as usize,
        owned: *owned,
        storage: budget.storage(),
        work: budget.work(),
        peak: budget.peak_storage(),
        denied_work: budget.failed_work().is_some(),
        denied_storage: budget.failed_storage().is_some(),
    }
}
#[derive(Debug)]
pub(in super::super) enum RetainedLivenessErrorV1 {
    Resource(Resource),
    Original(Error),
}
impl From<Resource> for RetainedLivenessErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
type RetainedResult<T> = std::result::Result<T, RetainedLivenessErrorV1>;

/// The three payload owners exist before the first fallible operation.
/// The caller keeps this owner through postflight (including error/unwind),
/// drops it, and only then refunds its original coupled preparation credits.
pub(in super::super) struct RetainedSharedLivenessV1<'a> {
    phase: Phase,
    source: Option<&'a SemanticFunctionDeclV1>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    schedule: Schedule,
    ready: Vec<usize>,
    failure: Option<Resource>,
}
impl<'a> RetainedSharedLivenessV1<'a> {
    pub(in super::super) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            source: None,
            entry: None,
            held: None,
            schedule: Schedule {
                locals: Vec::new(),
                indegrees: Vec::new(),
            },
            ready: Vec::new(),
            failure: None,
        }
    }
    pub(in super::super) fn prepare_into(
        &mut self,
        function: &'a SemanticFunctionDeclV1,
        budget: &mut Budget<'_>,
        owned: &mut usize,
    ) -> RetainedResult<()> {
        let fresh = self.phase == Phase::Fresh;
        self.phase = Phase::Terminal;
        let entry = snapshot(budget, owned);
        if !fresh || entry.denied_work || entry.denied_storage || entry.owned > entry.storage {
            return Err(Resource::Accounting.into());
        }
        self.source = Some(function);
        self.entry = Some(entry);
        let result = {
            let mut meter = RetainedMeter {
                budget,
                owned,
                failure: &mut self.failure,
            };
            meter.accept_work(32)?;
            meter.reserve(frame()?)?;
            build_into(&mut self.schedule, &mut self.ready, function, &mut meter)
        };
        if let Err(error) = result {
            return Err(match self.failure {
                Some(resource) => RetainedLivenessErrorV1::Resource(resource),
                None => RetainedLivenessErrorV1::Original(error),
            });
        }
        self.check(function, budget, owned)?;
        self.held = Some(snapshot(budget, owned));
        self.phase = Phase::Complete;
        Ok(())
    }
    fn check(
        &self,
        function: &SemanticFunctionDeclV1,
        budget: &Budget<'_>,
        owned: &usize,
    ) -> RetainedResult<()> {
        let source = self.source.ok_or(Resource::Accounting)?;
        let before = self.entry.ok_or(Resource::Accounting)?;
        let now = snapshot(budget, owned);
        let growth = now
            .owned
            .checked_sub(before.owned)
            .ok_or(Resource::Accounting)?;
        let expected = before
            .storage
            .checked_add(growth)
            .ok_or(Resource::Arithmetic)?;
        if let Some(held) = self.held {
            if now.owned < held.owned
                || now.storage < held.storage
                || now.work < held.work
                || now.peak < held.peak
            {
                return Err(Resource::Accounting.into());
            }
        }
        if !std::ptr::eq(source, function)
            || before.budget_slot != now.budget_slot
            || before.ledger != now.ledger
            || before.counter_slot != now.counter_slot
            || now.storage != expected
            || now.work < before.work
            || now.peak < before.peak
            || now.denied_work
            || now.denied_storage
            || self.failure.is_some()
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
    pub(in super::super) fn completed_for<'s>(
        &'s self,
        function: &SemanticFunctionDeclV1,
        budget: &Budget<'_>,
        owned: &usize,
    ) -> RetainedResult<&'s Schedule> {
        if self.phase != Phase::Complete {
            return Err(Resource::Accounting.into());
        }
        self.check(function, budget, owned)?;
        if self.schedule.locals.len() != function.locals().len()
            || self.schedule.indegrees.len() != function.blocks().len()
            || !self.ready.is_empty()
        {
            return Err(Resource::Accounting.into());
        }
        Ok(&self.schedule)
    }
    pub(in super::super) fn resource_failure(&self) -> Option<Resource> {
        self.failure
    }
}

/// A domain-specific internal seam: no replacement ledger, exposure of Budget,
/// callback, storage release, source facts or alternative BorrowWork policy.
struct RetainedMeter<'r, 'w> {
    budget: &'r mut Budget<'w>,
    owned: &'r mut usize,
    failure: &'r mut Option<Resource>,
}
impl RetainedMeter<'_, '_> {
    fn failed(&mut self, error: Resource) -> Resource {
        *self.failure.get_or_insert(error)
    }
    fn accept_work(&mut self, units: usize) -> std::result::Result<(), Resource> {
        self.budget
            .charge_work(units)
            .map_err(|error| self.failed(error))
    }
    fn reserve(&mut self, bytes: usize) -> std::result::Result<(), Resource> {
        let next = self
            .owned
            .checked_add(bytes)
            .ok_or(Resource::Arithmetic)
            .map_err(|error| self.failed(error))?;
        self.budget
            .reserve_storage(bytes)
            .map_err(|error| self.failed(error))?;
        *self.owned = next;
        Ok(())
    }
    fn capacity<T>(&mut self, output: &mut Vec<T>, count: usize) -> Result<()> {
        let bytes = count
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)
            .map_err(|error| {
                self.failed(error);
                Error::ResourceOverflow
            })?;
        self.reserve(bytes).map_err(|_| Error::ResourceOverflow)?;
        output.try_reserve_exact(count).map_err(|_| {
            self.failed(Resource::Allocation);
            Error::ResourceOverflow
        })?;
        let actual = output
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)
            .map_err(|error| {
                self.failed(error);
                Error::ResourceOverflow
            })?;
        let excess = actual
            .checked_sub(bytes)
            .ok_or(Resource::Accounting)
            .map_err(|error| {
                self.failed(error);
                Error::ResourceOverflow
            })?;
        self.reserve(excess).map_err(|_| Error::ResourceOverflow)?;
        Ok(())
    }
}
impl BorrowWork for RetainedMeter<'_, '_> {
    fn work(&mut self, units: usize) -> Result<()> {
        self.accept_work(units).map_err(|_| Error::ResourceOverflow)
    }
}

fn build_into(
    schedule: &mut Schedule,
    ready: &mut Vec<usize>,
    function: &SemanticFunctionDeclV1,
    meter: &mut RetainedMeter<'_, '_>,
) -> Result<()> {
    meter.work(
        function
            .locals()
            .len()
            .checked_add(
                function
                    .blocks()
                    .len()
                    .checked_mul(2)
                    .ok_or(Error::ResourceOverflow)?,
            )
            .ok_or(Error::ResourceOverflow)?,
    )?;
    meter.capacity(&mut schedule.locals, function.locals().len())?;
    for local in function.locals() {
        schedule.locals.push(Last {
            block: usize::MAX,
            statement: usize::MAX,
            pinned: local.role() == SemanticLocalRoleV1::Return,
        });
    }
    meter.capacity(&mut schedule.indegrees, function.blocks().len())?;
    schedule.indegrees.resize(function.blocks().len(), 0usize);
    meter.capacity(ready, function.blocks().len())?;
    for (block, body) in function.blocks().iter().enumerate() {
        meter.work(1)?;
        let mut census = Census {
            locals: &mut schedule.locals,
            meter,
            block,
            statement: 0,
        };
        for (index, row) in body.statements().iter().enumerate() {
            census.statement = index;
            statement(&mut census, row.kind())?;
        }
        census.statement = usize::MAX;
        terminator(&mut census, body.terminator().kind())?;
        body.terminator().kind().try_for_each_edge(|edge| {
            meter.work(3)?;
            let degree = schedule
                .indegrees
                .get_mut(edge.target().index() as usize)
                .ok_or(Error::ReplayMismatch)?;
            *degree = degree.checked_add(1).ok_or(Error::ResourceOverflow)?;
            Ok::<(), Error>(())
        })?;
    }
    for (block, degree) in schedule.indegrees.iter().enumerate() {
        meter.work(1)?;
        if *degree == 0 {
            ready.push(block);
        }
    }
    while let Some(block) = ready.pop() {
        meter.work(1)?;
        function.blocks()[block]
            .terminator()
            .kind()
            .try_for_each_edge(|edge| {
                meter.work(3)?;
                let target = edge.target().index() as usize;
                let degree = schedule
                    .indegrees
                    .get_mut(target)
                    .ok_or(Error::ReplayMismatch)?;
                *degree = degree.checked_sub(1).ok_or(Error::ReplayMismatch)?;
                if *degree == 0 {
                    ready.push(target);
                }
                Ok::<(), Error>(())
            })?;
    }
    Ok(())
}

fn frame() -> std::result::Result<usize, Resource> {
    // Typed source-policy envelopes, not native stack/RSS or allocator metadata.
    // Payload capacities are charged separately, before the corresponding use.
    let rows = [
        size_of::<RetainedSharedLivenessV1<'static>>(),
        size_of::<(
            Phase,
            Option<&SemanticFunctionDeclV1>,
            Option<Snapshot>,
            Option<Snapshot>,
            Schedule,
            Vec<usize>,
            Option<Resource>,
        )>(),
        size_of::<(
            Schedule,
            Last,
            Vec<Last>,
            Vec<usize>,
            Vec<usize>,
            &mut Vec<Last>,
            &mut Vec<usize>,
            &mut Vec<usize>,
        )>(),
        size_of::<(
            Snapshot,
            Option<Snapshot>,
            CanonicalKernelIrWorkLedgerIdentityV1,
            &Budget<'static>,
            &mut Budget<'static>,
            &usize,
            &mut usize,
        )>(),
        size_of::<(
            RetainedMeter<'static, 'static>,
            &mut RetainedMeter<'static, 'static>,
            &mut Option<Resource>,
            Option<Resource>,
            Resource,
            RetainedLivenessErrorV1,
            RetainedResult<()>,
            RetainedResult<&Schedule>,
            Result<()>,
            Error,
            std::result::Result<(), Resource>,
            std::result::Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError,
        )>(),
        size_of::<(
            &mut RetainedSharedLivenessV1<'static>,
            &RetainedSharedLivenessV1<'static>,
            &mut Schedule,
            &Schedule,
            &SemanticFunctionDeclV1,
            Option<&SemanticFunctionDeclV1>,
            Option<usize>,
            std::result::Result<usize, Resource>,
            bool,
            usize,
            usize,
            usize,
        )>(),
        size_of::<(
            Census<'static, RetainedMeter<'static, 'static>>,
            &mut Census<'static, RetainedMeter<'static, 'static>>,
            &[Last],
            &mut [Last],
            &mut Last,
            Option<&mut Last>,
        )>(),
        size_of::<(
            std::slice::Iter<'static, model::SemanticLocalDeclV1>,
            &[model::SemanticLocalDeclV1],
            &model::SemanticLocalDeclV1,
            model::SemanticLocalRoleV1,
            Last,
            &mut Vec<Last>,
            usize,
        )>(),
        size_of::<(
            std::iter::Enumerate<std::slice::Iter<'static, model::SemanticBasicBlockV1>>,
            (usize, &model::SemanticBasicBlockV1),
            &model::SemanticBasicBlockV1,
            &[model::SemanticBasicBlockV1],
            usize,
        )>(),
        size_of::<(
            std::iter::Enumerate<std::slice::Iter<'static, model::SemanticStatementV1>>,
            (usize, &model::SemanticStatementV1),
            &model::SemanticStatementV1,
            &[model::SemanticStatementV1],
            &model::SemanticStatementKindV1,
            usize,
        )>(),
        size_of::<(
            std::iter::Enumerate<std::slice::Iter<'static, usize>>,
            (usize, &usize),
            &usize,
            &mut usize,
            Option<&mut usize>,
            std::iter::Take<std::iter::Repeat<usize>>,
            usize,
        )>(),
        size_of::<(
            &mut RetainedMeter<'static, 'static>,
            &mut Schedule,
            &mut Vec<usize>,
            model::SemanticControlFlowEdgeV1,
            &model::SemanticControlFlowEdgeV1,
            model::SemanticBlockIdV1,
            Option<usize>,
            usize,
            usize,
            Result<()>,
        )>(),
        size_of::<(
            &model::SemanticTerminatorV1,
            &model::SemanticTerminatorKindV1,
            &model::SemanticDirectCallV1,
            &model::SemanticDirectTailCallV1,
            &[model::SemanticOperandV1],
            std::slice::Iter<'static, model::SemanticOperandV1>,
            &model::SemanticCallDestinationV1,
            Option<&model::SemanticCallDestinationV1>,
            model::SemanticUnwindActionV1,
            &model::SemanticAssertMessageV1,
        )>(),
        size_of::<(
            &model::SemanticPlaceV1,
            model::SemanticLocalIdV1,
            &model::SemanticLocalIdV1,
            u32,
            &[model::SemanticProjectionV1],
            std::slice::Iter<'static, model::SemanticProjectionV1>,
            &model::SemanticProjectionV1,
            model::SemanticProjectionV1,
            model::SemanticProjectionKindV1,
        )>(),
        size_of::<(
            &model::SemanticOperandV1,
            &model::SemanticAssignmentV1,
            &model::SemanticRvalueV1,
            &model::SemanticRvalueKindV1,
            &model::SemanticMemoryLoadV1,
            &model::SemanticMemoryStoreV1,
            &model::SemanticAtomicRmwV1,
            &model::SemanticAtomicCompareExchangeV1,
        )>(),
        size_of::<(
            &model::SemanticSwitchTargetsV1,
            &model::SemanticSwitchTargetV1,
            &[model::SemanticSwitchTargetV1],
            model::SemanticSwitchTargetV1,
            std::slice::Iter<'static, model::SemanticSwitchTargetV1>,
            model::SemanticControlFlowEdgeV1,
            model::SemanticUnwindActionV1,
            &model::SemanticUnwindActionV1,
        )>(),
        size_of::<(
            &mut Census<'static, RetainedMeter<'static, 'static>>,
            &model::SemanticOperandV1,
            Result<()>,
            &model::SemanticAssertMessageV1,
            &mut dyn FnMut(&model::SemanticOperandV1) -> Result<()>,
            &mut dyn FnMut(model::SemanticControlFlowEdgeV1) -> Result<()>,
        )>(),
        size_of::<(
            &model::SemanticAggregateRvalueV1,
            &model::SemanticCheckedBinaryRvalueV1,
            &model::SemanticUncheckedBinaryRvalueV1,
            &[model::SemanticOperandV1],
            std::slice::Iter<'static, model::SemanticOperandV1>,
            &model::SemanticOperandV1,
        )>(),
        size_of::<(
            &mut RetainedMeter<'static, 'static>,
            &mut Vec<Last>,
            &mut Vec<usize>,
            usize,
            usize,
            usize,
            usize,
            std::result::Result<usize, Resource>,
            std::result::Result<(), Resource>,
            Result<()>,
        )>(),
        size_of::<(
            &Budget<'static>,
            &mut Budget<'static>,
            &usize,
            &mut usize,
            *const Budget<'static>,
            *const usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            CanonicalKernelIrWorkLedgerIdentityV1,
            Option<usize>,
            bool,
            bool,
        )>(),
        size_of::<(
            Resource,
            Option<Resource>,
            &mut Option<Resource>,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1,
            fe2o3_kernel_ir::CanonicalKernelIrVerificationStorageLimitV1,
            std::result::Result<(), Resource>,
            RetainedLivenessErrorV1,
        )>(),
        size_of::<(
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            bool,
            bool,
            &SemanticFunctionDeclV1,
            &SemanticFunctionDeclV1,
        )>(),
        size_of::<(
            [usize; 23],
            std::array::IntoIter<usize, 23>,
            usize,
            usize,
            std::result::Result<usize, Resource>,
            Resource,
        )>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

#[cfg(test)]
#[path = "adapter_shared_primitive_liveness_retained_v1_tests.rs"]
mod tests;
