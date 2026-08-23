//! Barrier-convergence verification over bounded PLIRON invocation traces.

use std::{collections::HashMap, fmt};

use dialect_gpu::{AddressSpaceAttr, BarrierOp, HierarchyAttr};
use dialect_kernel::{AnalysisSplitOp, BranchOp, IndexLessThanBranchOp, ReturnOp};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    operation::Operation,
};

use crate::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceFailureV1, PlironTraceLocationV1,
    trace_pliron_invocations_v1,
};
use crate::{KernelCheckPassKindV1, run_pliron_ranked_bounds_check_v1};

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
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironBarrierReportV1 {
    findings: Vec<PlironBarrierFindingV1>,
}

impl PlironBarrierReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::BarrierConvergence
    }

    pub fn findings(&self) -> &[PlironBarrierFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
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

pub fn run_pliron_barrier_convergence_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironBarrierReportV1 {
    if !run_pliron_ranked_bounds_check_v1(context, function).is_clean() {
        return report(PlironBarrierFindingV1::BoundsPrerequisiteRejected);
    }
    run_pliron_barrier_convergence_check_after_bounds_v1(context, function)
}

pub(crate) fn run_pliron_barrier_convergence_check_after_bounds_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironBarrierReportV1 {
    if !function
        .get_region(context)
        .deref(context)
        .iter(context)
        .any(|block| {
            block.deref(context).iter(context).any(|operation| {
                Operation::get_op_dyn(operation, context)
                    .downcast_ref::<BarrierOp>()
                    .is_some()
            })
        })
    {
        return PlironBarrierReportV1 { findings: vec![] };
    }
    let trace_failure = match trace_pliron_invocations_v1(context, function) {
        Ok(traces) => {
            if traces.is_empty() {
                return report(PlironBarrierFindingV1::AnalysisIncomplete {
                    detail: "the launch domain is empty".to_owned(),
                });
            }
            if let Some(finding) = divergent_scope_trace(&traces, HierarchyAttr::Workgroup)
                .or_else(|| divergent_scope_trace(&traces, HierarchyAttr::Subgroup))
            {
                return report(finding);
            }
            return PlironBarrierReportV1 { findings: vec![] };
        }
        Err(failure) => failure,
    };
    if matches!(
        trace_failure,
        PlironTraceFailureV1::MissingExecutionLayout
            | PlironTraceFailureV1::InvalidExecutionLayout
            | PlironTraceFailureV1::UnsupportedGridSynchronization { .. }
            | PlironTraceFailureV1::PartialBarrierParticipants { .. }
    ) {
        return report(PlironBarrierFindingV1::AnalysisIncomplete {
            detail: trace_failure_detail(trace_failure),
        });
    }
    match summarize_all_barrier_paths(context, function) {
        BarrierPathSummaryV1::Unique => PlironBarrierReportV1 { findings: vec![] },
        BarrierPathSummaryV1::Divergent {
            first_trace,
            second_trace,
        } => report(PlironBarrierFindingV1::DivergentBarrierPaths {
            first_trace,
            second_trace,
        }),
        BarrierPathSummaryV1::Incomplete(path_detail) => {
            report(PlironBarrierFindingV1::AnalysisIncomplete {
                detail: format!(
                    "{}; all-path convergence proof also failed: {path_detail}",
                    trace_failure_detail(trace_failure),
                ),
            })
        }
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

fn summarize_all_barrier_paths(context: &Context, function: &FuncOp) -> BarrierPathSummaryV1 {
    let blocks = function
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect::<Vec<_>>();
    let block_indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<Ptr<BasicBlock>, usize>>();
    if blocks.is_empty() {
        return BarrierPathSummaryV1::Incomplete("the kernel CFG is empty".to_owned());
    }
    let mut states = vec![0_u8; blocks.len()];
    let mut summaries = vec![None; blocks.len()];
    match summarize_barrier_paths_from(
        context,
        &blocks,
        &block_indices,
        0,
        &mut states,
        &mut summaries,
    ) {
        Ok(_) => BarrierPathSummaryV1::Unique,
        Err(BarrierPathFailureV1::Divergent {
            first_trace,
            second_trace,
        }) => BarrierPathSummaryV1::Divergent {
            first_trace,
            second_trace,
        },
        Err(BarrierPathFailureV1::Incomplete(detail)) => BarrierPathSummaryV1::Incomplete(detail),
    }
}

enum BarrierPathFailureV1 {
    Divergent {
        first_trace: Vec<(usize, usize)>,
        second_trace: Vec<(usize, usize)>,
    },
    Incomplete(String),
}

fn summarize_barrier_paths_from(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
    block_index: usize,
    states: &mut [u8],
    summaries: &mut [Option<Vec<(usize, usize)>>],
) -> Result<Vec<(usize, usize)>, BarrierPathFailureV1> {
    match states.get(block_index).copied() {
        Some(2) => {
            return Ok(summaries[block_index]
                .as_ref()
                .expect("completed barrier path summary")
                .clone());
        }
        Some(1) => {
            return Err(BarrierPathFailureV1::Incomplete(format!(
                "block {block_index} participates in cyclic control flow"
            )));
        }
        Some(_) => {}
        None => {
            return Err(BarrierPathFailureV1::Incomplete(
                "a CFG successor is outside the kernel".to_owned(),
            ));
        }
    }
    states[block_index] = 1;
    let block = blocks[block_index];
    let terminator = block
        .deref(context)
        .get_terminator(context)
        .ok_or_else(|| {
            BarrierPathFailureV1::Incomplete(format!("block {block_index} has no terminator"))
        })?;
    let mut local = Vec::new();
    for (operation_index, operation) in block.deref(context).iter(context).enumerate() {
        if operation == terminator {
            continue;
        }
        if Operation::get_op_dyn(operation, context)
            .downcast_ref::<BarrierOp>()
            .is_some()
        {
            local.push((block_index, operation_index));
        }
    }
    let terminator = Operation::get_op_dyn(terminator, context);
    let raw = terminator.get_operation().deref(context);
    let successors = if terminator.downcast_ref::<ReturnOp>().is_some() {
        Vec::new()
    } else if terminator.downcast_ref::<BranchOp>().is_some()
        || terminator.downcast_ref::<IndexLessThanBranchOp>().is_some()
        || terminator.downcast_ref::<AnalysisSplitOp>().is_some()
    {
        raw.successors()
            .map(|successor| {
                block_indices.get(&successor).copied().ok_or_else(|| {
                    BarrierPathFailureV1::Incomplete(format!(
                        "block {block_index} targets a block outside the kernel"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        return Err(BarrierPathFailureV1::Incomplete(format!(
            "block {block_index} has an unsupported terminator"
        )));
    };
    let mut complete: Option<Vec<(usize, usize)>> = None;
    for successor in successors {
        let suffix = summarize_barrier_paths_from(
            context,
            blocks,
            block_indices,
            successor,
            states,
            summaries,
        )?;
        let mut candidate = local.clone();
        candidate.extend(suffix);
        if let Some(first) = &complete {
            if first != &candidate {
                return Err(BarrierPathFailureV1::Divergent {
                    first_trace: first.clone(),
                    second_trace: candidate,
                });
            }
        } else {
            complete = Some(candidate);
        }
    }
    let complete = complete.unwrap_or(local);
    states[block_index] = 2;
    summaries[block_index] = Some(complete.clone());
    Ok(complete)
}

pub(crate) fn require_pliron_barrier_convergence_after_bounds_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironBarrierReportV1, PlironBarrierCheckErrorV1> {
    let report = run_pliron_barrier_convergence_check_after_bounds_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironBarrierCheckErrorV1 { report })
    }
}

pub fn require_pliron_barrier_convergence_before_lowering_v1(
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
            } if *execution_scope == scope => Some((*location, *address_space)),
            PlironTraceEventV1::Barrier { .. }
            | PlironTraceEventV1::Fence { .. }
            | PlironTraceEventV1::Memory { .. } => None,
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
    match failure {
        PlironTraceFailureV1::Sparse(failure) => {
            format!("sparse index analysis failed: {failure:?}")
        }
        PlironTraceFailureV1::DynamicLaunch { dimension } => {
            format!("launch dimension {dimension} is dynamic")
        }
        PlironTraceFailureV1::LaunchTooLarge { invocations } => {
            format!("launch domain has {invocations} invocations")
        }
        PlironTraceFailureV1::UnresolvedBranch { block } => {
            format!("branch in block {block} has an unresolved condition")
        }
        PlironTraceFailureV1::ForeignView { block, operation } => {
            format!("memory view at block {block} op {operation} is unresolved")
        }
        PlironTraceFailureV1::UnsupportedTerminator { block } => {
            format!("block {block} has an unsupported terminator")
        }
        PlironTraceFailureV1::CyclicControlFlow { block } => {
            format!(
                "block {block} participates in cyclic control flow; progress-dependent spin synchronization is unsupported"
            )
        }
        PlironTraceFailureV1::MissingExecutionLayout => {
            "scoped synchronization lacks a retained gpu.execution_layout".to_owned()
        }
        PlironTraceFailureV1::InvalidExecutionLayout => {
            "gpu.execution_layout is malformed, duplicated, or outside the entry block".to_owned()
        }
        PlironTraceFailureV1::UnsupportedGridSynchronization { block, operation } => {
            format!(
                "ordinary grid-wide barriers are unsupported at block {block} op {operation}; use disjoint workgroup ownership or legal device-scope atomics"
            )
        }
        PlironTraceFailureV1::PartialBarrierParticipants {
            scope,
            invocations,
            participant_width,
        } => format!(
            "{scope:?} barrier has {invocations} logical invocations, which is not a multiple of participant width {participant_width}; rounded physical lanes and their activity paths are not represented"
        ),
        PlironTraceFailureV1::ResourceLimit => "trace resource limit exceeded".to_owned(),
    }
}
