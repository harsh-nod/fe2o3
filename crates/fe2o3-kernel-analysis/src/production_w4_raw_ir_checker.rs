use std::collections::{HashMap, HashSet, VecDeque};

use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, ExecutionLayoutOp, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr,
};
use dialect_kernel::{
    AccessKindAttr, AnalysisSplitOp, AtomicOrderingAttr, AtomicScopeAttr, BranchArgsOp,
    DYNAMIC_EXTENT, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp, IndexEqualBranchArgsOp,
    IndexEqualBranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp, InvocationIndexOp,
    MemorySpaceAttr, OwnershipContractOp, PipelineCreateOp, PipelineEventKindAttr, PipelineEventOp,
    RankedAccessOp, RankedViewOp, SemanticBinaryKindAttr, SemanticBinaryOp, SemanticConstantOp,
    SemanticExpressionCommitmentOp, SemanticSymbolOp, TensorLayoutOp,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, PropertyAttr,
    RequireEffectRefinementOp,
};
use pliron::{
    builtin::ops::FuncOp,
    context::{Context, Ptr},
    operation::Operation,
    value::Value,
};
use sha2::{Digest, Sha256};

use super::{
    ProductionW4CounterexampleClassV1, ProductionW4CounterexampleLocationV1,
    ProductionW4IndependentObligationCheckerV1, ProductionW4LiveFunctionV1,
    ProductionW4RawIrFailureV1, ProductionW4RawIrFunctionEvidenceV1, ProductionW4RawIrReceiptV1,
    current_pliron_epoch, independent_obligation_checker_tag,
};
use crate::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::{
    KernelCheckStatusV1, PlironIrStructuralIdentityV1, ProductionCapabilityAnalysisKindV1,
    ProductionW4AnalysisObligationKindV1, derive_pliron_ir_structural_identity_v1,
};

const RAW_IR_EVIDENCE_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-W4/RAW-IR-EVIDENCE/V1\0";
const RAW_GRID_SCOPE_V1: u8 = 3;
const RAW_WORKGROUP_SCOPE_V1: u8 = 2;
const RAW_SUBGROUP_SCOPE_V1: u8 = 1;
pub(super) const MAX_PRODUCTION_W4_RAW_IR_WORK_UNITS_V1: usize = 1_048_576;
pub(super) const MAX_PRODUCTION_W4_RAW_IR_VALUE_DEPTH_V1: usize = 512;
pub(super) const PRODUCTION_W4_RAW_IR_RECEIPT_COUNT_V1: usize = 9;

pub(super) fn production_w4_raw_ir_checker_identity_v1() -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(RAW_IR_EVIDENCE_DOMAIN_V1);
    digest.update(MAX_PRODUCTION_W4_RAW_IR_WORK_UNITS_V1.to_le_bytes());
    digest.update(MAX_PRODUCTION_W4_RAW_IR_VALUE_DEPTH_V1.to_le_bytes());
    digest.update(PRODUCTION_W4_RAW_IR_RECEIPT_COUNT_V1.to_le_bytes());
    digest.finalize().into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawControlScopeV1 {
    rank: u8,
    known: bool,
}

impl RawControlScopeV1 {
    const GRID: Self = Self {
        rank: RAW_GRID_SCOPE_V1,
        known: true,
    };
    const LANE: Self = Self {
        rank: 0,
        known: true,
    };
    const UNKNOWN: Self = Self {
        rank: 0,
        known: false,
    };

    const fn meet(self, other: Self) -> Self {
        Self {
            rank: if self.rank < other.rank {
                self.rank
            } else {
                other.rank
            },
            known: self.known && other.known,
        }
    }
}

#[derive(Clone, Copy)]
struct RawLayoutV1 {
    global: [u64; 3],
    workgroup: [u64; 3],
    subgroup: u64,
}

#[derive(Clone, Copy)]
struct RawBarrierV1 {
    location: ProductionW4CounterexampleLocationV1,
    execution_scope: HierarchyAttr,
    memory_scope: MemoryScopeAttr,
    address_space: AddressSpaceAttr,
    order: MemoryOrderAttr,
}

#[derive(Clone)]
struct RawAccessV1 {
    location: ProductionW4CounterexampleLocationV1,
    view: Value,
    indices: Vec<Value>,
    extents: Vec<RawExtentV1>,
    kind: AccessKindAttr,
    memory_space: MemorySpaceAttr,
    allocation_origin: u64,
    noalias_class: u64,
    atomic_ordering: Option<AtomicOrderingAttr>,
    atomic_scope: Option<AtomicScopeAttr>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RawExtentV1 {
    Static(u64),
    Dynamic(Value),
}

#[derive(Clone, Copy)]
struct RawPipelineCreateV1 {
    location: ProductionW4CounterexampleLocationV1,
    pipeline: Value,
    view: Value,
}

#[derive(Clone, Copy)]
struct RawPipelineEventV1 {
    location: ProductionW4CounterexampleLocationV1,
    pipeline: Value,
    epoch: Value,
    slot: Value,
    kind: PipelineEventKindAttr,
}

#[derive(Clone)]
struct RawEffectContractV1 {
    location: ProductionW4CounterexampleLocationV1,
    obligation: [u64; 4],
    view: Value,
    indices: Vec<Value>,
    expression_pairs: Vec<(Value, Value)>,
}

#[derive(Clone, Copy)]
struct RawProofObligationV1 {
    location: ProductionW4CounterexampleLocationV1,
    identity: [u64; 4],
    subject: [u64; 4],
    model: [u64; 4],
    property: Option<PropertyAttr>,
}

#[derive(Clone, Copy)]
struct RawProofEvidenceV1 {
    location: ProductionW4CounterexampleLocationV1,
    obligation: [u64; 4],
    evidence: [u64; 4],
    property: Option<PropertyAttr>,
    status: Option<EvidenceStatusAttr>,
    boundary: Option<CoveredBoundaryAttr>,
}

struct ProductionW4RawIrModelV1 {
    inventory: BoundedPlironFunctionInventoryV1,
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<(usize, usize)>>,
    dominators: Vec<HashSet<usize>>,
    control_scope: Vec<Option<RawControlScopeV1>>,
    layout: Option<RawLayoutV1>,
    barriers: Vec<RawBarrierV1>,
    tensor_collectives: Vec<ProductionW4CounterexampleLocationV1>,
    accesses: Vec<RawAccessV1>,
    pipeline_creates: Vec<RawPipelineCreateV1>,
    pipeline_events: Vec<RawPipelineEventV1>,
    ownership_views: HashSet<Value>,
    effect_contracts: Vec<RawEffectContractV1>,
    proof_obligations: Vec<RawProofObligationV1>,
    proof_evidence: Vec<RawProofEvidenceV1>,
}

impl ProductionW4RawIrModelV1 {
    fn build(context: &Context, function: &FuncOp) -> Result<Self, ProductionW4RawIrFailureV1> {
        let inventory =
            BoundedPlironFunctionInventoryV1::collect(context, function).map_err(|failure| {
                raw_ir_incomplete(
                    ProductionW4AnalysisObligationKindV1::Uniformity,
                    ProductionCapabilityAnalysisKindV1::Uniformity,
                    ProductionW4CounterexampleClassV1::Uniformity,
                    None,
                    format!(
                        "raw-IR inventory exceeds the {} bound: {} > {}",
                        failure.resource(),
                        failure.actual(),
                        failure.limit()
                    ),
                )
            })?;
        if inventory.blocks().is_empty() {
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                None,
                "raw-IR function has no entry block".to_owned(),
            ));
        }
        let block_indices = inventory
            .blocks()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, block)| (block, index))
            .collect::<HashMap<_, _>>();
        let mut successors = vec![Vec::new(); inventory.blocks().len()];
        let mut predecessors = vec![Vec::new(); inventory.blocks().len()];
        for block in 0..inventory.blocks().len() {
            let Some(terminator) = inventory.block_operations(block).last() else {
                continue;
            };
            let raw = terminator.pointer().deref(context);
            for successor_index in 0..raw.get_num_successors() {
                let successor = raw.get_successor(successor_index);
                let Some(&target) = block_indices.get(&successor) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::Uniformity,
                        ProductionCapabilityAnalysisKindV1::Uniformity,
                        ProductionW4CounterexampleClassV1::Uniformity,
                        Some(raw_location(*terminator)),
                        "raw-IR control edge leaves the retained function".to_owned(),
                    ));
                };
                successors[block].push(target);
                predecessors[target].push((block, successor_index));
            }
        }
        let layout = raw_layout(context, &inventory)?;
        let value_scopes = raw_value_scopes(context, &inventory, &predecessors)?;
        let control_scope =
            raw_block_control_scopes(context, &inventory, &successors, &value_scopes, layout)?;
        let dominators = raw_dominators(&successors, &predecessors)?;

        let mut barriers = Vec::new();
        let mut tensor_collectives = Vec::new();
        let mut accesses = Vec::new();
        let mut pipeline_creates = Vec::new();
        let mut pipeline_events = Vec::new();
        let mut ownership_views = HashSet::new();
        let mut effect_contracts = Vec::new();
        let mut proof_obligations = Vec::new();
        let mut proof_evidence = Vec::new();
        for site in inventory.operations() {
            let operation = Operation::get_op_dyn(site.pointer(), context);
            let location = raw_location(*site);
            if let Some(barrier) = operation.downcast_ref::<BarrierOp>() {
                let Some(execution_scope) = barrier.execution_scope(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::BarrierOrder,
                        ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                        ProductionW4CounterexampleClassV1::BarrierOrder,
                        Some(location),
                        "raw-IR barrier has no typed execution scope".to_owned(),
                    ));
                };
                let Some(memory_scope) = barrier.memory_scope(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::MemoryVisibility,
                        ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                        ProductionW4CounterexampleClassV1::MemoryVisibility,
                        Some(location),
                        "raw-IR barrier has no typed memory scope".to_owned(),
                    ));
                };
                let Some(address_space) = barrier.address_space(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::MemoryVisibility,
                        ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                        ProductionW4CounterexampleClassV1::MemoryVisibility,
                        Some(location),
                        "raw-IR barrier has no typed address space".to_owned(),
                    ));
                };
                let Some(order) = barrier.order(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::MemoryVisibility,
                        ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                        ProductionW4CounterexampleClassV1::MemoryVisibility,
                        Some(location),
                        "raw-IR barrier has no typed memory order".to_owned(),
                    ));
                };
                barriers.push(RawBarrierV1 {
                    location,
                    execution_scope,
                    memory_scope,
                    address_space,
                    order,
                });
            } else if operation.downcast_ref::<TensorLayoutOp>().is_some() {
                tensor_collectives.push(location);
            } else if let Some(access) = operation.downcast_ref::<RankedAccessOp>() {
                let Some(kind) = access.kind(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::MemoryVisibility,
                        ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                        ProductionW4CounterexampleClassV1::MemoryVisibility,
                        Some(location),
                        "raw-IR access has no typed access kind".to_owned(),
                    ));
                };
                let view = access.view(context);
                let Some(view_definition) = view.defining_op() else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::MemoryBounds,
                        ProductionCapabilityAnalysisKindV1::MemoryBounds,
                        ProductionW4CounterexampleClassV1::MemoryBounds,
                        Some(location),
                        "raw-IR access view has no ranked definition".to_owned(),
                    ));
                };
                let view_definition = Operation::get_op_dyn(view_definition, context);
                let Some(view_operation) = view_definition.downcast_ref::<RankedViewOp>() else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::MemoryBounds,
                        ProductionCapabilityAnalysisKindV1::MemoryBounds,
                        ProductionW4CounterexampleClassV1::MemoryBounds,
                        Some(location),
                        "raw-IR access view is not defined by kernel.ranked_view".to_owned(),
                    ));
                };
                let Some(view_type) = view_operation.view_type(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::MemoryBounds,
                        ProductionCapabilityAnalysisKindV1::MemoryBounds,
                        ProductionW4CounterexampleClassV1::MemoryBounds,
                        Some(location),
                        "raw-IR access view has no typed shape".to_owned(),
                    ));
                };
                let shape = view_type.deref(context).shape().to_vec();
                let extents = shape
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(dimension, extent)| {
                        if extent == DYNAMIC_EXTENT {
                            view_operation
                                .dynamic_extent(context, dimension)
                                .map(RawExtentV1::Dynamic)
                        } else {
                            Some(RawExtentV1::Static(extent))
                        }
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(extents) = extents else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::MemoryBounds,
                        ProductionCapabilityAnalysisKindV1::MemoryBounds,
                        ProductionW4CounterexampleClassV1::MemoryBounds,
                        Some(location),
                        "raw-IR dynamic view extent is not bound to an SSA value".to_owned(),
                    ));
                };
                let Some(memory_space) = raw_view_memory_space(context, view) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::MemoryVisibility,
                        ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                        ProductionW4CounterexampleClassV1::MemoryVisibility,
                        Some(location),
                        "raw-IR access view has no ranked memory-space definition".to_owned(),
                    ));
                };
                accesses.push(RawAccessV1 {
                    location,
                    view,
                    indices: access.indices(context),
                    extents,
                    kind,
                    memory_space,
                    allocation_origin: view_operation.allocation_origin(context).unwrap_or(0),
                    noalias_class: view_operation.noalias_class(context).unwrap_or(0),
                    atomic_ordering: access.atomic_ordering(context),
                    atomic_scope: access.atomic_scope(context),
                });
            } else if let Some(create) = operation.downcast_ref::<PipelineCreateOp>() {
                pipeline_creates.push(RawPipelineCreateV1 {
                    location,
                    pipeline: create.pipeline(context),
                    view: create.view(context),
                });
            } else if let Some(event) = operation.downcast_ref::<PipelineEventOp>() {
                let Some(kind) = event.kind(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::WorkgroupMemoryEpochs,
                        ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs,
                        ProductionW4CounterexampleClassV1::PipelineEpochProtocol,
                        Some(location),
                        "raw-IR pipeline event has no typed transition".to_owned(),
                    ));
                };
                pipeline_events.push(RawPipelineEventV1 {
                    location,
                    pipeline: event.pipeline(context),
                    epoch: event.epoch(context),
                    slot: event.slot(context),
                    kind,
                });
            } else if let Some(ownership) = operation.downcast_ref::<OwnershipContractOp>() {
                ownership_views.insert(ownership.view(context));
            } else if let Some(contract) = operation.downcast_ref::<RequireEffectRefinementOp>() {
                let Some(obligation) = contract.obligation_id(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::EffectRefinement,
                        ProductionCapabilityAnalysisKindV1::EffectRefinement,
                        ProductionW4CounterexampleClassV1::EffectRefinement,
                        Some(location),
                        "raw-IR effect contract has no typed obligation identity".to_owned(),
                    ));
                };
                if raw_identity_is_zero(obligation) {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::EffectRefinement,
                        ProductionCapabilityAnalysisKindV1::EffectRefinement,
                        ProductionW4CounterexampleClassV1::EffectRefinement,
                        Some(location),
                        "raw-IR effect contract has the reserved zero obligation identity"
                            .to_owned(),
                    ));
                }
                let mut expression_pairs = contract
                    .gpu_coordinates(context)
                    .into_iter()
                    .zip(contract.reference_coordinates(context))
                    .collect::<Vec<_>>();
                expression_pairs.extend([
                    (
                        contract.gpu_domain(context),
                        contract.reference_domain(context),
                    ),
                    (
                        contract.gpu_precondition(context),
                        contract.reference_precondition(context),
                    ),
                    (
                        contract.gpu_value(context),
                        contract.reference_value(context),
                    ),
                ]);
                effect_contracts.push(RawEffectContractV1 {
                    location,
                    obligation,
                    view: contract.view(context),
                    indices: contract.indices(context),
                    expression_pairs,
                });
            } else if let Some(obligation) = operation.downcast_ref::<ObligationOp>() {
                let Some(identity) = obligation.obligation_id(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::EffectRefinement,
                        ProductionCapabilityAnalysisKindV1::EffectRefinement,
                        ProductionW4CounterexampleClassV1::EffectRefinement,
                        Some(location),
                        "raw-IR proof obligation has no typed identity".to_owned(),
                    ));
                };
                let Some(subject) = obligation.subject_id(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::EffectRefinement,
                        ProductionCapabilityAnalysisKindV1::EffectRefinement,
                        ProductionW4CounterexampleClassV1::EffectRefinement,
                        Some(location),
                        "raw-IR proof obligation has no subject identity".to_owned(),
                    ));
                };
                let Some(reference_model) = obligation.model_id(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::EffectRefinement,
                        ProductionCapabilityAnalysisKindV1::EffectRefinement,
                        ProductionW4CounterexampleClassV1::EffectRefinement,
                        Some(location),
                        "raw-IR proof obligation has no reference-model identity".to_owned(),
                    ));
                };
                if [identity, subject, reference_model]
                    .into_iter()
                    .any(raw_identity_is_zero)
                {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::EffectRefinement,
                        ProductionCapabilityAnalysisKindV1::EffectRefinement,
                        ProductionW4CounterexampleClassV1::EffectRefinement,
                        Some(location),
                        "raw-IR proof obligation uses a reserved zero identity".to_owned(),
                    ));
                }
                proof_obligations.push(RawProofObligationV1 {
                    location,
                    identity,
                    subject,
                    model: reference_model,
                    property: obligation.property(context),
                });
            } else if let Some(evidence) = operation.downcast_ref::<EvidenceRefOp>() {
                let Some(obligation) = evidence.obligation_id(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::EffectRefinement,
                        ProductionCapabilityAnalysisKindV1::EffectRefinement,
                        ProductionW4CounterexampleClassV1::EffectRefinement,
                        Some(location),
                        "raw-IR proof evidence has no obligation identity".to_owned(),
                    ));
                };
                let Some(evidence_identity) = evidence.evidence_id(context) else {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::EffectRefinement,
                        ProductionCapabilityAnalysisKindV1::EffectRefinement,
                        ProductionW4CounterexampleClassV1::EffectRefinement,
                        Some(location),
                        "raw-IR proof evidence has no evidence identity".to_owned(),
                    ));
                };
                if raw_identity_is_zero(obligation) || raw_identity_is_zero(evidence_identity) {
                    return Err(raw_ir_incomplete(
                        ProductionW4AnalysisObligationKindV1::EffectRefinement,
                        ProductionCapabilityAnalysisKindV1::EffectRefinement,
                        ProductionW4CounterexampleClassV1::EffectRefinement,
                        Some(location),
                        "raw-IR proof evidence uses a reserved zero identity".to_owned(),
                    ));
                }
                proof_evidence.push(RawProofEvidenceV1 {
                    location,
                    obligation,
                    evidence: evidence_identity,
                    property: evidence.property(context),
                    status: evidence.status(context),
                    boundary: evidence.covered_boundary(context),
                });
            }
        }
        Ok(Self {
            inventory,
            successors,
            predecessors,
            dominators,
            control_scope,
            layout,
            barriers,
            tensor_collectives,
            accesses,
            pipeline_creates,
            pipeline_events,
            ownership_views,
            effect_contracts,
            proof_obligations,
            proof_evidence,
        })
    }
}

const fn raw_identity_is_zero(identity: [u64; 4]) -> bool {
    identity[0] == 0 && identity[1] == 0 && identity[2] == 0 && identity[3] == 0
}

fn raw_location(
    site: crate::pliron_function_inventory::PlironOperationSiteV1,
) -> ProductionW4CounterexampleLocationV1 {
    ProductionW4CounterexampleLocationV1 {
        block: site.block(),
        operation: site.operation(),
    }
}

fn raw_view_memory_space(context: &Context, view: Value) -> Option<MemorySpaceAttr> {
    let definition = view.defining_op()?;
    Operation::get_op_dyn(definition, context)
        .downcast_ref::<RankedViewOp>()
        .and_then(|view| view.memory_space(context))
}

fn raw_layout(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<Option<RawLayoutV1>, ProductionW4RawIrFailureV1> {
    let mut layout = None;
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(candidate) = operation.downcast_ref::<ExecutionLayoutOp>() else {
            continue;
        };
        if layout.is_some() {
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                Some(raw_location(*site)),
                "raw-IR function has more than one execution layout".to_owned(),
            ));
        }
        let Some(global) = candidate.global_extents(context) else {
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                Some(raw_location(*site)),
                "raw-IR execution layout has no global extents".to_owned(),
            ));
        };
        let Some(workgroup) = candidate.workgroup_extents(context) else {
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                Some(raw_location(*site)),
                "raw-IR execution layout has no workgroup extents".to_owned(),
            ));
        };
        let Some(subgroup) = candidate.subgroup_size(context) else {
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                Some(raw_location(*site)),
                "raw-IR execution layout has no subgroup size".to_owned(),
            ));
        };
        layout = Some(RawLayoutV1 {
            global,
            workgroup,
            subgroup,
        });
    }
    Ok(layout)
}

fn raw_value_scopes(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    predecessors: &[Vec<(usize, usize)>],
) -> Result<HashMap<Value, RawControlScopeV1>, ProductionW4RawIrFailureV1> {
    let mut scopes = HashMap::new();
    for block in inventory.blocks() {
        let raw = block.deref(context);
        for index in 0..raw.get_num_arguments() {
            scopes.insert(raw.get_argument(index), RawControlScopeV1::GRID);
        }
    }
    for site in inventory.operations() {
        let raw = site.pointer().deref(context);
        for index in 0..raw.get_num_results() {
            scopes.insert(raw.get_result(index), RawControlScopeV1::GRID);
        }
    }

    let iteration_limit = inventory
        .blocks()
        .len()
        .saturating_add(inventory.operations().len())
        .saturating_mul(4)
        .saturating_add(1);
    let mut work = 0_usize;
    for _ in 0..iteration_limit {
        let mut changed = false;
        for (block_index, block) in inventory.blocks().iter().copied().enumerate() {
            raw_charge_work(
                &mut work,
                1,
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                None,
                "raw-IR value-scope fixed point",
            )?;
            if block_index == 0 {
                continue;
            }
            let raw_block = block.deref(context);
            for argument_index in 0..raw_block.get_num_arguments() {
                let mut scope = RawControlScopeV1::GRID;
                let mut complete = true;
                for &(predecessor, successor_index) in &predecessors[block_index] {
                    let Some(terminator) = inventory.block_operations(predecessor).last() else {
                        complete = false;
                        break;
                    };
                    let Some(arguments) =
                        raw_edge_arguments(context, terminator.pointer(), successor_index)
                    else {
                        complete = false;
                        break;
                    };
                    let Some(value) = arguments.get(argument_index) else {
                        complete = false;
                        break;
                    };
                    scope = scope.meet(
                        scopes
                            .get(value)
                            .copied()
                            .unwrap_or(RawControlScopeV1::UNKNOWN),
                    );
                }
                if !complete || predecessors[block_index].is_empty() {
                    scope = RawControlScopeV1::UNKNOWN;
                }
                let argument = raw_block.get_argument(argument_index);
                if scopes.get(&argument).copied() != Some(scope) {
                    scopes.insert(argument, scope);
                    changed = true;
                }
            }
        }
        for site in inventory.operations() {
            raw_charge_work(
                &mut work,
                1,
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                Some(raw_location(*site)),
                "raw-IR value-scope fixed point",
            )?;
            let operation = Operation::get_op_dyn(site.pointer(), context);
            let raw = site.pointer().deref(context);
            let result_scope = if operation.downcast_ref::<IndexConstantOp>().is_some() {
                RawControlScopeV1::GRID
            } else if operation.downcast_ref::<InvocationIndexOp>().is_some() {
                RawControlScopeV1::LANE
            } else if operation.downcast_ref::<IndexBinaryOp>().is_some() {
                raw.operands()
                    .fold(RawControlScopeV1::GRID, |scope, value| {
                        scope.meet(
                            scopes
                                .get(&value)
                                .copied()
                                .unwrap_or(RawControlScopeV1::UNKNOWN),
                        )
                    })
            } else {
                RawControlScopeV1::UNKNOWN
            };
            for index in 0..raw.get_num_results() {
                let result = raw.get_result(index);
                if scopes.get(&result).copied() != Some(result_scope) {
                    scopes.insert(result, result_scope);
                    changed = true;
                }
            }
        }
        if !changed {
            return Ok(scopes);
        }
    }
    Err(raw_ir_incomplete(
        ProductionW4AnalysisObligationKindV1::Uniformity,
        ProductionCapabilityAnalysisKindV1::Uniformity,
        ProductionW4CounterexampleClassV1::Uniformity,
        None,
        "raw-IR value-scope analysis did not reach a bounded fixed point".to_owned(),
    ))
}

fn raw_edge_arguments(
    context: &Context,
    operation: Ptr<Operation>,
    successor: usize,
) -> Option<Vec<Value>> {
    let dynamic = Operation::get_op_dyn(operation, context);
    if let Some(branch) = dynamic.downcast_ref::<BranchArgsOp>() {
        return (successor == 0).then(|| branch.arguments(context));
    }
    if let Some(branch) = dynamic.downcast_ref::<IndexLessThanBranchArgsOp>() {
        return match successor {
            0 => Some(branch.true_arguments(context)),
            1 => Some(branch.false_arguments(context)),
            _ => None,
        };
    }
    if let Some(branch) = dynamic.downcast_ref::<IndexEqualBranchArgsOp>() {
        return match successor {
            0 => Some(branch.true_arguments(context)),
            1 => Some(branch.false_arguments(context)),
            _ => None,
        };
    }
    if let Some(split) = dynamic.downcast_ref::<AnalysisSplitOp>() {
        return match successor {
            0 => Some(split.first_arguments(context)),
            1 => Some(split.second_arguments(context)),
            _ => None,
        };
    }
    let raw = operation.deref(context);
    (successor < raw.get_num_successors()
        && raw
            .get_successor(successor)
            .deref(context)
            .get_num_arguments()
            == 0)
        .then(Vec::new)
}

fn raw_branch_scope(
    context: &Context,
    operation: Ptr<Operation>,
    value_scopes: &HashMap<Value, RawControlScopeV1>,
    layout: Option<RawLayoutV1>,
) -> RawControlScopeV1 {
    let raw = operation.deref(context);
    if raw.get_num_successors() <= 1 {
        return RawControlScopeV1::GRID;
    }
    let dynamic = Operation::get_op_dyn(operation, context);
    let operands = if let Some(branch) = dynamic.downcast_ref::<IndexLessThanBranchOp>() {
        Some((branch.lhs(context), branch.rhs(context), true))
    } else if let Some(branch) = dynamic.downcast_ref::<IndexLessThanBranchArgsOp>() {
        Some((branch.lhs(context), branch.rhs(context), true))
    } else if let Some(branch) = dynamic.downcast_ref::<IndexEqualBranchOp>() {
        Some((branch.lhs(context), branch.rhs(context), false))
    } else if let Some(branch) = dynamic.downcast_ref::<IndexEqualBranchArgsOp>() {
        Some((branch.lhs(context), branch.rhs(context), false))
    } else {
        None
    };
    if let Some((lhs, rhs, ordered)) = operands {
        let scope = value_scopes
            .get(&lhs)
            .copied()
            .unwrap_or(RawControlScopeV1::UNKNOWN)
            .meet(
                value_scopes
                    .get(&rhs)
                    .copied()
                    .unwrap_or(RawControlScopeV1::UNKNOWN),
            );
        if scope == RawControlScopeV1::GRID {
            return scope;
        }
        if ordered {
            if let Some(scope) = raw_aligned_invocation_partition(context, lhs, rhs, layout) {
                return scope;
            }
            if let Some(scope) = raw_aligned_invocation_partition(context, rhs, lhs, layout) {
                return scope;
            }
        }
        return scope;
    }
    if let Some(split) = dynamic.downcast_ref::<AnalysisSplitOp>() {
        let controls = split.control_dependencies(context);
        if controls.is_empty() {
            return RawControlScopeV1::UNKNOWN;
        }
        return controls
            .iter()
            .fold(RawControlScopeV1::GRID, |scope, value| {
                scope.meet(
                    value_scopes
                        .get(value)
                        .copied()
                        .unwrap_or(RawControlScopeV1::UNKNOWN),
                )
            });
    }
    RawControlScopeV1::UNKNOWN
}

fn raw_aligned_invocation_partition(
    context: &Context,
    invocation: Value,
    boundary: Value,
    layout: Option<RawLayoutV1>,
) -> Option<RawControlScopeV1> {
    let invocation = invocation
        .defining_op()
        .map(|operation| Operation::get_op_dyn(operation, context))?
        .downcast_ref::<InvocationIndexOp>()
        .copied()?;
    let boundary = boundary
        .defining_op()
        .map(|operation| Operation::get_op_dyn(operation, context))?
        .downcast_ref::<IndexConstantOp>()
        .and_then(|constant| constant.value(context))?;
    let layout = layout?;
    let dimension = usize::try_from(invocation.dimension(context)?).ok()?;
    let launch_extent = invocation.launch_extent(context)?;
    if boundary == 0 || boundary >= launch_extent {
        return Some(RawControlScopeV1::GRID);
    }
    let workgroup = *layout.workgroup.get(dimension)?;
    if workgroup != 0 && boundary % workgroup == 0 {
        return Some(RawControlScopeV1 {
            rank: RAW_WORKGROUP_SCOPE_V1,
            known: true,
        });
    }
    if dimension == 0 && layout.subgroup != 0 && boundary % layout.subgroup == 0 {
        return Some(RawControlScopeV1 {
            rank: RAW_SUBGROUP_SCOPE_V1,
            known: true,
        });
    }
    Some(RawControlScopeV1::LANE)
}

fn raw_definite_divergent_controller(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    required: u8,
    location: ProductionW4CounterexampleLocationV1,
) -> Option<ProductionW4CounterexampleLocationV1> {
    model
        .dominators
        .get(location.block)?
        .iter()
        .find_map(|&block| {
            let terminator = model.inventory.block_operations(block).last()?;
            let successors = model.successors.get(block)?;
            if successors.len() != 2 || successors[0] == successors[1] {
                return None;
            }
            let target_dominators = model.dominators.get(location.block)?;
            if successors
                .iter()
                .filter(|successor| target_dominators.contains(successor))
                .count()
                != 1
            {
                return None;
            }
            let operation = Operation::get_op_dyn(terminator.pointer(), context);
            let (lhs, rhs, equality) = operation
                .downcast_ref::<IndexLessThanBranchOp>()
                .map(|branch| (branch.lhs(context), branch.rhs(context), false))
                .or_else(|| {
                    operation
                        .downcast_ref::<IndexLessThanBranchArgsOp>()
                        .map(|branch| (branch.lhs(context), branch.rhs(context), false))
                })
                .or_else(|| {
                    operation
                        .downcast_ref::<IndexEqualBranchOp>()
                        .map(|branch| (branch.lhs(context), branch.rhs(context), true))
                })
                .or_else(|| {
                    operation
                        .downcast_ref::<IndexEqualBranchArgsOp>()
                        .map(|branch| (branch.lhs(context), branch.rhs(context), true))
                })?;
            let (invocation, boundary, invocation_is_lhs) =
                raw_invocation_boundary(context, lhs, rhs)
                    .map(|(invocation, boundary)| (invocation, boundary, true))
                    .or_else(|| {
                        raw_invocation_boundary(context, rhs, lhs)
                            .map(|(invocation, boundary)| (invocation, boundary, false))
                    })?;
            let dimension = usize::try_from(invocation.dimension(context)?).ok()?;
            let extent = invocation.launch_extent(context)?;
            let split = if equality {
                boundary
            } else if invocation_is_lhs {
                boundary
            } else {
                boundary.checked_add(1)?
            };
            raw_partition_definitely_diverges(
                model.layout,
                dimension,
                extent,
                split,
                required,
                equality,
            )
            .then(|| raw_location(*terminator))
        })
}

fn raw_invocation_boundary(
    context: &Context,
    invocation: Value,
    boundary: Value,
) -> Option<(InvocationIndexOp, u64)> {
    let invocation = invocation
        .defining_op()
        .map(|operation| Operation::get_op_dyn(operation, context))?
        .downcast_ref::<InvocationIndexOp>()
        .copied()?;
    Some((invocation, raw_constant(context, boundary)?))
}

fn raw_partition_definitely_diverges(
    layout: Option<RawLayoutV1>,
    dimension: usize,
    extent: u64,
    split: u64,
    required: u8,
    equality: bool,
) -> bool {
    if extent == 0 || split >= extent || required == 0 {
        return false;
    }
    if required == RAW_GRID_SCOPE_V1 {
        return if equality { extent > 1 } else { split != 0 };
    }
    let Some(layout) = layout else {
        return false;
    };
    let width = if required == RAW_WORKGROUP_SCOPE_V1 {
        layout.workgroup.get(dimension).copied().unwrap_or(0)
    } else if required == RAW_SUBGROUP_SCOPE_V1 && dimension == 0 {
        layout.subgroup
    } else {
        0
    };
    if width <= 1 {
        return false;
    }
    let group_start = split / width * width;
    let active = extent.saturating_sub(group_start).min(width);
    if equality {
        active > 1
    } else {
        split > group_start && split < group_start.saturating_add(active)
    }
}

fn raw_block_control_scopes(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    successors: &[Vec<usize>],
    value_scopes: &HashMap<Value, RawControlScopeV1>,
    layout: Option<RawLayoutV1>,
) -> Result<Vec<Option<RawControlScopeV1>>, ProductionW4RawIrFailureV1> {
    let mut scopes = vec![None; inventory.blocks().len()];
    scopes[0] = Some(RawControlScopeV1::GRID);
    let iteration_limit = inventory.blocks().len().saturating_mul(4).saturating_add(1);
    let mut work = 0_usize;
    for _ in 0..iteration_limit {
        let mut changed = false;
        for block in 0..inventory.blocks().len() {
            raw_charge_work(
                &mut work,
                successors[block].len().saturating_add(1),
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                inventory
                    .block_operations(block)
                    .last()
                    .copied()
                    .map(raw_location),
                "raw-IR control-scope fixed point",
            )?;
            let Some(source_scope) = scopes[block] else {
                continue;
            };
            let branch_scope = inventory
                .block_operations(block)
                .last()
                .map_or(RawControlScopeV1::GRID, |terminator| {
                    raw_branch_scope(context, terminator.pointer(), value_scopes, layout)
                });
            let edge_scope = source_scope.meet(branch_scope);
            for &successor in &successors[block] {
                let joined = scopes[successor].map_or(edge_scope, |prior| prior.meet(edge_scope));
                if scopes[successor] != Some(joined) {
                    scopes[successor] = Some(joined);
                    changed = true;
                }
            }
        }
        if !changed {
            return Ok(scopes);
        }
    }
    Err(raw_ir_incomplete(
        ProductionW4AnalysisObligationKindV1::Uniformity,
        ProductionCapabilityAnalysisKindV1::Uniformity,
        ProductionW4CounterexampleClassV1::Uniformity,
        None,
        "raw-IR control-scope analysis did not reach a bounded fixed point".to_owned(),
    ))
}

fn raw_dominators(
    successors: &[Vec<usize>],
    predecessors: &[Vec<(usize, usize)>],
) -> Result<Vec<HashSet<usize>>, ProductionW4RawIrFailureV1> {
    let dense_storage = successors
        .len()
        .checked_mul(successors.len())
        .ok_or_else(|| raw_dominator_resource_failure("storage size overflow"))?;
    if dense_storage > MAX_PRODUCTION_W4_RAW_IR_WORK_UNITS_V1 {
        return Err(raw_dominator_resource_failure(
            "dominator relation exceeds its bounded storage budget",
        ));
    }
    let mut reachable = HashSet::from([0]);
    let mut queue = VecDeque::from([0]);
    let mut work = 0_usize;
    while let Some(block) = queue.pop_front() {
        raw_charge_work(
            &mut work,
            successors[block].len().saturating_add(1),
            ProductionW4AnalysisObligationKindV1::HappensBefore,
            ProductionCapabilityAnalysisKindV1::RaceFreedom,
            ProductionW4CounterexampleClassV1::MemoryVisibility,
            None,
            "raw-IR CFG reachability",
        )?;
        for &successor in &successors[block] {
            if reachable.insert(successor) {
                queue.push_back(successor);
            }
        }
    }
    let all = reachable.clone();
    let mut dominators = (0..successors.len())
        .map(|block| {
            if block == 0 {
                HashSet::from([0])
            } else if reachable.contains(&block) {
                all.clone()
            } else {
                HashSet::new()
            }
        })
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for block in 1..successors.len() {
            if !reachable.contains(&block) {
                continue;
            }
            let mut incoming = predecessors[block]
                .iter()
                .filter(|(predecessor, _)| reachable.contains(predecessor));
            let Some((first, _)) = incoming.next() else {
                continue;
            };
            let mut next = dominators[*first].clone();
            for (predecessor, _) in incoming {
                raw_charge_work(
                    &mut work,
                    next.len().saturating_add(1),
                    ProductionW4AnalysisObligationKindV1::HappensBefore,
                    ProductionCapabilityAnalysisKindV1::RaceFreedom,
                    ProductionW4CounterexampleClassV1::MemoryVisibility,
                    None,
                    "raw-IR dominator fixed point",
                )?;
                next.retain(|candidate| dominators[*predecessor].contains(candidate));
            }
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
        if !changed {
            return Ok(dominators);
        }
    }
}

fn raw_dominator_resource_failure(detail: &str) -> ProductionW4RawIrFailureV1 {
    raw_ir_incomplete(
        ProductionW4AnalysisObligationKindV1::HappensBefore,
        ProductionCapabilityAnalysisKindV1::RaceFreedom,
        ProductionW4CounterexampleClassV1::MemoryVisibility,
        None,
        format!("raw-IR {detail}"),
    )
}

#[allow(clippy::too_many_arguments)]
fn raw_charge_work(
    consumed: &mut usize,
    additional: usize,
    obligation: ProductionW4AnalysisObligationKindV1,
    stage: ProductionCapabilityAnalysisKindV1,
    class: ProductionW4CounterexampleClassV1,
    location: Option<ProductionW4CounterexampleLocationV1>,
    analysis: &str,
) -> Result<(), ProductionW4RawIrFailureV1> {
    *consumed = consumed.checked_add(additional).ok_or_else(|| {
        raw_ir_incomplete(
            obligation,
            stage,
            class,
            location,
            format!("{analysis} work counter overflowed"),
        )
    })?;
    if *consumed > MAX_PRODUCTION_W4_RAW_IR_WORK_UNITS_V1 {
        return Err(raw_ir_incomplete(
            obligation,
            stage,
            class,
            location,
            format!(
                "{analysis} exceeds the {} work-unit budget",
                MAX_PRODUCTION_W4_RAW_IR_WORK_UNITS_V1
            ),
        ));
    }
    Ok(())
}

struct RawFactsV1 {
    digest: Sha256,
    checked_units: usize,
}

impl RawFactsV1 {
    fn new(
        checker: ProductionW4IndependentObligationCheckerV1,
        structural: &PlironIrStructuralIdentityV1,
        pliron_epoch: u64,
    ) -> Self {
        let mut digest = Sha256::new();
        digest.update(RAW_IR_EVIDENCE_DOMAIN_V1);
        digest.update([independent_obligation_checker_tag(checker)]);
        digest.update(structural.sha256());
        digest.update(structural.canonical_bytes_len().to_le_bytes());
        digest.update(pliron_epoch.to_le_bytes());
        Self {
            digest,
            checked_units: 0,
        }
    }

    fn fact(&mut self, values: &[u64]) {
        self.checked_units = self.checked_units.saturating_add(1);
        self.digest.update((values.len() as u64).to_le_bytes());
        for value in values {
            self.digest.update(value.to_le_bytes());
        }
    }

    fn finish(
        self,
        checker: ProductionW4IndependentObligationCheckerV1,
    ) -> ProductionW4RawIrReceiptV1 {
        ProductionW4RawIrReceiptV1 {
            checker,
            checked_units: self.checked_units,
            facts_identity: self.digest.finalize().into(),
        }
    }
}

fn raw_required_scope(scope: HierarchyAttr) -> u8 {
    match scope {
        HierarchyAttr::Grid => RAW_GRID_SCOPE_V1,
        HierarchyAttr::Workgroup => RAW_WORKGROUP_SCOPE_V1,
        HierarchyAttr::Subgroup => RAW_SUBGROUP_SCOPE_V1,
        HierarchyAttr::Lane => 0,
    }
}

#[derive(Clone, Copy)]
struct RawUpperBoundV1 {
    maximum: u64,
    is_constant: bool,
}

fn raw_constant(context: &Context, value: Value) -> Option<u64> {
    let definition = value.defining_op()?;
    Operation::get_op_dyn(definition, context)
        .downcast_ref::<IndexConstantOp>()
        .and_then(|constant| constant.value(context))
}

fn raw_single_predecessor_alias(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    value: Value,
) -> Option<Value> {
    for (block_index, block) in model.inventory.blocks().iter().copied().enumerate() {
        let raw_block = block.deref(context);
        for argument in 0..raw_block.get_num_arguments() {
            if raw_block.get_argument(argument) != value {
                continue;
            }
            let [(predecessor, successor)] = model.predecessors[block_index].as_slice() else {
                return None;
            };
            return raw_edge_arguments(
                context,
                model
                    .inventory
                    .block_operations(*predecessor)
                    .last()?
                    .pointer(),
                *successor,
            )?
            .get(argument)
            .copied();
        }
    }
    None
}

fn raw_values_equivalent(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    lhs: Value,
    rhs: Value,
    budget: usize,
) -> bool {
    if lhs == rhs {
        return true;
    }
    if budget == 0 {
        return false;
    }
    if let (Some(lhs), Some(rhs)) = (raw_constant(context, lhs), raw_constant(context, rhs)) {
        return lhs == rhs;
    }
    raw_single_predecessor_alias(context, model, lhs)
        .is_some_and(|alias| raw_values_equivalent(context, model, alias, rhs, budget - 1))
        || raw_single_predecessor_alias(context, model, rhs)
            .is_some_and(|alias| raw_values_equivalent(context, model, lhs, alias, budget - 1))
}

fn raw_upper_bound(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    value: Value,
    visiting: &mut HashSet<Value>,
    budget: usize,
) -> Option<RawUpperBoundV1> {
    if budget == 0 || !visiting.insert(value) {
        return None;
    }
    let result = if let Some(constant) = raw_constant(context, value) {
        Some(RawUpperBoundV1 {
            maximum: constant,
            is_constant: true,
        })
    } else if let Some(alias) = raw_single_predecessor_alias(context, model, value) {
        raw_upper_bound(context, model, alias, visiting, budget - 1)
    } else if let Some(definition) = value.defining_op() {
        let operation = Operation::get_op_dyn(definition, context);
        if let Some(invocation) = operation.downcast_ref::<InvocationIndexOp>() {
            invocation.launch_extent(context).and_then(|extent| {
                extent.checked_sub(1).map(|maximum| RawUpperBoundV1 {
                    maximum,
                    is_constant: false,
                })
            })
        } else if let Some(binary) = operation.downcast_ref::<IndexBinaryOp>() {
            let rhs = raw_upper_bound(context, model, binary.rhs(context), visiting, budget - 1);
            match binary.kind(context)? {
                IndexBinaryKindAttr::Remainder => rhs.and_then(|rhs| {
                    (rhs.is_constant && rhs.maximum != 0).then(|| RawUpperBoundV1 {
                        maximum: rhs.maximum - 1,
                        is_constant: false,
                    })
                }),
                kind => {
                    let lhs =
                        raw_upper_bound(context, model, binary.lhs(context), visiting, budget - 1);
                    lhs.zip(rhs).and_then(|(lhs, rhs)| {
                        let maximum = match kind {
                            IndexBinaryKindAttr::Add => lhs.maximum.checked_add(rhs.maximum),
                            IndexBinaryKindAttr::Multiply => lhs.maximum.checked_mul(rhs.maximum),
                            IndexBinaryKindAttr::Divide => (rhs.is_constant && rhs.maximum != 0)
                                .then(|| lhs.maximum / rhs.maximum),
                            IndexBinaryKindAttr::Remainder => unreachable!(),
                        }?;
                        Some(RawUpperBoundV1 {
                            maximum,
                            is_constant: lhs.is_constant && rhs.is_constant,
                        })
                    })
                }
            }
        } else {
            None
        }
    } else {
        None
    };
    visiting.remove(&value);
    result
}

fn raw_extent_constant(context: &Context, extent: RawExtentV1) -> Option<u64> {
    match extent {
        RawExtentV1::Static(extent) => Some(extent),
        RawExtentV1::Dynamic(value) => raw_constant(context, value),
    }
}

fn raw_extent_matches(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    value: Value,
    extent: RawExtentV1,
    budget: usize,
) -> bool {
    match extent {
        RawExtentV1::Static(extent) => raw_constant(context, value) == Some(extent),
        RawExtentV1::Dynamic(extent) => {
            raw_values_equivalent(context, model, value, extent, budget)
        }
    }
}

fn raw_dominating_guard_proves_bound(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    access_block: usize,
    index: Value,
    extent: RawExtentV1,
) -> bool {
    let budget = model
        .inventory
        .blocks()
        .len()
        .saturating_add(model.inventory.operations().len())
        .saturating_add(1)
        .min(MAX_PRODUCTION_W4_RAW_IR_VALUE_DEPTH_V1);
    model.dominators[access_block]
        .iter()
        .copied()
        .any(|guard_block| {
            let Some(terminator) = model.inventory.block_operations(guard_block).last() else {
                return false;
            };
            let operation = Operation::get_op_dyn(terminator.pointer(), context);
            let operands = operation
                .downcast_ref::<IndexLessThanBranchOp>()
                .map(|branch| (branch.lhs(context), branch.rhs(context)))
                .or_else(|| {
                    operation
                        .downcast_ref::<IndexLessThanBranchArgsOp>()
                        .map(|branch| (branch.lhs(context), branch.rhs(context)))
                });
            let Some((lhs, rhs)) = operands else {
                return false;
            };
            let [true_block, false_block] = model.successors[guard_block].as_slice() else {
                return false;
            };
            if true_block == false_block {
                return false;
            }
            model.dominators[access_block].contains(true_block)
                && raw_values_equivalent(context, model, lhs, index, budget)
                && raw_extent_matches(context, model, rhs, extent, budget)
        })
}

fn raw_memory_bounds_receipt(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrReceiptV1, ProductionW4RawIrFailureV1> {
    let checker = ProductionW4IndependentObligationCheckerV1::MemoryBoundsFreshLiveIrV1;
    let mut facts = RawFactsV1::new(checker, structural, pliron_epoch);
    let budget = model
        .inventory
        .blocks()
        .len()
        .saturating_add(model.inventory.operations().len())
        .saturating_add(1)
        .min(MAX_PRODUCTION_W4_RAW_IR_VALUE_DEPTH_V1);
    for access in &model.accesses {
        if access.indices.len() != access.extents.len() {
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::MemoryBounds,
                ProductionCapabilityAnalysisKindV1::MemoryBounds,
                ProductionW4CounterexampleClassV1::MemoryBounds,
                Some(access.location),
                "raw-IR access rank does not match its typed view rank".to_owned(),
            ));
        }
        for (dimension, (&index, &extent)) in access.indices.iter().zip(&access.extents).enumerate()
        {
            let upper = raw_upper_bound(context, model, index, &mut HashSet::new(), budget);
            let static_extent = raw_extent_constant(context, extent);
            if upper
                .zip(static_extent)
                .is_some_and(|(upper, extent)| upper.maximum < extent)
                || raw_dominating_guard_proves_bound(
                    context,
                    model,
                    access.location.block,
                    index,
                    extent,
                )
            {
                facts.fact(&[
                    access.location.block as u64,
                    access.location.operation as u64,
                    dimension as u64,
                    upper.map_or(u64::MAX, |upper| upper.maximum),
                    static_extent.unwrap_or(u64::MAX),
                ]);
                continue;
            }
            if let (Some(upper), Some(extent)) = (upper, static_extent)
                && upper.is_constant
                && upper.maximum >= extent
            {
                return Err(raw_ir_rejected(
                    ProductionW4AnalysisObligationKindV1::MemoryBounds,
                    ProductionCapabilityAnalysisKindV1::MemoryBounds,
                    ProductionW4CounterexampleClassV1::MemoryBounds,
                    Some(access.location),
                    None,
                    format!(
                        "raw-IR static index {} is outside extent {extent} in dimension {dimension}",
                        upper.maximum
                    ),
                ));
            }
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::MemoryBounds,
                ProductionCapabilityAnalysisKindV1::MemoryBounds,
                ProductionW4CounterexampleClassV1::MemoryBounds,
                Some(access.location),
                format!(
                    "raw-IR replay cannot prove access dimension {dimension} is within its typed extent"
                ),
            ));
        }
    }
    Ok(facts.finish(checker))
}

fn raw_uniformity_receipt(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrReceiptV1, ProductionW4RawIrFailureV1> {
    let checker = ProductionW4IndependentObligationCheckerV1::UniformityFreshLiveIrV1;
    let mut facts = RawFactsV1::new(checker, structural, pliron_epoch);
    for barrier in &model.barriers {
        let control = model.control_scope[barrier.location.block];
        raw_require_collective_scope(
            context,
            model,
            control,
            raw_required_scope(barrier.execution_scope),
            ProductionW4AnalysisObligationKindV1::Uniformity,
            ProductionCapabilityAnalysisKindV1::Uniformity,
            ProductionW4CounterexampleClassV1::Uniformity,
            barrier.location,
        )?;
        let control = control.expect("reachable collective checked above");
        facts.fact(&[
            barrier.location.block as u64,
            barrier.location.operation as u64,
            u64::from(control.rank),
            u64::from(control.known),
            u64::from(raw_required_scope(barrier.execution_scope)),
        ]);
    }
    for location in &model.tensor_collectives {
        let control = model.control_scope[location.block];
        raw_require_collective_scope(
            context,
            model,
            control,
            RAW_SUBGROUP_SCOPE_V1,
            ProductionW4AnalysisObligationKindV1::Uniformity,
            ProductionCapabilityAnalysisKindV1::Uniformity,
            ProductionW4CounterexampleClassV1::Uniformity,
            *location,
        )?;
        let control = control.expect("reachable collective checked above");
        facts.fact(&[
            location.block as u64,
            location.operation as u64,
            u64::from(control.rank),
            u64::from(control.known),
            u64::from(RAW_SUBGROUP_SCOPE_V1),
        ]);
    }
    Ok(facts.finish(checker))
}

fn raw_require_collective_scope(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    control: Option<RawControlScopeV1>,
    required: u8,
    obligation: ProductionW4AnalysisObligationKindV1,
    stage: ProductionCapabilityAnalysisKindV1,
    class: ProductionW4CounterexampleClassV1,
    location: ProductionW4CounterexampleLocationV1,
) -> Result<(), ProductionW4RawIrFailureV1> {
    let Some(control) = control else {
        return Err(raw_ir_incomplete(
            obligation,
            stage,
            class,
            Some(location),
            "raw-IR collective is unreachable from the retained entry".to_owned(),
        ));
    };
    if control.rank >= required {
        return Ok(());
    }
    let detail = format!(
        "raw-IR collective at block {} op {} requires scope rank {required}, observed {}",
        location.block, location.operation, control.rank
    );
    if let Some(controller) = raw_definite_divergent_controller(context, model, required, location)
    {
        Err(raw_ir_rejected(
            obligation,
            stage,
            class,
            Some(location),
            Some(controller),
            detail,
        ))
    } else {
        Err(raw_ir_incomplete(
            obligation,
            stage,
            class,
            Some(location),
            detail,
        ))
    }
}

fn raw_collective_participation_receipt(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrReceiptV1, ProductionW4RawIrFailureV1> {
    let checker = ProductionW4IndependentObligationCheckerV1::CollectiveParticipationFreshLiveIrV1;
    let mut facts = RawFactsV1::new(checker, structural, pliron_epoch);
    for barrier in &model.barriers {
        let required = raw_required_scope(barrier.execution_scope);
        let control = model.control_scope[barrier.location.block];
        raw_require_collective_scope(
            context,
            model,
            control,
            required,
            ProductionW4AnalysisObligationKindV1::CollectiveParticipation,
            ProductionCapabilityAnalysisKindV1::BarrierConvergence,
            ProductionW4CounterexampleClassV1::CollectiveParticipation,
            barrier.location,
        )?;
        facts.fact(&[
            barrier.location.block as u64,
            barrier.location.operation as u64,
            u64::from(required),
        ]);
    }
    for location in &model.tensor_collectives {
        raw_require_collective_scope(
            context,
            model,
            model.control_scope[location.block],
            RAW_SUBGROUP_SCOPE_V1,
            ProductionW4AnalysisObligationKindV1::CollectiveParticipation,
            ProductionCapabilityAnalysisKindV1::BarrierConvergence,
            ProductionW4CounterexampleClassV1::CollectiveParticipation,
            *location,
        )?;
        facts.fact(&[
            location.block as u64,
            location.operation as u64,
            u64::from(RAW_SUBGROUP_SCOPE_V1),
        ]);
    }
    Ok(facts.finish(checker))
}

fn raw_barrier_order_receipt(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrReceiptV1, ProductionW4RawIrFailureV1> {
    let checker = ProductionW4IndependentObligationCheckerV1::BarrierOrderFreshLiveIrV1;
    let mut facts = RawFactsV1::new(checker, structural, pliron_epoch);
    for barrier in &model.barriers {
        let required = raw_required_scope(barrier.execution_scope);
        raw_require_collective_scope(
            context,
            model,
            model.control_scope[barrier.location.block],
            required,
            ProductionW4AnalysisObligationKindV1::BarrierOrder,
            ProductionCapabilityAnalysisKindV1::BarrierConvergence,
            ProductionW4CounterexampleClassV1::BarrierOrder,
            barrier.location,
        )?;
        facts.fact(&[
            barrier.location.block as u64,
            barrier.location.operation as u64,
            u64::from(required),
            model.predecessors[barrier.location.block].len() as u64,
            model.dominators[barrier.location.block].len() as u64,
        ]);
    }
    Ok(facts.finish(checker))
}

fn raw_site_dominates(
    model: &ProductionW4RawIrModelV1,
    first: ProductionW4CounterexampleLocationV1,
    second: ProductionW4CounterexampleLocationV1,
) -> bool {
    first.block == second.block && first.operation < second.operation
        || first.block != second.block
            && model
                .dominators
                .get(second.block)
                .is_some_and(|dominators| dominators.contains(&first.block))
}

fn raw_pipeline_view(model: &ProductionW4RawIrModelV1, view: Value) -> bool {
    model
        .pipeline_creates
        .iter()
        .any(|pipeline| pipeline.view == view)
}

fn raw_accesses_may_alias(first: &RawAccessV1, second: &RawAccessV1) -> bool {
    if first.view == second.view {
        return true;
    }
    if first.allocation_origin != 0 && second.allocation_origin != 0 {
        return first.allocation_origin == second.allocation_origin;
    }
    first.noalias_class == 0
        || second.noalias_class == 0
        || first.noalias_class == second.noalias_class
}

fn raw_accesses_may_conflict(context: &Context, first: &RawAccessV1, second: &RawAccessV1) -> bool {
    raw_accesses_may_alias(first, second)
        && !first
            .indices
            .iter()
            .zip(&second.indices)
            .any(|(&lhs, &rhs)| {
                raw_constant(context, lhs)
                    .zip(raw_constant(context, rhs))
                    .is_some_and(|(lhs, rhs)| lhs != rhs)
            })
}

const fn raw_memory_scope_rank(scope: MemoryScopeAttr) -> u8 {
    match scope {
        MemoryScopeAttr::Subgroup => RAW_SUBGROUP_SCOPE_V1,
        MemoryScopeAttr::Workgroup => RAW_WORKGROUP_SCOPE_V1,
        MemoryScopeAttr::Device => RAW_GRID_SCOPE_V1,
        MemoryScopeAttr::System => RAW_GRID_SCOPE_V1 + 1,
    }
}

fn raw_barrier_covers_access(barrier: &RawBarrierV1, access: &RawAccessV1) -> bool {
    let address_matches = matches!(
        (barrier.address_space, access.memory_space),
        (AddressSpaceAttr::Workgroup, MemorySpaceAttr::Workgroup)
            | (AddressSpaceAttr::Global, MemorySpaceAttr::Global)
    );
    address_matches
        && matches!(
            barrier.order,
            MemoryOrderAttr::AcquireRelease | MemoryOrderAttr::SequentiallyConsistent
        )
        && raw_memory_scope_rank(barrier.memory_scope)
            >= raw_required_scope(barrier.execution_scope)
}

fn raw_barrier_publishes(
    model: &ProductionW4RawIrModelV1,
    write: &RawAccessV1,
    read: &RawAccessV1,
) -> bool {
    model.barriers.iter().any(|barrier| {
        raw_barrier_covers_access(barrier, write)
            && raw_barrier_covers_access(barrier, read)
            && raw_required_scope(barrier.execution_scope) >= RAW_WORKGROUP_SCOPE_V1
            && raw_site_dominates(model, write.location, barrier.location)
            && raw_site_dominates(model, barrier.location, read.location)
    })
}

fn raw_single_invocation(model: &ProductionW4RawIrModelV1) -> bool {
    model.layout.is_some_and(|layout| {
        layout.global.into_iter().try_fold(1_u64, u64::checked_mul) == Some(1)
    })
}

fn raw_atomic_orders_conflict(
    model: &ProductionW4RawIrModelV1,
    write: &RawAccessV1,
    read: &RawAccessV1,
) -> bool {
    if !write.kind.is_atomic() || !read.kind.is_atomic() {
        return false;
    }
    let required_scope = if write.memory_space == MemorySpaceAttr::Global
        && model.layout.is_some_and(|layout| {
            layout
                .global
                .into_iter()
                .zip(layout.workgroup)
                .any(|(global, workgroup)| global > workgroup)
        }) {
        AtomicScopeAttr::Agent.rank()
    } else {
        AtomicScopeAttr::Workgroup.rank()
    };
    let releases = matches!(
        write.atomic_ordering,
        Some(
            AtomicOrderingAttr::Release
                | AtomicOrderingAttr::AcquireRelease
                | AtomicOrderingAttr::SequentiallyConsistent
        )
    );
    let acquires = matches!(
        read.atomic_ordering,
        Some(
            AtomicOrderingAttr::Acquire
                | AtomicOrderingAttr::AcquireRelease
                | AtomicOrderingAttr::SequentiallyConsistent
        )
    );
    releases
        && acquires
        && write
            .atomic_scope
            .is_some_and(|scope| scope.rank() >= required_scope)
        && read
            .atomic_scope
            .is_some_and(|scope| scope.rank() >= required_scope)
}

fn raw_pipeline_access_is_placed(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    access: &RawAccessV1,
) -> bool {
    let mut creates = model
        .pipeline_creates
        .iter()
        .filter(|create| create.view == access.view);
    let Some(create) = creates.next() else {
        return false;
    };
    if creates.next().is_some() {
        return false;
    }
    let Some(&slot) = access.indices.first() else {
        return false;
    };
    let budget = model
        .inventory
        .blocks()
        .len()
        .saturating_add(model.inventory.operations().len())
        .saturating_add(1)
        .min(MAX_PRODUCTION_W4_RAW_IR_VALUE_DEPTH_V1);
    let matches = |event: &&RawPipelineEventV1, kind: PipelineEventKindAttr| {
        event.pipeline == create.pipeline
            && event.kind == kind
            && raw_values_equivalent(context, model, event.slot, slot, budget)
    };
    if access.kind.writes_memory() {
        model
            .pipeline_events
            .iter()
            .filter(|event| matches(event, PipelineEventKindAttr::Stage))
            .any(|stage| {
                model
                    .pipeline_events
                    .iter()
                    .filter(|event| matches(event, PipelineEventKindAttr::Commit))
                    .any(|commit| {
                        raw_values_equivalent(context, model, stage.epoch, commit.epoch, budget)
                            && raw_site_dominates(model, stage.location, access.location)
                            && raw_site_dominates(model, access.location, commit.location)
                    })
            })
    } else if access.kind.reads_memory() {
        model
            .pipeline_events
            .iter()
            .filter(|event| matches(event, PipelineEventKindAttr::Wait))
            .any(|wait| {
                model
                    .pipeline_events
                    .iter()
                    .filter(|event| matches(event, PipelineEventKindAttr::Consume))
                    .any(|consume| {
                        raw_values_equivalent(context, model, wait.epoch, consume.epoch, budget)
                            && raw_site_dominates(model, wait.location, consume.location)
                            && raw_site_dominates(model, consume.location, access.location)
                            && model
                                .pipeline_events
                                .iter()
                                .filter(|event| matches(event, PipelineEventKindAttr::Release))
                                .any(|release| {
                                    raw_values_equivalent(
                                        context,
                                        model,
                                        consume.epoch,
                                        release.epoch,
                                        budget,
                                    ) && raw_site_dominates(
                                        model,
                                        access.location,
                                        release.location,
                                    )
                                })
                    })
            })
    } else {
        false
    }
}

fn raw_pipeline_orders_pair(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    write: &RawAccessV1,
    read: &RawAccessV1,
) -> bool {
    write.view == read.view
        && raw_pipeline_access_is_placed(context, model, write)
        && raw_pipeline_access_is_placed(context, model, read)
}

fn raw_happens_before_receipt(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrReceiptV1, ProductionW4RawIrFailureV1> {
    let checker = ProductionW4IndependentObligationCheckerV1::HappensBeforeFreshLiveIrV1;
    let mut facts = RawFactsV1::new(checker, structural, pliron_epoch);
    raw_require_pair_budget(
        model.accesses.len(),
        ProductionW4AnalysisObligationKindV1::HappensBefore,
        ProductionCapabilityAnalysisKindV1::RaceFreedom,
        ProductionW4CounterexampleClassV1::MemoryVisibility,
        "raw-IR happens-before conflict pairs",
    )?;
    for read in model
        .accesses
        .iter()
        .filter(|access| access.kind.reads_memory())
    {
        for write in model.accesses.iter().filter(|access| {
            access.kind.writes_memory() && raw_accesses_may_conflict(context, access, read)
        }) {
            let ordered = write.memory_space == MemorySpaceAttr::Private
                || raw_pipeline_orders_pair(context, model, write, read)
                || raw_single_invocation(model)
                    && raw_site_dominates(model, write.location, read.location)
                || raw_barrier_publishes(model, write, read)
                || raw_atomic_orders_conflict(model, write, read);
            if !ordered {
                return Err(raw_ir_incomplete(
                    ProductionW4AnalysisObligationKindV1::HappensBefore,
                    ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                    ProductionW4CounterexampleClassV1::MemoryVisibility,
                    Some(write.location),
                    format!(
                        "raw-IR cannot construct a synchronizes-with edge from block {} op {} to block {} op {}",
                        write.location.block,
                        write.location.operation,
                        read.location.block,
                        read.location.operation
                    ),
                ));
            }
            facts.fact(&[
                write.location.block as u64,
                write.location.operation as u64,
                read.location.block as u64,
                read.location.operation as u64,
                u64::from(write.memory_space as u8),
            ]);
        }
    }
    Ok(facts.finish(checker))
}

fn raw_initialization_receipt(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrReceiptV1, ProductionW4RawIrFailureV1> {
    let checker = ProductionW4IndependentObligationCheckerV1::InitializationFreshLiveIrV1;
    let mut facts = RawFactsV1::new(checker, structural, pliron_epoch);
    raw_require_pair_budget(
        model.accesses.len(),
        ProductionW4AnalysisObligationKindV1::Initialization,
        ProductionCapabilityAnalysisKindV1::Initialization,
        ProductionW4CounterexampleClassV1::ReadBeforeInitialization,
        "raw-IR initialization pairs",
    )?;
    for read in model.accesses.iter().filter(|access| {
        access.memory_space == MemorySpaceAttr::Workgroup && access.kind.reads_memory()
    }) {
        if raw_pipeline_view(model, read.view) {
            if !raw_pipeline_access_is_placed(context, model, read) {
                return Err(raw_ir_incomplete(
                    ProductionW4AnalysisObligationKindV1::Initialization,
                    ProductionCapabilityAnalysisKindV1::Initialization,
                    ProductionW4CounterexampleClassV1::ReadBeforeInitialization,
                    Some(read.location),
                    "raw-IR pipeline read is not enclosed by wait/consume/release transitions"
                        .to_owned(),
                ));
            }
            facts.fact(&[
                read.location.block as u64,
                read.location.operation as u64,
                1,
            ]);
            continue;
        }
        let potential_writes = model
            .accesses
            .iter()
            .filter(|write| write.kind.writes_memory() && raw_accesses_may_alias(write, read));
        let potential_write_count = potential_writes.clone().count();
        let initialized = potential_writes.clone().any(|write| {
            write.view == read.view
                && raw_write_covers_read(context, model, write, read)
                && (raw_single_invocation(model)
                    && raw_site_dominates(model, write.location, read.location)
                    || raw_barrier_publishes(model, write, read))
        });
        if !initialized {
            let detail = format!(
                "raw-IR workgroup read at block {} op {} lacks a proved covering initialization and publication",
                read.location.block, read.location.operation
            );
            return Err(if potential_write_count == 0 {
                raw_ir_rejected(
                    ProductionW4AnalysisObligationKindV1::Initialization,
                    ProductionCapabilityAnalysisKindV1::Initialization,
                    ProductionW4CounterexampleClassV1::ReadBeforeInitialization,
                    Some(read.location),
                    None,
                    detail,
                )
            } else {
                raw_ir_incomplete(
                    ProductionW4AnalysisObligationKindV1::Initialization,
                    ProductionCapabilityAnalysisKindV1::Initialization,
                    ProductionW4CounterexampleClassV1::ReadBeforeInitialization,
                    Some(read.location),
                    detail,
                )
            });
        }
        facts.fact(&[
            read.location.block as u64,
            read.location.operation as u64,
            0,
        ]);
    }
    Ok(facts.finish(checker))
}

fn raw_write_covers_read(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    write: &RawAccessV1,
    read: &RawAccessV1,
) -> bool {
    if write.indices.len() != read.indices.len() {
        return false;
    }
    let budget = model
        .inventory
        .blocks()
        .len()
        .saturating_add(model.inventory.operations().len())
        .saturating_add(1)
        .min(MAX_PRODUCTION_W4_RAW_IR_VALUE_DEPTH_V1);
    write
        .indices
        .iter()
        .zip(&read.indices)
        .all(|(&write_index, &read_index)| {
            raw_values_equivalent(context, model, write_index, read_index, budget)
                || raw_invocation_covers_constant(context, write_index, read_index)
        })
        && model.control_scope[write.location.block]
            .is_some_and(|scope| scope.rank >= RAW_WORKGROUP_SCOPE_V1)
}

fn raw_invocation_covers_constant(context: &Context, invocation: Value, constant: Value) -> bool {
    let Some(definition) = invocation.defining_op() else {
        return false;
    };
    let operation = Operation::get_op_dyn(definition, context);
    let Some(invocation) = operation.downcast_ref::<InvocationIndexOp>() else {
        return false;
    };
    invocation
        .launch_extent(context)
        .zip(raw_constant(context, constant))
        .is_some_and(|(extent, index)| index < extent)
}

fn raw_visibility_receipt(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrReceiptV1, ProductionW4RawIrFailureV1> {
    let checker = ProductionW4IndependentObligationCheckerV1::MemoryVisibilityFreshLiveIrV1;
    let mut facts = RawFactsV1::new(checker, structural, pliron_epoch);
    raw_require_pair_budget(
        model.accesses.len(),
        ProductionW4AnalysisObligationKindV1::MemoryVisibility,
        ProductionCapabilityAnalysisKindV1::MemoryVisibility,
        ProductionW4CounterexampleClassV1::MemoryVisibility,
        "raw-IR visibility pairs",
    )?;
    for read in model
        .accesses
        .iter()
        .filter(|access| access.kind.reads_memory())
    {
        let writes = model
            .accesses
            .iter()
            .filter(|access| {
                access.kind.writes_memory() && raw_accesses_may_conflict(context, access, read)
            })
            .collect::<Vec<_>>();
        if writes.is_empty() {
            continue;
        }
        let visible = writes.iter().any(|write| {
            write.memory_space == MemorySpaceAttr::Private
                || raw_pipeline_orders_pair(context, model, write, read)
                || raw_atomic_orders_conflict(model, write, read)
                || raw_single_invocation(model)
                    && raw_site_dominates(model, write.location, read.location)
                || raw_barrier_publishes(model, write, read)
        });
        if !visible {
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::MemoryVisibility,
                ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                ProductionW4CounterexampleClassV1::MemoryVisibility,
                Some(read.location),
                format!(
                    "raw-IR read at block {} op {} has no independently checked visibility edge",
                    read.location.block, read.location.operation
                ),
            ));
        }
        facts.fact(&[
            read.location.block as u64,
            read.location.operation as u64,
            writes.len() as u64,
        ]);
    }
    Ok(facts.finish(checker))
}

fn raw_require_pair_budget(
    item_count: usize,
    obligation: ProductionW4AnalysisObligationKindV1,
    stage: ProductionCapabilityAnalysisKindV1,
    class: ProductionW4CounterexampleClassV1,
    analysis: &str,
) -> Result<(), ProductionW4RawIrFailureV1> {
    let pairs = item_count.checked_mul(item_count).ok_or_else(|| {
        raw_ir_incomplete(
            obligation,
            stage,
            class,
            None,
            format!("{analysis} count overflowed"),
        )
    })?;
    if pairs > MAX_PRODUCTION_W4_RAW_IR_WORK_UNITS_V1 {
        return Err(raw_ir_incomplete(
            obligation,
            stage,
            class,
            None,
            format!(
                "{analysis} require {pairs} units, exceeding the {} work-unit budget",
                MAX_PRODUCTION_W4_RAW_IR_WORK_UNITS_V1
            ),
        ));
    }
    Ok(())
}

fn raw_epoch_receipt(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrReceiptV1, ProductionW4RawIrFailureV1> {
    let checker = ProductionW4IndependentObligationCheckerV1::WorkgroupMemoryEpochsFreshLiveIrV1;
    let mut facts = RawFactsV1::new(checker, structural, pliron_epoch);
    for create in &model.pipeline_creates {
        let events = model
            .pipeline_events
            .iter()
            .filter(|event| event.pipeline == create.pipeline)
            .collect::<Vec<_>>();
        if events.is_empty() {
            return Err(raw_ir_rejected(
                ProductionW4AnalysisObligationKindV1::WorkgroupMemoryEpochs,
                ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs,
                ProductionW4CounterexampleClassV1::PipelineEpochProtocol,
                Some(create.location),
                None,
                "raw-IR pipeline has no epoch transitions".to_owned(),
            ));
        }
        for required in [
            PipelineEventKindAttr::Stage,
            PipelineEventKindAttr::Commit,
            PipelineEventKindAttr::Wait,
            PipelineEventKindAttr::Release,
        ] {
            if !events.iter().any(|event| event.kind == required) {
                return Err(raw_ir_incomplete(
                    ProductionW4AnalysisObligationKindV1::WorkgroupMemoryEpochs,
                    ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs,
                    ProductionW4CounterexampleClassV1::PipelineEpochProtocol,
                    Some(create.location),
                    format!("raw-IR pipeline is missing the {required:?} epoch transition"),
                ));
            }
        }
        if !events.iter().any(|event| {
            matches!(
                event.kind,
                PipelineEventKindAttr::Consume | PipelineEventKindAttr::Discard
            )
        }) {
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::WorkgroupMemoryEpochs,
                ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs,
                ProductionW4CounterexampleClassV1::PipelineEpochProtocol,
                Some(create.location),
                "raw-IR pipeline has no consume or discard transition".to_owned(),
            ));
        }
        let equivalence_budget = model
            .inventory
            .blocks()
            .len()
            .saturating_add(model.inventory.operations().len())
            .saturating_add(1);
        for event in &events {
            let prior_same_epoch_slot = |kind| {
                events.iter().any(|prior| {
                    prior.kind == kind
                        && raw_site_dominates(model, prior.location, event.location)
                        && raw_values_equivalent(
                            context,
                            model,
                            prior.epoch,
                            event.epoch,
                            equivalence_budget,
                        )
                        && raw_values_equivalent(
                            context,
                            model,
                            prior.slot,
                            event.slot,
                            equivalence_budget,
                        )
                })
            };
            let legal = match event.kind {
                PipelineEventKindAttr::Stage | PipelineEventKindAttr::Wait => true,
                PipelineEventKindAttr::Commit => {
                    prior_same_epoch_slot(PipelineEventKindAttr::Stage)
                }
                PipelineEventKindAttr::Consume | PipelineEventKindAttr::Discard => {
                    prior_same_epoch_slot(PipelineEventKindAttr::Wait)
                }
                PipelineEventKindAttr::Release => {
                    prior_same_epoch_slot(PipelineEventKindAttr::Consume)
                        || prior_same_epoch_slot(PipelineEventKindAttr::Discard)
                }
            };
            if !legal {
                return Err(raw_ir_rejected(
                    ProductionW4AnalysisObligationKindV1::WorkgroupMemoryEpochs,
                    ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs,
                    ProductionW4CounterexampleClassV1::PipelineEpochProtocol,
                    Some(event.location),
                    None,
                    format!(
                        "raw-IR {:?} at block {} op {} has no required same-epoch predecessor",
                        event.kind, event.location.block, event.location.operation
                    ),
                ));
            }
            facts.fact(&[
                event.location.block as u64,
                event.location.operation as u64,
                raw_pipeline_event_tag(event.kind),
            ]);
        }
    }
    for barrier in &model.barriers {
        facts.fact(&[
            barrier.location.block as u64,
            barrier.location.operation as u64,
            16 + u64::from(raw_required_scope(barrier.execution_scope)),
        ]);
    }
    Ok(facts.finish(checker))
}

const fn raw_pipeline_event_tag(kind: PipelineEventKindAttr) -> u64 {
    match kind {
        PipelineEventKindAttr::Stage => 0,
        PipelineEventKindAttr::Commit => 1,
        PipelineEventKindAttr::Wait => 2,
        PipelineEventKindAttr::Consume => 3,
        PipelineEventKindAttr::Discard => 4,
        PipelineEventKindAttr::Release => 5,
    }
}

fn raw_semantic_expression_identity(
    context: &Context,
    value: Value,
    visiting: &mut HashSet<Value>,
) -> Option<[u8; 32]> {
    if !visiting.insert(value) {
        return None;
    }
    let operation = value.defining_op()?;
    let dynamic = Operation::get_op_dyn(operation, context);
    let mut digest = Sha256::new();
    digest.update(RAW_IR_EVIDENCE_DOMAIN_V1);
    if let Some(symbol) = dynamic.downcast_ref::<SemanticSymbolOp>() {
        digest.update([0]);
        digest.update(symbol.symbol(context)?.to_le_bytes());
    } else if let Some(constant) = dynamic.downcast_ref::<SemanticConstantOp>() {
        digest.update([1]);
        digest.update(constant.value(context)?.to_le_bytes());
    } else if let Some(commitment) = dynamic.downcast_ref::<SemanticExpressionCommitmentOp>() {
        digest.update([2]);
        for word in commitment.identity(context)? {
            digest.update(word.to_le_bytes());
        }
    } else if let Some(binary) = dynamic.downcast_ref::<SemanticBinaryOp>() {
        digest.update([match binary.kind(context)? {
            SemanticBinaryKindAttr::Add => 3,
            SemanticBinaryKindAttr::Multiply => 4,
        }]);
        digest.update(raw_semantic_expression_identity(
            context,
            binary.lhs(context),
            visiting,
        )?);
        visiting.remove(&binary.lhs(context));
        digest.update(raw_semantic_expression_identity(
            context,
            binary.rhs(context),
            visiting,
        )?);
        visiting.remove(&binary.rhs(context));
    } else {
        return None;
    }
    visiting.remove(&value);
    Some(digest.finalize().into())
}

fn raw_effect_refinement_receipt(
    context: &Context,
    model: &ProductionW4RawIrModelV1,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrReceiptV1, ProductionW4RawIrFailureV1> {
    let checker = ProductionW4IndependentObligationCheckerV1::EffectRefinementFreshLiveIrV1;
    let mut facts = RawFactsV1::new(checker, structural, pliron_epoch);
    for contract in &model.effect_contracts {
        let obligations = model
            .proof_obligations
            .iter()
            .filter(|obligation| obligation.identity == contract.obligation)
            .collect::<Vec<_>>();
        let evidence = model
            .proof_evidence
            .iter()
            .filter(|evidence| evidence.obligation == contract.obligation)
            .collect::<Vec<_>>();
        let proof_complete = obligations.len() == 1
            && !raw_identity_is_zero(obligations[0].subject)
            && !raw_identity_is_zero(obligations[0].model)
            && obligations[0].property == Some(PropertyAttr::FunctionalRefinement)
            && evidence.len() == 1
            && !raw_identity_is_zero(evidence[0].evidence)
            && evidence[0].property == Some(PropertyAttr::FunctionalRefinement)
            && evidence[0].status == Some(EvidenceStatusAttr::Checked)
            && evidence[0].boundary == Some(CoveredBoundaryAttr::Mir);
        if !proof_complete {
            return Err(raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::EffectRefinement,
                ProductionCapabilityAnalysisKindV1::EffectRefinement,
                ProductionW4CounterexampleClassV1::EffectRefinement,
                Some(contract.location),
                "raw-IR effect contract lacks one exact MIR obligation/evidence pair".to_owned(),
            ));
        }
        let matching_writes = model
            .accesses
            .iter()
            .filter(|access| {
                access.view == contract.view
                    && access.kind.writes_memory()
                    && access.indices == contract.indices
            })
            .count();
        if !model.ownership_views.contains(&contract.view) || matching_writes != 1 {
            return Err(raw_ir_rejected(
                ProductionW4AnalysisObligationKindV1::EffectRefinement,
                ProductionCapabilityAnalysisKindV1::EffectRefinement,
                ProductionW4CounterexampleClassV1::EffectRefinement,
                Some(contract.location),
                None,
                "raw-IR effect contract does not select one owned GPU write".to_owned(),
            ));
        }
        for &(gpu, reference) in &contract.expression_pairs {
            let gpu = raw_semantic_expression_identity(context, gpu, &mut HashSet::new());
            let reference =
                raw_semantic_expression_identity(context, reference, &mut HashSet::new());
            let (Some(gpu), Some(reference)) = (gpu, reference) else {
                return Err(raw_ir_incomplete(
                    ProductionW4AnalysisObligationKindV1::EffectRefinement,
                    ProductionCapabilityAnalysisKindV1::EffectRefinement,
                    ProductionW4CounterexampleClassV1::EffectRefinement,
                    Some(contract.location),
                    "raw-IR effect expression uses an unsupported semantic operator".to_owned(),
                ));
            };
            if gpu != reference {
                return Err(raw_ir_rejected(
                    ProductionW4AnalysisObligationKindV1::EffectRefinement,
                    ProductionCapabilityAnalysisKindV1::EffectRefinement,
                    ProductionW4CounterexampleClassV1::EffectRefinement,
                    Some(contract.location),
                    None,
                    "raw-IR GPU and reference effect expressions differ".to_owned(),
                ));
            }
        }
        facts.fact(&[
            contract.location.block as u64,
            contract.location.operation as u64,
            matching_writes as u64,
            contract.expression_pairs.len() as u64,
        ]);
    }
    Ok(facts.finish(checker))
}

pub(super) fn derive_production_w4_raw_ir_evidence_v1(
    context: &Context,
    live: &ProductionW4LiveFunctionV1<'_>,
    structural: &PlironIrStructuralIdentityV1,
    pliron_epoch: u64,
) -> Result<ProductionW4RawIrFunctionEvidenceV1, ProductionW4RawIrFailureV1> {
    let observed_epoch = current_pliron_epoch(context).map_err(|error| {
        raw_ir_incomplete(
            ProductionW4AnalysisObligationKindV1::Uniformity,
            ProductionCapabilityAnalysisKindV1::Uniformity,
            ProductionW4CounterexampleClassV1::Uniformity,
            None,
            error.to_string(),
        )
    })?;
    let observed_identity =
        derive_pliron_ir_structural_identity_v1(context, live.pliron).map_err(|error| {
            raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                None,
                error.to_string(),
            )
        })?;
    if observed_epoch != pliron_epoch || !structural.exactly_matches(&observed_identity) {
        return Err(raw_ir_incomplete(
            ProductionW4AnalysisObligationKindV1::Uniformity,
            ProductionCapabilityAnalysisKindV1::Uniformity,
            ProductionW4CounterexampleClassV1::Uniformity,
            None,
            "raw-IR subject changed before independent obligation replay".to_owned(),
        ));
    }
    let model = ProductionW4RawIrModelV1::build(context, live.pliron)?;
    let receipts = vec![
        raw_uniformity_receipt(context, &model, structural, pliron_epoch)?,
        raw_memory_bounds_receipt(context, &model, structural, pliron_epoch)?,
        raw_happens_before_receipt(context, &model, structural, pliron_epoch)?,
        raw_barrier_order_receipt(context, &model, structural, pliron_epoch)?,
        raw_collective_participation_receipt(context, &model, structural, pliron_epoch)?,
        raw_initialization_receipt(context, &model, structural, pliron_epoch)?,
        raw_visibility_receipt(context, &model, structural, pliron_epoch)?,
        raw_epoch_receipt(context, &model, structural, pliron_epoch)?,
        raw_effect_refinement_receipt(context, &model, structural, pliron_epoch)?,
    ];
    let after_epoch = current_pliron_epoch(context).map_err(|error| {
        raw_ir_incomplete(
            ProductionW4AnalysisObligationKindV1::Uniformity,
            ProductionCapabilityAnalysisKindV1::Uniformity,
            ProductionW4CounterexampleClassV1::Uniformity,
            None,
            error.to_string(),
        )
    })?;
    let after_identity =
        derive_pliron_ir_structural_identity_v1(context, live.pliron).map_err(|error| {
            raw_ir_incomplete(
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionW4CounterexampleClassV1::Uniformity,
                None,
                error.to_string(),
            )
        })?;
    if after_epoch != pliron_epoch || !structural.exactly_matches(&after_identity) {
        return Err(raw_ir_incomplete(
            ProductionW4AnalysisObligationKindV1::Uniformity,
            ProductionCapabilityAnalysisKindV1::Uniformity,
            ProductionW4CounterexampleClassV1::Uniformity,
            None,
            "raw-IR subject changed during independent obligation replay".to_owned(),
        ));
    }
    Ok(ProductionW4RawIrFunctionEvidenceV1 {
        function: live.function.clone(),
        structural_sha256: *structural.sha256(),
        structural_bytes: structural.canonical_bytes_len(),
        pliron_epoch,
        receipts: receipts.into_boxed_slice(),
    })
}

fn raw_ir_rejected(
    obligation: ProductionW4AnalysisObligationKindV1,
    stage: ProductionCapabilityAnalysisKindV1,
    class: ProductionW4CounterexampleClassV1,
    primary: Option<ProductionW4CounterexampleLocationV1>,
    related: Option<ProductionW4CounterexampleLocationV1>,
    detail: String,
) -> ProductionW4RawIrFailureV1 {
    ProductionW4RawIrFailureV1 {
        obligation,
        stage,
        status: KernelCheckStatusV1::Rejected,
        class,
        primary,
        related,
        detail,
    }
}

fn raw_ir_incomplete(
    obligation: ProductionW4AnalysisObligationKindV1,
    stage: ProductionCapabilityAnalysisKindV1,
    class: ProductionW4CounterexampleClassV1,
    primary: Option<ProductionW4CounterexampleLocationV1>,
    detail: String,
) -> ProductionW4RawIrFailureV1 {
    ProductionW4RawIrFailureV1 {
        obligation,
        stage,
        status: KernelCheckStatusV1::Incomplete,
        class,
        primary,
        related: None,
        detail,
    }
}
