//! One staged, source-owned production continuation. No LLVM is produced here.
use super::*;
#[path = "production_expanded_decoded_source_v1.rs"]
pub(super) mod decoded_source;
pub use decoded_source::{
    CheckedDecodedExpandedSourceV1, DecodedExpandedSourceErrorV1,
    with_checked_decoded_expanded_source_v1,
};
use fe2o3_kernel_opt::{
    CheckedScalarFixedPointOwnerV1 as Scalar, prepare_checked_scalar_fixed_point_v1,
};
use std::{error::Error as StdError, fmt, mem::size_of};
#[cfg(test)]
#[path = "production_checked_output_expanded_resources_v1_tests.rs"]
mod resources_tests;

const POLICY: &[u8] = b"FE2O3/EXPANDED-PRODUCTION-POLICY/V1\0";
// Literal schedule components, not mutable Default implementations or caller
// tuning knobs. Source-specific admission ceilings remain in original custody.
const LOOPS: fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1 =
    fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1 {
        functions: 16_384,
        blocks: 65_536,
        edges: 262_144,
        definitions: 262_144,
        operations: 65_536,
        loops: 65_536,
        rows: 1_048_576,
    };
const UNROLL: fe2o3_kernel_analysis::CanonicalKirLoopUnrollLimitsV1 =
    fe2o3_kernel_analysis::CanonicalKirLoopUnrollLimitsV1 {
        loops: LOOPS,
        max_iterations: 8,
        max_output_operand_uses: 1_048_576,
        max_output_edge_arguments: 1_048_576,
        max_origin_rows: 4_194_304,
    };
const FORWARDING: fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1 =
    fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1 {
        memory: fe2o3_kernel_analysis::CanonicalKirMemorySsaLimitsV1 {
            functions: 16_384,
            blocks: 65_536,
            operations: 65_536,
            effects: 262_144,
            edges: 262_144,
        },
        control_flow: fe2o3_kernel_ir::ControlFlowLimits {
            blocks: 65_536,
            edges: 1_048_576,
            edge_arguments: 1_048_576,
            phi_inputs: 1_048_576,
            analysis_work: 16_777_216,
        },
    };
type Owner = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
type XResult<T> = Result<T, ProductionExpandedPolicyErrorV1>;

/// Typed refusal; no old-U or alternate-policy output is returned on failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionExpandedPolicyErrorV1 {
    /// Cumulative work, retained storage or active-ledger custody failed.
    Resource(AssertOriginResourceV1),
    /// Actual original source, complete prefix or U source replay failed.
    Source(ProductionLoopUnrollErrorV1),
    /// Complete decoded B-through-U semantic history failed independent replay.
    DecodedHistory(fe2o3_kernel_opt::LoopUnrollHistoryErrorV1),
    /// The fixed scalar schedule or its independent complete replay failed.
    Scalar(fe2o3_kernel_opt::CheckedScalarFixedPointErrorV1),
    /// Shared source/native/formal checking refused the actual subject.
    Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1),
    /// Complete retained final state disagreed with independent reconstruction.
    History,
    /// Genuine prefix used settings outside this closed production schedule.
    Policy,
    /// A private scope unwound without transferring a valid owner.
    Panicked,
}
impl From<AssertOriginResourceV1> for ProductionExpandedPolicyErrorV1 {
    fn from(value: AssertOriginResourceV1) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for ProductionExpandedPolicyErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expanded production policy: {self:?}")
    }
}
impl StdError for ProductionExpandedPolicyErrorV1 {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Source(e) => Some(e),
            Self::DecodedHistory(e) => Some(e),
            Self::Scalar(e) => Some(e),
            Self::Admission(e) => Some(e),
            _ => None,
        }
    }
}
type XError = ProductionExpandedPolicyErrorV1;

/// Added unreserved storage only, excluding the caller's prepaid source/U owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionExpandedPolicyStorageV1(usize);
impl ProductionExpandedPolicyStorageV1 {
    /// Reserve before any controlled use of the returned expanded owner.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

enum Prefix {
    Direct(ProductionOwnedLoopUnrollContinuationV1),
    Erased(ProductionOwnedUnitLocalLoopUnrollContinuationV1),
}
/// Immutable original source/U custody, never a constructor or detached grant.
pub enum ProductionExpandedPrefixV1<'a> {
    /// Genuine connected Direct source with its complete historical U prefix.
    Direct(&'a ProductionOwnedLoopUnrollContinuationV1),
    /// Genuine original source, UnitLocal erasure and complete U prefix.
    Erased(&'a ProductionOwnedUnitLocalLoopUnrollContinuationV1),
}
/// The complete closed V1 schedule has exactly one scalar cleanup node after U.
/// This borrowed view exposes all adjacent Integer/Policy3 owners and rows.
pub enum ProductionExpandedHistoryV1<'a> {
    /// One fixed-point owner, including every changing and terminal round.
    ScalarCleanup(&'a Scalar),
}
impl Prefix {
    fn policy(&self, budget: &mut AssertOriginBudgetV1<'_>) -> XResult<()> {
        budget.charge_work(31)?;
        let (unroll, forwarding, refinement) = match self {
            Self::Direct(v) => (
                v.limits(),
                v.prefix().limits(),
                v.prefix().prefix().limits(),
            ),
            Self::Erased(v) => (
                v.limits(),
                v.prefix().limits(),
                v.prefix().prefix().limits(),
            ),
        };
        if unroll != UNROLL || forwarding != FORWARDING || refinement != LOOPS {
            return Err(XError::Policy);
        }
        Ok(())
    }
    fn output(&self) -> &Owner {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn floor(&self) -> XResult<usize> {
        match self {
            Self::Direct(v) => v.retained_input_storage_floor_v1(),
            Self::Erased(v) => v.retained_input_storage_floor_v1(),
        }
        .map_err(XError::Source)
    }
    fn inline_size(&self) -> usize {
        match self {
            Self::Direct(_) => size_of::<ProductionOwnedLoopUnrollContinuationV1>(),
            Self::Erased(_) => size_of::<ProductionOwnedUnitLocalLoopUnrollContinuationV1>(),
        }
    }
    fn replay(&self, budget: &mut AssertOriginBudgetV1<'_>) -> XResult<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(XError::Source)
    }
    fn check_source(
        &self,
        core: &Scalar,
        origins: &mut Vec<ProductionExpandedSourceOriginV1>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> XResult<Box<[FormalMemoryObligations]>> {
        match self {
            Self::Direct(v) => v.check_expanded_source_v1(core, origins, budget),
            Self::Erased(v) => v.check_expanded_source_v1(core, origins, budget),
        }
        .map_err(XError::Source)
    }
    fn anchor(&self) -> CanonicalOutputFormalSourceAnchorV1<'_> {
        match self {
            Self::Direct(v) => v.expanded_source_anchor_v1(),
            Self::Erased(v) => v.expanded_source_anchor_v1(),
        }
    }
}

/// Sole move-only source/U custody and all fixed-point rounds. The V1 source
/// prefix is already target-bound: this is staged progress, not neutral-first
/// roadmap completion, native/nominal admission or default-pipeline activation.
/// Existing formal-engine internal allocation/comparison exclusions remain;
/// new headers, actual Vec capacity, report Box extent and full core receipts
/// are paid. There is no independent outer work-bound or corpus qualification.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedExpandedContinuationV1 as Owner;
/// fn duplicate(v: Owner) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedExpandedContinuationV1 as Owner;
/// fn forge() -> Owner { Owner::default() }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedExpandedContinuationV1 as Owner;
/// fn detach(v: Owner) { let _ = v.prefix; }
/// ```
pub struct ProductionOwnedExpandedContinuationV1 {
    prefix: Prefix,
    scalar: Scalar,
    origins: Vec<ProductionExpandedSourceOriginV1>,
    kernels: Box<[FormalMemoryObligations]>,
    added: usize,
}

impl ProductionOwnedExpandedContinuationV1 {
    /// Returns this closed policy's immutable prefix limits, not a selection or
    /// permission to admit a prefix. Construction independently checks the
    /// actual source-owned history against these same literal limits.
    pub const fn prefix_limits_v1() -> (
        fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1,
        fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1,
        fe2o3_kernel_analysis::CanonicalKirLoopUnrollLimitsV1,
    ) {
        (LOOPS, FORWARDING, UNROLL)
    }

    // Consumer-side shared source join, not a constructor or executable grant.
    // The caller prepays original source, complete decoded backing/receipts,
    // every scalar owner, final-origin capacity and report extent. `required`
    // is only a necessary numeric floor, never reservation/source provenance.
    pub(crate) fn check_decoded_source_history_v1(
        anchor: CanonicalOutputFormalSourceAnchorV1<'_>,
        history: &fe2o3_kernel_opt::CheckedLoopUnrollHistoryV1<'_>,
        scalar: &Scalar,
        origins: &mut Vec<ProductionExpandedSourceOriginV1>,
        required: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> XResult<Box<[FormalMemoryObligations]>> {
        let source_floor = match anchor {
            CanonicalOutputFormalSourceAnchorV1::Direct(v) => v
                .pre_ranked_retained_analysis_storage_v1()
                .ok_or(XError::History)?,
            CanonicalOutputFormalSourceAnchorV1::Erased(v) => v.retained_storage_floor_v1(),
        };
        let minimum = source_floor
            .checked_add(history.storage().retained_storage())
            .and_then(|n| n.checked_add(scalar.retained_storage()))
            .ok_or(AssertOriginResourceV1::Arithmetic)?;
        if required < minimum {
            return Err(AssertOriginResourceV1::Accounting.into());
        }
        scoped(required, budget, |budget, binding| {
            budget.charge_work(31)?;
            if history.limits() != UNROLL
                || history.prefix().limits().refinement != LOOPS
                || history.prefix().limits().forwarding != FORWARDING
            {
                return Err(XError::Policy);
            }
            history.replay(budget).map_err(XError::DecodedHistory)?;
            binding.check(budget)?;
            scalar
                .replay_against(history.output(), budget)
                .map_err(XError::Scalar)?;
            binding.check(budget)?;
            let reports = ProductionOwnedLoopUnrollContinuationV1::check_decoded_expanded_sites_v1(
                anchor, history, scalar, origins, required, budget,
            )
            .map_err(XError::Source)?;
            binding.check(budget)?;
            Ok(reports)
        })
    }
}

struct Binding {
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}
impl Binding {
    fn check(&self, budget: &AssertOriginBudgetV1<'_>) -> XResult<()> {
        if self.slot != budget as *const AssertOriginBudgetV1<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(AssertOriginResourceV1::Accounting.into());
        }
        Ok(())
    }
}
fn scoped<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    body: impl FnOnce(&mut AssertOriginBudgetV1<'w>, &Binding) -> XResult<T>,
) -> XResult<T> {
    if budget.storage() < required {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    let binding = Binding {
        slot: budget as *const AssertOriginBudgetV1<'_> as usize,
        ledger: budget.work_ledger_identity_v1(),
        floor: budget.storage(),
    };
    let paid = size_of::<Binding>()
        .checked_add(size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>())
        .and_then(|n| n.checked_add(size_of::<XResult<T>>()))
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(paid)?;
    let minimum = binding
        .floor
        .checked_add(paid)
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    let mut payloads = [None, None];
    let mut result =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(budget, &binding))) {
            Ok(result) => result,
            Err(payload) => {
                payloads[0] = Some(payload);
                Err(XError::Panicked)
            }
        };
    let same = binding.slot == budget as *const AssertOriginBudgetV1<'_> as usize
        && binding.ledger == budget.work_ledger_identity_v1();
    if !same || budget.storage() < minimum {
        let rejected =
            std::mem::replace(&mut result, Err(AssertOriginResourceV1::Accounting.into()));
        if let Err(payload) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(rejected)))
        {
            payloads[1] = Some(payload);
        }
    }
    if same && budget.storage() >= binding.floor {
        if let Err(error) = budget.release_storage(budget.storage() - binding.floor) {
            let rejected = std::mem::replace(&mut result, Err(error.into()));
            if let Err(payload) =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(rejected)))
            {
                payloads[1] = Some(payload);
            }
        }
    }
    drop(payloads);
    result
}
fn header(prefix: &Prefix) -> XResult<usize> {
    size_of::<ProductionOwnedExpandedContinuationV1>()
        .checked_sub(prefix.inline_size())
        .and_then(|n| n.checked_sub(size_of::<Scalar>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn report_extent(owner: &Owner) -> XResult<usize> {
    owner
        .module()
        .kernels
        .len()
        .checked_mul(size_of::<FormalMemoryObligations>())
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn row_table(
    count: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> XResult<Vec<ProductionExpandedSourceOriginV1>> {
    scratch(count, budget).map_err(XError::Admission)
}
fn added(value: &ProductionOwnedExpandedContinuationV1) -> XResult<usize> {
    header(&value.prefix)?
        .checked_add(value.scalar.retained_storage())
        .and_then(|n| {
            n.checked_add(
                value
                    .origins
                    .capacity()
                    .checked_mul(size_of::<ProductionExpandedSourceOriginV1>())?,
            )
        })
        .and_then(|n| n.checked_add(std::mem::size_of_val(value.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn prepare(
    prefix: Prefix,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> XResult<(
    ProductionOwnedExpandedContinuationV1,
    ProductionExpandedPolicyStorageV1,
)> {
    scoped(prefix.floor()?, budget, |budget, binding| {
        prefix.policy(budget)?;
        prefix.replay(budget)?;
        binding.check(budget)?;
        budget.reserve_storage(header(&prefix)?)?;
        let scalar = prepare_checked_scalar_fixed_point_v1(prefix.output(), budget)
            .map_err(XError::Scalar)?;
        binding.check(budget)?;
        budget.reserve_storage(scalar.retained_storage())?;
        scalar
            .replay_against(prefix.output(), budget)
            .map_err(XError::Scalar)?;
        binding.check(budget)?;
        budget.reserve_storage(report_extent(scalar.output())?)?;
        let final_count = scalar
            .rounds()
            .last()
            .ok_or(XError::History)?
            .scalar()
            .occurrences()
            .candidate()
            .operations
            .len();
        let mut origins = row_table(final_count, budget)?;
        let kernels = prefix.check_source(&scalar, &mut origins, budget)?;
        binding.check(budget)?;
        if origins.len() != final_count || kernels.len() != scalar.output().module().kernels.len() {
            return Err(XError::History);
        }
        let mut owner = ProductionOwnedExpandedContinuationV1 {
            prefix,
            scalar,
            origins,
            kernels,
            added: 0,
        };
        owner.added = added(&owner)?;
        budget.charge_work(1)?;
        let receipt = ProductionExpandedPolicyStorageV1(owner.added);
        Ok((owner, receipt))
    })
}

impl ProductionOwnedExpandedContinuationV1 {
    /// Fixed staged-bound schedule identity, not a caller-selected policy.
    pub const fn policy_identity(&self) -> &'static [u8] {
        POLICY
    }
    /// Actual final graph after all complete scalar rounds, before any LLVM.
    pub fn output(&self) -> &Owner {
        self.scalar.output()
    }
    /// Immutable borrow of the sole consumed source/U custody.
    pub fn prefix(&self) -> ProductionExpandedPrefixV1<'_> {
        match &self.prefix {
            Prefix::Direct(v) => ProductionExpandedPrefixV1::Direct(v),
            Prefix::Erased(v) => ProductionExpandedPrefixV1::Erased(v),
        }
    }
    /// Immutable complete variable-round history of the closed schedule.
    pub const fn history(&self) -> ProductionExpandedHistoryV1<'_> {
        ProductionExpandedHistoryV1::ScalarCleanup(&self.scalar)
    }
    /// Original genuine source anchor; not standalone final-lineage authority.
    pub fn source_anchor(&self) -> CanonicalOutputFormalSourceAnchorV1<'_> {
        self.prefix.anchor()
    }
    /// Complete inert final source occurrence roster.
    pub fn origins(&self) -> &[ProductionExpandedSourceOriginV1] {
        &self.origins
    }
    /// Fresh final guarded memory reports in unchanged original root order.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.kernels
    }
    /// Added ownership excluding all inherited source/U and sibling backing.
    pub const fn additional_retained_storage_v1(&self) -> usize {
        self.added
    }
    /// This pre-native continuation grants no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Checked inherited floor plus the complete actual-capacity addition.
    pub fn retained_input_storage_floor_v1(&self) -> XResult<usize> {
        if self.added != added(self)? {
            return Err(AssertOriginResourceV1::Accounting.into());
        }
        self.prefix
            .floor()?
            .checked_add(self.added)
            .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
    }
    /// Replays source/U and every complete adjacent pair without invoking the
    /// optimizer, then independently reconstructs final source/native/formal
    /// facts. Same-ledger caller backing remains prepaid until all borrows end.
    pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> XResult<()> {
        scoped(
            self.retained_input_storage_floor_v1()?,
            budget,
            |budget, binding| {
                self.prefix.policy(budget)?;
                self.prefix.replay(budget)?;
                binding.check(budget)?;
                self.scalar
                    .replay_against(self.prefix.output(), budget)
                    .map_err(XError::Scalar)?;
                binding.check(budget)?;
                budget.reserve_storage(report_extent(self.output())?)?;
                let mut origins = row_table(self.origins.len(), budget)?;
                let kernels = self
                    .prefix
                    .check_source(&self.scalar, &mut origins, budget)?;
                binding.check(budget)?;
                budget.charge_work(
                    origins
                        .len()
                        .checked_mul(size_of::<ProductionExpandedSourceOriginV1>())
                        .and_then(|n| n.checked_add(1))
                        .ok_or(AssertOriginResourceV1::Arithmetic)?,
                )?;
                if origins != self.origins || kernels != self.kernels {
                    return Err(XError::History);
                }
                Ok(())
            },
        )
    }
}
impl ProductionOwnedLoopUnrollContinuationV1 {
    /// Consumes genuine source/U custody. No pass or schedule selector exists.
    /// Reserve the returned addition before any further controlled operation.
    pub fn continue_expanded_production_policy_v1(
        self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> XResult<(
        ProductionOwnedExpandedContinuationV1,
        ProductionExpandedPolicyStorageV1,
    )> {
        prepare(Prefix::Direct(self), budget)
    }
}
impl ProductionOwnedUnitLocalLoopUnrollContinuationV1 {
    /// Same closed continuation, retaining original N/E custody exactly once.
    pub fn continue_expanded_production_policy_v1(
        self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> XResult<(
        ProductionOwnedExpandedContinuationV1,
        ProductionExpandedPolicyStorageV1,
    )> {
        prepare(Prefix::Erased(self), budget)
    }
}
