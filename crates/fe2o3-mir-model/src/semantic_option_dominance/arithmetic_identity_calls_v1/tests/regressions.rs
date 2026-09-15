use super::*;
use crate::semantic_option_dominance::semantic_unchecked_arithmetic_violation_with_budget_v1;

fn scalar_constant(ty: SemanticTypeIdV1, bits: u128, bytes: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, bytes).unwrap()),
    ))
}

fn zst_constant(ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::ZeroSized,
    ))
}

fn overflow_operand(moving: bool) -> SemanticOperandV1 {
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(3),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), BOOL).unwrap()],
        BOOL,
    )
    .unwrap();
    if moving {
        SemanticOperandV1::Move(field)
    } else {
        SemanticOperandV1::Copy(field)
    }
}

#[test]
fn constant_payloads_require_the_exact_declared_bool_or_unit_type() {
    let types = types();
    let function = helper(hint());
    for operand in [
        scalar_constant(BOOL, 0, 1),
        scalar_constant(BOOL, 1, 1),
        zst_constant(UNIT),
    ] {
        assert!(exact_operand(&types, &function, &operand));
    }
    for operand in [
        zst_constant(BOOL),
        scalar_constant(UNIT, 0, 1),
        scalar_constant(WORD, 0, 1),
        scalar_constant(BOOL, 2, 1),
        scalar_constant(BOOL, 1, 8),
        zst_constant(SemanticTypeIdV1::from_index(99)),
    ] {
        assert!(!exact_operand(&types, &function, &operand), "{operand:?}");
    }
    let mut identity = hint();
    identity[0].0.push(alias(3, zst_constant(UNIT)));
    assert!(accepted(
        &[
            caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
            helper(identity)
        ],
        &callables(),
    ));
    for operand in [zst_constant(BOOL), scalar_constant(UNIT, 0, 1)] {
        let mut identity = hint();
        let local = if operand.ty() == BOOL { 2 } else { 3 };
        identity[0].0.push(alias(local, operand));
        assert!(!accepted(
            &[
                caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
                helper(identity)
            ],
            &callables(),
        ));
    }
}

#[test]
fn canonical_structural_errors_in_later_helpers_precede_arithmetic_analysis() {
    for operand in [zst_constant(BOOL), scalar_constant(UNIT, 0, 1)] {
        let mut blocks = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
        blocks[1].0.push(alias(5, constant(0, BOOL)));
        let mut identity = hint();
        let local = if operand.ty() == BOOL { 2 } else { 3 };
        identity[0].0.push(alias(local, operand));
        let error = canonical_request(blocks, identity)
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err();
        assert!(
            matches!(
                error,
                SemanticMirErrorV1::InvalidTypeOperation {
                    operation: SemanticTypeOperationV1::Constant,
                    location: SemanticMirLocationV1::Statement { function, .. },
                } if function.index() == 1
            ),
            "{error:?}"
        );
    }
}

#[test]
fn identity_calls_can_move_back_into_the_same_flag_local() {
    let mut blocks = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
    blocks[0].1 = call(1, vec![moved(4, BOOL)], 4, BOOL, 1);
    blocks[1].1 = switch(moved(4, BOOL), 3, 2);
    assert!(accepted(
        &[caller(blocks.clone()), helper(hint())],
        &callables()
    ));
    canonical_request(blocks.clone(), hint())
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    blocks[1].0.push(alias(4, constant(0, BOOL)));
    assert!(!accepted(&[caller(blocks), helper(hint())], &callables()));
}

#[test]
fn projected_overflow_moves_return_a_flag_without_reviving_the_moved_field() {
    let mut blocks = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
    blocks[0].0.truncate(1);
    blocks[0].1 = call(1, vec![overflow_operand(true)], 5, BOOL, 1);
    assert!(accepted(
        &[caller(blocks.clone()), helper(hint())],
        &callables()
    ));
    canonical_request(blocks.clone(), hint())
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    blocks[1].1 = switch(overflow_operand(false), 3, 2);
    assert!(!accepted(&[caller(blocks), helper(hint())], &callables()));
}

fn joined_identity() -> Vec<Block> {
    let mut blocks = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
    blocks[1].1 = switch(copy(5, BOOL), 4, 5);
    blocks[2].0[0] = assignment(
        0,
        WORD,
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            SemanticUncheckedBinaryOpV1::Add,
            copy(6, WORD),
            copy(2, WORD),
        )),
    );
    for _ in 0..2 {
        blocks.push((
            vec![alias(4, copy(5, BOOL)), alias(6, copy(1, WORD))],
            goto(6),
        ));
    }
    blocks.push((vec![], switch(moved(4, BOOL), 3, 2)));
    blocks
}

#[test]
fn call_provenance_joins_require_agreeing_flags_and_operands_on_every_predecessor() {
    let blocks = joined_identity();
    assert!(accepted(
        &[caller(blocks.clone()), helper(hint())],
        &callables()
    ));
    canonical_request(blocks, hint())
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    for mutation in 0..3 {
        let mut blocks = joined_identity();
        match mutation {
            0 => blocks[5].0[0] = alias(4, constant(0, BOOL)),
            1 => blocks[5].0[1] = alias(6, constant(9, WORD)),
            // The recursive arithmetic caller has no finite bool-identity summary.
            2 => blocks[5].1 = call(0, vec![copy(1, WORD), copy(2, WORD)], 6, WORD, 6),
            _ => unreachable!(),
        }
        assert!(
            !accepted(&[caller(blocks), helper(hint())], &callables()),
            "{mutation}"
        );
    }
}

fn looping_identity() -> Vec<Block> {
    let mut blocks = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
    blocks[1].1 = switch(copy(5, BOOL), 3, 2);
    let body = std::mem::replace(&mut blocks[2], (vec![], goto(4)));
    blocks.push((body.0, switch(copy(5, BOOL), 3, 4)));
    blocks
}

#[test]
fn call_backedges_preserve_only_invariant_values_and_exact_identity_transfers() {
    for mutation in 0..4 {
        let mut blocks = looping_identity();
        match mutation {
            0 => (),
            1 => {
                blocks[4].1 = switch(copy(5, BOOL), 3, 5);
                blocks.push((vec![], call(1, vec![moved(5, BOOL)], 5, BOOL, 4)));
            }
            2 => blocks[4].0.push(alias(1, constant(9, WORD))),
            3 => {
                blocks[4].1 = switch(copy(5, BOOL), 3, 5);
                blocks.push((
                    vec![],
                    call(0, vec![copy(1, WORD), copy(2, WORD)], 6, WORD, 4),
                ));
            }
            _ => unreachable!(),
        }
        assert_eq!(
            accepted(&[caller(blocks), helper(hint())], &callables()),
            mutation < 2,
            "{mutation}"
        );
    }
}

fn with_unwind(
    terminator: &SemanticTerminatorKindV1,
    unwind: SemanticUnwindActionV1,
) -> SemanticTerminatorKindV1 {
    let SemanticTerminatorKindV1::Call(call) = terminator else {
        unreachable!()
    };
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            call.callee(),
            call.arguments().to_vec(),
            call.destination().cloned(),
            unwind,
        )
        .unwrap(),
    )
}

#[test]
fn caller_and_callee_unwind_paths_never_receive_identity_transfer() {
    for unwind in [
        SemanticUnwindActionV1::Continue,
        SemanticUnwindActionV1::Terminate,
        SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 3)),
    ] {
        let mut blocks = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
        blocks[0].1 = with_unwind(&blocks[0].1, unwind);
        assert!(!accepted(&[caller(blocks), helper(hint())], &callables()));
        let mut identity = hint();
        identity[1].1 = with_unwind(&identity[1].1, unwind);
        assert!(!accepted(
            &[
                caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
                helper(identity)
            ],
            &callables()
        ));
    }
    let unwinding_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([30; 32]),
        SemanticLayoutIdentityV1::from_sha256([30; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        true,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct(BOOL))],
        direct(BOOL),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let identity = function(30, unwinding_abi, &[BOOL, BOOL, BOOL, UNIT], hint());
    assert!(!accepted(
        &[
            caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
            identity
        ],
        &callables()
    ));
}

#[test]
fn repeated_function_analyses_consume_one_shared_budget() {
    let functions = [
        caller(checked_blocks(SemanticUncheckedBinaryOpV1::Add)),
        helper(hint()),
    ];
    let types = types();
    let calls = callables();
    let mut measured = WorkBudgetV1::default();
    assert!(
        semantic_unchecked_arithmetic_violation_with_budget_v1(
            &types,
            &functions,
            &calls,
            SemanticFunctionIdV1::from_index(0),
            &mut measured,
        )
        .unwrap()
        .is_none()
    );
    assert!(measured.used() > 0);
    let mut shared = WorkBudgetV1::with_limit(measured.used());
    assert!(
        semantic_unchecked_arithmetic_violation_with_budget_v1(
            &types,
            &functions,
            &calls,
            SemanticFunctionIdV1::from_index(0),
            &mut shared,
        )
        .unwrap()
        .is_none()
    );
    assert!(matches!(
        semantic_unchecked_arithmetic_violation_with_budget_v1(
            &types,
            &functions,
            &calls,
            SemanticFunctionIdV1::from_index(0),
            &mut shared,
        ),
        Err(SemanticOptionDominanceErrorV1::WorkLimit { .. })
    ));
}

fn minimum_validation_budget(request: &InertSemanticMirRequestV1) -> u64 {
    let mut low = 0;
    let mut high = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
    request
        .clone()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    while low < high {
        let middle = low + (high - low) / 2;
        let limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::ValidationWork, middle)
            .unwrap();
        match request.clone().admit_current_production(limits) {
            Ok(_) => high = middle,
            Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                ..
            }) => low = middle + 1,
            Err(error) => panic!("unexpected admission failure: {error:?}"),
        }
    }
    low
}

#[test]
fn canonical_request_budget_includes_all_arithmetic_analysis_work() {
    let proven = canonical_request(checked_blocks(SemanticUncheckedBinaryOpV1::Add), hint());
    let admitted = proven
        .clone()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let mut arithmetic = WorkBudgetV1::default();
    assert!(
        semantic_unchecked_arithmetic_violation_with_budget_v1(
            admitted.types(),
            admitted.functions(),
            admitted.callables(),
            SemanticFunctionIdV1::from_index(0),
            &mut arithmetic,
        )
        .unwrap()
        .is_none()
    );
    let mut blocks = checked_blocks(SemanticUncheckedBinaryOpV1::Add);
    blocks[2].0[0] = assignment(
        0,
        WORD,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left: copy(1, WORD),
            right: copy(2, WORD),
        },
    );
    let no_unchecked = canonical_request(blocks, hint());
    assert_eq!(
        minimum_validation_budget(&proven),
        minimum_validation_budget(&no_unchecked) + arithmetic.used() as u64
    );
}
