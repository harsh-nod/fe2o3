//! Source-bound expression availability over the actual final-native LICM owner.
//! A checked expression plan is not a generated or executed refinement theorem.
//!
//! ```compile_fail
//! use fe2o3_verifier::{PreparedMixedPureCseCfgRefinementV27, PreparedMixedRelocationExpressionsV28};
//! fn relabel<'h, 'n, 'p, 'v, 's>(value: PreparedMixedPureCseCfgRefinementV27<'h, 'v, 's>)
//!     -> PreparedMixedRelocationExpressionsV28<'h, 'n, 'p, 'v, 's> { value }
//! ```

use super::{Budget, FlowError, InventoryError, Pair, RelocationExpressionPlanV28, Resource};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV18 as Identity;
use fe2o3_kernel_opt::OwnedLicmErrorV1 as MotionError;
use fe2o3_lower_mir_kernel::{
    ProductionCheckedMixedPrefixViewV29 as PrefixView,
    ProductionConditionalMixedFixedpointOutputHandoffV29 as Policy11,
    ProductionConditionalMixedLicmOutputHandoffV28 as Handoff,
    ProductionConditionalMixedPureCseOutputHandoffV26 as Policy10,
    ProductionMixedLicmRelocationErrorV28 as RelocationError,
    ProductionMixedLicmRelocationV28 as Relocation,
    ProductionMixedPrefixExecutionViewV29 as ExecutionView,
    ProductionMixedPrefixOwnerV29 as PrefixOwner, ProductionSourceOwnedViewErrorV18 as SourceError,
    ProductionSourceOwnedViewV18 as Source,
};
use sha2::{Digest as _, Sha256};
use std::{
    fmt,
    mem::{align_of, size_of},
};

#[path = "mixed_optimizer_relocation_cfg_refinement_v28.rs"]
mod cfg;
pub use cfg::{
    MixedOptimizerRelocationCfgSubjectV28, PreparedMixedComposedRelocationCfgRefinementV28,
    PreparedMixedFixedpointComposedRelocationCfgRefinementV29,
    PreparedMixedFixedpointRelocationCfgRefinementV29, PreparedMixedRelocationCfgRefinementV28,
};

/// Failure to retain, replay, or generate a source-bound relocation request.
#[derive(Debug)]
pub enum MixedOptimizerRelocationErrorV28 {
    /// Original-source ownership or custody validation failed.
    Source(SourceError),
    /// The native handoff's checked relocation chain failed validation.
    Relocation(RelocationError),
    /// Checked LICM origin or endpoint replay failed.
    Motion(MotionError),
    /// Work or storage accounting failed.
    Resource(Resource),
    /// A canonical graph inventory could not be derived.
    Inventory(InventoryError),
    /// Bounded control-flow analysis failed.
    Flow(FlowError),
    /// Structured CFG theorem generation failed.
    Generation(super::super::MixedOptimizerRefinementErrorV26),
    /// An exact source, graph, or interpretation binding did not match.
    Binding(&'static str),
}
type Error = MixedOptimizerRelocationErrorV28;
type Result<T> = std::result::Result<T, Error>;
impl From<SourceError> for Error {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}
impl From<RelocationError> for Error {
    fn from(error: RelocationError) -> Self {
        Self::Relocation(error)
    }
}
impl From<MotionError> for Error {
    fn from(error: MotionError) -> Self {
        Self::Motion(error)
    }
}
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<super::super::MixedOptimizerRefinementErrorV26> for Error {
    fn from(error: super::super::MixedOptimizerRefinementErrorV26) -> Self {
        Self::Generation(error)
    }
}
impl From<super::Error> for Error {
    fn from(error: super::Error) -> Self {
        match error {
            super::Error::Resource(error) => Self::Resource(error),
            super::Error::Inventory(error) => Self::Inventory(error),
            super::Error::Flow(error) => Self::Flow(error),
            super::Error::Mismatch(message) => Self::Binding(message),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "source-bound mixed relocation expressions: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Relocation(error) => Some(error),
            Self::Motion(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::Flow(error) => Some(error),
            Self::Generation(error) => Some(error),
            Self::Binding(_) => None,
        }
    }
}

/// An inert observation, never a constructor argument or receipt. The request
/// retains actual owners, checked origin rows and the complete expression plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedOptimizerRelocationSubjectV28 {
    source_semantic: [u8; 32],
    source_ssa: [u8; 32],
    input: Identity,
    prefix: Identity,
    output: Identity,
    prefix_execution: [u8; 32],
    prefix_policy: u16,
    operations: usize,
    moved_operations: usize,
    moved_results: usize,
    cut_bindings: usize,
    runtime_occurrences: usize,
    slice_premises: usize,
}
impl MixedOptimizerRelocationSubjectV28 {
    /// Identity of the retained original semantic MIR.
    pub const fn source_semantic_identity(self) -> [u8; 32] {
        self.source_semantic
    }
    /// Identity of the retained original mixed-SSA program.
    pub const fn source_ssa_identity(self) -> [u8; 32] {
        self.source_ssa
    }
    /// Identity of the original canonical graph before the checked prefix.
    pub const fn input(self) -> Identity {
        self.input
    }
    /// Identity of the checked prefix output consumed by LICM.
    pub const fn prefix(self) -> Identity {
        self.prefix
    }
    /// Identity of the final LICM graph retained by native completion.
    pub const fn output(self) -> Identity {
        self.output
    }
    /// Fixed optimizer policy used by the checked prefix.
    pub const fn prefix_policy_version(self) -> u16 {
        self.prefix_policy
    }
    /// Identity of the compiler-owned prefix execution witness.
    pub const fn prefix_execution_identity(self) -> [u8; 32] {
        self.prefix_execution
    }
    /// Number of operations in the LICM input inventory.
    pub const fn original_operations(self) -> usize {
        self.operations
    }
    /// Number of operations relocated by the checked LICM tail.
    pub const fn moved_operations(self) -> usize {
        self.moved_operations
    }
    /// Number of relocated result definitions in the expression plan.
    pub const fn moved_results(self) -> usize {
        self.moved_results
    }
    /// Number of checked expression availability bindings at CFG cuts.
    pub const fn cut_bindings(self) -> usize {
        self.cut_bindings
    }
    /// Number of final-native runtime occurrences retained by the handoff.
    pub const fn runtime_occurrences(self) -> usize {
        self.runtime_occurrences
    }
    /// Number of final-native slice premises retained by the handoff.
    pub const fn slice_premises(self) -> usize {
        self.slice_premises
    }
}

/// Source, nominal checked prefix, actual LICM lineage, final native completion
/// and expression availability remain one borrowed chain. No caller-authored
/// rows, policy roster, owner hashes or execution receipts can construct it.
#[must_use = "discard this checked plan before its borrowed final-native owner"]
pub struct PreparedMixedRelocationExpressionsV28<
    'handoff,
    'native,
    'prefix,
    'view,
    'source,
    P: PrefixOwner<'view, 'source> = Policy10<'view, 'source>,
> {
    source: &'handoff Source<'source>,
    handoff: &'handoff Handoff<'native, 'prefix, 'view, 'source, P>,
    pair: Pair<'handoff>,
    plan: RelocationExpressionPlanV28<'handoff>,
    subject: MixedOptimizerRelocationSubjectV28,
    retained: usize,
    required: usize,
}

/// Actual Policy11 prefix retained through expression relocation checks.
/// ```compile_fail
/// use fe2o3_verifier::{PreparedMixedRelocationExpressionsV28, PreparedMixedFixedpointRelocationExpressionsV29};
/// fn relabel<'h, 'n, 'p, 'v, 's>(old: PreparedMixedRelocationExpressionsV28<'h, 'n, 'p, 'v, 's>)
///     -> PreparedMixedFixedpointRelocationExpressionsV29<'h, 'n, 'p, 'v, 's> { old }
/// ```
pub type PreparedMixedFixedpointRelocationExpressionsV29<'h, 'n, 'p, 'v, 's> =
    PreparedMixedRelocationExpressionsV28<'h, 'n, 'p, 'v, 's, Policy11<'v, 's>>;

/// Prepares the historical nominal Policy10 expression request.
pub fn prepare_mixed_relocation_expressions_v28<'h, 'n, 'p, 'v, 's>(
    source: &'h Source<'s>,
    handoff: &'h Handoff<'n, 'p, 'v, 's>,
    budget: &mut Budget<'_>,
) -> Result<PreparedMixedRelocationExpressionsV28<'h, 'n, 'p, 'v, 's>> {
    prepare_prefix_relocation_expressions_v29(source, handoff, budget)
}

/// Prepares an actual Policy11 request retaining the complete round witness.
pub fn prepare_mixed_fixedpoint_relocation_expressions_v29<'h, 'n, 'p, 'v, 's>(
    source: &'h Source<'s>,
    handoff: &'h Handoff<'n, 'p, 'v, 's, Policy11<'v, 's>>,
    budget: &mut Budget<'_>,
) -> Result<PreparedMixedFixedpointRelocationExpressionsV29<'h, 'n, 'p, 'v, 's>> {
    prepare_prefix_relocation_expressions_v29(source, handoff, budget)
}

type Built<'a> = (
    Pair<'a>,
    RelocationExpressionPlanV28<'a>,
    MixedOptimizerRelocationSubjectV28,
    usize,
);
type ConstructorCapture<'a, 'h, 'n, 'p, 'v, 's, 'w, P> = (
    &'h Source<'s>,
    &'h Handoff<'n, 'p, 'v, 's, P>,
    &'n Relocation<'p, 'v, 's, P>,
    &'a mut Budget<'w>,
    usize,
);
// Explicit scalar/borrow/hash locals. Pair and plan storage are separately
// receipted; the return and catch carriers below retain their own full layouts.
type ConstructorLocals<'a> = (
    usize,
    &'a super::Owner,
    PrefixView<'a>,
    ExecutionView<'a>,
    fe2o3_kernel_analysis::CanonicalKirLicmStorageV1,
    &'a fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    MixedOptimizerRelocationSubjectV28,
    usize,
    Sha256,
    [u8; 32],
    [&'a [u8]; 3],
    Option<usize>,
);

fn headers<'v, 's: 'v, P: PrefixOwner<'v, 's>>() -> Result<usize> {
    type Prepared<'a, 'v, 's, P> = PreparedMixedRelocationExpressionsV28<'a, 'a, 'a, 'v, 's, P>;
    // Pair and plan headers are paid by their own exact retained receipts.
    // Keep this small constructor envelope live with the request so the plan's
    // original floor never changes after its checked construction.
    let owner = size_of::<Prepared<'_, 'v, 's, P>>()
        .checked_sub(size_of::<Pair<'_>>())
        .and_then(|bytes| bytes.checked_sub(size_of::<RelocationExpressionPlanV28<'_>>()))
        .ok_or(Resource::Arithmetic)?;
    [
        owner,
        align_of::<Prepared<'_, 'v, 's, P>>(),
        size_of::<ConstructorCapture<'_, '_, '_, '_, 'v, 's, '_, P>>(),
        align_of::<ConstructorCapture<'_, '_, '_, '_, 'v, 's, '_, P>>(),
        size_of::<std::panic::AssertUnwindSafe<ConstructorCapture<'_, '_, '_, '_, 'v, 's, '_, P>>>(
        ),
        PrefixView::inspection_storage_v29()?,
        size_of::<ConstructorLocals<'_>>(),
        align_of::<ConstructorLocals<'_>>(),
        size_of::<Result<Built<'_>>>(),
        size_of::<std::thread::Result<Result<Built<'_>>>>(),
        size_of::<
            std::result::Result<
                (Pair<'_>, fe2o3_kernel_analysis::CanonicalKirLicmStorageV1),
                MotionError,
            >,
        >(),
        size_of::<std::result::Result<RelocationExpressionPlanV28<'_>, super::Error>>(),
        2 * size_of::<Result<()>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or(Resource::Arithmetic.into())
    })
}

impl<'v, 's, P: PrefixOwner<'v, 's>> PreparedMixedRelocationExpressionsV28<'_, '_, '_, 'v, 's, P> {
    fn custody(&self, budget: &Budget<'_>) -> Result<()> {
        let plan = self.plan.custody(budget).map_err(Error::from);
        let owner = self
            .handoff
            .observe_retained_storage_v28(self.required, budget);
        owner?;
        plan
    }
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        let custody = self.custody(budget);
        let source = self.source.check_query_v18(budget);
        let owner = self.handoff.relocation(budget);
        source?;
        owner?;
        custody
    }
    /// Observe the inert subject after checking the original custody ledger.
    pub fn subject(&self, budget: &Budget<'_>) -> Result<MixedOptimizerRelocationSubjectV28> {
        self.check(budget)?;
        Ok(self.subject)
    }
    /// Return the request's retained storage after checking custody.
    pub fn retained_storage(&self, budget: &Budget<'_>) -> Result<usize> {
        self.check(budget)?;
        Ok(self.retained)
    }
    /// Require the exact original mixed-SSA owner retained by the handoff.
    pub fn check_original_source(
        &self,
        source: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.check(budget)?;
        self.handoff.check_original_source(source, budget)?;
        Ok(())
    }
    /// Rebuild the actual prefix/final inventories and memory relations, then
    /// independently reconstruct complete expression dependencies and CFG cuts.
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.check(budget)?;
        let relocation = self.handoff.relocation(budget)?;
        relocation.check_original_source(self.source.source_ssa(budget)?, budget)?;
        relocation.replay(budget)?;
        if !std::ptr::eq(
            self.pair.input(),
            relocation
                .prefix(budget)?
                .checked_prefix_v29(budget)?
                .owner(),
        ) || !std::ptr::eq(self.pair.output(), self.handoff.output(budget)?)
        {
            return Err(Error::Binding("exact final-native LICM endpoints"));
        }
        self.plan.replay(&self.pair, budget)?;
        self.check(budget)
    }
    /// Drop all request backing before refunding its original storage ledger.
    pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.check(budget);
        let custody = self.custody(budget);
        let source = self.source;
        let retained = self.retained;
        // The complete request owns only closed compiler types. Neither plan
        // backing nor the checked pair may remain alive when credit is refunded.
        drop(self);
        let settled = custody.and_then(|()| {
            budget
                .release_storage(retained)
                .map_err(|error| Error::Source(source.retain_query_resource_error_v18(error)))
        });
        checked?;
        settled
    }
    /// Always false: expression replay does not execute a proof.
    pub const fn authenticates_executed_proof(&self) -> bool {
        false
    }
    /// Always false: an expression plan alone proves no CFG refinement.
    pub const fn proves_cfg_refinement(&self) -> bool {
        false
    }
    /// Always false: MIR-to-native semantics are outside this request.
    pub const fn proves_mir_to_native_lowering(&self) -> bool {
        false
    }
    /// Always false: this request grants neither artifact nor launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Replay the exact original-source/checked-prefix/LICM/native chain and retain its
/// checked expression plan, charging the supplied custody ledger.
fn prepare_prefix_relocation_expressions_v29<'h, 'n, 'p, 'v, 's, P: PrefixOwner<'v, 's>>(
    source: &'h Source<'s>,
    handoff: &'h Handoff<'n, 'p, 'v, 's, P>,
    budget: &mut Budget<'_>,
) -> Result<PreparedMixedRelocationExpressionsV28<'h, 'n, 'p, 'v, 's, P>> {
    source.check_query_v18(budget)?;
    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
    let relocation = handoff.relocation(budget)?;
    relocation.replay(budget)?;
    let floor = budget.storage();
    let capture: ConstructorCapture<'_, '_, '_, '_, 'v, 's, '_, P> =
        (source, handoff, relocation, &mut *budget, floor);
    let construct = move || {
        let (source, handoff, relocation, budget, floor) = std::convert::identity(capture);
        let header = headers::<P>()?;
        budget.reserve_storage(header)?;
        let original = source.canonical(budget)?;
        let prefix = relocation.prefix(budget)?.checked_prefix_v29(budget)?;
        let execution = prefix.execution();
        budget.charge_work(
            original
                .canonical_bytes()
                .len()
                .checked_add(prefix.input_audit_bytes().len())
                .and_then(|bytes| bytes.checked_add(execution.canonical_bytes().len()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if original.canonical_bytes() != prefix.input_audit_bytes()
            || execution.policy_version() != P::POLICY_VERSION
            || execution.graph_schema() != 18
        {
            return Err(Error::Binding(
                "exact original audit and nominal checked prefix",
            ));
        }
        let (pair, pair_storage) = relocation
            .tail(budget)?
            .replay_against(prefix.owner(), budget)?;
        budget.reserve_storage(pair_storage.retained_storage())?;
        if !std::ptr::eq(pair.output(), handoff.output(budget)?) {
            return Err(Error::Binding("actual final-native LICM output"));
        }
        let plan = super::build(&pair, budget)?;
        let source_ssa = source.source_ssa(budget)?;
        let subject = MixedOptimizerRelocationSubjectV28 {
            source_semantic: *source_ssa.source_semantic_sha256(),
            source_ssa: *source_ssa.identity().as_bytes(),
            input: *original.identity(),
            prefix: *pair.input().identity(),
            output: *pair.output().identity(),
            prefix_execution: Sha256::digest(execution.canonical_bytes()).into(),
            prefix_policy: P::POLICY_VERSION,
            operations: pair.origins().len(),
            moved_operations: plan.nodes.len(),
            moved_results: plan.results.len(),
            cut_bindings: plan.cuts.len(),
            runtime_occurrences: handoff.runtime_occurrences(budget)?.len(),
            slice_premises: handoff.runtime_premises(budget)?.len(),
        };
        let retained = header
            .checked_add(pair_storage.retained_storage())
            .and_then(|bytes| bytes.checked_add(plan.retained))
            .ok_or(Resource::Arithmetic)?;
        handoff.observe_retained_storage_v28(floor, budget)?;
        source.check_query_v18(budget)?;
        if floor.checked_add(retained) != Some(budget.storage()) {
            return Err(Error::Source(
                source.retain_query_resource_error_v18(Resource::Accounting),
            ));
        }
        Ok((pair, plan, subject, retained))
    };
    #[cfg(test)]
    {
        assert_eq!(
            std::mem::size_of_val(&construct),
            size_of::<ConstructorCapture<'_, '_, '_, '_, 'v, 's, '_, P>>()
        );
        assert_eq!(
            std::mem::align_of_val(&construct),
            align_of::<ConstructorCapture<'_, '_, '_, '_, 'v, 's, '_, P>>()
        );
    }
    let selected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(construct));
    match selected {
        Ok(Ok((pair, plan, subject, retained))) => Ok(PreparedMixedRelocationExpressionsV28 {
            source,
            handoff,
            pair,
            plan,
            subject,
            retained,
            required: budget.storage(),
        }),
        selected => {
            // Only closed compiler constructors run above. Their rejected rows
            // have dropped before this constructor-scratch settlement; no
            // arbitrary callback allocation ownership is inferred from a delta.
            if handoff.observe_retained_storage_v28(floor, budget).is_ok() {
                if let Some(credit) = budget.storage().checked_sub(floor) {
                    let _ = budget
                        .release_storage(credit)
                        .map_err(|error| source.retain_query_resource_error_v18(error));
                }
            }
            match selected {
                Ok(Err(error)) => Err(error),
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(Ok(_)) => unreachable!(),
            }
        }
    }
}
