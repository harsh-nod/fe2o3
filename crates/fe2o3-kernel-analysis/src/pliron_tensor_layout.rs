//! Bounded, workload-neutral verification of cooperative tensor distributions.

use std::{error::Error, fmt};

use dialect_kernel::{TensorConvergenceAttr, TensorLayoutOp};
use fe2o3_kernel_ir::{TensorLayoutFindingV1, verify_tensor_layout_contract_v1};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::Context,
    linked_list::ContainsLinkedList,
    operation::Operation,
};

use crate::pliron_barrier::trace_failure_detail;
use crate::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceLocationV1, trace_pliron_invocations_v1,
};
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1};

pub const MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1: usize = 16_384;
pub const MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironTensorLayoutFindingV1 {
    Contract {
        block: usize,
        operation: usize,
        finding: TensorLayoutFindingV1,
    },
    ActiveLaneMismatch {
        block: usize,
        operation: usize,
        actual: u32,
    },
    ConvergenceMismatch {
        block: usize,
        operation: usize,
        actual: TensorConvergenceAttr,
    },
    MalformedContract {
        block: usize,
        operation: usize,
    },
    DivergentInstructionTrace {
        first_invocation: Vec<u64>,
        first_trace: Vec<(usize, usize)>,
        second_invocation: Vec<u64>,
        second_trace: Vec<(usize, usize)>,
    },
    ConvergenceAnalysisIncomplete {
        detail: String,
    },
    ResourceLimitExceeded,
}

impl PlironTensorLayoutFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::Contract { finding, .. } if finding.is_incomplete() => {
                KernelCheckStatusV1::Incomplete
            }
            Self::ConvergenceAnalysisIncomplete { .. } | Self::ResourceLimitExceeded => {
                KernelCheckStatusV1::Incomplete
            }
            Self::Contract { .. }
            | Self::ActiveLaneMismatch { .. }
            | Self::ConvergenceMismatch { .. }
            | Self::MalformedContract { .. }
            | Self::DivergentInstructionTrace { .. } => KernelCheckStatusV1::Rejected,
        }
    }
}

impl fmt::Display for PlironTensorLayoutFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract {
                block,
                operation,
                finding,
            } if finding.is_incomplete() => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-002]: tensor layout analysis is incomplete at block {block} op {operation}: {finding}",
            ),
            Self::Contract {
                block,
                operation,
                finding,
            } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-001]: tensor layout rejected at block {block} op {operation}: {finding}",
            ),
            Self::ActiveLaneMismatch {
                block,
                operation,
                actual,
            } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-001]: tensor layout rejected at block {block} op {operation}: exact subgroup participation requires 64 active lanes, found {actual}",
            ),
            Self::ConvergenceMismatch {
                block,
                operation,
                actual,
            } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-001]: tensor layout rejected at block {block} op {operation}: exact uniform subgroup convergence is required, found {actual:?}",
            ),
            Self::MalformedContract { block, operation } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-001]: tensor layout rejected malformed contract at block {block} op {operation}",
            ),
            Self::DivergentInstructionTrace {
                first_invocation,
                first_trace,
                second_invocation,
                second_trace,
            } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-001]: divergent tensor-instruction trace; invocation {first_invocation:?} executes {first_trace:?}, while invocation {second_invocation:?} executes {second_trace:?}; every subgroup participant must execute the same tensor instructions in the same order",
            ),
            Self::ConvergenceAnalysisIncomplete { detail } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-002]: tensor convergence analysis is incomplete: {detail}",
            ),
            Self::ResourceLimitExceeded => formatter.write_str(
                "error[FE2O3-TENSOR-LAYOUT-003]: tensor layout analysis resource limit exceeded",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironTensorLayoutReportV1 {
    status: KernelCheckStatusV1,
    findings: Vec<PlironTensorLayoutFindingV1>,
}

impl PlironTensorLayoutReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::TensorLayout
    }

    pub const fn status(&self) -> KernelCheckStatusV1 {
        self.status
    }

    pub fn findings(&self) -> &[PlironTensorLayoutFindingV1] {
        &self.findings
    }

    pub const fn is_clean(&self) -> bool {
        matches!(self.status, KernelCheckStatusV1::Clean)
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironTensorLayoutCheckErrorV1 {
    report: PlironTensorLayoutReportV1,
}

impl PlironTensorLayoutCheckErrorV1 {
    pub const fn report(&self) -> &PlironTensorLayoutReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironTensorLayoutCheckErrorV1 {
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

impl Error for PlironTensorLayoutCheckErrorV1 {}

pub fn run_pliron_tensor_layout_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironTensorLayoutReportV1 {
    let mut findings = Vec::new();
    let mut operation_count = 0;
    let mut has_tensor_instruction = false;
    for (block_index, block) in function
        .get_region(context)
        .deref(context)
        .iter(context)
        .enumerate()
    {
        for (operation_index, operation) in block.deref(context).iter(context).enumerate() {
            operation_count += 1;
            if operation_count > MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1
                || findings.len() >= MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1
            {
                findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                return report(findings);
            }
            let operation = Operation::get_op_dyn(operation, context);
            let Some(layout) = operation.downcast_ref::<TensorLayoutOp>() else {
                continue;
            };
            has_tensor_instruction = true;
            let Ok(contract) = layout.contract(context) else {
                findings.push(PlironTensorLayoutFindingV1::MalformedContract {
                    block: block_index,
                    operation: operation_index,
                });
                continue;
            };
            if layout.active_lanes(context) != Some(64) {
                findings.push(PlironTensorLayoutFindingV1::ActiveLaneMismatch {
                    block: block_index,
                    operation: operation_index,
                    actual: layout.active_lanes(context).unwrap_or(0),
                });
            }
            if layout.convergence(context) != Some(TensorConvergenceAttr::UniformSubgroup) {
                findings.push(PlironTensorLayoutFindingV1::ConvergenceMismatch {
                    block: block_index,
                    operation: operation_index,
                    actual: layout
                        .convergence(context)
                        .unwrap_or(TensorConvergenceAttr::Opaque),
                });
            }
            for finding in verify_tensor_layout_contract_v1(&contract) {
                if findings.len() >= MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
                    findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                    return report(findings);
                }
                findings.push(PlironTensorLayoutFindingV1::Contract {
                    block: block_index,
                    operation: operation_index,
                    finding,
                });
            }
        }
    }
    if has_tensor_instruction {
        match trace_pliron_invocations_v1(context, function) {
            Ok(traces) if traces.len() < 64 => {
                findings.push(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "the retained launch describes {} invocations; exact wave64 participation needs at least 64",
                        traces.len()
                    ),
                });
            }
            Ok(traces) => {
                if let Some(first) = traces.first() {
                    let first_tensor = tensor_trace(first);
                    for trace in traces.iter().skip(1) {
                        let tensor = tensor_trace(trace);
                        if tensor != first_tensor {
                            findings.push(PlironTensorLayoutFindingV1::DivergentInstructionTrace {
                                first_invocation: first.invocation.clone(),
                                first_trace: first_tensor
                                    .iter()
                                    .map(|location| (location.block, location.operation))
                                    .collect(),
                                second_invocation: trace.invocation.clone(),
                                second_trace: tensor
                                    .iter()
                                    .map(|location| (location.block, location.operation))
                                    .collect(),
                            });
                            break;
                        }
                    }
                }
            }
            Err(failure) => {
                findings.push(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: trace_failure_detail(failure),
                });
            }
        }
    }
    report(findings)
}

fn tensor_trace(trace: &PlironInvocationTraceV1) -> Vec<PlironTraceLocationV1> {
    trace
        .events
        .iter()
        .filter_map(|event| match event {
            PlironTraceEventV1::TensorInstruction { location } => Some(*location),
            PlironTraceEventV1::Barrier { .. }
            | PlironTraceEventV1::Fence { .. }
            | PlironTraceEventV1::Memory { .. } => None,
        })
        .collect()
}

pub fn require_pliron_tensor_layout_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironTensorLayoutReportV1, PlironTensorLayoutCheckErrorV1> {
    let report = run_pliron_tensor_layout_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironTensorLayoutCheckErrorV1 { report })
    }
}

fn report(findings: Vec<PlironTensorLayoutFindingV1>) -> PlironTensorLayoutReportV1 {
    let status = findings
        .iter()
        .fold(KernelCheckStatusV1::Clean, |status, finding| {
            match (status, finding.status()) {
                (KernelCheckStatusV1::Rejected, _) | (_, KernelCheckStatusV1::Rejected) => {
                    KernelCheckStatusV1::Rejected
                }
                (KernelCheckStatusV1::Incomplete, _) | (_, KernelCheckStatusV1::Incomplete) => {
                    KernelCheckStatusV1::Incomplete
                }
                _ => KernelCheckStatusV1::Clean,
            }
        });
    PlironTensorLayoutReportV1 { status, findings }
}
