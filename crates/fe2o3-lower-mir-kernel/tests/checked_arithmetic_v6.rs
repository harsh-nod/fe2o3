use fe2o3_kernel_ir::{
    BinaryOp, BlockId, CheckedBinaryOperator, Operation, OperationKind, ScalarType, Terminator,
    Type, decode_module_v8, verify_module,
};
use fe2o3_lower_mir_kernel::{
    ProductionSemanticKirErrorV1, ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1,
    ProductionSemanticKirResourceV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const INTEGER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const CHECKED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

#[derive(Clone, Copy)]
struct IntegerCase {
    name: &'static str,
    signed: bool,
    bits: u16,
    scalar: ScalarType,
}

const INTEGER_CASES: [IntegerCase; 12] = [
    IntegerCase {
        name: "u8",
        signed: false,
        bits: 8,
        scalar: ScalarType::U8,
    },
    IntegerCase {
        name: "i8",
        signed: true,
        bits: 8,
        scalar: ScalarType::I8,
    },
    IntegerCase {
        name: "u16",
        signed: false,
        bits: 16,
        scalar: ScalarType::U16,
    },
    IntegerCase {
        name: "i16",
        signed: true,
        bits: 16,
        scalar: ScalarType::I16,
    },
    IntegerCase {
        name: "u32",
        signed: false,
        bits: 32,
        scalar: ScalarType::U32,
    },
    IntegerCase {
        name: "i32",
        signed: true,
        bits: 32,
        scalar: ScalarType::I32,
    },
    IntegerCase {
        name: "u64",
        signed: false,
        bits: 64,
        scalar: ScalarType::U64,
    },
    IntegerCase {
        name: "i64",
        signed: true,
        bits: 64,
        scalar: ScalarType::I64,
    },
    IntegerCase {
        name: "u128",
        signed: false,
        bits: 128,
        scalar: ScalarType::U128,
    },
    IntegerCase {
        name: "i128",
        signed: true,
        bits: 128,
        scalar: ScalarType::I128,
    },
    IntegerCase {
        name: "usize-gfx942",
        signed: false,
        bits: 64,
        scalar: ScalarType::U64,
    },
    IntegerCase {
        name: "isize-gfx942",
        signed: true,
        bits: 64,
        scalar: ScalarType::I64,
    },
];

const OPERATORS: [(SemanticCheckedBinaryOpV1, CheckedBinaryOperator); 3] = [
    (SemanticCheckedBinaryOpV1::Add, CheckedBinaryOperator::Add),
    (
        SemanticCheckedBinaryOpV1::Subtract,
        CheckedBinaryOperator::Subtract,
    ),
    (
        SemanticCheckedBinaryOpV1::Multiply,
        CheckedBinaryOperator::Multiply,
    ),
];

fn identity(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn unit_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(1)),
        SemanticLayoutIdentityV1::from_sha256(identity(1)),
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
    )
}

fn full_valid_range(bits: u16) -> SemanticScalarValidityRangeV1 {
    SemanticScalarValidityRangeV1::new(
        0,
        if bits == 128 {
            u128::MAX
        } else {
            (1_u128 << bits) - 1
        },
    )
}

fn integer_type(tag: u8, signed: bool, bits: u16) -> SemanticTypeDeclV1 {
    let size = u64::from(bits / 8);
    let alignment = size.min(8);
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            alignment,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, bits, alignment),
                full_valid_range(bits),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
    )
}

fn bool_type(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    )
}

fn float_type(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::float(32, 4),
                full_valid_range(32),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    )
}

fn validity_integer_type(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(1, u128::from(u32::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::ValidityScalar(
            SemanticValidityScalarTypeV1::new(
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
                vec![SemanticScalarValidityRangeV1::new(1, u128::from(u32::MAX))],
            )
            .unwrap(),
        ),
    )
}

fn checked_tuple_type(
    tag: u8,
    value_type: SemanticTypeIdV1,
    value_size: u64,
) -> SemanticTypeDeclV1 {
    let alignment = value_size.min(8);
    let unpadded_size = value_size + 1;
    let size = (unpadded_size + alignment - 1) & !(alignment - 1);
    let padding = (size > unpadded_size)
        .then(|| SemanticPaddingV1::new(unpadded_size, size - unpadded_size).unwrap())
        .into_iter()
        .collect();
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            alignment,
            SemanticAggregateLayoutV1::new(vec![0, value_size], padding).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![value_type, BOOL]).unwrap()),
    )
}

fn local_place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn field_place(field: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()],
        ty,
    )
    .unwrap()
}

fn scalar_constant(ty: SemanticTypeIdV1, value: u128, size: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, size).unwrap()),
    ))
}

fn checked_statement_with_operands(
    destination: u32,
    operation: SemanticCheckedBinaryOpV1,
    result_type: SemanticTypeIdV1,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(destination, result_type),
            SemanticRvalueV1::new(
                result_type,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    operation, left, right,
                )),
            ),
        )),
    )
}

fn checked_statement(
    destination: u32,
    operation: SemanticCheckedBinaryOpV1,
    operand_type: SemanticTypeIdV1,
    result_type: SemanticTypeIdV1,
    size: u8,
    left: u128,
    right: u128,
) -> SemanticStatementV1 {
    checked_statement_with_operands(
        destination,
        operation,
        result_type,
        scalar_constant(operand_type, left, size),
        scalar_constant(operand_type, right, size),
    )
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(identity(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn direct_value(ty: SemanticTypeIdV1, extension: SemanticAbiExtensionV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                extension,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn request(
    types: Vec<SemanticTypeDeclV1>,
    local_types: Vec<SemanticTypeIdV1>,
    blocks: Vec<SemanticBasicBlockV1>,
    symbol: &[u8],
) -> InertSemanticMirRequestV1 {
    request_with_arguments(types, local_types, 0, blocks, symbol)
}

fn request_with_arguments(
    types: Vec<SemanticTypeDeclV1>,
    local_types: Vec<SemanticTypeIdV1>,
    argument_count: usize,
    blocks: Vec<SemanticBasicBlockV1>,
    symbol: &[u8],
) -> InertSemanticMirRequestV1 {
    let arguments = local_types
        .iter()
        .skip(1)
        .take(argument_count)
        .copied()
        .map(|ty| {
            let extension = match types[ty.index() as usize].shape() {
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits })
                    if *bits < 32 =>
                {
                    if *signed {
                        SemanticAbiExtensionV1::SignExtend
                    } else {
                        SemanticAbiExtensionV1::ZeroExtend
                    }
                }
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => {
                    SemanticAbiExtensionV1::ZeroExtend
                }
                _ => SemanticAbiExtensionV1::None,
            };
            SemanticAbiArgumentV1::source(direct_value(ty, extension))
        })
        .collect();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(identity(30)),
        SemanticLayoutIdentityV1::from_sha256(identity(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        argument_count as u32,
        arguments,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = local_types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity(40 + index as u8)),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else if index <= argument_count {
                    SemanticLocalRoleV1::Argument((index - 1) as u32)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(50)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(identity(50)),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(50)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(50)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(50)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(symbol.to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(identity(51)),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(250))),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn owner(request: InertSemanticMirRequestV1) -> ProductionSemanticMirOwnerV1 {
    let admitted = request.admit(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn integer_owner(
    integer: IntegerCase,
    operation: SemanticCheckedBinaryOpV1,
) -> ProductionSemanticMirOwnerV1 {
    let size = u64::from(integer.bits / 8);
    owner(request_with_arguments(
        vec![
            unit_type(),
            integer_type(2, integer.signed, integer.bits),
            bool_type(3),
            checked_tuple_type(4, INTEGER, size),
        ],
        vec![UNIT, INTEGER, INTEGER, CHECKED],
        2,
        vec![block(
            60,
            vec![checked_statement_with_operands(
                3,
                operation,
                CHECKED,
                SemanticOperandV1::Copy(local_place(1, INTEGER)),
                SemanticOperandV1::Move(local_place(2, INTEGER)),
            )],
            SemanticTerminatorKindV1::Return,
        )],
        b"checked_integer_matrix",
    ))
}

fn constant_u32_owner(operation: SemanticCheckedBinaryOpV1) -> ProductionSemanticMirOwnerV1 {
    owner(request(
        vec![
            unit_type(),
            integer_type(2, false, 32),
            bool_type(3),
            checked_tuple_type(4, INTEGER, 4),
        ],
        vec![UNIT, CHECKED],
        vec![block(
            60,
            vec![checked_statement(1, operation, INTEGER, CHECKED, 4, 1, 2)],
            SemanticTerminatorKindV1::Return,
        )],
        b"checked_constant_u32",
    ))
}

fn checked_operation(operations: &[Operation]) -> &Operation {
    operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(_),
                    ..
                }
            )
        })
        .expect("checked operation")
}

fn checked_cfg_owner() -> ProductionSemanticMirOwnerV1 {
    let checked =
        |operation, left, right| checked_statement(1, operation, INTEGER, CHECKED, 4, left, right);
    let entry_switch = SemanticTerminatorKindV1::SwitchInt {
        discriminant: scalar_constant(BOOL, 1, 1),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                edge(SemanticEdgeRoleV1::SwitchValue, 1),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
        )
        .unwrap(),
    };
    let assertion = SemanticTerminatorKindV1::Assert {
        condition: SemanticOperandV1::Copy(field_place(1, BOOL)),
        expected: false,
        message: SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left: scalar_constant(INTEGER, 1, 4),
            right: scalar_constant(INTEGER, 2, 4),
        },
        target: edge(SemanticEdgeRoleV1::AssertSuccess, 4),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    let value_switch = SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(field_place(0, INTEGER)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, 5),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, 6),
        )
        .unwrap(),
    };
    owner(request(
        vec![
            unit_type(),
            integer_type(2, false, 32),
            bool_type(3),
            checked_tuple_type(4, INTEGER, 4),
        ],
        vec![UNIT, CHECKED],
        vec![
            block(60, vec![], entry_switch),
            block(
                61,
                vec![checked(SemanticCheckedBinaryOpV1::Add, 5, 7)],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                62,
                vec![checked(SemanticCheckedBinaryOpV1::Subtract, 11, 3)],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(63, vec![], assertion),
            block(64, vec![], value_switch),
            block(65, vec![], SemanticTerminatorKindV1::Return),
            block(66, vec![], SemanticTerminatorKindV1::Return),
        ],
        b"checked_cfg_results",
    ))
}

#[test]
fn every_operator_and_admitted_integer_width_lowers_with_scalar_signedness() {
    for integer in INTEGER_CASES {
        for (semantic_operator, expected_operator) in OPERATORS {
            let lowered = ProductionSemanticKirOwnerV1::try_lower(
                integer_owner(integer, semantic_operator),
                ProductionSemanticKirLimitsV1::default(),
            )
            .unwrap_or_else(|error| {
                panic!("{} {semantic_operator:?} failed: {error}", integer.name)
            });
            lowered.verify_equivalence().unwrap();
            verify_module(lowered.module()).unwrap();

            let body = lowered.module().functions[0].body.as_ref().unwrap();
            assert_eq!(body.blocks.len(), 1, "{}", integer.name);
            assert_eq!(body.blocks[0].operations.len(), 1, "{}", integer.name);
            let checked = checked_operation(&body.blocks[0].operations);
            let OperationKind::Binary { op, lhs, rhs } = checked.kind else {
                unreachable!();
            };
            assert_eq!(op, BinaryOp::Checked(expected_operator), "{}", integer.name);
            assert_eq!(lhs, body.parameters[0]);
            assert_eq!(rhs, body.parameters[1]);
            assert_eq!(checked.results.len(), 2);
            assert_eq!(checked.results[0].ty, Type::Scalar(integer.scalar));
            assert_eq!(checked.results[1].ty, Type::BOOL);
            assert_ne!(checked.results[0].id, checked.results[1].id);
            assert_eq!(lowered.module().functions.len(), 1);
            assert!(
                lowered
                    .correspondence()
                    .synthetic_operation_spans()
                    .is_empty()
            );

            let [span] = lowered.correspondence().statement_operation_spans() else {
                panic!("{} must have one statement span", integer.name);
            };
            assert_eq!(span.first_operation_ordinal(), 0);
            assert_eq!(span.operation_count(), 1);
        }
    }
}

#[test]
fn tuple_results_flow_independently_through_liveness_cfg_assert_and_switch() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        checked_cfg_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let blocks = &lowered.module().functions[0].body.as_ref().unwrap().blocks;
    let by_id = |id| blocks.iter().find(|block| block.id == BlockId(id)).unwrap();

    for id in [1, 2] {
        let block = by_id(id);
        let checked = checked_operation(&block.operations);
        let Terminator::Branch { target, arguments } = block.terminator.as_ref().unwrap() else {
            panic!("checked predecessor must branch to the join");
        };
        assert_eq!(*target, BlockId(3));
        assert_eq!(
            arguments,
            &vec![checked.results[0].id, checked.results[1].id]
        );
    }

    let join = by_id(3);
    assert_eq!(join.parameters.len(), 2);
    assert_eq!(join.parameters[0].ty, Type::Scalar(ScalarType::U32));
    assert_eq!(join.parameters[1].ty, Type::BOOL);
    let Terminator::ConditionalBranch {
        condition,
        then_target,
        else_target,
        else_arguments,
        ..
    } = join.terminator.as_ref().unwrap()
    else {
        panic!("explicit overflow assert must lower to a conditional branch");
    };
    assert_eq!(*condition, join.parameters[1].id);
    assert_eq!(*then_target, BlockId(7));
    assert_eq!(*else_target, BlockId(4));
    assert_eq!(
        else_arguments,
        &vec![join.parameters[0].id, join.parameters[1].id]
    );

    let switch = by_id(4);
    assert_eq!(switch.parameters.len(), 2);
    let Terminator::Switch { selector, .. } = switch.terminator.as_ref().unwrap() else {
        panic!("wrapped value must remain an integer switch selector");
    };
    assert_eq!(*selector, switch.parameters[0].id);
    assert_ne!(*selector, switch.parameters[1].id);
    assert_eq!(by_id(7).operations.len(), 1);

    let spans = lowered.correspondence().statement_operation_spans();
    assert_eq!(spans.len(), 2);
    assert!(spans.iter().all(|span| span.operation_count() == 3));
    assert_eq!(
        lowered.correspondence().synthetic_operation_spans().len(),
        1
    );
}

#[test]
fn checked_lowering_is_deterministic_in_module_and_correspondence_order() {
    let first = ProductionSemanticKirOwnerV1::try_lower(
        checked_cfg_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let second = ProductionSemanticKirOwnerV1::try_lower(
        checked_cfg_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(first.module(), second.module());
    assert_eq!(first.correspondence(), second.correspondence());
    assert_eq!(
        first.canonical_kernel_ir_v8().unwrap().canonical_bytes(),
        second.canonical_kernel_ir_v8().unwrap().canonical_bytes(),
    );
    assert_eq!(
        first.canonical_kernel_ir_v8_identity(),
        second.canonical_kernel_ir_v8_identity(),
    );
    let decoded =
        decode_module_v8(first.canonical_kernel_ir_v8().unwrap().canonical_bytes()).unwrap();
    assert_eq!(decoded, *first.module());
    assert!(
        decoded.functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(_),
                    ..
                }
            ) && operation.results.len() == 2)
    );
}

#[test]
fn checked_statement_operation_budget_is_exact_and_fail_closed() {
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            constant_u32_owner(SemanticCheckedBinaryOpV1::Multiply),
            ProductionSemanticKirLimitsV1::new_with_max_operations(1, 1, 1, 2),
        ),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Operations,
            actual: 3,
            limit: 2,
        })
    ));

    let exact = ProductionSemanticKirOwnerV1::try_lower(
        constant_u32_owner(SemanticCheckedBinaryOpV1::Multiply),
        ProductionSemanticKirLimitsV1::new_with_max_operations(1, 1, 1, 3),
    )
    .unwrap();
    assert_eq!(
        exact.correspondence().statement_operation_spans()[0].operation_count(),
        3
    );
    assert_eq!(
        exact.module().functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .len(),
        3
    );
}

#[test]
fn restricted_non_integer_and_mismatched_forms_never_reach_lowering() {
    let invalid_requests = [
        request(
            vec![
                unit_type(),
                validity_integer_type(2),
                bool_type(3),
                checked_tuple_type(4, INTEGER, 4),
            ],
            vec![UNIT, CHECKED],
            vec![block(
                70,
                vec![checked_statement(
                    1,
                    SemanticCheckedBinaryOpV1::Add,
                    INTEGER,
                    CHECKED,
                    4,
                    1,
                    2,
                )],
                SemanticTerminatorKindV1::Return,
            )],
            b"checked_restricted",
        ),
        request(
            vec![
                unit_type(),
                float_type(2),
                bool_type(3),
                checked_tuple_type(4, INTEGER, 4),
            ],
            vec![UNIT, CHECKED],
            vec![block(
                71,
                vec![checked_statement(
                    1,
                    SemanticCheckedBinaryOpV1::Subtract,
                    INTEGER,
                    CHECKED,
                    4,
                    1,
                    2,
                )],
                SemanticTerminatorKindV1::Return,
            )],
            b"checked_float",
        ),
        request(
            vec![
                unit_type(),
                integer_type(2, false, 32),
                bool_type(3),
                checked_tuple_type(4, INTEGER, 4),
                integer_type(5, true, 32),
            ],
            vec![UNIT, CHECKED],
            vec![block(
                72,
                vec![checked_statement_with_operands(
                    1,
                    SemanticCheckedBinaryOpV1::Multiply,
                    CHECKED,
                    scalar_constant(INTEGER, 1, 4),
                    scalar_constant(SemanticTypeIdV1::from_index(4), 2, 4),
                )],
                SemanticTerminatorKindV1::Return,
            )],
            b"checked_mismatch",
        ),
    ];

    for invalid in invalid_requests {
        let error = invalid.admit(SemanticMirLimitsV1::default()).unwrap_err();
        assert!(
            matches!(
                error,
                SemanticMirErrorV1::InvalidTypeOperation {
                    operation: SemanticTypeOperationV1::CheckedBinary,
                    ..
                }
            ),
            "unexpected hostile-form rejection: {error:?}"
        );
    }
}
