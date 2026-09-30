//! Lexical, exact-source value reads from the existing Shared alias analysis.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use nominal_reference_effects_v29::BorrowWork;
use std::{
    cell::Cell,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

/// No memory, effect, or artifact authority is created by this value-read view.
#[derive(Debug)]
pub enum ProductionSemanticSharedReadErrorV1 {
    Resource(Resource),
    Binding,
    Analysis(ProductionSemanticSsaErrorV1),
}
impl fmt::Display for ProductionSemanticSharedReadErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Binding => f.write_str("exact original SSA Shared value-read binding"),
            Self::Analysis(error) => error.fmt(f),
        }
    }
}
impl Error for ProductionSemanticSharedReadErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Analysis(e) => Some(e),
            Self::Binding => None,
        }
    }
}
impl From<Resource> for ProductionSemanticSharedReadErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type Failure = ProductionSemanticSharedReadErrorV1;
type R<T> = Result<T, Failure>;

#[cfg(test)]
#[path = "shared_primitive_reads_v1_tests.rs"]
mod tests;

#[derive(Clone, Copy)]
struct Read {
    borrow: SemanticTransparentBorrowSiteV1,
    source: u32,
    block: u32,
    statement: u32,
    // Compared only, never dereferenced. The view borrows the immutable owner.
    place: usize,
}
impl Read {
    fn key(&self) -> (u32, u32, usize) {
        (self.block, self.statement, self.place)
    }
}
struct Reads {
    rows: Vec<Read>,
    cap: usize,
}
impl adapter::shared_primitive_v29::ReadObserver for Reads {
    fn read(
        &mut self,
        borrow: SemanticTransparentBorrowSiteV1,
        source: u32,
        site: (u32, u32),
        place: &SemanticPlaceV1,
        meter: &mut impl BorrowWork,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        meter.work(1)?;
        if self.rows.len() == self.cap {
            return Err(ProductionSemanticSsaErrorV1::ResourceOverflow);
        }
        self.rows.push(Read {
            borrow,
            source,
            block: site.0,
            statement: site.1,
            place: place as *const SemanticPlaceV1 as usize,
        });
        Ok(())
    }
}
struct Meter<'a, 'w> {
    budget: &'a mut Budget<'w>,
    failure: Option<Resource>,
}
impl BorrowWork for Meter<'_, '_> {
    fn work(&mut self, units: usize) -> Result<(), ProductionSemanticSsaErrorV1> {
        if let Err(error) = self.budget.charge_work(units) {
            self.failure.get_or_insert(error);
            return Err(ProductionSemanticSsaErrorV1::ResourceOverflow);
        }
        Ok(())
    }
}
fn sum(a: usize, b: usize) -> Result<usize, Resource> {
    a.checked_add(b).ok_or(Resource::Arithmetic)
}
fn product(a: usize, b: usize) -> Result<usize, Resource> {
    a.checked_mul(b).ok_or(Resource::Arithmetic)
}
fn height(n: usize) -> usize {
    (usize::BITS - n.max(1).leading_zeros()) as usize + 1
}

/// Fresh per immutable SSA owner/function and ledger, never persisted or cloned.
///
/// The constructor is the only way to obtain this non-forgeable lexical view:
/// ```compile_fail
/// use fe2o3_pliron::ProductionSemanticSharedReadsV1;
/// let fabricated = ProductionSemanticSharedReadsV1 { rows: Vec::new() };
/// ```
pub struct ProductionSemanticSharedReadsV1<'s> {
    owner: &'s ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    rows: Vec<Read>,
    slot: usize,
    ledger: Ledger,
    floor: usize,
    owned: usize,
    failure: Cell<Option<Resource>>,
}

fn headers() -> Result<usize, Resource> {
    // Constructor and reusable allocation-free query/cleanup representations.
    let sizes = [
        size_of::<ProductionSemanticSharedReadsV1<'_>>(),
        size_of::<Option<ProductionSemanticSharedReadsV1<'_>>>(),
        size_of::<R<ProductionSemanticSharedReadsV1<'_>>>(),
        size_of::<std::thread::Result<R<ProductionSemanticSharedReadsV1<'_>>>>(),
        size_of::<Reads>(),
        size_of::<Meter<'_, '_>>(),
        size_of::<Read>(),
        size_of::<BTreeSet<SemanticTransparentBorrowSiteV1>>(),
        size_of::<Vec<bool>>(),
        size_of::<&ProductionSemanticSsaOwnerV1>(),
        size_of::<&AdmittedInertSemanticMirV1>(),
        size_of::<Option<&SemanticFunctionDeclV1>>(),
        size_of::<&SemanticFunctionDeclV1>(),
        size_of::<Option<&ProductionSemanticSsaFunctionPlanV1>>(),
        size_of::<&ProductionSemanticSsaFunctionPlanV1>(),
        size_of::<&[SemanticTypeDeclV1]>(),
        size_of::<&[SsaVariableIdV1]>(),
        size_of::<std::slice::Iter<'_, SsaVariableIdV1>>(),
        size_of::<std::slice::IterMut<'_, Read>>(),
        size_of::<&Read>(),
        size_of::<SemanticTransparentBorrowSiteV1>(),
        size_of::<(u32, u32, usize)>(),
        size_of::<Option<&bool>>(),
        size_of::<Option<u32>>(),
        size_of::<Option<u32>>(),
        size_of::<Result<usize, usize>>(),
        size_of::<R<bool>>(),
        size_of::<R<()>>(),
        size_of::<Result<(), Resource>>(),
        size_of::<Option<Resource>>(),
        size_of::<Failure>(),
        size_of::<Box<dyn std::any::Any + Send>>(),
    ];
    sum(
        sizes.into_iter().try_fold(0, sum)?,
        nominal_reference_effects_v29::scan_headers::<Meter<'_, '_>>()
            .map_err(|_| Resource::Arithmetic)?,
    )
}

impl<'s> ProductionSemanticSharedReadsV1<'s> {
    pub fn try_new(
        owner: &'s ProductionSemanticSsaOwnerV1,
        function: SemanticFunctionIdV1,
        budget: &mut Budget<'_>,
    ) -> R<Self> {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        budget.reserve_storage(headers()?)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            Self::construct(owner, function, floor, budget)
        }));
        match result {
            Ok(Ok(view)) => Ok(view),
            failed => {
                let cleanup = if budget.work_ledger_identity_v1() == ledger {
                    budget
                        .storage()
                        .checked_sub(floor)
                        .ok_or(Resource::Accounting)
                        .and_then(|bytes| budget.release_storage(bytes))
                } else {
                    Err(Resource::Accounting)
                };
                match failed {
                    Ok(Err(error)) => {
                        let _ = cleanup;
                        Err(error)
                    }
                    Err(panic) => resume_unwind(panic),
                    Ok(Ok(_)) => unreachable!(),
                }
            }
        }
    }

    fn construct(
        owner: &'s ProductionSemanticSsaOwnerV1,
        function: SemanticFunctionIdV1,
        floor: usize,
        budget: &mut Budget<'_>,
    ) -> R<Self> {
        budget.charge_work(8)?;
        let semantic = owner.source_semantic();
        let declaration = semantic
            .functions()
            .get(function.index() as usize)
            .ok_or(Failure::Binding)?;
        let plan = owner
            .plan_for_function(function)
            .ok_or(Failure::Binding)?
            .plan();
        let mut meter = Meter {
            budget,
            failure: None,
        };
        let analyzed = (|| {
            if !adapter::shared_primitive_v29::has_candidates(
                declaration,
                semantic.types(),
                &mut meter,
            )? {
                return Ok(None);
            }
            nominal_reference_effects_v29::scan_size(declaration, 0, &mut meter).map(Some)
        })();
        let dimensions = analyzed.map_err(|error| {
            meter
                .failure
                .map(Failure::Resource)
                .unwrap_or(Failure::Analysis(error))
        })?;
        let mut rows = Vec::new();
        if let Some((units, candidates)) = dimensions {
            // Same bounded alias-analysis scratch contract as the planner, in
            // logical machine words, plus a direct promotion membership mask.
            let mut scratch = sum(
                product(
                    sum(
                        sum(128, sum(product(units, 96)?, product(candidates, 64)?)?)?,
                        adapter::shared_primitive_v29::liveness_scratch_words(declaration)
                            .map_err(Failure::Analysis)?,
                    )?,
                    size_of::<usize>(),
                )?,
                product(declaration.locals().len(), size_of::<bool>())?,
            )?;
            meter.budget.reserve_storage(scratch)?;
            let row_bytes = product(units, size_of::<Read>())?;
            meter.budget.reserve_storage(row_bytes)?;
            rows.try_reserve_exact(units)
                .map_err(|_| Resource::Allocation)?;
            let actual = product(rows.capacity(), size_of::<Read>())?;
            if actual > row_bytes {
                meter.budget.reserve_storage(actual - row_bytes)?;
            }
            let mut observer = Reads { rows, cap: units };
            let analyzed = adapter::shared_primitive_v29::analyze_observed(
                declaration,
                semantic.types(),
                units,
                &mut meter,
                &mut observer,
            );
            let accepted = analyzed.map_err(|error| {
                meter
                    .failure
                    .map(Failure::Resource)
                    .unwrap_or(Failure::Analysis(error))
            })?;
            meter.budget.charge_work(sum(
                declaration.locals().len(),
                plan.promoted_variables().len(),
            )?)?;
            let mut promoted = Vec::new();
            promoted
                .try_reserve_exact(declaration.locals().len())
                .map_err(|_| Resource::Allocation)?;
            let extra = promoted
                .capacity()
                .checked_sub(declaration.locals().len())
                .ok_or(Resource::Accounting)?;
            if extra != 0 {
                meter.budget.reserve_storage(extra)?;
                scratch = sum(scratch, extra)?;
            }
            promoted.resize(declaration.locals().len(), false);
            for variable in plan.promoted_variables() {
                *promoted
                    .get_mut(variable.get() as usize)
                    .ok_or(Failure::Binding)? = true;
            }
            meter.budget.charge_work(product(
                observer.rows.len(),
                sum(4, height(accepted.len()))?,
            )?)?;
            observer.rows.retain(|row| {
                accepted.contains(&row.borrow) && promoted.get(row.source as usize) == Some(&true)
            });
            meter
                .budget
                .charge_work(product(observer.rows.len(), height(observer.rows.len()))?)?;
            observer.rows.sort_unstable_by_key(Read::key);
            rows = observer.rows;
            drop(promoted);
            drop(accepted);
            meter.budget.release_storage(scratch)?;
        }
        let budget = meter.budget;
        Ok(Self {
            owner,
            function,
            rows,
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor,
            owned: budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
            failure: Cell::new(None),
        })
    }

    fn retain<T>(&self, result: R<T>) -> R<T> {
        if let Err(Failure::Resource(error)) = &result {
            if self.failure.get().is_none() {
                self.failure.set(Some(*error));
            }
        }
        result
    }

    /// Exact original ordinary operand, not a structural or copied locator.
    pub fn contains(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        function: SemanticFunctionIdV1,
        declaration: &SemanticFunctionDeclV1,
        block: usize,
        statement: Option<usize>,
        place: &SemanticPlaceV1,
        budget: &mut Budget<'_>,
    ) -> R<bool> {
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        self.retain((|| {
            if self.slot != budget as *const Budget<'_> as usize
                || self.ledger != budget.work_ledger_identity_v1()
                || budget.storage() < sum(self.floor, self.owned)?
            {
                return Err(Resource::Accounting.into());
            }
            if !std::ptr::eq(owner, self.owner)
                || function != self.function
                || !std::ptr::eq(
                    declaration,
                    &owner.source_semantic().functions()[function.index() as usize],
                )
            {
                return Err(Failure::Binding);
            }
            budget.charge_work(sum(12, height(self.rows.len()))?)?;
            let (Ok(block), Some(statement)) = (
                u32::try_from(block),
                statement.and_then(|s| u32::try_from(s).ok()),
            ) else {
                return Ok(false);
            };
            Ok(self
                .rows
                .binary_search_by_key(
                    &(block, statement, place as *const SemanticPlaceV1 as usize),
                    Read::key,
                )
                .is_ok())
        })())
    }

    /// Drops this view before refunding only its own retained storage.
    pub fn release(self, budget: &mut Budget<'_>) -> R<()> {
        let first = self.failure.get();
        let intact = self.slot == budget as *const Budget<'_> as usize
            && self.ledger == budget.work_ledger_identity_v1()
            && budget.storage() >= sum(self.floor, self.owned)?;
        let bytes = self.owned;
        drop(self);
        let cleanup = if intact {
            budget.release_storage(bytes)
        } else {
            Err(Resource::Accounting)
        };
        if let Some(error) = first {
            return Err(error.into());
        }
        cleanup.map_err(Failure::Resource)
    }
}

// Inert concrete observer owner; ordinary SharedReads construction is unchanged.
#[allow(dead_code)]
#[path = "shared_primitive_reads_retained_observer_v1.rs"]
mod retained_observer;
#[allow(unused_imports)]
pub(super) use retained_observer::RetainedSharedObserverV1;

// Sealed row ownership transfer shared by concrete engine and actual wrapper.
pub(super) struct RetainedSharedRowsV1 {
    rows: Vec<Read>,
    transferred: bool,
}
#[path = "shared_primitive_reads_retained_v1.rs"]
mod retained_wrapper;
pub use retained_wrapper::ProductionSemanticSharedReadsPreparationV1;
