use super::*;
use crate::semantic_option_dominance::{
    MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1, semantic_unchecked_arithmetic_violation_v1,
    semantic_unchecked_arithmetic_violation_with_calls_v1,
};

mod regressions;
mod unsigned_underflow;

const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
type Block = (Vec<SemanticStatementV1>, SemanticTerminatorKindV1);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn moved(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(local, ty))
}
fn constant(value: u128, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(
            SemanticScalarValueV1::new(value, if ty == BOOL { 1 } else { 8 }).unwrap(),
        ),
    ))
}
fn assignment(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn alias(local: u32, operand: SemanticOperandV1) -> SemanticStatementV1 {
    assignment(local, operand.ty(), SemanticRvalueKindV1::Use(operand))
}
fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn goto(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}
fn switch(operand: SemanticOperandV1, one: u32, zero: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: operand,
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                edge(SemanticEdgeRoleV1::SwitchValue, one),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, zero),
        )
        .unwrap(),
    }
}
fn call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    local: u32,
    ty: SemanticTypeIdV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(local, ty),
                edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}
fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                if ty == BOOL {
                    SemanticAbiExtensionV1::ZeroExtend
                } else {
                    SemanticAbiExtensionV1::None
                },
                0,
                None,
            )
            .unwrap(),
        ),
    )
}
fn abi(tag: u8, args: &[SemanticTypeIdV1], output: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        args.len() as u32,
        args.iter()
            .map(|ty| SemanticAbiArgumentV1::source(direct(*ty)))
            .collect(),
        if output == UNIT {
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore)
        } else {
            direct(output)
        },
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; args.len()])
    .unwrap()
}
fn function(
    tag: u8,
    abi: SemanticFunctionAbiV1,
    locals: &[SemanticTypeIdV1],
    blocks: Vec<Block>,
) -> SemanticFunctionDeclV1 {
    let args = abi.source_input_types().len();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + index as u8; 32]),
                    *ty,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else if index <= args {
                        SemanticLocalRoleV1::Argument(index as u32 - 1)
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    SemanticSourceProvenanceV1::unavailable(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks
            .into_iter()
            .enumerate()
            .map(|(index, (statements, terminator))| {
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([tag + index as u8; 32]),
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
fn caller(blocks: Vec<Block>) -> SemanticFunctionDeclV1 {
    function(
        10,
        abi(10, &[WORD, WORD], WORD),
        &[WORD, WORD, WORD, PAIR, BOOL, BOOL, WORD, UNIT],
        blocks,
    )
}
fn helper(blocks: Vec<Block>) -> SemanticFunctionDeclV1 {
    function(
        30,
        abi(30, &[BOOL], BOOL),
        &[BOOL, BOOL, BOOL, UNIT],
        blocks,
    )
}
fn hint() -> Vec<Block> {
    vec![
        (vec![], switch(copy(1, BOOL), 1, 2)),
        (vec![], call(2, vec![], 3, UNIT, 3)),
        (vec![alias(0, constant(0, BOOL))], goto(4)),
        (vec![alias(0, constant(1, BOOL))], goto(4)),
        (vec![], SemanticTerminatorKindV1::Return),
    ]
}
fn checked_blocks(operation: SemanticUncheckedBinaryOpV1) -> Vec<Block> {
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
                assignment(
                    3,
                    PAIR,
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        operation.checked(),
                        copy(1, WORD),
                        copy(2, WORD),
                    )),
                ),
                alias(4, flag),
            ],
            call(1, vec![moved(4, BOOL)], 5, BOOL, 1),
        ),
        (vec![], switch(moved(5, BOOL), 3, 2)),
        (
            vec![assignment(
                0,
                WORD,
                SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                    operation,
                    copy(1, WORD),
                    copy(2, WORD),
                )),
            )],
            SemanticTerminatorKindV1::Return,
        ),
        (
            vec![alias(0, constant(0, WORD))],
            SemanticTerminatorKindV1::Return,
        ),
    ]
}
fn types() -> Vec<SemanticTypeDeclV1> {
    let word = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let boolean = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    let layouts = [
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(word),
            false,
        )
        .unwrap(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(boolean),
            false,
        )
        .unwrap(),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(word, boolean),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
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
    ];
    [
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD, BOOL]).unwrap()),
        SemanticTypeShapeV1::Unit,
    ]
    .into_iter()
    .zip(layouts)
    .enumerate()
    .map(|(index, (shape, layout))| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([index as u8; 32]),
            SemanticLayoutIdentityV1::from_sha256([index as u8; 32]),
            layout,
            shape,
        )
    })
    .collect()
}
fn callables() -> Vec<SemanticCallableDeclV1> {
    vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([90; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([90; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([90; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([90; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([90; 32]),
                SemanticSourceProvenanceV1::unavailable(),
                abi(90, &[], UNIT),
            ),
            operation: SemanticCompilerIntrinsicOperationV1::ColdPath,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([90; 32]),
        },
    ]
}
fn accepted(functions: &[SemanticFunctionDeclV1], calls: &[SemanticCallableDeclV1]) -> bool {
    semantic_unchecked_arithmetic_violation_with_calls_v1(
        &types(),
        functions,
        calls,
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap()
    .is_none()
}

fn canonical_request(checked: Vec<Block>, mut identity: Vec<Block>) -> InertSemanticMirRequestV1 {
    let root_id = SemanticFunctionIdV1::from_index(2);
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([60; 32]),
        SemanticLayoutIdentityV1::from_sha256([60; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(direct(WORD)),
            SemanticAbiArgumentV1::source(direct(WORD)),
        ],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let root = function(
        60,
        root_abi,
        &[UNIT, WORD, WORD, WORD],
        vec![
            (
                vec![],
                call(0, vec![copy(1, WORD), copy(2, WORD)], 3, WORD, 1),
            ),
            (vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_role(SemanticFunctionRoleV1::KernelRoot)
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"arithmetic_identity_source".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([60; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    // Defined bodies precede intrinsic callables in the canonical table.
    let mut calls = callables();
    calls.insert(2, SemanticCallableDeclV1::defined(root_id));
    if matches!(&identity[1].1, SemanticTerminatorKindV1::Call(call) if call.callee().index() == 2)
    {
        identity[1].1 = call(3, vec![], 3, UNIT, 3);
    }
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([100; 32])),
        types(),
        vec![],
        vec![],
        vec![],
        vec![caller(checked), helper(identity), root],
        calls,
        vec![root_id],
    )
    .unwrap()
}

#[test]
fn canonical_admission_and_decode_use_retained_boolean_identity_proofs() {
    for operation in [
        SemanticUncheckedBinaryOpV1::Add,
        SemanticUncheckedBinaryOpV1::Subtract,
        SemanticUncheckedBinaryOpV1::Multiply,
    ] {
        let source = caller(checked_blocks(operation));
        assert!(
            semantic_unchecked_arithmetic_violation_v1(&source)
                .unwrap()
                .is_some()
        );
        let admitted = canonical_request(checked_blocks(operation), hint())
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        admitted.require_complete_kernel_entries().unwrap();
        assert_eq!(admitted.functions()[0], source);
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.functions(), admitted.functions());
        assert_eq!(decoded.callables(), admitted.callables());
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
    }
}

#[test]
fn canonical_admission_rejects_mutated_proofs_and_nonidentity_bodies() {
    for mutation in 0..7 {
        let mut blocks = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
        let mut identity = hint();
        match mutation {
            0 => blocks[1].0.push(alias(1, constant(9, WORD))),
            1 => blocks[1].0.push(alias(5, constant(0, BOOL))),
            2 => blocks[0].0.push(alias(4, constant(0, BOOL))),
            3 => blocks[3].1 = goto(2),
            4 => {
                blocks[2].0[0] = assignment(
                    0,
                    WORD,
                    SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                        SemanticUncheckedBinaryOpV1::Add,
                        copy(1, WORD),
                        constant(1, WORD),
                    )),
                );
            }
            5 => {
                identity[2].0[0] = alias(0, constant(1, BOOL));
                identity[3].0[0] = alias(0, constant(0, BOOL));
            }
            // Trap after ColdPath so the retained callable closure stays complete.
            6 => identity[3].1 = SemanticTerminatorKindV1::Abort,
            _ => unreachable!(),
        }
        let error = canonical_request(blocks, identity)
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err();
        assert!(
            matches!(
                error,
                SemanticMirErrorV1::UnprovenUncheckedArithmetic {
                    operation: SemanticUncheckedBinaryOpV1::Add,
                    location: SemanticMirLocationV1::Statement { function, block, statement: 0 },
                } if function.index() == 0 && block.index() == 2
            ),
            "mutation {mutation}: {error:?}"
        );
    }
}

#[test]
fn retained_branching_boolean_identity_preserves_checked_add_sub_mul_proofs() {
    for operation in [
        SemanticUncheckedBinaryOpV1::Add,
        SemanticUncheckedBinaryOpV1::Subtract,
        SemanticUncheckedBinaryOpV1::Multiply,
    ] {
        let functions = vec![caller(checked_blocks(operation)), helper(hint())];
        let before = functions.clone();
        assert!(
            semantic_unchecked_arithmetic_violation_v1(&functions[0])
                .unwrap()
                .is_some()
        );
        assert!(accepted(&functions, &callables()));
        assert_eq!(functions, before);
    }
}

#[test]
fn exact_leaf_copy_move_identity_is_supported_without_name_matching() {
    let leaf = helper(vec![(
        vec![alias(2, moved(1, BOOL)), alias(0, moved(2, BOOL))],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(accepted(
        &[
            caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
            leaf
        ],
        &callables()
    ));
}

#[test]
fn stale_flags_operands_and_move_after_use_do_not_cross_identity_calls() {
    for mutation in 0..4 {
        let mut blocks = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
        match mutation {
            0 => blocks[1].0.push(alias(1, constant(9, WORD))),
            1 => blocks[1].0.push(alias(5, constant(0, BOOL))),
            2 => blocks[0].0.push(alias(4, constant(0, BOOL))),
            3 => blocks[1].1 = switch(copy(4, BOOL), 3, 2),
            _ => unreachable!(),
        }
        assert!(
            !accepted(&[caller(blocks), helper(hint())], &callables()),
            "mutation {mutation}"
        );
    }
}

#[test]
fn overflow_edge_bypass_and_different_checked_operands_stay_rejected() {
    let mut bypass = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
    bypass[3].1 = goto(2);
    assert!(!accepted(&[caller(bypass), helper(hint())], &callables()));
    let mut wrong = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
    wrong[2].0[0] = assignment(
        0,
        WORD,
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            SemanticUncheckedBinaryOpV1::Add,
            copy(1, WORD),
            constant(1, WORD),
        )),
    );
    assert!(!accepted(&[caller(wrong), helper(hint())], &callables()));
}

#[test]
fn boolean_nonidentity_and_unknown_callee_are_not_summarized() {
    let mut negated = hint();
    negated[2].0[0] = alias(0, constant(1, BOOL));
    negated[3].0[0] = alias(0, constant(0, BOOL));
    let caller = caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add));
    assert!(!accepted(&[caller.clone(), helper(negated)], &callables()));
    let fixed = helper(vec![(
        vec![alias(0, constant(0, BOOL))],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(!accepted(&[caller.clone(), fixed], &callables()));
    let mut unknown = callables();
    unknown[1] = SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(99));
    assert!(!accepted(&[caller, helper(hint())], &unknown));
}

#[test]
fn traps_recursion_and_unreachable_unknown_effects_reject_identity_summary() {
    for mutation in 0..4 {
        let mut body = hint();
        match mutation {
            0 => body[1].1 = SemanticTerminatorKindV1::Abort,
            1 => body[1].1 = call(1, vec![copy(1, BOOL)], 2, BOOL, 3),
            2 => body.push((vec![], call(99, vec![], 3, UNIT, 4))),
            3 => body[1].1 = goto(1),
            _ => unreachable!(),
        }
        assert!(
            !accepted(
                &[
                    caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
                    helper(body)
                ],
                &callables()
            ),
            "mutation {mutation}"
        );
    }
    let mut calls = callables();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut calls[2] else {
        unreachable!();
    };
    *operation = SemanticCompilerIntrinsicOperationV1::Trap;
    assert!(!accepted(
        &[
            caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
            helper(hint())
        ],
        &calls
    ));
}

#[test]
fn boolean_abi_ownership_and_type_are_required() {
    for ownership in [
        SemanticSourceArgumentOwnershipV1::Unspecified,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    ] {
        let abi = abi(30, &[BOOL], BOOL)
            .with_source_argument_ownership(vec![ownership])
            .unwrap();
        let bad = function(30, abi, &[BOOL, BOOL, BOOL, UNIT], hint());
        assert!(!accepted(
            &[
                caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
                bad
            ],
            &callables()
        ));
    }
    let functions = [
        caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
        helper(hint()),
    ];
    let mut wrong_types = types();
    wrong_types[1] = SemanticTypeDeclV1::new(
        wrong_types[1].identity(),
        wrong_types[1].layout_identity(),
        wrong_types[1].layout().clone(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 8,
        }),
    );
    assert!(
        semantic_unchecked_arithmetic_violation_with_calls_v1(
            &wrong_types,
            &functions,
            &callables(),
            SemanticFunctionIdV1::from_index(0)
        )
        .unwrap()
        .is_some()
    );
}

#[test]
fn identity_call_transfer_is_tied_to_exact_function_and_return_edge() {
    let functions = [
        caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
        helper(hint()),
    ];
    let calls = ArithmeticIdentityCallsV1::analyze(
        &types(),
        &functions,
        &callables(),
        &functions[0],
        &mut WorkBudgetV1::default(),
    )
    .unwrap();
    assert!(matches!(
        calls.transfer(&functions[0], 0, 1, 5, BOOL, false),
        ArithmeticCallTransferV1::Returned(_)
    ));
    assert!(matches!(
        calls.transfer(&functions[0].clone(), 0, 1, 5, BOOL, false),
        ArithmeticCallTransferV1::Unknown
    ));
    assert!(matches!(
        calls.transfer(&functions[0], 0, 2, 5, BOOL, false),
        ArithmeticCallTransferV1::Unknown
    ));
    assert!(matches!(
        calls.transfer(&functions[0], 0, 1, 5, WORD, false),
        ArithmeticCallTransferV1::Unknown
    ));
}

#[test]
fn identity_summary_and_analysis_limits_fail_closed() {
    let mut body = hint();
    body[0].0 = vec![
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Nop
        );
        MAX_IDENTITY_STATEMENTS + 1
    ];
    let functions = [
        caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
        helper(body),
    ];
    assert!(!accepted(&functions, &callables()));
    let mut budget = WorkBudgetV1::default();
    budget
        .charge(MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1)
        .unwrap();
    assert!(
        ArithmeticIdentityCallsV1::analyze(
            &types(),
            &functions,
            &callables(),
            &functions[0],
            &mut budget
        )
        .is_err()
    );
    assert!(
        semantic_unchecked_arithmetic_violation_with_calls_v1(
            &types(),
            &functions,
            &callables(),
            SemanticFunctionIdV1::from_index(99)
        )
        .is_err()
    );
}
