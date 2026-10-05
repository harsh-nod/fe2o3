//! Paid terminal-pair shape only; no native reports or source predicate proof.
use super::*;

/// Exact owner-bound trap shapes, borrowed only within a paid callback.
/// This is neither source truth nor a completed fixed-nine policy view.
///
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalTrapShapeV1;
/// fn forge() { let _ = CheckedCanonicalTrapShapeV1 {}; }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalTrapShapeV1;
/// fn copy(x: &CheckedCanonicalTrapShapeV1<'_, '_>) { let _ = (*x).clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::{CheckedCanonicalTrapShapeV1, CheckedCanonicalTrapPoliciesV1};
/// fn promote<'a, 'g>(x: CheckedCanonicalTrapShapeV1<'a, 'g>) -> CheckedCanonicalTrapPoliciesV1<'a, 'g> { x }
/// ```
pub struct CheckedCanonicalTrapShapeV1<'i, 'g> {
    facts: &'i CanonicalTrapPairsGraphFactsV1<'i, 'g>,
    guard: &'i Guard,
}
impl<'i, 'g> CheckedCanonicalTrapShapeV1<'i, 'g> {
    /// The actual inventory owner, not a same-byte replacement.
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g Owner, Failure> {
        self.guard.query(budget)?;
        Ok(self.facts.inventory.owner())
    }
    /// Complete module function roster, including the exact trap declaration.
    pub fn module_function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.facts.inventory.functions().len())
    }
    /// Actual body definitions; no declaration is counted as a report producer.
    pub fn definition_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.facts.definitions.len())
    }
    /// A definition's exact module coordinate.
    pub fn definition_coordinate(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<FunctionCoordinate, Failure> {
        self.guard.query(budget)?;
        self.facts
            .definitions
            .get(ordinal)
            .copied()
            .ok_or_else(|| self.guard.invalid(ordinal))
    }
    /// Number of actual retained diagnostic-call/Unreachable pairs.
    pub fn pair_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.facts.pairs.len())
    }
    /// Borrow a complete pair and its ordered incoming range.
    pub fn pair(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&CanonicalTrapPairV1, Failure> {
        self.guard.query(budget)?;
        self.facts
            .pairs
            .get(ordinal)
            .ok_or_else(|| self.guard.invalid(ordinal))
    }
    /// Borrow an exact condition and both ordered successor occurrences.
    pub fn incoming_edge(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&CanonicalTrapIncomingEdgeV1<'i, 'g>, Failure> {
        self.guard.query(budget)?;
        self.facts
            .incoming
            .get(ordinal)
            .ok_or_else(|| self.guard.invalid(ordinal))
    }
}

/// Reuse the complete trap census without invoking native policy producers.
/// The foundation view authenticates its exact paid inventory floor before any
/// shape scratch is reserved. All backing dies before the entry floor is restored.
///
/// ```compile_fail
/// use fe2o3_pliron::with_canonical_trap_shape_v1;
/// use fe2o3_kernel_analysis::CheckedCanonicalRankedViewV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape(view: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>, budget: &mut Budget<'_>) {
///     with_canonical_trap_shape_v1(view, budget, |shape, _| Ok(shape));
/// }
/// ```
pub fn with_canonical_trap_shape_v1<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    budget: &mut Budget<'w>,
    callback: impl for<'i, 'g> FnOnce(
        &CheckedCanonicalTrapShapeV1<'i, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, Failure> {
    protected(budget, |budget| {
        let inventory = checked.inventory(budget)?;
        let unwind = size_of::<std::thread::Result<Result<T, Failure>>>()
            .checked_mul(2)
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(checked_add(
            checked_add(
                size_of::<CheckedCanonicalTrapShapeV1<'_, '_>>(),
                size_of::<Guard>(),
            )?,
            unwind,
        )?)?;
        let facts = CanonicalTrapPairsGraphFactsV1::derive(inventory, budget)?;
        let guard = Guard::new(budget);
        let result = {
            let view = CheckedCanonicalTrapShapeV1 {
                facts: &facts,
                guard: &guard,
            };
            guard.callback(budget, |budget| callback(&view, budget))
        };
        drop(facts);
        result
    })
}

#[cfg(test)]
#[path = "canonical_trap_shape_v1_tests.rs"]
mod tests;
