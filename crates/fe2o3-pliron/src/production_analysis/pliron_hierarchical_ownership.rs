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
    fmt::Write as _,
};

use dialect_kernel::{
    AccessKindAttr, AllocationEffectOp, DYNAMIC_EXTENT, MemorySpaceAttr, OwnershipContractOp,
    OwnershipCoverageAttr, OwnershipPartitionAttr, RankedAccessOp, RankedViewOp,
};
use pliron::{
    builtin::ops::FuncOp, common_traits::Named, context::Context, op::Op, operation::Operation,
    value::Value,
};

use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_barrier::trace_failure_detail;
use crate::production_analysis::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceLocationV1,
    ProductionInvocationTraceResourceAdmissionV1,
};
use crate::production_analysis::pliron_race::run_pliron_ranked_race_check_with_analyses_v1;
use crate::production_analysis::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1};
use crate::{PresburgerCoverageDecisionV1, PresburgerFiniteImageV1};

#[path = "pliron_conditional_prefix_ownership_v1.rs"]
mod conditional_prefix_v1;
pub use conditional_prefix_v1::{
    ConditionalPrefixConditionV1, ConditionalPrefixCoverageV1, ConditionalPrefixDerivationErrorV1,
    ConditionalPrefixExtentSourceV1, ConditionalPrefixExtentV1, ConditionalPrefixGuardAtomV1,
    ConditionalPrefixHostBindingObligationV1, ConditionalPrefixLaunchV1, ConditionalPrefixSiteV1,
    MAX_CONDITIONAL_PREFIX_ARGUMENTS_V1, MAX_CONDITIONAL_PREFIX_ATTRIBUTE_TEXT_BYTES_V1,
    MAX_CONDITIONAL_PREFIX_ATTRIBUTES_PER_ENTITY_V1, MAX_CONDITIONAL_PREFIX_BLOCKS_V1,
    MAX_CONDITIONAL_PREFIX_DNF_TERMS_V1, MAX_CONDITIONAL_PREFIX_GUARD_ATOMS_V1,
    MAX_CONDITIONAL_PREFIX_INPUTS_V1, MAX_CONDITIONAL_PREFIX_OPERATIONS_V1,
    MAX_CONDITIONAL_PREFIX_WORK_V1,
};

/// Maximum logical output elements materialized by one exact coverage proof.
pub const MAX_HIERARCHICAL_OWNERSHIP_ELEMENTS_V1: usize = 1_048_576;
/// Maximum independently contracted output views in one function.
pub const MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1: usize = 256;
const MAX_HIERARCHICAL_OWNERSHIP_DIAGNOSTIC_BYTES_V1: usize = 4_096;

fn ownership_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        resource: "hierarchical ownership resource upper bound",
    }
}

fn checked_ownership_sum_v1(values: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(ownership_resource_overflow_v1)
    })
}

fn checked_ownership_product_v1(
    lhs: usize,
    rhs: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(ownership_resource_overflow_v1)
}

/// Charges all exact-domain ownership maps, coordinate copies, region
/// summaries, coverage searches, and diagnostics before they are built.
///
/// One write can be retained in the owner map and in each of four hierarchy
/// sets. A hierarchy set count is at most three per invocation plus the grid.
/// Tree comparisons and finite-domain coverage are charged by a conservative
/// quadratic term over the bounded event/element domain.
pub(crate) fn preflight_hierarchical_ownership_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    trace: Option<ProductionInvocationTraceResourceAdmissionV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let contracts = census
        .ownership_contracts
        .min(MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1);
    if contracts == 0 {
        return limits.require(
            ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
            ProductionAnalysisResourceUpperBoundV1::checked_phase(
                ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
                census.operations,
                0,
                0,
            )?,
        );
    }
    let (invocations, events, launch_rank) = trace
        .map(|admission| {
            (
                admission.invocation_count(),
                admission.event_upper_bound(),
                admission.launch_rank(),
            )
        })
        .unwrap_or((0, 0, 0));
    let rank = launch_rank.max(dialect_kernel::MAX_RANKED_MEMORY_RANK);
    let regions_per_contract = invocations
        .checked_mul(3)
        .and_then(|regions| regions.checked_add(1))
        .ok_or_else(ownership_resource_overflow_v1)?;
    let regions = checked_ownership_product_v1(contracts, regions_per_contract)?;
    let coordinate_items = checked_ownership_product_v1(events, rank)?;
    let hierarchy_coordinate_items = checked_ownership_product_v1(coordinate_items, 4)?;
    let finite_domain = MAX_HIERARCHICAL_OWNERSHIP_ELEMENTS_V1;
    let per_contract_work = checked_ownership_sum_v1(&[
        invocations,
        events,
        checked_ownership_product_v1(events, events)?,
        checked_ownership_product_v1(finite_domain, events.max(1))?,
        checked_ownership_product_v1(
            hierarchy_coordinate_items,
            rank.checked_add(1)
                .ok_or_else(ownership_resource_overflow_v1)?,
        )?,
    ])?;
    let work = checked_ownership_sum_v1(&[
        census.operations,
        checked_ownership_product_v1(contracts, census.operations)?,
        checked_ownership_product_v1(contracts, per_contract_work)?,
    ])?;

    // Reports retain only findings and compact region summaries. Full owner
    // maps and the four coordinate-set families are temporary per contract.
    let retained_regions = checked_ownership_sum_v1(&[
        checked_ownership_product_v1(
            regions,
            rank.checked_mul(3)
                .and_then(|n| n.checked_add(12))
                .ok_or_else(ownership_resource_overflow_v1)?,
        )?,
        checked_ownership_product_v1(regions_per_contract, census.identifier_bytes)?,
        checked_ownership_product_v1(regions, usize::BITS as usize)?,
    ])?;
    let retained_findings = checked_ownership_sum_v1(&[
        checked_ownership_product_v1(
            contracts.max(1),
            rank.checked_mul(4)
                .and_then(|items| items.checked_add(48))
                .ok_or_else(ownership_resource_overflow_v1)?,
        )?,
        census
            .identifier_bytes
            .checked_mul(4)
            .ok_or_else(ownership_resource_overflow_v1)?,
        checked_ownership_product_v1(contracts.max(1), usize::BITS as usize * 2)?,
        MAX_HIERARCHICAL_OWNERSHIP_DIAGNOSTIC_BYTES_V1,
    ])?;
    let temporary = checked_ownership_sum_v1(&[
        checked_ownership_product_v1(
            events,
            rank.checked_mul(2)
                .and_then(|n| n.checked_add(8))
                .ok_or_else(ownership_resource_overflow_v1)?,
        )?,
        hierarchy_coordinate_items,
        checked_ownership_product_v1(
            regions_per_contract,
            rank.checked_add(8)
                .ok_or_else(ownership_resource_overflow_v1)?,
        )?,
        checked_ownership_product_v1(MAX_HIERARCHICAL_OWNERSHIP_ELEMENTS_V1.min(events), rank)?,
    ])?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        work,
        checked_ownership_sum_v1(&[retained_regions, retained_findings])?,
        temporary,
    )?;
    let bound = bound.checked_then_retain(
        conditional_prefix_v1::resource_upper_bound_v1(census)?,
        ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
    )?;
    limits.require(
        ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        bound,
    )
}

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
    conditional_prefix:
        Option<Result<ConditionalPrefixCoverageV1, ConditionalPrefixDerivationErrorV1>>,
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

    /// Outstanding conditions; presence does not change status or proved counts.
    pub fn conditional_prefix_coverage(&self) -> Option<&ConditionalPrefixCoverageV1> {
        self.conditional_prefix.as_ref()?.as_ref().ok()
    }

    pub fn conditional_prefix_failure(&self) -> Option<&ConditionalPrefixDerivationErrorV1> {
        self.conditional_prefix.as_ref()?.as_ref().err()
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

include!("pliron_hierarchical_ownership/execution_v1.rs");
include!("pliron_hierarchical_ownership/contracts_v1.rs");
include!("pliron_hierarchical_ownership/resource_tests.rs");
