use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::semantic_unchecked_arithmetic_violation_v1;

mod actual_core;
mod unsigned_underflow;

const INTEGER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const OTHER_INTEGER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
type Block = (Vec<SemanticStatementV1>, SemanticTerminatorKindV1);

fn scalar_type(tag: u8, signed: bool, bits: u16) -> SemanticTypeDeclV1 {
    let bytes = u64::from(bits / 8);
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, bits, bytes),
                SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
    )
}

fn types(signed: bool, bits: u16) -> Vec<SemanticTypeDeclV1> {
    let bytes = u64::from(bits / 8);
    vec![
        scalar_type(1, signed, bits),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([2; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
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
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(bytes * 2),
                bytes,
                SemanticAggregateLayoutV1::new(
                    vec![0, bytes],
                    if bytes == 1 {
                        vec![]
                    } else {
                        vec![SemanticPaddingV1::new(bytes + 1, bytes - 1).unwrap()]
                    },
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![INTEGER, BOOL]).unwrap()),
        ),
        scalar_type(4, false, 64),
    ]
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}

fn moved(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(local, ty))
}

fn constant(bits: u128, ty: SemanticTypeIdV1, bytes: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, bytes).unwrap()),
    ))
}

fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}

fn alias(local: u32, operand: SemanticOperandV1) -> SemanticStatementV1 {
    assign(local, operand.ty(), SemanticRvalueKindV1::Use(operand))
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn fixture(operation: SemanticUncheckedBinaryOpV1) -> Vec<Block> {
    let flag = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), BOOL).unwrap()],
            BOOL,
        )
        .unwrap(),
    );
    vec![
        (
            vec![
                alias(5, copy(1, INTEGER)),
                alias(6, copy(2, INTEGER)),
                assign(
                    3,
                    PAIR,
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        operation.checked(),
                        copy(5, INTEGER),
                        copy(6, INTEGER),
                    )),
                ),
                alias(4, flag),
            ],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: moved(4, BOOL),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        (vec![], SemanticTerminatorKindV1::Return),
        (
            vec![
                assign(
                    7,
                    INTEGER,
                    SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                        operation,
                        moved(5, INTEGER),
                        moved(6, INTEGER),
                    )),
                ),
                alias(0, copy(7, INTEGER)),
            ],
            SemanticTerminatorKindV1::Return,
        ),
    ]
}

fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn function(blocks: Vec<Block>) -> SemanticFunctionDeclV1 {
    let locals = [
        INTEGER, INTEGER, INTEGER, PAIR, BOOL, INTEGER, INTEGER, INTEGER,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([index as u8; 32]),
            ty,
            match index {
                0 => SemanticLocalRoleV1::Return,
                1 | 2 => SemanticLocalRoleV1::Argument(index as u32 - 1),
                _ => SemanticLocalRoleV1::Temporary,
            },
            SemanticSourceProvenanceV1::unavailable(),
        )
    })
    .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([10; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([11; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([12; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([13; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([14; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([15; 32]),
            SemanticLayoutIdentityV1::from_sha256([16; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct(INTEGER), direct(INTEGER)],
            direct(INTEGER),
        )
        .unwrap(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks
            .into_iter()
            .enumerate()
            .map(|(index, (statements, terminator))| {
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([index as u8; 32]),
                    SemanticSourceProvenanceV1::unavailable(),
                    statements,
                    SemanticTerminatorV1::new(
                        SemanticSourceProvenanceV1::unavailable(),
                        terminator,
                    ),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn value(function: &SemanticFunctionDeclV1, block: usize, statement: usize) -> &SemanticRvalueV1 {
    let SemanticStatementKindV1::Assign(assignment) =
        function.blocks()[block].statements()[statement].kind()
    else {
        panic!("fixture assignment");
    };
    assignment.value()
}

fn unchecked(function: &SemanticFunctionDeclV1) -> &SemanticRvalueV1 {
    function
        .blocks()
        .iter()
        .flat_map(|block| block.statements())
        .find_map(|statement| match statement.kind() {
            SemanticStatementKindV1::Assign(assignment)
                if matches!(
                    assignment.value().kind(),
                    SemanticRvalueKindV1::UncheckedBinary(_)
                ) =>
            {
                Some(assignment.value())
            }
            _ => None,
        })
        .expect("fixture unchecked site")
}

fn source_is_proved(function: &SemanticFunctionDeclV1) {
    assert!(
        semantic_unchecked_arithmetic_violation_v1(function)
            .unwrap()
            .is_none()
    );
}

fn rejects(function: &SemanticFunctionDeclV1) {
    let types = types(false, 32);
    assert_eq!(
        GpuSemanticExpressionResolverV2::new(&types, function)
            .resolve_rvalue_v2(unchecked(function)),
        Err(UNPROVEN)
    );
}

#[test]
fn retained_unchecked_add_sub_mul_normalize_exact_aliases_without_rewriting_source() {
    for signed in [false, true] {
        for bits in [8, 16, 32, 64] {
            let types = types(signed, bits);
            for (operation, expected_op) in [
                (
                    SemanticUncheckedBinaryOpV1::Add,
                    ProductionSemanticBinaryOpV2::Add,
                ),
                (
                    SemanticUncheckedBinaryOpV1::Subtract,
                    ProductionSemanticBinaryOpV2::Subtract,
                ),
                (
                    SemanticUncheckedBinaryOpV1::Multiply,
                    ProductionSemanticBinaryOpV2::Multiply,
                ),
            ] {
                let function = function(fixture(operation));
                let before = function.clone();
                source_is_proved(&function);
                let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function);
                assert_eq!(resolver.unchecked.proof.as_ref().unwrap().sites.len(), 1);
                let expected = ProductionSemanticExpressionV2::Binary {
                    operation: expected_op,
                    scalar: ProductionSemanticScalarTypeV2::Integer { signed, bits },
                    overflow: ProductionOverflowContractV2::Wrapping,
                    lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                        symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(0).unwrap(),
                        scalar: ProductionSemanticScalarTypeV2::Integer { signed, bits },
                    }),
                    rhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                        symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(1).unwrap(),
                        scalar: ProductionSemanticScalarTypeV2::Integer { signed, bits },
                    }),
                };
                for _ in 0..2 {
                    assert_eq!(
                        resolver.resolve_rvalue_v2(unchecked(&function)).unwrap(),
                        expected
                    );
                    assert_eq!(
                        resolver.resolve_rvalue_v2(value(&function, 2, 1)).unwrap(),
                        expected
                    );
                    assert!(!resolver.unchecked.normalizing);
                    assert_eq!(resolver.unchecked.unproved_inputs, 0);
                }
                assert_eq!(before, function);
            }
        }
    }
}

#[test]
fn a_successful_body_analysis_does_not_authenticate_synthetic_rvalues_or_operands() {
    let types = types(false, 32);
    let function = function(fixture(SemanticUncheckedBinaryOpV1::Add));
    let original = unchecked(&function);
    let synthetic_value = original.clone();
    let synthetic_operand = copy(7, INTEGER);
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function);
    assert_eq!(original, &synthetic_value);
    assert_eq!(resolver.resolve_rvalue_v2(&synthetic_value), Err(UNPROVEN));
    assert_eq!(
        resolver.resolve_operand_v2(&synthetic_operand, 0),
        Err(UNPROVEN)
    );
    assert!(resolver.resolve_rvalue_v2(original).is_ok());
    assert!(!resolver.unchecked.normalizing);
    assert_eq!(resolver.unchecked.unproved_inputs, 0);
}

#[test]
fn cached_proofs_cannot_cross_equal_function_identities_or_type_table_borrows() {
    let types = types(false, 32);
    let other_types = types.clone();
    let first = function(fixture(SemanticUncheckedBinaryOpV1::Add));
    let second = first.clone();
    assert_eq!(first.identity(), second.identity());
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &first);
    assert_eq!(
        resolver.resolve_rvalue_v2(unchecked(&second)),
        Err(UNPROVEN)
    );
    assert_eq!(
        resolver.resolve_rvalue_v2(value(&second, 2, 1)),
        Err(UNPROVEN)
    );
    resolver.function = &second;
    assert_eq!(resolver.resolve_rvalue_v2(unchecked(&first)), Err(UNPROVEN));
    resolver.function = &first;
    resolver.types = &other_types;
    assert_eq!(resolver.resolve_rvalue_v2(unchecked(&first)), Err(UNPROVEN));
    resolver.types = &types;
    assert!(resolver.resolve_rvalue_v2(unchecked(&first)).is_ok());
}

#[test]
fn retained_operand_membership_does_not_waive_its_execution_site() {
    let mut blocks = fixture(SemanticUncheckedBinaryOpV1::Add);
    blocks[1].0.push(alias(0, copy(7, INTEGER)));
    let function = function(blocks);
    source_is_proved(&function);
    let types = types(false, 32);
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function);
    assert_eq!(
        resolver.resolve_rvalue_v2(value(&function, 1, 0)),
        Err(UNPROVEN)
    );
    assert!(resolver.resolve_rvalue_v2(value(&function, 2, 1)).is_ok());
}

#[test]
fn unchecked_mutations_overwrites_and_stale_flags_are_rejected() {
    let mut wrong_operation = fixture(SemanticUncheckedBinaryOpV1::Add);
    wrong_operation[2].0[0] = assign(
        7,
        INTEGER,
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            SemanticUncheckedBinaryOpV1::Subtract,
            moved(5, INTEGER),
            moved(6, INTEGER),
        )),
    );
    let mut changed_operand = fixture(SemanticUncheckedBinaryOpV1::Add);
    changed_operand[2].0[0] = assign(
        7,
        INTEGER,
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            SemanticUncheckedBinaryOpV1::Add,
            moved(5, INTEGER),
            copy(1, INTEGER),
        )),
    );
    let mut overwritten = fixture(SemanticUncheckedBinaryOpV1::Add);
    overwritten[0].0.push(alias(5, constant(9, INTEGER, 4)));
    let mut stale_flag = fixture(SemanticUncheckedBinaryOpV1::Add);
    stale_flag[0].0.push(alias(4, constant(0, BOOL, 1)));
    for blocks in [wrong_operation, changed_operand, overwritten, stale_flag] {
        let function = function(blocks);
        assert!(
            semantic_unchecked_arithmetic_violation_v1(&function)
                .unwrap()
                .is_some()
        );
        rejects(&function);
    }
}

#[test]
fn a_changed_result_width_does_not_reuse_operand_only_proof() {
    let mut blocks = fixture(SemanticUncheckedBinaryOpV1::Add);
    blocks[2].0[0] = assign(
        7,
        OTHER_INTEGER,
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            SemanticUncheckedBinaryOpV1::Add,
            moved(5, INTEGER),
            moved(6, INTEGER),
        )),
    );
    let function = function(blocks);
    source_is_proved(&function);
    rejects(&function);
}

#[test]
fn overflow_and_rejoined_edges_stay_rejected_by_the_consumer() {
    let mut overflow = fixture(SemanticUncheckedBinaryOpV1::Add);
    overflow[1].0 = std::mem::take(&mut overflow[2].0);
    let mut joined = fixture(SemanticUncheckedBinaryOpV1::Add);
    joined[1].1 = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2));
    for blocks in [overflow, joined] {
        rejects(&function(blocks));
    }
}

#[test]
fn valid_overflow_equality_after_argument_overwrite_cannot_emit_an_old_symbol() {
    let mut blocks = fixture(SemanticUncheckedBinaryOpV1::Add);
    blocks[0].0.insert(0, alias(1, constant(9, INTEGER, 4)));
    let function = function(blocks);
    source_is_proved(&function);
    let types = types(false, 32);
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function);
    assert!(resolver.unchecked.proof.is_ok());
    assert_eq!(
        resolver.resolve_rvalue_v2(unchecked(&function)),
        Err(UNPROVEN)
    );
    assert!(!resolver.unchecked.normalizing);
    assert_eq!(resolver.unchecked.unproved_inputs, 0);
}

#[test]
fn a_moved_result_cannot_be_reconstructed_from_its_historical_definition() {
    let mut blocks = fixture(SemanticUncheckedBinaryOpV1::Add);
    blocks[2].0[1] = alias(0, moved(7, INTEGER));
    blocks[2].0.push(alias(0, copy(7, INTEGER)));
    let function = function(blocks);
    source_is_proved(&function);
    let types = types(false, 32);
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function);
    assert_eq!(
        resolver.resolve_rvalue_v2(value(&function, 2, 2)),
        Err(UNPROVEN)
    );
    assert!(resolver.resolve_rvalue_v2(value(&function, 2, 1)).is_ok());
}

#[test]
fn an_unknown_call_cannot_transfer_proof_to_a_later_caller_use() {
    let mut blocks = fixture(SemanticUncheckedBinaryOpV1::Add);
    let result_use = blocks[2].0.pop().unwrap();
    blocks[2].1 = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new(
            SemanticFunctionIdV1::from_index(1),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(0, INTEGER),
                edge(SemanticEdgeRoleV1::CallReturn, 3),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    blocks.push((vec![result_use], SemanticTerminatorKindV1::Return));
    let function = function(blocks);
    source_is_proved(&function);
    let types = types(false, 32);
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function);
    assert_eq!(
        resolver.resolve_rvalue_v2(value(&function, 3, 0)),
        Err(UNPROVEN)
    );
    assert!(resolver.resolve_rvalue_v2(unchecked(&function)).is_ok());
}

#[test]
fn cached_operand_binding_walks_fail_closed_at_the_work_limit() {
    let function = function(fixture(SemanticUncheckedBinaryOpV1::Add));
    let types = types(false, 32);
    let mut bindings = BindingAuditV1::new(&function, BudgetV1::default()).unwrap();
    bindings.budget.used = MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1;
    let SemanticRvalueKindV1::UncheckedBinary(operation) = unchecked(&function).kind() else {
        unreachable!();
    };
    assert_eq!(
        bindings.matches_resolver(&types, operation.left(), (2, 0)),
        Err(WORK_LIMIT)
    );
}

#[path = "tests/csr_binding_v1.rs"]
mod csr_binding_v1;
