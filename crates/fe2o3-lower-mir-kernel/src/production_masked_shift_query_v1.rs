//! Scoped live-resource adapter for inert semantic masked-shift queries.
//! No assertion admission, source-custody, SSA, or native rule is changed here.

use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
    CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticBlockIdV1, SemanticFunctionIdV1,
};
use fe2o3_mir_model::{
    SemanticMaskedShiftErrorV1, SemanticMaskedShiftFactV1, SemanticMaskedShiftIndexV1,
    SemanticMaskedShiftLimitsV1, SemanticMaskedShiftMeterV1, SemanticMaskedShiftMeteredErrorV1,
};

/// Exact failure from the scoped query or its unchanged live resource meter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticMaskedShiftQueryErrorV1 {
    /// The dependency-neutral query rejected its model or local limit.
    Analysis(SemanticMaskedShiftErrorV1),
    /// The live budget failed, or its slot, ledger, or reserved floor changed.
    Resource(ResourceError),
    /// Construction or the callback unwound; owned query storage has been dropped.
    Panicked,
}

impl fmt::Display for ProductionSemanticMaskedShiftQueryErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::Panicked => f.write_str("scoped semantic masked-shift query panicked"),
        }
    }
}

impl Error for ProductionSemanticMaskedShiftQueryErrorV1 {}

impl From<ResourceError> for ProductionSemanticMaskedShiftQueryErrorV1 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

type R<T> = Result<T, ProductionSemanticMaskedShiftQueryErrorV1>;

struct Accounting {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    reserved: usize,
    failure: Option<ProductionSemanticMaskedShiftQueryErrorV1>,
    cleanup_allowed: bool,
}

impl Accounting {
    fn new(budget: &Budget<'_>) -> Self {
        Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            reserved: 0,
            failure: None,
            cleanup_allowed: true,
        }
    }

    fn fail(
        &mut self,
        error: ProductionSemanticMaskedShiftQueryErrorV1,
    ) -> ProductionSemanticMaskedShiftQueryErrorV1 {
        *self.failure.get_or_insert(error)
    }

    fn check(&mut self, budget: &Budget<'_>) -> R<()> {
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            // Even a later restoration must not authorize writes to a substituted
            // ledger or an undercut caller floor after an observed violation.
            self.cleanup_allowed = false;
            self.failure = Some(ResourceError::Accounting.into());
        } else if self.floor.checked_add(self.reserved) != Some(budget.storage()) {
            self.failure = Some(ResourceError::Accounting.into());
        }
        self.failure.map_or(Ok(()), Err)
    }

    fn model_error(
        &mut self,
        error: SemanticMaskedShiftMeteredErrorV1<ResourceError>,
    ) -> ProductionSemanticMaskedShiftQueryErrorV1 {
        let error = match error {
            SemanticMaskedShiftMeteredErrorV1::Analysis(error) => {
                ProductionSemanticMaskedShiftQueryErrorV1::Analysis(error)
            }
            SemanticMaskedShiftMeteredErrorV1::Meter(error) => error.into(),
        };
        self.fail(error)
    }
}

struct Meter<'a, 'work> {
    accounting: &'a mut Accounting,
    budget: &'a mut Budget<'work>,
}

impl SemanticMaskedShiftMeterV1 for Meter<'_, '_> {
    type Error = ResourceError;

    fn charge_work(&mut self, amount: usize) -> Result<(), ResourceError> {
        self.accounting
            .check(self.budget)
            .map_err(|_| ResourceError::Accounting)?;
        self.budget.charge_work(amount).inspect_err(|error| {
            self.accounting.fail((*error).into());
        })
    }

    fn reserve_storage(&mut self, amount: usize) -> Result<(), ResourceError> {
        self.accounting
            .check(self.budget)
            .map_err(|_| ResourceError::Accounting)?;
        let reserved = self
            .accounting
            .reserved
            .checked_add(amount)
            .ok_or_else(|| {
                self.accounting.fail(ResourceError::Arithmetic.into());
                ResourceError::Arithmetic
            })?;
        self.budget.reserve_storage(amount).inspect_err(|error| {
            self.accounting.fail((*error).into());
        })?;
        self.accounting.reserved = reserved;
        Ok(())
    }
}

/// A borrowed facade that cannot outlive its resource-owning scope.
///
/// It exposes only inert source-borrowed facts, never the underlying index. Every
/// lookup checks the same live budget slot, Work ledger, and exact query floor.
/// A failed lookup poisons the scope even if its caller ignores the error.
pub struct ProductionSemanticMaskedShiftQueryV1<'scope, 'source> {
    index: &'scope mut SemanticMaskedShiftIndexV1<'source>,
    accounting: &'scope mut Accounting,
}

impl<'source> ProductionSemanticMaskedShiftQueryV1<'_, 'source> {
    /// Queries one actual assertion occurrence, preserving the source borrow only.
    pub fn assertion(
        &mut self,
        block: SemanticBlockIdV1,
        budget: &mut Budget<'_>,
    ) -> R<Option<SemanticMaskedShiftFactV1<'source>>> {
        self.accounting.check(budget)?;
        let result = self.index.assertion_metered(
            block,
            &mut Meter {
                accounting: self.accounting,
                budget,
            },
        );
        result.map_err(|error| self.accounting.model_error(error))
    }

    /// Queries one actual shift occurrence, preserving the source borrow only.
    pub fn shift(
        &mut self,
        block: SemanticBlockIdV1,
        statement: u32,
        budget: &mut Budget<'_>,
    ) -> R<Option<SemanticMaskedShiftFactV1<'source>>> {
        self.accounting.check(budget)?;
        let result = self.index.shift_metered(
            block,
            statement,
            &mut Meter {
                accounting: self.accounting,
                budget,
            },
        );
        result.map_err(|error| self.accounting.model_error(error))
    }
}

/// Runs one once-per-function inert query against a live canonical resource budget.
///
/// Requested bytes and actual capacity excess are charged before initialization.
/// Construction scratch is conservatively retained at its peak through the scope.
/// All query tables are dropped before restoring the inherited storage floor.
/// Work, peak storage, and failed-storage history are never reset.
///
/// The callback may charge work or use temporary storage, but must restore all
/// temporary reservations before a query lookup and before returning. Output
/// tables must be prepaid outside this scope or use a separate explicit transfer
/// contract. Additional exit reservations reject and drop the callback result.
/// Budget replacement or an observed entry-floor undercut rejects without further
/// charging/releasing that ledger; its owner must handle recovery explicitly.
/// Caught panic payloads are destroyed only after cleanup; a malicious payload
/// destructor can itself unwind, but cannot skip that cleanup.
///
/// This adapter establishes resource ownership only. Consumers still authenticate
/// retained source, source initialization/use, assertion elision, and native facts.
/// No missing fact or resource failure authorizes a compiler transformation.
///
/// The resource-owning facade cannot escape; facts can retain only `source`:
/// ```
/// use fe2o3_lower_mir_kernel::{
///     with_production_semantic_masked_shift_query_v1, ProductionSemanticMaskedShiftQueryErrorV1,
/// };
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// use fe2o3_mir_model::{SemanticMaskedShiftFactV1, SemanticMaskedShiftLimitsV1};
/// use fe2o3_mir_model::semantic_mir_v1::{
///     AdmittedInertSemanticMirV1, SemanticBlockIdV1, SemanticFunctionIdV1,
/// };
/// fn source_fact_can_escape<'source>(
///     source: &'source AdmittedInertSemanticMirV1,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) -> Result<Option<SemanticMaskedShiftFactV1<'source>>, ProductionSemanticMaskedShiftQueryErrorV1> {
///     with_production_semantic_masked_shift_query_v1(
///         source, SemanticFunctionIdV1::from_index(0), SemanticMaskedShiftLimitsV1::default(), budget,
///         |query, budget| query.assertion(SemanticBlockIdV1::from_index(0), budget),
///     )
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::with_production_semantic_masked_shift_query_v1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// use fe2o3_mir_model::SemanticMaskedShiftLimitsV1;
/// use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticFunctionIdV1};
/// fn cannot_escape(source: &AdmittedInertSemanticMirV1, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _escaped = with_production_semantic_masked_shift_query_v1(
///         source, SemanticFunctionIdV1::from_index(0), SemanticMaskedShiftLimitsV1::default(), budget,
///         |query, _| Ok(query),
///     );
/// }
/// ```
/// The index is not available for unmetered lookups or ownership transfer:
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionSemanticMaskedShiftQueryV1;
/// fn no_raw_index(query: &mut ProductionSemanticMaskedShiftQueryV1<'_, '_>) {
///     let _index = &mut query.index;
/// }
/// ```
pub fn with_production_semantic_masked_shift_query_v1<'source, 'work, T>(
    source: &'source AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    limits: SemanticMaskedShiftLimitsV1,
    budget: &mut Budget<'work>,
    run: impl for<'scope> FnOnce(
        &mut ProductionSemanticMaskedShiftQueryV1<'scope, 'source>,
        &mut Budget<'work>,
    ) -> R<T>,
) -> R<T> {
    let mut accounting = Accounting::new(budget);
    let mut index = None;
    let mut deferred_panic = None;
    let mut result = match catch_unwind(AssertUnwindSafe(|| {
        index = Some(
            SemanticMaskedShiftIndexV1::analyze_metered(
                source,
                function,
                limits,
                &mut Meter {
                    accounting: &mut accounting,
                    budget,
                },
            )
            .map_err(|error| accounting.model_error(error))?,
        );
        let mut query = ProductionSemanticMaskedShiftQueryV1 {
            index: index.as_mut().expect("query index was just constructed"),
            accounting: &mut accounting,
        };
        run(&mut query, budget)
    })) {
        Ok(result) => result,
        Err(payload) => {
            deferred_panic = Some(payload);
            Err(ProductionSemanticMaskedShiftQueryErrorV1::Panicked)
        }
    };

    if let Err(error) = accounting.check(budget) {
        // A returned owner may have a destructor. Its rejection cannot skip the
        // query's cleanup, even if that destructor itself unwinds.
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(result))) {
            // A prior callback panic leaves only a plain error, whose drop
            // cannot panic, so this never replaces an earlier payload.
            deferred_panic = Some(payload);
        }
        result = Err(error);
    }
    drop(index);
    if accounting.cleanup_allowed {
        // The entry floor cannot be undercut here: check() records that condition
        // and disables cleanup. Extra same-ledger reservations reject above.
        let release = budget
            .storage()
            .checked_sub(accounting.floor)
            .ok_or(ResourceError::Accounting)
            .and_then(|bytes| budget.release_storage(bytes));
        if let Err(error) = release {
            drop(result);
            drop(deferred_panic);
            return Err(error.into());
        }
    }
    drop(deferred_panic);
    result
}

#[cfg(test)]
#[path = "production_masked_shift_query_v1_tests.rs"]
mod tests;
