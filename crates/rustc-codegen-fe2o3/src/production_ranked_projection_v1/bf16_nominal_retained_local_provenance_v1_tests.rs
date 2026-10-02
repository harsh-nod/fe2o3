//! Synthetic provenance controls, not authenticated source admission.
use super::*;
use crate::production_ranked_projection_v1::exclusive_owner_carrier_v1::retained_carrier_frame_v1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;

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

const LIMIT: usize = 8 * 1024 * 1024;
const FLOOR: usize = 11;
#[derive(Debug)]
struct Probe {
    result: Result<()>,
    phase: Phase,
    data: LocalProvenanceV1,
    carrier_invoked: bool,
    carrier_complete: bool,
    queues: [bool; 3],
    edge_lengths: [usize; 3],
    work: usize,
    storage: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn inventory(f: &SemanticFunctionDeclV1) -> AssertionDefinitionInventoryV1 {
    assertion_definition_inventory_with_resources_v1(f, &mut Resources::unmetered()).unwrap()
}
fn probe(
    f: &SemanticFunctionDeclV1,
    calls: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    defs: &[u8],
    escaped: &[bool],
    work_limit: usize,
    storage_limit: usize,
    mode: usize,
) -> Probe {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut pending = RetainedLocalProvenanceV1::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(calls, types, f, defs, escaped, &mut resources)?;
        assert_eq!(
            pending
                .completed_for(calls, types, f, defs, escaped, &resources)?
                .allocation_origins
                .len(),
            f.locals().len()
        );
        match mode {
            0 => Ok(()),
            1 => Err(Error::Incomplete("provenance callback")),
            _ => std::panic::panic_any(()),
        }
    }));
    let result = match result {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Incomplete("provenance component panic"))
        }
    };
    let (carrier_complete, queues) = {
        let resources = Resources::new(&mut budget, &mut owned);
        (
            pending
                .carrier
                .completed_for(calls, f, defs, &resources)
                .is_ok(),
            [
                pending.stable_queue.completed(&resources),
                pending.allocation_queue.completed(&resources),
                pending.contract_queue.completed(&resources),
            ],
        )
    };
    assert_eq!(budget.storage(), FLOOR + owned);
    let observed = Probe {
        result,
        phase: pending.phase,
        data: pending.data.clone(),
        carrier_invoked: pending.carrier_invoked,
        carrier_complete,
        queues,
        edge_lengths: [
            pending.stable_edges.len(),
            pending.allocation_edges.len(),
            pending.allocation_contract_edges.len(),
        ],
        work: budget.work(),
        storage: budget.storage(),
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    };
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_work(), observed.failed_work);
    assert_eq!(budget.failed_storage(), observed.failed_storage);
    observed
}
fn original(
    f: &SemanticFunctionDeclV1,
    calls: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    defs: &[u8],
    escaped: &[bool],
) -> (Result<LocalProvenanceV1>, usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let result = local_provenance_with_resources_v1(
        calls,
        types,
        f,
        defs,
        escaped,
        &mut Resources::new(&mut budget, &mut owned),
    );
    let saved = match &result {
        Ok(data) => Ok(data.clone()),
        Err(Error::Unsupported(reason)) => Err(Error::Unsupported(reason)),
        Err(Error::Incomplete(reason)) => Err(Error::Incomplete(reason)),
        Err(other) => panic!("unexpected original fixture error: {other:?}"),
    };
    let work = budget.work();
    drop(result);
    assert_eq!(budget.storage(), owned);
    budget.release_storage(owned).unwrap();
    (saved, work, owned)
}
fn compare(
    f: &SemanticFunctionDeclV1,
    calls: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
) -> Probe {
    let i = inventory(f);
    let (expected, work, storage) = original(f, calls, types, &i.counts, &i.address_escaped);
    let expected = expected.unwrap();
    let p = probe(
        f,
        calls,
        types,
        &i.counts,
        &i.address_escaped,
        LIMIT,
        LIMIT,
        0,
    );
    assert!(p.result.is_ok());
    assert_eq!(p.data, expected);
    assert!(p.carrier_invoked && p.carrier_complete);
    assert_eq!(p.queues, [true; 3]);
    let carrier_queue = usize::from(
        f.abi()
            .source_argument_ownership()
            .contains(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner),
    );
    assert_eq!(p.work, work + 32 + 32 + 3 * 32 + carrier_queue * 32);
    let frames = retained_provenance_frame_v1().unwrap()
        + retained_carrier_frame_v1().unwrap()
        + (2 + carrier_queue) * retained_origin_worklist_frame_v1::<u32>().unwrap()
        + retained_origin_worklist_frame_v1::<LocalAllocationProvenanceV1>().unwrap();
    // Both paths now separately fund the exact-reborrow callee. The retained
    // frame roster contains that same callee once, not a second live invocation.
    assert_eq!(
        p.storage,
        FLOOR + storage + frames
            - bf16_nominal_source_algorithms_v1::shared_slice_reborrow_frame_v1()
    );
    p
}
#[test]
fn original_complete_provenance_data_and_debits_match_for_copy_move_chains() {
    let calls = callables();
    let types = projection_types();
    for value in [SemanticOperandV1::Copy(place(1)), operand(1)] {
        let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(value))]);
        let p = compare(&f, &calls, &types);
        assert_eq!(p.data.allocation_origins[3], Some(0));
        assert_eq!(
            p.data.allocation_provenance[3],
            Some(LocalAllocationProvenanceV1::Argument(0))
        );
        assert_eq!(p.edge_lengths, [f.locals().len(); 3]);
    }
    let f = ordinary(vec![
        assign(7, SemanticRvalueKindV1::Use(operand(1))),
        assign(2, SemanticRvalueKindV1::Use(operand(7))),
    ]);
    assert_eq!(
        compare(&f, &calls, &types).data.allocation_origins[3],
        Some(0)
    );
}
#[test]
fn actual_nonexclusive_abi_runs_carrier_then_all_three_propagations() {
    for ownership in [
        SemanticSourceArgumentOwnershipV1::RawPointer,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::Unspecified,
    ] {
        let f = function(
            vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            vec![borrow(3, 2)],
            vec![operand(3), constant(0)],
            ownership,
        );
        let p = compare(&f, &callables(), &projection_types());
        assert_eq!(p.data.allocation_origins[3], None);
        assert_eq!(
            p.data.allocation_provenance[3],
            Some(LocalAllocationProvenanceV1::Private(
                SemanticLocalIdV1::from_index(2)
            ))
        );
    }
}
#[test]
fn raw_argument_copy_private_borrow_and_pointer_offset_keep_distinct_tables() {
    let raw = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(6)))]);
    let p = compare(&raw, &callables(), &projection_types());
    assert_eq!(p.data.allocation_origins[2], Some(1));
    assert_eq!(p.data.allocation_origins[3], None);
    assert_eq!(
        p.data.allocation_provenance[3],
        Some(LocalAllocationProvenanceV1::Private(
            SemanticLocalIdV1::from_index(2)
        ))
    );
    let offset = ordinary(vec![assign(
        2,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Offset,
            left: operand(1),
            right: constant(0),
        },
    )]);
    let p = compare(&offset, &callables(), &projection_types());
    assert_eq!(p.data.allocation_origins[2], Some(0));
    assert_eq!(p.data.allocation_provenance[2], None);
}
#[test]
fn scalar_shape_error_precedes_actual_carrier_and_later_tables() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(99)))]);
    let calls = callables();
    let types = projection_types();
    let i = inventory(&f);
    for (defs, escaped) in [
        (&i.counts[..i.counts.len() - 1], &i.address_escaped[..]),
        (
            &i.counts[..],
            &i.address_escaped[..i.address_escaped.len() - 1],
        ),
    ] {
        let (expected, work, storage) = original(&f, &calls, &types, defs, escaped);
        assert!(matches!(
            expected,
            Err(Error::Unsupported(
                "local provenance scalar custody tables do not match the semantic local table"
            ))
        ));
        let p = probe(&f, &calls, &types, defs, escaped, LIMIT, LIMIT, 0);
        assert!(matches!(
            p.result,
            Err(Error::Unsupported(
                "local provenance scalar custody tables do not match the semantic local table"
            ))
        ));
        assert!(!p.carrier_invoked && !p.carrier_complete);
        assert!(p.data.stable_argument_origins.is_empty());
        assert_eq!(p.work, work + 32);
        assert_eq!(
            p.storage,
            FLOOR + storage + retained_provenance_frame_v1().unwrap()
                - bf16_nominal_source_algorithms_v1::shared_slice_reborrow_frame_v1()
        );
    }
}
#[test]
fn original_bad_carrier_use_precedes_provenance_array_initialization() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(99)))]);
    let calls = callables();
    let types = projection_types();
    let i = inventory(&f);
    let (expected, _, _) = original(&f, &calls, &types, &i.counts, &i.address_escaped);
    assert!(matches!(
        expected,
        Err(Error::Unsupported(
            "ExclusiveOwner carrier use is outside the local table"
        ))
    ));
    let p = probe(
        &f,
        &calls,
        &types,
        &i.counts,
        &i.address_escaped,
        LIMIT,
        LIMIT,
        0,
    );
    assert!(matches!(
        p.result,
        Err(Error::Unsupported(
            "ExclusiveOwner carrier use is outside the local table"
        ))
    ));
    assert!(p.carrier_invoked && !p.carrier_complete);
    assert!(
        p.data.stable_argument_origins.is_empty()
            && p.data.allocation_origins.is_empty()
            && p.data.allocation_provenance.is_empty()
    );
    assert_eq!(p.queues, [false; 3]);
}
#[test]
fn exact_and_one_short_resources_keep_pending_partial_state() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let types = projection_types();
    let i = inventory(&f);
    let p = compare(&f, &calls, &types);
    assert!(
        probe(
            &f,
            &calls,
            &types,
            &i.counts,
            &i.address_escaped,
            p.work,
            p.storage,
            0
        )
        .result
        .is_ok()
    );
    for (w, s) in [(p.work - 1, p.storage), (p.work, p.storage - 1)] {
        let p = probe(&f, &calls, &types, &i.counts, &i.address_escaped, w, s, 0);
        assert!(p.result.is_err() && (p.failed_work.is_some() || p.failed_storage.is_some()));
        assert_eq!(p.phase, Phase::Terminal);
        assert_eq!(p.data.allocation_origins.len(), f.locals().len());
        assert_eq!(p.edge_lengths, [f.locals().len(); 3]);
    }
}
#[test]
fn every_work_boundary_observes_partial_results_without_credit_refund() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let types = projection_types();
    let i = inventory(&f);
    let full = compare(&f, &calls, &types);
    let mut partial_table = false;
    let mut partial_edges = false;
    for w in 0..full.work {
        let p = probe(
            &f,
            &calls,
            &types,
            &i.counts,
            &i.address_escaped,
            w,
            LIMIT,
            0,
        );
        assert!(p.result.is_err() && p.failed_work.is_some());
        assert_eq!(p.phase, Phase::Terminal);
        partial_table |=
            !p.data.stable_argument_origins.is_empty() && p.data.allocation_origins.is_empty();
        partial_edges |= p.edge_lengths[0] > 0 && p.edge_lengths[1] == 0;
    }
    assert!(partial_table && partial_edges);
}
#[test]
fn complete_nested_owners_survive_component_success_error_and_panic() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let types = projection_types();
    let i = inventory(&f);
    for mode in 0..3 {
        let p = probe(
            &f,
            &calls,
            &types,
            &i.counts,
            &i.address_escaped,
            LIMIT,
            LIMIT,
            mode,
        );
        assert_eq!(p.result.is_ok(), mode == 0);
        assert_eq!(p.phase, Phase::Complete);
        assert!(p.carrier_invoked && p.carrier_complete);
        assert_eq!(p.queues, [true; 3]);
    }
}
#[test]
fn retry_wrong_ledger_detached_inputs_and_sticky_denial_cannot_lend_data() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let types = projection_types();
    let i = inventory(&f);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedLocalProvenanceV1::new();
    pending
        .prepare_into(
            &calls,
            &types,
            &f,
            &i.counts,
            &i.address_escaped,
            &mut Resources::new(&mut budget, &mut owned),
        )
        .unwrap();
    let detached = f.clone();
    let other_types = types.clone();
    let other_escaped = i.address_escaped.clone();
    for (t, f, e) in [
        (&types[..], &detached, &i.address_escaped[..]),
        (&other_types[..], &f, &i.address_escaped[..]),
        (&types[..], &f, &other_escaped[..]),
    ] {
        assert!(
            pending
                .completed_for(
                    &calls,
                    t,
                    f,
                    &i.counts,
                    e,
                    &Resources::new(&mut budget, &mut owned)
                )
                .is_err()
        );
    }
    let mut other_work = Work::new(LIMIT);
    let mut other_budget = Budget::new(&mut other_work, LIMIT);
    let mut other_owned = 0;
    assert!(
        pending
            .completed_for(
                &calls,
                &types,
                &f,
                &i.counts,
                &i.address_escaped,
                &Resources::new(&mut other_budget, &mut other_owned)
            )
            .is_err()
    );
    let before = (
        budget.work(),
        owned,
        pending.data.allocation_origins.as_ptr(),
        pending.stable_edges.as_ptr(),
    );
    assert!(
        pending
            .prepare_into(
                &calls,
                &types,
                &f,
                &i.counts,
                &i.address_escaped,
                &mut Resources::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert_eq!(
        before,
        (
            budget.work(),
            owned,
            pending.data.allocation_origins.as_ptr(),
            pending.stable_edges.as_ptr()
        )
    );
    assert_eq!(pending.phase, Phase::Terminal);
    assert!(budget.charge_work(LIMIT).is_err());
    let before = (budget.work(), owned);
    let mut fresh = RetainedLocalProvenanceV1::new();
    assert!(
        fresh
            .prepare_into(
                &calls,
                &types,
                &f,
                &i.counts,
                &i.address_escaped,
                &mut Resources::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert_eq!(before, (budget.work(), owned));
    drop(fresh);
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn unmetered_component_refusal_and_unchanged_source_order_are_explicit() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let types = projection_types();
    let i = inventory(&f);
    let mut pending = RetainedLocalProvenanceV1::new();
    assert!(
        pending
            .prepare_into(
                &calls,
                &types,
                &f,
                &i.counts,
                &i.address_escaped,
                &mut Resources::unmetered()
            )
            .is_err()
    );
    assert_eq!(pending.phase, Phase::Terminal);
    let source = include_str!("bf16_nominal_retained_local_provenance_v1.rs");
    let start = source.find("fn prepare_attached(").unwrap();
    let end = source.find("fn source_key(").unwrap();
    let body: String = source[start..end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut position = 0;
    for needle in [
        "resources.reserve_storage(",
        "ifdefinitions.len()!=local_count||address_escaped.len()!=local_count",
        "self.carrier_invoked=true;",
        "self.carrier.prepare_into(",
        "self.carrier.completed_for(",
        "fill_attached(&mutself.data.stable_argument_origins,",
        "fill_attached(&mutself.data.allocation_origins,",
        "fill_attached(&mutself.data.allocation_provenance,",
        "nested_attached(&mutself.stable_edges,",
        "nested_attached(&mutself.allocation_edges,",
        "nested_attached(&mutself.allocation_contract_edges,",
        "prepay_provenance_spines_v1(",
        "self.stable_queue.prepare_into(",
        "self.allocation_queue.prepare_into(",
        "self.contract_queue.prepare_into(",
    ] {
        position += body[position..].find(needle).unwrap() + needle.len();
    }
    assert!(!body.contains("unmetered("));
    assert!(!body.contains("Budget::new("));
    assert_eq!(
        retained_provenance_frame_v1().unwrap(),
        typed_rows().unwrap().into_iter().sum::<usize>()
    );
}

#[test]
fn second_result_table_storage_denial_preserves_first_table_and_carrier_owner() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let types = projection_types();
    let i = inventory(&f);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut original_owned = 0;
    let result = crate::production_ranked_projection_v1::exclusive_owner_carrier_v1::origins_with_limit_and_resources(
        &calls, &f, &i.counts, MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1,
        &mut Resources::new(&mut budget, &mut original_owned)).unwrap();
    drop(result);
    budget.release_storage(original_owned).unwrap();
    let donor_header = size_of::<LocalProvenanceV1>()
        + 3 * size_of::<Vec<Vec<usize>>>()
        + size_of::<Vec<Option<u32>>>()
        + 4096;
    let carrier_storage = original_owned
        + retained_carrier_frame_v1().unwrap()
        + retained_origin_worklist_frame_v1::<u32>().unwrap();
    let limit = FLOOR
        + retained_provenance_frame_v1().unwrap()
        + donor_header
        + carrier_storage
        + f.locals().len() * size_of::<Option<u32>>();
    let p = probe(
        &f,
        &calls,
        &types,
        &i.counts,
        &i.address_escaped,
        LIMIT,
        limit,
        0,
    );
    assert!(p.result.is_err() && p.failed_storage.is_some());
    assert!(p.carrier_invoked);
    assert_eq!(p.phase, Phase::Terminal);
    assert_eq!(p.data.stable_argument_origins.len(), f.locals().len());
    assert!(p.data.allocation_origins.is_empty() && p.data.allocation_provenance.is_empty());
    assert_eq!(p.edge_lengths, [0; 3]);
}
