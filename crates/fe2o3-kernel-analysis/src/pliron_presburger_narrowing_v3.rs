//! Conservative affine-box narrowing on the existing cumulative query ledger.
//! Empty is mathematical evidence only, never compiler or launch authority.

use super::*;

/// An observation about the exact borrowed mathematical input set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresburgerAffineNarrowingDecisionV3<'query> {
    /// Checked affine consequences prove the original set has no point.
    Empty,
    /// All original solutions remain in these bounds. This does not claim that
    /// any solution exists: congruences and residual affine relations remain.
    Bounds {
        /// Inclusive lower bounds in the input coordinate order.
        lower: &'query [i128],
        /// Exclusive upper bounds; 2^64 is represented exactly in i128.
        upper_exclusive: &'query [i128],
    },
}

struct Bounds {
    lower: Vec<i128>,
    upper: Vec<i128>,
}

fn arithmetic<T>(value: Option<T>) -> Result<T> {
    value.ok_or_else(|| PresburgerFailureV1::ArithmeticOverflow.into())
}

fn floor_div(numerator: i128, denominator: i128) -> Result<i128> {
    let quotient = arithmetic(numerator.checked_div(denominator))?;
    let remainder = arithmetic(numerator.checked_rem(denominator))?;
    if remainder != 0 && (remainder < 0) != (denominator < 0) {
        arithmetic(quotient.checked_sub(1))
    } else {
        Ok(quotient)
    }
}

fn ceil_div(numerator: i128, denominator: i128) -> Result<i128> {
    let quotient = arithmetic(numerator.checked_div(denominator))?;
    let remainder = arithmetic(numerator.checked_rem(denominator))?;
    if remainder != 0 && (remainder < 0) == (denominator < 0) {
        arithmetic(quotient.checked_add(1))
    } else {
        Ok(quotient)
    }
}

fn copy_bounds(
    input: &[i128],
    scope: &mut PresburgerQueryScopeV2<'_>,
    budget: &mut Budget<'_>,
) -> Result<Vec<i128>> {
    scope.charge(
        budget,
        input.len().checked_add(4).ok_or(Resource::Arithmetic)?,
    )?;
    let bytes = input
        .len()
        .checked_mul(size_of::<i128>())
        .ok_or(Resource::Arithmetic)?;
    scope.reserve(budget, bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(input.len())
        .map_err(|_| Resource::Allocation)?;
    let excess = values
        .capacity()
        .checked_sub(input.len())
        .and_then(|n| n.checked_mul(size_of::<i128>()))
        .ok_or(Resource::Arithmetic)?;
    scope.reserve(budget, excess)?;
    values.extend_from_slice(input);
    Ok(values)
}

fn endpoint(bounds: &Bounds, dimension: usize, coefficient: i128) -> Result<i128> {
    if coefficient >= 0 {
        Ok(bounds.lower[dimension])
    } else {
        arithmetic(bounds.upper[dimension].checked_sub(1))
    }
}

// The first scan freezes a conservative minimum. Each coordinate is changed
// at most once in this pass, so its own old contribution is still available
// when subtracted. Earlier narrowing of other coordinates only makes the
// frozen remainder more conservative; a quadratic rescan is unnecessary.
fn inequality(
    expression: &super::super::PresburgerAffineExprV1,
    sign: i128,
    bounds: &mut Bounds,
    changed: &mut bool,
    scope: &mut PresburgerQueryScopeV2<'_>,
    budget: &mut Budget<'_>,
) -> Result<bool> {
    scope.charge(budget, 4)?;
    let mut minimum = arithmetic(expression.constant.checked_mul(sign))?;
    for (dimension, coefficient) in expression.coefficients.iter().copied().enumerate() {
        scope.charge(budget, 8)?;
        let coefficient = arithmetic(coefficient.checked_mul(sign))?;
        let contribution =
            arithmetic(coefficient.checked_mul(endpoint(bounds, dimension, coefficient)?))?;
        minimum = arithmetic(minimum.checked_add(contribution))?;
    }
    if minimum > 0 {
        return Ok(true);
    }
    for (dimension, coefficient) in expression.coefficients.iter().copied().enumerate() {
        scope.charge(budget, 12)?;
        let coefficient = arithmetic(coefficient.checked_mul(sign))?;
        if coefficient == 0 {
            continue;
        }
        let contribution =
            arithmetic(coefficient.checked_mul(endpoint(bounds, dimension, coefficient)?))?;
        let remainder = arithmetic(minimum.checked_sub(contribution))?;
        let right = arithmetic(remainder.checked_neg())?;
        let last = arithmetic(bounds.upper[dimension].checked_sub(1))?;
        if coefficient > 0 {
            let upper = floor_div(right, coefficient)?;
            if upper < bounds.lower[dimension] {
                return Ok(true);
            }
            if upper < last {
                bounds.upper[dimension] = arithmetic(upper.checked_add(1))?;
                *changed = true;
            }
        } else {
            let lower = ceil_div(right, coefficient)?;
            if lower > last {
                return Ok(true);
            }
            if lower > bounds.lower[dimension] {
                bounds.lower[dimension] = lower;
                *changed = true;
            }
        }
    }
    Ok(false)
}

fn narrow(
    set: &PresburgerSetV1,
    scope: &mut PresburgerQueryScopeV2<'_>,
    budget: &mut Budget<'_>,
) -> Result<Option<Bounds>> {
    let rank = set.domain.rank();
    scope.charge(budget, rank.checked_add(2).ok_or(Resource::Arithmetic)?)?;
    if set.domain.is_empty() {
        return Ok(None);
    }
    let mut bounds = Bounds {
        lower: copy_bounds(&set.domain.lower, scope, budget)?,
        upper: copy_bounds(&set.domain.upper_exclusive, scope, budget)?,
    };
    loop {
        scope.charge(budget, 1)?;
        let mut changed = false;
        for constraint in &set.constraints {
            scope.charge(budget, 2)?;
            let (expression, equality) = match constraint {
                PresburgerConstraintV1::LessEqualZero(expression) => (expression, false),
                PresburgerConstraintV1::EqualZero(expression) => (expression, true),
                PresburgerConstraintV1::CongruentZero { .. } => continue,
            };
            if inequality(expression, 1, &mut bounds, &mut changed, scope, budget)?
                || (equality
                    && inequality(expression, -1, &mut bounds, &mut changed, scope, budget)?)
            {
                return Ok(None);
            }
        }
        if !changed {
            return Ok(Some(bounds));
        }
    }
}

impl PresburgerQueryScopeV2<'_> {
    /// Conservatively narrows the original finite box using affine inequalities
    /// and both directions of equality. Congruences remain unrefined. The full
    /// legal U64 interval is representable as `[0, 1_i128 << 64)`; this query
    /// does not treat any finite endpoint as mathematical infinity.
    ///
    /// This is a distinct V3 operation. Existing V1/V2 deterministic searches,
    /// query decisions and cost equations are unchanged. Work, query count,
    /// scratch, custody and first failure are shared with this V2 session.
    /// Every sweep is charged; exhaustion or arithmetic overflow is an error,
    /// never Empty. The query stops only at checked emptiness or a fixed point.
    ///
    /// The input set and consumer-owned results remain the caller's accounting
    /// responsibility. Borrowed output bounds live only during the callback.
    /// This does not establish source identity, launch coverage, formal report
    /// completeness, a conflict discharge, or publication authority.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_analysis::{PresburgerAffineNarrowingDecisionV3, PresburgerQueryErrorV2, PresburgerQueryScopeV2, PresburgerSetV1};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape<'a>(scope: &mut PresburgerQueryScopeV2<'_>, set: &PresburgerSetV1, budget: &mut Budget<'_>) -> Result<&'a [i128], PresburgerQueryErrorV2> {
    ///     scope.with_affine_narrowing_v3(set, budget, |decision, _| match decision {
    ///         PresburgerAffineNarrowingDecisionV3::Bounds { lower, .. } => Ok(lower),
    ///         _ => unreachable!(),
    ///     })
    /// }
    /// ```
    pub fn with_affine_narrowing_v3<'work, T, C>(
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
        let mut pending = Some(consume);
        let floor = budget.storage();
        let mut required = floor;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
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
            let headers = frames::<T>()?
                .checked_add(size_of::<Option<C>>())
                .and_then(|n| n.checked_add(size_of::<Option<Bounds>>()))
                .and_then(|n| n.checked_add(size_of::<Result<Option<Bounds>>>()))
                .and_then(|n| n.checked_add(size_of::<Bounds>()))
                .and_then(|n| n.checked_add(size_of::<PresburgerAffineNarrowingDecisionV3<'_>>()))
                .and_then(|n| {
                    n.checked_add(size_of::<(
                        &PresburgerSetV1,
                        &mut PresburgerQueryScopeV2<'_>,
                        &mut Budget<'_>,
                    )>())
                })
                .ok_or(Resource::Arithmetic)?;
            self.reserve(budget, headers)?;
            required = budget.storage();
            validate(set, self, budget)?;
            let narrowed = narrow(set, self, budget)?;
            required = budget.storage();
            let decision = match &narrowed {
                None => PresburgerAffineNarrowingDecisionV3::Empty,
                Some(bounds) => PresburgerAffineNarrowingDecisionV3::Bounds {
                    lower: &bounds.lower,
                    upper_exclusive: &bounds.upper,
                },
            };
            let returned = catch_unwind(AssertUnwindSafe(|| {
                pending.take().expect("one V3 query callback")(decision, budget)
            }));
            let result = self.settle(returned, budget, required);
            drop(narrowed);
            result
        }));
        drain(pending.take());
        let result = self.settle(outcome, budget, required);
        self.refund(result, budget, floor)
    }
}

#[cfg(test)]
#[path = "pliron_presburger_narrowing_v3_tests.rs"]
mod tests;
