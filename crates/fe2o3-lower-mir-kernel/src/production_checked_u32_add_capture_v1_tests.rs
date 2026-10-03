use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

fn fixture(operation: SemanticCheckedBinaryOpV1, move_input: bool) -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let tuple_ty = SemanticTypeIdV1::from_index(3);
    let scalar = |tag, scalar, size, bits, maximum| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, bits, size),
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(scalar),
        )
    };
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        scalar(
            2,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
            4,
            32,
            u32::MAX.into(),
        ),
        scalar(3, SemanticScalarTypeV1::Bool, 1, 8, 1),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([4; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(
                    vec![0, 4],
                    vec![SemanticPaddingV1::new(5, 3).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![u32_ty, bool_ty]).unwrap(),
            ),
        ),
    ];
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let constant = |n| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            u32_ty,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(n, 4).unwrap()),
        ))
    };
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let input = if move_input {
        SemanticOperandV1::Move(place(1, u32_ty))
    } else {
        SemanticOperandV1::Copy(place(1, u32_ty))
    };
    let statements = vec![
        assign(1, u32_ty, SemanticRvalueKindV1::Use(constant(7))),
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Nop,
        ),
        assign(
            2,
            tuple_ty,
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                operation,
                input,
                constant(1),
            )),
        ),
    ];
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([10; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([11; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = [unit, u32_ty, tuple_ty]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([20 + index as u8; 32]),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([30; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"checked_u32_capture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([31; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn request() -> ProductionCheckedU32AddCaptureRequestV1 {
    ProductionCheckedU32AddCaptureRequestV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdV1::from_index(0),
        SemanticBlockIdV1::from_index(0),
        2,
    )
}

fn capture() -> ProductionSemanticKirOwnerV1 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    ProductionSemanticKirOwnerV1::try_lower_with_checked_u32_add_capture_v1(
        fixture(SemanticCheckedBinaryOpV1::Add, false),
        ProductionSemanticKirLimitsV1::default(),
        request(),
        &mut budget,
    )
    .unwrap()
}

#[test]
fn actual_capture_preserves_exact_legacy_v8_bytes_and_events() {
    let ordinary = ProductionSemanticKirOwnerV1::try_lower(
        fixture(SemanticCheckedBinaryOpV1::Add, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let captured = capture();
    assert!(ordinary.checked_u32_add_capture_v1().is_none());
    assert_eq!(
        ordinary.canonical_kernel_ir_bytes(),
        captured.canonical_kernel_ir_bytes()
    );
    assert_eq!(
        ordinary.canonical_kernel_ir_v8_identity(),
        captured.canonical_kernel_ir_v8_identity()
    );
    let fact = captured.checked_u32_add_capture_v1().unwrap();
    assert!(std::ptr::eq(fact.owner(), &captured));
    assert_eq!(fact.request(), request());
    assert_eq!(fact.lhs_local().index(), 1);
    assert_eq!(fact.tuple_local().index(), 2);
    assert_eq!((fact.use_event(), fact.define_event()), (1, 2));
    assert_eq!(fact.literal(), 1);
    assert!(!fact.proves_source_value_equivalence());
    assert!(!fact.grants_artifact_or_launch_authority());
    let function = captured
        .module()
        .functions
        .iter()
        .find(|row| row.id.as_str() == fact.kernel_ir_function())
        .unwrap();
    let block = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|row| row.id == fact.block())
        .unwrap();
    let operation = &block.operations[fact.operation() as usize];
    assert!(matches!(operation.kind, OperationKind::Binary { lhs, .. } if lhs == fact.operand()));
    assert_eq!(operation.results[0].id, fact.value());
    assert_eq!(operation.results[1].id, fact.overflow());
    captured.verify_equivalence().unwrap();
}

#[test]
fn wrong_source_coordinates_and_unsupported_shapes_fail_closed() {
    for selected in [
        ProductionCheckedU32AddCaptureRequestV1 {
            root: SemanticFunctionIdV1::from_index(1),
            ..request()
        },
        ProductionCheckedU32AddCaptureRequestV1 {
            function: SemanticFunctionIdV1::from_index(1),
            ..request()
        },
        ProductionCheckedU32AddCaptureRequestV1 {
            block: SemanticBlockIdV1::from_index(1),
            ..request()
        },
        ProductionCheckedU32AddCaptureRequestV1 {
            statement: 1,
            ..request()
        },
        ProductionCheckedU32AddCaptureRequestV1 {
            statement: 3,
            ..request()
        },
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        assert!(
            ProductionSemanticKirOwnerV1::try_lower_with_checked_u32_add_capture_v1(
                fixture(SemanticCheckedBinaryOpV1::Add, false),
                ProductionSemanticKirLimitsV1::default(),
                selected,
                &mut budget,
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 0);
    }
    for (operation, move_input) in [
        (SemanticCheckedBinaryOpV1::Subtract, false),
        (SemanticCheckedBinaryOpV1::Add, true),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        assert!(
            ProductionSemanticKirOwnerV1::try_lower_with_checked_u32_add_capture_v1(
                fixture(operation, move_input),
                ProductionSemanticKirLimitsV1::default(),
                request(),
                &mut budget,
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn replay_rejects_each_captured_coordinate_substitution() {
    for mutation in 0..10 {
        let mut owner = capture();
        let fact = owner.checked_u32_add_capture.as_mut().unwrap();
        match mutation {
            0 => fact.source.request.statement = 1,
            1 => fact.source.lhs_local = SemanticLocalIdV1::from_index(2),
            2 => fact.source.tuple_local = SemanticLocalIdV1::from_index(1),
            3 => fact.source.lhs_ssa = fact.source.tuple_ssa,
            4 => fact.source.use_event += 1,
            5 => fact.source.define_event += 1,
            6 => fact.source.literal += 1,
            7 => fact.operand = fact.value,
            8 => std::mem::swap(&mut fact.value, &mut fact.overflow),
            _ => fact.operation += 1,
        }
        assert!(owner.verify_equivalence().is_err(), "mutation {mutation}");
    }
}

#[test]
fn capture_storage_transfers_once_and_failures_preserve_caller_floor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(23).unwrap();
    let owner = ProductionSemanticKirOwnerV1::try_lower_with_checked_u32_add_capture_v1(
        fixture(SemanticCheckedBinaryOpV1::Add, false),
        ProductionSemanticKirLimitsV1::default(),
        request(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), 23);
    let retained = owner.checked_u32_add_capture_storage_v1().unwrap();
    assert!(retained > std::mem::size_of::<Option<Captured>>());
    budget.reserve_storage(retained).unwrap();
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 23);
    for (work_limit, storage_limit) in [(0, 1_000_000), (1_000_000, 23)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(23).unwrap();
        assert!(
            ProductionSemanticKirOwnerV1::try_lower_with_checked_u32_add_capture_v1(
                fixture(SemanticCheckedBinaryOpV1::Add, false),
                ProductionSemanticKirLimitsV1::default(),
                request(),
                &mut budget,
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 23);
    }
}
