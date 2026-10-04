use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Diagnostic,
    DiagnosticLocation, Function, FunctionBody, FunctionId, FunctionRole, IntrinsicOperation,
    MatrixElement, MatrixOperation, MemoryElementType, MemoryIntrinsicOperation, ModuleId,
    ScalarType, Signature, Terminator, ValueDef, VolatileAccessContract,
};

// A single block, one borrowed parameter definition and no operation results:
// block fill1, three B/O header visits3, definition fill1, terminal census1.
const INPUT_STATE_WORK: usize = 6;
const INPUT_STATE_STORAGE: usize = 3 + 6;
const ONE_DEFINITION_QUERY_WORK: usize = 1 + 1 + 1;
// Count1, source visit + two publications3, exact lookup3, completion1.
const ONE_OPERAND_ROSTER_WORK: usize = 1 + 3 + ONE_DEFINITION_QUERY_WORK + 1;
const ONE_OPERAND_ROSTER_STORAGE: usize = 2;
// Fixed rows hold owned String descriptors; visible identifier bytes are separate.
const DIAGNOSTIC_ROW_STORAGE: usize =
    std::mem::size_of::<Diagnostic>().div_ceil(std::mem::size_of::<usize>());

#[test]
fn registered_coordinate_scratch_has_exact_limits_and_cleanup_after_work_denial() {
    for (work_limit, storage_limit, rejected) in [
        (6, 4_103, None),
        (5, 4_103, Some("work")),
        (6, 4_102, Some("storage")),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(7).unwrap();
        let location = location(0);
        let mut diagnostics = VerificationDiagnosticCollectorV1::count();
        {
            let mut sink = RegisteredOperationIssueSinkV1 {
                location: location.borrowed_v1(),
                diagnostics: &mut diagnostics,
                budget: &mut budget,
            };
            let result = MatrixOperationIssueSinkV1::allocate_coordinates(&mut sink, 2048);
            match rejected {
                None => {
                    let coordinates = result.unwrap();
                    assert_eq!(coordinates.len(), 0);
                    assert!(coordinates.capacity() >= 2048);
                    assert_eq!(sink.budget.storage(), 4_103);
                    assert!(matches!(
                        sink.budget.charge_work(1),
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(_))
                    ));
                    MatrixOperationIssueSinkV1::release_coordinates(&mut sink, coordinates, 2048)
                        .unwrap();
                    assert_eq!(sink.budget.work(), 6);
                    assert_eq!(sink.budget.peak_storage(), 4_103);
                }
                Some("work") => {
                    assert!(matches!(result,
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                            if error.actual() == 6 && error.limit() == 5));
                    assert_eq!(sink.budget.work(), 0);
                }
                Some("storage") => {
                    assert!(matches!(result,
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                            if error.actual() == 4_103 && error.limit() == 4_102));
                    assert_eq!(sink.budget.work(), 6);
                }
                Some(_) => unreachable!(),
            }
            assert_eq!(sink.budget.storage(), 7);
        }
        diagnostics.abandon(&mut budget).unwrap();
        assert_eq!(budget.storage(), 7);
    }
}

fn input_function(ty: Type) -> Function {
    Function {
        id: FunctionId::new("input"),
        signature: Signature::new(vec![ty], vec![]),
        role: FunctionRole::InternalHelper,
        body: Some(FunctionBody {
            parameters: vec![ValueId(0)],
            blocks: vec![BasicBlock {
                id: BlockId(0),
                parameters: vec![],
                operations: vec![],
                terminator: Some(Terminator::Return { values: vec![] }),
            }],
        }),
        required_capabilities: BTreeSet::new(),
    }
}

fn pointer_type() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    )
}

fn location(name_capacity: usize) -> DiagnosticLocation {
    named_location("module", "function", name_capacity)
}

fn named_location(
    module_name: &str,
    function_name: &str,
    name_capacity: usize,
) -> DiagnosticLocation {
    let mut module = String::with_capacity(name_capacity);
    module.push_str(module_name);
    let mut function = String::with_capacity(name_capacity);
    function.push_str(function_name);
    DiagnosticLocation {
        module: ModuleId::from(module),
        function: Some(FunctionId::from(function)),
        kernel: None,
        block: Some(BlockId(0)),
        operation: Some(0),
    }
}

fn load(pointer: ValueId, result: bool) -> Operation {
    let element = MemoryElementType::Scalar(ScalarType::U32);
    Operation {
        results: if result {
            vec![ValueDef::new(ValueId(9), element.ir_type())]
        } else {
            vec![]
        },
        kind: OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
            pointer,
            element,
            address_space: AddressSpace::Global,
            layout: element.expected_layout(),
            contract: VolatileAccessContract::rust_allocation_load(),
        }),
    }
}

fn assert_work_error(
    result: Result<bool, CanonicalKernelIrVerificationResourceErrorV1>,
    actual: usize,
    limit: usize,
) {
    assert!(matches!(
        result,
        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
            if error.actual() == actual && error.limit() == limit
    ));
}

#[test]
fn registered_valid_load_has_independent_exact_work_storage_and_one_under_limits() {
    // Generic shape: three arity checks + one result row + one type node.
    // Payload: operation/layout/contract/pointer/type visits5 + pointer fields3.
    const SHAPE_WORK: usize = 3 + 1 + 1;
    const PAYLOAD_WORK: usize = 5 + 3;
    const EXACT_WORK: usize =
        INPUT_STATE_WORK + 1 + ONE_OPERAND_ROSTER_WORK + SHAPE_WORK + PAYLOAD_WORK;
    const EXACT_STORAGE: usize = INPUT_STATE_STORAGE + ONE_OPERAND_ROSTER_STORAGE;
    let function = input_function(pointer_type());
    let operation = load(ValueId(0), true);
    for (work_limit, storage_limit) in [
        (EXACT_WORK, EXACT_STORAGE),
        (EXACT_WORK - 1, EXACT_STORAGE),
        (EXACT_WORK, EXACT_STORAGE - 1),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
        let state = VerificationFunctionStateV1::build(&function, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(budget.work(), INPUT_STATE_WORK);
        assert_eq!(budget.storage(), INPUT_STATE_STORAGE);
        let mut diagnostics = VerificationDiagnosticCollectorV1::count();
        let result = try_verify_registered_operation_with_resources_v1(
            &operation,
            &location(8).borrowed_v1(),
            &state,
            None,
            &mut diagnostics,
            &mut budget,
        );
        if work_limit < EXACT_WORK {
            assert_work_error(result, EXACT_WORK, work_limit);
            assert_eq!(budget.work(), EXACT_WORK - 3);
        } else if storage_limit < EXACT_STORAGE {
            assert!(matches!(
                result,
                Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                    if error.actual() == EXACT_STORAGE && error.limit() == storage_limit
            ));
        } else {
            assert_eq!(result, Ok(true));
            assert_eq!(budget.work(), EXACT_WORK);
            assert_eq!(budget.peak_storage(), EXACT_STORAGE);
        }
        assert_eq!(diagnostics.counted(), Some(0));
        assert_eq!(budget.storage(), INPUT_STATE_STORAGE);
        state.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn registered_undefined_operand_preserves_the_prior_ssa_diagnostic_boundary() {
    // An unknown operand is a present None in the borrowed roster. The
    // enclosing SSA verifier diagnoses it; registered payload checking must
    // not add an incorrect "requires a pointer operand" finding.
    const SHAPE_WORK: usize = 5;
    const PAYLOAD_WITH_UNKNOWN_WORK: usize = 4;
    const EXACT_WORK: usize =
        INPUT_STATE_WORK + 1 + ONE_OPERAND_ROSTER_WORK + SHAPE_WORK + PAYLOAD_WITH_UNKNOWN_WORK;
    let function = input_function(pointer_type());
    let operation = load(ValueId(u32::MAX), true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut work,
        INPUT_STATE_STORAGE + ONE_OPERAND_ROSTER_STORAGE,
    );
    let state = VerificationFunctionStateV1::build(&function, &mut budget)
        .unwrap()
        .unwrap();
    let mut diagnostics = VerificationDiagnosticCollectorV1::count();
    assert_eq!(
        try_verify_registered_operation_with_resources_v1(
            &operation,
            &location(8).borrowed_v1(),
            &state,
            None,
            &mut diagnostics,
            &mut budget,
        ),
        Ok(true)
    );
    assert_eq!(diagnostics.counted(), Some(0));
    assert_eq!(budget.work(), EXACT_WORK);
    assert_eq!(budget.storage(), INPUT_STATE_STORAGE);
    state.release(&mut budget).unwrap();
}

#[test]
fn registered_operand_roster_borrows_deep_types_and_repeated_value_ids_exactly() {
    const OPERANDS: usize = 3;
    const ROSTER_WORK: usize = OPERANDS * (1 + 3 + ONE_DEFINITION_QUERY_WORK) + 1;
    const EXACT_WORK: usize = INPUT_STATE_WORK + ROSTER_WORK;
    const EXACT_STORAGE: usize = INPUT_STATE_STORAGE + 2 * OPERANDS;
    for depth in [1, 32, 64] {
        let mut ty = Type::INDEX;
        for _ in 0..depth {
            ty = Type::pointer(ty, AddressSpace::Global, AccessMode::ReadWrite);
        }
        let mut function = input_function(ty);
        function.signature.parameters.reserve(4_096);
        let element = MemoryElementType::Scalar(ScalarType::U32);
        let operation = Operation {
            results: vec![],
            kind: OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(0),
                destination: ValueId(0),
                count: ValueId(0),
                element,
                source_address_space: AddressSpace::Global,
                destination_address_space: AddressSpace::Global,
                layout: element.expected_layout(),
                contract: crate::CopyNonOverlappingContract::supported_rust(),
            }),
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, EXACT_STORAGE);
        let state = VerificationFunctionStateV1::build(&function, &mut budget)
            .unwrap()
            .unwrap();
        let roster = RegisteredOperandRosterV1::build(&operation, &state, &mut budget).unwrap();
        assert_eq!(roster.operands, [ValueId(0); OPERANDS]);
        for actual in &roster.types {
            assert!(std::ptr::eq(
                actual.unwrap(),
                &function.signature.parameters[0]
            ));
        }
        assert_eq!(budget.work(), EXACT_WORK);
        assert_eq!(budget.peak_storage(), EXACT_STORAGE);
        roster.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), INPUT_STATE_STORAGE);
        state.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn registered_result_diagnostic_preserves_code_message_and_exact_two_pass_boundaries() {
    const MESSAGE: &str = "operation defines 0 results but 1 are required";
    const MESSAGE_UPPER: usize = 512;
    const IDENTIFIER_STORAGE: usize = 6 + 8;
    const OWNED_LOCATION_WORK: usize = 5 + IDENTIFIER_STORAGE + 2 * 2;
    const ISSUE_MAPPING: usize = 1;
    const BORROWED_LOCATION_COPY: usize = 5;
    const COLLECTOR_ROSTER: usize = 1;
    const SHAPE_WITHOUT_COMPARISONS: usize = 3;
    const PAYLOAD_WORK: usize = 8;
    const BASE_PASS: usize = 1 + ONE_OPERAND_ROSTER_WORK + SHAPE_WITHOUT_COMPARISONS + PAYLOAD_WORK;
    const COUNT_PASS: usize =
        BASE_PASS + ISSUE_MAPPING + BORROWED_LOCATION_COPY + COLLECTOR_ROSTER + MESSAGE_UPPER;
    const MATERIALIZE_EXTRA: usize = OWNED_LOCATION_WORK + MESSAGE_UPPER + MESSAGE.len() + 1;
    const FINISH_WORK: usize = 1;
    const EXACT_WORK: usize = INPUT_STATE_WORK + 2 * COUNT_PASS + MATERIALIZE_EXTRA + FINISH_WORK;
    const EXACT_STORAGE: usize = INPUT_STATE_STORAGE
        + DIAGNOSTIC_ROW_STORAGE
        + ONE_OPERAND_ROSTER_STORAGE
        + IDENTIFIER_STORAGE
        + MESSAGE.len();

    let function = input_function(pointer_type());
    let operation = load(ValueId(0), false);
    for name_capacity in [8, 16_384] {
        for (work_limit, storage_limit) in [
            (EXACT_WORK, EXACT_STORAGE),
            (EXACT_WORK - 1, EXACT_STORAGE),
            (EXACT_WORK, EXACT_STORAGE - 1),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
            let state = VerificationFunctionStateV1::build(&function, &mut budget)
                .unwrap()
                .unwrap();
            let mut count = VerificationDiagnosticCollectorV1::count();
            assert_eq!(
                try_verify_registered_operation_with_resources_v1(
                    &operation,
                    &location(name_capacity).borrowed_v1(),
                    &state,
                    None,
                    &mut count,
                    &mut budget,
                ),
                Ok(true)
            );
            assert_eq!(count.counted(), Some(1));
            assert_eq!(budget.work(), INPUT_STATE_WORK + COUNT_PASS);
            assert_eq!(budget.storage(), INPUT_STATE_STORAGE);

            let mut materialized =
                VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
            let result = try_verify_registered_operation_with_resources_v1(
                &operation,
                &location(name_capacity).borrowed_v1(),
                &state,
                None,
                &mut materialized,
                &mut budget,
            );
            if storage_limit < EXACT_STORAGE {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                        if error.actual() == EXACT_STORAGE && error.limit() == storage_limit
                ));
                assert_eq!(
                    budget.storage(),
                    INPUT_STATE_STORAGE + DIAGNOSTIC_ROW_STORAGE
                );
                materialized.abandon(&mut budget).unwrap();
            } else {
                assert_eq!(result, Ok(true));
                let finished = materialized.finish_materialized(&mut budget);
                if work_limit < EXACT_WORK {
                    assert!(matches!(
                        finished,
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                            if error.actual() == EXACT_WORK && error.limit() == work_limit
                    ));
                    assert_eq!(budget.work(), EXACT_WORK - 1);
                } else {
                    assert_eq!(
                        finished.unwrap(),
                        vec![Diagnostic {
                            location: location(name_capacity),
                            code: DiagnosticCode::ResultArity,
                            message: MESSAGE.to_owned(),
                        }]
                    );
                    assert_eq!(budget.work(), EXACT_WORK);
                    assert_eq!(budget.peak_storage(), EXACT_STORAGE);
                }
            }
            assert_eq!(budget.storage(), INPUT_STATE_STORAGE);
            state.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    }
}

#[test]
fn registered_materialization_owns_visible_identifiers_with_exact_resource_boundaries() {
    const MESSAGE: &str = "mapped";
    const MESSAGE_UPPER: usize = 6;
    const BORROWED_LOCATION_COPY: usize = 5;
    const COUNT_WORK: usize = BORROWED_LOCATION_COPY + 1 + MESSAGE_UPPER;
    const FINISH_WORK: usize = 1;
    for (identifier_bytes, spare_capacity) in [
        (0, 16_384),
        (1, 0),
        (1, 16_384),
        (4_096, 0),
        (4_096, 16_384),
    ] {
        let module_name = "m".repeat(identifier_bytes);
        let function_name = "f".repeat(identifier_bytes);
        // Two present identifiers, including present empty identifiers.
        let identifier_storage = 2 * identifier_bytes;
        let owned_location_work = 5 + identifier_storage + 2 * 2;
        let exact_work =
            2 * COUNT_WORK + owned_location_work + MESSAGE_UPPER + MESSAGE.len() + 1 + FINISH_WORK;
        let exact_storage = DIAGNOSTIC_ROW_STORAGE + identifier_storage + MESSAGE.len();
        assert_eq!(exact_work, 47 + 2 * identifier_bytes);
        for (work_limit, storage_limit) in [
            (exact_work, exact_storage),
            (exact_work - 1, exact_storage),
            (exact_work, exact_storage - 1),
        ] {
            let source = named_location(
                &module_name,
                &function_name,
                identifier_bytes + spare_capacity,
            );
            if spare_capacity != 0 {
                assert!(source.module.retained_capacity_bytes() > identifier_bytes);
                assert!(
                    source.function.as_ref().unwrap().retained_capacity_bytes() > identifier_bytes
                );
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
            let mut count = VerificationDiagnosticCollectorV1::count();
            emit_registered_diagnostic_v1(
                &source.borrowed_v1(),
                DiagnosticCode::TypeMismatch,
                MESSAGE_UPPER,
                format_args!("{MESSAGE}"),
                &mut count,
                &mut budget,
            )
            .unwrap();
            assert_eq!(count.counted(), Some(1));
            assert_eq!((budget.work(), budget.storage()), (COUNT_WORK, 0));
            count.abandon(&mut budget).unwrap();

            let mut materialized =
                VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
            let result = emit_registered_diagnostic_v1(
                &source.borrowed_v1(),
                DiagnosticCode::TypeMismatch,
                MESSAGE_UPPER,
                format_args!("{MESSAGE}"),
                &mut materialized,
                &mut budget,
            );
            if storage_limit < exact_storage {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                        if error.actual() == exact_storage && error.limit() == storage_limit
                ));
                assert_eq!(budget.work(), exact_work - FINISH_WORK);
                assert_eq!(budget.storage(), DIAGNOSTIC_ROW_STORAGE);
                assert_eq!(budget.peak_storage(), DIAGNOSTIC_ROW_STORAGE);
                assert_eq!(budget.failed_storage(), Some(exact_storage));
                materialized.abandon(&mut budget).unwrap();
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(budget.storage(), exact_storage);
                let finished = materialized.finish_materialized(&mut budget);
                if work_limit < exact_work {
                    assert!(matches!(
                        finished,
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                            if error.actual() == exact_work && error.limit() == work_limit
                    ));
                    assert_eq!(budget.work(), exact_work - FINISH_WORK);
                } else {
                    let diagnostics = finished.unwrap();
                    assert_eq!(diagnostics.len(), 1);
                    let owned = &diagnostics[0].location;
                    assert_eq!(owned, &source);
                    assert_eq!(owned.module.retained_capacity_bytes(), identifier_bytes);
                    assert_eq!(
                        owned.function.as_ref().unwrap().retained_capacity_bytes(),
                        identifier_bytes
                    );
                    if identifier_bytes != 0 {
                        assert_ne!(
                            owned.module.as_str().as_ptr(),
                            source.module.as_str().as_ptr()
                        );
                        assert_ne!(
                            owned.function.as_ref().unwrap().as_str().as_ptr(),
                            source.function.as_ref().unwrap().as_str().as_ptr()
                        );
                    }
                    drop(source);
                    assert_eq!(owned.module.as_str(), module_name);
                    assert_eq!(owned.function.as_ref().unwrap().as_str(), function_name);
                    assert_eq!(diagnostics[0].code, DiagnosticCode::TypeMismatch);
                    assert_eq!(diagnostics[0].message, MESSAGE);
                    assert_eq!(budget.work(), exact_work);
                }
                assert_eq!(budget.peak_storage(), exact_storage);
                assert_eq!(budget.failed_storage(), None);
            }
            assert_eq!(budget.storage(), 0);
            assert_eq!(
                work.failed_work(),
                (work_limit < exact_work).then_some(exact_work)
            );
        }
    }
}

#[test]
fn registered_dispatch_checks_capabilities_before_returning_to_legacy() {
    const MESSAGE: &str = "target does not support required capability WorkgroupMemory";
    const VISIT_PREFLIGHT: usize = 1;
    const CAPABILITY_PUBLICATION: usize = 1;
    const EMPTY_SUPPORT_QUERY: usize = 1;
    const BORROWED_LOCATION_COPY: usize = 5;
    const COLLECTOR_ROSTER: usize = 1;
    const MESSAGE_UPPER: usize = 128;
    const DISPATCH: usize = 1;
    const COUNT_PASS: usize = VISIT_PREFLIGHT
        + CAPABILITY_PUBLICATION
        + EMPTY_SUPPORT_QUERY
        + BORROWED_LOCATION_COPY
        + COLLECTOR_ROSTER
        + MESSAGE_UPPER
        + DISPATCH;
    const IDENTIFIER_STORAGE: usize = 6 + 8;
    const OWNED_LOCATION_WORK: usize = 5 + IDENTIFIER_STORAGE + 2 * 2;
    const EXACT_WORK: usize = INPUT_STATE_WORK
        + 2 * COUNT_PASS
        + OWNED_LOCATION_WORK
        + MESSAGE_UPPER
        + MESSAGE.len()
        + 1
        + 1;
    const EXACT_STORAGE: usize =
        INPUT_STATE_STORAGE + DIAGNOSTIC_ROW_STORAGE + IDENTIFIER_STORAGE + MESSAGE.len();
    let function = input_function(Type::INDEX);
    let operation = Operation {
        results: vec![],
        kind: OperationKind::Alloca {
            element: Type::INDEX,
            count: None,
            address_space: AddressSpace::Workgroup,
            alignment: 8,
        },
    };
    let supported = BTreeSet::new();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, EXACT_STORAGE);
    let state = VerificationFunctionStateV1::build(&function, &mut budget)
        .unwrap()
        .unwrap();
    let mut count = VerificationDiagnosticCollectorV1::count();
    assert_eq!(
        try_verify_registered_operation_with_resources_v1(
            &operation,
            &location(8).borrowed_v1(),
            &state,
            Some(&supported),
            &mut count,
            &mut budget,
        ),
        Ok(false)
    );
    assert_eq!(count.counted(), Some(1));
    assert_eq!(budget.work(), INPUT_STATE_WORK + COUNT_PASS);
    let mut materialized = VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
    assert_eq!(
        try_verify_registered_operation_with_resources_v1(
            &operation,
            &location(8).borrowed_v1(),
            &state,
            Some(&supported),
            &mut materialized,
            &mut budget,
        ),
        Ok(false)
    );
    assert_eq!(
        materialized.finish_materialized(&mut budget).unwrap(),
        vec![Diagnostic {
            location: location(8),
            code: DiagnosticCode::UnsupportedCapability,
            message: MESSAGE.to_owned(),
        }]
    );
    assert_eq!(budget.work(), EXACT_WORK);
    assert_eq!(budget.peak_storage(), EXACT_STORAGE);
    state.release(&mut budget).unwrap();

    // A non-extension requirement never traverses unrelated candidate names.
    for bytes in [1, 16_384] {
        let candidate = TargetCapability::Extension {
            namespace: "n".repeat(bytes),
            name: "x".repeat(bytes),
        };
        let supported = BTreeSet::from([candidate]);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(INPUT_STATE_WORK + COUNT_PASS + 4);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, INPUT_STATE_STORAGE);
        let state = VerificationFunctionStateV1::build(&function, &mut budget)
            .unwrap()
            .unwrap();
        let mut count = VerificationDiagnosticCollectorV1::count();
        assert_eq!(
            try_verify_registered_operation_with_resources_v1(
                &operation,
                &location(8).borrowed_v1(),
                &state,
                Some(&supported),
                &mut count,
                &mut budget,
            ),
            Ok(false)
        );
        assert_eq!(budget.work(), INPUT_STATE_WORK + COUNT_PASS + 4);
        assert_eq!(count.counted(), Some(1));
        state.release(&mut budget).unwrap();
    }
}

#[test]
fn registered_capability_message_upper_covers_debug_escaping_without_ownership() {
    for required in [
        TargetCapabilityRefV1::extension("\0\n\"\\", "\u{7f}\u{85}\u{10ffff}"),
        TargetCapabilityRefV1::extension_lower_hex("digest", &[0, 15, 16, 255]),
    ] {
        let expected = format!("target does not support required capability {required:?}");
        let upper = registered_capability_message_work_upper_v1(required).unwrap();
        assert!(expected.len() <= upper);
        let exact_work = 5 + 1 + upper;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work - 1);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let mut count = VerificationDiagnosticCollectorV1::count();
        assert!(matches!(
            emit_registered_diagnostic_v1(
                &location(8).borrowed_v1(),
                DiagnosticCode::UnsupportedCapability,
                upper,
                format_args!("target does not support required capability {required:?}"),
                &mut count,
                &mut budget,
            ),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                if error.actual() == exact_work && error.limit() == exact_work - 1
        ));
        assert_eq!(count.counted(), Some(0));
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn registered_intrinsic_and_matrix_dispatch_do_not_repeat_lds_producer_checks() {
    let function = input_function(Type::pointer(
        Type::Scalar(ScalarType::Bf16),
        AddressSpace::Workgroup,
        AccessMode::ReadOnly,
    ));
    let operations = [
        Operation {
            results: vec![ValueDef::new(ValueId(9), Type::INDEX)],
            kind: OperationKind::Intrinsic(IntrinsicOperation::launch_extent_1d()),
        },
        Operation {
            results: (1..=4)
                .map(|id| ValueDef::new(ValueId(id), Type::Scalar(ScalarType::Bf16)))
                .collect(),
            kind: OperationKind::Matrix(MatrixOperation::lds_load(ValueId(0), MatrixElement::Bf16)),
        },
    ];
    // Intrinsic: dispatch1 + empty-roster completion1 + shape/payload7.
    // Matrix LDS load: dispatch1 + one-operand roster8 + payload22.
    for (operation, pass_work, scratch_storage) in [
        (&operations[0], 1 + 1 + 7, 0),
        (&operations[1], 1 + ONE_OPERAND_ROSTER_WORK + 22, 2),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(INPUT_STATE_WORK + pass_work);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            INPUT_STATE_STORAGE + scratch_storage,
        );
        let state = VerificationFunctionStateV1::build(&function, &mut budget)
            .unwrap()
            .unwrap();
        let mut count = VerificationDiagnosticCollectorV1::count();
        assert_eq!(
            try_verify_registered_operation_with_resources_v1(
                operation,
                &location(8).borrowed_v1(),
                &state,
                None,
                &mut count,
                &mut budget,
            ),
            Ok(true)
        );
        assert_eq!(count.counted(), Some(0));
        assert_eq!(budget.work(), INPUT_STATE_WORK + pass_work);
        assert_eq!(budget.peak_storage(), INPUT_STATE_STORAGE + scratch_storage);
        assert_eq!(budget.storage(), INPUT_STATE_STORAGE);
        state.release(&mut budget).unwrap();
    }
}

#[test]
fn registered_sink_maps_every_issue_code_without_reordering_locations() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let mut diagnostics = VerificationDiagnosticCollectorV1::materialize(7, &mut budget).unwrap();
    let mut expected = Vec::new();
    for (ordinal, (kind, code)) in [
        (
            SemanticOperationIssueKind::InvalidStructure,
            DiagnosticCode::InvalidSemanticOperation,
        ),
        (
            SemanticOperationIssueKind::InvalidOperandType,
            DiagnosticCode::InvalidOperandType,
        ),
        (
            SemanticOperationIssueKind::ResultArity,
            DiagnosticCode::ResultArity,
        ),
        (
            SemanticOperationIssueKind::TypeMismatch,
            DiagnosticCode::TypeMismatch,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut current = location(8);
        current.operation = Some(ordinal);
        let mut sink = RegisteredOperationIssueSinkV1 {
            location: current.borrowed_v1(),
            diagnostics: &mut diagnostics,
            budget: &mut budget,
        };
        SemanticOperationIssueSinkV1::emit(&mut sink, kind, 6, format_args!("mapped")).unwrap();
        expected.push(Diagnostic {
            location: current,
            code,
            message: "mapped".to_owned(),
        });
    }
    for (ordinal, (kind, code)) in [
        (
            MatrixVerificationIssueKind::InvalidStructure,
            DiagnosticCode::InvalidSemanticOperation,
        ),
        (
            MatrixVerificationIssueKind::InvalidOperandType,
            DiagnosticCode::InvalidOperandType,
        ),
        (
            MatrixVerificationIssueKind::InvalidResult,
            DiagnosticCode::TypeMismatch,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut current = location(8);
        current.operation = Some(ordinal + 4);
        let mut sink = RegisteredOperationIssueSinkV1 {
            location: current.borrowed_v1(),
            diagnostics: &mut diagnostics,
            budget: &mut budget,
        };
        MatrixOperationIssueSinkV1::emit(&mut sink, kind, 6, format_args!("mapped")).unwrap();
        expected.push(Diagnostic {
            location: current,
            code,
            message: "mapped".to_owned(),
        });
    }
    assert_eq!(
        diagnostics.finish_materialized(&mut budget).unwrap(),
        expected
    );
    assert_eq!(budget.storage(), 0);
}
