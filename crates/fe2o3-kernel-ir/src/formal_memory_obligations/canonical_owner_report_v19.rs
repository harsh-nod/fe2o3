//! Fixed lexical adapter over the original paid report-construction chain.
use super::private_slots_bytes_v18::pointer_source_bytes_v18::accesses::body_source_v19::{
    ActualOwnerReportViewV19, BodyErrorV19,
};
use super::*;
use crate::{CanonicalEffectErrorV19, CanonicalEffectScopeV19};
use std::fmt;

/// Descriptive invocation interpretation, not authenticated target authority.
/// A physical envelope must cover every source-static active dimension; the
/// original immutable source domain is never rewritten to fit the envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalFormalLaunchInputV19 {
    /// Require the original exact-extent interpretation, including static equality.
    Exact(ExplicitLaunchExtent),
    /// Analyze the supplied physical extent, retaining any padded invocations.
    PhysicalEnvelope(ExplicitLaunchExtent),
}
impl CanonicalFormalLaunchInputV19 {
    fn parts(self) -> (ExplicitLaunchExtent, PhysicalLaunchInterpretationV2) {
        match self {
            Self::Exact(extent) => (extent, PhysicalLaunchInterpretationV2::Exact),
            Self::PhysicalEnvelope(extent) => (extent, PhysicalLaunchInterpretationV2::Envelope),
        }
    }
}

/// Exact refusal from the original source analyses, ledger, or lexical consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalFormalReportErrorV19 {
    /// The original cumulative work or typed-storage ledger refused the request.
    Resource(FormalGuardedMemoryResourceErrorV1),
    /// The original source CFG is outside the existing analysis contract.
    ControlFlow(crate::ControlFlowError),
    /// The genuine owner-borrowed effect scope refused its original query.
    Effects(CanonicalEffectErrorV19),
    /// The original invocation range is invalid or overflows its representation.
    Invocation(RegionValidationError),
    /// The caller rejected the borrowed report; no report authority escapes.
    ConsumerRejected,
    /// Trusted compiler callback code or its captured values unwound.
    Panicked,
}
impl fmt::Display for CanonicalFormalReportErrorV19 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "canonical owner formal report V19: {self:?}")
    }
}
impl std::error::Error for CanonicalFormalReportErrorV19 {}
impl From<Failure> for CanonicalFormalReportErrorV19 {
    fn from(error: Failure) -> Self {
        match error {
            Failure::Resource(error) => Self::Resource(error),
            Failure::ControlFlow(error) => Self::ControlFlow(error),
        }
    }
}
impl From<ResourceError> for CanonicalFormalReportErrorV19 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}
impl From<crate::CanonicalKernelIrVerificationResourceErrorV1> for CanonicalFormalReportErrorV19 {
    fn from(error: crate::CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error.into())
    }
}
impl From<BodyErrorV19> for CanonicalFormalReportErrorV19 {
    fn from(error: BodyErrorV19) -> Self {
        match error {
            BodyErrorV19::Source(error) => error.into(),
            BodyErrorV19::Effects(error) => Self::Effects(error),
            BodyErrorV19::Invocation(error) => Self::Invocation(error),
            BodyErrorV19::Rejected => Self::ConsumerRejected,
            BodyErrorV19::Panicked => Self::Panicked,
        }
    }
}
type PublicResult<T> = std::result::Result<T, CanonicalFormalReportErrorV19>;

#[path = "canonical_report_source_v20.rs"]
mod source_v20;
pub use source_v20::CanonicalFormalSourceScopeV20;

/// Complete extraction result borrowed with its exact source and live paid rows.
/// It is not a memory-admission receipt, source/output equivalence proof, launch
/// authority, or permission to publish code. No owned report is returned.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::*;
/// fn escape<'owner, 'work>(
///     owner: &'owner VerifiedCanonicalKernelIrModuleV18,
///     effects: &CanonicalEffectScopeV19<'owner>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'work>,
/// ) -> Option<&'owner FormalMemoryObligationAnalysis> {
///     let mut escaped = None;
///     with_canonical_owner_formal_report_v19(
///         owner, 0, effects,
///         CanonicalFormalLaunchInputV19::Exact(ExplicitLaunchExtent::Unknown),
///         FormalIndexWidth::Unknown, ControlFlowLimits::DEFAULT, budget,
///         |report, _| { escaped = Some(report.analysis()); Ok(()) },
///     ).unwrap();
///     escaped
/// }
/// ```
pub struct CanonicalFormalReportViewV19<'report, 'owner> {
    source: CanonicalFormalSourceScopeV20<'report, 'owner>,
}
impl<'report, 'owner> CanonicalFormalReportViewV19<'report, 'owner> {
    /// Borrows the already retained source CFG/definition context on its original
    /// account. This does not rebuild a graph or grant branch/launch authority.
    pub fn source_scope_v20(
        &self,
        budget: &mut Budget<'_>,
    ) -> PublicResult<&CanonicalFormalSourceScopeV20<'report, 'owner>> {
        self.source
            .check(self.original_owner(), self.root_index(), budget)?;
        Ok(&self.source)
    }
    /// Exact immutable canonical owner used by every analysis in this report.
    pub fn original_owner(&self) -> &VerifiedCanonicalKernelIrModuleV18 {
        self.source.original_owner()
    }
    /// Actual original entry function, not a copied or reconstructed function.
    pub fn original_function(&self) -> &Function {
        self.source.original_function()
    }
    /// Original kernel ordinal in the actual owner's kernel roster.
    pub fn root_index(&self) -> usize {
        self.source.root_index()
    }
    /// Descriptive extent and interpretation used by this extraction.
    pub fn launch_input(&self) -> CanonicalFormalLaunchInputV19 {
        self.source.launch_input()
    }
    /// Descriptive index width; unsupported widths retain their original reason.
    pub fn index_width(&self) -> FormalIndexWidth {
        self.source.index_width()
    }
    /// Original ordered report, including every incomplete reason and conflict.
    pub fn analysis(&self) -> &FormalMemoryObligationAnalysis {
        self.source.report()
    }
}

fn drain<T>(value: T) -> bool {
    let mut outcome = catch_unwind(AssertUnwindSafe(|| drop(value)));
    let mut panicked = false;
    while let Err(payload) = outcome {
        panicked = true;
        outcome = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
    panicked
}

/// Runs the existing paid source/CFG/affine/private-slot/pointer/access/report
/// algorithms on one authentic owner and the caller's original cumulative ledger.
/// The supplied effects scope is reused and checked against that same owner.
/// All report and analysis backing dies before this scope's credit is retired.
/// Consumer-owned backing is not paid here and cannot escape via the unit result.
///
/// Callback code and destructors are trusted compiler code, not a sandbox: their
/// arbitrary execution and indefinitely self-reproducing panic payloads are not
/// bounded by compiler work accounting. Extent/index inputs remain descriptive.
pub fn with_canonical_owner_formal_report_v19<'owner, 'work, F>(
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    root_index: usize,
    effects: &CanonicalEffectScopeV19<'owner>,
    launch: CanonicalFormalLaunchInputV19,
    width: FormalIndexWidth,
    limits: ControlFlowLimits,
    budget: &mut Budget<'work>,
    consume: F,
) -> PublicResult<()>
where
    F: for<'report> FnOnce(
        &CanonicalFormalReportViewV19<'report, 'owner>,
        &mut Budget<'work>,
    ) -> PublicResult<()>,
{
    let floor = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut pending = Some(consume);
    let mut required = None;
    let mut consumer_error = None;
    let mut construction = |budget: &mut Budget<'work>| -> PublicResult<()> {
        prior_denial_v2(budget)?;
        budget.charge_work(2)?;
        let mut affine = ActualOwnerAffineV18::build(owner, root_index, limits, budget)?;
        let mut slots = affine.private_slots(owner, root_index, budget)?;
        let mut pointers = slots.pointers(owner, root_index, budget)?;
        let mut accesses = pointers.accesses(owner, root_index, budget)?;
        required = Some(budget.storage());
        let (extent, interpretation) = launch.parts();
        accesses
            .with_report_v19(
                owner,
                root_index,
                effects,
                extent,
                width,
                interpretation,
                budget,
                |inner, budget| {
                    let view = CanonicalFormalReportViewV19 {
                        source: CanonicalFormalSourceScopeV20::new(inner, launch, width),
                    };
                    let consumer = pending.take().ok_or(ResourceError::Accounting)?;
                    match consumer(&view, budget) {
                        Ok(()) => Ok(()),
                        Err(error) => {
                            consumer_error = Some(error);
                            Err(BodyErrorV19::Rejected)
                        }
                    }
                },
            )
            .map_err(Into::into)
    };
    let construction_bytes = std::mem::size_of_val(&construction);
    let outcome = catch_unwind(AssertUnwindSafe(|| -> PublicResult<()> {
        prior_denial_v2(budget)?;
        let headers = [
            construction_bytes,
            size_of::<Option<F>>(),
            size_of::<F>(),
            size_of::<CanonicalFormalReportViewV19<'_, '_>>(),
            size_of::<Option<CanonicalFormalReportErrorV19>>(),
            size_of::<std::thread::Result<PublicResult<()>>>() * 2,
            size_of::<std::thread::Result<()>>(),
            size_of::<(
                usize,
                crate::CanonicalKernelIrWorkLedgerIdentityV1,
                Option<usize>,
            )>(),
            size_of::<(&mut Budget<'_>, &mut (), &usize)>(),
        ]
        .into_iter()
        .try_fold(0_usize, |sum, bytes| sum.checked_add(bytes))
        .ok_or(ResourceError::Arithmetic)?;
        budget.reserve_storage(headers)?;
        construction(budget)
    }));
    drop(construction);
    // Every predecessor and its concrete backing is dead here. A completed
    // body scope has already retired only its own report rows, never ours.
    let same = |budget: &Budget<'_>| {
        slot == std::ptr::from_ref(budget) as usize && ledger == budget.work_ledger_identity_v1()
    };
    let selected = if !same(budget) {
        Some(ResourceError::Accounting.into())
    } else if let Err(error) = prior_denial_v2(budget) {
        Some(error.into())
    } else if budget.storage() < floor || required.is_some_and(|n| budget.storage() != n) {
        Some(ResourceError::Accounting.into())
    } else {
        None
    };
    let mut result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drain(payload);
            Err(CanonicalFormalReportErrorV19::Panicked)
        }
    };
    if result == Err(CanonicalFormalReportErrorV19::ConsumerRejected) {
        if let Some(error) = consumer_error.take() {
            result = Err(error);
        }
    }
    if let Some(error) = selected {
        result = Err(error);
    }
    let captures_panicked = drain(pending);
    drain(consumer_error);
    let after = if !same(budget) {
        Some(ResourceError::Accounting.into())
    } else if let Err(error) = prior_denial_v2(budget) {
        Some(error.into())
    } else if budget.storage() < floor || required.is_some_and(|n| budget.storage() != n) {
        Some(ResourceError::Accounting.into())
    } else if captures_panicked {
        Some(CanonicalFormalReportErrorV19::Panicked)
    } else {
        None
    };
    if result.is_ok() {
        if let Some(error) = after {
            result = Err(error);
        }
    }
    let cleanup = if !same(budget) {
        Err(ResourceError::Accounting.into())
    } else if let Some(required) = required {
        if budget.storage() < required {
            Err(ResourceError::Accounting.into())
        } else {
            budget.release_storage(required - floor).map_err(Into::into)
        }
    } else {
        budget.rollback_storage(floor).map_err(Into::into)
    };
    result.and(cleanup)
}
