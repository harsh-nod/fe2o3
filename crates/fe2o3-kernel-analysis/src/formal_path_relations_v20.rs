//! Caller-owned relation inputs, paid separately from V2/V3 solver scratch.
use super::*;
use crate::{PresburgerAffineNarrowingDecisionV3, PresburgerQueryErrorV2, PresburgerQueryScopeV2};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

type QueryResult<T> = std::result::Result<T, PresburgerQueryErrorV2>;

pub(super) struct InputCredit {
    pub(super) bytes: usize,
}
impl InputCredit {
    pub(super) fn reserve(&mut self, budget: &mut Budget<'_>, bytes: usize) -> QueryResult<()> {
        let next = self.bytes.checked_add(bytes).ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(bytes)?;
        self.bytes = next;
        Ok(())
    }
    pub(super) fn vector<T>(
        &mut self,
        capacity: usize,
        budget: &mut Budget<'_>,
    ) -> QueryResult<Vec<T>> {
        budget.charge_work(1)?;
        self.reserve(
            budget,
            capacity
                .checked_mul(size_of::<T>())
                .and_then(|n| n.checked_add(size_of::<Vec<T>>()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| Resource::Allocation)?;
        self.reserve(
            budget,
            values
                .capacity()
                .checked_sub(capacity)
                .and_then(|n| n.checked_mul(size_of::<T>()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        Ok(values)
    }
    pub(super) fn push<T: Copy>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
        budget: &mut Budget<'_>,
    ) -> QueryResult<()> {
        if values.len() == values.capacity() {
            let next = values
                .capacity()
                .checked_mul(2)
                .ok_or(Resource::Arithmetic)?
                .max(4);
            let mut replacement = self.vector(next, budget)?;
            budget.charge_work(values.len())?;
            replacement.extend_from_slice(values);
            let retired = values
                .capacity()
                .checked_mul(size_of::<T>())
                .and_then(|n| n.checked_add(size_of::<Vec<T>>()))
                .ok_or(Resource::Arithmetic)?;
            let old = std::mem::replace(values, replacement);
            drop(old);
            budget.release_storage(retired)?;
            self.bytes = self
                .bytes
                .checked_sub(retired)
                .ok_or(Resource::Accounting)?;
        }
        budget.charge_work(1)?;
        values.push(value);
        Ok(())
    }
    fn pair(&mut self, values: [i128; 2], budget: &mut Budget<'_>) -> QueryResult<Vec<i128>> {
        let mut result = self.vector(2, budget)?;
        budget.charge_work(2)?;
        result.extend_from_slice(&values);
        Ok(result)
    }
}

fn headers() -> QueryResult<usize> {
    [
        size_of::<InputCredit>(),
        size_of::<PresburgerSetV1>(),
        size_of::<PresburgerBoxV1>(),
        size_of::<Expr>(),
        size_of::<Constraint>(),
        size_of::<QueryResult<PresburgerSetV1>>(),
        size_of::<QueryResult<bool>>(),
        size_of::<std::thread::Result<QueryResult<bool>>>(),
        size_of::<std::thread::Result<()>>(),
        size_of::<(&[Fact], &[Fact], [i128; 2], usize, usize)>(),
    ]
    .into_iter()
    .try_fold(0_usize, |sum, n| sum.checked_add(n))
    .ok_or_else(|| Resource::Arithmetic.into())
}

fn relation(
    left: &[Fact],
    right: &[Fact],
    order: [i128; 2],
    credit: &mut InputCredit,
    budget: &mut Budget<'_>,
) -> QueryResult<PresburgerSetV1> {
    let count = left
        .len()
        .checked_add(right.len())
        .and_then(|n| n.checked_add(1))
        .ok_or(Resource::Arithmetic)?;
    let mut constraints = credit.vector(count, budget)?;
    for (dimension, facts) in [(0, left), (1, right)] {
        for fact in facts {
            budget.charge_work(6)?;
            let mut values = [0; 2];
            values[dimension] = fact.expression.coefficient;
            let coefficients = credit.pair(values, budget)?;
            let expression = Expr::new(fact.expression.constant, coefficients)?;
            constraints.push(if fact.equality {
                Constraint::EqualZero(expression)
            } else {
                Constraint::LessEqualZero(expression)
            });
        }
    }
    budget.charge_work(4)?;
    constraints.push(Constraint::LessEqualZero(Expr::new(
        1,
        credit.pair(order, budget)?,
    )?));
    let lower = credit.pair([0; 2], budget)?;
    let upper = credit.pair([1_i128 << 64; 2], budget)?;
    budget.charge_work(
        count
            .checked_mul(4)
            .and_then(|n| n.checked_add(8))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(PresburgerSetV1::new(
        PresburgerBoxV1::new(lower, upper)?,
        constraints,
    )?)
}

// Only a fixed, noncapturing decision observer enters the solver. No arbitrary
// caller code or caller-owned destructor runs while these input rows are live.
fn one_order(
    left: &[Fact],
    right: &[Fact],
    order: [i128; 2],
    queries: &mut PresburgerQueryScopeV2<'_>,
    budget: &mut Budget<'_>,
) -> QueryResult<bool> {
    budget.check_prior_denials_v1()?;
    let floor = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut credit = InputCredit { bytes: 0 };
    let mut construct = |credit: &mut InputCredit, budget: &mut Budget<'_>| {
        let set = relation(left, right, order, credit, budget)?;
        queries.with_affine_narrowing_v3(&set, budget, |decision, _| {
            Ok(matches!(
                decision,
                PresburgerAffineNarrowingDecisionV3::Empty
            ))
        })
        // `set` and all its owned coefficient/constraint/bound vectors drop
        // before any of their reservation is returned below.
    };
    let frame = headers()?
        .checked_add(std::mem::size_of_val(&construct))
        .and_then(|n| {
            n.checked_add(size_of::<(&mut InputCredit, &mut Budget<'_>, &mut (), usize)>())
        })
        .ok_or(Resource::Arithmetic)?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        credit.reserve(budget, frame)?;
        construct(&mut credit, budget)
    }));
    drop(construct);
    let same =
        slot == std::ptr::from_ref(&*budget) as usize && ledger == budget.work_ledger_identity_v1();
    let first = if same {
        budget.check_prior_denials_v1().err().map(Into::into)
    } else {
        Some(Resource::Accounting.into())
    };
    let result = match result {
        Ok(result) => result,
        Err(payload) => {
            super::paid_scope_v20::drain(payload);
            Err(PresburgerQueryErrorV2::Panicked)
        }
    };
    let result = first.map_or(result, Err);
    let expected = floor.checked_add(credit.bytes);
    let cleanup = if slot != std::ptr::from_ref(&*budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || expected.is_none_or(|expected| budget.storage() < expected)
    {
        Err(Resource::Accounting)
    } else {
        let surplus = Some(budget.storage()) != expected;
        budget.release_storage(credit.bytes).and_then(|()| {
            if surplus {
                Err(Resource::Accounting)
            } else {
                Ok(())
            }
        })
    };
    result.and_then(|value| cleanup.map(|()| value).map_err(Into::into))
}

pub(super) fn exclude(
    left: &[Fact],
    right: &[Fact],
    queries: &mut PresburgerQueryScopeV2<'_>,
    budget: &mut Budget<'_>,
) -> QueryResult<bool> {
    // This observes the original cumulative session identity before allocating
    // caller-side input; a foreign solver is not a fresh budget for this query.
    queries.usage(budget)?;
    budget.check_prior_denials_v1()?;
    budget.charge_work(4)?;
    let count = left
        .len()
        .checked_add(right.len())
        .and_then(|n| n.checked_add(1))
        .ok_or(Resource::Arithmetic)?;
    if count > crate::MAX_PRESBURGER_CONSTRAINTS_V1 {
        return Ok(false);
    }
    let forward = one_order(left, right, [1, -1], queries, budget)?;
    let backward = one_order(left, right, [-1, 1], queries, budget)?;
    Ok(forward && backward)
}

#[cfg(test)]
#[path = "formal_path_relations_v20_tests.rs"]
mod tests;
