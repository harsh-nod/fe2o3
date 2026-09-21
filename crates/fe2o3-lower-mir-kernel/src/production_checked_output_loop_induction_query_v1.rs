//! Inert induction queries retaining genuine LICM source/output custody.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInductionErrorV1 as AnalysisError, CanonicalKirInductionFactsV1 as Facts,
    CanonicalKirInductionRowV1 as Row, CanonicalKirLoopLimitsV1 as Limits,
    CanonicalKirLoopsV1 as Loops,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;

/// Failure of a source-owned, read-only loop-induction query.
#[derive(Debug)]
pub enum ProductionLoopInductionQueryErrorV1 {
    /// The cumulative budget, allocation or same-ledger custody failed.
    Resource(AssertOriginResourceV1),
    /// The genuine retained source and LICM continuation failed replay.
    Prefix(Box<ProductionLicmErrorV1>),
    /// Fresh actual-output inventory, loops or independent facts were refused.
    Analysis(AnalysisError),
    /// The supplied genuine source owner is not the report's borrowed owner.
    ForeignOwner,
    /// One or more of the seven derivation limits changed.
    LimitsMismatch,
    /// Complete freshly checked rows differ from the retained report.
    ReplayMismatch,
    /// A private query phase unwound; no partial report was transferred.
    Panicked,
}
type QError = ProductionLoopInductionQueryErrorV1;
type QResult<T> = Result<T, QError>;
impl From<AssertOriginResourceV1> for QError {
    fn from(error: AssertOriginResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for QError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned loop-induction query: {self:?}")
    }
}
impl Error for QError {}

/// Unreserved addition for the report header and actual copied-row capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionLoopInductionQueryStorageV1(usize);
impl ProductionLoopInductionQueryStorageV1 {
    /// Reserve this addition while the returned borrowed report remains live.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy)]
enum Subject<'a> {
    Direct(&'a ProductionOwnedLicmContinuationV1),
    Erased(&'a ProductionOwnedUnitLocalLicmContinuationV1),
}
impl<'a> Subject<'a> {
    fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::Direct(a), Self::Direct(b)) => std::ptr::eq(a, b),
            (Self::Erased(a), Self::Erased(b)) => std::ptr::eq(a, b),
            _ => false,
        }
    }
    fn output(self) -> &'a StoreOwner {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn floor(self) -> QResult<usize> {
        match self {
            Self::Direct(v) => v.retained_input_storage_floor_v1(),
            Self::Erased(v) => v.retained_input_storage_floor_v1(),
        }
        .map_err(prefix_error)
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> QResult<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(prefix_error)
    }
}
fn prefix_error(error: LError) -> QError {
    match error {
        LError::Resource(error) => QError::Resource(error),
        LError::Panicked => QError::Panicked,
        other => QError::Prefix(Box::new(other)),
    }
}
fn check_binding(binding: &PromotionBinding, budget: &AssertOriginBudgetV1<'_>) -> QResult<()> {
    binding
        .check(budget)
        .map_err(LError::from)
        .map_err(prefix_error)
}
fn query_scoped<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&mut AssertOriginBudgetV1<'w>, &PromotionBinding) -> QResult<T>,
) -> QResult<T> {
    // The inner typed error owns no retained credit. All failed local values
    // drop before licm_scoped restores its actual same-ledger incoming floor.
    match licm_scoped(required, budget, |budget, binding| Ok(run(budget, binding))) {
        Ok(result) => result,
        Err(error) => Err(prefix_error(error)),
    }
}

/// Borrowed source-owned report, not a loop-motion or executed-trip certificate.
///
/// The complete genuine LICM owner stays borrowed. Rows refer to its actual final
/// graph; they never attach a detached graph or invent source spans. Symbolic
/// guard-distance is not an observed execution count, and stronger completion
/// statements remain conditional. No policy, default or launch authority follows.
/// Existing source/ranked/formal replay retains its documented resource domains;
/// this report meters its new header/rows and all actual analysis receipts.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionLoopInductionQueryV1;
/// fn mutate(report: &mut ProductionLoopInductionQueryV1<'_>) { report.rows.clear(); }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedLicmContinuationV1, ProductionLoopInductionQueryV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape(owner: ProductionOwnedLicmContinuationV1, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let (report,_) = owner.derive_loop_induction_facts_v1(Default::default(),budget).unwrap();
///     drop(owner); let _ = report.rows();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionLoopInductionQueryV1;
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
/// fn attach(graph: &VerifiedCanonicalKernelIrModuleV12) { let _ = ProductionLoopInductionQueryV1::from_graph(graph); }
/// ```
pub struct ProductionLoopInductionQueryV1<'owner> {
    subject: Subject<'owner>,
    limits: Limits,
    rows: Vec<Row>,
    retained: usize,
    floor: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
}
impl<'owner> ProductionLoopInductionQueryV1<'owner> {
    /// Complete ordered outcomes, including unsupported recurrences.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    /// Exact actual final graph, borrowed through the genuine source owner.
    pub fn output(&self) -> &'owner StoreOwner {
        self.subject.output()
    }
    /// All seven exact limits used for derivation and subsequent replay.
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    /// Actual report header and retained row capacity, excluding the owner.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Inert analysis never grants transformation, artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Freshly replays the retained genuine source and complete actual-output facts.
    pub fn replay(&self, limits: Limits, budget: &mut AssertOriginBudgetV1<'_>) -> QResult<()> {
        replay(self, self.subject, limits, budget)
    }
}

fn with_fresh_rows<'w, T>(
    subject: Subject<'_>,
    limits: Limits,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    run: impl for<'rows> FnOnce(&'rows [Row], &mut AssertOriginBudgetV1<'w>) -> QResult<T>,
) -> QResult<T> {
    subject.replay(budget)?;
    check_binding(binding, budget)?;
    let (inventory, storage) =
        Inventory::derive(subject.output(), budget).map_err(|e| QError::Analysis(e.into()))?;
    check_binding(binding, budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (loops, storage) =
        Loops::derive(&inventory, limits, budget).map_err(|e| QError::Analysis(e.into()))?;
    check_binding(binding, budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (facts, storage) = Facts::derive(&loops, limits, budget).map_err(QError::Analysis)?;
    check_binding(binding, budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let result = run(facts.rows(), budget);
    check_binding(binding, budget)?;
    // Scratch analyses cannot escape the closed callback. Their backing remains
    // reserved until Rust drops these locals before the outer scope's cleanup.
    result
}
fn check_rows(
    stored: &[Row],
    actual: &[Row],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> QResult<()> {
    budget.charge_work(1)?;
    if stored.len() != actual.len() {
        return Err(QError::ReplayMismatch);
    }
    for (stored, actual) in stored.iter().zip(actual) {
        budget.charge_work(128)?;
        if stored != actual {
            return Err(QError::ReplayMismatch);
        }
    }
    budget.charge_work(3)?;
    Ok(())
}
fn derive<'owner>(
    subject: Subject<'owner>,
    limits: Limits,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> QResult<(
    ProductionLoopInductionQueryV1<'owner>,
    ProductionLoopInductionQueryStorageV1,
)> {
    let required = subject.floor()?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    query_scoped(required, budget, |budget, binding| {
        with_fresh_rows(subject, limits, budget, binding, |actual, budget| {
            let header = size_of::<ProductionLoopInductionQueryV1<'_>>();
            budget.reserve_storage(header)?;
            let requested = actual
                .len()
                .checked_mul(size_of::<Row>())
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
            budget.reserve_storage(requested)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(actual.len())
                .map_err(|_| AssertOriginResourceV1::Allocation)?;
            let bytes = rows
                .capacity()
                .checked_mul(size_of::<Row>())
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
            budget.reserve_storage(
                bytes
                    .checked_sub(requested)
                    .ok_or(AssertOriginResourceV1::Accounting)?,
            )?;
            budget.charge_work(actual.len())?;
            rows.extend_from_slice(actual);
            let retained = header
                .checked_add(bytes)
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
            let report = ProductionLoopInductionQueryV1 {
                subject,
                limits,
                rows,
                retained,
                floor,
                ledger,
            };
            check_rows(&report.rows, actual, budget)?;
            Ok((report, ProductionLoopInductionQueryStorageV1(retained)))
        })
    })
}
fn replay(
    report: &ProductionLoopInductionQueryV1<'_>,
    subject: Subject<'_>,
    limits: Limits,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> QResult<()> {
    budget.charge_work(4)?;
    if !report.subject.same(subject) || !std::ptr::eq(report.output(), subject.output()) {
        return Err(QError::ForeignOwner);
    }
    budget.charge_work(7)?;
    if limits != report.limits {
        return Err(QError::LimitsMismatch);
    }
    budget.charge_work(3)?;
    let required = report
        .floor
        .checked_add(report.retained)
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    if report.ledger != budget.work_ledger_identity_v1()
        || budget.storage() < required
        || budget.storage() < subject.floor()?
    {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    query_scoped(required, budget, |budget, binding| {
        with_fresh_rows(subject, limits, budget, binding, |actual, budget| {
            check_rows(&report.rows, actual, budget)
        })
    })
}
macro_rules! owner_query {
    ($owner:ty,$variant:ident) => {
        impl $owner {
            /// Derives inert facts from this genuine owner's actual final graph.
            /// The returned receipt is additional and initially unreserved.
            pub fn derive_loop_induction_facts_v1(
                &self,
                limits: Limits,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> QResult<(
                ProductionLoopInductionQueryV1<'_>,
                ProductionLoopInductionQueryStorageV1,
            )> {
                derive(Subject::$variant(self), limits, budget)
            }
            /// Rejects any foreign report owner before fresh source/output replay.
            pub fn replay_loop_induction_facts_v1(
                &self,
                report: &ProductionLoopInductionQueryV1<'_>,
                limits: Limits,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> QResult<()> {
                replay(report, Subject::$variant(self), limits, budget)
            }
        }
    };
}
owner_query!(ProductionOwnedLicmContinuationV1, Direct);
owner_query!(ProductionOwnedUnitLocalLicmContinuationV1, Erased);

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    struct PartialRows {
        rows: Vec<u8>,
        dropped: Rc<Cell<bool>>,
    }
    impl Drop for PartialRows {
        fn drop(&mut self) {
            assert_eq!(self.rows, [17; 13]);
            self.dropped.set(true);
        }
    }
    #[test]
    fn loop_induction_nested_typed_error_and_panic_drop_before_same_ledger_floor_cleanup() {
        for panic in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
            let sibling = [0x74u8; 29];
            budget.reserve_storage(sibling.len()).unwrap();
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let dropped = Rc::new(Cell::new(false));
            let result: QResult<()> = query_scoped(floor, &mut budget, |budget, binding| {
                check_binding(binding, budget)?;
                budget.reserve_storage(size_of::<PartialRows>() + 13)?;
                let rows = vec![17; 13];
                budget.reserve_storage(rows.capacity() - 13)?;
                let _partial = PartialRows {
                    rows,
                    dropped: dropped.clone(),
                };
                budget.charge_work(5)?;
                if panic {
                    std::panic::panic_any(19u8);
                }
                Err(QError::ReplayMismatch)
            });
            assert!(matches!(
                (panic, result),
                (false, Err(QError::ReplayMismatch)) | (true, Err(QError::Panicked))
            ));
            assert!(dropped.get());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.work(), 5);
            assert_eq!(sibling, [0x74; 29]);
        }
    }
}
