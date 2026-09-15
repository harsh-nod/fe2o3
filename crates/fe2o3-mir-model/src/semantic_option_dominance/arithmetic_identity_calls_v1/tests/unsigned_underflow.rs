use super::*;
use crate::semantic_option_dominance::semantic_unchecked_arithmetic_violation_with_types_v1;

fn comparison(left: SemanticOperandV1, right: SemanticOperandV1) -> SemanticStatementV1 {
    assignment(
        4,
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right,
        },
    )
}

fn subtraction(left: SemanticOperandV1, right: SemanticOperandV1) -> SemanticStatementV1 {
    assignment(
        0,
        WORD,
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            SemanticUncheckedBinaryOpV1::Subtract,
            left,
            right,
        )),
    )
}

fn fixture() -> Vec<Block> {
    vec![
        (
            vec![comparison(copy(1, WORD), copy(2, WORD))],
            switch(moved(4, BOOL), 1, 2),
        ),
        (
            vec![alias(0, constant(0, WORD))],
            SemanticTerminatorKindV1::Return,
        ),
        (
            vec![subtraction(copy(1, WORD), copy(2, WORD))],
            SemanticTerminatorKindV1::Return,
        ),
    ]
}

fn check(types: &[SemanticTypeDeclV1], blocks: Vec<Block>, expected: bool) {
    let function = caller(blocks);
    let original = function.clone();
    let result = semantic_unchecked_arithmetic_violation_with_types_v1(types, &function).unwrap();
    assert_eq!(result.is_none(), expected, "{result:?}");
    if let Some(violation) = result {
        assert_eq!(violation.operation(), SemanticUncheckedBinaryOpV1::Subtract);
        let statement = &function.blocks()[violation.block().index() as usize].statements()
            [violation.statement() as usize];
        assert!(
            matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::UncheckedBinary(_)))
        );
    }
    assert_eq!(
        function, original,
        "arithmetic proof must retain original MIR"
    );
}

fn integer_types(signed: bool, bits: u16) -> Vec<SemanticTypeDeclV1> {
    let mut types = types();
    let bytes = u64::from(bits / 8);
    let word = &types[0];
    types[0] = SemanticTypeDeclV1::new(
        word.identity(),
        word.layout_identity(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, bits, bytes),
                SemanticScalarValidityRangeV1::new(0, u128::MAX >> (128 - bits)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
    );
    types
}

#[test]
fn unsigned_underflow_requires_typed_unsigned_operands_and_exact_false_edge() {
    let original = caller(fixture());
    assert!(
        semantic_unchecked_arithmetic_violation_v1(&original)
            .unwrap()
            .is_some(),
        "the untyped standalone API cannot infer signedness from a type ID"
    );
    for bits in [8, 16, 32, 64, 128] {
        check(&integer_types(false, bits), fixture(), true);
        check(&integer_types(true, bits), fixture(), false);
    }
    let mut wrong_edge = fixture();
    wrong_edge[0].1 = switch(moved(4, BOOL), 2, 1);
    check(&types(), wrong_edge, false);
    let mut bypass = fixture();
    bypass[0].1 = goto(2);
    check(&types(), bypass, false);
    let mut rejoined = fixture();
    rejoined[1].1 = goto(2);
    check(&types(), rejoined, false);
    let mut nonboolean = fixture();
    nonboolean[0].1 = SemanticTerminatorKindV1::SwitchInt {
        discriminant: moved(4, BOOL),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                2,
                edge(SemanticEdgeRoleV1::SwitchValue, 1),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
        )
        .unwrap(),
    };
    check(&types(), nonboolean, false);
}

#[test]
fn unsigned_underflow_rejects_operand_swap_operator_and_nominal_type_substitutions() {
    let mut swapped = fixture();
    swapped[0].0[0] = comparison(copy(2, WORD), copy(1, WORD));
    check(&types(), swapped, false);
    let mut swapped = fixture();
    swapped[2].0[0] = subtraction(copy(2, WORD), copy(1, WORD));
    check(&types(), swapped, false);
    for operation in [
        SemanticBinaryOpV1::GreaterThan,
        SemanticBinaryOpV1::LessOrEqual,
        SemanticBinaryOpV1::Equal,
    ] {
        let mut blocks = fixture();
        blocks[0].0[0] = assignment(
            4,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation,
                left: copy(1, WORD),
                right: copy(2, WORD),
            },
        );
        check(&types(), blocks, false);
    }
    let mut wrong_result = fixture();
    wrong_result[0].0[0] = assignment(
        4,
        WORD,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: copy(1, WORD),
            right: copy(2, WORD),
        },
    );
    check(&types(), wrong_result, false);
    let mut wrong_destination = fixture();
    wrong_destination[0].0[0] = assignment(
        6,
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: copy(1, WORD),
            right: copy(2, WORD),
        },
    );
    wrong_destination[0].1 = switch(copy(6, BOOL), 1, 2);
    check(&types(), wrong_destination, false);
    let mut wrong_sub_result = fixture();
    wrong_sub_result[2].0[0] = assignment(
        0,
        BOOL,
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            SemanticUncheckedBinaryOpV1::Subtract,
            copy(1, WORD),
            copy(2, WORD),
        )),
    );
    check(&types(), wrong_sub_result, false);
    for bits in [32, 64] {
        let mut declarations = types();
        let other = SemanticTypeIdV1::from_index(declarations.len() as u32);
        let declaration = integer_types(false, bits).remove(0);
        declarations.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([199; 32]),
            declaration.layout_identity(),
            declaration.layout().clone(),
            declaration.shape().clone(),
        ));
        let mut blocks = fixture();
        blocks[0].0[0] = comparison(copy(1, WORD), copy(2, other));
        check(&declarations, blocks, false);
    }
    let wrong_size = SemanticOperandV1::Constant(SemanticConstantV1::new(
        WORD,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 4).unwrap()),
    ));
    let mut blocks = fixture();
    blocks[0].0[0] = comparison(copy(1, WORD), wrong_size.clone());
    blocks[2].0[0] = subtraction(copy(1, WORD), wrong_size);
    check(&types(), blocks, false);
}

#[test]
fn unsigned_underflow_tracks_move_aliases_and_rejects_reassigned_values() {
    let mut aliases = fixture();
    aliases[0].0 = vec![
        alias(6, moved(1, WORD)),
        comparison(copy(6, WORD), copy(2, WORD)),
        alias(5, moved(4, BOOL)),
    ];
    aliases[0].1 = switch(moved(5, BOOL), 1, 2);
    aliases[2].0[0] = subtraction(moved(6, WORD), moved(2, WORD));
    check(&types(), aliases.clone(), true);
    for (local, operand) in [
        (6, constant(9, WORD)),
        (2, constant(1, WORD)),
        (5, constant(0, BOOL)),
    ] {
        let mut blocks = aliases.clone();
        blocks[0].0.push(alias(local, operand));
        check(&types(), blocks, false);
    }
    let mut moved_flag = fixture();
    moved_flag[0].0.push(alias(5, moved(4, BOOL)));
    check(&types(), moved_flag, false);
    let mut moved_operand = fixture();
    moved_operand[0].0[0] = comparison(moved(1, WORD), copy(2, WORD));
    check(&types(), moved_operand, false);
    let mut intra_statement_move = fixture();
    intra_statement_move[0].0 = vec![
        alias(6, copy(1, WORD)),
        comparison(moved(1, WORD), copy(1, WORD)),
    ];
    intra_statement_move[2].0[0] = subtraction(copy(6, WORD), copy(6, WORD));
    check(&types(), intra_statement_move, false);
    let mut intra_statement_move = fixture();
    intra_statement_move[0].0 = vec![
        alias(6, copy(1, WORD)),
        comparison(copy(1, WORD), copy(1, WORD)),
    ];
    intra_statement_move[2].0[0] = subtraction(moved(6, WORD), copy(6, WORD));
    check(&types(), intra_statement_move, false);
    let mut reassigned_predicate = fixture();
    reassigned_predicate[0]
        .0
        .push(comparison(copy(2, WORD), copy(1, WORD)));
    check(&types(), reassigned_predicate, false);
    for kind in [
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(6)),
        SemanticStatementKindV1::Deinitialize(place(6, WORD)),
    ] {
        let mut blocks = aliases.clone();
        blocks[2].0.insert(
            0,
            SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
        );
        check(&types(), blocks, false);
    }
}

#[test]
fn unsigned_underflow_join_and_backedge_sources_must_agree() {
    let mut blocks = fixture();
    blocks[0].0.push(alias(5, constant(0, BOOL)));
    blocks[0].1 = switch(copy(5, BOOL), 3, 4);
    blocks.extend([
        (vec![alias(5, copy(4, BOOL))], goto(5)),
        (vec![alias(5, copy(4, BOOL))], goto(5)),
        (vec![], switch(moved(5, BOOL), 1, 2)),
    ]);
    check(&types(), blocks.clone(), true);
    let mut disagreement = blocks.clone();
    disagreement[4].0[0] = alias(5, constant(0, BOOL));
    check(&types(), disagreement, false);
    let mut disagreement = blocks.clone();
    disagreement[4]
        .0
        .insert(0, comparison(copy(2, WORD), copy(1, WORD)));
    check(&types(), disagreement, false);
    // An invariant flag can pass through a loop; reevaluating a comparison
    // at a cyclic definition site is not an exact dynamic version.
    let mut invariant = fixture();
    invariant[0].0.push(alias(5, moved(4, BOOL)));
    invariant[0].1 = goto(3);
    invariant.push((vec![], switch(copy(5, BOOL), 3, 4)));
    invariant.push((vec![], switch(moved(5, BOOL), 1, 2)));
    check(&types(), invariant.clone(), true);
    invariant[3].0.push(alias(5, constant(0, BOOL)));
    check(&types(), invariant, false);
    let mut cyclic = fixture();
    cyclic[1].1 = goto(0);
    check(&types(), cyclic, false);
}

fn through_hint(mut blocks: Vec<Block>) -> Vec<Block> {
    blocks[0].1 = call(1, vec![moved(4, BOOL)], 5, BOOL, 3);
    blocks.push((vec![], switch(moved(5, BOOL), 1, 2)));
    blocks
}

#[test]
fn unsigned_underflow_canonical_admission_roundtrip_retains_comparison_and_call_proof() {
    let blocks = through_hint(fixture());
    let functions = [caller(blocks.clone()), helper(hint())];
    assert!(accepted(&functions, &callables()));
    let mut unknown = hint();
    unknown[3].1 = SemanticTerminatorKindV1::Abort;
    assert!(!accepted(
        &[caller(blocks.clone()), helper(unknown)],
        &callables()
    ));
    let admitted = canonical_request(blocks.clone(), hint())
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(admitted.functions()[0], caller(blocks.clone()));
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        admitted.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.functions(), admitted.functions());
    let mut stale = blocks;
    stale[2].0.insert(0, alias(2, constant(9, WORD)));
    assert!(matches!(
        canonical_request(stale, hint()).admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::UnprovenUncheckedArithmetic {
            operation: SemanticUncheckedBinaryOpV1::Subtract,
            ..
        })
    ));
}

#[test]
fn unsigned_underflow_uses_the_existing_request_work_budget() {
    use crate::semantic_option_dominance::{
        WorkBudgetV1, semantic_unchecked_arithmetic_violation_with_budget_v1,
    };
    let functions = [caller(through_hint(fixture())), helper(hint())];
    let mut budget = WorkBudgetV1::default();
    for _ in 0..2 {
        let previous = budget.used();
        assert!(
            semantic_unchecked_arithmetic_violation_with_budget_v1(
                &types(),
                &functions,
                &callables(),
                SemanticFunctionIdV1::from_index(0),
                &mut budget,
            )
            .unwrap()
            .is_none()
        );
        assert!(budget.used() > previous);
    }
    let mut empty = WorkBudgetV1::with_limit(0);
    assert!(matches!(
        semantic_unchecked_arithmetic_violation_with_budget_v1(
            &types(),
            &functions,
            &callables(),
            SemanticFunctionIdV1::from_index(0),
            &mut empty,
        ),
        Err(SemanticOptionDominanceErrorV1::WorkLimit { .. })
    ));
}
