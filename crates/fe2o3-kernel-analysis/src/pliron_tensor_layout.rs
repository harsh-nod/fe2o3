//! Bounded, workload-neutral verification of cooperative tensor distributions.

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    error::Error,
    fmt,
};

use dialect_kernel::{
    AnalysisSplitOp, BranchArgsOp, BranchOp, DeterministicJoinOp, IndexBinaryOp, IndexConstantOp,
    IndexEqualBranchArgsOp, IndexEqualBranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp,
    InvocationIndexOp, MAX_DETERMINISTIC_JOIN_INPUTS_V1, ReturnOp, TensorConvergenceAttr,
    TensorLayoutOp,
};
use fe2o3_kernel_ir::{TensorLayoutFindingV1, verify_tensor_layout_contract_v1};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    operation::Operation,
    value::Value,
};

use crate::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::pliron_barrier::trace_failure_detail;
use crate::pliron_invocation_trace::{
    PlironExecutionLayoutV1, PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceFailureV1,
    PlironTraceLocationV1,
};
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1, SparseIndexAnalysisV1, SparseIndexFactV1};

pub const MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1: usize = 16_384;
pub const MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1: usize = 256;
pub const MAX_PLIRON_TENSOR_UNIFORMITY_VALUES_V1: usize = 65_536;
pub const MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1: usize = 1_048_576;

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
        expected: u64,
        actual: u32,
    },
    ExecutionLayoutMismatch {
        block: usize,
        operation: usize,
        declared: u64,
        required: u64,
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
    PartialSubgroupParticipation {
        grid: u64,
        workgroup: u64,
        subgroup: u64,
        expected: u64,
        actual: usize,
    },
    DivergentSubgroupControl {
        block: usize,
        operation: usize,
        controller: usize,
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
            | Self::ExecutionLayoutMismatch { .. }
            | Self::ConvergenceMismatch { .. }
            | Self::MalformedContract { .. }
            | Self::DivergentInstructionTrace { .. }
            | Self::PartialSubgroupParticipation { .. }
            | Self::DivergentSubgroupControl { .. } => KernelCheckStatusV1::Rejected,
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
                expected,
                actual,
            } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-001]: tensor layout rejected at block {block} op {operation}: authenticated execution requires {expected} active lanes, found {actual}",
            ),
            Self::ExecutionLayoutMismatch {
                block,
                operation,
                declared,
                required,
            } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-001]: tensor layout rejected at block {block} op {operation}: authenticated execution layout declares subgroup width {declared}, but the tensor contract requires {required}",
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
            Self::PartialSubgroupParticipation {
                grid,
                workgroup,
                subgroup,
                expected,
                actual,
            } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-001]: tensor subgroup ({grid}, {workgroup}, {subgroup}) has {actual} retained participants; the authenticated execution layout requires all {expected} lanes",
            ),
            Self::DivergentSubgroupControl {
                block,
                operation,
                controller,
            } => write!(
                formatter,
                "error[FE2O3-TENSOR-LAYOUT-001]: tensor instruction at block {block} op {operation} is control-dependent on subgroup-varying branch block {controller}",
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

    /// Contract consistency is not a source-to-IR or producer/dominance proof.
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    /// Raw ranked declarations never authorize artifact publication or launch.
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
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_tensor_layout_check_with_analyses_v1(context, function, &mut analyses)
}

pub(crate) fn run_pliron_tensor_layout_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironTensorLayoutReportV1 {
    let mut findings = Vec::new();
    let mut operation_count = 0;
    let mut tensor_sites = Vec::new();
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
            tensor_sites.push((
                block_index,
                operation_index,
                layout.active_lanes(context),
                None,
            ));
            let Ok(contract) = layout.contract(context) else {
                findings.push(PlironTensorLayoutFindingV1::MalformedContract {
                    block: block_index,
                    operation: operation_index,
                });
                continue;
            };
            tensor_sites
                .last_mut()
                .expect("just inserted tensor site")
                .3 = Some(u64::from(contract.subgroup_width));
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
    if !tensor_sites.is_empty() {
        analyses.prepare_execution_layout(context, function);
        let layout = match analyses.execution_layout() {
            Ok(Some(layout)) => layout,
            Ok(None) => {
                findings.push(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "tensor instructions require one authenticated gpu.execution_layout in the entry block"
                        .to_owned(),
                });
                return report(findings);
            }
            Err(failure) => {
                findings.push(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: trace_failure_detail(failure),
                });
                return report(findings);
            }
        };
        for (block, operation, active_lanes, contract_width) in &tensor_sites {
            if let Some(required) = contract_width
                && layout.subgroup_size != *required
            {
                if findings.len() >= MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
                    findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                    return report(findings);
                }
                findings.push(PlironTensorLayoutFindingV1::ExecutionLayoutMismatch {
                    block: *block,
                    operation: *operation,
                    declared: layout.subgroup_size,
                    required: *required,
                });
            }
            if let Some(actual) = active_lanes
                && u64::from(*actual) != layout.subgroup_size
            {
                if findings.len() >= MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
                    findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                    return report(findings);
                }
                findings.push(PlironTensorLayoutFindingV1::ActiveLaneMismatch {
                    block: *block,
                    operation: *operation,
                    expected: layout.subgroup_size,
                    actual: *actual,
                });
            }
        }
        analyses.prepare_exact_trace(context, function);
        match analyses.exact_trace() {
            Ok(traces) => {
                if let Some(finding) = exact_subgroup_trace_finding(&traces, layout.subgroup_size) {
                    findings.push(finding);
                }
            }
            Err(
                PlironTraceFailureV1::DynamicLaunch { .. }
                | PlironTraceFailureV1::LaunchTooLarge { .. }
                | PlironTraceFailureV1::UnresolvedBranch { .. }
                | PlironTraceFailureV1::CyclicControlFlow { .. }
                | PlironTraceFailureV1::UnsupportedTerminator { .. },
            ) => match symbolic_subgroup_convergence(
                context,
                function,
                layout,
                analyses,
                &tensor_sites
                    .iter()
                    .map(|(block, operation, _, _)| (*block, *operation))
                    .collect::<Vec<_>>(),
            ) {
                Ok(()) => {}
                Err(finding) => findings.push(finding),
            },
            Err(failure) => {
                findings.push(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: trace_failure_detail(failure),
                });
            }
        }
    }
    report(findings)
}

fn exact_subgroup_trace_finding(
    traces: &[PlironInvocationTraceV1],
    subgroup_size: u64,
) -> Option<PlironTensorLayoutFindingV1> {
    let mut groups = BTreeMap::<(u64, u64, u64), Vec<&PlironInvocationTraceV1>>::new();
    for trace in traces {
        groups
            .entry((trace.grid, trace.workgroup, trace.subgroup))
            .or_default()
            .push(trace);
    }
    for ((grid, workgroup, subgroup), group) in groups {
        if !group.iter().any(|trace| !tensor_trace(trace).is_empty()) {
            continue;
        }
        let lanes = group.iter().map(|trace| trace.lane).collect::<HashSet<_>>();
        let complete = lanes.len() == subgroup_size as usize
            && group.len() == subgroup_size as usize
            && (0..subgroup_size).all(|lane| lanes.contains(&lane));
        if !complete {
            return Some(PlironTensorLayoutFindingV1::PartialSubgroupParticipation {
                grid,
                workgroup,
                subgroup,
                expected: subgroup_size,
                actual: lanes.len(),
            });
        }
        let first = group[0];
        let first_tensor = tensor_trace(first);
        for trace in group.iter().copied().skip(1) {
            let tensor = tensor_trace(trace);
            if tensor != first_tensor {
                return Some(PlironTensorLayoutFindingV1::DivergentInstructionTrace {
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
            }
        }
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubgroupBranchUniformityV1 {
    Uniform,
    Varying,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubgroupValueUniformityV1 {
    Uniform,
    Unknown,
    Varying,
}

impl SubgroupValueUniformityV1 {
    const fn rank(self) -> u8 {
        match self {
            Self::Uniform => 0,
            Self::Unknown => 1,
            Self::Varying => 2,
        }
    }

    fn merge(inputs: impl IntoIterator<Item = Self>) -> Self {
        inputs
            .into_iter()
            .max_by_key(|uniformity| uniformity.rank())
            .unwrap_or(Self::Unknown)
    }
}

enum SubgroupValueDefinitionV1 {
    Fixed(SubgroupValueUniformityV1),
    Merge(Vec<Value>),
}

struct PlironSubgroupUniformityV1 {
    facts: HashMap<Value, SubgroupValueUniformityV1>,
}

impl PlironSubgroupUniformityV1 {
    fn fact(&self, value: Value) -> SubgroupValueUniformityV1 {
        self.facts
            .get(&value)
            .copied()
            .unwrap_or(SubgroupValueUniformityV1::Unknown)
    }
}

struct SymbolicTensorCfgV1 {
    successors: Vec<Vec<usize>>,
    branch_uniformity: Vec<SubgroupBranchUniformityV1>,
    reachable: Vec<bool>,
}

fn symbolic_subgroup_convergence(
    context: &Context,
    function: &FuncOp,
    layout: PlironExecutionLayoutV1,
    analyses: &mut PlironAnalysisManagerV1,
    tensor_sites: &[(usize, usize)],
) -> Result<(), PlironTensorLayoutFindingV1> {
    let potentially_partial = layout
        .global_extents
        .iter()
        .zip(layout.workgroup_extents)
        .any(|(global, workgroup)| *global == 0 || !global.is_multiple_of(workgroup));
    if potentially_partial
        && layout.execution_domain != dialect_gpu::ExecutionDomainAttr::FullPhysicalWorkgroups
    {
        return Err(
            PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "symbolic tensor convergence cannot establish full subgroup participation for a partial workgroup"
                    .to_owned(),
            },
        );
    }
    analyses.prepare_sparse_indices(context, function);
    let sparse = analyses.sparse_indices().map_err(|failure| {
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: format!("sparse predicate analysis failed: {failure:?}"),
        }
    })?;
    let uniformity = analyze_pliron_subgroup_uniformity(context, function, layout)?;
    let blocks = function
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect::<Vec<_>>();
    if blocks.is_empty() || blocks.len() > MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1 {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    }
    let block_indices = blocks
        .iter()
        .copied()
        .enumerate()
        .map(|(index, block)| (block, index))
        .collect::<HashMap<_, _>>();
    let entry = function.get_entry_block(context);
    let mut successors = Vec::with_capacity(blocks.len());
    let mut branch_uniformity = Vec::with_capacity(blocks.len());
    for (block_index, block) in blocks.iter().copied().enumerate() {
        let terminator = block
            .deref(context)
            .get_terminator(context)
            .ok_or_else(
                || PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!("block {block_index} has no terminator"),
                },
            )?;
        let terminator = Operation::get_op_dyn(terminator, context);
        let raw = terminator.get_operation().deref(context);
        let (kind, expected_successors) = if terminator.downcast_ref::<ReturnOp>().is_some() {
            (SubgroupBranchUniformityV1::Uniform, 0)
        } else if terminator.downcast_ref::<BranchOp>().is_some()
            || terminator.downcast_ref::<BranchArgsOp>().is_some()
        {
            (SubgroupBranchUniformityV1::Uniform, 1)
        } else if let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchOp>() {
            if raw.get_num_operands() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} index comparison has a malformed operand count"
                    ),
                });
            }
            (
                classify_subgroup_predicate(
                    entry,
                    layout,
                    &sparse,
                    &uniformity,
                    branch.lhs(context),
                    branch.rhs(context),
                ),
                2,
            )
        } else if let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchArgsOp>() {
            if raw.get_num_successors() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} typed index comparison has a malformed successor count"
                    ),
                });
            }
            let expected_operands = 2
                + raw.get_successor(0).deref(context).get_num_arguments()
                + raw.get_successor(1).deref(context).get_num_arguments();
            if raw.get_num_operands() != expected_operands {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} typed index comparison has a malformed operand count"
                    ),
                });
            }
            (
                classify_subgroup_predicate(
                    entry,
                    layout,
                    &sparse,
                    &uniformity,
                    branch.lhs(context),
                    branch.rhs(context),
                ),
                2,
            )
        } else if let Some(branch) = terminator.downcast_ref::<IndexEqualBranchOp>() {
            if raw.get_num_operands() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} equality comparison has a malformed operand count"
                    ),
                });
            }
            (
                classify_subgroup_equality(
                    entry,
                    layout,
                    &sparse,
                    &uniformity,
                    branch.lhs(context),
                    branch.rhs(context),
                ),
                2,
            )
        } else if let Some(branch) = terminator.downcast_ref::<IndexEqualBranchArgsOp>() {
            if raw.get_num_successors() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} typed equality comparison has a malformed successor count"
                    ),
                });
            }
            let expected_operands = 2
                + raw.get_successor(0).deref(context).get_num_arguments()
                + raw.get_successor(1).deref(context).get_num_arguments();
            if raw.get_num_operands() != expected_operands {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} typed equality comparison has a malformed operand count"
                    ),
                });
            }
            (
                classify_subgroup_equality(
                    entry,
                    layout,
                    &sparse,
                    &uniformity,
                    branch.lhs(context),
                    branch.rhs(context),
                ),
                2,
            )
        } else if terminator.downcast_ref::<AnalysisSplitOp>().is_some() {
            (SubgroupBranchUniformityV1::Unknown, 2)
        } else {
            return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: format!("block {block_index} has an unsupported terminator"),
            });
        };
        if raw.get_num_successors() != expected_successors {
            return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: format!(
                    "block {block_index} terminator has {} successors, expected {expected_successors}",
                    raw.get_num_successors()
                ),
            });
        }
        let targets = raw
            .successors()
            .map(|successor| {
                block_indices.get(&successor).copied().ok_or_else(|| {
                    PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: format!("block {block_index} targets a block outside the kernel"),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        successors.push(targets);
        branch_uniformity.push(kind);
    }

    let mut reachable = vec![false; blocks.len()];
    let entry_index = block_indices.get(&entry).copied().ok_or_else(|| {
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: "the kernel entry block is outside its body region".to_owned(),
        }
    })?;
    let mut worklist = VecDeque::from([entry_index]);
    while let Some(block) = worklist.pop_front() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        worklist.extend(successors[block].iter().copied());
    }
    let cfg = SymbolicTensorCfgV1 {
        successors,
        branch_uniformity,
        reachable,
    };

    let tensor_blocks = tensor_sites.iter().copied().fold(
        BTreeMap::<usize, usize>::new(),
        |mut blocks, (block, operation)| {
            blocks.entry(block).or_insert(operation);
            blocks
        },
    );
    if tensor_blocks.len() > MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    }
    for (tensor_block, tensor_operation) in tensor_blocks {
        if tensor_block >= cfg.successors.len() || !cfg.reachable[tensor_block] {
            continue;
        }
        let can_reach_tensor = reverse_reachable(&cfg.successors, tensor_block);
        let can_avoid_tensor = can_avoid_tensor(&cfg.successors, tensor_block);
        for controller in 0..cfg.successors.len() {
            let kind = cfg.branch_uniformity[controller];
            let controls_future_tensor = cfg.successors[controller]
                .iter()
                .any(|successor| can_reach_tensor[*successor]);
            if !cfg.reachable[controller]
                || !can_reach_tensor[controller]
                || !controls_future_tensor
                || kind == SubgroupBranchUniformityV1::Uniform
            {
                continue;
            }
            let control_can_avoid_tensor = cfg.successors[controller]
                .iter()
                .copied()
                .any(|successor| can_avoid_tensor[successor]);
            if !control_can_avoid_tensor {
                continue;
            }
            return Err(match kind {
                SubgroupBranchUniformityV1::Varying => {
                    PlironTensorLayoutFindingV1::DivergentSubgroupControl {
                        block: tensor_block,
                        operation: tensor_operation,
                        controller,
                    }
                }
                SubgroupBranchUniformityV1::Unknown => {
                    PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: format!(
                            "tensor instruction at block {tensor_block} op {tensor_operation} is control-dependent on unresolved branch block {controller}"
                        ),
                    }
                }
                SubgroupBranchUniformityV1::Uniform => unreachable!(),
            });
        }
    }
    Ok(())
}

fn classify_subgroup_predicate(
    entry: Ptr<BasicBlock>,
    layout: PlironExecutionLayoutV1,
    sparse: &SparseIndexAnalysisV1,
    uniformity: &PlironSubgroupUniformityV1,
    lhs: Value,
    rhs: Value,
) -> SubgroupBranchUniformityV1 {
    if lhs == rhs {
        return SubgroupBranchUniformityV1::Uniform;
    }
    let lhs_is_entry_argument = value_is_entry_argument(lhs, entry);
    let rhs_is_entry_argument = value_is_entry_argument(rhs, entry);
    let lhs_uniformity = uniformity.fact(lhs);
    let rhs_uniformity = uniformity.fact(rhs);
    let lhs = sparse.fact(lhs);
    let rhs = sparse.fact(rhs);
    if (lhs_is_entry_argument
        || lhs_uniformity == SubgroupValueUniformityV1::Uniform
        || sparse_fact_is_subgroup_uniform(&lhs, layout))
        && (rhs_is_entry_argument
            || rhs_uniformity == SubgroupValueUniformityV1::Uniform
            || sparse_fact_is_subgroup_uniform(&rhs, layout))
    {
        return SubgroupBranchUniformityV1::Uniform;
    }
    if let (Some(lhs), Some(rhs)) = (lhs.affine(), rhs.affine()) {
        let differing_lane_coefficient = lhs
            .coefficients()
            .iter()
            .zip(rhs.coefficients())
            .enumerate()
            .any(|(dimension, (lhs, rhs))| {
                lhs != rhs && !invocation_axis_is_subgroup_uniform(dimension, layout)
            });
        if !differing_lane_coefficient
            && affine_is_total_over_layout(lhs, layout)
            && affine_is_total_over_layout(rhs, layout)
        {
            return SubgroupBranchUniformityV1::Uniform;
        }
    }
    if let Some(classification) = classify_coordinate_cutoff(&lhs, &rhs, layout, true) {
        return classification;
    }
    if let Some(classification) = classify_coordinate_cutoff(&rhs, &lhs, layout, false) {
        return classification;
    }
    if matches!(
        (lhs_uniformity, rhs_uniformity),
        (
            SubgroupValueUniformityV1::Varying,
            SubgroupValueUniformityV1::Uniform
        ) | (
            SubgroupValueUniformityV1::Uniform,
            SubgroupValueUniformityV1::Varying
        )
    ) {
        return SubgroupBranchUniformityV1::Varying;
    }
    SubgroupBranchUniformityV1::Unknown
}

fn classify_subgroup_equality(
    entry: Ptr<BasicBlock>,
    layout: PlironExecutionLayoutV1,
    sparse: &SparseIndexAnalysisV1,
    uniformity: &PlironSubgroupUniformityV1,
    lhs: Value,
    rhs: Value,
) -> SubgroupBranchUniformityV1 {
    if lhs == rhs {
        return SubgroupBranchUniformityV1::Uniform;
    }
    let lhs_uniformity = uniformity.fact(lhs);
    let rhs_uniformity = uniformity.fact(rhs);
    let lhs_fact = sparse.fact(lhs);
    let rhs_fact = sparse.fact(rhs);
    let lhs_is_uniform = value_is_entry_argument(lhs, entry)
        || lhs_uniformity == SubgroupValueUniformityV1::Uniform
        || sparse_fact_is_subgroup_uniform(&lhs_fact, layout);
    let rhs_is_uniform = value_is_entry_argument(rhs, entry)
        || rhs_uniformity == SubgroupValueUniformityV1::Uniform
        || sparse_fact_is_subgroup_uniform(&rhs_fact, layout);
    if lhs_is_uniform && rhs_is_uniform {
        return SubgroupBranchUniformityV1::Uniform;
    }
    if let (Some(lhs), Some(rhs)) = (lhs_fact.affine(), rhs_fact.affine())
        && lhs
            .coefficients()
            .iter()
            .zip(rhs.coefficients())
            .enumerate()
            .all(|(dimension, (lhs, rhs))| {
                lhs == rhs || invocation_axis_is_subgroup_uniform(dimension, layout)
            })
        && affine_is_total_over_layout(lhs, layout)
        && affine_is_total_over_layout(rhs, layout)
    {
        return SubgroupBranchUniformityV1::Uniform;
    }
    if matches!(
        (lhs_uniformity, rhs_uniformity),
        (
            SubgroupValueUniformityV1::Varying,
            SubgroupValueUniformityV1::Uniform
        ) | (
            SubgroupValueUniformityV1::Uniform,
            SubgroupValueUniformityV1::Varying
        )
    ) {
        return SubgroupBranchUniformityV1::Varying;
    }
    SubgroupBranchUniformityV1::Unknown
}

fn affine_is_total_over_layout(
    affine: &crate::SparseAffineIndexV1,
    layout: PlironExecutionLayoutV1,
) -> bool {
    let mut maximum = affine.constant_term();
    for (dimension, coefficient) in affine.coefficients().iter().copied().enumerate() {
        if coefficient == 0 {
            continue;
        }
        let Some(extent) = layout.global_extents.get(dimension).copied() else {
            return false;
        };
        let Some(coordinate) = extent.checked_sub(1) else {
            return false;
        };
        let Some(term) = coefficient.checked_mul(coordinate) else {
            return false;
        };
        let Some(next) = maximum.checked_add(term) else {
            return false;
        };
        maximum = next;
    }
    true
}

fn analyze_pliron_subgroup_uniformity(
    context: &Context,
    function: &FuncOp,
    layout: PlironExecutionLayoutV1,
) -> Result<PlironSubgroupUniformityV1, PlironTensorLayoutFindingV1> {
    let entry = function.get_entry_block(context);
    let mut definitions = HashMap::<Value, SubgroupValueDefinitionV1>::new();
    let mut definition_order = Vec::new();
    let mut dependents = HashMap::<Value, Vec<Value>>::new();
    let mut block_arguments = HashMap::<Ptr<BasicBlock>, Vec<Value>>::new();
    for block in function.get_region(context).deref(context).iter(context) {
        let arguments = block.deref(context).arguments().collect::<Vec<_>>();
        for argument in &arguments {
            definition_order.push(*argument);
            definitions.insert(
                *argument,
                if block == entry {
                    SubgroupValueDefinitionV1::Fixed(SubgroupValueUniformityV1::Uniform)
                } else {
                    SubgroupValueDefinitionV1::Merge(Vec::new())
                },
            );
        }
        block_arguments.insert(block, arguments);
        for operation in block.deref(context).iter(context) {
            let dynamic = Operation::get_op_dyn(operation, context);
            let raw = operation.deref(context);
            for result_index in 0..raw.get_num_results() {
                let result = raw.get_result(result_index);
                let definition = if dynamic.downcast_ref::<IndexConstantOp>().is_some() {
                    SubgroupValueDefinitionV1::Fixed(SubgroupValueUniformityV1::Uniform)
                } else if let Some(invocation) = dynamic.downcast_ref::<InvocationIndexOp>() {
                    let uniformity = match invocation
                        .dimension(context)
                        .and_then(|dimension| usize::try_from(dimension).ok())
                    {
                        Some(dimension)
                            if invocation_axis_is_subgroup_uniform(dimension, layout) =>
                        {
                            SubgroupValueUniformityV1::Uniform
                        }
                        Some(_) => SubgroupValueUniformityV1::Varying,
                        None => SubgroupValueUniformityV1::Unknown,
                    };
                    SubgroupValueDefinitionV1::Fixed(uniformity)
                } else if let Some(binary) = dynamic.downcast_ref::<IndexBinaryOp>()
                    && raw.get_num_operands() == 2
                {
                    SubgroupValueDefinitionV1::Merge(vec![binary.lhs(context), binary.rhs(context)])
                } else if let Some(join) = dynamic.downcast_ref::<DeterministicJoinOp>() {
                    let dependencies = join.dependencies(context);
                    if dependencies.is_empty() {
                        return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                            detail: "deterministic join has no explicit dependencies".to_owned(),
                        });
                    }
                    if dependencies.len() > MAX_DETERMINISTIC_JOIN_INPUTS_V1 {
                        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                    }
                    SubgroupValueDefinitionV1::Merge(dependencies)
                } else {
                    SubgroupValueDefinitionV1::Fixed(SubgroupValueUniformityV1::Unknown)
                };
                definition_order.push(result);
                definitions.insert(result, definition);
            }
        }
    }
    if definitions.len() > MAX_PLIRON_TENSOR_UNIFORMITY_VALUES_V1 {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    }

    for block in function.get_region(context).deref(context).iter(context) {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            continue;
        };
        let dynamic = Operation::get_op_dyn(terminator, context);
        let raw = terminator.deref(context);
        if dynamic
            .downcast_ref::<IndexLessThanBranchArgsOp>()
            .is_some()
            || dynamic.downcast_ref::<IndexEqualBranchArgsOp>().is_some()
        {
            if raw.get_num_successors() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "typed conditional edge has a malformed successor count".to_owned(),
                });
            }
            let expected_operands = 2
                + raw.get_successor(0).deref(context).get_num_arguments()
                + raw.get_successor(1).deref(context).get_num_arguments();
            if raw.get_num_operands() != expected_operands {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "typed conditional edge has a malformed operand count".to_owned(),
                });
            }
        }
        for (successor_index, successor) in raw.successors().enumerate() {
            let Some(arguments) = block_arguments.get(&successor) else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "a branch targets a block outside the kernel".to_owned(),
                });
            };
            if arguments.is_empty() {
                continue;
            }
            let incoming = if let Some(branch) = dynamic.downcast_ref::<BranchArgsOp>() {
                branch.arguments(context)
            } else if let Some(branch) = dynamic.downcast_ref::<IndexLessThanBranchArgsOp>() {
                if successor_index == 0 {
                    branch.true_arguments(context)
                } else {
                    branch.false_arguments(context)
                }
            } else if let Some(branch) = dynamic.downcast_ref::<IndexEqualBranchArgsOp>() {
                if successor_index == 0 {
                    branch.true_arguments(context)
                } else {
                    branch.false_arguments(context)
                }
            } else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "a block argument has a predecessor without typed edge operands"
                        .to_owned(),
                });
            };
            if incoming.len() != arguments.len() {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "typed edge operand and block argument counts differ".to_owned(),
                });
            }
            for (argument, incoming) in arguments.iter().zip(incoming) {
                let Some(SubgroupValueDefinitionV1::Merge(values)) = definitions.get_mut(argument)
                else {
                    return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: "an entry argument cannot receive a CFG edge operand".to_owned(),
                    });
                };
                values.push(incoming);
            }
        }
    }

    for value in &definition_order {
        let definition = definitions
            .get(value)
            .expect("ordered uniformity definition exists");
        if let SubgroupValueDefinitionV1::Merge(inputs) = definition {
            for input in inputs {
                dependents.entry(*input).or_default().push(*value);
            }
        }
    }
    let mut facts = definition_order
        .iter()
        .copied()
        .map(|value| (value, SubgroupValueUniformityV1::Uniform))
        .collect::<HashMap<_, _>>();
    let mut worklist = definition_order.into_iter().collect::<VecDeque<_>>();
    let mut work_units = 0_usize;
    while let Some(value) = worklist.pop_front() {
        work_units = work_units.saturating_add(1);
        if work_units > MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1 {
            return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
        }
        let next = match definitions.get(&value).expect("queued definition exists") {
            SubgroupValueDefinitionV1::Fixed(uniformity) => *uniformity,
            SubgroupValueDefinitionV1::Merge(inputs) => {
                SubgroupValueUniformityV1::merge(inputs.iter().map(|input| {
                    facts
                        .get(input)
                        .copied()
                        .unwrap_or(SubgroupValueUniformityV1::Unknown)
                }))
            }
        };
        let current = facts.get_mut(&value).expect("definition has a fact");
        if next.rank() <= current.rank() {
            continue;
        }
        *current = next;
        worklist.extend(dependents.get(&value).into_iter().flatten().copied());
    }
    Ok(PlironSubgroupUniformityV1 { facts })
}

fn value_is_entry_argument(value: Value, entry: Ptr<BasicBlock>) -> bool {
    value.defining_block() == Some(entry)
}

fn sparse_fact_is_subgroup_uniform(
    fact: &SparseIndexFactV1,
    layout: PlironExecutionLayoutV1,
) -> bool {
    match fact {
        SparseIndexFactV1::Affine(affine) => {
            affine
                .coefficients()
                .iter()
                .enumerate()
                .all(|(dimension, coefficient)| {
                    *coefficient == 0 || invocation_axis_is_subgroup_uniform(dimension, layout)
                })
        }
        SparseIndexFactV1::Remainder { dividend, modulus } => {
            *modulus == 1
                || dividend
                    .coefficients()
                    .iter()
                    .enumerate()
                    .all(|(dimension, coefficient)| {
                        *coefficient == 0 || invocation_axis_is_subgroup_uniform(dimension, layout)
                    })
        }
        SparseIndexFactV1::Unknown | SparseIndexFactV1::CheckedTiled2D(_) => false,
    }
}

fn invocation_axis_is_subgroup_uniform(dimension: usize, layout: PlironExecutionLayoutV1) -> bool {
    let Some(&extent) = layout.workgroup_extents.get(dimension) else {
        return false;
    };
    if extent == 1 {
        return true;
    }
    let Some(stride) = layout.workgroup_extents[..dimension]
        .iter()
        .try_fold(1_u64, |stride, extent| stride.checked_mul(*extent))
    else {
        return false;
    };
    stride >= layout.subgroup_size && stride.is_multiple_of(layout.subgroup_size)
}

fn classify_coordinate_cutoff(
    coordinate: &SparseIndexFactV1,
    cutoff: &SparseIndexFactV1,
    layout: PlironExecutionLayoutV1,
    coordinate_is_lhs: bool,
) -> Option<SubgroupBranchUniformityV1> {
    let affine = coordinate.affine()?;
    if affine.constant_term() != 0 {
        return None;
    }
    let mut dimensions = affine
        .coefficients()
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, coefficient)| *coefficient != 0);
    let (dimension, coefficient) = dimensions.next()?;
    if coefficient != 1 || dimensions.next().is_some() {
        return None;
    }
    let cutoff = cutoff.constant_value()?;
    let global_extent = layout.global_extents.get(dimension).copied()?;
    if (coordinate_is_lhs && cutoff == 0)
        || (!coordinate_is_lhs
            && (cutoff == u64::MAX
                || (global_extent != 0 && cutoff >= global_extent.saturating_sub(1))))
        || (coordinate_is_lhs && global_extent != 0 && cutoff >= global_extent)
        || invocation_axis_is_subgroup_uniform(dimension, layout)
    {
        return Some(SubgroupBranchUniformityV1::Uniform);
    }
    let boundary = if coordinate_is_lhs {
        cutoff
    } else {
        cutoff.checked_add(1)?
    };
    let extent = *layout.workgroup_extents.get(dimension)?;
    let stride = layout.workgroup_extents[..dimension]
        .iter()
        .try_fold(1_u64, |stride, extent| stride.checked_mul(*extent))?;
    let period = extent.checked_mul(stride)?;
    let transition = (boundary % extent).checked_mul(stride)?;
    if period.is_multiple_of(layout.subgroup_size) {
        return Some(if transition.is_multiple_of(layout.subgroup_size) {
            SubgroupBranchUniformityV1::Uniform
        } else {
            SubgroupBranchUniformityV1::Varying
        });
    }
    None
}

fn reverse_reachable(successors: &[Vec<usize>], target: usize) -> Vec<bool> {
    let mut predecessors = vec![Vec::new(); successors.len()];
    for (block, targets) in successors.iter().enumerate() {
        for target in targets {
            predecessors[*target].push(block);
        }
    }
    let mut reachable = vec![false; successors.len()];
    let mut worklist = VecDeque::from([target]);
    while let Some(block) = worklist.pop_front() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        worklist.extend(predecessors[block].iter().copied());
    }
    reachable
}

fn can_avoid_tensor(successors: &[Vec<usize>], tensor_block: usize) -> Vec<bool> {
    let mut predecessors = vec![Vec::new(); successors.len()];
    for (block, targets) in successors.iter().enumerate() {
        if block == tensor_block {
            continue;
        }
        for target in targets
            .iter()
            .copied()
            .filter(|target| *target != tensor_block)
        {
            predecessors[target].push(block);
        }
    }

    // Iterative Kosaraju avoids verifier stack growth on a legal large CFG.
    let mut visited = vec![false; successors.len()];
    visited[tensor_block] = true;
    let mut finish_order = Vec::with_capacity(successors.len());
    for start in 0..successors.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start, 0_usize)];
        while let Some((block, successor_index)) = stack.last_mut() {
            while *successor_index < successors[*block].len()
                && successors[*block][*successor_index] == tensor_block
            {
                *successor_index += 1;
            }
            if *successor_index == successors[*block].len() {
                finish_order.push(*block);
                stack.pop();
                continue;
            }
            let successor = successors[*block][*successor_index];
            *successor_index += 1;
            if !visited[successor] {
                visited[successor] = true;
                stack.push((successor, 0));
            }
        }
    }

    let mut component = vec![usize::MAX; successors.len()];
    let mut cyclic = Vec::new();
    for start in finish_order.into_iter().rev() {
        if component[start] != usize::MAX {
            continue;
        }
        let component_index = cyclic.len();
        component[start] = component_index;
        let mut nodes = Vec::new();
        let mut worklist = vec![start];
        while let Some(block) = worklist.pop() {
            nodes.push(block);
            for predecessor in predecessors[block].iter().copied() {
                if component[predecessor] == usize::MAX {
                    component[predecessor] = component_index;
                    worklist.push(predecessor);
                }
            }
        }
        cyclic.push(
            nodes.len() > 1
                || nodes
                    .first()
                    .is_some_and(|node| successors[*node].contains(node)),
        );
    }

    let mut can_avoid = vec![false; successors.len()];
    let mut worklist = VecDeque::new();
    for block in 0..successors.len() {
        let is_return = successors[block].is_empty();
        let is_cycle = component[block] != usize::MAX && cyclic[component[block]];
        if block != tensor_block && (is_return || is_cycle) {
            worklist.push_back(block);
        }
    }
    while let Some(block) = worklist.pop_front() {
        if can_avoid[block] {
            continue;
        }
        can_avoid[block] = true;
        worklist.extend(predecessors[block].iter().copied());
    }
    can_avoid
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

pub(crate) fn require_pliron_tensor_layout_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<PlironTensorLayoutReportV1, PlironTensorLayoutCheckErrorV1> {
    let report = run_pliron_tensor_layout_check_with_analyses_v1(context, function, analyses);
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
