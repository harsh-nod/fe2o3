//! Workload-neutral ownership reconstruction across the GPU hierarchy.
//!
//! The pass consumes actual guarded write traces, ranked-view shapes, and the
//! retained execution layout. It never recognizes an algorithm or kernel
//! name. Dialect verification checks the local ownership-contract payload;
//! this module proves whole-function range, injectivity, coverage, and
//! invocation/subgroup/workgroup/grid partitions.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
};

use dialect_kernel::{
    AccessKindAttr, AllocationEffectOp, DYNAMIC_EXTENT, MemorySpaceAttr, OwnershipContractOp,
    OwnershipCoverageAttr, OwnershipPartitionAttr, RankedAccessOp, RankedViewOp,
};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::Context,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    value::Value,
};

use crate::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::pliron_barrier::trace_failure_detail;
use crate::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceLocationV1,
};
use crate::pliron_race::run_pliron_ranked_race_check_with_analyses_v1;
use crate::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1;
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1};

/// Maximum logical output elements materialized by one exact coverage proof.
pub const MAX_HIERARCHICAL_OWNERSHIP_ELEMENTS_V1: usize = 1_048_576;
/// Maximum independently contracted output views in one function.
pub const MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HierarchicalOwnershipLocationV1 {
    block: usize,
    operation: usize,
}

impl HierarchicalOwnershipLocationV1 {
    pub const fn block(self) -> usize {
        self.block
    }

    pub const fn operation(self) -> usize {
        self.operation
    }
}

impl From<PlironTraceLocationV1> for HierarchicalOwnershipLocationV1 {
    fn from(location: PlironTraceLocationV1) -> Self {
        Self {
            block: location.block,
            operation: location.operation,
        }
    }
}

/// Exact execution owner of one logical output coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HierarchicalOwnerWitnessV1 {
    invocation: Vec<u64>,
    workgroup: u64,
    subgroup: u64,
    lane: u64,
    location: HierarchicalOwnershipLocationV1,
}

/// Exact hierarchy identity of an invocation required by a collective
/// contribution contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HierarchicalInvocationWitnessV1 {
    invocation: Vec<u64>,
    workgroup: u64,
    subgroup: u64,
    lane: u64,
}

impl HierarchicalInvocationWitnessV1 {
    pub fn invocation(&self) -> &[u64] {
        &self.invocation
    }

    pub const fn workgroup(&self) -> u64 {
        self.workgroup
    }

    pub const fn subgroup(&self) -> u64 {
        self.subgroup
    }

    pub const fn lane(&self) -> u64 {
        self.lane
    }
}

impl HierarchicalOwnerWitnessV1 {
    pub fn invocation(&self) -> &[u64] {
        &self.invocation
    }

    pub const fn workgroup(&self) -> u64 {
        self.workgroup
    }

    pub const fn subgroup(&self) -> u64 {
        self.subgroup
    }

    pub const fn lane(&self) -> u64 {
        self.lane
    }

    pub const fn location(&self) -> HierarchicalOwnershipLocationV1 {
        self.location
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HierarchicalOwnershipLevelV1 {
    Invocation,
    Subgroup,
    Workgroup,
    Grid,
}

/// Stable identity of one hierarchy region in a summary.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HierarchicalRegionIdentityV1 {
    Invocation(Vec<u64>),
    Subgroup { workgroup: u64, subgroup: u64 },
    Workgroup(u64),
    Grid(u64),
}

impl HierarchicalRegionIdentityV1 {
    pub const fn level(&self) -> HierarchicalOwnershipLevelV1 {
        match self {
            Self::Invocation(_) => HierarchicalOwnershipLevelV1::Invocation,
            Self::Subgroup { .. } => HierarchicalOwnershipLevelV1::Subgroup,
            Self::Workgroup(_) => HierarchicalOwnershipLevelV1::Workgroup,
            Self::Grid(_) => HierarchicalOwnershipLevelV1::Grid,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HierarchicalDimensionRangeV1 {
    minimum: u64,
    maximum: u64,
}

impl HierarchicalDimensionRangeV1 {
    pub const fn minimum(self) -> u64 {
        self.minimum
    }

    pub const fn maximum(self) -> u64 {
        self.maximum
    }
}

/// Bounded summary derived from the exact owned set at one hierarchy level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HierarchicalOwnershipRegionV1 {
    view: String,
    coverage: OwnershipCoverageAttr,
    identity: HierarchicalRegionIdentityV1,
    element_count: usize,
    bounds: Vec<HierarchicalDimensionRangeV1>,
    dense_rectangle: bool,
}

impl HierarchicalOwnershipRegionV1 {
    pub fn view(&self) -> &str {
        &self.view
    }

    pub const fn coverage(&self) -> OwnershipCoverageAttr {
        self.coverage
    }

    pub const fn identity(&self) -> &HierarchicalRegionIdentityV1 {
        &self.identity
    }

    pub const fn element_count(&self) -> usize {
        self.element_count
    }

    pub fn bounds(&self) -> &[HierarchicalDimensionRangeV1] {
        &self.bounds
    }

    pub const fn is_dense_rectangle(&self) -> bool {
        self.dense_rectangle
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HierarchicalOverlapClassV1 {
    WithinSubgroup,
    AcrossSubgroups,
    AcrossWorkgroups,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HierarchicalOwnershipFindingV1 {
    ContractLimitExceeded {
        actual: usize,
        limit: usize,
    },
    DuplicateContract {
        view: String,
        first: HierarchicalOwnershipLocationV1,
        second: HierarchicalOwnershipLocationV1,
    },
    ContractOutsideEntry {
        view: String,
        location: HierarchicalOwnershipLocationV1,
    },
    MalformedContract {
        location: HierarchicalOwnershipLocationV1,
        detail: &'static str,
    },
    ExecutionLayoutIncomplete {
        detail: String,
    },
    SparseIndexAnalysisIncomplete {
        detail: String,
    },
    TraceIncomplete {
        detail: String,
    },
    EffectDomainIncomplete {
        detail: String,
    },
    DynamicExtentIncomplete {
        view: String,
        dimension: usize,
    },
    ElementLimitExceeded {
        view: String,
        actual: u64,
        limit: usize,
    },
    UnresolvedCoordinate {
        view: String,
        location: HierarchicalOwnershipLocationV1,
        invocation: Vec<u64>,
        dimension: usize,
    },
    OutOfRange {
        view: String,
        coordinate: Vec<u64>,
        extents: Vec<u64>,
        owner: HierarchicalOwnerWitnessV1,
    },
    OverlappingOwners {
        view: String,
        coordinate: Vec<u64>,
        class: HierarchicalOverlapClassV1,
        first: HierarchicalOwnerWitnessV1,
        second: HierarchicalOwnerWitnessV1,
    },
    CoverageHole {
        view: String,
        coordinate: Vec<u64>,
        extents: Vec<u64>,
    },
    OutputOverwritten {
        view: String,
        coordinate: Vec<u64>,
        first: HierarchicalOwnerWitnessV1,
        overwrite: HierarchicalOwnerWitnessV1,
    },
    UnmodeledObservableWrite {
        view: String,
        location: HierarchicalOwnershipLocationV1,
    },
    UnmodeledObservableAllocationWrite {
        allocation_origin: u64,
        noalias_class: u64,
        location: HierarchicalOwnershipLocationV1,
    },
    MayAliasObservableWrite {
        contracted_view: String,
        alias_view: String,
        contracted_noalias_class: u64,
        alias_noalias_class: u64,
        location: HierarchicalOwnershipLocationV1,
    },
    AbnormalCompletion {
        view: String,
        invocation: HierarchicalInvocationWitnessV1,
        location: HierarchicalOwnershipLocationV1,
    },
    NonAtomicContribution {
        view: String,
        owner: HierarchicalOwnerWitnessV1,
    },
    MissingContribution {
        view: String,
        invocation: HierarchicalInvocationWitnessV1,
    },
    DuplicateContribution {
        view: String,
        invocation: HierarchicalInvocationWitnessV1,
        first: HierarchicalOwnerWitnessV1,
        second: HierarchicalOwnerWitnessV1,
    },
    NonRectangularRegion {
        view: String,
        region: HierarchicalRegionIdentityV1,
        missing: Vec<u64>,
    },
}

impl HierarchicalOwnershipFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::DuplicateContract { .. }
            | Self::ContractOutsideEntry { .. }
            | Self::OutOfRange { .. }
            | Self::OverlappingOwners { .. }
            | Self::CoverageHole { .. }
            | Self::OutputOverwritten { .. }
            | Self::UnmodeledObservableWrite { .. }
            | Self::UnmodeledObservableAllocationWrite { .. }
            | Self::MayAliasObservableWrite { .. }
            | Self::AbnormalCompletion { .. }
            | Self::NonAtomicContribution { .. }
            | Self::MissingContribution { .. }
            | Self::DuplicateContribution { .. }
            | Self::NonRectangularRegion { .. } => KernelCheckStatusV1::Rejected,
            Self::ContractLimitExceeded { .. }
            | Self::MalformedContract { .. }
            | Self::ExecutionLayoutIncomplete { .. }
            | Self::SparseIndexAnalysisIncomplete { .. }
            | Self::TraceIncomplete { .. }
            | Self::EffectDomainIncomplete { .. }
            | Self::DynamicExtentIncomplete { .. }
            | Self::ElementLimitExceeded { .. }
            | Self::UnresolvedCoordinate { .. } => KernelCheckStatusV1::Incomplete,
        }
    }
}

impl fmt::Display for HierarchicalOwnershipFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContractLimitExceeded { actual, limit } => write!(
                formatter,
                "error[FE2O3-OWN-003]: function has {actual} ownership contracts, exceeding analysis limit {limit}",
            ),
            Self::DuplicateContract {
                view,
                first,
                second,
            } => write!(
                formatter,
                "error[FE2O3-OWN-001]: {view} has duplicate ownership contracts at block {} op {} and block {} op {}",
                first.block, first.operation, second.block, second.operation,
            ),
            Self::ContractOutsideEntry { view, location } => write!(
                formatter,
                "error[FE2O3-OWN-001]: ownership contract for {view} appears at block {} op {}; contracts must be unconditional entry-block metadata",
                location.block, location.operation,
            ),
            Self::MalformedContract { location, detail } => write!(
                formatter,
                "error[FE2O3-OWN-002]: malformed ownership contract at block {} op {}: {detail}",
                location.block, location.operation,
            ),
            Self::ExecutionLayoutIncomplete { detail } => write!(
                formatter,
                "error[FE2O3-OWN-002]: GPU hierarchy ownership is incomplete because execution layout is unavailable: {detail}",
            ),
            Self::SparseIndexAnalysisIncomplete { detail } => write!(
                formatter,
                "error[FE2O3-OWN-002]: GPU hierarchy ownership is incomplete because sparse index analysis failed: {detail}",
            ),
            Self::TraceIncomplete { detail } => write!(
                formatter,
                "error[FE2O3-OWN-002]: GPU hierarchy ownership is incomplete because guarded invocation tracing failed: {detail}",
            ),
            Self::EffectDomainIncomplete { detail } => write!(
                formatter,
                "error[FE2O3-OWN-002]: mandatory ownership coverage is incomplete: {detail}",
            ),
            Self::DynamicExtentIncomplete { view, dimension } => write!(
                formatter,
                "error[FE2O3-OWN-002]: ownership of {view} dimension {dimension} is incomplete; its dynamic extent has no compile-time value or symbolic coverage proof",
            ),
            Self::ElementLimitExceeded {
                view,
                actual,
                limit,
            } => write!(
                formatter,
                "error[FE2O3-OWN-003]: exact ownership domain for {view} has {actual} elements, exceeding analysis limit {limit}",
            ),
            Self::UnresolvedCoordinate {
                view,
                location,
                invocation,
                dimension,
            } => write!(
                formatter,
                "error[FE2O3-OWN-002]: cannot resolve {view} coordinate dimension {dimension} for invocation {invocation:?} at block {} op {}",
                location.block, location.operation,
            ),
            Self::OutOfRange {
                view,
                coordinate,
                extents,
                owner,
            } => write!(
                formatter,
                "error[FE2O3-OWN-004]: invocation {:?} (workgroup {}, subgroup {}, lane {}) owns out-of-range {view}{coordinate:?} for extents {extents:?}; write is at block {} op {}",
                owner.invocation,
                owner.workgroup,
                owner.subgroup,
                owner.lane,
                owner.location.block,
                owner.location.operation,
            ),
            Self::OverlappingOwners {
                view,
                coordinate,
                class,
                first,
                second,
            } => write!(
                formatter,
                "error[FE2O3-OWN-005]: {view}{coordinate:?} has {:?} owners: invocation {:?} (workgroup {}, subgroup {}, lane {}) at block {} op {} and invocation {:?} (workgroup {}, subgroup {}, lane {}) at block {} op {}; failed proof: hierarchy partitions must be disjoint",
                class,
                first.invocation,
                first.workgroup,
                first.subgroup,
                first.lane,
                first.location.block,
                first.location.operation,
                second.invocation,
                second.workgroup,
                second.subgroup,
                second.lane,
                second.location.block,
                second.location.operation,
            ),
            Self::CoverageHole {
                view,
                coordinate,
                extents,
            } => write!(
                formatter,
                "error[FE2O3-OWN-006]: exact ownership of {view} has a hole at logical coordinate {coordinate:?} within extents {extents:?}; no invocation, subgroup, or workgroup owns that element",
            ),
            Self::OutputOverwritten {
                view,
                coordinate,
                first,
                overwrite,
            } => write!(
                formatter,
                "error[FE2O3-OWN-008]: total output {view}{coordinate:?} is overwritten by invocation {:?} at block {} op {}; its first write was invocation {:?} at block {} op {}; failed proof: every logical output must have one final observable write",
                overwrite.invocation,
                overwrite.location.block,
                overwrite.location.operation,
                first.invocation,
                first.location.block,
                first.location.operation,
            ),
            Self::UnmodeledObservableWrite { view, location } => write!(
                formatter,
                "error[FE2O3-OWN-009]: observable global write to {view} at block {} op {} has no ownership contract; total-output admission requires every observable global write to be modeled",
                location.block, location.operation,
            ),
            Self::UnmodeledObservableAllocationWrite {
                allocation_origin,
                noalias_class,
                location,
            } => write!(
                formatter,
                "error[FE2O3-OWN-015]: whole-allocation global write at block {} op {} (allocation origin {allocation_origin}, noalias class {noalias_class}) has no coordinate-level ownership contract; total-output admission requires every observable global write to be modeled",
                location.block, location.operation,
            ),
            Self::MayAliasObservableWrite {
                contracted_view,
                alias_view,
                contracted_noalias_class,
                alias_noalias_class,
                location,
            } => write!(
                formatter,
                "error[FE2O3-OWN-013]: total output {contracted_view} may alias observable write through {alias_view} at block {} op {} (noalias classes {contracted_noalias_class} and {alias_noalias_class}); distinct nonzero classes are required to prove disjoint final outputs",
                location.block, location.operation,
            ),
            Self::AbnormalCompletion {
                view,
                invocation,
                location,
            } => write!(
                formatter,
                "error[FE2O3-OWN-014]: invocation {:?} (workgroup {}, subgroup {}, lane {}) traps at block {} op {} while proving coverage of {view}; total and collective coverage require normal completion",
                invocation.invocation,
                invocation.workgroup,
                invocation.subgroup,
                invocation.lane,
                location.block,
                location.operation,
            ),
            Self::NonAtomicContribution { view, owner } => write!(
                formatter,
                "error[FE2O3-OWN-010]: collective contribution to {view} by invocation {:?} at block {} op {} is not atomic; contribution coverage requires atomic writes",
                owner.invocation, owner.location.block, owner.location.operation,
            ),
            Self::MissingContribution { view, invocation } => write!(
                formatter,
                "error[FE2O3-OWN-011]: collective contribution coverage for {view} has no contribution from invocation {:?} (workgroup {}, subgroup {}, lane {})",
                invocation.invocation, invocation.workgroup, invocation.subgroup, invocation.lane,
            ),
            Self::DuplicateContribution {
                view,
                invocation,
                first,
                second,
            } => write!(
                formatter,
                "error[FE2O3-OWN-012]: collective contribution coverage for {view} has two contributions from invocation {:?} (workgroup {}, subgroup {}, lane {}) at block {} op {} and block {} op {}",
                invocation.invocation,
                invocation.workgroup,
                invocation.subgroup,
                invocation.lane,
                first.location.block,
                first.location.operation,
                second.location.block,
                second.location.operation,
            ),
            Self::NonRectangularRegion {
                view,
                region,
                missing,
            } => write!(
                formatter,
                "error[FE2O3-OWN-007]: {region:?} ownership of {view} is not a dense tile; coordinate {missing:?} is missing inside its bounding rectangle",
            ),
        }
    }
}

/// Non-vacuous counts retained with the mandatory ownership report.
///
/// A declared contract is proved only when its complete finite-domain analysis
/// adds no rejected or incomplete finding. These counts are compiler evidence,
/// not artifact or launch authority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HierarchicalCoverageProofSummaryV1 {
    total_view_declared: usize,
    total_view_proved: usize,
    collective_contributions_declared: usize,
    collective_contributions_proved: usize,
}

impl HierarchicalCoverageProofSummaryV1 {
    pub const fn total_view_declared(self) -> usize {
        self.total_view_declared
    }

    pub const fn total_view_proved(self) -> usize {
        self.total_view_proved
    }

    pub const fn collective_contributions_declared(self) -> usize {
        self.collective_contributions_declared
    }

    pub const fn collective_contributions_proved(self) -> usize {
        self.collective_contributions_proved
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HierarchicalOwnershipReportV1 {
    findings: Vec<HierarchicalOwnershipFindingV1>,
    regions: Vec<HierarchicalOwnershipRegionV1>,
    coverage_summary: HierarchicalCoverageProofSummaryV1,
}

impl HierarchicalOwnershipReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::HierarchicalOwnership
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[HierarchicalOwnershipFindingV1] {
        &self.findings
    }

    pub fn regions(&self) -> &[HierarchicalOwnershipRegionV1] {
        &self.regions
    }

    pub const fn coverage_summary(&self) -> HierarchicalCoverageProofSummaryV1 {
        self.coverage_summary
    }

    /// True only for a clean report containing at least one proved total-view
    /// contract. A clean function with no such declaration is not evidence.
    pub fn all_total_view_contracts_are_proved(&self) -> bool {
        self.is_clean()
            && self.coverage_summary.total_view_declared != 0
            && self.coverage_summary.total_view_declared == self.coverage_summary.total_view_proved
    }

    /// True only for a clean report containing at least one proved collective
    /// contribution contract. This does not prove a reduction's value.
    pub fn all_collective_contribution_contracts_are_proved(&self) -> bool {
        self.is_clean()
            && self.coverage_summary.collective_contributions_declared != 0
            && self.coverage_summary.collective_contributions_declared
                == self.coverage_summary.collective_contributions_proved
    }

    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HierarchicalOwnershipCheckErrorV1 {
    report: HierarchicalOwnershipReportV1,
}

impl HierarchicalOwnershipCheckErrorV1 {
    pub const fn report(&self) -> &HierarchicalOwnershipReportV1 {
        &self.report
    }
}

impl fmt::Display for HierarchicalOwnershipCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, finding) in self.report.findings.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for HierarchicalOwnershipCheckErrorV1 {}

#[derive(Clone)]
struct ContractV1 {
    location: HierarchicalOwnershipLocationV1,
    view: Value,
    view_name: String,
    view_op: RankedViewOp,
    coverage: OwnershipCoverageAttr,
    partition: OwnershipPartitionAttr,
}

pub fn run_pliron_hierarchical_ownership_check_v1(
    context: &Context,
    function: &FuncOp,
) -> HierarchicalOwnershipReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_hierarchical_ownership_check_with_analyses_v1(context, function, &mut analyses)
}

pub(crate) fn run_pliron_hierarchical_ownership_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> HierarchicalOwnershipReportV1 {
    let contracts = match collect_contracts(context, function) {
        Ok(contracts) if contracts.is_empty() => return clean(),
        Ok(contracts) => contracts,
        Err(finding) => return one(*finding),
    };
    let mut coverage_summary = declared_coverage_summary(&contracts);

    analyses.prepare_execution_layout(context, function);
    let layout = match analyses.execution_layout() {
        Ok(Some(layout)) => layout,
        Ok(None) => {
            return one_with_summary(
                HierarchicalOwnershipFindingV1::ExecutionLayoutIncomplete {
                    detail: "kernel.ownership_contract requires gpu.execution_layout".to_owned(),
                },
                coverage_summary,
            );
        }
        Err(failure) => {
            return one_with_summary(
                HierarchicalOwnershipFindingV1::ExecutionLayoutIncomplete {
                    detail: trace_failure_detail(failure),
                },
                coverage_summary,
            );
        }
    };
    analyses.prepare_sparse_indices(context, function);
    if let Err(failure) = analyses.sparse_indices() {
        return one_with_summary(
            HierarchicalOwnershipFindingV1::SparseIndexAnalysisIncomplete {
                detail: format!("{failure:?}"),
            },
            coverage_summary,
        );
    }
    let needs_effect_domain = contracts
        .iter()
        .any(|contract| contract.coverage == OwnershipCoverageAttr::ExactEffectDomain);
    let needs_mandatory_bounds = contracts.iter().any(|contract| {
        matches!(
            contract.coverage,
            OwnershipCoverageAttr::ExactEffectDomain
                | OwnershipCoverageAttr::TotalView
                | OwnershipCoverageAttr::CollectiveContributions
        )
    });
    let mut mandatory_bounds_failure = None;
    if needs_mandatory_bounds {
        let bounds = run_pliron_ranked_bounds_check_with_analyses_v1(context, function, analyses);
        if !bounds.is_clean() {
            let detail = bounds
                .findings()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            let detail = format!("mandatory ranked bounds failed: {detail}");
            if needs_effect_domain {
                return one_with_summary(
                    HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail },
                    coverage_summary,
                );
            }
            mandatory_bounds_failure = Some(detail);
        }
    }
    if needs_effect_domain {
        let race = run_pliron_ranked_race_check_with_analyses_v1(context, function, analyses);
        if !race.is_clean() {
            return one_with_summary(
                HierarchicalOwnershipFindingV1::EffectDomainIncomplete {
                    detail: race
                        .findings()
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; "),
                },
                coverage_summary,
            );
        }
    }
    let needs_total_output = contracts
        .iter()
        .any(|contract| contract.coverage == OwnershipCoverageAttr::TotalView);
    if needs_total_output
        && let Some(finding) =
            first_unmodeled_or_aliasing_observable_write(context, function, &contracts)
    {
        return one_with_summary(finding, coverage_summary);
    }
    let needs_exact_trace = contracts.iter().any(|contract| {
        matches!(
            contract.coverage,
            OwnershipCoverageAttr::ExactView
                | OwnershipCoverageAttr::TotalView
                | OwnershipCoverageAttr::CollectiveContributions
        )
    });
    if needs_exact_trace {
        analyses.prepare_exact_trace(context, function);
    }
    let traces = if needs_exact_trace {
        match analyses.exact_trace() {
            Ok(traces) => Some(traces),
            Err(failure) => {
                return one_with_summary(
                    HierarchicalOwnershipFindingV1::TraceIncomplete {
                        detail: trace_failure_detail(failure),
                    },
                    coverage_summary,
                );
            }
        }
    } else {
        None
    };
    let sparse = analyses
        .sparse_indices()
        .expect("sparse analysis was checked before ownership");

    let mut findings = Vec::new();
    let mut regions = Vec::new();
    for contract in contracts {
        let (extents, element_count) = match contract.coverage {
            OwnershipCoverageAttr::ExactView | OwnershipCoverageAttr::TotalView => {
                let extents = match resolve_extents(context, sparse, &contract) {
                    Ok(extents) => extents,
                    Err(finding) => {
                        findings.push(*finding);
                        continue;
                    }
                };
                let element_count = match bounded_element_count(&contract.view_name, &extents) {
                    Ok(count) => count,
                    Err(finding) => {
                        findings.push(*finding);
                        continue;
                    }
                };
                (Some(extents), Some(element_count))
            }
            OwnershipCoverageAttr::CollectiveContributions => {
                let extents = match resolve_extents(context, sparse, &contract) {
                    Ok(extents) => extents,
                    Err(finding) => {
                        findings.push(*finding);
                        continue;
                    }
                };
                (Some(extents), None)
            }
            OwnershipCoverageAttr::ExactEffectDomain => {
                if let Err(finding) = validate_effect_domain_site(context, function, &contract) {
                    findings.push(*finding);
                }
                continue;
            }
        };
        let finding_count = findings.len();
        analyze_contract(
            context,
            &contract,
            extents.as_deref(),
            element_count,
            traces.expect("whole-domain contracts prepared exact traces"),
            layout.grid,
            &mut findings,
            &mut regions,
        );
        if findings.len() == finding_count && mandatory_bounds_failure.is_none() {
            match contract.coverage {
                OwnershipCoverageAttr::TotalView => coverage_summary.total_view_proved += 1,
                OwnershipCoverageAttr::CollectiveContributions => {
                    coverage_summary.collective_contributions_proved += 1;
                }
                OwnershipCoverageAttr::ExactView | OwnershipCoverageAttr::ExactEffectDomain => {}
            }
        }
    }
    if findings.is_empty()
        && let Some(detail) = mandatory_bounds_failure
    {
        findings.push(HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail });
    }
    HierarchicalOwnershipReportV1 {
        findings,
        regions,
        coverage_summary,
    }
}

fn first_unmodeled_or_aliasing_observable_write(
    context: &Context,
    function: &FuncOp,
    contracts: &[ContractV1],
) -> Option<HierarchicalOwnershipFindingV1> {
    let modeled = contracts
        .iter()
        .map(|contract| contract.view)
        .collect::<HashSet<_>>();
    let total_outputs = contracts
        .iter()
        .filter(|contract| contract.coverage == OwnershipCoverageAttr::TotalView)
        .collect::<Vec<_>>();
    for (block, basic_block) in function
        .get_region(context)
        .deref(context)
        .iter(context)
        .enumerate()
    {
        for (operation, raw) in basic_block.deref(context).iter(context).enumerate() {
            let op = Operation::get_op_dyn(raw, context);
            if let Some(effect) = op.downcast_ref::<AllocationEffectOp>()
                && effect.memory_space(context) == Some(MemorySpaceAttr::Global)
                && effect
                    .kind(context)
                    .is_some_and(|kind| kind.writes_memory())
            {
                return Some(
                    HierarchicalOwnershipFindingV1::UnmodeledObservableAllocationWrite {
                        allocation_origin: effect.allocation_origin(context).unwrap_or(0),
                        noalias_class: effect.noalias_class(context).unwrap_or(0),
                        location: HierarchicalOwnershipLocationV1 { block, operation },
                    },
                );
            }
            let Some(access) = op.downcast_ref::<RankedAccessOp>() else {
                continue;
            };
            if !access
                .kind(context)
                .is_some_and(|kind| kind.writes_memory())
            {
                continue;
            }
            let view = access.view(context);
            let Some(view_op) = view
                .defining_op()
                .map(|definition| Operation::get_op_dyn(definition, context))
                .and_then(|definition| definition.downcast_ref::<RankedViewOp>().copied())
            else {
                continue;
            };
            if view_op.memory_space(context) != Some(MemorySpaceAttr::Global) {
                continue;
            }
            let location = HierarchicalOwnershipLocationV1 { block, operation };
            if !modeled.contains(&view) {
                return Some(HierarchicalOwnershipFindingV1::UnmodeledObservableWrite {
                    view: view.unique_name(context).to_string(),
                    location,
                });
            }
            let alias_noalias_class = view_op.noalias_class(context).unwrap_or(0);
            for contract in &total_outputs {
                if contract.view == view {
                    continue;
                }
                let contracted_noalias_class = contract.view_op.noalias_class(context).unwrap_or(0);
                if contracted_noalias_class == 0
                    || alias_noalias_class == 0
                    || contracted_noalias_class == alias_noalias_class
                {
                    return Some(HierarchicalOwnershipFindingV1::MayAliasObservableWrite {
                        contracted_view: contract.view_name.clone(),
                        alias_view: view.unique_name(context).to_string(),
                        contracted_noalias_class,
                        alias_noalias_class,
                        location,
                    });
                }
            }
        }
    }
    None
}

pub(crate) fn require_pliron_hierarchical_ownership_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<HierarchicalOwnershipReportV1, HierarchicalOwnershipCheckErrorV1> {
    let report =
        run_pliron_hierarchical_ownership_check_with_analyses_v1(context, function, analyses);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(HierarchicalOwnershipCheckErrorV1 { report })
    }
}

pub fn require_pliron_hierarchical_ownership_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<HierarchicalOwnershipReportV1, HierarchicalOwnershipCheckErrorV1> {
    let report = run_pliron_hierarchical_ownership_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(HierarchicalOwnershipCheckErrorV1 { report })
    }
}

fn collect_contracts(
    context: &Context,
    function: &FuncOp,
) -> Result<Vec<ContractV1>, Box<HierarchicalOwnershipFindingV1>> {
    let mut contracts = Vec::new();
    let mut by_view = HashMap::new();
    for (block, basic_block) in function
        .get_region(context)
        .deref(context)
        .iter(context)
        .enumerate()
    {
        for (operation, raw) in basic_block.deref(context).iter(context).enumerate() {
            let op = Operation::get_op_dyn(raw, context);
            let Some(contract) = op.downcast_ref::<OwnershipContractOp>() else {
                continue;
            };
            if contracts.len() == MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1 {
                return Err(Box::new(
                    HierarchicalOwnershipFindingV1::ContractLimitExceeded {
                        actual: contracts.len() + 1,
                        limit: MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1,
                    },
                ));
            }
            let location = HierarchicalOwnershipLocationV1 { block, operation };
            let raw = contract.get_operation().deref(context);
            if raw.get_num_operands() != 1
                || contract.coverage(context).is_none()
                || contract.partition(context).is_none()
            {
                return Err(Box::new(
                    HierarchicalOwnershipFindingV1::MalformedContract {
                        location,
                        detail: "expected one ranked-view operand and closed coverage/partition attributes",
                    },
                ));
            }
            let view = contract.view(context);
            let view_name = view.unique_name(context).to_string();
            if block != 0 {
                return Err(Box::new(
                    HierarchicalOwnershipFindingV1::ContractOutsideEntry {
                        view: view_name,
                        location,
                    },
                ));
            }
            if let Some(first) = by_view.insert(view, location) {
                return Err(Box::new(
                    HierarchicalOwnershipFindingV1::DuplicateContract {
                        view: view_name,
                        first,
                        second: location,
                    },
                ));
            }
            let Some(definition) = view.defining_op() else {
                return Err(Box::new(
                    HierarchicalOwnershipFindingV1::SparseIndexAnalysisIncomplete {
                        detail: format!("contracted view {view_name} has no definition"),
                    },
                ));
            };
            let definition = Operation::get_op_dyn(definition, context);
            let Some(view_op) = definition.downcast_ref::<RankedViewOp>() else {
                return Err(Box::new(
                    HierarchicalOwnershipFindingV1::SparseIndexAnalysisIncomplete {
                        detail: format!("contracted value {view_name} is not a ranked view"),
                    },
                ));
            };
            contracts.push(ContractV1 {
                location,
                view,
                view_name,
                view_op: *view_op,
                coverage: contract
                    .coverage(context)
                    .unwrap_or(OwnershipCoverageAttr::ExactView),
                partition: contract
                    .partition(context)
                    .unwrap_or(OwnershipPartitionAttr::ExactSets),
            });
        }
    }
    Ok(contracts)
}

fn validate_effect_domain_site(
    context: &Context,
    function: &FuncOp,
    contract: &ContractV1,
) -> Result<(), Box<HierarchicalOwnershipFindingV1>> {
    let mut writes = 0_usize;
    for block in function.get_region(context).deref(context).iter(context) {
        for raw in block.deref(context).iter(context) {
            let operation = Operation::get_op_dyn(raw, context);
            let Some(access) = operation.downcast_ref::<RankedAccessOp>() else {
                continue;
            };
            if access.view(context) == contract.view
                && access
                    .kind(context)
                    .is_some_and(|kind| kind.writes_memory())
            {
                writes = writes.saturating_add(1);
            }
        }
    }
    if writes != 1 {
        return Err(Box::new(
            HierarchicalOwnershipFindingV1::MalformedContract {
                location: contract.location,
                detail: "ExactEffectDomain requires exactly one bounds-clean, race-free write site on the contracted view",
            },
        ));
    }
    Ok(())
}

fn resolve_extents(
    context: &Context,
    sparse: &crate::SparseIndexAnalysisV1,
    contract: &ContractV1,
) -> Result<Vec<u64>, Box<HierarchicalOwnershipFindingV1>> {
    let Some(view_type) = contract.view_op.view_type(context) else {
        return Err(Box::new(
            HierarchicalOwnershipFindingV1::SparseIndexAnalysisIncomplete {
                detail: format!("contracted view {} has no ranked type", contract.view_name),
            },
        ));
    };
    view_type
        .deref(context)
        .shape()
        .iter()
        .copied()
        .enumerate()
        .map(|(dimension, extent)| {
            if extent != DYNAMIC_EXTENT {
                return Ok(extent);
            }
            contract
                .view_op
                .dynamic_extent(context, dimension)
                .and_then(|value| sparse.fact(value).constant_value())
                .ok_or_else(|| {
                    Box::new(HierarchicalOwnershipFindingV1::DynamicExtentIncomplete {
                        view: contract.view_name.clone(),
                        dimension,
                    })
                })
        })
        .collect()
}

fn bounded_element_count(
    view: &str,
    extents: &[u64],
) -> Result<usize, Box<HierarchicalOwnershipFindingV1>> {
    let actual = extents
        .iter()
        .try_fold(1_u64, |total, extent| total.checked_mul(*extent))
        .unwrap_or(u64::MAX);
    if actual > MAX_HIERARCHICAL_OWNERSHIP_ELEMENTS_V1 as u64 {
        return Err(Box::new(
            HierarchicalOwnershipFindingV1::ElementLimitExceeded {
                view: view.to_owned(),
                actual,
                limit: MAX_HIERARCHICAL_OWNERSHIP_ELEMENTS_V1,
            },
        ));
    }
    Ok(actual as usize)
}

#[allow(clippy::too_many_arguments)]
fn analyze_contract(
    context: &Context,
    contract: &ContractV1,
    extents: Option<&[u64]>,
    element_count: Option<usize>,
    traces: &[PlironInvocationTraceV1],
    grid: u64,
    findings: &mut Vec<HierarchicalOwnershipFindingV1>,
    regions: &mut Vec<HierarchicalOwnershipRegionV1>,
) {
    let mut owners = BTreeMap::<Vec<u64>, HierarchicalOwnerWitnessV1>::new();
    let mut sets = BTreeMap::<HierarchicalRegionIdentityV1, BTreeSet<Vec<u64>>>::new();
    let collective = contract.coverage == OwnershipCoverageAttr::CollectiveContributions;
    for trace in traces {
        if matches!(
            contract.coverage,
            OwnershipCoverageAttr::TotalView | OwnershipCoverageAttr::CollectiveContributions
        ) && let Some(PlironTraceEventV1::Trap { location }) = trace
            .events
            .iter()
            .find(|event| matches!(event, PlironTraceEventV1::Trap { .. }))
        {
            findings.push(HierarchicalOwnershipFindingV1::AbnormalCompletion {
                view: contract.view_name.clone(),
                invocation: HierarchicalInvocationWitnessV1 {
                    invocation: trace.invocation.clone(),
                    workgroup: trace.workgroup,
                    subgroup: trace.subgroup,
                    lane: trace.lane,
                },
                location: (*location).into(),
            });
            return;
        }
        let mut contributions = Vec::<HierarchicalOwnerWitnessV1>::new();
        for event in &trace.events {
            let PlironTraceEventV1::Memory {
                location,
                view,
                access,
                indices,
                ..
            } = event
            else {
                continue;
            };
            if *view != contract.view || !access.writes_memory() {
                continue;
            }
            let witness = HierarchicalOwnerWitnessV1 {
                invocation: trace.invocation.clone(),
                workgroup: trace.workgroup,
                subgroup: trace.subgroup,
                lane: trace.lane,
                location: (*location).into(),
            };
            let Some(coordinate) = indices.iter().copied().collect::<Option<Vec<_>>>() else {
                let dimension = indices
                    .iter()
                    .position(Option::is_none)
                    .expect("failed coordinate contains an unresolved dimension");
                findings.push(HierarchicalOwnershipFindingV1::UnresolvedCoordinate {
                    view: contract.view_name.clone(),
                    location: witness.location,
                    invocation: witness.invocation,
                    dimension,
                });
                return;
            };
            let rank = contract
                .view_op
                .view_type(context)
                .map(|ty| ty.deref(context).shape().len())
                .unwrap_or(0);
            if coordinate.len() != rank
                || extents.is_some_and(|extents| {
                    coordinate
                        .iter()
                        .zip(extents)
                        .any(|(coordinate, extent)| coordinate >= extent)
                })
            {
                findings.push(HierarchicalOwnershipFindingV1::OutOfRange {
                    view: contract.view_name.clone(),
                    coordinate,
                    extents: extents.unwrap_or_default().to_vec(),
                    owner: witness,
                });
                return;
            }
            if collective {
                if !matches!(
                    access,
                    AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite
                ) {
                    findings.push(HierarchicalOwnershipFindingV1::NonAtomicContribution {
                        view: contract.view_name.clone(),
                        owner: witness,
                    });
                    return;
                }
                if let Some(first) = contributions.first() {
                    findings.push(HierarchicalOwnershipFindingV1::DuplicateContribution {
                        view: contract.view_name.clone(),
                        invocation: HierarchicalInvocationWitnessV1 {
                            invocation: trace.invocation.clone(),
                            workgroup: trace.workgroup,
                            subgroup: trace.subgroup,
                            lane: trace.lane,
                        },
                        first: first.clone(),
                        second: witness,
                    });
                    return;
                }
                contributions.push(witness.clone());
            } else if let Some(first) = owners.get(&coordinate) {
                if first.invocation != witness.invocation {
                    let class = if first.workgroup != witness.workgroup {
                        HierarchicalOverlapClassV1::AcrossWorkgroups
                    } else if first.subgroup != witness.subgroup {
                        HierarchicalOverlapClassV1::AcrossSubgroups
                    } else {
                        HierarchicalOverlapClassV1::WithinSubgroup
                    };
                    findings.push(HierarchicalOwnershipFindingV1::OverlappingOwners {
                        view: contract.view_name.clone(),
                        coordinate,
                        class,
                        first: first.clone(),
                        second: witness,
                    });
                    return;
                }
                if contract.coverage == OwnershipCoverageAttr::TotalView {
                    findings.push(HierarchicalOwnershipFindingV1::OutputOverwritten {
                        view: contract.view_name.clone(),
                        coordinate,
                        first: first.clone(),
                        overwrite: witness,
                    });
                    return;
                }
            }
            owners.entry(coordinate.clone()).or_insert(witness);
            for identity in [
                HierarchicalRegionIdentityV1::Invocation(trace.invocation.clone()),
                HierarchicalRegionIdentityV1::Subgroup {
                    workgroup: trace.workgroup,
                    subgroup: trace.subgroup,
                },
                HierarchicalRegionIdentityV1::Workgroup(trace.workgroup),
                HierarchicalRegionIdentityV1::Grid(grid),
            ] {
                sets.entry(identity).or_default().insert(coordinate.clone());
            }
        }
        if collective && contributions.is_empty() {
            findings.push(HierarchicalOwnershipFindingV1::MissingContribution {
                view: contract.view_name.clone(),
                invocation: HierarchicalInvocationWitnessV1 {
                    invocation: trace.invocation.clone(),
                    workgroup: trace.workgroup,
                    subgroup: trace.subgroup,
                    lane: trace.lane,
                },
            });
            return;
        }
    }

    if contract.partition == OwnershipPartitionAttr::DenseRectangles {
        for (identity, coordinates) in &sets {
            if matches!(
                identity,
                HierarchicalRegionIdentityV1::Subgroup { .. }
                    | HierarchicalRegionIdentityV1::Workgroup(_)
            ) && let Some(missing) = first_rectangle_hole(coordinates)
            {
                findings.push(HierarchicalOwnershipFindingV1::NonRectangularRegion {
                    view: contract.view_name.clone(),
                    region: identity.clone(),
                    missing,
                });
                return;
            }
        }
    }

    if !collective
        && let (Some(extents), Some(element_count)) = (extents, element_count)
        && owners.len() != element_count
        && let Some(coordinate) = first_domain_hole(extents, &owners)
    {
        findings.push(HierarchicalOwnershipFindingV1::CoverageHole {
            view: contract.view_name.clone(),
            coordinate,
            extents: extents.to_vec(),
        });
        return;
    }

    regions.extend(sets.into_iter().map(|(identity, coordinates)| {
        let bounds = coordinate_bounds(&coordinates);
        let dense_rectangle = rectangle_volume(&bounds) == Some(coordinates.len());
        HierarchicalOwnershipRegionV1 {
            view: contract.view_name.clone(),
            coverage: contract.coverage,
            identity,
            element_count: coordinates.len(),
            bounds,
            dense_rectangle,
        }
    }));
}

fn coordinate_bounds(coordinates: &BTreeSet<Vec<u64>>) -> Vec<HierarchicalDimensionRangeV1> {
    let Some(first) = coordinates.first() else {
        return Vec::new();
    };
    let mut bounds = first
        .iter()
        .map(|coordinate| HierarchicalDimensionRangeV1 {
            minimum: *coordinate,
            maximum: *coordinate,
        })
        .collect::<Vec<_>>();
    for coordinate in coordinates.iter().skip(1) {
        for (range, coordinate) in bounds.iter_mut().zip(coordinate) {
            range.minimum = range.minimum.min(*coordinate);
            range.maximum = range.maximum.max(*coordinate);
        }
    }
    bounds
}

fn rectangle_volume(bounds: &[HierarchicalDimensionRangeV1]) -> Option<usize> {
    bounds.iter().try_fold(1_usize, |volume, range| {
        let extent = range.maximum.checked_sub(range.minimum)?.checked_add(1)?;
        volume.checked_mul(usize::try_from(extent).ok()?)
    })
}

fn first_rectangle_hole(coordinates: &BTreeSet<Vec<u64>>) -> Option<Vec<u64>> {
    let bounds = coordinate_bounds(coordinates);
    if rectangle_volume(&bounds) == Some(coordinates.len()) {
        return None;
    }
    first_coordinate_matching(
        &bounds.iter().map(|range| range.minimum).collect::<Vec<_>>(),
        &bounds.iter().map(|range| range.maximum).collect::<Vec<_>>(),
        |coordinate| !coordinates.contains(coordinate),
    )
}

fn first_domain_hole(
    extents: &[u64],
    owners: &BTreeMap<Vec<u64>, HierarchicalOwnerWitnessV1>,
) -> Option<Vec<u64>> {
    if extents.contains(&0) {
        return None;
    }
    first_coordinate_matching(
        &vec![0; extents.len()],
        &extents.iter().map(|extent| extent - 1).collect::<Vec<_>>(),
        |coordinate| !owners.contains_key(coordinate),
    )
}

fn first_coordinate_matching(
    minima: &[u64],
    maxima: &[u64],
    mut predicate: impl FnMut(&Vec<u64>) -> bool,
) -> Option<Vec<u64>> {
    if minima.is_empty() || minima.len() != maxima.len() {
        return None;
    }
    let mut coordinate = minima.to_vec();
    loop {
        if predicate(&coordinate) {
            return Some(coordinate);
        }
        let mut dimension = 0;
        loop {
            if dimension == coordinate.len() {
                return None;
            }
            if coordinate[dimension] < maxima[dimension] {
                coordinate[dimension] += 1;
                coordinate[..dimension].copy_from_slice(&minima[..dimension]);
                break;
            }
            dimension += 1;
        }
    }
}

fn clean() -> HierarchicalOwnershipReportV1 {
    HierarchicalOwnershipReportV1 {
        findings: Vec::new(),
        regions: Vec::new(),
        coverage_summary: HierarchicalCoverageProofSummaryV1::default(),
    }
}

fn one(finding: HierarchicalOwnershipFindingV1) -> HierarchicalOwnershipReportV1 {
    one_with_summary(finding, HierarchicalCoverageProofSummaryV1::default())
}

fn one_with_summary(
    finding: HierarchicalOwnershipFindingV1,
    coverage_summary: HierarchicalCoverageProofSummaryV1,
) -> HierarchicalOwnershipReportV1 {
    HierarchicalOwnershipReportV1 {
        findings: vec![finding],
        regions: Vec::new(),
        coverage_summary,
    }
}

fn declared_coverage_summary(contracts: &[ContractV1]) -> HierarchicalCoverageProofSummaryV1 {
    let mut summary = HierarchicalCoverageProofSummaryV1::default();
    for contract in contracts {
        match contract.coverage {
            OwnershipCoverageAttr::TotalView => summary.total_view_declared += 1,
            OwnershipCoverageAttr::CollectiveContributions => {
                summary.collective_contributions_declared += 1;
            }
            OwnershipCoverageAttr::ExactView | OwnershipCoverageAttr::ExactEffectDomain => {}
        }
    }
    summary
}
