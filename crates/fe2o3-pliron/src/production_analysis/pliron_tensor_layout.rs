//! Bounded, workload-neutral verification of cooperative tensor distributions.

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    error::Error,
    fmt,
};

#[cfg(test)]
#[path = "pliron_tensor_layout/boolean_control_v3_tests.rs"]
mod boolean_control_v3_tests;

use dialect_kernel::{
    AnalysisSplitOp, BranchArgsOp, BranchOp, DeterministicJoinOp, IndexBinaryKindAttr,
    IndexBinaryOp, IndexConstantOp, IndexEqualBranchArgsOp, IndexEqualBranchOp,
    IndexLessThanBranchArgsOp, IndexLessThanBranchOp, IndexUnsignedCastOp, InvocationIndexOp,
    MAX_DETERMINISTIC_JOIN_INPUTS_V1, ReturnOp, TensorConvergenceAttr, TensorLayoutOp, TrapOp,
};
use fe2o3_kernel_ir::{
    TensorFragmentLayoutV1, TensorInstructionProfileV1, TensorLayoutFindingV1, TensorOperandRoleV1,
    verify_tensor_layout_contract_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::ops::FuncOp,
    context::{Context, Ptr},
    operation::Operation,
    value::Value,
};

use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_barrier::trace_failure_detail;
use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::production_analysis::pliron_invocation_trace::{
    PlironExecutionLayoutV1, PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceFailureV1,
    PlironTraceLocationV1, ProductionInvocationTraceResourceAdmissionV1,
};
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::{
    KernelCheckPassKindV1, KernelCheckStatusV1, SparseIndexAnalysisV1, SparseIndexFactV1,
    SparseIndexFailureV1,
};

pub const MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1: usize = 16_384;
pub const MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1: usize = 256;
pub const MAX_PLIRON_TENSOR_UNIFORMITY_VALUES_V1: usize = 65_536;
pub const MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1: usize = 1_048_576;
pub const MAX_PLIRON_TENSOR_DATAFLOW_ROOTS_V1: usize = 16_384;
pub const MAX_PLIRON_TENSOR_DATAFLOW_EDGES_V1: usize = 65_536;

// The admitted profiles have three fragments. The widest is a wave64 gfx950
// operand with 32 components per lane, so one fragment verifier can retain at
// most 2,048 coordinate-map entries at once.
const MAX_TENSOR_CONTRACT_FRAGMENTS_V1: usize = 3;
const MAX_TENSOR_FRAGMENT_COORDINATES_V1: usize = 64 * 32;
const MAX_TENSOR_CONTRACT_FINDINGS_PER_SITE_V1: usize = 30;
const TENSOR_COORDINATE_MAP_ITEMS_PER_ENTRY_V1: usize = 8;

fn tensor_layout_resource_error_v1(resource: &'static str) -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::TensorLayout,
        resource,
    }
}

fn checked_tensor_layout_sum_v1(
    values: &[usize],
    resource: &'static str,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| tensor_layout_resource_error_v1(resource))
    })
}

fn checked_tensor_layout_product_v1(
    lhs: usize,
    rhs: usize,
    resource: &'static str,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(|| tensor_layout_resource_error_v1(resource))
}

/// Bounds tensor-contract/dataflow checking and either exact-trace comparison
/// or symbolic convergence. Symbolic fixed points are charged through their
/// rejecting step (`limit + 1`); an exact trace contributes its authenticated
/// invocation and event cardinalities instead of a workload-shaped guess.
pub(crate) fn preflight_tensor_layout_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    trace: Option<ProductionInvocationTraceResourceAdmissionV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::TensorLayout;
    if census.operations > MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1 {
        return Err(tensor_layout_resource_error_v1(
            "tensor-layout operation hard limit",
        ));
    }
    let values = census
        .results
        .checked_add(census.block_arguments)
        .ok_or_else(|| tensor_layout_resource_error_v1("tensor-layout value upper bound"))?;
    if values > MAX_PLIRON_TENSOR_UNIFORMITY_VALUES_V1 {
        return Err(tensor_layout_resource_error_v1(
            "tensor-layout value hard limit",
        ));
    }
    let tensor_sites = census.operations.min(MAX_PLIRON_TENSOR_DATAFLOW_ROOTS_V1);
    let dataflow_edges = checked_tensor_layout_product_v1(
        tensor_sites,
        3,
        "tensor-layout dataflow edge upper bound",
    )?;
    if dataflow_edges > MAX_PLIRON_TENSOR_DATAFLOW_EDGES_V1 {
        return Err(tensor_layout_resource_error_v1(
            "tensor-layout dataflow edge hard limit",
        ));
    }
    // One site can emit 23 contract findings, three execution/convergence
    // findings, and four dataflow findings. The trace comparison can add one
    // more function-level finding.
    let findings = checked_tensor_layout_product_v1(
        tensor_sites,
        MAX_TENSOR_CONTRACT_FINDINGS_PER_SITE_V1,
        "tensor-layout finding upper bound",
    )?
    .checked_add(1)
    .ok_or_else(|| tensor_layout_resource_error_v1("tensor-layout finding upper bound"))?
    .min(MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 + 1);
    let structural_work = census.checked_structural_items(phase)?;
    let sort_height = usize::BITS as usize - tensor_sites.leading_zeros() as usize;
    let dataflow_work = checked_tensor_layout_sum_v1(
        &[
            census.operations,
            checked_tensor_layout_product_v1(
                tensor_sites,
                sort_height.checked_add(8).ok_or_else(|| {
                    tensor_layout_resource_error_v1("tensor-layout dataflow work upper bound")
                })?,
                "tensor-layout dataflow work upper bound",
            )?,
            dataflow_edges,
        ],
        "tensor-layout dataflow work upper bound",
    )?;
    let coordinate_tree_height =
        usize::BITS as usize - MAX_TENSOR_FRAGMENT_COORDINATES_V1.leading_zeros() as usize;
    let contract_coordinate_work = checked_tensor_layout_product_v1(
        checked_tensor_layout_product_v1(
            tensor_sites,
            MAX_TENSOR_CONTRACT_FRAGMENTS_V1,
            "tensor-layout contract verification work upper bound",
        )?,
        checked_tensor_layout_product_v1(
            MAX_TENSOR_FRAGMENT_COORDINATES_V1,
            coordinate_tree_height.checked_add(16).ok_or_else(|| {
                tensor_layout_resource_error_v1(
                    "tensor-layout contract verification work upper bound",
                )
            })?,
            "tensor-layout contract verification work upper bound",
        )?,
        "tensor-layout contract verification work upper bound",
    )?;
    let rejecting_fixed_point_work = MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1
        .checked_add(1)
        .ok_or_else(|| tensor_layout_resource_error_v1("tensor-layout work upper bound"))?;
    let symbolic_work = checked_tensor_layout_product_v1(
        rejecting_fixed_point_work,
        3,
        "tensor-layout symbolic work upper bound",
    )?;
    let (invocations, events, launch_rank) = trace
        .map(|admission| {
            (
                admission.invocation_count(),
                admission.event_upper_bound(),
                admission.launch_rank(),
            )
        })
        .unwrap_or((0, 0, 0));
    let exact_work = checked_tensor_layout_sum_v1(
        &[
            checked_tensor_layout_product_v1(
                invocations,
                8,
                "tensor-layout exact-trace work upper bound",
            )?,
            checked_tensor_layout_product_v1(
                events,
                3,
                "tensor-layout exact-trace work upper bound",
            )?,
        ],
        "tensor-layout exact-trace work upper bound",
    )?;
    let work = checked_tensor_layout_sum_v1(
        &[
            checked_tensor_layout_product_v1(structural_work, 8, "tensor-layout work upper bound")?,
            dataflow_work,
            contract_coordinate_work,
            symbolic_work.max(exact_work),
        ],
        "tensor-layout work upper bound",
    )?;

    let per_finding = 96_usize
        .checked_add(dialect_kernel::MAX_RANKED_MEMORY_RANK * 4)
        .ok_or_else(|| tensor_layout_resource_error_v1("tensor-layout report upper bound"))?;
    let divergent_trace_storage = checked_tensor_layout_sum_v1(
        &[
            checked_tensor_layout_product_v1(
                events,
                2,
                "tensor-layout divergent trace upper bound",
            )?,
            checked_tensor_layout_product_v1(
                launch_rank,
                2,
                "tensor-layout divergent trace upper bound",
            )?,
        ],
        "tensor-layout divergent trace upper bound",
    )?;
    let retained_report = checked_tensor_layout_sum_v1(
        &[
            checked_tensor_layout_product_v1(
                findings,
                per_finding,
                "tensor-layout report upper bound",
            )?,
            census.identifier_bytes,
            divergent_trace_storage,
        ],
        "tensor-layout report upper bound",
    )?;
    // The manager retains dataflow facts beside the pass report.
    let retained_dataflow = checked_tensor_layout_sum_v1(
        &[
            checked_tensor_layout_product_v1(
                tensor_sites,
                24,
                "tensor-layout dataflow storage upper bound",
            )?,
            checked_tensor_layout_product_v1(
                findings,
                24,
                "tensor-layout dataflow storage upper bound",
            )?,
        ],
        "tensor-layout dataflow storage upper bound",
    )?;
    let retained = checked_tensor_layout_sum_v1(
        &[retained_report, retained_dataflow],
        "tensor-layout retained storage upper bound",
    )?;

    let block_words = census.blocks.div_ceil(u64::BITS as usize);
    let forwarding_storage = subgroup_forwarding_storage_items_v1(census.block_arguments)
        .ok_or_else(|| {
            tensor_layout_resource_error_v1("tensor-layout forwarding storage upper bound")
        })?;
    let selector_storage = checked_tensor_layout_product_v1(
        if census.block_arguments == 0 {
            0
        } else {
            census.blocks
        },
        5,
        "tensor-layout phi-selection storage upper bound",
    )?;
    let symbolic_temporary = checked_tensor_layout_sum_v1(
        &[
            // Phi rows coexist with either the forwarding index/memo/path or
            // selector headers/facts. The forwarding owner is dropped before
            // any controller allocation; the common symbolic bound below is
            // conservatively retained for both phases.
            checked_tensor_layout_product_v1(
                census.block_arguments,
                2,
                "tensor-layout phi-selection storage upper bound",
            )?,
            forwarding_storage.max(selector_storage),
            checked_tensor_layout_product_v1(
                values,
                16,
                "tensor-layout symbolic storage upper bound",
            )?,
            checked_tensor_layout_product_v1(
                census.operands,
                3,
                "tensor-layout symbolic storage upper bound",
            )?,
            checked_tensor_layout_product_v1(
                census.blocks,
                16,
                "tensor-layout symbolic storage upper bound",
            )?,
            checked_tensor_layout_product_v1(
                census.successors,
                6,
                "tensor-layout symbolic storage upper bound",
            )?,
            checked_tensor_layout_product_v1(
                census.blocks,
                block_words.checked_mul(3).ok_or_else(|| {
                    tensor_layout_resource_error_v1("tensor-layout symbolic storage upper bound")
                })?,
                "tensor-layout symbolic storage upper bound",
            )?,
        ],
        "tensor-layout symbolic storage upper bound",
    )?;
    let exact_temporary = checked_tensor_layout_sum_v1(
        &[
            checked_tensor_layout_product_v1(
                invocations,
                6,
                "tensor-layout exact-trace storage upper bound",
            )?,
            divergent_trace_storage,
        ],
        "tensor-layout exact-trace storage upper bound",
    )?;
    let dataflow_temporary = checked_tensor_layout_product_v1(
        tensor_sites,
        32,
        "tensor-layout dataflow temporary storage upper bound",
    )?;
    let contract_verification_temporary = checked_tensor_layout_sum_v1(
        &[
            checked_tensor_layout_product_v1(
                MAX_TENSOR_FRAGMENT_COORDINATES_V1,
                TENSOR_COORDINATE_MAP_ITEMS_PER_ENTRY_V1,
                "tensor-layout contract verification storage upper bound",
            )?,
            23,
        ],
        "tensor-layout contract verification storage upper bound",
    )?;
    let temporary = checked_tensor_layout_sum_v1(
        &[
            dataflow_temporary,
            contract_verification_temporary,
            symbolic_temporary.max(exact_temporary),
        ],
        "tensor-layout temporary storage upper bound",
    )?;
    let bound =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, temporary)?;
    limits.require(phase, bound)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlironTensorLayoutLocationV1 {
    pub block: usize,
    pub operation: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlironTensorLayoutFactV1 {
    pub layout: TensorFragmentLayoutV1,
    pub subgroup_width: u16,
    pub profile: TensorInstructionProfileV1,
    pub producer: PlironTensorLayoutLocationV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironTensorLayoutDataflowIssueV1 {
    MergeConflict {
        root: [u64; 4],
        first: PlironTensorLayoutFactV1,
        second: PlironTensorLayoutFactV1,
    },
    ConsumerMismatch {
        root: [u64; 4],
        producer: PlironTensorLayoutFactV1,
        consumer: PlironTensorLayoutLocationV1,
        consumer_profile: TensorInstructionProfileV1,
        operand: TensorOperandRoleV1,
        expected: TensorFragmentLayoutV1,
        expected_subgroup_width: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironTensorLayoutDataflowFailureV1 {
    ResourceLimit,
    MalformedSite { block: usize, operation: usize },
}

/// Whole-function layout facts for compiler-derived cooperative-tensor roots.
///
/// A root may have multiple CFG producers. Equal layouts join; unequal layouts
/// become an explicit conflict. Missing producer facts denote external checked
/// loads or zero initializers, not proof of an arbitrary layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironTensorLayoutDataflowAnalysisV1 {
    facts: BTreeMap<[u64; 4], PlironTensorLayoutFactV1>,
    conflicted_roots: HashSet<[u64; 4]>,
    issues: Vec<PlironTensorLayoutDataflowIssueV1>,
    bound_sites: usize,
}

impl PlironTensorLayoutDataflowAnalysisV1 {
    pub fn fact(&self, root: [u64; 4]) -> Option<PlironTensorLayoutFactV1> {
        (!self.conflicted_roots.contains(&root))
            .then(|| self.facts.get(&root).copied())
            .flatten()
    }

    pub fn issues(&self) -> &[PlironTensorLayoutDataflowIssueV1] {
        &self.issues
    }

    pub const fn bound_site_count(&self) -> usize {
        self.bound_sites
    }
}

#[derive(Clone, Copy)]
struct TensorDataflowSiteV1 {
    location: PlironTensorLayoutLocationV1,
    roots: dialect_kernel::TensorDataflowRootsV1,
    contract: fe2o3_kernel_ir::TensorLayoutContractV1,
}

pub(crate) fn analyze_pliron_tensor_layout_dataflow_with_inventory_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<PlironTensorLayoutDataflowAnalysisV1, PlironTensorLayoutDataflowFailureV1> {
    let mut sites = Vec::new();
    let mut operation_count = 0usize;
    for site in inventory.operations() {
        let block = site.block();
        let operation = site.operation();
        operation_count = operation_count.saturating_add(1);
        if operation_count > MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1 {
            return Err(PlironTensorLayoutDataflowFailureV1::ResourceLimit);
        }
        let operation_ref = Operation::get_op_dyn(site.pointer(), context);
        let Some(tensor) = operation_ref.downcast_ref::<TensorLayoutOp>() else {
            continue;
        };
        let roots = tensor
            .dataflow_roots(context)
            .map_err(|_| PlironTensorLayoutDataflowFailureV1::MalformedSite { block, operation })?;
        let Some(roots) = roots else {
            continue;
        };
        let contract = tensor
            .contract(context)
            .map_err(|_| PlironTensorLayoutDataflowFailureV1::MalformedSite { block, operation })?;
        if sites.len() == MAX_PLIRON_TENSOR_DATAFLOW_ROOTS_V1 {
            return Err(PlironTensorLayoutDataflowFailureV1::ResourceLimit);
        }
        sites.push(TensorDataflowSiteV1 {
            location: PlironTensorLayoutLocationV1 { block, operation },
            roots,
            contract,
        });
    }

    let mut facts = BTreeMap::new();
    let mut conflicted_roots = HashSet::new();
    let mut issues = Vec::new();
    for site in &sites {
        let fact = PlironTensorLayoutFactV1 {
            layout: site.contract.accumulator,
            subgroup_width: site.contract.subgroup_width,
            profile: site.contract.profile,
            producer: site.location,
        };
        match facts.get(&site.roots.result).copied() {
            None => {
                facts.insert(site.roots.result, fact);
            }
            Some(first)
                if first.layout == fact.layout && first.subgroup_width == fact.subgroup_width => {}
            Some(first) => {
                conflicted_roots.insert(site.roots.result);
                issues.push(PlironTensorLayoutDataflowIssueV1::MergeConflict {
                    root: site.roots.result,
                    first,
                    second: fact,
                });
            }
        }
        if facts.len() > MAX_PLIRON_TENSOR_DATAFLOW_ROOTS_V1
            || issues.len() > MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1
        {
            return Err(PlironTensorLayoutDataflowFailureV1::ResourceLimit);
        }
    }

    let mut edge_count = 0usize;
    for site in &sites {
        for (root, operand, expected) in [
            (site.roots.lhs, TensorOperandRoleV1::A, site.contract.a),
            (site.roots.rhs, TensorOperandRoleV1::B, site.contract.b),
            (
                site.roots.accumulator,
                TensorOperandRoleV1::Accumulator,
                site.contract.accumulator,
            ),
        ] {
            edge_count = edge_count.saturating_add(1);
            if edge_count > MAX_PLIRON_TENSOR_DATAFLOW_EDGES_V1 {
                return Err(PlironTensorLayoutDataflowFailureV1::ResourceLimit);
            }
            let Some(producer) = facts.get(&root).copied() else {
                continue;
            };
            if conflicted_roots.contains(&root) {
                continue;
            }
            if producer.layout != expected
                || producer.subgroup_width != site.contract.subgroup_width
            {
                issues.push(PlironTensorLayoutDataflowIssueV1::ConsumerMismatch {
                    root,
                    producer,
                    consumer: site.location,
                    consumer_profile: site.contract.profile,
                    operand,
                    expected,
                    expected_subgroup_width: site.contract.subgroup_width,
                });
            }
            if issues.len() > MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
                return Err(PlironTensorLayoutDataflowFailureV1::ResourceLimit);
            }
        }
    }

    Ok(PlironTensorLayoutDataflowAnalysisV1 {
        facts,
        conflicted_roots,
        issues,
        bound_sites: sites.len(),
    })
}

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
    Dataflow(Box<PlironTensorLayoutDataflowIssueV1>),
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
            | Self::DivergentSubgroupControl { .. }
            | Self::Dataflow(_) => KernelCheckStatusV1::Rejected,
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
            Self::Dataflow(issue) => display_dataflow_issue(formatter, issue),
            Self::ResourceLimitExceeded => formatter.write_str(
                "error[FE2O3-TENSOR-LAYOUT-003]: tensor layout analysis resource limit exceeded; help: split the kernel or reduce tensor/control-flow graph size so the bounded analysis can complete",
            ),
        }
    }
}

fn display_dataflow_issue(
    formatter: &mut fmt::Formatter<'_>,
    issue: &PlironTensorLayoutDataflowIssueV1,
) -> fmt::Result {
    match issue {
        PlironTensorLayoutDataflowIssueV1::MergeConflict {
            root,
            first,
            second,
        } => write!(
            formatter,
            "error[FE2O3-TENSOR-LAYOUT-004]: incompatible tensor layouts reach value root {} from block {} op {} ({}) and block {} op {} ({}); help: make every control-flow producer use the same fragment layout, or insert an explicit checked conversion before the join",
            display_root(*root),
            first.producer.block,
            first.producer.operation,
            describe_layout(*first),
            second.producer.block,
            second.producer.operation,
            describe_layout(*second),
        ),
        PlironTensorLayoutDataflowIssueV1::ConsumerMismatch {
            root,
            producer,
            consumer,
            consumer_profile,
            operand,
            expected,
            expected_subgroup_width,
        } => write!(
            formatter,
            "error[FE2O3-TENSOR-LAYOUT-005]: tensor value root {} is produced at block {} op {} as {}, but block {} op {} uses it as {operand:?} for profile {consumer_profile:?}, which requires {}; help: {}",
            display_root(*root),
            producer.producer.block,
            producer.producer.operation,
            describe_layout(*producer),
            consumer.block,
            consumer.operation,
            describe_fragment(*expected, *expected_subgroup_width),
            layout_mismatch_repair(*producer, *operand, *consumer_profile),
        ),
    }
}

fn display_root(root: [u64; 4]) -> String {
    format!(
        "{:016x}{:016x}{:016x}{:016x}",
        root[0], root[1], root[2], root[3]
    )
}

fn describe_layout(fact: PlironTensorLayoutFactV1) -> String {
    format!(
        "profile {:?}, {}",
        fact.profile,
        describe_fragment(fact.layout, fact.subgroup_width)
    )
}

fn describe_fragment(layout: TensorFragmentLayoutV1, subgroup_width: u16) -> String {
    format!(
        "{:?} {:?} {}x{} fragment with {} components across wave{}",
        layout.role,
        layout.element,
        layout.shape[0],
        layout.shape[1],
        layout.fragment_elements,
        subgroup_width,
    )
}

fn layout_mismatch_repair(
    producer: PlironTensorLayoutFactV1,
    operand: TensorOperandRoleV1,
    consumer_profile: TensorInstructionProfileV1,
) -> String {
    if operand == TensorOperandRoleV1::Accumulator {
        format!(
            "select a consumer instruction whose accumulator ABI accepts profile {:?}, or explicitly convert the accumulator before profile {consumer_profile:?}",
            producer.profile,
        )
    } else {
        format!(
            "insert a checked conversion/repack from the produced accumulator layout to the required {operand:?} fragment, or choose a consumer instruction whose {operand:?} ABI accepts profile {:?}",
            producer.profile,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironTensorLayoutReportV1 {
    findings: Vec<PlironTensorLayoutFindingV1>,
}

super::pliron_report_payload_receipt::impl_empty_findings_payload_v1!(PlironTensorLayoutReportV1);

impl PlironTensorLayoutReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::TensorLayout
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[PlironTensorLayoutFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
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

include!("pliron_tensor_layout/execution_v1.rs");
include!("pliron_tensor_layout/subgroup_convergence_v1.rs");
include!("pliron_tensor_layout/subgroup_uniformity_v1.rs");
include!("pliron_tensor_layout/control_flow_v1.rs");
include!("pliron_tensor_layout/resource_tests.rs");
