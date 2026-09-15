use super::*;
use fe2o3_mir_model::{SsaEdgeIdV1, SsaEdgeRoleV1};

fn query_use<'a>(
    query: &ProductionSemanticSsaSourceQueryV1<'a>,
    site: Site,
) -> ProductionSemanticSsaValueV1<'a> {
    query
        .operand_use(site, operand(query.function(), site), &mut || true)
        .unwrap()
        .retained_value()
}

fn assignment_site(function: &SemanticFunctionDeclV1, local: u32) -> Site {
    function
        .blocks()
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.statements()
                .iter()
                .enumerate()
                .find_map(|(statement, value)| {
                    matches!(value.kind(), SemanticStatementKindV1::Assign(value)
                if value.destination().local().index() == local)
                    .then_some(Site::new(
                        SemanticBlockIdV1::from_index(block as u32),
                        Some(statement as u32),
                    ))
                })
        })
        .unwrap()
}

fn merge_source(backedge: bool, parallel: bool) -> AdmittedInertSemanticMirV1 {
    with_blocks(if backedge {
        vec![
            cfg_block(220, vec![assign(1, constant(7))], go(1)),
            cfg_block(
                221,
                vec![assign(2, copy(1)), assign(1, constant(9))],
                switch(1, 2),
            ),
            cfg_block(222, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![
            cfg_block(220, vec![], switch(1, 2)),
            cfg_block(
                221,
                vec![assign(1, constant(7))],
                if parallel { switch(3, 3) } else { go(3) },
            ),
            cfg_block(222, vec![assign(1, constant(9))], go(3)),
            cfg_block(223, vec![], go(4)),
            cfg_block(
                224,
                vec![assign(2, copy(1))],
                SemanticTerminatorKindV1::Return,
            ),
            // A declared, unreachable predecessor is not an incoming SSA edge.
            cfg_block(225, vec![assign(1, constant(17))], go(3)),
        ]
    })
}

#[test]
fn retained_value_origins_exact_event_versions_in_source_and_expanded_owners() {
    for expanded in [false, true] {
        let owner = owner(expanded);
        owner.verify_replay().unwrap();
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let mut previous = None;
        for (use_statement, definition) in [(1, 0), (3, 2), (5, 4)] {
            let value = query_use(
                &query,
                source_site(view, u32::from(expanded), use_statement),
            );
            assert!(value.belongs_to(&query));
            let ProductionSemanticSsaValueOriginV1::Event { block, event, site } =
                query.value_origin(&value, &mut || true).unwrap()
            else {
                panic!("event origin")
            };
            assert_eq!(site, source_site(view, u32::from(expanded), definition));
            assert!(matches!(query.plan().plan().resolved_event(block,event),
                Some(SsaResolvedEventV1::Define {variable,value:actual})
                if *variable==value.variable() && *actual==value.value()));
            if let Some(previous) = previous {
                assert_ne!(previous, value.value());
            }
            previous = Some(value.value());
        }
    }
}

#[test]
fn retained_value_origins_compact_coordinates_recheck_the_planner_variable() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = source_site(view, 0, 1);
    let mut value = query
        .operand_use(site, operand(view.body(), site), &mut || true)
        .unwrap();
    value.variable = SsaVariableIdV1::new(u32::MAX);
    assert!(matches!(
        query.value_origin(&value.retained_value(), &mut || true),
        Err(QueryError::MissingDefinition)
    ));
}

#[test]
fn retained_source_use_switch_keeps_exact_operand_site_and_shared_work() {
    let SemanticTerminatorKindV1::SwitchInt { targets, .. } = switch(1, 2) else {
        unreachable!()
    };
    let source = with_blocks(vec![
        cfg_block(
            230,
            vec![assign(1, constant(1))],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: copy(1),
                targets,
            },
        ),
        cfg_block(231, vec![], SemanticTerminatorKindV1::Return),
        cfg_block(232, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let owner = make_owner(source, ProductionSemanticSsaLimitsV1::default()).unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } =
        view.body().blocks()[0].terminator().kind()
    else {
        panic!("switch")
    };
    let site = Site::new(SemanticBlockIdV1::from_index(0), None);
    let value = query.operand_use(site, discriminant, &mut || true).unwrap();
    assert!(
        matches!(query.value_origin(&value.retained_value(),&mut ||true).unwrap(),
        ProductionSemanticSsaValueOriginV1::Event { site,.. } if site.statement()==Some(0))
    );
    assert!(matches!(
        query.operand_use(site, &discriminant.clone(), &mut || true),
        Err(QueryError::OperandOutsideSite)
    ));
    assert!(matches!(
        query.operand_use(site, discriminant, &mut || false),
        Err(QueryError::WorkLimit)
    ));
}

#[test]
fn retained_value_predecessors_keep_parallel_roles_and_existing_budget() {
    let owner = make_owner(
        merge_source(false, true),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let merge = SsaBlockIdV1::new(3);
    assert_eq!(query.predecessor_count(merge, &mut || true).unwrap(), 3);
    let mut edges = Vec::new();
    for index in 0..3 {
        let edge = query
            .predecessor(merge, index, &mut || true)
            .unwrap()
            .unwrap();
        assert_eq!(edge.target(), merge);
        edges.push((edge.id(), edge.role()));
    }
    assert_eq!(edges[0].0.source(), edges[1].0.source());
    assert_ne!(edges[0].0.ordinal(), edges[1].0.ordinal());
    assert_ne!(edges[0].1, edges[1].1);
    assert!(query.predecessor(merge, 3, &mut || true).unwrap().is_none());
    assert_eq!(
        query.predecessor_count(SsaBlockIdV1::new(5), &mut || true),
        Err(QueryError::InvalidSite)
    );
    for limit in 0..2 {
        let mut remaining = limit;
        assert!(matches!(
            query.predecessor(merge, 0, &mut charger(&mut remaining)),
            Err(QueryError::WorkLimit)
        ));
        assert_eq!(remaining, 0);
    }
}

#[test]
fn retained_value_origins_complete_edges_belong_to_actual_merge_not_use_block() {
    for (backedge, parallel) in [(false, false), (false, true), (true, false)] {
        let owner = make_owner(
            merge_source(backedge, parallel),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        owner.verify_replay().unwrap();
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let site = assignment_site(view.body(), 2);
        let value = query_use(&query, site);
        let ProductionSemanticSsaValueOriginV1::BlockArgument(incoming) =
            query.value_origin(&value, &mut || true).unwrap()
        else {
            panic!("complete merge relation")
        };
        assert!(incoming.belongs_to(&query));
        assert!(incoming.external_entry().is_none());
        assert_eq!(incoming.edge_count(), if parallel { 3 } else { 2 });
        if !backedge {
            assert_ne!(incoming.block().get(), site.block().index());
        }
        let mut actual = Vec::new();
        for index in 0..incoming.edge_count() {
            let (edge, argument) = incoming.edge(index, &mut || true).unwrap().unwrap();
            assert_eq!(edge.target(), incoming.block());
            assert!(argument.belongs_to(&query));
            let expected = query
                .plan()
                .plan()
                .edge_arguments(edge.id())
                .unwrap()
                .iter()
                .find(|argument| argument.variable() == value.variable())
                .unwrap();
            assert_eq!(argument.value(), expected.value());
            assert!(matches!(
                query.value_origin(&argument, &mut || true).unwrap(),
                ProductionSemanticSsaValueOriginV1::Event { .. }
            ));
            actual.push((edge.id(), edge.role(), argument.value()));
        }
        if parallel {
            assert_eq!(actual[0].0.source(), actual[1].0.source());
            assert_ne!(actual[0].0.ordinal(), actual[1].0.ordinal());
            assert_ne!(actual[0].1, actual[1].1);
            assert_eq!(actual[0].2, actual[1].2);
        }
        assert!(
            incoming
                .edge(incoming.edge_count(), &mut || true)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn retained_value_origins_owner_binding_and_shared_query_budget_fail_closed() {
    let owner = make_owner(
        merge_source(false, true),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let other = make_owner(
        merge_source(false, true),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let other_query = other
        .source_query_for_root(ROOT, other.execution_view_for_root(ROOT).unwrap().body())
        .unwrap();
    let value = query_use(&query, assignment_site(view.body(), 2));
    assert!(matches!(
        other_query.value_origin(&value, &mut || true),
        Err(QueryError::WrongOwner)
    ));
    let mut remaining = 4096;
    query
        .value_origin(&value, &mut charger(&mut remaining))
        .unwrap();
    let cost = 4096 - remaining;
    assert!(cost >= 4, "all three incoming rows must be charged");
    for budget in 0..cost {
        let mut remaining = budget;
        assert!(matches!(
            query.value_origin(&value, &mut charger(&mut remaining)),
            Err(QueryError::WorkLimit)
        ));
        assert_eq!(remaining, 0);
    }
    let mut remaining = cost;
    let ProductionSemanticSsaValueOriginV1::BlockArgument(incoming) = query
        .value_origin(&value, &mut charger(&mut remaining))
        .unwrap()
    else {
        panic!("merge")
    };
    assert_eq!(remaining, 0);
    assert!(matches!(
        incoming.edge(0, &mut charger(&mut remaining)),
        Err(QueryError::WorkLimit)
    ));
    assert!(matches!(
        query.value_origin(&value, &mut charger(&mut remaining)),
        Err(QueryError::WorkLimit)
    ));
}

#[test]
fn retained_value_origins_source_index_omission_changes_identity_and_replay() {
    let mut owner = owner(false);
    let before = owner.source_identity;
    owner.source_plans[0].value_origins = Default::default();
    let after = derive_semantic_ssa_identity_v1(
        &owner.source_semantic_sha256,
        &owner.source_plans,
        owner.source_summary,
    );
    assert_ne!(before, after);
    assert_eq!(
        owner.verify_replay(),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
}

#[test]
fn retained_source_use_store_exact_value_copy_move_and_cloned_operand() {
    for moved in [false, true] {
        let source = source_with_statements(
            false,
            vec![
                assign(1, constant(7)),
                SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        place(2),
                        if moved {
                            SemanticOperandV1::Move(place(1))
                        } else {
                            copy(1)
                        },
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
            ],
        );
        let owner = make_owner(source, ProductionSemanticSsaLimitsV1::default()).unwrap();
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let site = source_site(view, 0, 1);
        let SemanticStatementKindV1::Store(store) = view.body().blocks()
            [site.block().index() as usize]
            .statements()[site.statement().unwrap() as usize]
            .kind()
        else {
            panic!("retained store")
        };
        let retained = query
            .operand_use(site, store.value(), &mut || true)
            .unwrap();
        assert!(
            matches!(query.value_origin(&retained.retained_value(),&mut || true).unwrap(),
            ProductionSemanticSsaValueOriginV1::Event{site:actual,..} if actual==source_site(view,0,0))
        );
        assert!(matches!(
            query.operand_use(site, &store.value().clone(), &mut || true),
            Err(QueryError::OperandOutsideSite)
        ));
        assert!(matches!(
            query.operand_use(source_site(view, 0, 0), store.value(), &mut || true),
            Err(QueryError::OperandOutsideSite)
        ));
        if moved {
            assert!(query.plan().plan().resolved_events(SsaBlockIdV1::new(site.block().index())).unwrap().iter().any(|(at,event)|
                retained.event_range().contains(&(*at as usize)) && matches!(event,SsaResolvedEventV1::Kill{variable,previous:Some(value)} if *variable==retained.variable() && *value==retained.value())));
        }
    }
}

fn abi(
    canon: SemanticCanonAbiV1,
    unwind: bool,
    arguments: Vec<SemanticAbiValueV1>,
    result: SemanticAbiValueV1,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([241; 32]),
        SemanticLayoutIdentityV1::from_sha256([242; 32]),
        canon,
        unwind,
        false,
        arguments,
        result,
    )
    .unwrap()
}
fn direct() -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        U32,
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

pub(super) fn parameter_source(
    blocks: Vec<SemanticBasicBlockV1>,
    float: bool,
    with_call: bool,
) -> AdmittedInertSemanticMirV1 {
    let seed = source(false);
    let original = &seed.functions()[0];
    let mut types = seed.types().to_vec();
    if float {
        types[1] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([243; 32]),
            SemanticLayoutIdentityV1::from_sha256([244; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::float(32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX as u128),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        );
    }
    let mut locals = original.locals().to_vec();
    locals[1] = SemanticLocalDeclV1::new(
        locals[1].identity(),
        U32,
        SemanticLocalRoleV1::Argument(0),
        locals[1].source(),
    );
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        abi(
            SemanticCanonAbiV1::GpuKernel,
            false,
            vec![direct()],
            original.abi().return_value().clone(),
        ),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let callables = if with_call {
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256([245; 32]),
                    SemanticItemDefinitionIdentityV1::from_sha256([246; 32]),
                    SemanticMonomorphizationIdentityV1::from_sha256([247; 32]),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256([248; 32]),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256([249; 32]),
                    SemanticSourceProvenanceV1::unavailable(),
                    abi(SemanticCanonAbiV1::Rust, false, vec![direct()], direct()),
                ),
                operation: SemanticCompilerIntrinsicOperationV1::FabsF32,
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([250; 32]),
            },
        ]
    } else {
        vec![SemanticCallableDeclV1::defined(ROOT)]
    };
    InertSemanticMirRequestV1::new_with_callables(
        seed.target().clone(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn retained_value_origins_entry_loop_keeps_external_entry_and_backedge_separate() {
    let source = parameter_source(
        vec![
            cfg_block(
                220,
                vec![assign(2, copy(1)), assign(1, constant(9))],
                switch(0, 1),
            ),
            cfg_block(221, vec![], SemanticTerminatorKindV1::Return),
        ],
        false,
        false,
    );
    let owner = make_owner(source, ProductionSemanticSsaLimitsV1::default()).unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let value = query_use(&query, assignment_site(view.body(), 2));
    let ProductionSemanticSsaValueOriginV1::BlockArgument(incoming) =
        query.value_origin(&value, &mut || true).unwrap()
    else {
        panic!("entry merge")
    };
    assert_eq!(incoming.block().get(), view.body().entry().index());
    assert_eq!(incoming.edge_count(), 1);
    let entry = incoming
        .external_entry()
        .expect("external invocation entry");
    assert!(matches!(
        query.value_origin(&entry, &mut || true).unwrap(),
        ProductionSemanticSsaValueOriginV1::Entry { argument: 0 }
    ));
    let (edge, back) = incoming.edge(0, &mut || true).unwrap().unwrap();
    assert_eq!(edge.id().source(), incoming.block());
    assert_ne!(entry.value(), back.value());
    assert!(matches!(
        query.value_origin(&back, &mut || true).unwrap(),
        ProductionSemanticSsaValueOriginV1::Event { .. }
    ));
}

#[test]
fn retained_source_use_direct_call_keeps_operand_site_move_and_return_edge_definition() {
    direct_call_query(false);
    direct_call_query(true);
}

fn direct_call_query(cleanup: bool) {
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![SemanticOperandV1::Move(place(1))],
        Some(SemanticCallDestinationV1::new(
            place(2),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        if cleanup {
            SemanticUnwindActionV1::Cleanup(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallUnwind,
                SemanticBlockIdV1::from_index(2),
            ))
        } else {
            SemanticUnwindActionV1::Unreachable
        },
    )
    .unwrap();
    let source = parameter_source(
        vec![
            cfg_block(220, vec![], SemanticTerminatorKindV1::Call(call)),
            cfg_block(
                221,
                vec![assign(1, copy(2))],
                SemanticTerminatorKindV1::Return,
            ),
            cfg_block(222, vec![], SemanticTerminatorKindV1::Unreachable),
        ],
        true,
        true,
    );
    if cleanup {
        let plan = plan_semantic_function_ssa_with_module_v1(
            ROOT,
            &source.functions()[0],
            source.types(),
            source.callables(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let entry = SsaBlockIdV1::new(source.functions()[0].entry().index());
        assert_eq!(
            plan.plan()
                .edge_definitions(SsaEdgeIdV1::new(entry, 0))
                .unwrap()
                .len(),
            1
        );
        assert!(
            plan.plan()
                .edge_definitions(SsaEdgeIdV1::new(entry, 1))
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            make_owner(source, ProductionSemanticSsaLimitsV1::default()),
            Err(ProductionSemanticSsaErrorV1::CallExpansion(
                SemanticCallExpansionErrorV1::Unsupported {
                    reason: "call unwind is not unreachable",
                    ..
                }
            ))
        ));
        return;
    }
    let owner = make_owner(source, ProductionSemanticSsaLimitsV1::default()).unwrap();
    owner.verify_replay().unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let (block, call) = view
        .body()
        .blocks()
        .iter()
        .enumerate()
        .find_map(|(block, body)| match body.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => Some((block, call)),
            _ => None,
        })
        .unwrap();
    let site = Site::new(SemanticBlockIdV1::from_index(block as u32), None);
    let mut value = query
        .operand_use(site, &call.arguments()[0], &mut || true)
        .unwrap();
    assert!(matches!(
        query
            .value_origin(&value.retained_value(), &mut || true)
            .unwrap(),
        ProductionSemanticSsaValueOriginV1::Entry { argument: 0 }
    ));
    assert!(matches!(
        query.operand_use(site, &call.arguments()[0].clone(), &mut || true),
        Err(QueryError::OperandOutsideSite)
    ));
    assert!(query.plan().plan().resolved_events(SsaBlockIdV1::new(block as u32)).unwrap().iter().any(|(_,event)|
        matches!(event,SsaResolvedEventV1::Kill{variable,previous:Some(previous)} if *variable==value.variable() && *previous==value.value())));
    value.variable = SsaVariableIdV1::new(u32::MAX);
    assert!(matches!(
        query.value_origin(&value.retained_value(), &mut || true),
        Err(QueryError::MissingDefinition)
    ));
    let returned = query_use(&query, assignment_site(view.body(), 1));
    let ProductionSemanticSsaValueOriginV1::Edge { edge, definition } =
        query.value_origin(&returned, &mut || true).unwrap()
    else {
        panic!("edge result, not statement event")
    };
    assert_eq!(
        edge.id(),
        SsaEdgeIdV1::new(SsaBlockIdV1::new(block as u32), 0)
    );
    assert_eq!(definition, 0);
    assert_eq!(
        edge.target().get(),
        call.destination().unwrap().edge().target().index()
    );
    assert_eq!(
        query.plan().plan().edge_definitions(edge.id()).unwrap()[definition].value(),
        returned.value()
    );
    let returned_site = assignment_site(view.body(), 1);
    let mut returned_use = query
        .operand_use(
            returned_site,
            operand(view.body(), returned_site),
            &mut || true,
        )
        .unwrap();
    returned_use.variable = SsaVariableIdV1::new(u32::MAX);
    assert!(matches!(
        query.value_origin(&returned_use.retained_value(), &mut || true),
        Err(QueryError::MissingDefinition)
    ));
    assert!(
        query
            .plan()
            .plan()
            .edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(block as u32), 1))
            .is_none()
    );
}
