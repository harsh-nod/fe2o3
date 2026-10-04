//! Synthetic allocation component controls; no constructor grants source authority.
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

fn neutral_pointer_type_v1(
    tag: u8,
    pointee: SemanticTypeIdV1,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    validity_start: u128,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(validity_start, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                kind,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

#[derive(Clone)]
struct Argument {
    ownership: SemanticSourceArgumentOwnershipV1,
    pointee: Option<SemanticAbiPointeeKindV1>,
    mode: SemanticAbiPassModeV1,
}
fn attributes(noalias: bool) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(noalias, None, false, false, false, false),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap()
}
fn arg(
    ownership: SemanticSourceArgumentOwnershipV1,
    pointee: Option<SemanticAbiPointeeKindV1>,
    noalias: bool,
) -> Argument {
    Argument {
        ownership,
        pointee,
        mode: SemanticAbiPassModeV1::Direct(attributes(noalias)),
    }
}
fn allocation_fixture(
    arguments: &[Argument],
    fallback: Option<SemanticAbiPointeeKindV1>,
) -> (
    SemanticFunctionDeclV1,
    Vec<SemanticTypeDeclV1>,
    Vec<Option<u32>>,
) {
    let mut types = projection_types();
    types[POINTER_TYPE.index() as usize] = neutral_pointer_type_v1(
        242,
        SCALAR_TYPE,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Mutable,
        0,
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            fallback.map(|kind| SemanticAbiPointeeInfoV1::new(kind, 0, 1).unwrap()),
            None,
        ),
    );
    let values = arguments
        .iter()
        .map(|argument| {
            let value = SemanticAbiValueV1::new(POINTER_TYPE, argument.mode.clone());
            match argument.pointee {
                Some(kind) => {
                    value.with_pointee_override(SemanticAbiPointeeInfoV1::new(kind, 0, 1).unwrap())
                }
                None => value,
            }
        })
        .collect();
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(243)),
        SemanticLayoutIdentityV1::from_sha256(bytes(244)),
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        values,
        SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(
        arguments
            .iter()
            .map(|argument| argument.ownership)
            .collect(),
    )
    .unwrap();
    let mut locals = vec![local(245, SCALAR_TYPE, SemanticLocalRoleV1::Return)];
    let mut origins = vec![None];
    for index in 0..arguments.len() {
        locals.push(local(
            100 + index as u8,
            POINTER_TYPE,
            SemanticLocalRoleV1::Argument(index as u32),
        ));
        origins.push(Some(index as u32));
    }
    locals.push(local(246, POINTER_TYPE, SemanticLocalRoleV1::Temporary));
    origins.push(if arguments.is_empty() { None } else { Some(0) });
    locals.push(local(247, POINTER_TYPE, SemanticLocalRoleV1::Temporary));
    origins.push(Some(99));
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(248)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(249)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(250)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(251)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(252)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block(253, vec![], SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    (function, types, origins)
}
fn fixture() -> (
    SemanticFunctionDeclV1,
    Vec<SemanticTypeDeclV1>,
    Vec<Option<u32>>,
) {
    allocation_fixture(
        &[
            arg(
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                Some(SemanticAbiPointeeKindV1::Raw),
                false,
            ),
            arg(
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                Some(SemanticAbiPointeeKindV1::SharedReference { frozen: true }),
                false,
            ),
        ],
        None,
    )
}
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 11;
#[derive(Debug)]
struct Probe {
    result: Result<()>,
    phase: Phase,
    arguments: Vec<Option<AllocationContractV1>>,
    rows: Vec<Option<AllocationContractV1>>,
    entered: bool,
    work: usize,
    live: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn probe(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    origins: &[Option<u32>],
    work_limit: usize,
    storage_limit: usize,
    mode: usize,
) -> Probe {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut pending = RetainedAllocationContractsV1::new();
    let mut entered = false;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(types, function, origins, &mut resources)?;
        assert_eq!(
            pending
                .completed_for(types, function, origins, &resources)?
                .len(),
            function.locals().len()
        );
        entered = true;
        match mode {
            0 => Ok(()),
            1 => Err(Error::Incomplete("allocation component callback control")),
            _ => std::panic::panic_any(()),
        }
    }));
    let result = match outcome {
        Ok(value) => value,
        Err(payload) => {
            drop(payload);
            Err(Error::Incomplete("allocation component panic control"))
        }
    };
    let observed = Probe {
        result,
        phase: pending.phase,
        arguments: pending.arguments.clone(),
        rows: pending.result.clone(),
        entered,
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
    assert_eq!(budget.failed_work(), observed.failed_work);
    assert_eq!(budget.failed_storage(), observed.failed_storage);
    observed
}
fn original(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    origins: &[Option<u32>],
) -> (Result<Vec<Option<AllocationContractV1>>>, usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let result = local_allocation_contracts_with_resources_v1(
        types,
        function,
        origins,
        &mut Resources::new(&mut budget, &mut owned),
    );
    let observed_work = budget.work();
    assert_eq!(budget.storage(), owned);
    drop(result);
    budget.release_storage(owned).unwrap();
    // Separate unchanged original invocation supplies synthetic expected DATA.
    // This helper is outside the retained component's resource protocol.
    let expected = local_allocation_contracts_with_resources_v1(
        types,
        function,
        origins,
        &mut Resources::unmetered(),
    );
    (expected, observed_work, owned)
}
fn compare(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    origins: &[Option<u32>],
) -> Probe {
    let (expected, work, storage) = original(function, types, origins);
    let observed = probe(function, types, origins, LIMIT, LIMIT, 0);
    assert_eq!(observed.work, work + 32);
    assert_eq!(
        observed.live,
        FLOOR + storage + retained_allocation_frame_v1().unwrap()
    );
    match expected {
        Ok(rows) => {
            assert!(observed.result.is_ok());
            assert_eq!(observed.rows, rows);
            assert!(observed.entered);
        }
        Err(error) => {
            match (observed.result.as_ref(), error) {
                (Err(Error::Unsupported(actual)), Error::Unsupported(expected)) => {
                    assert_eq!(*actual, expected);
                }
                (actual, expected) => {
                    panic!("original allocation error differs: {actual:?} versus {expected:?}")
                }
            }
            assert!(!observed.entered);
        }
    }
    observed
}

#[test]
fn complete_contract_data_and_original_debits_match_after_explicit_prefix() {
    let (f, t, o) = fixture();
    let p = compare(&f, &t, &o);
    assert_eq!(p.phase, Phase::Complete);
    assert_eq!(p.arguments.len(), 2);
    assert_eq!(
        p.rows[1],
        Some(AllocationContractV1 {
            allocation_origin: 1,
            noalias_class: 2,
            writable: true,
            singleton_object: true
        })
    );
    assert_eq!(
        p.rows[2],
        Some(AllocationContractV1 {
            allocation_origin: 2,
            noalias_class: 1,
            writable: false,
            singleton_object: false
        })
    );
    assert_eq!(p.rows[3], p.rows[1]);
    assert_eq!(p.rows[0], None);
    assert_eq!(p.rows[4], None); // Original out-of-range origin remains absent, not a new refusal.
}
#[test]
fn each_supported_pointee_and_ownership_policy_matches_original() {
    for argument in [
        arg(
            SemanticSourceArgumentOwnershipV1::RawPointer,
            Some(SemanticAbiPointeeKindV1::Raw),
            false,
        ),
        arg(
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            Some(SemanticAbiPointeeKindV1::Raw),
            false,
        ),
        arg(
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            Some(SemanticAbiPointeeKindV1::SharedReference { frozen: true }),
            false,
        ),
        arg(
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            Some(SemanticAbiPointeeKindV1::SharedReference { frozen: false }),
            false,
        ),
        arg(
            SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            Some(SemanticAbiPointeeKindV1::MutableReference { unpin: true }),
            true,
        ),
        arg(
            SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            Some(SemanticAbiPointeeKindV1::Box {
                unpin: true,
                global: true,
            }),
            true,
        ),
        arg(
            SemanticSourceArgumentOwnershipV1::ByValue,
            Some(SemanticAbiPointeeKindV1::Raw),
            false,
        ),
    ] {
        let (f, t, o) = allocation_fixture(&[argument], None);
        assert!(compare(&f, &t, &o).result.is_ok());
    }
}
#[test]
fn pointee_override_precedes_type_fallback_and_absent_pointee_stays_absent() {
    let (f, t, o) = allocation_fixture(
        &[arg(
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            Some(SemanticAbiPointeeKindV1::Raw),
            false,
        )],
        Some(SemanticAbiPointeeKindV1::SharedReference { frozen: true }),
    );
    assert_eq!(compare(&f, &t, &o).rows[1].unwrap().noalias_class, 2);
    let (f, t, o) = allocation_fixture(
        &[arg(
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            None,
            false,
        )],
        Some(SemanticAbiPointeeKindV1::SharedReference { frozen: true }),
    );
    assert!(!compare(&f, &t, &o).rows[1].unwrap().writable);
    let (f, t, o) = allocation_fixture(
        &[arg(
            SemanticSourceArgumentOwnershipV1::Unspecified,
            None,
            false,
        )],
        None,
    );
    assert!(compare(&f, &t, &o).rows.iter().all(Option::is_none));
}
#[test]
fn pair_first_noalias_and_ignore_policy_preserve_original_classification() {
    let mut argument = arg(
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        Some(SemanticAbiPointeeKindV1::MutableReference { unpin: true }),
        false,
    );
    argument.mode = SemanticAbiPassModeV1::Pair {
        first: attributes(true),
        second: attributes(false),
    };
    let (f, t, o) = allocation_fixture(&[argument.clone()], None);
    assert!(compare(&f, &t, &o).result.is_ok());
    argument.mode = SemanticAbiPassModeV1::Pair {
        first: attributes(false),
        second: attributes(true),
    };
    let (f, t, o) = allocation_fixture(&[argument.clone()], None);
    assert!(matches!(
        compare(&f, &t, &o).result,
        Err(Error::Unsupported(
            "source ownership disagrees with rustc ABI pointer provenance"
        ))
    ));
    argument.mode = SemanticAbiPassModeV1::Ignore;
    let (f, t, o) = allocation_fixture(&[argument], None);
    assert!(compare(&f, &t, &o).result.is_err());
}
#[test]
fn original_shape_error_precedes_missing_type_and_retains_no_arrays() {
    let (f, _, o) = fixture();
    let p = compare(&f, &[], &o[..o.len() - 1]);
    assert!(matches!(
        p.result,
        Err(Error::Unsupported(
            "allocation-origin and semantic-local tables have different lengths"
        ))
    ));
    assert!(p.arguments.is_empty() && p.rows.is_empty());
    let p = compare(&f, &[], &o);
    assert!(matches!(
        p.result,
        Err(Error::Unsupported(
            "a kernel argument type is outside the semantic type table"
        ))
    ));
    assert_eq!(p.arguments, vec![None, None]);
    assert!(p.rows.is_empty());
}
#[test]
fn second_argument_refusal_retains_first_exact_contract_without_result_rows() {
    let (f, t, o) = allocation_fixture(
        &[
            arg(
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                Some(SemanticAbiPointeeKindV1::Raw),
                false,
            ),
            arg(
                SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                Some(SemanticAbiPointeeKindV1::Raw),
                false,
            ),
        ],
        None,
    );
    let p = compare(&f, &t, &o);
    assert!(matches!(
        p.result,
        Err(Error::Unsupported(
            "source ownership disagrees with rustc ABI pointer provenance"
        ))
    ));
    assert!(p.arguments[0].is_some() && p.arguments[1].is_none() && p.rows.is_empty());
    assert_eq!(p.phase, Phase::Terminal);
}
#[test]
fn exact_and_one_short_limits_preserve_partial_custody_and_first_denial() {
    let (f, t, o) = fixture();
    let full = probe(&f, &t, &o, LIMIT, LIMIT, 0);
    let exact = probe(&f, &t, &o, full.work, full.peak, 0);
    assert!(exact.result.is_ok());
    assert_eq!((exact.work, exact.peak), (full.work, full.peak));
    let work = probe(&f, &t, &o, full.work - 1, full.peak, 0);
    assert!(work.failed_work.is_some() && work.failed_storage.is_none() && !work.entered);
    assert!(work.arguments.iter().all(Option::is_some));
    assert!(work.rows.len() < o.len());
    let storage = probe(&f, &t, &o, full.work, full.peak - 1, 0);
    assert!(storage.failed_storage.is_some() && storage.failed_work.is_none() && !storage.entered);
    assert!(storage.arguments.iter().all(Option::is_some) && storage.rows.is_empty());
}
#[test]
fn every_insufficient_work_limit_keeps_all_accepted_partial_arrays() {
    let (f, t, o) = fixture();
    let full = probe(&f, &t, &o, LIMIT, LIMIT, 0);
    let mut argument_partial = false;
    let mut result_partial = false;
    for limit in 0..full.work {
        let p = probe(&f, &t, &o, limit, LIMIT, 0);
        assert!(p.result.is_err() && p.failed_work.is_some() && !p.entered);
        assert_eq!(p.phase, Phase::Terminal);
        argument_partial |=
            p.arguments.first().is_some_and(Option::is_some) && p.arguments.get(1) == Some(&None);
        result_partial |= !p.rows.is_empty() && p.rows.len() < o.len();
    }
    assert!(argument_partial && result_partial);
}
#[test]
fn complete_owner_survives_callback_success_error_and_panic() {
    let (f, t, o) = fixture();
    for mode in 0..3 {
        let p = probe(&f, &t, &o, LIMIT, LIMIT, mode);
        assert_eq!(p.phase, Phase::Complete);
        assert!(p.entered && p.rows.len() == o.len() && p.arguments.iter().all(Option::is_some));
        assert!(p.failed_work.is_none() && p.failed_storage.is_none());
        match mode {
            0 => assert!(p.result.is_ok()),
            1 => assert!(matches!(
                p.result,
                Err(Error::Incomplete("allocation component callback control"))
            )),
            _ => assert!(matches!(
                p.result,
                Err(Error::Incomplete("allocation component panic control"))
            )),
        }
    }
}
#[test]
fn retry_does_not_replace_arguments_or_result_and_is_terminal_without_debit() {
    let (f, t, o) = fixture();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedAllocationContractsV1::new();
    pending
        .prepare_into(&t, &f, &o, &mut Resources::new(&mut budget, &mut owned))
        .unwrap();
    let pointers = (pending.arguments.as_ptr(), pending.result.as_ptr());
    let saved = (budget.work(), budget.storage(), owned);
    {
        let mut resources = Resources::new(&mut budget, &mut owned);
        assert!(pending.prepare_into(&t, &f, &o, &mut resources).is_err());
        assert!(pending.completed_for(&t, &f, &o, &resources).is_err());
    }
    assert_eq!(pending.phase, Phase::Terminal);
    assert_eq!(
        pointers,
        (pending.arguments.as_ptr(), pending.result.as_ptr())
    );
    assert_eq!(saved, (budget.work(), budget.storage(), owned));
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn detached_function_types_origins_wrong_ledger_and_denial_cannot_lend_data() {
    let (f, t, o) = fixture();
    let f2 = f.clone();
    let t2 = t.clone();
    let o2 = o.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedAllocationContractsV1::new();
    {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(&t, &f, &o, &mut resources).unwrap();
        assert!(pending.completed_for(&t, &f2, &o, &resources).is_err());
        assert!(pending.completed_for(&t2, &f, &o, &resources).is_err());
        assert!(pending.completed_for(&t, &f, &o2, &resources).is_err());
    }
    let mut other_work = Work::new(LIMIT);
    let mut other_budget = Budget::new(&mut other_work, LIMIT);
    let mut other_owned = 0;
    assert!(
        pending
            .completed_for(
                &t,
                &f,
                &o,
                &Resources::new(&mut other_budget, &mut other_owned)
            )
            .is_err()
    );
    assert!(budget.charge_work(LIMIT).is_err());
    assert!(
        pending
            .completed_for(&t, &f, &o, &Resources::new(&mut budget, &mut owned))
            .is_err()
    );
    let saved = (budget.work(), budget.storage(), owned, budget.failed_work());
    let mut fresh = RetainedAllocationContractsV1::new();
    assert!(
        fresh
            .prepare_into(&t, &f, &o, &mut Resources::new(&mut budget, &mut owned))
            .is_err()
    );
    assert!(fresh.arguments.is_empty() && fresh.result.is_empty());
    assert_eq!(
        saved,
        (budget.work(), budget.storage(), owned, budget.failed_work())
    );
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn unmetered_refusal_empty_arguments_and_typed_checked_sum_remain_explicit() {
    let (f, t, o) = fixture();
    let mut pending = RetainedAllocationContractsV1::new();
    assert!(
        pending
            .prepare_into(&t, &f, &o, &mut Resources::unmetered())
            .is_err()
    );
    assert_eq!(pending.phase, Phase::Terminal);
    assert!(pending.arguments.is_empty() && pending.result.is_empty());
    let (f, t, o) = allocation_fixture(&[], None);
    let p = compare(&f, &t, &o);
    assert!(p.entered && p.arguments.is_empty() && p.rows.iter().all(Option::is_none));
    let rows = typed_rows().unwrap();
    assert_eq!(rows.len(), FRAME_ROWS);
    assert_eq!(
        rows.into_iter().sum::<usize>(),
        retained_allocation_frame_v1().unwrap()
    );
}
#[test]
fn donor_order_and_no_refund_or_new_ledger_are_visible() {
    let text: String = include_str!("bf16_nominal_retained_allocation_contracts_v1.rs")
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    let body = text
        .split("fnprepare_attached(")
        .nth(1)
        .unwrap()
        .split("fnsource_key(")
        .next()
        .unwrap();
    let header = body.find("resources.reserve_storage(").unwrap();
    let shape = body
        .find("iforigins.len()!=function.locals().len()")
        .unwrap();
    let fill = body.find("fill_attached(&mutself.arguments,").unwrap();
    let policy = body
        .find("authenticated_source_allocation_contract_v1(")
        .unwrap();
    let result = body
        .find("resources.reserve(&mutself.result,origins.len())")
        .unwrap();
    let push = body.find("resources.push(&mutself.result,").unwrap();
    assert!(header < shape && shape < fill && fill < policy && policy < result && result < push);
    for forbidden in ["Budget::new", "Work::new", "release_storage(", "unmetered("] {
        assert!(!body.contains(forbidden));
    }
}

#[test]
fn new_frame_original_header_and_first_array_denial_retain_exact_empty_state() {
    let (f, t, o) = fixture();
    let frame = retained_allocation_frame_v1().unwrap();
    let header = 2 * std::mem::size_of::<Vec<Option<AllocationContractV1>>>() + 4096;
    for limit in [
        FLOOR + frame - 1,
        FLOOR + frame + header - 1,
        FLOOR + frame + header,
    ] {
        let p = probe(&f, &t, &o, LIMIT, limit, 0);
        assert!(p.failed_storage.is_some() && p.failed_work.is_none() && !p.entered);
        assert_eq!(p.phase, Phase::Terminal);
        assert!(p.arguments.is_empty() && p.rows.is_empty());
    }
    let p = probe(&f, &t, &o, 31, LIMIT, 0);
    assert!(p.failed_work.is_some() && p.failed_storage.is_none());
    assert_eq!(p.live, FLOOR);
    assert!(p.arguments.is_empty() && p.rows.is_empty());
}
