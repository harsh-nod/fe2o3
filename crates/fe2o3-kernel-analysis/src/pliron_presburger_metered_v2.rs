//! Shared-ledger witness queries. A decision is about a borrowed mathematical
//! set, not an authenticated compiler owner, memory effect, or runtime launch.

use super::{PresburgerConstraintV1, PresburgerFailureV1, PresburgerSetV1, WitnessSearchMeter};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::{
    fmt,
    marker::PhantomData,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Local session caps, in addition to the caller's cumulative shared ledger.
/// Values larger than the defaults are clamped; no query resets these counters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresburgerQueryLimitsV2 {
    /// Accepted session work including construction checks and coefficient scans.
    pub work: usize,
    /// Number of witness queries admitted by this session.
    pub queries: usize,
    /// Maximum additional live logical bytes above the session's caller floor.
    pub scratch_bytes: usize,
}
impl Default for PresburgerQueryLimitsV2 {
    fn default() -> Self {
        Self {
            work: super::MAX_PRESBURGER_WORK_UNITS_V1,
            queries: 2048,
            scratch_bytes: 65_536,
        }
    }
}

/// Which local session cap refused the next operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresburgerQueryResourceV2 {
    /// Cumulative session work.
    Work,
    /// Cumulative session queries.
    Queries,
    /// Additional live logical scratch bytes.
    Scratch,
}

/// A refusal or incomplete search, never an empty-set proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerQueryErrorV2 {
    /// The existing cumulative caller ledger or allocator refused an operation.
    Resource(Resource),
    /// Invalid mathematical input or arithmetic incompleteness.
    Model(PresburgerFailureV1),
    /// A separately versioned local session cap was reached.
    Limit {
        /// Local resource whose next operation was refused.
        resource: PresburgerQueryResourceV2,
        /// Attempted amount; arithmetic overflow is represented by usize::MAX.
        actual: usize,
        /// Clamped admitted cap.
        limit: usize,
    },
    /// A caller swallowed a denial made directly on the shared ledger.
    PriorDenial {
        /// First attempted cumulative work, if present.
        work: Option<usize>,
        /// First attempted live storage, if present.
        storage: Option<usize>,
    },
    /// Explicit consumer rejection, retained like every other query error.
    CallbackRejected,
    /// A consumer or rejected result destructor unwound.
    Panicked,
}
impl From<Resource> for PresburgerQueryErrorV2 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<PresburgerFailureV1> for PresburgerQueryErrorV2 {
    fn from(error: PresburgerFailureV1) -> Self {
        Self::Model(error)
    }
}
impl fmt::Display for PresburgerQueryErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "metered Presburger query V2: {self:?}")
    }
}
impl std::error::Error for PresburgerQueryErrorV2 {}
type Error = PresburgerQueryErrorV2;
type Result<T> = std::result::Result<T, Error>;

/// A decision borrowed only while its charged search backing remains live.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresburgerQueryDecisionV2<'query> {
    /// The complete bounded search proved this set empty.
    Empty,
    /// One point satisfying the original borrowed set, in V1 search order.
    Witness(&'query [i128]),
}

/// Inert usage observations, not a solver or compiler admission receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresburgerQueryUsageV2 {
    /// Accepted cumulative session work, including this observation.
    pub work: usize,
    /// Admitted queries, including queries that later refused.
    pub queries: usize,
    /// Maximum additional live logical bytes above the session caller floor.
    pub peak_scratch_bytes: usize,
}

/// A lexical cumulative solver session on the original budget address/ledger.
/// A first query, consumer, or custody error poisons all subsequent queries and
/// the outer callback, even when the caller ignores the individual error.
pub struct PresburgerQueryScopeV2<'scope> {
    limits: PresburgerQueryLimitsV2,
    work: usize,
    queries: usize,
    peak: usize,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    caller_floor: usize,
    floor: usize,
    failure: Option<Error>,
    lexical: PhantomData<&'scope mut &'scope ()>,
}

fn drain<T>(value: T) {
    let mut dropped = catch_unwind(AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = dropped {
        dropped = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
}

fn frames<T>() -> Result<usize> {
    size_of::<std::thread::Result<Result<T>>>()
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of::<std::thread::Result<()>>()))
        .ok_or_else(|| Resource::Arithmetic.into())
}

impl PresburgerQueryScopeV2<'_> {
    fn retain<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            if self.failure.is_none() {
                self.failure = Some(error.clone());
            }
        }
        result
    }
    fn identity(&self, budget: &Budget<'_>) -> bool {
        self.slot == std::ptr::from_ref(budget) as usize
            && self.ledger == budget.work_ledger_identity_v1()
    }
    fn check(&mut self, budget: &Budget<'_>) -> Result<()> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if !self.identity(budget) {
            return self.retain(Err(Resource::Accounting.into()));
        }
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return self.retain(Err(Error::PriorDenial {
                work: budget.failed_work(),
                storage: budget.failed_storage(),
            }));
        }
        if budget.storage() < self.floor {
            return self.retain(Err(Resource::Accounting.into()));
        }
        Ok(())
    }
    fn charge(&mut self, budget: &mut Budget<'_>, amount: usize) -> Result<()> {
        self.check(budget)?;
        let next = self.work.checked_add(amount).ok_or(Resource::Arithmetic);
        let next = self.retain(next.map_err(Into::into))?;
        if next > self.limits.work {
            return self.retain(Err(Error::Limit {
                resource: PresburgerQueryResourceV2::Work,
                actual: next,
                limit: self.limits.work,
            }));
        }
        self.retain(budget.charge_work(amount).map_err(Into::into))?;
        self.work = next;
        Ok(())
    }
    fn reserve(&mut self, budget: &mut Budget<'_>, amount: usize) -> Result<()> {
        self.check(budget)?;
        let next = budget
            .storage()
            .checked_sub(self.caller_floor)
            .and_then(|n| n.checked_add(amount))
            .ok_or(Resource::Arithmetic);
        let next = self.retain(next.map_err(Into::into))?;
        if next > self.limits.scratch_bytes {
            return self.retain(Err(Error::Limit {
                resource: PresburgerQueryResourceV2::Scratch,
                actual: next,
                limit: self.limits.scratch_bytes,
            }));
        }
        self.retain(budget.reserve_storage(amount).map_err(Into::into))?;
        self.peak = self.peak.max(next);
        Ok(())
    }
    fn settle<T>(
        &mut self,
        outcome: std::thread::Result<Result<T>>,
        budget: &Budget<'_>,
        required: usize,
    ) -> Result<T> {
        let unwound = outcome.is_err();
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => {
                drain(payload);
                Err(Error::Panicked)
            }
        };
        let _ = self.check(budget);
        if let Err(error) = &result {
            let _ = self.retain::<()>(Err(error.clone()));
        }
        if !self.identity(budget)
            || budget.storage() < required
            || (!unwound && result.is_ok() && budget.storage() != required)
        {
            let _ = self.retain::<()>(Err(Resource::Accounting.into()));
        }
        if let Some(error) = self.failure.clone() {
            drain(result);
            Err(error)
        } else {
            result
        }
    }
    fn refund<T>(&mut self, result: Result<T>, budget: &mut Budget<'_>, floor: usize) -> Result<T> {
        if !self.identity(budget) || budget.storage() < floor {
            let _ = self.retain::<()>(Err(Resource::Accounting.into()));
            drain(result);
            return Err(self.failure.clone().unwrap_or(Resource::Accounting.into()));
        }
        if let Err(error) = budget.release_storage(budget.storage() - floor) {
            let _ = self.retain::<()>(Err(error.into()));
            drain(result);
            return Err(self.failure.clone().unwrap_or(error.into()));
        }
        result
    }

    /// Performs a paid observation without clearing any denial or counter.
    pub fn usage(&mut self, budget: &mut Budget<'_>) -> Result<PresburgerQueryUsageV2> {
        self.charge(budget, 1)?;
        Ok(PresburgerQueryUsageV2 {
            work: self.work,
            queries: self.queries,
            peak_scratch_bytes: self.peak,
        })
    }

    /// Searches the exact borrowed set with V1's finite-box/interval-pruning
    /// algorithm. All V2 limits are cumulative across this session. Invalid,
    /// exhausted or overflowing queries return a sticky error, never Empty.
    ///
    /// The caller owns and accounts for constructing/retaining the input set.
    /// Point/witness backing, typed query scratch and the concrete callback
    /// capture header are paid here; borrowed input backing is not duplicated.
    /// Callback scratch must be dropped/refunded before a successful return.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_analysis::{PresburgerQueryScopeV2, PresburgerQueryDecisionV2, PresburgerQueryErrorV2, PresburgerSetV1};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape<'a>(scope: &mut PresburgerQueryScopeV2<'_>, set: &PresburgerSetV1, budget: &mut Budget<'_>) -> Result<&'a [i128], PresburgerQueryErrorV2> {
    ///     scope.with_witness(set, budget, |decision, _| match decision {
    ///         PresburgerQueryDecisionV2::Witness(point) => Ok(point),
    ///         _ => unreachable!(),
    ///     })
    /// }
    /// ```
    pub fn with_witness<'work, T>(
        &mut self,
        set: &PresburgerSetV1,
        budget: &mut Budget<'work>,
        consume: impl for<'query> FnOnce(
            PresburgerQueryDecisionV2<'query>,
            &mut Budget<'work>,
        ) -> Result<T>,
    ) -> Result<T> {
        self.check(budget)?;
        let queries = self.queries.checked_add(1).ok_or(Resource::Arithmetic);
        let queries = self.retain(queries.map_err(Into::into))?;
        if queries > self.limits.queries {
            return self.retain(Err(Error::Limit {
                resource: PresburgerQueryResourceV2::Queries,
                actual: queries,
                limit: self.limits.queries,
            }));
        }
        self.charge(budget, 4)?;
        self.queries = queries;
        let floor = budget.storage();
        let mut required = floor;
        let capture_bytes = std::mem::size_of_val(&consume);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let headers = frames::<T>()?
                .checked_add(size_of::<Option<Vec<i128>>>())
                .and_then(|n| n.checked_add(size_of::<Vec<i128>>()))
                .and_then(|n| n.checked_add(size_of::<Meter<'_, '_, '_>>()))
                .and_then(|n| n.checked_add(capture_bytes))
                .ok_or(Resource::Arithmetic)?;
            self.reserve(budget, headers)?;
            required = budget.storage();
            validate(set, self, budget)?;
            // Typed logical recursive-frame allowance, not an allocator/RSS claim.
            let frames = set
                .domain
                .rank()
                .checked_add(1)
                .and_then(|n| {
                    n.checked_mul(size_of::<(
                        usize,
                        i128,
                        &mut [i128],
                        Result<Option<Vec<i128>>>,
                    )>())
                })
                .ok_or(Resource::Arithmetic)?;
            self.reserve(budget, frames)?;
            let found = set.find_witness_with_meter(&mut Meter {
                scope: self,
                budget,
            })?;
            required = budget.storage();
            let decision = match &found {
                Some(point) => PresburgerQueryDecisionV2::Witness(point),
                None => PresburgerQueryDecisionV2::Empty,
            };
            let returned = catch_unwind(AssertUnwindSafe(|| consume(decision, budget)));
            let result = self.settle(returned, budget, required);
            drop(found);
            result
        }));
        let result = self.settle(outcome, budget, required);
        self.refund(result, budget, floor)
    }
}

/// Opens a lexical V2 query session on the original shared budget. Local limits
/// cannot replace cumulative shared accounting; prior denials are refused.
/// First failures remain sticky through ignored errors, callbacks and teardown.
/// The original caller floor is restored only on the original budget/ledger.
/// A recorded denial on that ledger precedes a later-observed floor violation;
/// foreign budget identity is refused before consulting unrelated denials.
/// The concrete callback capture header is reserved before invocation.
///
/// This does not authenticate source owners, conflict coverage, launch inputs,
/// formal completion or publication. Borrowed mathematical inputs and consumer-
/// owned results remain the caller's separate accounting responsibility.
pub fn with_presburger_queries_v2<'work, T>(
    limits: PresburgerQueryLimitsV2,
    budget: &mut Budget<'work>,
    consume: impl for<'scope> FnOnce(
        &mut PresburgerQueryScopeV2<'scope>,
        &mut Budget<'work>,
    ) -> Result<T>,
) -> Result<T> {
    let cap = PresburgerQueryLimitsV2::default();
    let floor = budget.storage();
    let mut scope = PresburgerQueryScopeV2 {
        limits: PresburgerQueryLimitsV2 {
            work: limits.work.min(cap.work),
            queries: limits.queries.min(cap.queries),
            scratch_bytes: limits.scratch_bytes.min(cap.scratch_bytes),
        },
        work: 0,
        queries: 0,
        peak: 0,
        slot: std::ptr::from_ref(&*budget) as usize,
        ledger: budget.work_ledger_identity_v1(),
        caller_floor: floor,
        floor,
        failure: None,
        lexical: PhantomData,
    };
    let capture_bytes = std::mem::size_of_val(&consume);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        scope.charge(budget, 4)?;
        let headers = frames::<T>()?
            .checked_add(size_of::<PresburgerQueryScopeV2<'_>>())
            .and_then(|n| n.checked_add(capture_bytes))
            .ok_or(Resource::Arithmetic)?;
        scope.reserve(budget, headers)?;
        scope.floor = budget.storage();
        consume(&mut scope, budget)
    }));
    let result = scope.settle(outcome, budget, scope.floor);
    scope.refund(result, budget, floor)
}

fn validate(
    set: &PresburgerSetV1,
    scope: &mut PresburgerQueryScopeV2<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scope.charge(budget, 4)?;
    let rank = set.domain.rank();
    if rank > super::MAX_PRESBURGER_VARIABLES_V1
        || rank != set.domain.upper_exclusive.len()
        || set.constraints.len() > super::MAX_PRESBURGER_CONSTRAINTS_V1
    {
        return Err(PresburgerFailureV1::InvalidModel {
            detail: "V2 box or constraint shape exceeds V1 model",
        }
        .into());
    }
    for constraint in &set.constraints {
        scope.charge(budget, 3)?;
        let (expression, valid_modulus) = match constraint {
            PresburgerConstraintV1::LessEqualZero(expression)
            | PresburgerConstraintV1::EqualZero(expression) => (expression, true),
            PresburgerConstraintV1::CongruentZero {
                expression,
                modulus,
            } => (expression, *modulus > 0),
        };
        if expression.coefficients.len() != rank || !valid_modulus {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "V2 constraint rank or modulus differs",
            }
            .into());
        }
    }
    Ok(())
}

struct Meter<'scope, 'borrow, 'work> {
    scope: &'borrow mut PresburgerQueryScopeV2<'scope>,
    budget: &'borrow mut Budget<'work>,
}
impl WitnessSearchMeter for Meter<'_, '_, '_> {
    type Error = Error;
    fn node(&mut self) -> Result<()> {
        self.scope.charge(self.budget, 1)
    }
    fn traversal(&mut self, work: usize) -> Result<()> {
        self.scope.charge(self.budget, work)
    }
    fn copy_point(&mut self, point: &[i128]) -> Result<Vec<i128>> {
        self.scope.charge(
            self.budget,
            point.len().checked_add(4).ok_or(Resource::Arithmetic)?,
        )?;
        let bytes = point
            .len()
            .checked_mul(size_of::<i128>())
            .ok_or(Resource::Arithmetic)?;
        self.scope.reserve(self.budget, bytes)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(point.len())
            .map_err(|_| Resource::Allocation)?;
        let excess = result
            .capacity()
            .checked_sub(point.len())
            .and_then(|n| n.checked_mul(size_of::<i128>()))
            .ok_or(Resource::Arithmetic)?;
        self.scope.reserve(self.budget, excess)?;
        result.extend_from_slice(point);
        Ok(result)
    }
}

#[cfg(test)]
#[path = "pliron_presburger_metered_v2_tests.rs"]
mod tests;
