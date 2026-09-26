//! Live-budget ownership for source-borrowed semantic assertion queries.
//! Canonical source correspondence and trap admission are separate consumers.

use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticBlockIdV1, SemanticFunctionIdV1,
};
use fe2o3_mir_model::{
    SemanticAssertionAnalysisV1 as Analysis, SemanticAssertionErrorV1 as AnalysisError,
    SemanticAssertionLimitsV1 as Limits, SemanticAssertionMeterV1,
    SemanticAssertionMeteredErrorV1 as MeteredError, SemanticAssertionOutcomeV1 as Outcome,
};

/// A source-model, live-resource or unwind refusal; never a success fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticAssertionQueryErrorV1 {
    /// The shared semantic analysis refused its model or local resource limit.
    Analysis(AnalysisError),
    /// The caller's budget refused work/storage or its identity/floor changed.
    Resource(Resource),
    /// Construction, querying or callback execution unwound.
    Panicked,
}
impl fmt::Display for ProductionSemanticAssertionQueryErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::Panicked => f.write_str("scoped semantic assertion query panicked"),
        }
    }
}
impl Error for ProductionSemanticAssertionQueryErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Analysis(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Panicked => None,
        }
    }
}
impl From<Resource> for ProductionSemanticAssertionQueryErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type R<T> = Result<T, ProductionSemanticAssertionQueryErrorV1>;

#[path = "production_semantic_assertion_query_resources_v1.rs"]
mod resources;
use resources::{Accounting, Meter};

/// Once-per-function shared analysis bound to one live resource scope.
///
/// The index is private. Every lookup requires the same Budget slot, ledger and
/// exact reserved floor. Ignoring a failed lookup does not clear its refusal.
pub struct ProductionSemanticAssertionQueryV1<'scope, 'source> {
    analysis: &'scope mut Analysis<'source>,
    accounting: &'scope mut Accounting,
}
impl<'source> ProductionSemanticAssertionQueryV1<'_, 'source> {
    /// Derive the actual source assertion's outcome, retaining the query borrow.
    /// NotProved, Refuted and NotAnAssertion are distinct from a proved fact.
    pub fn assertion<'query>(
        &'query mut self,
        block: SemanticBlockIdV1,
        budget: &mut Budget<'_>,
    ) -> R<Outcome<'query, 'source>> {
        self.accounting.check(budget)?;
        self.accounting.charge(budget, 1)?;
        let result = self.analysis.assertion_at_v1(
            block,
            &mut Meter {
                accounting: self.accounting,
                budget,
            },
        );
        result.map_err(|error| self.accounting.model_error(error))
    }
}

/// Query one actual admitted source function without exporting the analysis.
///
/// The caller retains the source owner's reservation and prepays any escaping
/// callback-owned payload. Query backing, temporary construction capacity and
/// wrapper headers remain paid until destruction. The model pays its own
/// Analysis header; this wrapper does not reserve that header a second time.
/// Caller scratch must be restored before a lookup and before callback return.
/// A recursively panicking panic-payload destructor can propagate its panic;
/// a drop guard still restores the uncontaminated caller ledger on that unwind.
/// A substituted ledger or observed caller-floor undercut disables cleanup of
/// that ledger. Work and peak/denial history are never refunded or reset.
///
/// This scope owns resources, not canonical source custody or execution rights.
/// Consumers must independently join actual source facts to graph occurrences.
/// No caller-supplied predicate/proof set is accepted. BoundsNeedsIndependentRule
/// remains unresolved unless a separate authenticated bounds consumer proves it.
///
/// The facade and its borrowed facts cannot escape the paid scope:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::with_production_semantic_assertion_query_v1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// use fe2o3_mir_model::SemanticAssertionLimitsV1;
/// use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticFunctionIdV1};
/// fn escape(source: &AdmittedInertSemanticMirV1, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = with_production_semantic_assertion_query_v1(
///         source, SemanticFunctionIdV1::from_index(0), SemanticAssertionLimitsV1::new(1000, 1000),
///         budget, |query, _| Ok(query),
///     );
/// }
/// ```
/// A source fact also retains the paid query borrow:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::with_production_semantic_assertion_query_v1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// use fe2o3_mir_model::SemanticAssertionLimitsV1;
/// use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticFunctionIdV1, SemanticBlockIdV1};
/// fn escape(source: &AdmittedInertSemanticMirV1, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = with_production_semantic_assertion_query_v1(
///         source, SemanticFunctionIdV1::from_index(0), SemanticAssertionLimitsV1::new(1000, 1000),
///         budget, |query, budget| query.assertion(SemanticBlockIdV1::from_index(0), budget),
///     );
/// }
/// ```
/// The raw shared analysis is not available for detached or unmetered queries:
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionSemanticAssertionQueryV1;
/// fn raw(query: &mut ProductionSemanticAssertionQueryV1<'_, '_>) {
///     let _ = &mut query.analysis;
/// }
/// ```
pub fn with_production_semantic_assertion_query_v1<'source, 'work, T>(
    source: &'source AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    limits: Limits,
    budget: &mut Budget<'work>,
    run: impl for<'scope> FnOnce(
        &mut ProductionSemanticAssertionQueryV1<'scope, 'source>,
        &mut Budget<'work>,
    ) -> R<T>,
) -> R<T> {
    let mut accounting = Accounting::new(budget);
    let result = catch_unwind(AssertUnwindSafe(|| {
        accounting.charge(budget, 2)?;
        let headers = std::mem::size_of::<Accounting>()
            .checked_add(std::mem::size_of::<
                ProductionSemanticAssertionQueryV1<'_, '_>,
            >())
            .and_then(|n| n.checked_add(std::mem::size_of::<std::thread::Result<R<T>>>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<Outcome<'_, '_>>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<resources::Cleanup<'_, '_>>()))
            .ok_or_else(|| accounting.fail(Resource::Arithmetic.into()))?;
        accounting.reserve(budget, headers)?;
        let function = source
            .functions()
            .get(function.index() as usize)
            .ok_or_else(|| {
                accounting.fail(ProductionSemanticAssertionQueryErrorV1::Analysis(
                    AnalysisError::InvalidModel("function is outside the admitted source owner"),
                ))
            })?;
        let mut analysis = Analysis::new_metered(
            source.types(),
            function,
            limits,
            &mut Meter {
                accounting: &mut accounting,
                budget,
            },
        )
        .map_err(|error| accounting.model_error(error))?;
        let returned = catch_unwind(AssertUnwindSafe(|| {
            run(
                &mut ProductionSemanticAssertionQueryV1 {
                    analysis: &mut analysis,
                    accounting: &mut accounting,
                },
                budget,
            )
        }));
        let check = accounting.check(budget);
        // Callback values and panic payloads are rejected while all backing is
        // still paid. The outer catch also covers a rejected value's destructor.
        if let Err(error) = check {
            drop(returned);
            return Err(error);
        }
        match returned {
            Ok(value) => value,
            Err(payload) => {
                drop(payload);
                Err(ProductionSemanticAssertionQueryErrorV1::Panicked)
            }
        }
    }));
    // Analysis/backing has dropped. A cleanup guard also covers a destructor
    // that panics while retiring an already-caught panic payload.
    accounting.finish(budget, result)
}

#[cfg(test)]
#[path = "production_semantic_assertion_query_v1_tests.rs"]
mod tests;
