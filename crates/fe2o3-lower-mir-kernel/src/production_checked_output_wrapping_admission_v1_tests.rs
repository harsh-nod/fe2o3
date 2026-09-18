use super::*;
use fe2o3_kernel_ir::CheckedBinaryOperator;

const WRAP_SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const WRAP_PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const WRAP_OPERATIONS: [(
    SemanticBinaryOpV1,
    SemanticCheckedBinaryOpV1,
    CheckedBinaryOperator,
); 3] = [
    (
        SemanticBinaryOpV1::Add,
        SemanticCheckedBinaryOpV1::Add,
        CheckedBinaryOperator::Add,
    ),
    (
        SemanticBinaryOpV1::Subtract,
        SemanticCheckedBinaryOpV1::Subtract,
        CheckedBinaryOperator::Subtract,
    ),
    (
        SemanticBinaryOpV1::Multiply,
        SemanticCheckedBinaryOpV1::Multiply,
        CheckedBinaryOperator::Multiply,
    ),
];

fn wrapping_types(signed: bool, bits: u16) -> Vec<SemanticTypeDeclV1> {
    let mut declarations = types();
    declarations.truncate(2);
    let bytes = u64::from(bits / 8);
    let alignment = bytes.min(8);
    let maximum = if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    };
    declarations.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([71; 32]),
        SemanticLayoutIdentityV1::from_sha256([72; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            alignment,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, bits, alignment),
                SemanticScalarValidityRangeV1::new(0, maximum),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
    ));
    declarations.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([73; 32]),
        SemanticLayoutIdentityV1::from_sha256([74; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some((bytes + 1).div_ceil(alignment) * alignment),
            alignment,
            SemanticAggregateLayoutV1::new(vec![0, bytes], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WRAP_SCALAR, BOOL]).unwrap()),
    ));
    declarations
}

fn pair_field(field: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(6),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}

fn wrapping_source(
    signed: bool,
    bits: u16,
    operation: usize,
    explicit: bool,
) -> ProductionPreRankedKirOwnerV1 {
    wrapping_source_mode(
        signed,
        bits,
        operation,
        if explicit {
            WrappingMode::Checked
        } else {
            WrappingMode::Wrapping
        },
    )
}

#[derive(Clone, Copy)]
enum WrappingMode {
    Wrapping,
    Checked,
    ProvenUnchecked,
}

fn wrapping_source_mode(
    signed: bool,
    bits: u16,
    operation: usize,
    mode: WrappingMode,
) -> ProductionPreRankedKirOwnerV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let (ordinary, checked, _) = WRAP_OPERATIONS[operation];
    let store_read = vec![
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(4, WRAP_SCALAR),
                value(3, WRAP_SCALAR),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ),
        assignment(
            5,
            WRAP_SCALAR,
            SemanticRvalueKindV1::Use(value(4, WRAP_SCALAR)),
        ),
    ];
    let blocks = if matches!(mode, WrappingMode::ProvenUnchecked) {
        let unchecked = match operation {
            0 => SemanticUncheckedBinaryOpV1::Add,
            1 => SemanticUncheckedBinaryOpV1::Subtract,
            2 => SemanticUncheckedBinaryOpV1::Multiply,
            _ => unreachable!(),
        };
        let mut successful = vec![assignment(
            3,
            WRAP_SCALAR,
            SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                unchecked,
                value(1, WRAP_SCALAR),
                value(2, WRAP_SCALAR),
            )),
        )];
        successful.extend(store_read);
        vec![
            block(
                31,
                vec![
                    assignment(
                        6,
                        WRAP_PAIR,
                        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                            checked,
                            value(1, WRAP_SCALAR),
                            value(2, WRAP_SCALAR),
                        )),
                    ),
                    assignment(7, BOOL, SemanticRvalueKindV1::Use(pair_field(1, BOOL))),
                ],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: value(7, BOOL),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(32, successful, SemanticTerminatorKindV1::Return),
            block(33, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else if matches!(mode, WrappingMode::Checked) {
        vec![
            block(
                31,
                vec![
                    assignment(
                        6,
                        WRAP_PAIR,
                        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                            checked,
                            value(1, WRAP_SCALAR),
                            value(2, WRAP_SCALAR),
                        )),
                    ),
                    assignment(
                        3,
                        WRAP_SCALAR,
                        SemanticRvalueKindV1::Use(pair_field(0, WRAP_SCALAR)),
                    ),
                    assignment(7, BOOL, SemanticRvalueKindV1::Use(pair_field(1, BOOL))),
                ],
                SemanticTerminatorKindV1::Assert {
                    condition: value(7, BOOL),
                    expected: false,
                    message: SemanticAssertMessageV1::Overflow {
                        operation: ordinary,
                        left: value(1, WRAP_SCALAR),
                        right: value(2, WRAP_SCALAR),
                    },
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(32, store_read, SemanticTerminatorKindV1::Return),
        ]
    } else {
        let mut statements = vec![assignment(
            3,
            WRAP_SCALAR,
            SemanticRvalueKindV1::Binary {
                operation: ordinary,
                left: value(1, WRAP_SCALAR),
                right: value(2, WRAP_SCALAR),
            },
        )];
        statements.extend(store_read);
        vec![block(31, statements, SemanticTerminatorKindV1::Return)]
    };
    let argument = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        WRAP_SCALAR,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                if bits < 32 {
                    if signed {
                        SemanticAbiExtensionV1::SignExtend
                    } else {
                        SemanticAbiExtensionV1::ZeroExtend
                    }
                } else {
                    SemanticAbiExtensionV1::None
                },
                0,
                None,
            )
            .unwrap(),
        ),
    ));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([30; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        vec![argument.clone(), argument],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let local_types = [
        (UNIT, SemanticLocalRoleV1::Return),
        (WRAP_SCALAR, SemanticLocalRoleV1::Argument(0)),
        (WRAP_SCALAR, SemanticLocalRoleV1::Argument(1)),
        (WRAP_SCALAR, SemanticLocalRoleV1::Temporary),
        (WRAP_SCALAR, SemanticLocalRoleV1::Temporary),
        (WRAP_SCALAR, SemanticLocalRoleV1::Temporary),
        (WRAP_PAIR, SemanticLocalRoleV1::Temporary),
        (BOOL, SemanticLocalRoleV1::Temporary),
    ];
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([30; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
        source,
        abi,
        local_types
            .into_iter()
            .enumerate()
            .map(|(ordinal, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([40 + ordinal as u8; 32]),
                    ty,
                    role,
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"private_array_relation".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([30; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        wrapping_types(signed, bits),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "logical_0",
            [30; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn wrapping_receipt(
    signed: bool,
    bits: u16,
    operation: usize,
    explicit: bool,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    // The ranked launch/effect component is genuine, as in existing private
    // scalar tests. Recursive ranked value-expression normalization and the
    // backend source projector are separate qualification boundaries.
    array_output_ranked_receipt_v1(wrapping_source(signed, bits, operation, explicit))
}

fn checked_operation(module: &Module) -> &Operation {
    let mut operations = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(_),
                    ..
                }
            )
        });
    let operation = operations
        .next()
        .expect("one source wrapping/checked arithmetic operation");
    assert!(operations.next().is_none());
    operation
}

fn private_counts(module: &Module) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for operation in module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        match operation.kind {
            OperationKind::Alloca {
                address_space: AddressSpace::Private,
                ..
            } => counts.0 += 1,
            OperationKind::Store { access, .. }
                if access.address_space == AddressSpace::Private =>
            {
                counts.1 += 1
            }
            OperationKind::Load { access, .. } if access.address_space == AddressSpace::Private => {
                counts.2 += 1
            }
            _ => {}
        }
    }
    counts
}

fn assert_wrapping_value_and_store(module: &Module, signed: bool, bits: u16, operation: usize) {
    let checked = checked_operation(module);
    assert!(
        matches!(checked.kind, OperationKind::Binary { op: BinaryOp::Checked(actual), lhs, rhs }
        if actual == WRAP_OPERATIONS[operation].2 && lhs != rhs)
    );
    let [value, overflow] = checked.results.as_slice() else {
        panic!("exact value and overflow pair")
    };
    let scalar = value.ty.as_scalar().unwrap();
    assert_eq!(scalar.is_signed_integer(), signed);
    assert_eq!(scalar.bit_width(), Some(bits));
    assert_eq!(overflow.ty, Type::BOOL);
    let stores = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(operation.kind, OperationKind::Store { value: actual, access, .. }
            if actual == value.id && access.address_space == AddressSpace::Private)
        })
        .count();
    assert_eq!(
        stores, 1,
        "the dynamic wrapped numeric result, not overflow or an unrelated value, is stored"
    );
}

#[test]
fn general_policy3_wrapping_integer_widths_and_signs_reach_actual_live_private_store() {
    for signed in [false, true] {
        for bits in [8, 16, 32, 64] {
            for operation in 0..WRAP_OPERATIONS.len() {
                let receipt = wrapping_receipt(signed, bits, operation, false);
                assert_wrapping_value_and_store(
                    receipt.materialized.executable().module(),
                    signed,
                    bits,
                    operation,
                );
                with_prepared(prepare(receipt, Profile::Gfx942, None), |input, budget| {
                    assert_wrapping_value_and_store(input.bound.module(), signed, bits, operation);
                    let floor = budget.storage();
                    let owner = AdmittedOutput::try_admit_general_v1(
                        input.receipt,
                        input.bound,
                        input.output,
                        budget,
                    )
                    .unwrap();
                    assert_wrapping_value_and_store(
                        owner.output().module(),
                        signed,
                        bits,
                        operation,
                    );
                    assert_eq!(private_counts(owner.output().module()), (1, 1, 1));
                    owner.verify_equivalence(budget).unwrap();
                    assert_eq!(budget.storage(), floor);
                    assert!(!owner.grants_artifact_or_launch_authority());
                });
            }
        }
    }
}

#[test]
fn general_policy3_wrapping_128_bit_source_remains_outside_the_admission_grammar() {
    for signed in [false, true] {
        for operation in 0..WRAP_OPERATIONS.len() {
            with_prepared(
                prepare(
                    wrapping_receipt(signed, 128, operation, false),
                    Profile::Gfx942,
                    None,
                ),
                |input, budget| {
                    let floor = budget.storage();
                    assert!(matches!(
                        AdmittedOutput::try_admit_general_v1(
                            input.receipt,
                            input.bound,
                            input.output,
                            budget
                        ),
                        Err(AdmissionError::Unsupported {
                            phase: "source",
                            detail: "total scalar/global recipe; no unchecked arithmetic",
                        })
                    ));
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }
}

#[test]
fn general_policy3_wrapping_does_not_admit_even_independently_guarded_unchecked_source() {
    for operation in 0..WRAP_OPERATIONS.len() {
        // Semantic admission independently proves exact checked-overflow zero
        // dominance first. That proof does not enlarge this production grammar.
        let source = wrapping_source_mode(false, 32, operation, WrappingMode::ProvenUnchecked);
        with_prepared(
            prepare(
                array_output_ranked_receipt_v1(source),
                Profile::Gfx942,
                None,
            ),
            |input, budget| {
                let floor = budget.storage();
                assert!(matches!(
                    AdmittedOutput::try_admit_general_v1(
                        input.receipt,
                        input.bound,
                        input.output,
                        budget
                    ),
                    Err(AdmissionError::Unsupported {
                        phase: "source",
                        detail: "total scalar/global recipe; no unchecked arithmetic",
                    })
                ));
                assert_eq!(budget.storage(), floor);
            },
        );
    }
}

#[test]
fn general_policy4_wrapping_restores_full_input_floor_after_late_work_failure() {
    fn prepared() -> (
        ProductionMaterializedRankedModuleReceiptV1,
        VerifiedCanonicalKernelIrModuleV12,
        fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1,
        usize,
    ) {
        let Prepared {
            receipt,
            bound,
            output,
            source_storage,
            bound_storage,
            ..
        } = prepare(wrapping_receipt(false, 64, 2, false), Profile::Gfx942, None);
        drop(output);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        let floor = FLOOR + source_storage + bound_storage;
        budget.reserve_storage(floor).unwrap();
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, &mut budget)
                .unwrap();
        assert_eq!(checked.forwarding_rows().len(), 1);
        assert_eq!(budget.storage(), floor);
        let retained = floor + checked.retained_storage();
        (receipt, bound, checked, retained)
    }
    let (receipt, bound, checked, floor) = prepared();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    drop(
        crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            receipt,
            bound,
            checked,
            &mut budget,
        )
        .unwrap(),
    );
    let used = budget.work();
    assert!(used > 8);
    assert_eq!(budget.storage(), floor);
    budget.release_storage(floor - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);

    let (receipt, bound, checked, floor) = prepared();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(used - 1);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    assert!(
        crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            receipt,
            bound,
            checked,
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), floor);
    budget.release_storage(floor - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert!(work.failed_work().is_some());
}

#[test]
fn general_policy4_wrapping_dynamic_store_value_survives_real_nonidentity_forwarding() {
    for signed in [false, true] {
        for bits in [8, 16, 32, 64] {
            for operation in 0..WRAP_OPERATIONS.len() {
                let Prepared {
                    receipt,
                    bound,
                    output,
                    source_storage,
                    bound_storage,
                    ..
                } = prepare(
                    wrapping_receipt(signed, bits, operation, false),
                    Profile::Gfx942,
                    None,
                );
                drop(output);
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget
                    .reserve_storage(FLOOR + source_storage + bound_storage)
                    .unwrap();
                let checked = fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(
                    &bound,
                    &mut budget,
                )
                .unwrap();
                assert_eq!(checked.forwarding_rows().len(), 1);
                assert_eq!(
                    private_counts(checked.intermediate_policy3().owner().module()),
                    (1, 1, 1)
                );
                assert_eq!(private_counts(checked.owner().module()), (1, 1, 0));
                assert_ne!(
                    checked
                        .intermediate_policy3()
                        .owner()
                        .canonical()
                        .identity(),
                    checked.owner().canonical().identity()
                );
                budget.reserve_storage(checked.retained_storage()).unwrap();
                let floor = budget.storage();
                let owner = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                    receipt,
                    bound,
                    checked,
                    &mut budget,
                )
                .unwrap();
                assert_wrapping_value_and_store(owner.output().module(), signed, bits, operation);
                assert_eq!(private_counts(owner.output().module()), (1, 1, 0));
                owner.verify_equivalence(&mut budget).unwrap();
                assert_eq!(budget.storage(), floor);
                drop(owner);
                budget.release_storage(floor - FLOOR).unwrap();
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    }
}

fn change_wrapping_operand(module: &mut Module) {
    let operation = module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(_),
                    ..
                }
            )
        })
        .unwrap();
    let OperationKind::Binary { lhs, rhs, .. } = &mut operation.kind else {
        unreachable!()
    };
    *lhs = *rhs;
}

fn change_wrapping_operator(module: &mut Module) {
    let operation = module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(_),
                    ..
                }
            )
        })
        .unwrap();
    let OperationKind::Binary { op, .. } = &mut operation.kind else {
        unreachable!()
    };
    *op = BinaryOp::Checked(CheckedBinaryOperator::Subtract);
}

fn replace_checked_with_plain_integer(module: &mut Module) {
    let operation = module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(_),
                    ..
                }
            )
        })
        .unwrap();
    let OperationKind::Binary { op, .. } = &mut operation.kind else {
        unreachable!()
    };
    *op = BinaryOp::Add;
    operation.results.truncate(1);
}

#[test]
fn general_policy3_wrapping_operand_operator_and_plain_opcode_mutations_fail_source_join() {
    for mutate in [
        change_wrapping_operand as fn(&mut Module),
        change_wrapping_operator,
        replace_checked_with_plain_integer,
    ] {
        with_prepared(
            prepare(
                wrapping_receipt(false, 64, 0, false),
                Profile::Gfx942,
                Some(mutate),
            ),
            |input, budget| {
                let floor = budget.storage();
                // These freshly verified hostile B components fail N/B before the
                // native grammar. This is not a claim that plain integers reached
                // or passed the later native opcode census.
                assert!(matches!(
                    AdmittedOutput::try_admit_general_v1(
                        input.receipt,
                        input.bound,
                        input.output,
                        budget
                    ),
                    Err(AdmissionError::Coordinates(_))
                ));
                assert_eq!(budget.storage(), floor);
            },
        );
    }
}

#[test]
fn general_policy3_wrapping_signedness_and_width_histories_are_not_interchangeable() {
    for (signed, bits) in [(true, 32), (false, 16), (false, 64)] {
        let mut original = prepare(wrapping_receipt(false, 32, 0, false), Profile::Gfx942, None);
        let replacement = prepare(
            wrapping_receipt(signed, bits, 0, false),
            Profile::Gfx942,
            None,
        );
        assert_ne!(
            original.bound.canonical().identity(),
            replacement.bound.canonical().identity()
        );
        original.output = replacement.output;
        original.output_storage = replacement.output_storage;
        drop((replacement.receipt, replacement.bound));
        with_prepared(original, |input, budget| {
            assert!(matches!(
                AdmittedOutput::try_admit_general_v1(
                    input.receipt,
                    input.bound,
                    input.output,
                    budget
                ),
                Err(AdmissionError::SourceOutput(
                    ProductionSourceOutputErrorV1::InputCustody
                ))
            ));
        });
    }
}

#[test]
fn general_policy3_explicit_checked_arithmetic_keeps_overflow_and_failure_trap() {
    for signed in [false, true] {
        for operation in 0..WRAP_OPERATIONS.len() {
            with_prepared(
                prepare(
                    wrapping_receipt(signed, 32, operation, true),
                    Profile::Gfx942,
                    None,
                ),
                |input, budget| {
                    let source_pair = checked_operation(input.bound.module());
                    assert_eq!(source_pair.results.len(), 2);
                    let overflow = source_pair.results[1].id;
                    assert_eq!(source_pair.results[1].ty, Type::BOOL);
                    assert!(input.bound.module().functions[0].body.as_ref().unwrap().blocks.iter()
                    .any(|block| matches!(block.terminator, Some(Terminator::ConditionalBranch { condition, .. }) if condition == overflow)));
                    let floor = budget.storage();
                    let owner = AdmittedOutput::try_admit_general_v1(
                        input.receipt,
                        input.bound,
                        input.output,
                        budget,
                    )
                    .unwrap();
                    let pair = checked_operation(owner.output().module());
                    assert_eq!(pair.results.len(), 2);
                    let overflow = pair.results[1].id;
                    let body = owner.output().module().functions[0].body.as_ref().unwrap();
                    assert!(body.blocks.iter().any(|block| matches!(block.terminator,
                    Some(Terminator::ConditionalBranch { condition, .. }) if condition == overflow)));
                    assert!(body.blocks.iter().any(|block| block.operations.iter().any(|operation|
                    matches!(&operation.kind, OperationKind::Call { callee, arguments }
                        if matches!(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                            Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap))))));
                    owner.verify_equivalence(budget).unwrap();
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }
}
