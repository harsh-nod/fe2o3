// Included after the old tuple-result Store tests. The old expression route
// remains covered there and continues to reject unsupported tuple operands.

fn canonical_private_constant_store_source_v1() -> ProductionPreRankedKirOwnerV1 {
    // The established sparse private-array fixture, through the same admitted
    // source/materializer constructor, without any synthetic completed input.
    assertion_materialized(assertion_root_with_access(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_ARRAY, SemanticLocalRoleV1::Temporary),
            (A_U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![],
        vec![
            block(
                201,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
            ),
            block(202, vec![], SemanticTerminatorKindV1::Return),
            block(
                203,
                assertion_ranked_write_statements(1, 2),
                SemanticTerminatorKindV1::Return,
            ),
        ],
        false,
    ))
}

fn canonical_same_typed_tuple_store_source_v1(
    field: u32,
    body_shape: BodyShape,
) -> ProductionPreRankedKirOwnerV1 {
    canonical_same_typed_tuple_store_source_with_extra_index_v1(field, body_shape, false)
}

fn canonical_same_typed_tuple_store_source_with_extra_index_v1(
    field: u32,
    body_shape: BodyShape,
    extra_index: bool,
) -> ProductionPreRankedKirOwnerV1 {
    assert!(field < 2);
    let seed = source_fixture(ResultShape::Tuple, body_shape);
    let semantic = seed.source_semantic();
    let mut types = semantic.types().to_vec();
    let SemanticBackendReprV1::Scalar(component) =
        types[A_U32.index() as usize].layout().backend_repr()
    else {
        panic!("fixture scalar must have its admitted ABI representation");
    };
    types[RESULT.index() as usize] = SemanticTypeDeclV1::new(
        types[RESULT.index() as usize].identity(),
        types[RESULT.index() as usize].layout_identity(),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![4, 0], vec![1, 0]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::scalar_pair(*component, *component),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![4, 0], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![A_U32, A_U32]).unwrap()),
    );
    let mut functions = semantic.functions().to_vec();
    for function in &mut functions {
        let blocks = function
            .blocks()
            .iter()
            .map(|block| {
                let statements = block
                    .statements()
                    .iter()
                    .map(|statement| {
                        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                            return statement.clone();
                        };
                        if assignment.destination().ty() != RESULT {
                            return statement.clone();
                        }
                        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
                        else {
                            return statement.clone();
                        };
                        assert_eq!(aggregate.operands().len(), 2);
                        SemanticStatementV1::new(
                            statement.source(),
                            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                                assignment.destination().clone(),
                                SemanticRvalueV1::new(
                                    RESULT,
                                    SemanticRvalueKindV1::Aggregate(
                                        SemanticAggregateRvalueV1::new(
                                            SemanticAggregateKindV1::Tuple,
                                            vec![
                                                aggregate.operands()[0].clone(),
                                                typed_constant(A_U32, 29, 4),
                                            ],
                                        )
                                        .unwrap(),
                                    ),
                                ),
                            )),
                        )
                    })
                    .collect();
                SemanticBasicBlockV1::new(
                    block.identity(),
                    block.source(),
                    statements,
                    block.terminator().clone(),
                )
                .unwrap()
            })
            .collect();
        *function = ordinary_rebuild_v1(
            function,
            function.abi().clone(),
            function.locals().to_vec(),
            blocks,
        );
    }
    let root = &functions[0];
    let result_local = root.locals().len() as u32 - 1;
    assert_eq!(root.locals()[result_local as usize].ty(), RESULT);
    let value = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(result_local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), A_U32).unwrap()],
        A_U32,
    )
    .unwrap();
    let mut blocks = root.blocks().to_vec();
    let old = &blocks[1];
    let [statement] = old.statements() else {
        panic!("fixture must retain the single guarded ordinary Global Store");
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        panic!("fixture Store must be an assignment");
    };
    blocks[1] = SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        vec![SemanticStatementV1::new(
            statement.source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                assignment.destination().clone(),
                SemanticRvalueV1::new(
                    A_U32,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(value)),
                ),
            )),
        )],
        old.terminator().clone(),
    )
    .unwrap();
    // A fresh admitted fourth source argument supplies the index. The old
    // literal-index fixture is left unchanged in the legacy tests above.
    // In particular, IndexUnknown is not treated as a literal/formal without
    // the completing control consumer's independent source anchor check.
    let mut removed = 0;
    for block in &mut blocks {
        let statements = block
            .statements()
            .iter()
            .filter(|statement| {
                if matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if assignment.destination().local().index() == 8
                    && assignment.destination().projections().is_empty())
                {
                    removed += 1;
                    false
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        *block = SemanticBasicBlockV1::new(
            block.identity(),
            block.source(),
            statements,
            block.terminator().clone(),
        )
        .unwrap();
    }
    assert_eq!(removed, 1);
    let mut locals = root.locals().to_vec();
    assert_eq!(locals[8].ty(), A_U64);
    locals[8] = SemanticLocalDeclV1::new(
        locals[8].identity(),
        A_U64,
        SemanticLocalRoleV1::Argument(3),
        locals[8].source(),
    );
    let old_abi = root.abi();
    assert_eq!(old_abi.fixed_count(), 3);
    let mut arguments = old_abi.arguments().to_vec();
    arguments.push(SemanticAbiArgumentV1::source(
        neutral_plain_direct_abi_value_v1(A_U64),
    ));
    let mut ownership = old_abi.source_argument_ownership().to_vec();
    ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
    if extra_index {
        locals.push(local(250, A_U64, SemanticLocalRoleV1::Argument(4)));
        arguments.push(SemanticAbiArgumentV1::source(
            neutral_plain_direct_abi_value_v1(A_U64),
        ));
        ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
    }
    let abi = SemanticFunctionAbiV1::from_rustc(
        old_abi.identity(),
        old_abi.layout_identity(),
        old_abi.canon_abi(),
        old_abi.extern_abi(),
        old_abi.can_unwind(),
        old_abi.c_variadic(),
        4 + u32::from(extra_index),
        arguments,
        old_abi.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap();
    functions[0] = ordinary_rebuild_v1(root, abi, locals, blocks);
    materialize_ranked_fixture_v1(
        assertion_ssa_functions(types, functions),
        &[ranked_root_input_1d(A_NAME, 247, 1)],
    )
    .unwrap()
}

// Real policy3 execution/capture and source-qualified endpoint. No policy cast,
// constructed output module or imported alternate work ledger is used.
fn with_actual_policy3_canonical_view_v1(
    source: &ProductionPreRankedKirOwnerV1,
    profile: Profile,
    body: impl FnOnce(
        &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
        &ProductionSourceOutputOccurrencesV1<'_, '_>,
        &mut Budget<'_>,
    ),
) {
    let bound =
        dialect_amdgcn::bind_production_target_v1(source.executable().module(), profile).unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(PREFIX).unwrap();
    let source_bytes = source.executable_storage().retained_storage()
        + source.assert_origin_storage().payload_storage();
    budget.reserve_storage(source_bytes).unwrap();
    let (input, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let observed =
        fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(&input, &mut budget).unwrap();
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
    let output_bytes = checked.storage().retained_storage();
    budget.reserve_storage(output_bytes).unwrap();
    let (coordinates, coordinate_storage) =
        dialect_amdgcn::check_production_target_coordinate_preservation_v1(
            source.executable(),
            &input,
            profile,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(coordinate_storage.retained_storage())
        .unwrap();
    let (view, view_storage) = fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
        source,
        &coordinates,
        &checked,
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(view_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    body(&checked, &view, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(view);
    budget
        .release_storage(view_storage.retained_storage())
        .unwrap();
    budget
        .release_storage(coordinate_storage.retained_storage())
        .unwrap();
    drop(checked);
    budget.release_storage(output_bytes).unwrap();
    drop(input);
    budget.release_storage(receipt.retained_storage()).unwrap();
    budget.release_storage(source_bytes).unwrap();
    assert_eq!(budget.storage(), PREFIX);
}

// Adversarial-input helper only: the live pipeline is genuine, and only inert
// projection claims are mutable before any scoped check succeeds.
fn with_canonical_control_candidate_test_v1(
    source: &ProductionPreRankedKirOwnerV1,
    profile: Profile,
    mutate: impl FnOnce(&mut fe2o3_lower_mir_kernel::ProductionProjectionControlCandidateV1),
    body: impl FnOnce(
        &fe2o3_lower_mir_kernel::ProductionSourceOutputOccurrencesV1<'_, '_>,
        &[fe2o3_lower_mir_kernel::ProductionCanonicalMemoryAnalysisCandidateV1<'_>],
        &mut Budget<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let mut result = None;
    with_actual(source, profile, |bound, checked, budget| {
        let projection_source = RankedProjectionSourceV1::from_legacy(source).unwrap();
        let inputs = [ranked_root_input_1d(A_NAME, 247, 1)];
        let references =
            crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
        result = Some(with_ranked_root_preparation_v1(
            &projection_source,
            &inputs,
            &references,
            |effects, references| {
                with_checked_output_assertions_budget_v1(
                    source,
                    bound,
                    checked,
                    profile,
                    budget,
                    |session| {
                        session.with_canonical_memory_scope_v1(|session| {
                    let [source_root] = projection_source.source_launch().roots() else {
                        panic!("single-root adversarial fixture required");
                    };
                    let semantic = projection_source.semantic_ssa().source_semantic();
                    let selection = semantic.select_kernel_body_for_root_v1(source_root.selected_root()).unwrap();
                    let mut root = {
                        let mut facts = session.for_source(source_root.selected_root(), selection.body());
                        project_canonical_memory_root_v1(
                            projection_source.semantic_ssa(), effects, selection, &inputs[0], *source_root,
                            &references[0], &mut facts,
                        )?
                    };
                    mutate(root.control.candidate_mut());
                    session.with_output_occurrences_v1(|view, budget| {
                        let candidate = fe2o3_lower_mir_kernel::ProductionCanonicalMemoryAnalysisCandidateV1 {
                            selected_root: root.selected_root, selected_function: root.selected_function,
                            lowering: &root.lowering, access_sources: &root.access_sources,
                            executable_effect_sources: &root.executable_effect_sources,
                            control: root.control.candidate(),
                        };
                        body(view, &[candidate], budget).map_err(|error| ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Output(error),
                        ))
                    })
                })
                    },
                )
            },
        ));
    });
    result.expect("real source/output fixture callback must run")
}

// Independent bounded fixture oracle, not a production normalization rule. Both
// leaves have U32 type, so the exact Call result ordinal is the distinguishing
// source fact. The fixture is acyclic and has one ordinary Store.
fn canonical_tuple_slot_oracle_v1(owner: &Owner, slot: u32, budget: &mut Budget<'_>) {
    use fe2o3_kernel_ir::{
        AddressSpace, CanonicalKirDefinitionCoordinateV1 as Definition, OperationKind, ScalarType,
    };
    let floor = budget.storage();
    let (inventory, storage) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(owner, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let function = &inventory.functions()[0];
    let call = inventory.operations()[function.operations.clone()].iter().find(|row| {
        matches!(&row.operation.kind, OperationKind::Call { callee, .. } if *callee == owner.module().functions[1].id)
    }).unwrap();
    let store = inventory.operations()[function.operations.clone()].iter().find(|row| {
        matches!(&row.operation.kind, OperationKind::Store { access, .. } if access.address_space == AddressSpace::Global)
    }).unwrap();
    assert_eq!(call.results.len(), 2);
    assert!(inventory.definitions().len() <= 256);
    let mut pending = [usize::MAX; 128];
    let mut seen = [false; 256];
    pending[0] = inventory.uses()[store.operands.start + 1].definition;
    let mut count = 1;
    let mut leaves = 0;
    while count != 0 {
        budget.charge_work(3).unwrap();
        count -= 1;
        let definition = pending[count];
        if seen[definition] {
            continue;
        }
        seen[definition] = true;
        let row = &inventory.definitions()[definition];
        assert_eq!(row.ty.as_scalar(), Some(ScalarType::U32));
        match row.coordinate {
            Definition::Result { operation, result } => {
                assert_eq!(operation, call.coordinate);
                assert_eq!(result, slot);
                leaves += 1;
            }
            Definition::BlockArgument { .. } => {
                let mut incoming = 0;
                for edge in &inventory.edge_arguments()[function.edge_arguments.clone()] {
                    budget.charge_work(2).unwrap();
                    if edge.target_definition == definition {
                        assert!(count < pending.len());
                        pending[count] = edge.incoming_definition;
                        count += 1;
                        incoming += 1;
                    }
                }
                assert!(incoming > 0);
            }
            Definition::FunctionArgument { .. } => panic!("the Store lost its tuple Call result"),
        }
    }
    assert_eq!(leaves, 1);
    drop(inventory);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn canonical_store_relation_joins_each_same_typed_source_field_to_actual_o_operand() {
    use fe2o3_kernel_ir::{CanonicalKirUseCoordinateV1 as Use, OperationKind, ScalarType};
    for field in 0..2 {
        let source = canonical_same_typed_tuple_store_source_v1(field, BodyShape::Straight);
        let [captured] = source.source_store_value_uses_v1() else {
            panic!("one actual ordinary Global Store capture is required");
        };
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual(&source, profile, |bound, checked, budget| {
                canonical_tuple_slot_oracle_v1(source.executable(), field, budget);
                canonical_tuple_slot_oracle_v1(bound, field, budget);
                canonical_tuple_slot_oracle_v1(checked.owner(), field, budget);
            });
            with_global_view(&source, profile, |view, budget| {
                let relation = view
                    .source_store_value_v1(
                        ROOT,
                        ROOT,
                        SemanticBlockIdV1::from_index(1),
                        0,
                        0,
                        budget,
                    )
                    .unwrap()
                    .unwrap();
                assert!(std::ptr::eq(relation.source(), captured));
                assert_eq!(relation.source().scalar(), ScalarType::U32);
                assert!(relation.executable());
                let actual = relation.output_use().unwrap();
                let Use::OperationOperand {
                    operation,
                    operand: 1,
                } = actual.coordinate
                else {
                    panic!("the checked use must be the real Store operand1");
                };
                let body = view.output().module().functions[operation.block.function.0 as usize]
                    .body
                    .as_ref()
                    .unwrap();
                let OperationKind::Store { value, .. } = &body.blocks
                    [operation.block.block as usize]
                    .operations[operation.operation as usize]
                    .kind
                else {
                    panic!("the output relation must name an actual Store");
                };
                let (inventory, storage) =
                    fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(view.output(), budget)
                        .unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let definition = inventory
                    .definition_for_value(operation.block.function, *value, budget)
                    .unwrap()
                    .unwrap();
                assert_eq!(definition.coordinate, actual.definition);
                assert_eq!(definition.ty.as_scalar(), Some(ScalarType::U32));
                let call = ordinary_call_coordinate_v1(view, 0, 1);
                let binding = view
                    .retained_ordinary_call_bindings_v1(ROOT, ROOT, call, budget)
                    .unwrap()
                    .unwrap();
                assert_eq!(binding.results.len(), 2);
                assert_eq!(binding.results[0].scalar, ScalarType::U32);
                assert_eq!(binding.results[1].scalar, ScalarType::U32);
                assert_ne!(binding.results[0].output, binding.results[1].output);
                drop(inventory);
                budget.release_storage(storage.retained_storage()).unwrap();
            });
        }
    }
}

#[test]
fn canonical_tuple_store_completes_only_the_scoped_graph_memory_consumer() {
    for field in 0..2 {
        for shape in [BodyShape::Straight, BodyShape::Join, BodyShape::Loop] {
            let source = canonical_same_typed_tuple_store_source_v1(field, shape);
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_actual(&source, profile, |bound, checked, budget| {
                    let floor = budget.storage();
                    let mut called = false;
                    with_projected_canonical_memory_analysis_v1(
                    &source,
                    bound,
                    checked,
                    profile,
                    &[ranked_root_input_1d(A_NAME, 247, 1)],
                    &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                    budget,
                    |roots, budget| {
                        called = true;
                        let [root] = roots else {
                            panic!("complete source root roster required");
                        };
                        assert_eq!(root.selected_root(), ROOT);
                        assert_eq!(root.selected_function(), ROOT);
                        assert!(std::ptr::eq(root.output(), checked.owner()));
                        assert!(std::ptr::eq(
                            root.conditional_control().output(),
                            checked.owner()
                        ));
                        assert_eq!(root.access_count(), 1);
                        let access = root.access(0, budget)?.unwrap();
                        let relation = access.store_value().unwrap();
                        assert!(std::ptr::eq(
                            relation.source(),
                            &source.source_store_value_uses_v1()[0]
                        ));
                        assert_eq!(
                            relation.output_use().unwrap().coordinate,
                            fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                                operation: access.operation(),
                                operand: 1,
                            }
                        );
                        assert_eq!(access.source().semantic_block(), 1);
                        assert_eq!(access.source().semantic_statement(), Some(0));
                        assert!(root.access(1, budget)?.is_none());
                        // This API exposes neither a legacy root/roster nor an
                        // external-reference or formal-memory proof conversion.
                        Ok(())
                    },
                )
                .unwrap();
                    assert!(called);
                    assert_eq!(budget.storage(), floor);
                });
            }
        }
    }
}

#[test]
fn canonical_private_constant_store_consumes_the_existing_allocation_index_rule() {
    let source = canonical_private_constant_store_source_v1();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            let floor = budget.storage();
            let mut completed = false;
            with_projected_canonical_memory_analysis_v1(
                &source,
                bound,
                checked,
                profile,
                &[ranked_root_input_1d(A_NAME, 247, 64)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |roots, budget| {
                    let [root] = roots else { panic!("one private-array source root"); };
                    let access = root.access(0, budget)?.unwrap();
                    assert_eq!(root.access_count(), 1);
                    assert_eq!(access.source().semantic_block(), 2);
                    assert_eq!(access.source().semantic_statement(), Some(1));
                    let relation = access.store_value().unwrap();
                    assert_eq!(relation.source().source_statement(), (SemanticBlockIdV1::from_index(2), 1));
                    let coordinate = access.operation();
                    let operation = &root.output().module().functions[coordinate.block.function.0 as usize]
                        .body.as_ref().unwrap().blocks[coordinate.block.block as usize]
                        .operations[coordinate.operation as usize];
                    assert!(matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Store { access, .. }
                        if access.address_space == fe2o3_kernel_ir::AddressSpace::Private));
                    completed = true;
                    Ok(())
                },
            ).unwrap();
            assert!(completed);
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn canonical_tuple_store_callback_error_and_unwind_drop_all_new_scope_storage() {
    let source = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            for panic in [false, true] {
                let floor = budget.storage();
                let work = budget.work();
                let mut called = false;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_projected_canonical_memory_analysis_v1(
                        &source, bound, checked, profile,
                        &[ranked_root_input_1d(A_NAME, 247, 1)],
                        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                        budget, |roots, budget| -> Result<(), fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1> {
                            called = true;
                            assert!(roots[0].access(0, budget)?.is_some());
                            if panic { panic!("deliberate completing callback unwind"); }
                            Err(fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::Invalid("deliberate completing callback refusal"))
                        },
                    )
                }));
                assert!(called);
                if panic {
                    assert!(result.is_err());
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > work);
            }
        });
    }
}

#[test]
fn canonical_store_completed_access_preserves_ledger_custody_and_live_partition() {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    use fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1 as Error;
    // Access inspection only: source/view floor4 + partition floor1 +
    // exact control identity14 + ordinal1 + source Store row2 = 22.
    const ACCESS: usize = 22;
    const IDENTITY_PREFIX: usize = 19;
    const HISTORY: usize = 7;
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            let outer_floor = budget.storage();
            with_projected_canonical_memory_analysis_v1(
                &source,
                bound,
                checked,
                profile,
                &[ranked_root_input_1d(A_NAME, 247, 1)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |roots, budget| {
                    let root = &roots[0];
                    let floor = budget.storage();
                    for _ in 0..2 {
                        let before = budget.work();
                        assert!(root.access(0, budget)?.is_some());
                        assert_eq!(budget.work(), before + ACCESS);
                        assert_eq!(budget.storage(), floor);
                    }
                    let before = budget.work();
                    assert!(root.access(1, budget)?.is_none());
                    assert_eq!(budget.work(), before + ACCESS - 2);

                    // A released byte is still owned by this live completed
                    // partition. Restore the hostile test mutation immediately.
                    budget.release_storage(1).unwrap();
                    let before = budget.work();
                    let result = root.access(0, budget);
                    budget.reserve_storage(1).unwrap();
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(budget.work(), before + 5);

                    // A separately prepaid meter is not this scope's meter.
                    // The declared exact/under limits cover that refusal prefix,
                    // not construction of the source/view/control/Store scope.
                    for exact in [true, false] {
                        let total = HISTORY + IDENTITY_PREFIX;
                        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
                            total - usize::from(!exact),
                        );
                        let mut other = Budget::new(&mut work, floor);
                        other.reserve_storage(floor).unwrap();
                        other.charge_work(HISTORY).unwrap();
                        let result = root.access(0, &mut other);
                        if exact {
                            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                        } else {
                            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                        }
                        assert_eq!(other.storage(), floor);
                        assert_eq!(other.peak_storage(), floor);
                        other.release_storage(floor).unwrap();
                        if exact {
                            assert_eq!(work.work(), total);
                            assert_eq!(work.failed_work(), None);
                        } else {
                            assert_eq!(work.work(), HISTORY + 5);
                            assert_eq!(work.failed_work(), Some(total));
                        }
                    }
                    assert!(root.access(0, budget)?.is_some());
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(budget.storage(), outer_floor);
        });
    }
}

#[test]
fn canonical_policy3_completing_callbacks_keep_actual_o_and_cleanup_on_every_exit() {
    for private in [false, true] {
        let source = if private {
            canonical_private_constant_store_source_v1()
        } else {
            canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight)
        };
        let inputs = [ranked_root_input_1d(
            A_NAME,
            247,
            if private { 64 } else { 1 },
        )];
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual_policy3_canonical_view_v1(&source, profile, |checked, view, budget| {
                assert!(std::ptr::eq(view.output(), checked.owner()));
                assert!(std::ptr::eq(
                    view.policy3_execution_v1(budget).unwrap().unwrap(),
                    checked.execution()
                ));
                assert_eq!(checked.execution().policy_version(), 3);
                let source = RankedProjectionSourceV1::from_legacy(&source).unwrap();
                let references =
                    crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
                for outcome in 0..3 {
                    let floor = budget.storage();
                    let before = budget.work();
                    let mut completed = false;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        with_ranked_root_preparation_v1(
                            &source,
                            &inputs,
                            &references,
                            |effects, references| {
                                checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                                view, budget, |session| {
                                    with_prepared_canonical_memory_session_v1(
                                        &source, &inputs, effects, references, session,
                                        |roots, budget| {
                                            completed = true;
                                            assert_eq!(roots.len(), 1);
                                            assert!(std::ptr::eq(roots[0].output(), checked.owner()));
                                            assert!(roots[0].access(0, budget)?.is_some());
                                            if outcome == 2 { panic!("actual policy3 completing callback unwind"); }
                                            if outcome == 1 {
                                                return Err(ProductionSourceOutputErrorV1::Invalid(
                                                    "actual policy3 completing callback refusal",
                                                ));
                                            }
                                            Ok(())
                                        },
                                    )
                                },
                            )
                            },
                        )
                    }));
                    assert!(completed);
                    match outcome {
                        0 => result.unwrap().unwrap(),
                        1 => assert!(result.unwrap().is_err()),
                        _ => assert!(result.is_err()),
                    }
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > before);
                }
            });
        }
    }
}

#[test]
fn canonical_store_relation_source_queries_reject_unrelated_coordinates() {
    let source = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_global_view(&source, profile, |view, budget| {
            let other = SemanticFunctionIdV1::from_index(1);
            for (owner, function, block, statement, component) in [
                (other, ROOT, 1, 0, 0),
                (ROOT, other, 1, 0, 0),
                (ROOT, ROOT, 0, 0, 0),
                (ROOT, ROOT, 1, 1, 0),
                (ROOT, ROOT, 1, 0, 1),
            ] {
                assert!(
                    view.source_store_value_v1(
                        owner,
                        function,
                        SemanticBlockIdV1::from_index(block),
                        statement,
                        component,
                        budget,
                    )
                    .unwrap()
                    .is_none()
                );
            }
        });
    }
}

#[test]
fn canonical_store_relation_query_exact_and_under_share_the_declared_ledger() {
    // One retained row: floor4 + key1 + binary iteration(1+5) + borrow2 = 13.
    // This is a query component boundary, excluding materialization/admission.
    const QUERY: usize = 13;
    const HISTORY: usize = 7;
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_global_view(&source, profile, |view, inherited| {
            let floor = inherited.storage();
            for repeats in [1, 2] {
                for exact in [true, false] {
                    let total = HISTORY + repeats * QUERY;
                    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
                        total - usize::from(!exact),
                    );
                    let mut budget = Budget::new(&mut work, floor);
                    budget.reserve_storage(floor).unwrap();
                    budget.charge_work(HISTORY).unwrap();
                    for ordinal in 0..repeats {
                        let result = view.source_store_value_v1(
                            ROOT,
                            ROOT,
                            SemanticBlockIdV1::from_index(1),
                            0,
                            0,
                            &mut budget,
                        );
                        if exact || ordinal + 1 < repeats {
                            assert!(result.unwrap().is_some());
                        } else {
                            assert!(matches!(result, Err(fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::Resource(
                                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(_)
                            ))));
                        }
                        assert_eq!(budget.storage(), floor);
                        assert_eq!(budget.peak_storage(), floor);
                    }
                    budget.release_storage(floor).unwrap();
                    if exact {
                        assert_eq!(work.work(), total);
                        assert_eq!(work.failed_work(), None);
                    } else {
                        assert_eq!(work.work(), total - 2);
                        assert_eq!(work.failed_work(), Some(total));
                    }
                }
            }
        });
    }
}

#[test]
fn canonical_store_consumer_rejects_missing_duplicate_and_substituted_source_claims() {
    use fe2o3_lower_mir_kernel::{
        ProductionCanonicalMemoryAnalysisCandidateV1 as Candidate,
        ProductionRankedAccessSourceV1 as Access,
    };
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in 0..8 {
            with_canonical_control_candidate_test_v1(
                &source,
                profile,
                |_| {},
                |view, roots, budget| {
                    let root = &roots[0];
                    let mut sources = root.access_sources.to_vec();
                    assert_eq!(sources.len(), 1);
                    let old = sources[0];
                    match mutation {
                        0 => sources.clear(),
                        1 => sources.push(old),
                        2 => {
                            sources[0] = Access::new(
                                old.semantic_block(),
                                Some(99),
                                old.semantic_access_ordinal(),
                                old.ranked_block(),
                                old.ranked_operation(),
                            )
                        }
                        3 => {
                            sources[0] = Access::new(
                                0,
                                old.semantic_statement(),
                                old.semantic_access_ordinal(),
                                old.ranked_block(),
                                old.ranked_operation(),
                            )
                        }
                        4 => {
                            sources[0] = Access::new(
                                old.semantic_block(),
                                old.semantic_statement(),
                                1,
                                old.ranked_block(),
                                old.ranked_operation(),
                            )
                        }
                        5 => {
                            sources[0] = Access::new(
                                old.semantic_block(),
                                old.semantic_statement(),
                                old.semantic_access_ordinal(),
                                old.ranked_block(),
                                u32::MAX,
                            )
                        }
                        _ => {}
                    }
                    let bad = [Candidate {
                        selected_root: if mutation == 6 {
                            SemanticFunctionIdV1::from_index(1)
                        } else {
                            root.selected_root
                        },
                        selected_function: if mutation == 7 {
                            SemanticFunctionIdV1::from_index(1)
                        } else {
                            root.selected_function
                        },
                        lowering: root.lowering,
                        access_sources: &sources,
                        executable_effect_sources: root.executable_effect_sources,
                        control: root.control,
                    }];
                    let floor = budget.storage();
                    let mut completed = false;
                    let result = view.with_conditional_memory_control_coverage_v1(
                        &bad,
                        budget,
                        |control, budget| {
                            view.with_canonical_store_analysis_v1(&bad, control, budget, |_, _| {
                                completed = true;
                                Ok(())
                            })
                        },
                    );
                    assert!(result.is_err(), "mutation {mutation}");
                    assert!(!completed);
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            )
            .unwrap();
        }
    }
}

#[test]
fn canonical_store_same_typed_call_result_swap_is_refused_before_output_view_authority() {
    // Mutation of actual O, not an independently invented executable fixture.
    // The established checked-transition gate must reject before any Store
    // query can bind this changed use to the original source operand.
    let source = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            let floor = budget.storage();
            let mut changed = checked.owner().module().clone();
            let callee = changed.functions[1].id.clone();
            let call = changed.functions[0].body.as_mut().unwrap().blocks.iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Call { callee: id, .. } if *id == callee))
                .unwrap();
            assert_eq!(call.results.len(), 2);
            assert_eq!(call.results[0].ty, call.results[1].ty);
            call.results.swap(0, 1);
            let (changed, changed_storage) =
                Owner::from_module_ref_with_verification_budget_v12(&changed, budget).unwrap();
            budget
                .reserve_storage(changed_storage.retained_storage())
                .unwrap();
            let (input, input_storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(bound, budget).unwrap();
            budget
                .reserve_storage(input_storage.retained_storage())
                .unwrap();
            let (output, output_storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&changed, budget).unwrap();
            budget
                .reserve_storage(output_storage.retained_storage())
                .unwrap();
            let check_floor = budget.storage();
            assert!(
                fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
                    &input,
                    &output,
                    checked.occurrences().candidate(),
                    budget,
                )
                .is_err()
            );
            assert_eq!(budget.storage(), check_floor);
            drop(output);
            budget
                .release_storage(output_storage.retained_storage())
                .unwrap();
            drop(input);
            budget
                .release_storage(input_storage.retained_storage())
                .unwrap();
            drop(changed);
            budget
                .release_storage(changed_storage.retained_storage())
                .unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

include!("canonical_memory_control_v1_tests.rs");
include!("source_output_formal_complete_v1_tests.rs");
