//! Synthetic carrier controls; copied constructors do not confer source authority.
use super::*;
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

const LIMIT: usize = 8 * 1024 * 1024;
const FLOOR: usize = 11;
#[derive(Debug)]
struct Probe {
    result: Result<()>,
    phase: Phase,
    origins: Vec<Option<u32>>,
    scan_invoked: bool,
    bad_carrier_len: usize,
    bad_receiver_len: usize,
    receiver_uses_len: usize,
    copies_len: usize,
    borrows_len: usize,
    propagation_complete: bool,
    census: usize,
    work: usize,
    storage: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn definitions(function: &SemanticFunctionDeclV1) -> Vec<u8> {
    crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::assertion_definition_inventory_with_resources_v1(function, &mut Resources::unmetered()).unwrap().counts
}
fn probe(
    function: &SemanticFunctionDeclV1,
    calls: &[SemanticCallableDeclV1],
    definitions: &[u8],
    local_limit: usize,
    work_limit: usize,
    storage_limit: usize,
    mode: usize,
) -> Probe {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut pending = RetainedExclusiveCarrierV1::new();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(calls, function, definitions, local_limit, &mut resources)?;
        let (origins, _) = pending.completed_for(calls, function, definitions, &resources)?;
        assert_eq!(origins.len(), function.locals().len());
        match mode {
            0 => Ok(()),
            1 => Err(Error::Incomplete("carrier callback")),
            _ => std::panic::panic_any(()),
        }
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Incomplete("carrier component panic"))
        }
    };
    let propagation_complete = pending
        .propagation
        .completed(&Resources::new(&mut budget, &mut owned));
    assert_eq!(budget.storage(), FLOOR + owned);
    let observed = Probe {
        result,
        phase: pending.phase,
        origins: pending.origins.clone(),
        scan_invoked: pending.scan_invoked,
        bad_carrier_len: pending.scan.bad_carrier.len(),
        bad_receiver_len: pending.scan.bad_receiver.len(),
        receiver_uses_len: pending.scan.receiver_uses.len(),
        copies_len: pending.copies.len(),
        borrows_len: pending.borrows.len(),
        propagation_complete,
        census: if pending.scan_invoked {
            pending.scan.work
        } else {
            pending.initial_work
        },
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
    function: &SemanticFunctionDeclV1,
    calls: &[SemanticCallableDeclV1],
    definitions: &[u8],
    local_limit: usize,
) -> (Result<(Vec<Option<u32>>, usize)>, usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let result = origins_with_limit_and_resources(
        calls,
        function,
        definitions,
        local_limit,
        &mut Resources::new(&mut budget, &mut owned),
    );
    let work = budget.work();
    // Clone only for synthetic comparison, drop the charged original before refund.
    let saved = match &result {
        Ok((data, census)) => Ok((data.clone(), *census)),
        Err(Error::Unsupported(reason)) => Err(Error::Unsupported(reason)),
        Err(Error::Incomplete(reason)) => Err(Error::Incomplete(reason)),
        Err(other) => panic!("unexpected original fixture error: {other:?}"),
    };
    drop(result);
    assert_eq!(budget.storage(), owned);
    budget.release_storage(owned).unwrap();
    (saved, work, owned)
}
fn compare(function: &SemanticFunctionDeclV1, calls: &[SemanticCallableDeclV1]) -> Probe {
    let definitions = definitions(function);
    let (original, work, storage) = original(function, calls, &definitions, usize::MAX);
    let expected = original.unwrap();
    let p = probe(function, calls, &definitions, expected.1, LIMIT, LIMIT, 0);
    assert!(p.result.is_ok());
    assert_eq!(p.origins, expected.0);
    assert_eq!(p.census, expected.1);
    let queue = usize::from(p.scan_invoked);
    assert_eq!(p.work, work + 32 + queue * 32);
    assert_eq!(
        p.storage,
        FLOOR
            + storage
            + retained_carrier_frame_v1().unwrap()
            + queue * retained_origin_worklist_frame_v1::<u32>().unwrap()
    );
    assert_eq!(p.propagation_complete, p.scan_invoked);
    p
}
#[test]
fn retained_copy_move_and_transitive_carrier_data_match_original() {
    for value in [SemanticOperandV1::Copy(place(1)), operand(1)] {
        let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(value))]);
        assert_eq!(compare(&f, &callables()).origins[2], Some(0));
    }
    let f = ordinary(vec![
        assign(7, SemanticRvalueKindV1::Use(operand(1))),
        assign(2, SemanticRvalueKindV1::Use(operand(7))),
    ]);
    let p = compare(&f, &callables());
    assert_eq!(p.origins[2], Some(0));
    assert_eq!(p.bad_carrier_len, f.locals().len());
    assert_eq!(p.bad_receiver_len, f.locals().len());
    assert_eq!(p.receiver_uses_len, f.locals().len());
    assert_eq!(p.copies_len, f.locals().len());
    assert!(p.borrows_len > 0 && p.propagation_complete);
}
#[test]
fn actual_nonexclusive_abi_takes_unchanged_zero_origin_fast_path() {
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
        let p = compare(&f, &callables());
        assert!(p.origins.iter().all(Option::is_none));
        assert!(!p.scan_invoked && !p.propagation_complete);
        assert_eq!(
            (
                p.bad_carrier_len,
                p.bad_receiver_len,
                p.receiver_uses_len,
                p.copies_len,
                p.borrows_len
            ),
            (0, 0, 0, 0, 0)
        );
        assert_eq!(
            p.census,
            f.locals().len() + f.abi().source_argument_ownership().len()
        );
    }
}
#[test]
fn unknown_receiver_mutation_alias_and_late_escape_match_original_refusals() {
    for f in [
        ordinary(vec![
            assign(2, SemanticRvalueKindV1::Use(operand(1))),
            assign(2, SemanticRvalueKindV1::Use(operand(6))),
        ]),
        ordinary(vec![
            assign(1, SemanticRvalueKindV1::Use(operand(6))),
            assign(2, SemanticRvalueKindV1::Use(operand(1))),
        ]),
        ordinary(vec![
            assign(7, SemanticRvalueKindV1::Use(operand(1))),
            assign(2, SemanticRvalueKindV1::Use(operand(7))),
            assign(
                4,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: place(7),
                },
            ),
        ]),
        function(
            vec![
                assign(2, SemanticRvalueKindV1::Use(operand(1))),
                borrow(4, 2),
            ],
            call(1, vec![operand(4)], 1),
            vec![borrow(3, 2)],
            vec![operand(3), constant(0)],
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        ),
        function(
            vec![
                assign(2, SemanticRvalueKindV1::Use(operand(1))),
                borrow(4, 2),
                assign(7, SemanticRvalueKindV1::Use(operand(4))),
            ],
            call(0, vec![operand(4), constant(0)], 1),
            vec![borrow(3, 2)],
            vec![operand(3), constant(0)],
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        ),
    ] {
        assert_eq!(compare(&f, &callables()).origins[2], None);
    }
}
#[test]
fn receiver_position_missing_callable_and_projected_copy_match_original() {
    for arguments in [vec![constant(0), operand(3)], vec![operand(3), operand(3)]] {
        let f = function(
            vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            vec![borrow(3, 2)],
            arguments,
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        );
        assert_eq!(compare(&f, &callables()).origins[2], None);
    }
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    assert_eq!(compare(&f, &[]).origins[2], None);
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), POINTER_TYPE).unwrap()],
        POINTER_TYPE,
    )
    .unwrap();
    let f = ordinary(vec![assign(
        2,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
    )]);
    assert_eq!(compare(&f, &callables()).origins[2], None);
}
#[test]
fn original_local_census_limit_and_definition_shape_error_priority_remain() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let defs = definitions(&f);
    let baseline = compare(&f, &calls);
    let p = probe(&f, &calls, &defs, baseline.census - 1, LIMIT, LIMIT, 0);
    assert!(matches!(
        p.result,
        Err(Error::Unsupported(
            "ExclusiveOwner carrier census exceeds the projection work limit"
        ))
    ));
    let (expected, _, _) = original(&f, &calls, &defs, baseline.census - 1);
    assert!(matches!(
        expected,
        Err(Error::Unsupported(
            "ExclusiveOwner carrier census exceeds the projection work limit"
        ))
    ));
    assert!(!p.propagation_complete);
    assert!(p.failed_work.is_none() && p.failed_storage.is_none());
    let p = probe(&f, &calls, &defs[..defs.len() - 1], 0, LIMIT, LIMIT, 0);
    let (expected, work, storage) = original(&f, &calls, &defs[..defs.len() - 1], 0);
    assert!(matches!(
        expected,
        Err(Error::Unsupported(
            "ExclusiveOwner carrier definitions do not match the local table"
        ))
    ));
    assert!(matches!(
        p.result,
        Err(Error::Unsupported(
            "ExclusiveOwner carrier definitions do not match the local table"
        ))
    ));
    assert_eq!((work, storage), (0, 0));
    assert_eq!(
        (p.work, p.storage),
        (32, FLOOR + retained_carrier_frame_v1().unwrap())
    );
    assert!(p.origins.is_empty() && !p.scan_invoked);
}
#[test]
fn exact_and_one_short_budgets_retain_partial_owner() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let defs = definitions(&f);
    let full = compare(&f, &calls);
    assert!(
        probe(&f, &calls, &defs, full.census, full.work, full.storage, 0)
            .result
            .is_ok()
    );
    for (w, s) in [(full.work - 1, full.storage), (full.work, full.storage - 1)] {
        let p = probe(&f, &calls, &defs, full.census, w, s, 0);
        assert!(p.result.is_err());
        assert_eq!(p.phase, Phase::Terminal);
        assert!(p.failed_work.is_some() || p.failed_storage.is_some());
        assert_eq!(p.origins.len(), f.locals().len());
        assert!(p.borrows_len > 0);
    }
}
#[test]
fn every_work_boundary_and_partial_scan_array_storage_keep_custody() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let defs = definitions(&f);
    let full = compare(&f, &calls);
    let mut partial_scan = false;
    let mut retained_borrows = false;
    for w in 0..full.work {
        let p = probe(&f, &calls, &defs, usize::MAX, w, LIMIT, 0);
        assert!(p.result.is_err() && p.failed_work.is_some());
        assert_eq!(p.phase, Phase::Terminal);
        partial_scan |= p.bad_carrier_len > 0 && p.bad_receiver_len == 0;
        retained_borrows |= p.borrows_len > 0;
    }
    assert!(partial_scan && retained_borrows);
    let n = f.locals().len();
    let limit = FLOOR
        + retained_carrier_frame_v1().unwrap()
        + carrier_frame_storage_v1().unwrap()
        + n * size_of::<Option<u32>>()
        + n * size_of::<bool>();
    let p = probe(&f, &calls, &defs, usize::MAX, LIMIT, limit, 0);
    assert!(p.result.is_err() && p.failed_storage.is_some());
    assert_eq!(
        (
            p.origins.len(),
            p.bad_carrier_len,
            p.bad_receiver_len,
            p.receiver_uses_len
        ),
        (n, n, 0, 0)
    );
}
#[test]
fn owners_survive_component_success_error_panic_and_original_bad_use() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let defs = definitions(&f);
    for mode in 0..3 {
        let p = probe(&f, &calls, &defs, usize::MAX, LIMIT, LIMIT, mode);
        assert_eq!(p.result.is_ok(), mode == 0);
        assert_eq!(p.phase, Phase::Complete);
        assert!(p.propagation_complete && p.borrows_len > 0);
    }
    let bad = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(99)))]);
    let defs = definitions(&bad);
    let (expected, _, _) = original(&bad, &calls, &defs, usize::MAX);
    assert!(matches!(
        expected,
        Err(Error::Unsupported(
            "ExclusiveOwner carrier use is outside the local table"
        ))
    ));
    let p = probe(&bad, &calls, &defs, usize::MAX, LIMIT, LIMIT, 0);
    assert!(matches!(
        p.result,
        Err(Error::Unsupported(
            "ExclusiveOwner carrier use is outside the local table"
        ))
    ));
    assert_eq!(p.phase, Phase::Terminal);
    assert_eq!(p.bad_carrier_len, bad.locals().len());
}
#[test]
fn occupied_retry_identity_mismatch_sticky_denial_and_unmetered_refuse() {
    let f = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
    let calls = callables();
    let defs = definitions(&f);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedExclusiveCarrierV1::new();
    pending
        .prepare_into(
            &calls,
            &f,
            &defs,
            usize::MAX,
            &mut Resources::new(&mut budget, &mut owned),
        )
        .unwrap();
    let detached = f.clone();
    let other_defs = defs.clone();
    let other_calls = calls.clone();
    for (c, function, d) in [
        (&calls[..], &detached, &defs[..]),
        (&calls[..], &f, &other_defs[..]),
        (&other_calls[..], &f, &defs[..]),
    ] {
        assert!(
            pending
                .completed_for(c, function, d, &Resources::new(&mut budget, &mut owned))
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
                &f,
                &defs,
                &Resources::new(&mut other_budget, &mut other_owned)
            )
            .is_err()
    );
    let before = (
        budget.work(),
        owned,
        pending.origins.as_ptr(),
        pending.scan.bad_carrier.as_ptr(),
        pending.borrows.as_ptr(),
    );
    assert!(
        pending
            .prepare_into(
                &calls,
                &f,
                &defs,
                usize::MAX,
                &mut Resources::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert_eq!(
        before,
        (
            budget.work(),
            owned,
            pending.origins.as_ptr(),
            pending.scan.bad_carrier.as_ptr(),
            pending.borrows.as_ptr()
        )
    );
    assert_eq!(pending.phase, Phase::Terminal);
    assert!(budget.charge_work(LIMIT).is_err());
    let before = (budget.work(), owned);
    let mut fresh = RetainedExclusiveCarrierV1::new();
    assert!(
        fresh
            .prepare_into(
                &calls,
                &f,
                &defs,
                usize::MAX,
                &mut Resources::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert_eq!(before, (budget.work(), owned));
    drop(fresh);
    drop(pending);
    budget.release_storage(owned).unwrap();
    let mut fresh = RetainedExclusiveCarrierV1::new();
    assert!(
        fresh
            .prepare_into(&calls, &f, &defs, usize::MAX, &mut Resources::unmetered())
            .is_err()
    );
    assert_eq!(fresh.phase, Phase::Terminal);
}
#[test]
fn source_order_attached_scan_and_borrowed_census_are_visible() {
    let source = include_str!("retained_exclusive_owner_carrier_v1.rs");
    let start = source.find("fn prepare_attached(").unwrap();
    let end = source.find("fn fill_attached").unwrap();
    let body: String = source[start..end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut position = 0;
    for needle in [
        "ifdefinitions.len()!=count",
        "resources.reserve_storage(carrier_frame_storage_v1()?)?;",
        "fill_attached(&mutself.origins,",
        "self.scan_invoked=true;",
        "self.scan.prepare_retained_into(",
        "nested_attached(&mutself.copies,",
        "for(carrier,receiver)inself.borrows.iter().copied()",
        "for(source,edges)inself.copies.iter_mut().enumerate()",
        "self.propagation.prepare_into(",
    ] {
        position += body[position..].find(needle).unwrap() + needle.len();
    }
    assert!(!body.contains("CarrierScan::new("));
    assert!(!body.contains("inself.borrows{"));
    assert!(!body.contains("Budget::new("));
    assert!(!body.contains("unmetered("));
    assert_eq!(
        retained_carrier_frame_v1().unwrap(),
        typed_rows().unwrap().into_iter().sum::<usize>()
    );
}

#[test]
fn long_transparent_spine_keeps_original_prepaid_walk_and_local_census() {
    fn make(count: usize) -> SemanticFunctionDeclV1 {
        let projections = (0..count)
            .map(|index| {
                SemanticProjectionV1::new(
                    if index % 2 == 0 {
                        SemanticProjectionKindV1::Field(0)
                    } else {
                        SemanticProjectionKindV1::OpaqueCast
                    },
                    POINTER_TYPE,
                )
                .unwrap()
            })
            .collect();
        let source =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, POINTER_TYPE)
                .unwrap();
        ordinary(vec![assign(
            2,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)),
        )])
    }
    let calls = callables();
    let short = compare(&make(1), &calls);
    let long = compare(&make(257), &calls);
    assert!(long.origins.iter().all(Option::is_none));
    assert_eq!(long.census - short.census, 256);
    assert_eq!(long.work - short.work, 512);
    assert_eq!(long.storage, short.storage);
}
#[test]
fn long_copy_graph_preserves_exact_census_and_one_short_local_refusal() {
    let mut statements = vec![assign(7, SemanticRvalueKindV1::Use(operand(1)))];
    for index in 8..72 {
        statements.push(assign(index, SemanticRvalueKindV1::Use(operand(index - 1))));
    }
    statements.push(assign(2, SemanticRvalueKindV1::Use(operand(71))));
    let base = ordinary(statements);
    let mut locals = base.locals().to_vec();
    for index in 8..72 {
        locals.push(local(
            index as u8,
            POINTER_TYPE,
            SemanticLocalRoleV1::Temporary,
        ));
    }
    let f = with_parts(&base, base.abi().clone(), locals);
    let calls = callables();
    let p = compare(&f, &calls);
    assert_eq!(p.origins[2], Some(0));
    assert!(p.census < 30 * f.locals().len());
    let defs = definitions(&f);
    let refused = probe(&f, &calls, &defs, p.census - 1, LIMIT, LIMIT, 0);
    assert!(matches!(
        refused.result,
        Err(Error::Unsupported(
            "ExclusiveOwner carrier census exceeds the projection work limit"
        ))
    ));
    assert!(refused.failed_work.is_none() && refused.failed_storage.is_none());
}
