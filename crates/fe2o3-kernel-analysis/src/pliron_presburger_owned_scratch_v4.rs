//! Exact solver-owned credit, independent of already-paid borrowed inputs.
use super::narrowing_v3::{Bounds, NarrowingMeterV4};
use super::*;

/// One cumulative solver session with separately owned scratch accounting.
/// Borrowed input/report backing remains the caller's paid responsibility.
/// This authenticates no compiler source, launch or final memory authority.
pub struct PresburgerQueryScopeV4<'scope> {
    state: PresburgerQueryScopeV2<'scope>,
    session_bytes: usize,
    refund_denied: bool,
}

struct Credit {
    floor: usize,
    bytes: usize,
}
impl Credit {
    fn required(&self) -> Result<usize> {
        self.floor
            .checked_add(self.bytes)
            .ok_or_else(|| Resource::Arithmetic.into())
    }
    fn observe(&self, scope: &mut PresburgerQueryScopeV4<'_>, budget: &Budget<'_>) -> Result<()> {
        scope.observe_session(budget)?;
        if !scope.state.identity(budget)
            || self.required().is_err()
            || budget.storage() < self.required()?
        {
            scope.refund_denied = true;
            return scope.state.retain(Err(Resource::Accounting.into()));
        }
        Ok(())
    }
    fn reserve(
        &mut self,
        scope: &mut PresburgerQueryScopeV4<'_>,
        budget: &mut Budget<'_>,
        amount: usize,
    ) -> Result<()> {
        scope.check(budget)?;
        self.observe(scope, budget)?;
        let bytes = self.bytes.checked_add(amount).ok_or(Resource::Arithmetic)?;
        let local = scope
            .session_bytes
            .checked_add(bytes)
            .ok_or(Resource::Arithmetic)?;
        if local > scope.state.limits.scratch_bytes {
            return scope.state.retain(Err(Error::Limit {
                resource: PresburgerQueryResourceV2::Scratch,
                actual: local,
                limit: scope.state.limits.scratch_bytes,
            }));
        }
        scope
            .state
            .retain(budget.reserve_storage(amount).map_err(Into::into))?;
        self.bytes = bytes;
        scope.state.peak = scope.state.peak.max(local);
        Ok(())
    }
    fn finish<T>(
        self,
        scope: &mut PresburgerQueryScopeV4<'_>,
        budget: &mut Budget<'_>,
        result: Result<T>,
    ) -> Result<T> {
        let result = scope
            .state
            .settle(Ok(result), budget, self.required().unwrap_or(usize::MAX));
        let _ = self.observe(scope, budget);
        // Only known solver credits retire, never an entire caller floor delta.
        if !scope.refund_denied {
            if let Err(error) = budget.release_storage(self.bytes) {
                scope.refund_denied = true;
                let _ = scope.state.retain::<()>(Err(error.into()));
                drain(result);
                return Err(scope.state.failure.clone().unwrap_or_else(|| error.into()));
            }
        }
        result
    }
}

struct OwnedMeter<'query, 'scope> {
    scope: &'query mut PresburgerQueryScopeV4<'scope>,
    credit: &'query mut Credit,
}
impl NarrowingMeterV4 for OwnedMeter<'_, '_> {
    fn charge(&mut self, budget: &mut Budget<'_>, amount: usize) -> Result<()> {
        self.scope.state.charge(budget, amount)
    }
    fn reserve(&mut self, budget: &mut Budget<'_>, amount: usize) -> Result<()> {
        self.credit.reserve(self.scope, budget, amount)
    }
}

struct SearchMeter<'query, 'scope, 'work> {
    owned: OwnedMeter<'query, 'scope>,
    budget: &'query mut Budget<'work>,
}
impl WitnessSearchMeter for SearchMeter<'_, '_, '_> {
    type Error = Error;
    fn node(&mut self) -> Result<()> {
        self.owned.charge(self.budget, 1)
    }
    fn traversal(&mut self, work: usize) -> Result<()> {
        self.owned.charge(self.budget, work)
    }
    fn copy_point(&mut self, point: &[i128]) -> Result<Vec<i128>> {
        self.owned.charge(
            self.budget,
            point.len().checked_add(4).ok_or(Resource::Arithmetic)?,
        )?;
        let requested = point
            .len()
            .checked_mul(size_of::<i128>())
            .ok_or(Resource::Arithmetic)?;
        self.owned.reserve(self.budget, requested)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(point.len())
            .map_err(|_| Resource::Allocation)?;
        let excess = result
            .capacity()
            .checked_sub(point.len())
            .and_then(|n| n.checked_mul(size_of::<i128>()))
            .ok_or(Resource::Arithmetic)?;
        self.owned.reserve(self.budget, excess)?;
        result.extend_from_slice(point);
        Ok(result)
    }
}

impl PresburgerQueryScopeV4<'_> {
    fn observe_session(&mut self, budget: &Budget<'_>) -> Result<()> {
        if !self.state.identity(budget) || budget.storage() < self.state.floor {
            self.refund_denied = true;
            return self.state.retain(Err(Resource::Accounting.into()));
        }
        Ok(())
    }

    fn check(&mut self, budget: &Budget<'_>) -> Result<()> {
        let selected = self.state.check(budget);
        let custody = self.observe_session(budget);
        selected.and(custody)
    }

    /// Observes the cumulative work/query history without clearing failure.
    /// In this V4 scope, the inert peak counter describes solver-owned bytes,
    /// not separately paid borrowed input growth above the session entry.
    pub fn usage(&mut self, budget: &mut Budget<'_>) -> Result<PresburgerQueryUsageV2> {
        let selected = self.state.usage(budget);
        let custody = self.observe_session(budget);
        selected.and_then(|usage| custody.map(|()| usage))
    }

    fn query<'work, T, O, C, B, V>(
        &mut self,
        set: &PresburgerSetV1,
        budget: &mut Budget<'work>,
        consume: C,
        build: B,
        visit: V,
    ) -> Result<T>
    where
        B: FnOnce(&mut OwnedMeter<'_, '_>, &mut Budget<'work>) -> Result<O>,
        V: FnOnce(&O, C, &mut Budget<'work>) -> Result<T>,
    {
        let mut credit = Credit {
            floor: budget.storage(),
            bytes: 0,
        };
        let mut pending = Some(consume);
        let mut build = Some(build);
        let mut visit = Some(visit);
        let mut execute = |scope: &mut Self, credit: &mut Credit, budget: &mut Budget<'work>| {
            scope.check(budget)?;
            let queries = scope
                .state
                .queries
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
            if queries > scope.state.limits.queries {
                return scope.state.retain(Err(Error::Limit {
                    resource: PresburgerQueryResourceV2::Queries,
                    actual: queries,
                    limit: scope.state.limits.queries,
                }));
            }
            scope.state.charge(budget, 8)?;
            scope.state.queries = queries;
            validate(set, &mut scope.state, budget)?;
            let mut meter = OwnedMeter { scope, credit };
            let owned = build.take().expect("single owned query construction")(&mut meter, budget)?;
            let required = meter.credit.required()?;
            let returned = catch_unwind(AssertUnwindSafe(|| {
                visit.take().expect("single owned query observation")(
                    &owned,
                    pending.take().expect("single owned query consumer"),
                    budget,
                )
            }));
            let result = meter.scope.state.settle(returned, budget, required);
            let _ = meter.credit.observe(meter.scope, budget);
            drop(owned);
            result
        };
        let execute_bytes = std::mem::size_of_val(&execute);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            self.check(budget)?;
            let headers = frames::<T>()?
                .checked_add(size_of::<Option<C>>())
                .and_then(|n| n.checked_add(size_of::<Option<B>>()))
                .and_then(|n| n.checked_add(size_of::<Option<V>>()))
                .and_then(|n| n.checked_add(size_of::<O>()))
                .and_then(|n| n.checked_add(size_of::<Result<O>>()))
                .and_then(|n| n.checked_add(size_of::<Bounds>()))
                .and_then(|n| n.checked_add(size_of::<PresburgerQueryDecisionV2<'_>>()))
                .and_then(|n| n.checked_add(size_of::<PresburgerAffineNarrowingDecisionV3<'_>>()))
                .and_then(|n| n.checked_add(size_of::<Credit>()))
                .and_then(|n| n.checked_add(size_of::<OwnedMeter<'_, '_>>()))
                .and_then(|n| n.checked_add(size_of::<SearchMeter<'_, '_, '_>>()))
                .and_then(|n| n.checked_add(execute_bytes))
                .and_then(|n| {
                    n.checked_add(
                        2 * size_of::<(&mut Self, &mut Budget<'_>, &mut Credit, &mut (), usize)>(),
                    )
                })
                .ok_or(Resource::Arithmetic)?;
            credit.reserve(self, budget, headers)?;
            execute(self, &mut credit, budget)
        }));
        drop(execute);
        let result = self
            .state
            .settle(outcome, budget, credit.required().unwrap_or(usize::MAX));
        let _ = credit.observe(self, budget);
        drain(pending);
        drain(build);
        drain(visit);
        credit.finish(self, budget, result)
    }

    /// Runs the unchanged V1 finite-box search with owned V4 query scratch.
    /// Work/query limits and first failure stay cumulative. Borrowed inputs
    /// already live at entry are paid by their caller, never refunded here.
    /// Callback results cannot borrow temporary witness backing.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_analysis::{PresburgerQueryScopeV4, PresburgerQueryDecisionV2, PresburgerQueryErrorV2, PresburgerSetV1};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape<'a>(scope: &mut PresburgerQueryScopeV4<'_>, set: &PresburgerSetV1, budget: &mut Budget<'_>) -> Result<&'a [i128], PresburgerQueryErrorV2> {
    ///     scope.with_witness(set, budget, |decision, _| match decision {
    ///         PresburgerQueryDecisionV2::Witness(point) => Ok(point),
    ///         _ => unreachable!(),
    ///     })
    /// }
    /// ```
    pub fn with_witness<'work, T, C>(
        &mut self,
        set: &PresburgerSetV1,
        budget: &mut Budget<'work>,
        consume: C,
    ) -> Result<T>
    where
        C: for<'query> FnOnce(PresburgerQueryDecisionV2<'query>, &mut Budget<'work>) -> Result<T>,
    {
        self.query(
            set,
            budget,
            consume,
            |meter, budget| {
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
                meter.reserve(budget, frames)?;
                set.find_witness_with_meter(&mut SearchMeter {
                    owned: OwnedMeter {
                        scope: &mut *meter.scope,
                        credit: &mut *meter.credit,
                    },
                    budget,
                })
            },
            |owned, consume, budget| {
                consume(
                    match owned {
                        Some(point) => PresburgerQueryDecisionV2::Witness(point),
                        None => PresburgerQueryDecisionV2::Empty,
                    },
                    budget,
                )
            },
        )
    }

    /// Runs the shared unchanged V3 affine narrowing grammar on this history.
    /// Overflow and resource uncertainty never become emptiness proofs.
    pub fn with_affine_narrowing_v4<'work, T, C>(
        &mut self,
        set: &PresburgerSetV1,
        budget: &mut Budget<'work>,
        consume: C,
    ) -> Result<T>
    where
        C: for<'query> FnOnce(
            PresburgerAffineNarrowingDecisionV3<'query>,
            &mut Budget<'work>,
        ) -> Result<T>,
    {
        self.query(
            set,
            budget,
            consume,
            |meter, budget| narrowing_v3::narrow(set, meter, budget),
            |owned: &Option<Bounds>, consume, budget| {
                consume(
                    match owned {
                        None => PresburgerAffineNarrowingDecisionV3::Empty,
                        Some(bounds) => PresburgerAffineNarrowingDecisionV3::Bounds {
                            lower: &bounds.lower,
                            upper_exclusive: &bounds.upper,
                        },
                    },
                    budget,
                )
            },
        )
    }
}

/// Opens one cumulative owned-scratch session on the original ledger.
/// Limits are clamped to unchanged V2 defaults. Exact solver-owned credits
/// retire after backing/captures drop; caller-owned input/report or surplus
/// credit is never part of a rollback-to-floor calculation. Prior denial,
/// foreign custody and observed undercut stay fail-closed. Trusted callback
/// cleanup does not certify arbitrary Rust destructor termination.
pub fn with_presburger_queries_v4<'work, T, C>(
    limits: PresburgerQueryLimitsV2,
    budget: &mut Budget<'work>,
    consume: C,
) -> Result<T>
where
    C: for<'scope> FnOnce(&mut PresburgerQueryScopeV4<'scope>, &mut Budget<'work>) -> Result<T>,
{
    let cap = PresburgerQueryLimitsV2::default();
    let floor = budget.storage();
    let mut scope = PresburgerQueryScopeV4 {
        state: PresburgerQueryScopeV2 {
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
        },
        session_bytes: 0,
        refund_denied: false,
    };
    let mut credit = Credit { floor, bytes: 0 };
    let mut pending = Some(consume);
    let mut execute = |scope: &mut PresburgerQueryScopeV4<'_>, budget: &mut Budget<'work>| {
        pending
            .take()
            .expect("single owned-scratch session consumer")(scope, budget)
    };
    let execute_bytes = std::mem::size_of_val(&execute);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        scope.state.charge(budget, 4)?;
        let headers = frames::<T>()?
            .checked_add(size_of::<PresburgerQueryScopeV4<'_>>())
            .and_then(|n| n.checked_add(size_of::<Option<C>>()))
            .and_then(|n| n.checked_add(size_of::<Credit>()))
            .and_then(|n| n.checked_add(execute_bytes))
            .and_then(|n| {
                n.checked_add(
                    2 * size_of::<(
                        &mut PresburgerQueryScopeV4<'_>,
                        &mut Budget<'_>,
                        &mut Credit,
                        &mut (),
                        usize,
                    )>(),
                )
            })
            .ok_or(Resource::Arithmetic)?;
        credit.reserve(&mut scope, budget, headers)?;
        scope.state.floor = credit.required()?;
        scope.session_bytes = credit.bytes;
        execute(&mut scope, budget)
    }));
    drop(execute);
    let result = scope
        .state
        .settle(outcome, budget, credit.required().unwrap_or(usize::MAX));
    let _ = credit.observe(&mut scope, budget);
    drain(pending);
    credit.finish(&mut scope, budget, result)
}

#[cfg(test)]
#[path = "pliron_presburger_owned_scratch_v4_tests.rs"]
mod tests;
