//! Whole-function bounds verification for target-neutral `kernel.*` Pliron IR.
//!
//! Local operation/type invariants remain in `dialect-kernel`. This module is
//! the fixed `MemoryBounds` analysis stage: it intersects facts from every CFG
//! predecessor and accepts an access only when each dimension is statically
//! in range or protected by an `index < extent` fact on every incoming path.
//! Checked mappings additionally require the complete arithmetic domain and
//! an independently matched ordinary expression on every reaching edge.

use std::{
    collections::{HashMap, VecDeque},
    fmt,
};

use dialect_gpu::{BarrierOp, ExecutionLayoutOp, FenceOp};
use dialect_kernel::{
    AccessKindAttr, AllocationEffectOp, AnalysisSplitOp, BranchArgsOp, BranchOp,
    CheckedRowStripedIndex2DOp, CheckedTiledIndex2DOp, DeterministicJoinOp, DimensionOp,
    IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp, IndexEqualBranchArgsOp,
    IndexEqualBranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp, IndexUnknownOp,
    IndexUnsignedCastOp, InvocationIndexOp, MAX_RANKED_MEMORY_RANK, OwnershipContractOp,
    PipelineCreateOp, PipelineEventOp, RankedAccessOp, RankedViewOp, RankedViewType,
    RequireEquivalentOp, RequireFiniteFoldOp, RequireFiniteRecurrenceOp,
    RequirePermutationGatherOp, ReturnOp, SemanticBinaryOp, SemanticConstantOp,
    SemanticExpressionCommitmentOp, SemanticSymbolOp, SemanticTypedBinaryOp, SemanticTypedCastOp,
    SemanticTypedCompareOp, SemanticTypedConstantOp, SemanticTypedExpressionRootOp,
    SemanticTypedReadOp, SemanticTypedSelectOp, SemanticTypedSymbolOp, SemanticTypedUnaryOp,
    TensorLayoutOp, TensorResultComponentOp, TrapOp, ranked_view_type,
};
use dialect_proof::{
    EvidenceRefOp, ObligationOp, RequireEffectRefinementOp, RequireNumericalRefinementOp,
    RequireRefinementOp, RequireTensorRefinementOp,
};
use pliron::{
    builtin::ops::FuncOp,
    common_traits::Named,
    context::{Context, Ptr},
    op::Op,
    operation::{Operation, verify_operation},
    r#type::{Typed, TypedHandle},
    value::Value,
};

use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_control_edges_v1::{ControlViewV1, EdgeViewV1};
use crate::production_analysis::pliron_presburger_adapter::PlironPresburgerAnalysisV1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::{
    KernelCheckPassKindV1, KernelCheckStatusV1, PresburgerRangeDecisionV1, SparseIndexAnalysisV1,
    SparseIndexFailureV1,
};

pub const MAX_RANKED_BOUNDS_BLOCKS: usize = 1_024;
pub const MAX_RANKED_BOUNDS_OPERATIONS: usize = 65_536;
pub const MAX_RANKED_BOUNDS_EDGES: usize = MAX_RANKED_BOUNDS_BLOCKS * 2;
pub const MAX_RANKED_BOUNDS_FACTS: usize = MAX_RANKED_BOUNDS_BLOCKS;
pub const MAX_RANKED_BOUNDS_OPERATION_ITEMS: usize =
    MAX_RANKED_BOUNDS_OPERATIONS * (MAX_RANKED_MEMORY_RANK + 4);
pub const MAX_RANKED_BOUNDS_FINDINGS: usize = 4_096;
pub const MAX_RANKED_BOUNDS_STORAGE_ITEMS: usize = 131_072;
pub const MAX_RANKED_BOUNDS_WORK_UNITS: usize = MAX_RANKED_BOUNDS_OPERATIONS * 128;

// Numeric Value IDs contain at most 21 bytes. An arbitrary usize dimension
// adds at most 28, fitting this explicitly reserved diagnostic buffer.
const NUMERIC_BOUNDS_DIAGNOSTIC_BYTES_V1: usize = 64;
// The longest produced sparse-error rendering is 119 bytes; pinned String
// growth can retain 236. Debug aliases never participate in either bound.
const SPARSE_BOUNDS_DIAGNOSTIC_BYTES_V1: usize = 256;

fn ranked_bounds_resource_error_v1(resource: &'static str) -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::MemoryBounds,
        resource,
    }
}

fn checked_ranked_bounds_sum_v1(
    values: &[usize],
    resource: &'static str,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| ranked_bounds_resource_error_v1(resource))
    })
}

fn checked_ranked_bounds_product_v1(
    lhs: usize,
    rhs: usize,
    resource: &'static str,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(|| ranked_bounds_resource_error_v1(resource))
}

/// Bounds CFG fact intersection, ranked-access checks, and the retained
/// memory-bounds report. Verified blocks each have a terminator, so the census
/// fact bound equals the block count, not the observed guard-fact count. FIFO
/// propagation, one-fact edge generation, and at most two successors bound both
/// unchanged predecessor rescans and changed-node successor visits by the
/// conservative graph-wave allowance below. Every operand is treated as a
/// possible access dimension and charged through the Presburger solver's
/// rejecting (`limit + 1`) step.
pub(crate) fn preflight_ranked_bounds_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::MemoryBounds;
    let operation_items = checked_ranked_bounds_sum_v1(
        &[
            census.operands,
            census.results,
            census.successors,
            census.attributes,
        ],
        "memory-bounds operation-item upper bound",
    )?;
    if census.blocks > MAX_RANKED_BOUNDS_BLOCKS
        || census.operations > MAX_RANKED_BOUNDS_OPERATIONS
        || census.successors > MAX_RANKED_BOUNDS_EDGES
        || operation_items > MAX_RANKED_BOUNDS_OPERATION_ITEMS
    {
        return Err(ranked_bounds_resource_error_v1(
            "memory-bounds structural hard limit",
        ));
    }

    let facts = census
        .blocks
        .min(census.operations)
        .min(MAX_RANKED_BOUNDS_FACTS);
    let fact_words = facts.div_ceil(u64::BITS as usize);
    let waves = facts
        .checked_add(1)
        .ok_or_else(|| ranked_bounds_resource_error_v1("memory-bounds wave upper bound"))?;
    let graph_scan = census
        .blocks
        .checked_add(census.successors)
        .ok_or_else(|| ranked_bounds_resource_error_v1("memory-bounds graph upper bound"))?;
    let intersection_work = checked_ranked_bounds_product_v1(
        checked_ranked_bounds_product_v1(
            graph_scan,
            fact_words.checked_add(1).ok_or_else(|| {
                ranked_bounds_resource_error_v1("memory-bounds fact-word upper bound")
            })?,
            "memory-bounds intersection work upper bound",
        )?,
        waves,
        "memory-bounds intersection work upper bound",
    )?;
    let charged_work = checked_ranked_bounds_sum_v1(
        &[
            census.operations,
            operation_items,
            checked_ranked_bounds_product_v1(
                graph_scan,
                5,
                "memory-bounds charged work upper bound",
            )?,
            intersection_work,
            census.operations,
            census.operands,
            // Companion validation scans every operation and compares each
            // typed read's operands once, without building another read roster.
            census.operations,
            census.operands,
        ],
        "memory-bounds charged work upper bound",
    )?;
    if charged_work > MAX_RANKED_BOUNDS_WORK_UNITS {
        return Err(ranked_bounds_resource_error_v1(
            "memory-bounds work hard limit",
        ));
    }
    // Args-edge relations have a different closure from the legacy global
    // guard-bitset waves. Their bounded execution includes the denied prefix.
    let (transport_work, transport_storage) = bounds_transport_resource_bound_v1(census)?;
    let charged_work = checked_ranked_bounds_sum_v1(
        &[charged_work, transport_work],
        "memory-bounds transported work upper bound",
    )?;

    let findings = census
        .blocks
        .checked_add(census.operands)
        .ok_or_else(|| ranked_bounds_resource_error_v1("memory-bounds finding upper bound"))?
        .min(MAX_RANKED_BOUNDS_FINDINGS);
    let internal_storage = checked_ranked_bounds_sum_v1(
        &[
            checked_ranked_bounds_product_v1(
                census.blocks,
                9,
                "memory-bounds storage upper bound",
            )?,
            facts,
            checked_ranked_bounds_product_v1(
                census.successors,
                3,
                "memory-bounds storage upper bound",
            )?,
            checked_ranked_bounds_product_v1(
                census.blocks,
                fact_words,
                "memory-bounds storage upper bound",
            )?,
            findings,
        ],
        "memory-bounds storage upper bound",
    )?;
    if internal_storage > MAX_RANKED_BOUNDS_STORAGE_ITEMS {
        return Err(ranked_bounds_resource_error_v1(
            "memory-bounds storage hard limit",
        ));
    }
    // The cumulative runtime storage admission precedes each scratch growth;
    // a denied request allocates nothing. Released query buffers are still
    // charged cumulatively, so this also covers old+new growth overlap.
    let internal_storage = checked_ranked_bounds_sum_v1(
        &[internal_storage, transport_storage],
        "memory-bounds transported storage upper bound",
    )?
    .min(MAX_RANKED_BOUNDS_STORAGE_ITEMS);

    let (domain_work, domain_storage) = checked_domain_resource_bound_v1(census)?;
    let charged_work = checked_ranked_bounds_sum_v1(
        &[charged_work, domain_work],
        "memory-bounds checked-domain work upper bound",
    )?;
    let internal_storage = checked_ranked_bounds_sum_v1(
        &[internal_storage, domain_storage],
        "memory-bounds checked-domain storage upper bound",
    )?
    .min(MAX_RANKED_BOUNDS_STORAGE_ITEMS);

    let presburger_queries = census.operands.min(MAX_RANKED_BOUNDS_OPERATION_ITEMS);
    let presburger_work_per_query = fe2o3_kernel_analysis::MAX_PRESBURGER_WORK_UNITS_V1
        .checked_add(1)
        .ok_or_else(|| {
            ranked_bounds_resource_error_v1("memory-bounds Presburger work upper bound")
        })?;
    let structural_work = census.checked_structural_items(phase)?;
    let work = checked_ranked_bounds_sum_v1(
        &[
            checked_ranked_bounds_product_v1(structural_work, 3, "memory-bounds work upper bound")?,
            charged_work,
            census.native_switch_verification_work,
            checked_ranked_bounds_product_v1(
                presburger_queries,
                presburger_work_per_query,
                "memory-bounds Presburger work upper bound",
            )?,
        ],
        "memory-bounds work upper bound",
    )?;
    let per_finding = MAX_RANKED_MEMORY_RANK
        .checked_mul(2)
        .and_then(|items| items.checked_add(48))
        .ok_or_else(|| ranked_bounds_resource_error_v1("memory-bounds report upper bound"))?;
    const MAX_DIAGNOSTIC_STRINGS_PER_FINDING_V1: usize = 3;
    let diagnostic_string_bytes = checked_ranked_bounds_product_v1(
        NUMERIC_BOUNDS_DIAGNOSTIC_BYTES_V1,
        MAX_DIAGNOSTIC_STRINGS_PER_FINDING_V1,
        "memory-bounds diagnostic string upper bound",
    )?;
    let finding_payload = per_finding
        .checked_add(diagnostic_string_bytes)
        .ok_or_else(|| ranked_bounds_resource_error_v1("memory-bounds report upper bound"))?;
    // Four bounded byte traversals cover numeric rendering, Identifier
    // validation, output copying and growth. Early op-id/sparse errors are
    // separate from the repeated numeric finding payload.
    let diagnostic_copy_work = checked_ranked_bounds_sum_v1(
        &[
            checked_ranked_bounds_product_v1(
                findings,
                diagnostic_string_bytes * 4,
                "memory-bounds diagnostic copy work upper bound",
            )?,
            checked_ranked_bounds_product_v1(
                census.identifier_bytes,
                4,
                "memory-bounds diagnostic copy work upper bound",
            )?,
            SPARSE_BOUNDS_DIAGNOSTIC_BYTES_V1 * 4,
        ],
        "memory-bounds diagnostic copy work upper bound",
    )?;
    let work = work.checked_add(diagnostic_copy_work).ok_or_else(|| {
        ranked_bounds_resource_error_v1("memory-bounds diagnostic copy work upper bound")
    })?;
    let numeric_retained = checked_ranked_bounds_sum_v1(
        &[checked_ranked_bounds_product_v1(
            findings,
            finding_payload,
            "memory-bounds report upper bound",
        )?],
        "memory-bounds report upper bound",
    )?;
    // Arbitrary op-id text can occur only in an early singleton error.
    // Its components are identity-censused, unlike excluded debug aliases.
    let singleton_text = checked_ranked_bounds_product_v1(
        census.identifier_bytes,
        2,
        "memory-bounds singleton diagnostic upper bound",
    )?;
    let singleton_retained = checked_ranked_bounds_sum_v1(
        &[
            per_finding,
            singleton_text,
            SPARSE_BOUNDS_DIAGNOSTIC_BYTES_V1,
        ],
        "memory-bounds singleton diagnostic upper bound",
    )?;
    let retained = numeric_retained.max(singleton_retained);
    let temporary = checked_ranked_bounds_sum_v1(
        &[
            internal_storage,
            census.native_switch_verification_scratch,
            structural_work,
            // Op-id components and a previous growing output buffer can
            // overlap the final singleton; numeric formatting needs two
            // additional bounded buffers at most.
            singleton_text,
            NUMERIC_BOUNDS_DIAGNOSTIC_BYTES_V1 * 2,
            checked_ranked_bounds_product_v1(
                presburger_queries.min(1),
                MAX_RANKED_MEMORY_RANK * 6,
                "memory-bounds temporary storage upper bound",
            )?,
        ],
        "memory-bounds temporary storage upper bound",
    )?;
    let bound =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, temporary)?;
    limits.require(phase, bound)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RankedBoundsFindingV1 {
    StructuralVerificationFailed,
    ResourceLimitExceeded {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    UnreachableBlock {
        block: usize,
    },
    UnsupportedTerminator {
        block: usize,
        operation: String,
    },
    UnsupportedOperation {
        block: usize,
        operation: usize,
        kind: String,
    },
    UnpairedSemanticRead {
        block: usize,
        operation: usize,
    },
    SparseIndexAnalysisFailed {
        detail: String,
    },
    StaticOutOfBounds {
        block: usize,
        operation: usize,
        access: AccessKindAttr,
        view: String,
        dimension: usize,
        index: u64,
        extent: u64,
    },
    PresburgerOutOfBounds {
        block: usize,
        operation: usize,
        access: AccessKindAttr,
        view: String,
        dimension: usize,
        invocation: Vec<i128>,
        index: i128,
        extent: u64,
    },
    MachineIntegerOverflow {
        block: usize,
        operation: usize,
        access: AccessKindAttr,
        view: String,
        dimension: usize,
        invocation: Vec<u64>,
        source_operation: dialect_kernel::IndexBinaryKindAttr,
        lhs: u64,
        rhs: u64,
        path_complete: bool,
    },
    UnprovedBound {
        block: usize,
        operation: usize,
        access: AccessKindAttr,
        view: String,
        dimension: usize,
        index: String,
        extent: String,
    },
}

impl RankedBoundsFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::StaticOutOfBounds { .. } | Self::PresburgerOutOfBounds { .. } => {
                KernelCheckStatusV1::Rejected
            }
            Self::MachineIntegerOverflow {
                path_complete: true,
                ..
            } => KernelCheckStatusV1::Rejected,
            Self::StructuralVerificationFailed
            | Self::ResourceLimitExceeded { .. }
            | Self::UnreachableBlock { .. }
            | Self::UnsupportedTerminator { .. }
            | Self::UnsupportedOperation { .. }
            | Self::UnpairedSemanticRead { .. }
            | Self::SparseIndexAnalysisFailed { .. }
            | Self::MachineIntegerOverflow {
                path_complete: false,
                ..
            }
            | Self::UnprovedBound { .. } => KernelCheckStatusV1::Incomplete,
        }
    }
}

impl fmt::Display for RankedBoundsFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructuralVerificationFailed => formatter.write_str(
                "error[FE2O3-BOUNDS-000]: Pliron structural verification failed before bounds analysis",
            ),
            Self::ResourceLimitExceeded {
                resource,
                limit,
                actual,
            } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: {resource} count {actual} exceeds analysis limit {limit}",
            ),
            Self::UnreachableBlock { block } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: block {block} is unreachable in the closed kernel CFG",
            ),
            Self::UnsupportedTerminator { block, operation } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: block {block} uses unsupported terminator {operation}",
            ),
            Self::UnsupportedOperation {
                block,
                operation,
                kind,
            } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: block {block} op {operation} uses unsupported operation {kind}",
            ),
            Self::SparseIndexAnalysisFailed { detail } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: sparse index analysis failed before bounds verification: {detail}",
            ),
            Self::UnpairedSemanticRead { block, operation } => write!(
                formatter,
                "error[FE2O3-BOUNDS-007]: block {block} op {operation} has no adjacent ordinary read with identical view and ordered indices; guarded, checked, and atomic read companions are unsupported",
            ),
            Self::StaticOutOfBounds {
                block,
                operation,
                access,
                view,
                dimension,
                index,
                extent,
            } => write!(
                formatter,
                "error[FE2O3-BOUNDS-001]: statically out-of-bounds {access:?} at block {block} op {operation}; access: {view} dimension {dimension}; required: {index} < {extent}",
            ),
            Self::PresburgerOutOfBounds {
                block,
                operation,
                access,
                view,
                dimension,
                invocation,
                index,
                extent,
            } => write!(
                formatter,
                "error[FE2O3-BOUNDS-004]: affine {access:?} is out of bounds at block {block} op {operation}; access: {view} dimension {dimension}; counterexample invocation {invocation:?} computes index {index}, violating {index} < {extent}; help: guard the access with the failed relation or reduce the launch domain",
            ),
            Self::MachineIntegerOverflow {
                block,
                operation,
                access,
                view,
                dimension,
                invocation,
                source_operation,
                lhs,
                rhs,
                path_complete,
            } => {
                let code = if *path_complete {
                    "FE2O3-BOUNDS-005"
                } else {
                    "FE2O3-BOUNDS-006"
                };
                write!(
                    formatter,
                    "error[{code}]: checked {access:?} index arithmetic may overflow at block {block} op {operation}; access: {view} dimension {dimension}; counterexample invocation {invocation:?} evaluates {lhs} {source_operation:?} {rhs} outside the unsigned 64-bit range; {}help: use checked arithmetic, narrow the launch domain, or prove a guard that keeps the expression and this block reachable only in range",
                    if *path_complete {
                        ""
                    } else {
                        "path reachability for this non-entry block is not represented in the current Presburger domain; "
                    },
                )
            }
            Self::UnprovedBound {
                block,
                operation,
                access,
                view,
                dimension,
                index,
                extent,
            } => write!(
                formatter,
                "error[FE2O3-BOUNDS-002]: cannot prove {access:?} is in bounds at block {block} op {operation}; access: {view} dimension {dimension}; unproven bound: {index} < {extent}; help: guard every path to the access or use an explicitly checked access",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedBoundsReportV1 {
    findings: Vec<RankedBoundsFindingV1>,
}

super::pliron_report_payload_receipt::impl_empty_findings_payload_v1!(RankedBoundsReportV1);

impl RankedBoundsReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::MemoryBounds
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[RankedBoundsFindingV1] {
        &self.findings
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

/// Terminal compile-time failure from the ranked-memory bounds stage.
///
/// This error is the lowering-facing API: callers must not lower the function
/// when it is returned. It retains every stable finding so the frontend can
/// render all unsafe dimensions in one diagnostic batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedBoundsCheckErrorV1 {
    report: RankedBoundsReportV1,
}

impl RankedBoundsCheckErrorV1 {
    pub fn report(&self) -> &RankedBoundsReportV1 {
        &self.report
    }
}

impl fmt::Display for RankedBoundsCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, finding) in self.report.findings().iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for RankedBoundsCheckErrorV1 {}

include!("pliron_ranked_bounds/facts_v1.rs");
include!("pliron_ranked_bounds/edge_transport_v1.rs");
include!("pliron_ranked_bounds/checked_domain_dag_v1.rs");
include!("pliron_ranked_bounds/checked_domain_state_v1.rs");
include!("pliron_ranked_bounds/checked_domain_solver_v1.rs");
include!("pliron_ranked_bounds/checked_domain_access_v1.rs");
include!("pliron_ranked_bounds/execution_v1.rs");
include!("pliron_ranked_bounds/access_proofs_v1.rs");
include!("pliron_ranked_bounds/semantic_reads_v1.rs");
include!("pliron_ranked_bounds/resource_tests.rs");
#[cfg(test)]
#[path = "pliron_ranked_bounds/edge_transport_v1_tests.rs"]
mod edge_transport_v1_tests;

#[cfg(test)]
#[path = "pliron_ranked_bounds/numeric_diagnostics_tests.rs"]
mod numeric_diagnostics_tests;

#[cfg(test)]
#[path = "pliron_ranked_bounds/checked_domain_v1_tests.rs"]
mod checked_domain_v1_tests;
