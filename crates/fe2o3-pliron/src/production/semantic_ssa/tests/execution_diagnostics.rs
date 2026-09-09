use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1, SemanticAggregateLayoutV1, SemanticAggregateRvalueV1,
    SemanticAggregateTypeV1, SemanticAssignmentV1,
};

fn ignored_aggregate_return(explicit: bool) -> AdmittedInertSemanticMirV1 {
    let source = admitted_helper_semantic();
    let aggregate = SemanticTypeIdV1::from_index(1);
    let mut types = source.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(180)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(181)),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    let original = &source.functions()[0];
    let mut locals = original.locals().to_vec();
    locals.push(test_local(182, 1, SemanticLocalRoleV1::Temporary));
    let mut blocks = Vec::new();
    for index in 0..2 {
        blocks.push(test_block(
            183 + index as u8,
            vec![],
            test_call(
                1,
                vec![],
                Some(SemanticCallDestinationV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], aggregate)
                        .unwrap(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(index + 1),
                    ),
                )),
            ),
        ));
    }
    blocks.push(test_block(185, vec![], SemanticTerminatorKindV1::Return));
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let original = &source.functions()[1];
    let statements = if explicit {
        vec![SemanticStatementV1::new(
            original.source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], aggregate).unwrap(),
                SemanticRvalueV1::new(
                    aggregate,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, vec![])
                            .unwrap(),
                    ),
                ),
            )),
        )]
    } else {
        vec![]
    };
    let helper = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        SemanticFunctionAbiV1::new(
            original.abi().identity(),
            original.abi().layout_identity(),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![],
            SemanticAbiValueV1::new(aggregate, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap(),
        vec![test_local(186, 1, SemanticLocalRoleV1::Return)],
        original.entry(),
        vec![test_block(
            187,
            statements,
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        source.target().clone(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn elided_ignored_nonunit_return_reports_exact_synthetic_transfer() {
    let semantic = ignored_aggregate_return(false);
    // The source ABI ignores the unwritten return, but execution must not fabricate it.
    construct_semantic_ssa_plans_v1(&semantic, ProductionSemanticSsaLimitsV1::default()).unwrap();
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(SemanticFunctionIdV1::from_index(0)).unwrap();
    let error = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap_err();
    let rendered = error.to_string();
    assert!(rendered.contains("ReturnTransfer"));
    assert!(rendered.contains("execution-coordinate cause"));
    assert!(std::error::Error::source(&error).is_some());
    let ProductionSemanticSsaErrorV1::ExpandedExecution {
        root,
        execution_view_identity,
        source_block: Some((instance, function, block)),
        source_statement: Some(SemanticExpandedStatementOriginV1::ReturnTransfer { callee }),
        source_terminator,
        source_local: Some(local),
        error,
        ..
    } = error
    else {
        panic!("missing exact return diagnostic: {rendered}")
    };
    assert_eq!(root, view.root());
    assert_eq!(&execution_view_identity, view.identity());
    assert_eq!(function, SemanticFunctionIdV1::from_index(1));
    assert_eq!(block, SemanticBlockIdV1::from_index(0));
    assert_eq!(instance, callee);
    assert_eq!(local.instance(), instance);
    assert_eq!(local.function(), function);
    assert_eq!(local.local(), SemanticLocalIdV1::from_index(0));
    assert!(source_terminator.is_none());
    let ProductionSemanticSsaErrorV1::Planner {
        error: SsaPlannerErrorV1::UndefinedAtUse {
            block, variable, ..
        },
        ..
    } = *error
    else {
        panic!("unexpected execution failure")
    };
    assert_eq!(
        view.block_origins()[block.get() as usize].instance(),
        instance
    );
    assert_eq!(view.local_origins()[variable.get() as usize], local);
}

#[test]
fn explicit_ignored_nonunit_returns_survive_repeated_call_expansion() {
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            ignored_aggregate_return(true),
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    assert!(!owner.grants_proof_or_artifact_authority());
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    assert_eq!(view.instances().len(), 3);
    let mut instances = Vec::new();
    for frame in view.instances().iter().skip(1) {
        let error = ProductionSemanticSsaErrorV1::PartialMove {
            function: view.source_body(),
            block: frame.block_start(),
            statement: Some(0),
            local: frame.local_start(),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
        };
        let translated = execution::wrap_error(
            view,
            &execution::ExecutionEventOriginsV1::default(),
            error.clone(),
        );
        let ProductionSemanticSsaErrorV1::ExpandedExecution {
            source_block: Some((instance, function, block)),
            source_statement,
            source_local: Some(local),
            error: cause,
            ..
        } = translated
        else {
            panic!("missing source mappings")
        };
        assert_eq!(*cause, error);
        assert_eq!(function, frame.function());
        assert_eq!(block, SemanticBlockIdV1::from_index(0));
        assert_eq!(local.local(), SemanticLocalIdV1::from_index(0));
        assert_eq!(local.instance(), instance);
        assert_eq!(
            source_statement,
            Some(SemanticExpandedStatementOriginV1::Source { statement: 0 })
        );
        instances.push(instance);
    }
    assert_ne!(instances[0], instances[1]);
}

#[test]
fn unknown_execution_coordinates_never_acquire_source_origins() {
    let semantic = admitted_helper_semantic();
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = &expansion.roots()[0];
    let error = ProductionSemanticSsaErrorV1::Planner {
        function: view.source_body(),
        error: SsaPlannerErrorV1::UndefinedAtUse {
            block: SsaBlockIdV1::new(u32::MAX),
            event: u32::MAX,
            variable: SsaVariableIdV1::new(u32::MAX),
        },
    };
    assert!(matches!(
        execution::wrap_error(view, &execution::ExecutionEventOriginsV1::default(), error),
        ProductionSemanticSsaErrorV1::ExpandedExecution {
            source_block: None,
            source_target: None,
            source_local: None,
            source_statement: None,
            source_terminator: None,
            ..
        }
    ));
}

#[test]
fn event_origins_follow_actual_adapter_events_including_elided_borrows() {
    let (types, function, callables) = test_elided_grid_leader_case(0, true, false, false, false);
    let transparent = transparent_borrow_sites_v1(&function, &callables);
    let mut origins = execution::ExecutionEventOriginsV1::default();
    let traced = adapter::semantic_function_ssa_input_with_event_origins_v1(
        &function,
        Some(&types),
        &callables,
        &transparent,
        |block, statement, events| origins.record(block, statement, events),
    );
    assert_eq!(
        traced,
        semantic_function_ssa_input_v1(&function, Some(&types), &callables, &transparent)
    );
    assert!(origins.resources().unwrap().storage_words > 0);
    assert_eq!(origins.statement(2, 0), Some(Some(0)));
    assert_eq!(origins.statement(2, 1), Some(None));
    assert_eq!(origins.statement(2, 2), Some(None));
    assert_eq!(origins.statement(2, 3), None);
    assert_eq!(origins.statement(u32::MAX, 0), None);
}

#[test]
fn edge_diagnostics_keep_caller_and_callee_coordinate_spaces_distinct() {
    let semantic = admitted_helper_semantic();
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = &expansion.roots()[0];
    let child = &view.instances()[1];
    let error = execution::wrap_error(
        view,
        &execution::ExecutionEventOriginsV1::default(),
        ProductionSemanticSsaErrorV1::Planner {
            function: view.source_body(),
            error: SsaPlannerErrorV1::UndefinedAtEdge {
                edge: fe2o3_mir_model::SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0),
                target: SsaBlockIdV1::new(child.block_start()),
                variable: SsaVariableIdV1::new(child.local_start()),
            },
        },
    );
    let ProductionSemanticSsaErrorV1::ExpandedExecution {
        source_block: Some((caller, function, _)),
        source_target: Some((callee, target_function, target_block)),
        source_terminator:
            Some(SemanticExpandedTerminatorOriginV1::CallEntry {
                callee: entry_callee,
            }),
        source_local: Some(local),
        ..
    } = error
    else {
        panic!("missing edge source/target origins")
    };
    assert_eq!(function, view.source_body());
    assert_eq!(target_function, child.function());
    assert_eq!(target_block, SemanticBlockIdV1::from_index(0));
    assert_ne!(caller, callee);
    assert_eq!(callee, entry_callee);
    assert_eq!(local.instance(), callee);
}

#[test]
fn event_origins_preserve_move_event_ranges_and_empty_statements() {
    let function = test_function(vec![test_block(
        0,
        vec![
            SemanticStatementV1::new(
                fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Nop,
            ),
            test_assign(2, SemanticOperandV1::Move(test_place(1, Some(0)))),
            test_assign(3, SemanticOperandV1::Move(test_typed_place(2, 1))),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    let mut origins = execution::ExecutionEventOriginsV1::default();
    let (input, _, _) = adapter::semantic_function_ssa_input_with_event_origins_v1(
        &function,
        None,
        &[],
        &BTreeSet::new(),
        |block, statement, events| origins.record(block, statement, events),
    );
    assert_eq!(input.blocks()[0].events().len(), 5);
    assert_eq!(origins.statement(0, 0), Some(Some(1)));
    assert_eq!(origins.statement(0, 1), Some(Some(1)));
    for event in 2..5 {
        assert_eq!(origins.statement(0, event), Some(Some(2)));
    }
    assert_eq!(origins.statement(0, 5), None);
}
