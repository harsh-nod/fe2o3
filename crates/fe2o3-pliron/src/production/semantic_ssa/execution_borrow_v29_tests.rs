use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticExecutionOperationV29 as Execution;

fn derive(context: u32) -> SemanticCompilerIntrinsicOperationV1 {
    SemanticCompilerIntrinsicOperationV1::Execution(Execution::WorkgroupDerive {
        context: SemanticTypeIdV1::from_index(context),
        workgroup: SemanticTypeIdV1::from_index(1),
    })
}

fn tile_load(workgroup: u32) -> SemanticCompilerIntrinsicOperationV1 {
    SemanticCompilerIntrinsicOperationV1::Execution(Execution::MaskedTileLoadU32 {
        workgroup: SemanticTypeIdV1::from_index(workgroup),
        tile: SemanticTypeIdV1::from_index(1),
    })
}

fn shared_borrow(reference_local: u32, source: SemanticPlaceV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(
            fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1::new(
                test_typed_place(reference_local, 1),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1::from_index(1),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: source,
                    },
                ),
            ),
        ),
    )
}

fn consumer(
    statements: Vec<SemanticStatementV1>,
    arguments: Vec<SemanticOperandV1>,
) -> SemanticFunctionDeclV1 {
    test_function(vec![test_block(
        232,
        statements,
        test_call(0, arguments, None),
    )])
}

fn reference(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Move(test_scalar_place(local))
}

#[test]
fn execution_derive_borrow_keeps_the_source_use_and_reference_definition() {
    let function = consumer(vec![test_borrow(2, 1)], vec![reference(2)]);
    let callables = [test_operation_callable(
        function.abi().clone(),
        derive(0),
        150,
    )];
    let sites = transparent_borrow_sites_v1(&function, &callables);
    let (input, implicit, _) = semantic_function_ssa_input_v1(&function, None, &callables, &sites);
    assert_eq!(sites.len(), 1);
    assert!(input.promotable()[1]);
    assert!(implicit.is_empty());
    assert_eq!(
        input.blocks()[0].events(),
        [
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
            SsaEventV1::Define(SsaVariableIdV1::new(2)),
            SsaEventV1::Use(SsaVariableIdV1::new(2)),
            SsaEventV1::Kill(SsaVariableIdV1::new(2)),
        ],
    );
    let plan = plan_semantic_function_ssa_with_callables_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &callables,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert!(plan.implicit_entry_variables().is_empty());
}

#[test]
fn execution_derive_reborrow_keeps_each_source_use_and_definition() {
    let function = consumer(
        vec![test_borrow(2, 1), test_reborrow(3, 2)],
        vec![reference(3)],
    );
    let callables = [test_operation_callable(
        function.abi().clone(),
        derive(0),
        150,
    )];
    let sites = transparent_borrow_sites_v1(&function, &callables);
    let (input, implicit, _) = semantic_function_ssa_input_v1(&function, None, &callables, &sites);
    assert_eq!(sites.len(), 2);
    assert!(input.promotable().iter().all(|promotable| *promotable));
    assert!(implicit.is_empty());
    assert_eq!(
        input.blocks()[0].events(),
        [
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
            SsaEventV1::Define(SsaVariableIdV1::new(2)),
            SsaEventV1::Use(SsaVariableIdV1::new(2)),
            SsaEventV1::Define(SsaVariableIdV1::new(3)),
            SsaEventV1::Use(SsaVariableIdV1::new(3)),
            SsaEventV1::Kill(SsaVariableIdV1::new(3)),
        ],
    );
}

#[test]
fn execution_borrow_requires_the_exact_operation_context_and_argument() {
    let context = SemanticTypeIdV1::from_index(0);
    for (operation, arguments) in [
        (derive(1), vec![reference(2)]),
        (derive(0), vec![reference(3), reference(2)]),
        (
            SemanticCompilerIntrinsicOperationV1::Execution(Execution::ContextIssue { context }),
            vec![reference(2)],
        ),
        (
            SemanticCompilerIntrinsicOperationV1::Execution(Execution::MaskedTileIntoFragmentU32 {
                tile: context,
                fragment: SemanticTypeIdV1::from_index(1),
            }),
            vec![reference(2)],
        ),
    ] {
        let function = consumer(vec![test_borrow(2, 1)], arguments);
        let callables = [test_operation_callable(
            function.abi().clone(),
            operation,
            150,
        )];
        assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
        assert!(!source_is_promotable(&function, &callables));
    }
}

#[test]
fn execution_derive_does_not_hide_reference_escapes_or_forks() {
    for extra in [
        test_assign(3, SemanticOperandV1::Copy(test_scalar_place(2))),
        test_reborrow(3, 2),
        test_assign_to(
            test_dereference_place(2, 0),
            SemanticOperandV1::Copy(test_place(1, None)),
        ),
    ] {
        let function = consumer(vec![test_borrow(2, 1), extra], vec![reference(2)]);
        let callables = [test_operation_callable(
            function.abi().clone(),
            derive(0),
            150,
        )];
        assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
        assert!(!source_is_promotable(&function, &callables));
    }
}

#[test]
fn execution_borrow_does_not_authorize_a_defined_helper_without_its_body() {
    let function = consumer(vec![test_borrow(2, 1)], vec![reference(2)]);
    let callables = [SemanticCallableDeclV1::defined(
        SemanticFunctionIdV1::from_index(0),
    )];
    assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
    assert!(!source_is_promotable(&function, &callables));
}

#[test]
fn execution_tile_load_borrow_keeps_exact_source_and_reference_events() {
    for moved in [false, true] {
        let argument = if moved {
            reference(2)
        } else {
            SemanticOperandV1::Copy(test_scalar_place(2))
        };
        let function = consumer(
            vec![shared_borrow(2, test_typed_place(1, 0))],
            vec![argument],
        );
        let callables = [test_operation_callable(
            function.abi().clone(),
            tile_load(0),
            150,
        )];
        let sites = transparent_borrow_sites_v1(&function, &callables);
        let (input, implicit, _) =
            semantic_function_ssa_input_v1(&function, None, &callables, &sites);
        assert_eq!(sites.len(), 1);
        assert!(input.promotable().iter().all(|promotable| *promotable));
        assert!(implicit.is_empty());
        let mut expected = vec![
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
            SsaEventV1::Define(SsaVariableIdV1::new(2)),
            SsaEventV1::Use(SsaVariableIdV1::new(2)),
        ];
        if moved {
            expected.push(SsaEventV1::Kill(SsaVariableIdV1::new(2)));
        }
        assert_eq!(input.blocks()[0].events(), expected);
        let plan = plan_semantic_function_ssa_with_callables_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            &callables,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        assert!(plan.implicit_entry_variables().is_empty());
    }
}

#[test]
fn execution_tile_load_reborrow_keeps_each_source_use_and_definition() {
    let function = consumer(
        vec![
            shared_borrow(2, test_typed_place(1, 0)),
            shared_borrow(3, test_dereference_place(2, 0)),
        ],
        vec![reference(3)],
    );
    let callables = [test_operation_callable(
        function.abi().clone(),
        tile_load(0),
        150,
    )];
    let sites = transparent_borrow_sites_v1(&function, &callables);
    let (input, implicit, _) = semantic_function_ssa_input_v1(&function, None, &callables, &sites);
    assert_eq!(sites.len(), 2);
    assert!(input.promotable().iter().all(|promotable| *promotable));
    assert!(implicit.is_empty());
    assert_eq!(
        input.blocks()[0].events(),
        [
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
            SsaEventV1::Define(SsaVariableIdV1::new(2)),
            SsaEventV1::Use(SsaVariableIdV1::new(2)),
            SsaEventV1::Define(SsaVariableIdV1::new(3)),
            SsaEventV1::Use(SsaVariableIdV1::new(3)),
            SsaEventV1::Kill(SsaVariableIdV1::new(3)),
        ],
    );
}

#[test]
fn execution_tile_load_requires_exact_workgroup_and_argument() {
    for (operation, arguments) in [
        (tile_load(1), vec![reference(2)]),
        (tile_load(0), vec![reference(3), reference(2)]),
        (tile_load(0), vec![reference(3), reference(3), reference(2)]),
        (tile_load(0), vec![reference(2), reference(2)]),
    ] {
        let function = consumer(vec![shared_borrow(2, test_typed_place(1, 0))], arguments);
        let callables = [test_operation_callable(
            function.abi().clone(),
            operation,
            150,
        )];
        assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
        assert!(!source_is_promotable(&function, &callables));
    }
}

#[test]
fn execution_tile_load_does_not_hide_reference_escapes_or_forks() {
    for extra in [
        test_assign(3, SemanticOperandV1::Copy(test_scalar_place(2))),
        shared_borrow(3, test_dereference_place(2, 0)),
        test_assign_to(
            test_dereference_place(2, 0),
            SemanticOperandV1::Copy(test_place(1, None)),
        ),
    ] {
        let function = consumer(
            vec![shared_borrow(2, test_typed_place(1, 0)), extra],
            vec![reference(2)],
        );
        let callables = [test_operation_callable(
            function.abi().clone(),
            tile_load(0),
            150,
        )];
        assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
        assert!(!source_is_promotable(&function, &callables));
    }
}
