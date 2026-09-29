//! Shared-ledger formal report construction phases. This module stays private:
//! effects, definitions, private slots and access extraction are not metered by
//! this phase and must not be represented as paid by its report-row credit.

use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::{cmp::Ordering, mem::size_of, panic::AssertUnwindSafe};

pub(super) trait ReportMeterV18 {
    fn bounds_work(&mut self) -> Result<(), GuardedResourceErrorV1>;
    fn charge(&mut self, work: usize) -> Result<(), GuardedResourceErrorV1>;
    fn push<T>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), GuardedResourceErrorV1>;
    fn sort<T>(
        &mut self,
        rows: &mut [T],
        compare: impl FnMut(&T, &T) -> Ordering,
    ) -> Result<(), GuardedResourceErrorV1>;
    fn retire<T>(&mut self, rows: Vec<T>) -> Result<(), GuardedResourceErrorV1>;
}

pub(super) trait BoundsReasonSinkV18 {
    fn overflow(
        &mut self,
        location: FunctionOperationLocation,
        meter: &mut impl ReportMeterV18,
    ) -> Result<(), GuardedResourceErrorV1>;
}

impl BoundsReasonSinkV18 for BTreeSet<FormalMemoryIncompleteReason> {
    fn overflow(
        &mut self,
        location: FunctionOperationLocation,
        _: &mut impl ReportMeterV18,
    ) -> Result<(), GuardedResourceErrorV1> {
        self.insert(FormalMemoryIncompleteReason::AddressArithmeticOverflow { location });
        Ok(())
    }
}

impl BoundsReasonSinkV18 for Vec<FunctionOperationLocation> {
    fn overflow(
        &mut self,
        location: FunctionOperationLocation,
        meter: &mut impl ReportMeterV18,
    ) -> Result<(), GuardedResourceErrorV1> {
        let mut position = self.len();
        for (index, existing) in self.iter().enumerate() {
            meter.charge(1)?;
            match existing.cmp(&location) {
                Ordering::Less => (),
                Ordering::Equal => return Ok(()),
                Ordering::Greater => {
                    position = index;
                    break;
                }
            }
        }
        meter.charge(self.len() - position)?;
        meter.push(self, location)?;
        self[position..].rotate_right(1);
        Ok(())
    }
}

pub(super) struct LiveReportMeterV18<'budget, 'work> {
    pub(super) budget: &'budget mut Budget<'work>,
}

fn bytes<T>(capacity: usize) -> Result<usize, GuardedResourceErrorV1> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(GuardedResourceErrorV1::Arithmetic)
}

impl ReportMeterV18 for LiveReportMeterV18<'_, '_> {
    fn bounds_work(&mut self) -> Result<(), GuardedResourceErrorV1> {
        self.charge(guarded_access_v1::BOUNDS_WORK)
    }

    fn charge(&mut self, work: usize) -> Result<(), GuardedResourceErrorV1> {
        self.budget.charge_work(work).map_err(Into::into)
    }

    fn push<T>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), GuardedResourceErrorV1> {
        self.charge(1)?;
        if rows.len() == rows.capacity() {
            let capacity = rows
                .capacity()
                .max(1)
                .checked_mul(2)
                .ok_or(GuardedResourceErrorV1::Arithmetic)?;
            self.charge(
                rows.len()
                    .checked_add(2)
                    .ok_or(GuardedResourceErrorV1::Arithmetic)?,
            )?;
            let old = bytes::<T>(rows.capacity())?;
            self.budget.reserve_storage(bytes::<T>(capacity)?)?;
            rows.try_reserve_exact(
                capacity
                    .checked_sub(rows.len())
                    .ok_or(GuardedResourceErrorV1::Accounting)?,
            )
            .map_err(|_| GuardedResourceErrorV1::Allocation)?;
            let excess = rows
                .capacity()
                .checked_sub(capacity)
                .ok_or(GuardedResourceErrorV1::Accounting)?;
            self.budget.reserve_storage(bytes::<T>(excess)?)?;
            // Old and replacement capacities overlap until allocation succeeds.
            self.budget.release_storage(old)?;
        }
        rows.push(value);
        Ok(())
    }

    fn sort<T>(
        &mut self,
        rows: &mut [T],
        compare: impl FnMut(&T, &T) -> Ordering,
    ) -> Result<(), GuardedResourceErrorV1> {
        crate::verification_index_v1::verification_bounded_sort_by_v1(rows, 1, self.budget, compare)
            .map_err(Into::into)
    }

    fn retire<T>(&mut self, rows: Vec<T>) -> Result<(), GuardedResourceErrorV1> {
        let retained = bytes::<T>(rows.capacity())?;
        drop(rows);
        self.budget.release_storage(retained).map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReportConstructionErrorV18 {
    Resource(GuardedResourceErrorV1),
    PriorDenial {
        work: Option<usize>,
        storage: Option<usize>,
    },
    Rejected,
    Panicked,
}
impl From<GuardedResourceErrorV1> for ReportConstructionErrorV18 {
    fn from(error: GuardedResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}
impl From<Resource> for ReportConstructionErrorV18 {
    fn from(error: Resource) -> Self {
        Self::Resource(error.into())
    }
}

type ResultV18<T> = Result<T, ReportConstructionErrorV18>;

/// A private borrowed phase view, not a replacement report or admission token.
/// The original report, including every reason and conflict, is retained.
pub(super) struct ReportRowsV18<'view, 'owner> {
    original: &'view CanonicalOwnerFormalAnalysisV18<'owner>,
    bounds: &'view Vec<FormalBoundsRequirement>,
    overflows: &'view Vec<FunctionOperationLocation>,
    aliases: &'view Vec<RuntimeAliasRequirement>,
    conflicts: &'view Vec<InterInvocationConflictRequirement>,
}

struct PhaseStateV18 {
    floor: usize,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    required: usize,
    selected: Option<ReportConstructionErrorV18>,
}
impl PhaseStateV18 {
    fn same(&self, budget: &Budget<'_>) -> bool {
        self.slot == std::ptr::from_ref(budget) as usize
            && self.ledger == budget.work_ledger_identity_v1()
    }
}

fn drain<T>(value: T) {
    let mut result = std::panic::catch_unwind(AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = result {
        result = std::panic::catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
}

fn headers<T, F>() -> ResultV18<usize> {
    size_of::<std::thread::Result<ResultV18<T>>>()
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of::<std::thread::Result<()>>()))
        .and_then(|n| n.checked_add(size_of::<ReportRowsV18<'_, '_>>()))
        .and_then(|n| n.checked_add(size_of::<LiveReportMeterV18<'_, '_>>()))
        .and_then(|n| n.checked_add(size_of::<PhaseStateV18>()))
        .and_then(|n| {
            n.checked_add(size_of::<(
                &CanonicalOwnerFormalAnalysisV18<'_>,
                &mut Budget<'_>,
            )>())
        })
        .and_then(|n| {
            n.checked_add(size_of::<Vec<(FormalAllocationIdentity, AllocationEnvelope)>>())
        })
        .and_then(|n| n.checked_add(size_of::<Vec<RuntimeAliasRequirement>>()))
        .and_then(|n| n.checked_add(size_of::<Vec<InterInvocationConflictRequirement>>()))
        .and_then(|n| n.checked_add(size_of::<Vec<FormalBoundsRequirement>>()))
        .and_then(|n| n.checked_add(size_of::<Vec<FunctionOperationLocation>>()))
        .and_then(|n| n.checked_add(size_of::<F>()))
        .and_then(|n| n.checked_add(size_of::<Option<F>>()))
        .ok_or(Resource::Arithmetic.into())
}

fn prior(budget: &Budget<'_>) -> Option<ReportConstructionErrorV18> {
    (budget.failed_work().is_some() || budget.failed_storage().is_some()).then(|| {
        ReportConstructionErrorV18::PriorDenial {
            work: budget.failed_work(),
            storage: budget.failed_storage(),
        }
    })
}

/// Internal phase composition only. The input owner/report and predecessor
/// construction remain caller-owned, separately paid dependencies. This does
/// not invoke the old unmetered extraction engine or claim its work was paid.
/// The callback can borrow the newly paid vectors while their capacities live;
/// a future complete formal scope must use this phase before publishing a view.
pub(super) fn with_report_rows_v18<'owner, 'work, T, F>(
    original: &CanonicalOwnerFormalAnalysisV18<'owner>,
    budget: &mut Budget<'work>,
    consume: F,
) -> ResultV18<T>
where
    F: for<'view> FnOnce(&ReportRowsV18<'view, 'owner>, &mut Budget<'work>) -> ResultV18<T>,
{
    if let Some(error) = prior(budget) {
        drain(consume);
        return Err(error);
    }
    // Keep uninvoked captures outside construction's unwind frame so a denial
    // is selected before a rejected capture destructor can itself unwind.
    let mut pending = Some(consume);
    let mut state = PhaseStateV18 {
        floor: budget.storage(),
        slot: std::ptr::from_ref(&*budget) as usize,
        ledger: budget.work_ledger_identity_v1(),
        required: budget.storage(),
        selected: None,
    };
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(4)?;
        budget.reserve_storage(headers::<T, F>()?)?;
        let (bounds, overflows, aliases, conflicts) = {
            let accesses = original.analysis().obligations().accesses();
            let mut meter = LiveReportMeterV18 { budget };
            let mut overflows = Vec::new();
            let bounds =
                derive_bounds_requirements_with_meter(accesses, &mut overflows, &mut meter)?;
            let aliases = derive_alias_requirements_with_meter(accesses, &mut meter)?;
            let conflicts = derive_inter_invocation_conflicts_with_meter(accesses, &mut meter)?;
            (bounds, overflows, aliases, conflicts)
        };
        state.required = budget.storage();
        let view = ReportRowsV18 {
            original,
            bounds: &bounds,
            overflows: &overflows,
            aliases: &aliases,
            conflicts: &conflicts,
        };
        let consume = pending.take().ok_or(Resource::Accounting)?;
        let callback = std::panic::catch_unwind(AssertUnwindSafe(|| consume(&view, budget)));
        state.selected = if !state.same(budget) {
            Some(Resource::Accounting.into())
        } else {
            prior(budget).or_else(|| {
                (budget.storage() < state.required
                    || (matches!(&callback, Ok(Ok(_))) && budget.storage() != state.required))
                    .then_some(Resource::Accounting.into())
            })
        };
        let result = match callback {
            Ok(result) => result,
            Err(payload) => {
                drain(payload);
                Err(ReportConstructionErrorV18::Panicked)
            }
        };
        drop(aliases);
        drop(conflicts);
        drop(bounds);
        drop(overflows);
        if let Some(error) = state.selected {
            drain(result);
            Err(error)
        } else {
            result
        }
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            if state.selected.is_none() {
                state.selected = if state.same(budget) {
                    prior(budget)
                } else {
                    Some(Resource::Accounting.into())
                };
            }
            drain(payload);
            Err(state
                .selected
                .unwrap_or(ReportConstructionErrorV18::Panicked))
        }
    };
    drain(pending);
    if !state.same(budget) || budget.storage() < state.floor {
        drain(result);
        return Err(state.selected.unwrap_or(Resource::Accounting.into()));
    }
    // Every output and scratch vector has been destroyed before its credit.
    if let Err(error) = budget.release_storage(budget.storage() - state.floor) {
        drain(result);
        return Err(state.selected.unwrap_or(error.into()));
    }
    result
}

#[cfg(test)]
#[path = "report_construction_v18_tests.rs"]
mod tests;
