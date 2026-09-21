//! Barrier-convergence verification over bounded PLIRON invocation traces.

use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, ExecutionDomainAttr, HierarchyAttr, MemoryOrderAttr,
    MemoryScopeAttr,
};
use dialect_kernel::{PipelineEventKindAttr, PipelineEventOp, TensorLayoutOp};
use pliron::{builtin::ops::FuncOp, context::Context, operation::Operation};

use crate::production_analysis::pliron_analysis_manager::{
    PlironAnalysisManagerV1, PlironSimtProtocolAnalysisFailureV1,
};
use crate::production_analysis::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceFailureV1, PlironTraceLocationV1,
    ProductionInvocationTraceResourceAdmissionV1,
};
use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationObserverV1;
use crate::production_analysis::pliron_pipeline_protocol::run_pliron_pipeline_protocol_with_observation_v1;
#[cfg(test)]
use crate::production_analysis::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::production_analysis::pliron_simt_protocol::{
    PlironProtocolEventV1, PlironSimtProtocolIssueV1,
};
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1};

const MAX_PLIRON_BARRIER_DIAGNOSTIC_BYTES_V1: usize = 4_096;
const MAX_FALLBACK_BARRIER_CFG_BLOCKS_V1: usize = 512;
const MAX_FALLBACK_BARRIER_PATH_EVENTS_V1: usize = 256;
const FALLBACK_BARRIER_SOURCE_CLONE_ITEMS_V1: usize = 4 * MAX_FALLBACK_BARRIER_PATH_EVENTS_V1;
const FALLBACK_BARRIER_PREPEND_ITEMS_V1: usize = 4 * MAX_FALLBACK_BARRIER_PATH_EVENTS_V1;
const FALLBACK_BARRIER_COMPARISON_ITEMS_V1: usize = 8 * MAX_FALLBACK_BARRIER_PATH_EVENTS_V1;
const FALLBACK_BARRIER_DIVERGENCE_CLONE_ITEMS_V1: usize = 4 * MAX_FALLBACK_BARRIER_PATH_EVENTS_V1;
const FALLBACK_BARRIER_EDGE_WORK_V1: usize = FALLBACK_BARRIER_SOURCE_CLONE_ITEMS_V1
    + FALLBACK_BARRIER_PREPEND_ITEMS_V1
    + FALLBACK_BARRIER_COMPARISON_ITEMS_V1
    + FALLBACK_BARRIER_DIVERGENCE_CLONE_ITEMS_V1;
const FALLBACK_BARRIER_DIAGNOSTIC_RENDER_PASSES_V1: usize = 2;
const FALLBACK_BARRIER_SUMMARY_ITEMS_V1: usize = 4 * MAX_FALLBACK_BARRIER_PATH_EVENTS_V1;
// Both original and condensed graphs, SCC workspace, headers and queues.
// Geometric Vec capacity growth is included; mutually exclusive owners are
// conservatively summed. A location is two cells, not one.
const FALLBACK_BARRIER_GRAPH_BLOCK_ITEMS_V1: usize = 120;
const FALLBACK_BARRIER_GRAPH_EDGE_ITEMS_V1: usize = 11;
const FALLBACK_BARRIER_GRAPH_LOCAL_ITEMS_V1: usize = 6;
const FALLBACK_BARRIER_FIXED_ITEMS_V1: usize = 96;
const FALLBACK_BARRIER_TRANSIENT_SUMMARIES_V1: usize = 4;

type BarrierObserverV1<'o, 'p, 'r> = Option<&'o InvocationObserverV1<'p, 'r>>;

fn observe_barrier_quota_v1(observer: BarrierObserverV1<'_, '_, '_>, resource: &'static str) {
    if let Some(observer) = observer {
        observer.deny(ProductionAnalysisResourceLimitV1 {
            phase: ProductionAnalysisResourcePhaseV1::BarrierConvergence,
            resource,
        });
    }
}

fn observe_barrier_trace_failure_v1(
    observer: BarrierObserverV1<'_, '_, '_>,
    failure: &PlironTraceFailureV1,
) {
    let (phase, resource) = match failure {
        PlironTraceFailureV1::ResourceLimit => (
            ProductionAnalysisResourcePhaseV1::InvocationTrace,
            "barrier trace resource limit",
        ),
        PlironTraceFailureV1::LaunchTooLarge { .. } => (
            ProductionAnalysisResourcePhaseV1::InvocationTrace,
            "barrier trace launch limit",
        ),
        PlironTraceFailureV1::Sparse(crate::SparseIndexFailureV1::ResourceLimit {
            resource,
            ..
        }) => (ProductionAnalysisResourcePhaseV1::SparseIndex, *resource),
        _ => return,
    };
    if let Some(observer) = observer {
        observer.deny(ProductionAnalysisResourceLimitV1 { phase, resource });
    }
}

fn barrier_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::BarrierConvergence,
        resource: "barrier convergence resource upper bound",
    }
}

fn checked_barrier_sum_v1(values: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(barrier_resource_overflow_v1)
    })
}

fn checked_barrier_product_v1(
    lhs: usize,
    rhs: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(barrier_resource_overflow_v1)
}

/// Bounds exact trace comparison and the bounded all-path fallback. Cached
/// trace/SIMT state is charged by its owning phase; this charge covers only
/// barrier-local maps, path summaries, diagnostics, and scans.
pub(crate) fn preflight_barrier_convergence_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    trace: Option<ProductionInvocationTraceResourceAdmissionV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let (invocations, events, launch_rank) = trace
        .map(|admission| {
            (
                admission.invocation_count(),
                admission.event_upper_bound(),
                admission.launch_rank(),
            )
        })
        .unwrap_or((0, 0, 0));
    let fallback_blocks = census.blocks.min(MAX_FALLBACK_BARRIER_CFG_BLOCKS_V1);
    let fallback_path_items =
        checked_barrier_product_v1(fallback_blocks, FALLBACK_BARRIER_SUMMARY_ITEMS_V1)?;
    let exact_comparison_work = checked_barrier_product_v1(invocations.max(1), events)?;
    let local_events = census.operations.min(checked_barrier_product_v1(
        fallback_blocks,
        MAX_FALLBACK_BARRIER_PATH_EVENTS_V1,
    )?);
    let fallback_work = checked_barrier_sum_v1(&[
        256,
        checked_barrier_product_v1(census.operations, 32)?,
        checked_barrier_product_v1(fallback_blocks, 512)?,
        checked_barrier_product_v1(census.successors, 96)?,
        // Bound even worst-case collisions in the exact block-pointer map.
        checked_barrier_product_v1(
            checked_barrier_product_v1(fallback_blocks, 8)?,
            checked_barrier_sum_v1(&[fallback_blocks, census.successors])?,
        )?,
        checked_barrier_product_v1(census.successors, FALLBACK_BARRIER_EDGE_WORK_V1)?,
        fallback_path_items,
    ])?;
    let diagnostic_work = checked_barrier_product_v1(
        FALLBACK_BARRIER_DIAGNOSTIC_RENDER_PASSES_V1,
        MAX_PLIRON_BARRIER_DIAGNOSTIC_BYTES_V1,
    )?;
    let work = checked_barrier_sum_v1(&[
        census.operations,
        exact_comparison_work,
        fallback_work,
        diagnostic_work,
    ])?;
    let diagnostic_trace_items = events
        .max(MAX_FALLBACK_BARRIER_PATH_EVENTS_V1)
        .checked_mul(4)
        .ok_or_else(barrier_resource_overflow_v1)?;
    let retained = checked_barrier_sum_v1(&[
        diagnostic_trace_items,
        launch_rank
            .checked_mul(2)
            .ok_or_else(barrier_resource_overflow_v1)?,
        census.identifier_bytes,
        MAX_PLIRON_BARRIER_DIAGNOSTIC_BYTES_V1,
    ])?;
    let temporary = checked_barrier_sum_v1(&[
        checked_barrier_product_v1(
            invocations,
            launch_rank
                .checked_add(4)
                .ok_or_else(barrier_resource_overflow_v1)?,
        )?,
        events
            .checked_mul(2)
            .ok_or_else(barrier_resource_overflow_v1)?,
        checked_barrier_product_v1(fallback_blocks, FALLBACK_BARRIER_GRAPH_BLOCK_ITEMS_V1)?,
        checked_barrier_product_v1(census.successors, FALLBACK_BARRIER_GRAPH_EDGE_ITEMS_V1)?,
        checked_barrier_product_v1(local_events, FALLBACK_BARRIER_GRAPH_LOCAL_ITEMS_V1)?,
        fallback_path_items,
        FALLBACK_BARRIER_FIXED_ITEMS_V1,
        checked_barrier_product_v1(
            FALLBACK_BARRIER_TRANSIENT_SUMMARIES_V1,
            FALLBACK_BARRIER_SUMMARY_ITEMS_V1,
        )?,
        checked_barrier_product_v1(
            FALLBACK_BARRIER_DIAGNOSTIC_RENDER_PASSES_V1,
            MAX_PLIRON_BARRIER_DIAGNOSTIC_BYTES_V1,
        )?,
    ])?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::BarrierConvergence,
        work,
        retained,
        temporary,
    )?;
    limits.require(ProductionAnalysisResourcePhaseV1::BarrierConvergence, bound)
}

struct BoundedBarrierDiagnosticWriterV1 {
    detail: String,
    truncated: bool,
}

impl fmt::Write for BoundedBarrierDiagnosticWriterV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Err(fmt::Error);
        }
        let payload_limit = MAX_PLIRON_BARRIER_DIAGNOSTIC_BYTES_V1 - 3;
        let remaining = payload_limit.saturating_sub(self.detail.len());
        if value.len() <= remaining {
            self.detail.push_str(value);
            return Ok(());
        }
        let mut end = remaining;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.detail.push_str(&value[..end]);
        self.truncated = true;
        Err(fmt::Error)
    }
}

fn bounded_barrier_diagnostic_v1(value: impl fmt::Display) -> String {
    let mut writer = BoundedBarrierDiagnosticWriterV1 {
        detail: String::with_capacity(MAX_PLIRON_BARRIER_DIAGNOSTIC_BYTES_V1),
        truncated: false,
    };
    let _ = write!(&mut writer, "{value}");
    if writer.truncated {
        writer.detail.push_str("...");
    }
    writer.detail
}

struct TraceFailureDetailV1<'a>(&'a PlironTraceFailureV1);

impl fmt::Display for TraceFailureDetailV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_trace_failure_detail_v1(formatter, self.0)
    }
}

struct BarrierFallbackDiagnosticV1<'a> {
    trace_failure: &'a PlironTraceFailureV1,
    path_detail: &'a str,
    epoch_detail: &'a str,
}

impl fmt::Display for BarrierFallbackDiagnosticV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}; all-path convergence proof also failed: {}; uniform epoch proof also failed: {}",
            TraceFailureDetailV1(self.trace_failure),
            self.path_detail,
            self.epoch_detail,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironBarrierFindingV1 {
    BoundsPrerequisiteRejected,
    AnalysisIncomplete {
        detail: String,
    },
    DivergentBarrierTrace {
        first_invocation: Vec<u64>,
        first_trace: Vec<(usize, usize)>,
        second_invocation: Vec<u64>,
        second_trace: Vec<(usize, usize)>,
    },
    DivergentBarrierPaths {
        first_trace: Vec<(usize, usize)>,
        second_trace: Vec<(usize, usize)>,
    },
    SimtProtocolViolation {
        issue: Box<PlironSimtProtocolIssueV1>,
    },
}

impl PlironBarrierFindingV1 {
    pub fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::DivergentBarrierTrace { .. } | Self::DivergentBarrierPaths { .. } => {
                KernelCheckStatusV1::Rejected
            }
            Self::BoundsPrerequisiteRejected | Self::AnalysisIncomplete { .. } => {
                KernelCheckStatusV1::Incomplete
            }
            Self::SimtProtocolViolation { issue } => match issue.as_ref() {
                PlironSimtProtocolIssueV1::ResourceLimitExceeded => KernelCheckStatusV1::Incomplete,
                PlironSimtProtocolIssueV1::PhaseMismatch { .. }
                | PlironSimtProtocolIssueV1::PartialTensorParticipation { .. }
                | PlironSimtProtocolIssueV1::ClaimedActiveMaskMismatch { .. } => {
                    KernelCheckStatusV1::Rejected
                }
            },
        }
    }
}

impl fmt::Display for PlironBarrierFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundsPrerequisiteRejected => formatter.write_str(
                "error[FE2O3-BARRIER-000]: bounds prerequisite rejected before barrier-convergence analysis",
            ),
            Self::AnalysisIncomplete { detail } => write!(
                formatter,
                "error[FE2O3-BARRIER-002]: cannot prove barrier convergence: {detail}",
            ),
            Self::DivergentBarrierTrace {
                first_invocation,
                first_trace,
                second_invocation,
                second_trace,
            } => write!(
                formatter,
                "error[FE2O3-BARRIER-001]: divergent collective barrier trace; invocation {first_invocation:?} executes {}, while invocation {second_invocation:?} executes {}; failed proof: every participating invocation reaches the same barriers in the same order; help: move the barrier out of invocation-varying control flow",
                describe_trace(first_trace),
                describe_trace(second_trace),
            ),
            Self::DivergentBarrierPaths {
                first_trace,
                second_trace,
            } => write!(
                formatter,
                "error[FE2O3-BARRIER-001]: divergent collective barrier paths execute {} and {}; failed proof: every possible invocation path must reach the same barriers in the same order; help: move the barrier after the branch reconverges",
                describe_trace(first_trace),
                describe_trace(second_trace),
            ),
            Self::SimtProtocolViolation { issue } => format_protocol_issue(formatter, issue),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironBarrierReportV1 {
    findings: Vec<PlironBarrierFindingV1>,
}

super::pliron_report_payload_receipt::impl_empty_findings_payload_v1!(PlironBarrierReportV1);

impl PlironBarrierReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::BarrierConvergence
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[PlironBarrierFindingV1] {
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
pub struct PlironBarrierCheckErrorV1 {
    report: PlironBarrierReportV1,
}

impl PlironBarrierCheckErrorV1 {
    pub fn report(&self) -> &PlironBarrierReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironBarrierCheckErrorV1 {
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

impl std::error::Error for PlironBarrierCheckErrorV1 {}

#[cfg(test)]
pub(crate) fn run_pliron_barrier_convergence_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironBarrierReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    if !run_pliron_ranked_bounds_check_with_analyses_v1(context, function, &mut analyses).is_clean()
    {
        return report(PlironBarrierFindingV1::BoundsPrerequisiteRejected);
    }
    run_pliron_barrier_convergence_check_with_analyses_v1(context, function, &mut analyses)
}

#[cfg(test)]
fn run_barrier_with_progress_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    prove_progress: impl FnOnce() -> Result<
        crate::production_analysis::pliron_progress::PlironProgressReportV1,
        crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
    >,
) -> Result<
    PlironBarrierReportV1,
    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
> {
    run_barrier_with_progress_observation_v1(context, function, analyses, prove_progress, None)
}

fn run_barrier_with_progress_observation_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    prove_progress: impl FnOnce() -> Result<
        crate::production_analysis::pliron_progress::PlironProgressReportV1,
        crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
    >,
    observer: BarrierObserverV1<'_, '_, '_>,
) -> Result<
    PlironBarrierReportV1,
    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
> {
    let run = || {
        analyses.prepare_function_inventory(context, function);
        let inventory = match analyses.function_inventory_handle() {
            Ok(inventory) => inventory,
            Err(failure) => {
                observe_barrier_quota_v1(observer, failure.resource());
                return Ok(report(PlironBarrierFindingV1::AnalysisIncomplete {
                    detail: "the bounded function inventory limit was exceeded".to_owned(),
                }));
            }
        };
        let mut has_barrier = false;
        let mut has_tensor = false;
        for site in inventory.operations() {
            let operation = Operation::get_op_dyn(site.pointer(), context);
            has_barrier |= operation.downcast_ref::<BarrierOp>().is_some();
            has_tensor |= operation.downcast_ref::<TensorLayoutOp>().is_some();
        }
        if !has_barrier && !has_tensor {
            return Ok(PlironBarrierReportV1 { findings: vec![] });
        }
        analyses.prepare_simt_protocol(context, function);
        match analyses.simt_protocol() {
            Ok(protocol) => {
                if let Some(issue) = protocol.issues().first() {
                    if matches!(issue, PlironSimtProtocolIssueV1::ResourceLimitExceeded)
                        && let Some(observer) = observer
                    {
                        observer.deny(ProductionAnalysisResourceLimitV1 {
                            phase: ProductionAnalysisResourcePhaseV1::SimtProtocol,
                            resource: "SIMT protocol issue limit",
                        });
                    }
                    return Ok(report(PlironBarrierFindingV1::SimtProtocolViolation {
                        issue: Box::new(issue.clone()),
                    }));
                }
            }
            Err(PlironSimtProtocolAnalysisFailureV1::Trace(failure)) => {
                observe_barrier_trace_failure_v1(observer, &failure);
            }
        }
        // The existing all-path barrier proof can still decide some dynamic or
        // cyclic cases for which exact active-mask tracing is unavailable.
        if !has_barrier {
            // Tensor-layout analysis retains the static convergence proof when an
            // exact active-mask trace is unavailable. This stage only adds a
            // counterexample when the exact SIMT trace succeeds.
            return Ok(PlironBarrierReportV1 { findings: vec![] });
        }
        let trace_failure = match analyses.exact_trace() {
            Ok(traces) => {
                if traces.is_empty() {
                    return Ok(report(PlironBarrierFindingV1::AnalysisIncomplete {
                        detail: "the launch domain is empty".to_owned(),
                    }));
                }
                if let Some(finding) = divergent_scope_trace(traces, HierarchyAttr::Workgroup)
                    .or_else(|| divergent_scope_trace(traces, HierarchyAttr::Subgroup))
                {
                    return Ok(report(finding));
                }
                return Ok(PlironBarrierReportV1 { findings: vec![] });
            }
            Err(failure) => {
                observe_barrier_trace_failure_v1(observer, &failure);
                failure
            }
        };
        if matches!(
            trace_failure,
            PlironTraceFailureV1::MissingExecutionLayout
                | PlironTraceFailureV1::InvalidExecutionLayout
                | PlironTraceFailureV1::UnsupportedGridSynchronization { .. }
                | PlironTraceFailureV1::PartialBarrierParticipants { .. }
        ) {
            return Ok(report(PlironBarrierFindingV1::AnalysisIncomplete {
                detail: trace_failure_detail(trace_failure),
            }));
        }
        if matches!(trace_failure, PlironTraceFailureV1::DynamicLaunch { .. })
            && !matches!(
                analyses.execution_layout(),
                Ok(Some(layout))
                    if layout.execution_domain == ExecutionDomainAttr::FullPhysicalWorkgroups
            )
        {
            return Ok(report(PlironBarrierFindingV1::AnalysisIncomplete {
                detail:
                    "dynamic barrier convergence requires authenticated full physical workgroups"
                        .to_owned(),
            }));
        }
        Ok(
            match barrier_paths::summarize_all_barrier_paths_with_observation_v1(
                context,
                &inventory,
                prove_progress,
                observer,
            )? {
                BarrierPathSummaryV1::Unique => PlironBarrierReportV1 { findings: vec![] },
                BarrierPathSummaryV1::Divergent {
                    first_trace,
                    second_trace,
                } => report(PlironBarrierFindingV1::DivergentBarrierPaths {
                    first_trace,
                    second_trace,
                }),
                BarrierPathSummaryV1::Incomplete(path_detail) => {
                    let epoch_detail = match pipeline_barriers_have_uniform_epoch_proof(
                        context, function, &inventory, analyses, observer,
                    ) {
                        Ok(()) => return Ok(PlironBarrierReportV1 { findings: vec![] }),
                        Err(detail) => detail,
                    };
                    report(PlironBarrierFindingV1::AnalysisIncomplete {
                        detail: bounded_barrier_diagnostic_v1(BarrierFallbackDiagnosticV1 {
                            trace_failure: &trace_failure,
                            path_detail: &path_detail,
                            epoch_detail: &epoch_detail,
                        }),
                    })
                }
            },
        )
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}

fn pipeline_barriers_have_uniform_epoch_proof(
    context: &Context,
    function: &FuncOp,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
    analyses: &mut PlironAnalysisManagerV1,
    observer: BarrierObserverV1<'_, '_, '_>,
) -> Result<(), String> {
    let protocol =
        run_pliron_pipeline_protocol_with_observation_v1(context, function, analyses, observer);
    if !protocol.is_clean() {
        return Err(protocol
            .findings()
            .first()
            .map(bounded_barrier_diagnostic_v1)
            .unwrap_or_else(|| "pipeline protocol rejected without a finding".to_owned()));
    }
    if protocol.certificates().is_empty() {
        return Err("the function has no staged-pipeline certificate".to_owned());
    }
    let mut certificates = HashMap::new();
    for certificate in protocol.certificates() {
        let Some(site) = inventory.operations().iter().find(|site| {
            site.block() == certificate.pipeline_block()
                && site.operation() == certificate.pipeline_operation()
        }) else {
            return Err("a pipeline certificate has no exact creation site".to_owned());
        };
        if certificate.dynamic_loop().is_none() || !certificate.access_refinement_proven() {
            return Err(
                "a pipeline certificate lacks a uniform dynamic-loop/access refinement proof"
                    .to_owned(),
            );
        }
        if certificates.insert(site.pointer(), certificate).is_some() {
            return Err("two pipeline certificates name the same creation".to_owned());
        }
    }
    let mut barriers = 0_usize;
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(barrier) = operation.downcast_ref::<BarrierOp>() else {
            continue;
        };
        barriers = barriers.saturating_add(1);
        if barrier.execution_scope(context) != Some(HierarchyAttr::Workgroup)
            || barrier.memory_scope(context) != Some(MemoryScopeAttr::Workgroup)
            || barrier.address_space(context) != Some(AddressSpaceAttr::Workgroup)
            || barrier.order(context) != Some(MemoryOrderAttr::AcquireRelease)
        {
            return Err("a barrier paired with a pipeline wait changed its exact workgroup acquire-release contract".to_owned());
        }
        let block_operations = inventory.block_operations(site.block());
        if block_operations
            .iter()
            .filter(|candidate| {
                Operation::get_op_dyn(candidate.pointer(), context)
                    .downcast_ref::<BarrierOp>()
                    .is_some()
            })
            .count()
            != 1
        {
            return Err(format!(
                "pipeline-wait block {} contains more than one barrier",
                site.block()
            ));
        }
        let matched_waits = block_operations
            .iter()
            .filter(|candidate| {
                let operation = Operation::get_op_dyn(candidate.pointer(), context);
                let Some(event) = operation.downcast_ref::<PipelineEventOp>() else {
                    return false;
                };
                if event.kind(context) != Some(PipelineEventKindAttr::Wait) {
                    return false;
                }
                let Some(owner) = event.pipeline(context).defining_op() else {
                    return false;
                };
                let Some(certificate) = certificates.get(&owner) else {
                    return false;
                };
                let Some(summary) = certificate.dynamic_loop() else {
                    return false;
                };
                summary.body().contains(&site.block())
                    || summary.prologue_blocks().contains(&site.block())
                    || summary.drain_blocks().contains(&site.block())
            })
            .count();
        if matched_waits != 1 {
            return Err(format!(
                "barrier in block {} has {matched_waits} certified pipeline waits",
                site.block()
            ));
        }
    }
    if barriers == 0 {
        Err("the function has no barrier to justify".to_owned())
    } else {
        Ok(())
    }
}

fn describe_protocol_sequence(sequence: &[PlironProtocolEventV1]) -> String {
    if sequence.is_empty() {
        return "no collective events".to_owned();
    }
    sequence
        .iter()
        .map(|event| {
            format!(
                "{:?}@block {} op {}",
                event.kind(),
                event.location().block(),
                event.location().operation(),
            )
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn format_protocol_issue(
    formatter: &mut fmt::Formatter<'_>,
    issue: &PlironSimtProtocolIssueV1,
) -> fmt::Result {
    match issue {
        PlironSimtProtocolIssueV1::PhaseMismatch {
            grid,
            workgroup,
            subgroup,
            first_invocation,
            first,
            second_invocation,
            second,
        } => write!(
            formatter,
            "error[FE2O3-PROTOCOL-001]: collective phase mismatch in grid {grid} workgroup {workgroup} subgroup {subgroup}; invocation {first_invocation:?} executes {}, while invocation {second_invocation:?} executes {}; failed proof: all active lanes must execute the same tensor/barrier protocol in the same order; help: reconverge control flow before the collective and keep collective phases in one uniform sequence",
            describe_protocol_sequence(first),
            describe_protocol_sequence(second),
        ),
        PlironSimtProtocolIssueV1::PartialTensorParticipation {
            grid,
            workgroup,
            subgroup,
            location,
            expected_lanes,
            actual_lanes,
        } => write!(
            formatter,
            "error[FE2O3-PROTOCOL-002]: tensor collective at block {} op {} in grid {grid} workgroup {workgroup} subgroup {subgroup} requires {expected_lanes} active lanes, but the actual CFG paths reach it with lanes {actual_lanes:?}; failed proof: the physical subgroup participates as one active mask; help: move the tensor instruction after subgroup reconvergence and predicate its inputs instead of the collective",
            location.block(),
            location.operation(),
        ),
        PlironSimtProtocolIssueV1::ClaimedActiveMaskMismatch {
            location,
            claimed_active_lanes,
            actual_active_lanes,
        } => write!(
            formatter,
            "error[FE2O3-PROTOCOL-003]: tensor collective at block {} op {} claims {claimed_active_lanes} active lanes, but CFG-derived execution has {actual_active_lanes}; failed proof: retained participation metadata matches the executed active mask; help: derive the tensor site after reconvergence and regenerate its compiler-owned participation metadata",
            location.block(),
            location.operation(),
        ),
        PlironSimtProtocolIssueV1::ResourceLimitExceeded => formatter.write_str(
            "error[FE2O3-PROTOCOL-004]: SIMT protocol analysis exceeded its bounded issue limit",
        ),
    }
}

enum BarrierPathSummaryV1 {
    Unique,
    Divergent {
        first_trace: Vec<(usize, usize)>,
        second_trace: Vec<(usize, usize)>,
    },
    Incomplete(String),
}

enum BarrierPathFailureV1 {
    Preservation(crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1),
    Divergent {
        first_trace: Vec<(usize, usize)>,
        second_trace: Vec<(usize, usize)>,
    },
    Incomplete(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BarrierPathBlockSummaryV1 {
    normal: Option<Vec<(usize, usize)>>,
    trapped_prefix: Option<Vec<(usize, usize)>>,
}

#[cfg(test)]
fn prepend_barrier_path_v1(
    local: &[(usize, usize)],
    summary: BarrierPathBlockSummaryV1,
) -> Result<BarrierPathBlockSummaryV1, BarrierPathFailureV1> {
    prepend_barrier_path_with_observation_v1(local, summary, None)
}

fn prepend_barrier_path_with_observation_v1(
    local: &[(usize, usize)],
    summary: BarrierPathBlockSummaryV1,
    observer: BarrierObserverV1<'_, '_, '_>,
) -> Result<BarrierPathBlockSummaryV1, BarrierPathFailureV1> {
    let prepend = |mut suffix: Vec<(usize, usize)>| {
        let total = local.len().checked_add(suffix.len()).ok_or_else(|| {
            observe_barrier_quota_v1(observer, "fallback barrier path length overflow");
            BarrierPathFailureV1::Incomplete("fallback barrier path length overflowed".to_owned())
        })?;
        if total > MAX_FALLBACK_BARRIER_PATH_EVENTS_V1 {
            observe_barrier_quota_v1(observer, "fallback barrier path event limit");
            return Err(BarrierPathFailureV1::Incomplete(format!(
                "a fallback barrier path has more than {MAX_FALLBACK_BARRIER_PATH_EVENTS_V1} events"
            )));
        }
        let mut trace = Vec::with_capacity(total);
        trace.extend_from_slice(local);
        trace.append(&mut suffix);
        Ok(trace)
    };
    Ok(BarrierPathBlockSummaryV1 {
        normal: summary.normal.map(&prepend).transpose()?,
        trapped_prefix: summary.trapped_prefix.map(prepend).transpose()?,
    })
}

fn merge_barrier_path_summary_v1(
    complete: &mut BarrierPathBlockSummaryV1,
    candidate: BarrierPathBlockSummaryV1,
) -> Result<(), BarrierPathFailureV1> {
    if let Some(candidate_normal) = candidate.normal {
        if let Some(first_normal) = &complete.normal
            && first_normal != &candidate_normal
        {
            return Err(BarrierPathFailureV1::Divergent {
                first_trace: first_normal.clone(),
                second_trace: candidate_normal,
            });
        }
        complete.normal = Some(candidate_normal);
    }
    if let Some(candidate_trap) = candidate.trapped_prefix {
        match &complete.trapped_prefix {
            Some(first_trap) if first_trap.starts_with(&candidate_trap) => {}
            Some(first_trap) if candidate_trap.starts_with(first_trap) => {
                complete.trapped_prefix = Some(candidate_trap);
            }
            Some(first_trap) => {
                return Err(BarrierPathFailureV1::Divergent {
                    first_trace: first_trap.clone(),
                    second_trace: candidate_trap,
                });
            }
            None => complete.trapped_prefix = Some(candidate_trap),
        }
    }
    if let (Some(normal), Some(trapped_prefix)) = (&complete.normal, &complete.trapped_prefix)
        && !normal.starts_with(trapped_prefix)
    {
        return Err(BarrierPathFailureV1::Divergent {
            first_trace: normal.clone(),
            second_trace: trapped_prefix.clone(),
        });
    }
    Ok(())
}

include!("pliron_barrier/scoped_progress_v1.rs");
include!("pliron_barrier/progress_admission_v1.rs");

mod barrier_paths;

#[cfg(test)]
pub(crate) fn require_pliron_barrier_convergence_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironBarrierReportV1, PlironBarrierCheckErrorV1> {
    let report = run_pliron_barrier_convergence_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironBarrierCheckErrorV1 { report })
    }
}

fn barrier_trace(
    trace: &PlironInvocationTraceV1,
    scope: HierarchyAttr,
) -> Vec<(PlironTraceLocationV1, AddressSpaceAttr)> {
    trace
        .events
        .iter()
        .filter_map(|event| match event {
            PlironTraceEventV1::Barrier {
                location,
                execution_scope,
                address_space,
                ..
            } if *execution_scope == scope => Some((*location, *address_space)),
            PlironTraceEventV1::Barrier { .. }
            | PlironTraceEventV1::Fence { .. }
            | PlironTraceEventV1::TensorInstruction { .. }
            | PlironTraceEventV1::Trap { .. }
            | PlironTraceEventV1::Memory { .. }
            | PlironTraceEventV1::CollectiveAllocation { .. } => None,
        })
        .collect()
}

fn divergent_scope_trace(
    traces: &[PlironInvocationTraceV1],
    scope: HierarchyAttr,
) -> Option<PlironBarrierFindingV1> {
    let mut first_by_group: HashMap<(u64, u64, Option<u64>), &PlironInvocationTraceV1> =
        HashMap::new();
    for trace in traces {
        let group = (
            trace.grid,
            trace.workgroup,
            (scope == HierarchyAttr::Subgroup).then_some(trace.subgroup),
        );
        let Some(first) = first_by_group.get(&group).copied() else {
            first_by_group.insert(group, trace);
            continue;
        };
        let first_barriers = barrier_trace(first, scope);
        let barriers = barrier_trace(trace, scope);
        if barriers != first_barriers {
            return Some(PlironBarrierFindingV1::DivergentBarrierTrace {
                first_invocation: first.invocation.clone(),
                first_trace: first_barriers
                    .iter()
                    .map(|(location, _)| (location.block, location.operation))
                    .collect(),
                second_invocation: trace.invocation.clone(),
                second_trace: barriers
                    .iter()
                    .map(|(location, _)| (location.block, location.operation))
                    .collect(),
            });
        }
    }
    None
}

fn report(finding: PlironBarrierFindingV1) -> PlironBarrierReportV1 {
    PlironBarrierReportV1 {
        findings: vec![finding],
    }
}

fn describe_trace(trace: &[(usize, usize)]) -> String {
    if trace.is_empty() {
        return "no barrier".to_owned();
    }
    trace
        .iter()
        .map(|(block, operation)| format!("barrier(block {block}, op {operation})"))
        .collect::<Vec<_>>()
        .join(" -> ")
}

pub(crate) fn trace_failure_detail(failure: PlironTraceFailureV1) -> String {
    TraceFailureDetailV1(&failure).to_string()
}

fn write_trace_failure_detail_v1(
    formatter: &mut fmt::Formatter<'_>,
    failure: &PlironTraceFailureV1,
) -> fmt::Result {
    match failure {
        PlironTraceFailureV1::Sparse(failure) => {
            write!(formatter, "sparse index analysis failed: {failure:?}")
        }
        PlironTraceFailureV1::DynamicLaunch { dimension } => {
            write!(formatter, "launch dimension {dimension} is dynamic")
        }
        PlironTraceFailureV1::LaunchTooLarge { invocations } => {
            write!(formatter, "launch domain has {invocations} invocations")
        }
        PlironTraceFailureV1::UnresolvedBranch { block } => {
            write!(
                formatter,
                "branch in block {block} has an unresolved condition"
            )
        }
        PlironTraceFailureV1::ForeignView { block, operation } => {
            write!(
                formatter,
                "memory view at block {block} op {operation} is unresolved"
            )
        }
        PlironTraceFailureV1::UnsupportedTerminator { block } => {
            write!(formatter, "block {block} has an unsupported terminator")
        }
        PlironTraceFailureV1::CyclicControlFlow { block } => write!(
            formatter,
            "block {block} participates in cyclic control flow; progress-dependent spin synchronization is unsupported"
        ),
        PlironTraceFailureV1::MissingExecutionLayout => {
            formatter.write_str("scoped synchronization lacks a retained gpu.execution_layout")
        }
        PlironTraceFailureV1::InvalidExecutionLayout => formatter
            .write_str("gpu.execution_layout is malformed, duplicated, or outside the entry block"),
        PlironTraceFailureV1::UnsupportedGridSynchronization { block, operation } => write!(
            formatter,
            "ordinary grid-wide barriers are unsupported at block {block} op {operation}; use disjoint workgroup ownership or legal device-scope atomics"
        ),
        PlironTraceFailureV1::PartialBarrierParticipants {
            scope,
            dimension,
            global_extent,
            workgroup_extent,
        } => write!(
            formatter,
            "{scope:?} barrier has global extent {global_extent} on axis {dimension}, which is not a multiple of workgroup extent {workgroup_extent}; rounded physical lanes and their activity paths are not represented"
        ),
        PlironTraceFailureV1::ResourceLimit => formatter.write_str("trace resource limit exceeded"),
    }
}

include!("pliron_barrier/resource_tests.rs");

#[cfg(test)]
#[path = "pliron_barrier/progress_admission_v1_tests.rs"]
mod progress_admission_v1_tests;
