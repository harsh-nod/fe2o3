// Included in the existing parent test module to exercise the production hook,
// not merely the private comparison method.
fn unsigned_comparison_guarded_exclusive_fixture_v1(
    bound: u64,
) -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
    usize,
    usize,
    usize,
) {
    let (types, callables, original) = derived_exclusive_index_fixture(64, 16, 63, true, false);
    let mut locals = original.locals().to_vec();
    let masked = locals.len() as u32 - 2;
    let condition = locals.len();
    locals.push(local(250, BOOL_TYPE, SemanticLocalRoleV1::Temporary));
    let mut blocks = original.blocks().to_vec();
    let write_block = blocks.len();
    let return_block = write_block + 1;
    let original_write = blocks[7].clone();
    let mut statements = original_write.statements().to_vec();
    statements.push(typed_assignment(
        condition as u32,
        BOOL_TYPE,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::GreaterOrEqual,
            left: typed_operand(masked, U64_TYPE),
            right: typed_constant(U64_TYPE, u128::from(bound), 8),
        },
    ));
    blocks[7] = block(
        250,
        statements,
        zero_switch(
            condition as u32,
            BOOL_TYPE,
            write_block as u32,
            return_block as u32,
        ),
    );
    blocks.push(block(
        251,
        vec![],
        original_write.terminator().kind().clone(),
    ));
    blocks.push(block(252, vec![], SemanticTerminatorKindV1::Return));
    (
        types,
        callables,
        typed_global_fixture_with_body_v1(&original, locals, blocks),
        condition,
        write_block,
        return_block,
    )
}

#[test]
fn unsigned_comparison_production_hook_retains_original_ge_false_edge() {
    let (types, callables, function, condition, write, returned) =
        unsigned_comparison_guarded_exclusive_fixture_v1(16);
    let retained = function.clone();
    let (projection, operations) =
        project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
            .unwrap();
    let [(lhs, rhs)] = projection.direct_switch_predicates[condition]
        .as_ref()
        .expect("production source comparison hook")
        .comparisons
        .as_slice()
    else {
        panic!()
    };
    assert!(projection.deterministic_switches[7].is_none());
    let projected = projected_cfg_terminator(
        &function,
        7,
        &callables,
        true,
        &constant_locals(&function).unwrap(),
        &projection.direct_switch_predicates,
        &projection.deterministic_switches,
    )
    .unwrap();
    assert!(
        matches!(projected, ProjectedCfgTerminatorV1::Predicate { true_block, false_block, .. }
        if true_block == returned && false_block == write)
    );
    let effect = projection.direct_write_effects[write].as_ref().unwrap();
    for invocation in 0..1024 {
        let inactive = evaluate_projected_index(&operations, *lhs, invocation)
            < evaluate_projected_index(&operations, *rhs, invocation);
        assert_eq!(inactive, invocation & 63 >= 16);
        assert_eq!(
            evaluate_projected_index(&operations, effect.indices[0], invocation),
            (invocation / 64) * 16 + (invocation & 63)
        );
    }
    assert_eq!(function, retained);
}

#[test]
fn unsigned_comparison_production_hook_does_not_strengthen_wrong_threshold() {
    let (types, callables, function, condition, write, _) =
        unsigned_comparison_guarded_exclusive_fixture_v1(63);
    let (projection, operations) =
        project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
            .unwrap();
    let [(lhs, rhs)] = projection.direct_switch_predicates[condition]
        .as_ref()
        .unwrap()
        .comparisons
        .as_slice()
    else {
        panic!()
    };
    let effect = projection.direct_write_effects[write].as_ref().unwrap();
    for invocation in [16, 64] {
        assert!(
            evaluate_projected_index(&operations, *lhs, invocation)
                >= evaluate_projected_index(&operations, *rhs, invocation)
        );
        assert_eq!(
            evaluate_projected_index(&operations, effect.indices[0], invocation),
            16
        );
    }
}
