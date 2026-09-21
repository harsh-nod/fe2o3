//! Workgroup-memory initialization, publication, and race verification.

use std::fmt::Write as _;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
};

use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, ExecutionDomainAttr, HierarchyAttr, MemoryOrderAttr,
    MemoryScopeAttr,
};
use dialect_kernel::{
    AccessKindAttr, AllocationEffectOp, AnalysisSplitOp, BranchArgsOp, BranchOp,
    GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
    GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
    GFX950_TRANSPOSE_FP8_WORKGROUP_ALLOCATION_ORIGIN_V1,
    GFX950_TRANSPOSE_FP8_WORKGROUP_NOALIAS_CLASS_V1, IndexEqualBranchArgsOp, IndexEqualBranchOp,
    IndexLessThanBranchArgsOp, IndexLessThanBranchOp, MemorySpaceAttr, PipelineCreateOp,
    RankedAccessOp, RankedViewOp, ReturnOp, TrapOp, is_supported_allocation_effect_contract_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::ops::FuncOp,
    context::{Context, Ptr},
    operation::Operation,
    value::Value,
};

use crate::production_analysis::pliron_analysis_manager::{
    PlironAnalysisManagerV1, PlironMemoryOrderAnalysisFailureV1,
};
#[cfg(test)]
use crate::production_analysis::pliron_barrier::run_pliron_barrier_convergence_check_with_analyses_v1;
use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::production_analysis::pliron_invocation_trace::{
    PlironTraceLocationV1, pliron_execution_layout_with_inventory_v1,
};
use crate::production_analysis::pliron_memory_order::{
    PlironMemoryOrderFailureV1, PlironMemoryOrderIssueV1, ProductionMemoryOrderResourceAdmissionV1,
};
use crate::production_analysis::pliron_pipeline_protocol::run_pliron_pipeline_protocol_with_observation_v1;
use crate::production_analysis::pliron_provenance_alias::MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1;
#[cfg(test)]
use crate::production_analysis::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1, trace_failure_detail};

pub const MAX_PLIRON_WORKGROUP_FINDINGS_V1: usize = 4_096;
const MAX_PLIRON_WORKGROUP_DIAGNOSTIC_BYTES_V1: usize = 1_024;
const MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1: usize = 3;
const MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1: usize = 8;
const COLLECTIVE_TRANSPOSE_SUMMARY_TRACES_V1: usize = MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1 + 1;
// Each trace contributes one roster slot and up to three event slots.
const COLLECTIVE_TRANSPOSE_SUMMARY_ITEMS_V1: usize =
    COLLECTIVE_TRANSPOSE_SUMMARY_TRACES_V1 * (MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1 + 1);
const COLLECTIVE_TRANSPOSE_SUMMARY_CLONE_WORK_V1: usize = COLLECTIVE_TRANSPOSE_SUMMARY_ITEMS_V1;
// Prepending visits every trace, copies the local prefix, moves its suffix,
// performs four fixed length/allocation steps, and collects eight trap slots.
const COLLECTIVE_TRANSPOSE_SUMMARY_PREPEND_WORK_V1: usize = COLLECTIVE_TRANSPOSE_SUMMARY_TRACES_V1
    * (2 * MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1 + 4)
    + MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1;
// Eight candidate traps can each scan eight retained traps and compare all
// three events before discovering a late mismatch or equality.
const COLLECTIVE_TRANSPOSE_TRAP_DEDUP_WORK_V1: usize = MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1
    * MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1
    * MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1;
// Vec equality checks length once before comparing events: once for the
// normal trace and once for every candidate/retained trap pair.
const COLLECTIVE_TRANSPOSE_TRACE_LENGTH_COMPARISON_WORK_V1: usize =
    1 + MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1 * MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1;
// One normal-trace comparison plus visit/contains/cap-or-push bookkeeping for
// each candidate trap.
const COLLECTIVE_TRANSPOSE_SUMMARY_MERGE_WORK_V1: usize =
    MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1 + 3 * MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1;
const COLLECTIVE_TRANSPOSE_EDGE_WORK_V1: usize = COLLECTIVE_TRANSPOSE_SUMMARY_CLONE_WORK_V1
    + COLLECTIVE_TRANSPOSE_SUMMARY_PREPEND_WORK_V1
    + COLLECTIVE_TRANSPOSE_TRAP_DEDUP_WORK_V1
    + COLLECTIVE_TRANSPOSE_TRACE_LENGTH_COMPARISON_WORK_V1
    + COLLECTIVE_TRANSPOSE_SUMMARY_MERGE_WORK_V1;

fn workgroup_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
        resource: "workgroup memory resource upper bound",
    }
}

fn checked_workgroup_sum_v1(values: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(workgroup_resource_overflow_v1)
    })
}

fn checked_workgroup_product_v1(
    lhs: usize,
    rhs: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(workgroup_resource_overflow_v1)
}

/// Bounds workgroup-specific inventory, collective-path summaries, and the
/// diagnostic projection of the separately charged memory-order cache.
fn workgroup_memory_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    issue_count: usize,
    rank: usize,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let issue_count = if census.workgroup_ranked_accesses == 0 {
        0
    } else {
        issue_count.min(MAX_PLIRON_WORKGROUP_FINDINGS_V1)
    };
    let has_collective_transpose = census.collective_transpose_candidates != 0;
    // A malformed reserved collective can emit one bounded diagnostic before
    // the no-workgroup-access fast path. Memory-order issues otherwise give
    // the exact report cardinality supplied by the prepared cache.
    let report_count = issue_count.max(usize::from(has_collective_transpose));
    let collective_summary_items = if has_collective_transpose {
        checked_workgroup_product_v1(census.blocks, COLLECTIVE_TRANSPOSE_SUMMARY_ITEMS_V1)?
    } else {
        0
    };
    let collective_edge_work = if has_collective_transpose {
        checked_workgroup_product_v1(census.successors, COLLECTIVE_TRANSPOSE_EDGE_WORK_V1)?
    } else {
        0
    };
    let work = checked_workgroup_sum_v1(&[
        checked_workgroup_product_v1(census.operations, 4)?,
        collective_edge_work,
        collective_summary_items,
        checked_workgroup_product_v1(
            issue_count,
            rank.checked_mul(3)
                .and_then(|n| n.checked_add(12))
                .ok_or_else(workgroup_resource_overflow_v1)?,
        )?,
    ])?;
    let retained = checked_workgroup_sum_v1(&[
        checked_workgroup_product_v1(
            report_count,
            rank.checked_mul(3)
                .and_then(|items| items.checked_add(24))
                .ok_or_else(workgroup_resource_overflow_v1)?,
        )?,
        checked_workgroup_product_v1(report_count, MAX_PLIRON_WORKGROUP_DIAGNOSTIC_BYTES_V1)?,
        MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1,
    ])?;
    let temporary = checked_workgroup_sum_v1(&[
        checked_workgroup_product_v1(census.operations, 8)?,
        checked_workgroup_product_v1(census.blocks, usize::from(has_collective_transpose) * 6)?,
        checked_workgroup_product_v1(census.successors, usize::from(has_collective_transpose) * 2)?,
        collective_summary_items,
        // One source clone and its prepended candidate coexist with all block summaries.
        checked_workgroup_product_v1(
            COLLECTIVE_TRANSPOSE_SUMMARY_ITEMS_V1,
            usize::from(has_collective_transpose) * 2,
        )?,
    ])?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
        work,
        retained,
        temporary,
    )?;
    limits.require(ProductionAnalysisResourcePhaseV1::WorkgroupMemory, bound)
}

/// Uses the static memory-order admission when no prepared cache is available.
/// A missing admission still reserves the one diagnostic emitted for an
/// unavailable prerequisite rather than treating absent facts as a clean run.
#[cfg(test)]
pub(crate) fn preflight_workgroup_memory_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    memory_order: Option<ProductionMemoryOrderResourceAdmissionV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let (issue_count, rank) = memory_order
        .map(|admission| {
            (
                admission.issue_upper_bound().max(1),
                admission.vector_rank(),
            )
        })
        .unwrap_or((1, dialect_kernel::MAX_RANKED_MEMORY_RANK));
    workgroup_memory_resource_upper_bound_v1(census, issue_count, rank, limits)
}

/// Refines only the workgroup report's output cardinality from the exact cache
/// already admitted and materialized by memory-order analysis. The cache's own
/// retained receipt remains unchanged. An unavailable cache produces one
/// bounded `AnalysisIncomplete` finding.
pub(crate) fn preflight_prepared_workgroup_memory_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    admission: ProductionMemoryOrderResourceAdmissionV1,
    memory_order_issues: Result<&[PlironMemoryOrderIssueV1], PlironMemoryOrderAnalysisFailureV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let issue_count = match memory_order_issues {
        Ok(issues) => {
            let issue_count = issues.len();
            if issue_count > admission.issue_upper_bound() {
                return Err(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
                    resource: "authenticated memory-order issue count",
                });
            }
            issue_count
        }
        Err(_) => 1,
    };
    workgroup_memory_resource_upper_bound_v1(census, issue_count, admission.vector_rank(), limits)
}

const fn is_reserved_collective_transpose_identity_v1(origin: u64, class: u64) -> bool {
    matches!(
        (origin, class),
        (
            GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
            GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
        ) | (
            GFX950_TRANSPOSE_FP8_WORKGROUP_ALLOCATION_ORIGIN_V1,
            GFX950_TRANSPOSE_FP8_WORKGROUP_NOALIAS_CLASS_V1,
        )
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironWorkgroupMemoryFindingV1 {
    BoundsPrerequisiteRejected,
    BarrierPrerequisiteRejected,
    AnalysisIncomplete {
        detail: String,
    },
    ReadBeforeInitialization {
        invocation: Vec<u64>,
        block: usize,
        operation: usize,
        indices: Vec<u64>,
    },
    ConflictingEffects {
        indices: Vec<u64>,
        first_invocation: Vec<u64>,
        first_block: usize,
        first_operation: usize,
        first_access: AccessKindAttr,
        second_invocation: Vec<u64>,
        second_block: usize,
        second_operation: usize,
        second_access: AccessKindAttr,
    },
    FindingLimitExceeded,
}

impl PlironWorkgroupMemoryFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::ReadBeforeInitialization { .. } | Self::ConflictingEffects { .. } => {
                KernelCheckStatusV1::Rejected
            }
            Self::BoundsPrerequisiteRejected
            | Self::BarrierPrerequisiteRejected
            | Self::AnalysisIncomplete { .. }
            | Self::FindingLimitExceeded => KernelCheckStatusV1::Incomplete,
        }
    }
}

impl fmt::Display for PlironWorkgroupMemoryFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundsPrerequisiteRejected => formatter.write_str(
                "error[FE2O3-WORKGROUP-000]: bounds prerequisite rejected before workgroup-memory analysis",
            ),
            Self::BarrierPrerequisiteRejected => formatter.write_str(
                "error[FE2O3-WORKGROUP-000]: barrier-convergence prerequisite rejected before workgroup-memory analysis",
            ),
            Self::AnalysisIncomplete { detail } => write!(
                formatter,
                "error[FE2O3-WORKGROUP-003]: cannot prove workgroup-memory safety: {detail}",
            ),
            Self::ReadBeforeInitialization {
                invocation,
                block,
                operation,
                indices,
            } => write!(
                formatter,
                "error[FE2O3-WORKGROUP-001]: invocation {invocation:?} reads uninitialized workgroup address {indices:?} at block {block} op {operation}; failed proof: the address is not initialized by this invocation and no convergent workgroup-memory barrier published a prior write; help: initialize the address and publish it with a workgroup acquire-release barrier before the read",
            ),
            Self::ConflictingEffects {
                indices,
                first_invocation,
                first_block,
                first_operation,
                first_access,
                second_invocation,
                second_block,
                second_operation,
                second_access,
            } => write!(
                formatter,
                "error[FE2O3-WORKGROUP-002]: conflicting {first_access:?}/{second_access:?} workgroup-memory effects at address {indices:?}; invocation {first_invocation:?} block {first_block} op {first_operation} conflicts with invocation {second_invocation:?} block {second_block} op {second_operation}; help: use disjoint coordinates, a convergent workgroup barrier between epochs, or compatible atomic operations",
            ),
            Self::FindingLimitExceeded => formatter.write_str(
                "error[FE2O3-WORKGROUP-003]: workgroup-memory finding limit exceeded",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironWorkgroupMemoryReportV1 {
    findings: Vec<PlironWorkgroupMemoryFindingV1>,
}

super::pliron_report_payload_receipt::impl_empty_findings_payload_v1!(
    PlironWorkgroupMemoryReportV1
);

impl PlironWorkgroupMemoryReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::WorkgroupMemory
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[PlironWorkgroupMemoryFindingV1] {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironWorkgroupMemoryCheckErrorV1 {
    report: PlironWorkgroupMemoryReportV1,
}

impl PlironWorkgroupMemoryCheckErrorV1 {
    pub fn report(&self) -> &PlironWorkgroupMemoryReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironWorkgroupMemoryCheckErrorV1 {
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

impl std::error::Error for PlironWorkgroupMemoryCheckErrorV1 {}

#[cfg(test)]
pub(crate) fn run_pliron_workgroup_memory_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironWorkgroupMemoryReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    if !run_pliron_ranked_bounds_check_with_analyses_v1(context, function, &mut analyses).is_clean()
    {
        return one(PlironWorkgroupMemoryFindingV1::BoundsPrerequisiteRejected);
    }
    if !run_pliron_barrier_convergence_check_with_analyses_v1(context, function, &mut analyses)
        .is_clean()
    {
        return one(PlironWorkgroupMemoryFindingV1::BarrierPrerequisiteRejected);
    }
    run_pliron_workgroup_memory_check_with_analyses_v1(context, function, &mut analyses)
}

fn run_pliron_workgroup_memory_observed_inner_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    observer: WorkgroupObserverV1<'_, '_, '_>,
) -> PlironWorkgroupMemoryReportV1 {
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(failure) => {
            observe_workgroup_quota_v1(observer, failure.resource());
            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: "the bounded function inventory limit was exceeded".to_owned(),
            });
        }
    };
    let (workgroup_access_views, pipeline_views, collective_effect_sites) =
        inventory.operations().iter().fold(
            (
                HashSet::<Value>::new(),
                Vec::new(),
                HashSet::<PlironTraceLocationV1>::new(),
            ),
            |(mut accesses, mut pipelines, mut collective_effects), site| {
                let operation = Operation::get_op_dyn(site.pointer(), context);
                if let Some(access) = operation.downcast_ref::<RankedAccessOp>()
                    && access
                        .view(context)
                        .defining_op()
                        .is_some_and(|definition| {
                            Operation::get_op_dyn(definition, context)
                                .downcast_ref::<RankedViewOp>()
                                .is_some_and(|view| {
                                    view.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
                                })
                        })
                {
                    accesses.insert(access.view(context));
                }
                if let Some(create) = operation.downcast_ref::<PipelineCreateOp>() {
                    pipelines.push((site.block(), site.operation(), create.view(context)));
                }
                if operation
                    .downcast_ref::<AllocationEffectOp>()
                    .is_some_and(|effect| {
                        effect.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
                            && effect
                                .allocation_origin(context)
                                .zip(effect.noalias_class(context))
                                .is_some_and(|(origin, class)| {
                                    is_reserved_collective_transpose_identity_v1(origin, class)
                                })
                    })
                {
                    collective_effects.insert(PlironTraceLocationV1 {
                        block: site.block(),
                        operation: site.operation(),
                    });
                }
                (accesses, pipelines, collective_effects)
            },
        );
    if !collective_effect_sites.is_empty() {
        let layout = match pliron_execution_layout_with_inventory_v1(context, &inventory) {
            Ok(Some(layout)) => layout,
            Ok(None) => {
                return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                    detail: "the collective transpose lifecycle has no execution layout".to_owned(),
                });
            }
            Err(failure) => {
                return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                    detail: trace_failure_detail(failure),
                });
            }
        };
        let workgroup_extents = layout.workgroup_extents;
        let workgroup_size = workgroup_extents
            .into_iter()
            .try_fold(1_u64, u64::checked_mul);
        let wave_private_layout = workgroup_extents[1] == 1
            && workgroup_extents[2] == 1
            && (64..=256).contains(&workgroup_extents[0])
            && workgroup_extents[0].is_multiple_of(64);
        if !wave_private_layout
            || workgroup_size.is_none()
            || layout.subgroup_size != 64
            || layout.execution_domain != ExecutionDomainAttr::FullPhysicalWorkgroups
        {
            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: "the gfx950 transpose tile requires one to four full physical Wave64s in a one-dimensional workgroup; lowering assigns each wave a disjoint LDS tile".to_owned(),
            });
        }
        if let Err(detail) = validate_collective_transpose_lifecycle_v1(
            context,
            &inventory,
            &collective_effect_sites,
            observer,
        ) {
            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete { detail });
        }
    }
    if workgroup_access_views.is_empty() {
        return PlironWorkgroupMemoryReportV1 { findings: vec![] };
    }
    if !pipeline_views.is_empty() {
        let pipeline =
            run_pliron_pipeline_protocol_with_observation_v1(context, function, analyses, observer);
        if pipeline.is_clean() {
            let certified = pipeline
                .certificates()
                .iter()
                .filter(|certificate| certificate.access_refinement_proven())
                .filter_map(|certificate| {
                    pipeline_views
                        .iter()
                        .find(|(block, operation, _)| {
                            *block == certificate.pipeline_block()
                                && *operation == certificate.pipeline_operation()
                        })
                        .map(|(_, _, view)| *view)
                })
                .collect::<HashSet<_>>();
            if workgroup_access_views.is_subset(&certified) {
                return PlironWorkgroupMemoryReportV1 { findings: vec![] };
            }
        }
    }
    analyses.prepare_memory_order(context, function);
    let memory_order = match analyses.memory_order() {
        Ok(analysis) => analysis,
        Err(failure) => {
            observe_workgroup_dependency_failure_v1(observer, &failure);
            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: memory_order_failure_detail(failure),
            });
        }
    };
    let mut findings = Vec::new();
    for issue in memory_order.issues() {
        let finding = match issue {
            PlironMemoryOrderIssueV1::ReadBeforeInitialization {
                invocation,
                location,
                address,
            } => PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization {
                invocation: invocation.clone(),
                block: location.block(),
                operation: location.operation(),
                indices: address.indices().to_vec(),
            },
            PlironMemoryOrderIssueV1::ConflictingEffects {
                address,
                first_invocation,
                first_location,
                first_access,
                second_invocation,
                second_location,
                second_access,
            } => PlironWorkgroupMemoryFindingV1::ConflictingEffects {
                indices: address.indices().to_vec(),
                first_invocation: first_invocation.clone(),
                first_block: first_location.block(),
                first_operation: first_location.operation(),
                first_access: *first_access,
                second_invocation: second_invocation.clone(),
                second_block: second_location.block(),
                second_operation: second_location.operation(),
                second_access: *second_access,
            },
            PlironMemoryOrderIssueV1::AtomicReadFromUnresolved {
                invocation,
                location,
                address,
                detail,
            } => PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: bounded_workgroup_detail_v1(format_args!(
                    "invocation {invocation:?} block {} op {} cannot derive read-from for workgroup address {:?}: {detail}",
                    location.block(),
                    location.operation(),
                    address.indices(),
                )),
            },
        };
        if findings.len() == MAX_PLIRON_WORKGROUP_FINDINGS_V1 {
            return one(PlironWorkgroupMemoryFindingV1::FindingLimitExceeded);
        }
        findings.push(finding);
    }
    PlironWorkgroupMemoryReportV1 { findings }
}

struct BoundedWorkgroupDetailWriterV1 {
    value: String,
}

impl fmt::Write for BoundedWorkgroupDetailWriterV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let remaining = MAX_PLIRON_WORKGROUP_DIAGNOSTIC_BYTES_V1.saturating_sub(self.value.len());
        if remaining == 0 {
            return Ok(());
        }
        let mut end = value.len().min(remaining);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.value.push_str(&value[..end]);
        Ok(())
    }
}

fn bounded_workgroup_detail_v1(arguments: fmt::Arguments<'_>) -> String {
    let mut writer = BoundedWorkgroupDetailWriterV1 {
        value: String::with_capacity(MAX_PLIRON_WORKGROUP_DIAGNOSTIC_BYTES_V1),
    };
    let _ = writer.write_fmt(arguments);
    writer.value
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectiveTransposePathEventV1 {
    Allocation {
        location: PlironTraceLocationV1,
        access: AccessKindAttr,
        allocation_origin: u64,
        noalias_class: u64,
    },
    Barrier {
        location: PlironTraceLocationV1,
        execution_scope: HierarchyAttr,
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CollectiveTransposePathSummaryV1 {
    normal: Option<Vec<CollectiveTransposePathEventV1>>,
    trapped: Vec<Vec<CollectiveTransposePathEventV1>>,
}

fn prepend_collective_path_with_observation_v1(
    local: &[CollectiveTransposePathEventV1],
    summary: CollectiveTransposePathSummaryV1,
    observer: WorkgroupObserverV1<'_, '_, '_>,
) -> Result<CollectiveTransposePathSummaryV1, String> {
    let prepend = |mut suffix: Vec<CollectiveTransposePathEventV1>| {
        let total = local.len().checked_add(suffix.len()).ok_or_else(|| {
            observe_workgroup_quota_v1(observer, "collective transpose path length overflow");
            "collective transpose path length overflowed".to_owned()
        })?;
        if total > MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1 {
            observe_workgroup_quota_v1(observer, "collective transpose path event limit");
            return Err(format!(
                "collective transpose path has more than {MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1} events"
            ));
        }
        let mut trace = Vec::with_capacity(total);
        trace.extend_from_slice(local);
        trace.append(&mut suffix);
        Ok(trace)
    };
    let normal = summary.normal.map(&prepend).transpose()?;
    let trapped = summary
        .trapped
        .into_iter()
        .map(prepend)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CollectiveTransposePathSummaryV1 { normal, trapped })
}

fn merge_collective_path_with_observation_v1(
    complete: &mut CollectiveTransposePathSummaryV1,
    candidate: CollectiveTransposePathSummaryV1,
    observer: WorkgroupObserverV1<'_, '_, '_>,
) -> Result<(), String> {
    if let Some(candidate_normal) = candidate.normal {
        if let Some(first_normal) = &complete.normal
            && first_normal != &candidate_normal
        {
            return Err("normal paths have different collective transpose traces".to_owned());
        }
        complete.normal = Some(candidate_normal);
    }
    for candidate_trap in candidate.trapped {
        if !complete.trapped.contains(&candidate_trap) {
            if complete.trapped.len() == MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1 {
                observe_workgroup_quota_v1(
                    observer,
                    "collective transpose terminal trap trace limit",
                );
                return Err(format!(
                    "collective transpose CFG has more than {MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1} distinct terminal trap traces"
                ));
            }
            complete.trapped.push(candidate_trap);
        }
    }
    Ok(())
}

fn validate_collective_trap_paths_v1(
    normal: &[CollectiveTransposePathEventV1],
    trapped: &[Vec<CollectiveTransposePathEventV1>],
) -> Result<(), String> {
    if trapped
        .iter()
        .any(|trace| !trace.is_empty() && trace.as_slice() != normal)
    {
        return Err(
            "a terminal trap path partially executes the collective transpose lifecycle".to_owned(),
        );
    }
    Ok(())
}

fn collective_block_events_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    block: usize,
    terminator: Ptr<Operation>,
    observer: WorkgroupObserverV1<'_, '_, '_>,
) -> Result<Vec<CollectiveTransposePathEventV1>, String> {
    let mut events = Vec::with_capacity(MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1);
    for site in inventory.block_operations(block) {
        if site.pointer() == terminator {
            continue;
        }
        let operation = Operation::get_op_dyn(site.pointer(), context);
        if let Some(effect) = operation.downcast_ref::<AllocationEffectOp>()
            && effect.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
        {
            let (Some(access), Some(allocation_origin), Some(noalias_class)) = (
                effect.kind(context),
                effect.allocation_origin(context),
                effect.noalias_class(context),
            ) else {
                return Err(format!(
                    "block {block} op {} has a malformed collective transpose effect",
                    site.operation()
                ));
            };
            if !is_supported_allocation_effect_contract_v1(
                access,
                MemorySpaceAttr::Workgroup,
                allocation_origin,
                noalias_class,
            ) {
                return Err(format!(
                    "block {block} op {} uses a non-reserved collective transpose identity",
                    site.operation()
                ));
            }
            if !is_reserved_collective_transpose_identity_v1(allocation_origin, noalias_class) {
                continue;
            }
            if events.len() == MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1 {
                observe_workgroup_quota_v1(observer, "collective transpose block event limit");
                return Err(format!(
                    "block {block} has more than {MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1} collective transpose events"
                ));
            }
            events.push(CollectiveTransposePathEventV1::Allocation {
                location: PlironTraceLocationV1 {
                    block,
                    operation: site.operation(),
                },
                access,
                allocation_origin,
                noalias_class,
            });
        } else if let Some(barrier) = operation.downcast_ref::<BarrierOp>() {
            let (Some(execution_scope), Some(memory_scope), Some(address_space), Some(order)) = (
                barrier.execution_scope(context),
                barrier.memory_scope(context),
                barrier.address_space(context),
                barrier.order(context),
            ) else {
                return Err(format!(
                    "block {block} op {} has a malformed collective transpose barrier",
                    site.operation()
                ));
            };
            if events.len() == MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1 {
                observe_workgroup_quota_v1(observer, "collective transpose block event limit");
                return Err(format!(
                    "block {block} has more than {MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1} collective transpose events"
                ));
            }
            events.push(CollectiveTransposePathEventV1::Barrier {
                location: PlironTraceLocationV1 {
                    block,
                    operation: site.operation(),
                },
                execution_scope,
                memory_scope,
                address_space,
                order,
            });
        }
    }
    Ok(events)
}

fn validate_collective_transpose_lifecycle_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    expected_sites: &HashSet<PlironTraceLocationV1>,
    observer: WorkgroupObserverV1<'_, '_, '_>,
) -> Result<(), String> {
    let blocks = inventory.blocks();
    let block_indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<Ptr<BasicBlock>, usize>>();
    let mut successors = vec![Vec::new(); blocks.len()];
    let mut predecessors = vec![Vec::new(); blocks.len()];
    let mut local_events = Vec::with_capacity(blocks.len());
    let mut terminal_kinds = vec![0_u8; blocks.len()];

    for (block_index, block) in blocks.iter().copied().enumerate() {
        let terminator = block
            .deref(context)
            .get_terminator(context)
            .ok_or_else(|| format!("block {block_index} has no terminator"))?;
        local_events.push(collective_block_events_v1(
            context,
            inventory,
            block_index,
            terminator,
            observer,
        )?);
        let terminator_op = Operation::get_op_dyn(terminator, context);
        let raw = terminator_op.get_operation().deref(context);
        if terminator_op.downcast_ref::<ReturnOp>().is_some() {
            terminal_kinds[block_index] = 1;
        } else if terminator_op.downcast_ref::<TrapOp>().is_some() {
            terminal_kinds[block_index] = 2;
        } else if terminator_op.downcast_ref::<BranchOp>().is_some()
            || terminator_op.downcast_ref::<BranchArgsOp>().is_some()
            || terminator_op
                .downcast_ref::<IndexLessThanBranchOp>()
                .is_some()
            || terminator_op
                .downcast_ref::<IndexLessThanBranchArgsOp>()
                .is_some()
            || terminator_op.downcast_ref::<IndexEqualBranchOp>().is_some()
            || terminator_op
                .downcast_ref::<IndexEqualBranchArgsOp>()
                .is_some()
            || terminator_op.downcast_ref::<AnalysisSplitOp>().is_some()
            || terminator_op
                .downcast_ref::<dialect_gpu::optimization_v1::CondBranchOp>()
                .is_some()
        {
            for successor in raw.successors() {
                let target = block_indices.get(&successor).copied().ok_or_else(|| {
                    format!("block {block_index} targets a block outside the kernel")
                })?;
                successors[block_index].push(target);
                predecessors[target].push(block_index);
            }
            if successors[block_index].is_empty() {
                return Err(format!("block {block_index} has no CFG successor"));
            }
        } else {
            return Err(format!(
                "block {block_index} has an unsupported collective transpose terminator"
            ));
        }
    }

    let mut remaining = successors.iter().map(Vec::len).collect::<Vec<_>>();
    let mut summaries = vec![None; blocks.len()];
    let mut ready = VecDeque::new();
    for block in 0..blocks.len() {
        match terminal_kinds[block] {
            1 => {
                summaries[block] = Some(CollectiveTransposePathSummaryV1 {
                    normal: Some(local_events[block].clone()),
                    trapped: Vec::new(),
                });
                ready.push_back(block);
            }
            2 => {
                summaries[block] = Some(CollectiveTransposePathSummaryV1 {
                    normal: None,
                    trapped: vec![local_events[block].clone()],
                });
                ready.push_back(block);
            }
            _ => {}
        }
    }
    while let Some(completed) = ready.pop_front() {
        for predecessor in predecessors[completed].iter().copied() {
            remaining[predecessor] = remaining[predecessor]
                .checked_sub(1)
                .ok_or_else(|| "collective transpose CFG accounting underflowed".to_owned())?;
            if remaining[predecessor] != 0 {
                continue;
            }
            let mut summary = CollectiveTransposePathSummaryV1::default();
            for successor in successors[predecessor].iter().copied() {
                let suffix = summaries[successor].clone().ok_or_else(|| {
                    format!("block {predecessor} has an unresolved cyclic CFG successor")
                })?;
                let candidate = prepend_collective_path_with_observation_v1(
                    &local_events[predecessor],
                    suffix,
                    observer,
                )?;
                merge_collective_path_with_observation_v1(&mut summary, candidate, observer)?;
            }
            summaries[predecessor] = Some(summary);
            ready.push_back(predecessor);
        }
    }

    let summary = summaries.first().and_then(Option::as_ref).ok_or_else(|| {
        "the collective transpose entry participates in cyclic control flow".to_owned()
    })?;
    let normal = summary
        .normal
        .as_deref()
        .ok_or_else(|| "the collective transpose CFG has no normal return path".to_owned())?;
    validate_collective_trap_paths_v1(normal, &summary.trapped)?;
    let [
        CollectiveTransposePathEventV1::Allocation {
            location: write_location,
            access: AccessKindAttr::Write,
            allocation_origin: write_origin,
            noalias_class: write_class,
        },
        CollectiveTransposePathEventV1::Barrier {
            execution_scope: HierarchyAttr::Workgroup,
            memory_scope: MemoryScopeAttr::Workgroup,
            address_space: AddressSpaceAttr::Workgroup,
            order: MemoryOrderAttr::AcquireRelease,
            ..
        },
        CollectiveTransposePathEventV1::Allocation {
            location: read_location,
            access: AccessKindAttr::Read,
            allocation_origin: read_origin,
            noalias_class: read_class,
        },
    ] = normal
    else {
        return Err(
            "every normal path must execute exactly stage, workgroup acquire-release publication, then read"
                .to_owned(),
        );
    };
    if (write_origin, write_class) != (read_origin, read_class) {
        return Err("the collective transpose stage and read formats differ".to_owned());
    }
    let observed_sites = HashSet::from([*write_location, *read_location]);
    if observed_sites != *expected_sites || expected_sites.len() != 2 {
        return Err(
            "the collective transpose path does not execute every reserved effect exactly once"
                .to_owned(),
        );
    }
    Ok(())
}
fn memory_order_failure_detail(failure: PlironMemoryOrderAnalysisFailureV1) -> String {
    match failure {
        PlironMemoryOrderAnalysisFailureV1::Trace(failure) => trace_failure_detail(failure),
        PlironMemoryOrderAnalysisFailureV1::Provenance(detail) => detail,
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::UnresolvedAddress { location },
        ) => format!(
            "workgroup address at block {} op {} is unresolved",
            location.block(),
            location.operation(),
        ),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::MismatchedBarrierPhase {
                grid,
                workgroup,
                epoch,
            },
        ) => format!(
            "grid {grid} workgroup {workgroup} has mismatched workgroup-barrier participation at memory epoch {epoch}"
        ),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::SubgroupPublicationUnsupported { .. },
        ) => "subgroup-local LDS publication requires a retained per-subgroup epoch/read-from relation; a subgroup barrier never publishes to sibling waves".to_owned(),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::FencePublicationUnsupported { .. },
        ) => "fence-mediated LDS publication requires a retained read-from/synchronizes-with relation; a non-collective fence is not a workgroup barrier".to_owned(),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::VersionLimitExceeded,
        ) => "workgroup memory-version limit exceeded".to_owned(),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::PublicationEdgeLimitExceeded,
        ) => "workgroup memory publication-edge limit exceeded".to_owned(),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::IssueLimitExceeded,
        ) => "workgroup memory-order issue limit exceeded".to_owned(),
    }
}

#[cfg(test)]
pub(crate) fn require_pliron_workgroup_memory_safety_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironWorkgroupMemoryReportV1, PlironWorkgroupMemoryCheckErrorV1> {
    let report = run_pliron_workgroup_memory_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironWorkgroupMemoryCheckErrorV1 { report })
    }
}

fn one(finding: PlironWorkgroupMemoryFindingV1) -> PlironWorkgroupMemoryReportV1 {
    PlironWorkgroupMemoryReportV1 {
        findings: vec![finding],
    }
}

include!("pliron_workgroup_memory/resource_tests.rs");
include!("pliron_workgroup_memory/observation_v1.rs");
include!("pliron_workgroup_memory/observation_v1_tests.rs");
