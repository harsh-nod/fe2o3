//! Barrier-convergence verification over bounded PLIRON invocation traces.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use dialect_gpu::{AddressSpaceAttr, BarrierOp};
use dialect_kernel::{BranchOp, ReturnOp};
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
    if has_one_unconditional_control_flow_trace(context, function) {
        return PlironBarrierReportV1 { findings: vec![] };
    }
    let traces = match trace_pliron_invocations_v1(context, function) {
        Ok(traces) => traces,
        Err(failure) => {
            return report(PlironBarrierFindingV1::AnalysisIncomplete {
                detail: trace_failure_detail(failure),
            });
        }
    };
    let Some(first) = traces.first() else {
        return report(PlironBarrierFindingV1::AnalysisIncomplete {
            detail: "the launch domain is empty".to_owned(),
        });
    };
    let first_barriers = barrier_trace(first);
    for trace in traces.iter().skip(1) {
        let barriers = barrier_trace(trace);
        if barriers != first_barriers {
            return report(PlironBarrierFindingV1::DivergentBarrierTrace {
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
    PlironBarrierReportV1 { findings: vec![] }
}

/// Proves convergence without enumerating invocations when every invocation
/// necessarily follows one finite chain of unconditional CFG edges.
fn has_one_unconditional_control_flow_trace(context: &Context, function: &FuncOp) -> bool {
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
    let Some(mut block) = blocks.first().copied() else {
        return false;
    };
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(block) {
            return false;
        }
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            return false;
        };
        let terminator = Operation::get_op_dyn(terminator, context);
        if terminator.downcast_ref::<ReturnOp>().is_some() {
            return true;
        }
        if terminator.downcast_ref::<BranchOp>().is_none() {
            return false;
        }
        let Some(successor) = terminator
            .get_operation()
            .deref(context)
            .successors()
            .next()
        else {
            return false;
        };
        let Some(index) = block_indices.get(&successor).copied() else {
            return false;
        };
        block = blocks[index];
    }
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
) -> Vec<(PlironTraceLocationV1, AddressSpaceAttr)> {
    trace
        .events
        .iter()
        .filter_map(|event| match event {
            PlironTraceEventV1::Barrier {
                location,
                address_space,
            } => Some((*location, *address_space)),
            PlironTraceEventV1::Memory { .. } => None,
        })
        .collect()
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
            format!("block {block} participates in cyclic control flow")
        }
        PlironTraceFailureV1::ResourceLimit => "trace resource limit exceeded".to_owned(),
    }
}
