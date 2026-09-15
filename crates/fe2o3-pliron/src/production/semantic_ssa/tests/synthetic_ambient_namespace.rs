use super::*;

fn fixture() -> (
    SemanticFunctionDeclV1,
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
) {
    let function = test_implicit_scope_function(
        0,
        vec![test_block(
            104,
            vec![test_typed_borrow(2, 2, 1, 0)],
            test_call(
                0,
                vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                None,
            ),
        )],
    );
    let callables = vec![test_intrinsic_callable(function.abi().clone())];
    (function, implicit_scope_types(), callables)
}

#[test]
fn synthetic_events_and_edge_definitions_have_no_source_local_ambient_authority() {
    let (function, types, callables) = fixture();
    let sites = adapter::typed_transparent_borrow_sites_v1(&function, &types, &callables);
    let (input, original, _) =
        adapter::semantic_function_ssa_input_v1(&function, Some(&types), &callables, &sites);
    assert_eq!(original, [SsaVariableIdV1::new(1)]);
    let synthetic = SsaVariableIdV1::new(input.variable_count());
    let mut events = input.blocks()[0].events().to_vec();
    events.extend([
        SsaEventV1::Define(synthetic),
        SsaEventV1::Use(synthetic),
        SsaEventV1::Kill(synthetic),
    ]);
    let edge = SsaEdgeInputV1::new(SsaEdgeRoleV1::new(1), SsaBlockIdV1::new(0), vec![synthetic]);
    let observe = |events, edges| {
        adapter::authenticated_implicit_entry_variables_v1(
            &function,
            Some(&types),
            &callables,
            &sites,
            &vec![true; input.variable_count() as usize + 1],
            &[SsaBlockInputV1::new(events, edges)],
        )
    };
    assert_eq!(observe(events.clone(), vec![edge.clone()]), original);
    events.push(SsaEventV1::Define(SsaVariableIdV1::new(1)));
    assert!(observe(events, vec![edge]).is_empty());
    let source_edge = SsaEdgeInputV1::new(
        SsaEdgeRoleV1::new(1),
        SsaBlockIdV1::new(0),
        vec![SsaVariableIdV1::new(1)],
    );
    assert!(observe(input.blocks()[0].events().to_vec(), vec![source_edge]).is_empty());
}

#[test]
fn synthetic_use_still_requires_its_own_ssa_definition() {
    let (function, types, callables) = fixture();
    let sites = adapter::typed_transparent_borrow_sites_v1(&function, &types, &callables);
    let (input, original, _) =
        adapter::semantic_function_ssa_input_v1(&function, Some(&types), &callables, &sites);
    let synthetic = SsaVariableIdV1::new(input.variable_count());
    for defined in [false, true] {
        let mut events = input.blocks()[0].events().to_vec();
        if defined {
            events.push(SsaEventV1::Define(synthetic));
        }
        events.push(SsaEventV1::Use(synthetic));
        let blocks = vec![SsaBlockInputV1::new(events, vec![])];
        let promotable = vec![true; input.variable_count() as usize + 1];
        let implicit = adapter::authenticated_implicit_entry_variables_v1(
            &function,
            Some(&types),
            &callables,
            &sites,
            &promotable,
            &blocks,
        );
        assert_eq!(implicit, original);
        assert!(!implicit.contains(&synthetic));
        let input = SsaConstructionInputV1::new(
            input.entry(),
            input.variable_count() + 1,
            promotable,
            implicit,
            blocks,
        );
        let result = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default());
        if defined {
            result.expect("synthetic lifecycle definition remains a separate SSA obligation");
        } else {
            assert!(matches!(result,
                Err(SsaPlannerErrorV1::UndefinedAtUse { variable, .. }) if variable == synthetic
            ));
        }
    }
}
