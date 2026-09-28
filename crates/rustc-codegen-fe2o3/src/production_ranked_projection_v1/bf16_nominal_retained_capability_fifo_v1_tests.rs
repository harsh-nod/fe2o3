//! Synthetic FIFO/resource controls only; never genuine nominal-source evidence.
use super::*;
use crate::production_ranked_projection_v1::{
    MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1, ProjectedCapabilityOriginV1,
    ProjectedCapabilityStateV1, assertion_definition_inventory,
    charged_unique_capability_successors_v1, constant_locals, local_allocation_contracts,
    local_provenance_with_scalar_inventory_v1, merge_capability_states_v1,
    propagate_capability_dataflow_v1, try_clone_capability_state_v1,
    workgroup_pipeline_local_owners_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
use std::collections::VecDeque;

const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);

const ARRAY_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);

const POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(tag)),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn cfg_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn projection_function(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    projection_function_with_locals(
        blocks,
        vec![
            local(20, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(21, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
            local(22, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(23, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    )
}

fn projection_function_with_locals(
    blocks: Vec<SemanticBasicBlockV1>,
    locals: Vec<SemanticLocalDeclV1>,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(10)),
        SemanticLayoutIdentityV1::from_sha256(bytes(10)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(11)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(12)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(13)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(14)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(15)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

fn constant(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SCALAR_TYPE,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

fn compiler_intrinsic_callable(
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    let abi = projection_function(vec![block(116, vec![], SemanticTerminatorKindV1::Return)])
        .abi()
        .clone();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(116)),
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(117)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(118)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(119)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(120)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(121)),
    }
}

fn place(index: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], POINTER_TYPE).unwrap()
}

fn operand(index: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(index))
}

fn assign(index: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(index),
        SemanticRvalueV1::new(POINTER_TYPE, value),
    )))
}

fn borrow(destination: u32, source: u32) -> SemanticStatementV1 {
    assign(
        destination,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: place(source),
        },
    )
}

fn call(callee: u32, arguments: Vec<SemanticOperandV1>, target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], SCALAR_TYPE)
                    .unwrap(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn callables() -> Vec<SemanticCallableDeclV1> {
    vec![
        compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice: POINTER_TYPE,
            index_witness: SCALAR_TYPE,
            element: SCALAR_TYPE,
            raw_index: SCALAR_TYPE,
        }),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
    ]
}

fn function(
    first: Vec<SemanticStatementV1>,
    first_terminator: SemanticTerminatorKindV1,
    second: Vec<SemanticStatementV1>,
    second_arguments: Vec<SemanticOperandV1>,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(210)),
        SemanticLayoutIdentityV1::from_sha256(bytes(210)),
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        vec![
            SemanticAbiValueV1::new(
                POINTER_TYPE,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain())
            );
            2
        ],
        SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        ownership,
        SemanticSourceArgumentOwnershipV1::RawPointer,
    ])
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(211)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(212)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(213)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(214)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(215)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![
            local(210, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(211, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
            local(212, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            local(213, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            local(214, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            local(215, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(216, POINTER_TYPE, SemanticLocalRoleV1::Argument(1)),
            local(217, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(210, first, first_terminator),
            block(211, second, call(0, second_arguments, 2)),
            block(212, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
}

fn ordinary(first: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    function(
        first,
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        vec![borrow(3, 2)],
        vec![operand(3), constant(0)],
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
    )
}

fn with_parts(
    base: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        base.identity(),
        base.role(),
        base.item_definition_identity(),
        base.monomorphization_identity(),
        base.generic_type_arguments_identity(),
        base.const_generic_arguments_identity(),
        base.source(),
        abi,
        locals,
        base.entry(),
        base.blocks().to_vec(),
    )
    .unwrap()
}

fn projection_types() -> Vec<SemanticTypeDeclV1> {
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(1)),
            SemanticLayoutIdentityV1::from_sha256(bytes(1)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(2)),
            SemanticLayoutIdentityV1::from_sha256(bytes(2)),
            SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
            SemanticTypeShapeV1::Array {
                element: SCALAR_TYPE,
                length: 4,
            },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(3)),
            SemanticLayoutIdentityV1::from_sha256(bytes(3)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    SCALAR_TYPE,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ]
}

struct SyntheticConsumer<'b, 'w> {
    budget: &'b mut Budget<'w>,
}
impl NominalCapabilityConsumerV1 for SyntheticConsumer<'_, '_> {
    fn charge_work_v1(&mut self, amount: usize) -> Result<()> {
        self.budget.charge_work(amount).map_err(Error::Resource)
    }
    fn reserve_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.budget.reserve_storage(amount).map_err(Error::Resource)
    }
    fn release_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.budget.release_storage(amount).map_err(Error::Resource)
    }
    fn ledger_v1(&self) -> NominalCapabilityLedgerV1 {
        NominalCapabilityLedgerV1 {
            slot: self.budget as *const Budget<'_> as usize,
            identity: self.budget.work_ledger_identity_v1(),
            work: self.budget.work(),
            storage: self.budget.storage(),
            peak: self.budget.peak_storage(),
            denied_work: self.budget.failed_work().is_some(),
            denied_storage: self.budget.failed_storage().is_some(),
        }
    }
    fn with_nominal_call_v1(
        &mut self,
        _: usize,
        _: &SemanticDirectCallV1,
        _: SemanticSourceProvenanceV1,
        _: &mut NominalCallVisitorV1<'_>,
    ) -> Result<()> {
        Err(Error::Unavailable(
            "synthetic consumer has no actual nominal candidate",
        ))
    }
}

// Exact original FIFO donor with trace-only instrumentation; unchanged donor also checked.
fn traced_original(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    local_allocations: &[Option<AllocationContractV1>],
    constants: &[Option<u64>],
    pipeline_owners: &[Option<usize>],
    pipeline_payloads: &HashMap<usize, ProjectedMfmaOperandV1>,
    entry: usize,
    work: &mut usize,
) -> std::result::Result<
    (Vec<Option<ProjectedCapabilityStateV1>>, Vec<usize>),
    ProductionRankedProjectionErrorV1,
> {
    let mut entries: Vec<Option<ProjectedCapabilityStateV1>> = vec![None; function.blocks().len()];
    entries[entry] = Some(HashMap::new());
    let mut worklist = VecDeque::from([entry]);
    let mut stored_entries = 0_usize;
    let mut visits = Vec::new();
    while let Some(block_index) = worklist.pop_front() {
        visits.push(block_index);
        let entry_state = entries.get(block_index).and_then(Option::as_ref).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a queued capability dataflow block has no entry state",
            ),
        )?;
        charge_capability_dataflow_work_v1(
            work,
            entry_state.len().checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported("capability clone work overflow"),
            )?,
        )?;
        let mut state = try_clone_capability_state_v1(entry_state)?;
        transfer_capability_statements_v1(
            function,
            block_index,
            &mut state,
            enum_payload_dominance,
        )?;
        transfer_capability_terminator_v1(
            callables,
            function,
            block_index,
            &mut state,
            local_allocations,
            constants,
            pipeline_owners,
            pipeline_payloads,
            false,
        )?;
        charge_capability_dataflow_work_v1(
            work,
            function.blocks()[block_index]
                .statements()
                .len()
                .checked_add(state.len())
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "capability dataflow work overflow",
                ))?,
        )?;
        if state.len() > MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1 {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "capability dataflow exceeds the charged projection limit",
            ));
        }
        let successors = charged_unique_capability_successors_v1(
            function.blocks()[block_index].terminator().kind(),
            state.len(),
            work,
        )?;
        for target in successors {
            let target_entry =
                entries
                    .get_mut(target)
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "a capability CFG edge outside the semantic function",
                    ))?;
            let changed = match target_entry {
                None => {
                    let next_stored_entries =
                        checked_capability_stored_entries_v1(stored_entries, state.len())?;
                    let successor_state = try_clone_capability_state_v1(&state)?;
                    stored_entries = next_stored_entries;
                    *target_entry = Some(successor_state);
                    true
                }
                Some(existing) => {
                    let additional = state
                        .keys()
                        .filter(|key| !existing.contains_key(key))
                        .count();
                    let next_stored_entries =
                        checked_capability_stored_entries_v1(stored_entries, additional)?;
                    let changed = merge_capability_states_v1(existing, &state)?;
                    stored_entries = next_stored_entries;
                    changed
                }
            };
            if changed {
                worklist.push_back(target);
            }
        }
    }
    Ok((entries, visits))
}

const LIMIT: usize = 8 * 1024 * 1024;
const FLOOR: usize = 11;
fn context_call(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(1),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}
fn branch(left: u32, right: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: constant(0),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                cfg_edge(SemanticEdgeRoleV1::SwitchValue, left),
            )],
            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, right),
        )
        .unwrap(),
    }
}
fn graph(kind: usize) -> SemanticFunctionDeclV1 {
    let (entry, blocks) = match kind {
        0 => (
            0,
            vec![
                block(1, vec![], context_call(1)),
                block(
                    2,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
                ),
                block(3, vec![], SemanticTerminatorKindV1::Return),
            ],
        ),
        1 => (
            0,
            vec![
                block(1, vec![], context_call(1)),
                block(2, vec![], branch(2, 3)),
                block(
                    3,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
                ),
                block(
                    4,
                    vec![statement(SemanticStatementKindV1::StorageDead(
                        SemanticLocalIdV1::from_index(1),
                    ))],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
                ),
                block(5, vec![], SemanticTerminatorKindV1::Return),
            ],
        ),
        _ => (
            4,
            vec![
                block(
                    1,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(2, vec![], SemanticTerminatorKindV1::Return),
                block(3, vec![], branch(0, 3)),
                block(
                    4,
                    vec![statement(SemanticStatementKindV1::StorageDead(
                        SemanticLocalIdV1::from_index(1),
                    ))],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(5, vec![], context_call(2)),
            ],
        ),
    };
    let base = projection_function_with_locals(
        blocks,
        vec![
            local(11, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(12, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            local(13, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            local(14, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    );
    SemanticFunctionDeclV1::new(
        base.identity(),
        base.role(),
        base.item_definition_identity(),
        base.monomorphization_identity(),
        base.generic_type_arguments_identity(),
        base.const_generic_arguments_identity(),
        base.source(),
        base.abi().clone(),
        base.locals().to_vec(),
        SemanticBlockIdV1::from_index(entry),
        base.blocks().to_vec(),
    )
    .unwrap()
}
struct Inputs {
    function: SemanticFunctionDeclV1,
    callables: Vec<SemanticCallableDeclV1>,
    dominance: SemanticEnumPayloadDominanceV1,
    allocations: Vec<Option<AllocationContractV1>>,
    constants: Vec<Option<u64>>,
    pipeline_owners: Vec<Option<usize>>,
    pipeline_payloads: HashMap<usize, ProjectedMfmaOperandV1>,
}
fn inputs(kind: usize) -> Inputs {
    let function = graph(kind);
    let callables = vec![compiler_intrinsic_callable(
        SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent {
            context: POINTER_TYPE,
        },
    )];
    let types = projection_types();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let scalar = assertion_definition_inventory(&function).unwrap();
    let provenance = local_provenance_with_scalar_inventory_v1(
        &callables,
        &types,
        &function,
        &scalar.counts,
        &scalar.address_escaped,
    )
    .unwrap();
    let allocations =
        local_allocation_contracts(&types, &function, &provenance.allocation_origins).unwrap();
    let constants = constant_locals(&function).unwrap();
    let pipeline_owners = workgroup_pipeline_local_owners_v1(&callables, &function).unwrap();
    Inputs {
        function,
        callables,
        dominance,
        allocations,
        constants,
        pipeline_owners,
        pipeline_payloads: HashMap::new(),
    } // Exact original INITIAL propagation input.
}
fn original(
    input: &Inputs,
    work: &mut usize,
) -> std::result::Result<
    (Vec<Option<ProjectedCapabilityStateV1>>, Vec<usize>),
    ProductionRankedProjectionErrorV1,
> {
    traced_original(
        &input.callables,
        &input.function,
        &input.dominance,
        &input.allocations,
        &input.constants,
        &input.pipeline_owners,
        &input.pipeline_payloads,
        input.function.entry().index() as usize,
        work,
    )
}
fn actual_transfer(input: &Inputs, block: usize, scratch: &mut [Slot]) -> Result<()> {
    let mut state = DenseCapabilityStateV1::new(scratch);
    transfer_capability_statements_v1(&input.function, block, &mut state, &input.dominance)
        .map_err(transfer_error)?;
    state.check_bounds_v1()?;
    let effects = transfer_capability_terminator_v1(
        &input.callables,
        &input.function,
        block,
        &mut state,
        &input.allocations,
        &input.constants,
        &input.pipeline_owners,
        &input.pipeline_payloads,
        false,
    )
    .map_err(transfer_error)?;
    drop(effects);
    state.check_bounds_v1()
}
fn compare_data(
    expected: &[Option<ProjectedCapabilityStateV1>],
    view: CompletedFifoPassV1<'_>,
    locals: usize,
) {
    assert_eq!(expected.len(), view.reached.len());
    for (block, state) in expected.iter().enumerate() {
        assert_eq!(state.is_some(), view.reached[block]);
        for local in 0..locals {
            assert_eq!(
                state.as_ref().and_then(|state| state.get(&local)).copied(),
                view.entries[block * locals + local]
            );
        }
    }
}
struct Probe {
    result: Result<()>,
    phase: Phase,
    entries: Vec<Slot>,
    reached: Vec<bool>,
    scratch: Vec<Slot>,
    queue: Vec<usize>,
    successors: Vec<usize>,
    visits: Vec<usize>,
    head: usize,
    current: Option<usize>,
    original_work: usize,
    work: usize,
    live: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn probe(
    input: &Inputs,
    work_limit: usize,
    storage_limit: usize,
    mode: usize,
    prefix: usize,
) -> Probe {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut pending = RetainedCapabilityFifoPassV1::new();
    let mut original_work = prefix;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut consumer = SyntheticConsumer {
            budget: &mut budget,
        };
        pending.prepare_into(
            &input.function,
            &mut consumer,
            &mut owned,
            &mut original_work,
            |block, scratch, _| {
                actual_transfer(input, block, scratch)?;
                if block == 3 && mode != 0 {
                    if mode == 1 {
                        return Err(Error::Unavailable("synthetic transfer error"));
                    }
                    std::panic::panic_any(());
                }
                Ok(())
            },
        )
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    let observation = Probe {
        result,
        phase: pending.phase,
        entries: pending.entries.clone(),
        reached: pending.reached.clone(),
        scratch: pending.scratch.clone(),
        queue: pending.queue.clone(),
        successors: pending.successors.clone(),
        visits: pending.visits.clone(),
        head: pending.head,
        current: pending.current,
        original_work,
        work: budget.work(),
        live: budget.storage(),
        peak: budget.peak_storage(),
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    };
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_work(), observation.failed_work);
    assert_eq!(budget.failed_storage(), observation.failed_storage);
    observation
}
#[test]
fn original_complete_states_fifo_visits_and_legacy_work_match() {
    for (kind, expected_order) in [
        (0, vec![0, 1, 2]),
        (1, vec![0, 1, 2, 3, 4, 4]),
        (2, vec![4, 2, 0, 3, 1, 1]),
    ] {
        let input = inputs(kind);
        let mut old_work = 0;
        let (expected, original_visits) = original(&input, &mut old_work).unwrap();
        assert_eq!(original_visits, expected_order);
        let mut unchanged_work = 0;
        let unchanged = propagate_capability_dataflow_v1(
            &input.callables,
            &input.function,
            &input.dominance,
            &input.allocations,
            &input.constants,
            &input.pipeline_owners,
            &input.pipeline_payloads,
            input.function.entry().index() as usize,
            &mut unchanged_work,
        )
        .unwrap();
        assert_eq!(expected, unchanged);
        assert_eq!(old_work, unchanged_work);
        let p = probe(&input, LIMIT, LIMIT, 0, 0);
        assert_eq!(p.result, Ok(()));
        assert_eq!(p.phase, Phase::Complete);
        assert_eq!(p.visits, original_visits);
        assert_eq!(p.original_work, old_work);
        assert!(p.work > p.original_work);
        compare_data(
            &expected,
            CompletedFifoPassV1 {
                entries: &p.entries,
                reached: &p.reached,
                visits: &p.visits,
            },
            input.function.locals().len(),
        );
    }
}
#[test]
fn changed_existing_value_enqueues_even_without_an_added_key() {
    let input = inputs(1);
    let p = probe(&input, LIMIT, LIMIT, 0, 0);
    assert_eq!(p.result, Ok(()));
    assert_eq!(p.visits, vec![0, 1, 2, 3, 4, 4]);
    assert_eq!(
        p.entries[4 * input.function.locals().len() + 1],
        Some(ProjectedCapabilityValueV1::Invalid)
    );
    assert_eq!(p.queue, p.visits);
    assert_eq!(p.head, p.queue.len());
}
#[test]
fn sorted_unique_successors_keep_duplicate_raw_edge_work_and_full_merge_precharge() {
    let terminator = SemanticTerminatorKindV1::SwitchInt {
        discriminant: constant(0),
        targets: SemanticSwitchTargetsV1::new(
            vec![
                SemanticSwitchTargetV1::new(0, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3)),
                SemanticSwitchTargetV1::new(1, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2)),
                SemanticSwitchTargetV1::new(2, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3)),
            ],
            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
        )
        .unwrap(),
    };
    let mut old_work = 0;
    let expected = charged_unique_capability_successors_v1(&terminator, 2, &mut old_work).unwrap();
    assert_eq!(expected, vec![2, 3]);
    assert_eq!(old_work, 4 + 2 * 3);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut next = Vec::new();
    let mut actual_work = 0;
    collect_successors(
        &terminator,
        2,
        &mut next,
        &mut SyntheticConsumer {
            budget: &mut budget,
        },
        &mut owned,
        &mut actual_work,
    )
    .unwrap();
    assert_eq!(next, expected);
    assert_eq!(actual_work, old_work);
    drop(next);
    budget.release_storage(owned).unwrap();
}
#[test]
fn real_original_semantic_work_limit_refuses_at_same_prefix() {
    let input = inputs(1);
    let prefix = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1;
    let mut expected_work = prefix;
    let expected = original(&input, &mut expected_work).unwrap_err();
    let p = probe(&input, LIMIT, LIMIT, 0, prefix);
    assert_eq!(p.result, Err(transfer_error(expected)));
    assert_eq!(p.original_work, expected_work);
    assert_eq!(p.visits, vec![0]);
    assert_eq!(p.phase, Phase::Terminal);
    assert!(p.failed_work.is_none() && p.failed_storage.is_none());
}
#[test]
fn exact_and_one_short_budget_limits_keep_partial_owned_graph() {
    let input = inputs(1);
    let full = probe(&input, LIMIT, LIMIT, 0, 0);
    let exact = probe(&input, full.work, full.peak, 0, 0);
    assert_eq!(exact.result, Ok(()));
    assert_eq!((exact.work, exact.peak), (full.work, full.peak));
    let work = probe(&input, full.work - 1, full.peak, 0, 0);
    assert!(work.result.is_err() && work.failed_work.is_some() && work.failed_storage.is_none());
    let storage = probe(&input, full.work, full.peak - 1, 0, 0);
    assert!(
        storage.result.is_err()
            && storage.failed_storage.is_some()
            && storage.failed_work.is_none()
    );
    assert!(!work.entries.is_empty() && !storage.entries.is_empty());
    assert!(work.live > FLOOR && storage.live > FLOOR);
}
#[test]
fn all_insufficient_work_prefixes_retain_accepted_tables_and_queue_progress() {
    let input = inputs(1);
    let full = probe(&input, LIMIT, LIMIT, 0, 0);
    let mut partial_tables = false;
    let mut partial_queue = false;
    let mut partial_successors = false;
    for limit in 0..full.work {
        let p = probe(&input, limit, LIMIT, 0, 0);
        assert!(p.result.is_err() && p.failed_work.is_some());
        assert_eq!(p.phase, Phase::Terminal);
        partial_tables |= !p.entries.is_empty() && p.reached.is_empty();
        partial_queue |= p.head > 0 && p.head < p.queue.len();
        partial_successors |= !p.successors.is_empty();
    }
    assert!(partial_tables && partial_queue && partial_successors);
}
#[test]
fn transfer_error_and_panic_preserve_entries_scratch_and_remaining_queue() {
    let input = inputs(1);
    for mode in [1, 2] {
        let p = probe(&input, LIMIT, LIMIT, mode, 0);
        assert_eq!(
            p.result,
            Err(if mode == 1 {
                Error::Unavailable("synthetic transfer error")
            } else {
                Error::CallbackPanicked
            })
        );
        assert_eq!(p.phase, Phase::Terminal);
        assert_eq!(p.current, Some(3));
        assert_eq!(p.visits, vec![0, 1, 2, 3]);
        assert!(p.queue.len() > p.head && !p.entries.is_empty() && !p.scratch.is_empty());
        assert!(p.live > FLOOR && p.failed_work.is_none() && p.failed_storage.is_none());
    }
}
#[test]
fn occupied_retry_preserves_data_pointers_without_further_debits() {
    let input = inputs(1);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut original_work = 0;
    let mut pending = RetainedCapabilityFifoPassV1::new();
    pending
        .prepare_into(
            &input.function,
            &mut SyntheticConsumer {
                budget: &mut budget,
            },
            &mut owned,
            &mut original_work,
            |block, slots, _| actual_transfer(&input, block, slots),
        )
        .unwrap();
    let pointers = (
        pending.entries.as_ptr(),
        pending.queue.as_ptr(),
        pending.visits.as_ptr(),
    );
    let before = (budget.work(), budget.storage(), original_work, owned);
    let mut entered = false;
    assert!(
        pending
            .prepare_into(
                &input.function,
                &mut SyntheticConsumer {
                    budget: &mut budget
                },
                &mut owned,
                &mut original_work,
                |_, _, _| {
                    entered = true;
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(!entered);
    assert_eq!(
        before,
        (budget.work(), budget.storage(), original_work, owned)
    );
    assert_eq!(
        pointers,
        (
            pending.entries.as_ptr(),
            pending.queue.as_ptr(),
            pending.visits.as_ptr()
        )
    );
    assert_eq!(pending.phase, Phase::Terminal);
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn detached_source_wrong_ledger_and_sticky_denial_cannot_borrow_complete_data() {
    let input = inputs(0);
    let detached = input.function.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut original_work = 0;
    let mut pending = RetainedCapabilityFifoPassV1::new();
    {
        let mut consumer = SyntheticConsumer {
            budget: &mut budget,
        };
        pending
            .prepare_into(
                &input.function,
                &mut consumer,
                &mut owned,
                &mut original_work,
                |block, slots, _| actual_transfer(&input, block, slots),
            )
            .unwrap();
        assert!(pending.completed_for(&input.function, &consumer).is_ok());
        assert!(pending.completed_for(&detached, &consumer).is_err());
    }
    let mut other_work = Work::new(LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    assert!(
        pending
            .completed_for(&input.function, &SyntheticConsumer { budget: &mut other })
            .is_err()
    );
    assert!(budget.charge_work(LIMIT).is_err());
    assert!(
        pending
            .completed_for(
                &input.function,
                &SyntheticConsumer {
                    budget: &mut budget
                }
            )
            .is_err()
    );
    let before = (budget.work(), budget.storage(), owned, budget.failed_work());
    let mut fresh = RetainedCapabilityFifoPassV1::new();
    assert!(
        fresh
            .prepare_into(
                &input.function,
                &mut SyntheticConsumer {
                    budget: &mut budget
                },
                &mut owned,
                &mut original_work,
                |_, _, _| Ok(())
            )
            .is_err()
    );
    assert!(fresh.entries.is_empty() && fresh.queue.is_empty());
    assert_eq!(
        before,
        (budget.work(), budget.storage(), owned, budget.failed_work())
    );
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn merge_denial_and_original_stored_limit_precede_row_mutation() {
    let known =
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::MatrixContext { root: 7 });
    for low_work in [true, false] {
        let mut work = Work::new(if low_work { 0 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut row = [Some(known), None];
        let incoming = [None, Some(known)];
        let mut stored = if low_work {
            1
        } else {
            MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1
        };
        let before = row;
        assert!(
            merge_original(
                &mut row,
                &incoming,
                &mut stored,
                &mut SyntheticConsumer {
                    budget: &mut budget
                }
            )
            .is_err()
        );
        assert_eq!(row, before);
    }
}
#[test]
fn explicit_frame_and_source_order_have_no_refund_topology_or_queue_coalescing() {
    let rows = fifo_frame_rows::<()>().unwrap();
    assert_eq!(rows.len(), FIFO_FRAME_ROWS);
    assert_eq!(rows.into_iter().sum::<usize>(), fifo_frame::<()>().unwrap());
    let text: String = include_str!("bf16_nominal_retained_capability_fifo_v1.rs")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(
        text.find("collect_successors(function.blocks()").unwrap()
            < text.find("forindexin0..self.successors.len()").unwrap()
    );
    assert!(text.contains("ifchanged{push(&mutself.queue,target,consumer,owned)?;}"));
    for forbidden in [
        "release_storage_v1(",
        "Budget::new",
        "Work::new",
        "order_cfg(",
        "queued[",
        "HashMap::new",
    ] {
        assert!(!text.contains(forbidden));
    }
}
