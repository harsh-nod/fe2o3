//! Lexical cleanup around the existing-owner path consumer; no final authority.
use super::paid_engine_v20::{
    FormalPaidPathErrorV20 as Error, FormalPaidPathObservationV20 as Observation, PaidEngine,
    Queries,
};
use super::paid_relations_v20::InputCredit;
use super::*;
use crate::PresburgerQueryScopeV2;
use fe2o3_kernel_ir::{
    BasicBlock, CanonicalFormalLaunchInputV19, CanonicalFormalSourceScopeV20,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, VerifiedCanonicalKernelIrModuleV18,
};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};
type PathResult<T> = std::result::Result<T, Error>;

/// Original report plus ordered conditional path observations. This borrows
/// the genuine report rows; it cannot clear reasons, supply missing pairs or
/// authorize final memory, target lowering or publication.
pub struct FormalPaidPathViewV20<'view> {
    owner: &'view VerifiedCanonicalKernelIrModuleV18,
    function: &'view Function,
    root_index: usize,
    launch: CanonicalFormalLaunchInputV19,
    width: FormalIndexWidth,
    report: &'view FormalMemoryObligationAnalysis,
    observations: &'view [Observation],
}
impl FormalPaidPathViewV20<'_> {
    /// Exact borrowed original canonical owner.
    pub fn original_owner(&self) -> &VerifiedCanonicalKernelIrModuleV18 {
        self.owner
    }
    /// Actual original entry function.
    pub fn original_function(&self) -> &Function {
        self.function
    }
    /// Original kernel ordinal in the owner's roster.
    pub fn root_index(&self) -> usize {
        self.root_index
    }
    /// Original descriptive launch input, not launch authority.
    pub fn launch_input(&self) -> CanonicalFormalLaunchInputV19 {
        self.launch
    }
    /// Original descriptive logical index width.
    pub fn index_width(&self) -> FormalIndexWidth {
        self.width
    }
    /// Exact unchanged report, retaining all reasons and conflict rows.
    pub fn analysis(&self) -> &FormalMemoryObligationAnalysis {
        self.report
    }
    /// One fixed-size observation per original conflict, in original order.
    pub fn observations(&self) -> &[Observation] {
        self.observations
    }
}

impl<'owner> Queries<'owner> for CanonicalFormalSourceScopeV20<'_, 'owner> {
    fn analysis(&self) -> &FormalMemoryObligationAnalysis {
        self.report()
    }
    fn original_owner(&self) -> &VerifiedCanonicalKernelIrModuleV18 {
        self.original_owner()
    }
    fn original_function(&self) -> &Function {
        self.original_function()
    }
    fn root_index(&self) -> usize {
        self.root_index()
    }
    fn launch(&self) -> CanonicalFormalLaunchInputV19 {
        self.launch_input()
    }
    fn width(&self) -> FormalIndexWidth {
        self.index_width()
    }
    fn check(&self, budget: &mut Budget<'_>) -> PathResult<()> {
        Ok(self.check(self.original_owner(), self.root_index(), budget)?)
    }
    fn block_count(&self, budget: &mut Budget<'_>) -> PathResult<usize> {
        Ok(self.block_count(budget)?)
    }
    fn block_at(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> PathResult<Option<&'owner BasicBlock>> {
        Ok(self.block_at(ordinal, budget)?)
    }
    fn block_ordinal(&self, block: BlockId, budget: &mut Budget<'_>) -> PathResult<Option<usize>> {
        Ok(self.block_ordinal(block, budget)?)
    }
    fn reachable(&self, block: BlockId, budget: &mut Budget<'_>) -> PathResult<bool> {
        Ok(self.reachable(block, budget)?)
    }
    fn definition_count(&self, budget: &mut Budget<'_>) -> PathResult<usize> {
        Ok(self.definition_count(budget)?)
    }
    fn definition(
        &self,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> PathResult<Option<(usize, Option<&'owner Operation>)>> {
        Ok(self
            .definition(value, budget)?
            .map(|(ordinal, _, operation)| (ordinal, operation)))
    }
    fn unique_predecessor_dominates(
        &self,
        source: BlockId,
        target: BlockId,
        access: BlockId,
        budget: &mut Budget<'_>,
    ) -> PathResult<bool> {
        Ok(self.unique_predecessor_dominates(source, target, access, budget)?)
    }
}

/// Observes control-path exclusion for the exact original report, using its
/// live retained CFG/definition indices and the caller's cumulative V2 session.
///
/// Only complete Bits64 D1 reports enter the full-coordinate proof grammar;
/// every other recorded conflict remains `NotProved`. A proved row does not
/// supply missing full-domain conflict rows, launch/native authority, or final
/// memory/publication admission. Neither the source owner nor its CFG is copied.
/// The callback is trusted compiler code, not an arbitrary-Rust sandbox.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::*;
/// use fe2o3_kernel_ir::*;
/// fn escape(source: &CanonicalFormalSourceScopeV20<'_, '_>, queries: &mut PresburgerQueryScopeV2<'_>, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) -> &'static [FormalPaidPathObservationV20] {
///     let mut escaped = None;
///     with_formal_path_observations_v20(source, queries, budget, |view, _| {
///         escaped = Some(view.observations());
///         Ok(())
///     }).unwrap();
///     escaped.unwrap()
/// }
/// ```
pub fn with_formal_path_observations_v20<'work>(
    source: &CanonicalFormalSourceScopeV20<'_, '_>,
    queries: &mut PresburgerQueryScopeV2<'_>,
    budget: &mut Budget<'work>,
    consume: impl for<'view> FnOnce(&FormalPaidPathViewV20<'view>, &mut Budget<'work>) -> PathResult<()>,
) -> PathResult<()> {
    with_queries(source, queries, budget, consume)
}

// Logical typed carriers of the shared original bounded-recursion grammar.
// The old depth-32 limit permits 33 live frames per grammar; a predicate can
// call affine while all its own frames remain live. This is not native stack
// layout or whole-process RSS accounting.
fn grammar_frames() -> PathResult<usize> {
    type Frame<'a> = (
        &'a (),
        ValueId,
        usize,
        Option<&'a Operation>,
        Option<&'a Type>,
        Option<Affine>,
        Option<Affine>,
        Option<Affine>,
        Option<Fact>,
        Option<u64>,
        bool,
        ComparePredicate,
        &'a mut Vec<Fact>,
    );
    size_of::<Frame<'_>>()
        .checked_mul(66)
        .ok_or_else(|| Resource::Arithmetic.into())
}

pub(super) fn drain<T>(value: T) -> bool {
    // As in the report/solver scopes, callbacks and their destructors are
    // trusted compiler code. The resource ledger does not certify termination
    // of an adversarial self-reproducing panic-payload destructor.
    let mut result = catch_unwind(AssertUnwindSafe(|| drop(value)));
    let mut panicked = false;
    while let Err(payload) = result {
        panicked = true;
        result = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
    panicked
}

pub(super) fn with_queries<'owner, 'work, Q, F>(
    source: &Q,
    queries: &mut PresburgerQueryScopeV2<'_>,
    budget: &mut Budget<'work>,
    consume: F,
) -> PathResult<()>
where
    Q: Queries<'owner>,
    F: for<'view> FnOnce(&FormalPaidPathViewV20<'view>, &mut Budget<'work>) -> PathResult<()>,
{
    let floor = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut pending = Some(consume);
    let mut credit = InputCredit { bytes: 0 };
    let mut required = None;
    let mut authenticated = false;
    let mut construct = |queries: &mut PresburgerQueryScopeV2<'_>,
                         budget: &mut Budget<'work>,
                         credit: &mut InputCredit|
     -> PathResult<()> {
        source.check(budget)?;
        let mut engine = PaidEngine::build(source, budget, credit)?;
        let observations = engine.run(queries)?;
        drop(engine);
        // Scratch backing has died, but remains conservatively charged through
        // this fixed lexical callback. Output rows stay live until it returns.
        required = Some(budget.storage());
        let view = FormalPaidPathViewV20 {
            owner: source.original_owner(),
            function: source.original_function(),
            root_index: source.root_index(),
            launch: source.launch(),
            width: source.width(),
            report: source.analysis(),
            observations: &observations,
        };
        let consumer = pending.take().ok_or(Error::InconsistentOriginalReport)?;
        consumer(&view, budget)?;
        source.check(budget)?;
        budget.check_prior_denials_v1()?;
        Ok(())
    };
    let construct_bytes = std::mem::size_of_val(&construct);
    let result = catch_unwind(AssertUnwindSafe(|| -> PathResult<()> {
        // Authenticate the genuine original source account before reserving
        // anything on a potentially foreign budget.
        source.check(budget)?;
        authenticated = true;
        queries.usage(budget)?;
        budget.check_prior_denials_v1()?;
        let header = [
            construct_bytes,
            size_of::<F>(),
            size_of::<Option<F>>(),
            size_of::<InputCredit>(),
            size_of::<PaidEngine<'_, 'owner, '_, 'work, Q>>(),
            size_of::<FormalPaidPathViewV20<'_>>(),
            size_of::<Vec<Observation>>(),
            size_of::<std::thread::Result<PathResult<()>>>() * 2,
            size_of::<std::thread::Result<()>>(),
            size_of::<(
                &mut PresburgerQueryScopeV2<'_>,
                &mut Budget<'_>,
                &mut InputCredit,
                &usize,
            )>(),
            grammar_frames()?,
        ]
        .into_iter()
        .try_fold(0_usize, |sum, size| sum.checked_add(size))
        .ok_or(Resource::Arithmetic)?;
        budget.charge_work(header)?;
        credit.reserve(budget, header)?;
        construct(queries, budget, &mut credit)
    }));
    drop(construct);
    let same = |budget: &Budget<'_>| {
        slot == std::ptr::from_ref(budget) as usize && ledger == budget.work_ledger_identity_v1()
    };
    let selected = |budget: &Budget<'_>| -> Option<Error> {
        // An initial foreign-account rejection must not be replaced by a
        // pre-existing failure read from that unrelated caller account.
        if !authenticated {
            return None;
        }
        if !same(budget) {
            return Some(Resource::Accounting.into());
        }
        if let Err(error) = budget.check_prior_denials_v1() {
            return Some(error.into());
        }
        if budget.storage() < floor || required.is_some_and(|value| budget.storage() != value) {
            return Some(Resource::Accounting.into());
        }
        None
    };
    let first = selected(budget);
    let mut result = match result {
        Ok(result) => result,
        Err(payload) => {
            drain(payload);
            Err(Error::Panicked)
        }
    };
    if let Some(error) = first {
        result = Err(error);
    }
    let capture_panicked = drain(pending);
    if result.is_ok() {
        if let Some(error) = selected(budget) {
            result = Err(error);
        } else if capture_panicked {
            result = Err(Error::Panicked);
        }
    }
    // The complete graph/fact/relation/observation backing and captured F have
    // all died. Foreign or undercut accounts cannot authorize a refund.
    let cleanup = if !same(budget) {
        Err(Resource::Accounting)
    } else if let Some(minimum) = floor.checked_add(credit.bytes) {
        if budget.storage() < minimum {
            Err(Resource::Accounting)
        } else {
            budget.release_storage(credit.bytes)
        }
    } else {
        Err(Resource::Arithmetic)
    };
    result.and_then(|()| cleanup.map_err(Into::into))
}
