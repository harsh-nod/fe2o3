fn indexed_owner(blocks: Vec<SemanticBasicBlockV1>) -> fe2o3_pliron::ProductionSemanticSsaOwnerV1 {
    indexed_owner_with_parameter(blocks, false)
}

fn indexed_owner_with_parameter(
    blocks: Vec<SemanticBasicBlockV1>,
    parameter: bool,
) -> fe2o3_pliron::ProductionSemanticSsaOwnerV1 {
    let seed = super::super::resource_tests::noop_semantic_owner(&["definition_index_fixture"]);
    let semantic = seed.semantic();
    let mut function =
        body(blocks).with_kernel_entry(semantic.functions()[0].kernel_entry().unwrap().clone());
    let mut types = semantic.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([211; 32]),
        SemanticLayoutIdentityV1::from_sha256([212; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                if parameter {
                    SemanticBackendPrimitiveV1::float(32, 4)
                } else {
                    SemanticBackendPrimitiveV1::integer(false, 32, 4)
                },
                SemanticScalarValidityRangeV1::new(0, u32::MAX as u128),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(if parameter {
            SemanticScalarTypeV1::Float { bits: 32 }
        } else {
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }
        }),
    ));
    let root = SemanticFunctionIdV1::from_index(0);
    let mut callables = vec![SemanticCallableDeclV1::defined(root)];
    if parameter {
        let direct = || {
            SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(1),
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
        };
        let abi = |canon, result| {
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([213; 32]),
                SemanticLayoutIdentityV1::from_sha256([214; 32]),
                canon,
                false,
                false,
                vec![direct()],
                result,
            )
            .unwrap()
        };
        let mut locals = function.locals().to_vec();
        locals[1] = SemanticLocalDeclV1::new(
            locals[1].identity(),
            locals[1].ty(),
            SemanticLocalRoleV1::Argument(0),
            locals[1].source(),
        );
        function = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            abi(
                SemanticCanonAbiV1::GpuKernel,
                function.abi().return_value().clone(),
            ),
            locals,
            function.entry(),
            function.blocks().to_vec(),
        )
        .unwrap()
        .with_kernel_entry(function.kernel_entry().unwrap().clone());
        callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([215; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([216; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([217; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([218; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([219; 32]),
                SemanticSourceProvenanceV1::unavailable(),
                abi(SemanticCanonAbiV1::Rust, direct()),
            ),
            operation: SemanticCompilerIntrinsicOperationV1::FabsF32,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([220; 32]),
        });
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target().clone(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![root],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
        fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn indexed_call(block: u8, input: u32, output: u32) -> SemanticBasicBlockV1 {
    indexed_call_block(
        block,
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![SemanticOperandV1::Copy(place(input))],
                Some(SemanticCallDestinationV1::new(
                    place(output),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(u32::from(block) + 1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    )
}

fn indexed_call_block(block: u8, terminator: SemanticTerminatorKindV1) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([60 + block; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

#[test]
fn indexed_normal_call_edges_match_scan_and_entry_is_not_definition_authority() {
    let owner = indexed_owner_with_parameter(
        vec![
            indexed_call(0, 1, 2),
            indexed_call(1, 2, 3),
            indexed_call(2, 3, 2),
            block(
                3,
                vec![assign(3, Some(SemanticOperandV1::Copy(place(2))))],
                None,
            ),
        ],
        true,
    );
    owner.verify_replay().unwrap();
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let source = owner.source_query_for_root(root, view.body()).unwrap();
    let plan = source.plan().plan();
    let mut graph = CapabilitySsaGraphV1::new(view.body(), plan, 100_000)
        .unwrap()
        .with_definition_source(&source)
        .unwrap();
    let entry = plan.entry_definitions()[0].value();
    assert!(graph.definition(entry).is_err());
    reuse_assert_same(graph.definition(entry), graph.definition_scan(entry));
    for block in 0..3 {
        let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(block), 0);
        let row = plan.edge_definitions(edge).unwrap()[0];
        let site = graph.definition(row.value()).unwrap();
        assert_eq!(
            site,
            CapabilityDefinitionSiteV1 {
                block,
                statement: None,
                local: row.variable().get()
            }
        );
        reuse_assert_same(Ok(site), graph.definition_scan(row.value()));
        assert!(matches!(
            graph.definition_call_source(SsaEdgeIdV1::new(edge.source(), 1), row.variable().get()),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert!(graph.definition_call_source(edge, 0).is_err());
    }
}

#[test]
fn indexed_call_source_still_rejects_non_call_projected_and_substituted_destinations() {
    let owner = indexed_owner_with_parameter(
        vec![
            indexed_call(0, 1, 2),
            block(
                1,
                vec![assign(3, Some(SemanticOperandV1::Copy(place(2))))],
                None,
            ),
        ],
        true,
    );
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let source = owner.source_query_for_root(root, view.body()).unwrap();
    let value = source
        .plan()
        .plan()
        .edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0))
        .unwrap()[0]
        .value();
    let SemanticTerminatorKindV1::Call(call) = view.body().blocks()[0].terminator().kind() else {
        panic!("call");
    };
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), place(2).ty()).unwrap()],
        place(2).ty(),
    )
    .unwrap();
    let changed_call = |destination| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                call.arguments().to_vec(),
                Some(SemanticCallDestinationV1::new(
                    destination,
                    call.destination().unwrap().edge(),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    for terminator in [
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(1),
        )),
        changed_call(place(3)),
        changed_call(projected),
    ] {
        let mutated = body(vec![
            indexed_call_block(0, terminator),
            view.body().blocks()[1].clone(),
        ]);
        assert!(
            CapabilitySsaGraphV1::new(&mutated, source.plan().plan(), 100_000)
                .unwrap()
                .with_definition_source(&source)
                .is_err()
        );
        let mut graph = CapabilitySsaGraphV1::new(&mutated, source.plan().plan(), 100_000).unwrap();
        let actual = graph.definition_from_index(&source, value);
        assert!(actual.is_err());
        reuse_assert_same(actual, graph.definition_scan(value));
    }
}

fn indexed_blocks(count: u8) -> Vec<SemanticBasicBlockV1> {
    (0..count)
        .map(|n| {
            block(
                n,
                vec![
                    assign(1, None),
                    assign(2, Some(SemanticOperandV1::Copy(place(1)))),
                ],
                (n + 1 < count).then_some(u32::from(n) + 1),
            )
        })
        .collect()
}

#[test]
fn retained_definition_index_matches_all_unique_values_and_reduces_full_scans() {
    let owner = indexed_owner(indexed_blocks(64));
    owner.verify_replay().unwrap();
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let source = owner.source_query_for_root(root, view.body()).unwrap();
    let plan = source.plan().plan();
    let mut indexed = CapabilitySsaGraphV1::new(view.body(), plan, 1_048_576)
        .unwrap()
        .with_definition_source(&source)
        .unwrap();
    let mut reference = CapabilitySsaGraphV1::new(view.body(), plan, 1_048_576).unwrap();
    let before = (indexed.remaining, reference.remaining);
    let mut count = 0;
    for block in plan.reverse_postorder() {
        for (_, event) in plan.resolved_events(*block).unwrap() {
            if let SsaResolvedEventV1::Define { value, .. } = event {
                reuse_assert_same(indexed.definition(*value), reference.definition(*value));
                count += 1;
            }
        }
    }
    assert!(count >= 64);
    let spent = (before.0 - indexed.remaining, before.1 - reference.remaining);
    assert!(spent.0 < spent.1, "indexed={spent:?}");
    assert_eq!(
        indexed.reuse.definitions.rows,
        reference.reuse.definitions.rows
    );
    assert_eq!(indexed.reuse.definitions.len(), count);
    assert_eq!(reference.reuse.definitions.len(), count);
    for memo in [&indexed.reuse.definitions, &reference.reuse.definitions] {
        assert_eq!(memo.rows.len(), plan.definition_count());
        let (bound_body, bound_ssa) = memo.owner.unwrap();
        assert!(std::ptr::eq(bound_body, view.body()));
        assert!(std::ptr::eq(bound_ssa, plan));
    }
    assert!(std::ptr::eq(
        indexed.reuse.definition_source.unwrap().plan(),
        source.plan()
    ));
    println!(
        "synthetic retained owner: {count} distinct definitions, indexed={} scan={}",
        spent.0, spent.1
    );
}

#[test]
fn definition_source_attachment_rejects_cloned_body_plan_foreign_owner_and_reattach() {
    let owner = indexed_owner(indexed_blocks(1));
    let other = indexed_owner(indexed_blocks(1));
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let source = owner.source_query_for_root(root, view.body()).unwrap();
    let foreign = other
        .source_query_for_root(root, other.execution_view_for_root(root).unwrap().body())
        .unwrap();
    let cloned_body = view.body().clone();
    let cloned_plan = source.plan().plan().clone();
    for (body, plan, query) in [
        (&cloned_body, source.plan().plan(), &source),
        (view.body(), &cloned_plan, &source),
        (view.body(), source.plan().plan(), &foreign),
    ] {
        assert!(matches!(
            CapabilitySsaGraphV1::new(body, plan, 100_000)
                .unwrap()
                .with_definition_source(query),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }
    let graph = CapabilitySsaGraphV1::new(view.body(), source.plan().plan(), 100_000)
        .unwrap()
        .with_definition_source(&source)
        .unwrap();
    assert!(matches!(
        graph.with_definition_source(&source),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    let multi = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
        super::super::resource_tests::noop_semantic_owner(&["lookup_root_a", "lookup_root_b"]),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    multi.verify_replay().unwrap();
    let first = multi.execution_view_for_root(root).unwrap();
    let second_root = SemanticFunctionIdV1::from_index(1);
    let second = multi.execution_view_for_root(second_root).unwrap();
    let other_root = multi
        .source_query_for_root(second_root, second.body())
        .unwrap();
    assert!(matches!(
        CapabilitySsaGraphV1::new(
            first.body(),
            multi.execution_plan_for_root(root).unwrap().plan(),
            100_000
        )
        .unwrap()
        .with_definition_source(&other_root),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[test]
fn indexed_definition_budget_keeps_spent_prefix_and_final_publication() {
    let owner = indexed_owner(indexed_blocks(2));
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let source = owner.source_query_for_root(root, view.body()).unwrap();
    let mut complete = CapabilitySsaGraphV1::new(view.body(), source.plan().plan(), 100_000)
        .unwrap()
        .with_definition_source(&source)
        .unwrap();
    let value = complete.use_value(0, 1).unwrap();
    let SsaValueV1::Definition(id) = value else {
        panic!("statement definition")
    };
    let expected = CapabilityDefinitionSiteV1 {
        block: 0,
        statement: Some(0),
        local: 1,
    };
    let mut charges = vec![1, 1, 3, reuse_definition_slot_words()];
    source
        .definition_origin(id, &mut || {
            charges.push(1);
            true
        })
        .unwrap();
    charges.push(
        source
            .plan()
            .plan()
            .resolved_events(SsaBlockIdV1::new(0))
            .unwrap()
            .len(),
    );
    charges.push(view.body().blocks()[0].statements().len());
    charges.extend(reuse_definition_first_publication_charges(
        source.plan().plan().definition_count(),
    ));
    let needed: usize = charges.iter().sum();
    let before = complete.remaining;
    assert_eq!(complete.definition(value).unwrap(), expected);
    assert_eq!(before - complete.remaining, needed);
    for allowance in 0..=needed {
        let mut graph = CapabilitySsaGraphV1::new(view.body(), source.plan().plan(), 100_000)
            .unwrap()
            .with_definition_source(&source)
            .unwrap();
        graph.charge(graph.remaining - allowance).unwrap();
        let result = graph.definition(value);
        let mut remaining = allowance;
        for &charge in &charges {
            if charge > remaining {
                break;
            }
            remaining -= charge;
        }
        assert_eq!(graph.remaining, remaining, "allowance={allowance}");
        if allowance < needed {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: 100_001,
                    limit: 100_000,
                })
            ));
            assert!(graph.reuse.definitions.is_empty());
            assert!(graph.reuse.definitions.owner.is_none());
            assert!(graph.reuse.definitions.rows.is_empty());
            assert_eq!(graph.reuse.definitions.rows.capacity(), 0);
        } else {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(graph.remaining, 0);
            assert_eq!(graph.reuse.definitions.len(), 1);
            assert_eq!(
                graph.reuse.definitions.rows.len(),
                source.plan().plan().definition_count()
            );
            assert_eq!(
                graph.reuse.definitions.rows[id.get() as usize],
                Some(expected)
            );
            let (bound_body, bound_ssa) = graph.reuse.definitions.owner.unwrap();
            assert!(std::ptr::eq(bound_body, view.body()));
            assert!(std::ptr::eq(bound_ssa, source.plan().plan()));
        }
        let retained_source = graph.reuse.definition_source.unwrap();
        assert!(std::ptr::eq(retained_source.function(), view.body()));
        assert!(std::ptr::eq(retained_source.plan(), source.plan()));
    }
}

#[test]
fn indexed_and_scan_reject_missing_phi_and_repeated_source_definitions() {
    let owner = indexed_owner(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            assign(1, None),
            assign(3, Some(SemanticOperandV1::Copy(place(1)))),
        ],
        None,
    )]);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let source = owner.source_query_for_root(root, view.body()).unwrap();
    let mut values = vec![
        SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(u32::MAX)),
        SsaValueV1::BlockArgument {
            block: SsaBlockIdV1::new(0),
            variable: fe2o3_mir_model::SsaVariableIdV1::new(1),
        },
    ];
    for (_, event) in source
        .plan()
        .plan()
        .resolved_events(SsaBlockIdV1::new(0))
        .unwrap()
    {
        if let SsaResolvedEventV1::Define { variable, value } = event
            && variable.get() == 1
        {
            values.push(*value);
        }
    }
    assert!(values.len() >= 4);
    for value in values {
        let mut graph = CapabilitySsaGraphV1::new(view.body(), source.plan().plan(), 100_000)
            .unwrap()
            .with_definition_source(&source)
            .unwrap();
        let actual = graph.definition(value);
        assert!(actual.is_err());
        reuse_assert_same(actual, graph.definition_scan(value));
        assert!(graph.reuse.definitions.is_empty());
        assert!(graph.reuse.definitions.owner.is_none());
        assert!(graph.reuse.definitions.rows.is_empty());
        assert_eq!(graph.reuse.definitions.rows.capacity(), 0);
    }
}

#[test]
fn indexed_selected_assignment_checks_match_scan_without_shape_substitution() {
    let owner = indexed_owner(indexed_blocks(1));
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let source = owner.source_query_for_root(root, view.body()).unwrap();
    let mut original =
        CapabilitySsaGraphV1::new(view.body(), source.plan().plan(), 100_000).unwrap();
    let value = original.use_value(0, 1).unwrap();
    let statements = view.body().blocks()[0].statements();
    let SemanticStatementKindV1::Assign(first) = statements[0].kind() else {
        panic!("assignment");
    };
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), first.destination().ty())
                .unwrap(),
        ],
        first.destination().ty(),
    )
    .unwrap();
    for changed in [
        vec![
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                projected,
                first.value().clone(),
            ))),
            statements[1].clone(),
        ],
        vec![
            statement(SemanticStatementKindV1::Nop),
            statements[1].clone(),
        ],
        vec![
            statements[0].clone(),
            statements[0].clone(),
            statements[1].clone(),
        ],
    ] {
        let mutated = body(vec![block(0, changed, None)]);
        assert!(
            CapabilitySsaGraphV1::new(&mutated, source.plan().plan(), 100_000)
                .unwrap()
                .with_definition_source(&source)
                .is_err()
        );
        // Adversarial private-helper test, not a production attachment. Both
        // algorithms must retain their selected-source validation below it.
        let mut graph = CapabilitySsaGraphV1::new(&mutated, source.plan().plan(), 100_000).unwrap();
        let actual = graph.definition_from_index(&source, value);
        assert!(actual.is_err());
        reuse_assert_same(actual, graph.definition_scan(value));
    }
}

#[test]
fn unused_malformed_assignments_and_unreachable_rows_do_not_change_selected_definition() {
    let owner = indexed_owner(vec![
        block(
            0,
            vec![
                assign(1, None),
                assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            ],
            None,
        ),
        block(1, vec![assign(3, None), assign(3, None)], None),
    ]);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let source = owner.source_query_for_root(root, view.body()).unwrap();
    let mut graph = CapabilitySsaGraphV1::new(view.body(), source.plan().plan(), 100_000)
        .unwrap()
        .with_definition_source(&source)
        .unwrap();
    let value = graph.use_value(0, 1).unwrap();
    reuse_assert_same(graph.definition(value), graph.definition_scan(value));
    assert_eq!(source.plan().plan().reverse_postorder().len(), 1);
    // An unrelated reachable local's repeated assignments are not a selected
    // definition error. Never globally reject source statements while merely
    // indexing the planner's already-checked definition identities.
    let mut blocks = view.body().blocks().to_vec();
    blocks[0] = block(
        0,
        vec![
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            assign(3, None),
            assign(3, None),
        ],
        None,
    );
    let unrelated = body(blocks);
    let mut raw = CapabilitySsaGraphV1::new(&unrelated, source.plan().plan(), 100_000).unwrap();
    assert!(raw.reuse.definition_source.is_none());
    reuse_assert_same(raw.definition(value), raw.definition_scan(value));
}
