mod input_return_tests {
    include!("production_execution_identity_return_v1_tests.rs");
}

mod retained_entry_tests {
    include!("production_execution_identity_retained_v1_tests.rs");
}

#[derive(Clone, Copy, Debug)]
enum NominalLoopCase {
    Entry,
    Header,
    Borrowed,
    ConflictingBorrow,
    Mixed,
    Nested,
    Repeated,
}

fn nominal_loop_owner(case: NominalLoopCase) -> ProductionSemanticSsaOwnerV1 {
    let original = if matches!(case, NominalLoopCase::Repeated) {
        scoped_root_tests::fixtures::repeated_owner()
    } else {
        lifecycle_owner(false)
    };
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let callback = &functions[CALLBACK.index() as usize];
    let workgroup = callback.locals()[1].ty();
    let decrement = || {
        assign(
            place(2, U32),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Subtract,
                left: SemanticOperandV1::Copy(place(2, U32)),
                right: literal(1),
            },
        )
    };
    let goto = |target| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(target),
        ))
    };
    let finish = || {
        block(
            118,
            vec![assign(
                place(0, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, U32))),
            )],
            SemanticTerminatorKindV1::Return,
        )
    };
    let move_self = |local, ty| {
        assign(
            place(local, ty),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(local, ty))),
        )
    };
    let mut locals = callback.locals()[..3].to_vec();
    let blocks = match case {
        NominalLoopCase::Entry => vec![
            block(
                115,
                vec![move_self(1, workgroup), decrement()],
                switch(SemanticOperandV1::Copy(place(2, U32)), 1, 0),
            ),
            finish(),
        ],
        NominalLoopCase::Header => vec![
            block(
                115,
                vec![],
                switch(SemanticOperandV1::Copy(place(2, U32)), 3, 1),
            ),
            block(116, vec![move_self(1, workgroup), decrement()], goto(2)),
            block(
                117,
                vec![],
                switch(SemanticOperandV1::Copy(place(2, U32)), 3, 1),
            ),
            finish(),
        ],
        NominalLoopCase::Borrowed | NominalLoopCase::ConflictingBorrow => {
            let reference_type = reference(
                &mut types,
                workgroup,
                SemanticMutabilityV1::Immutable,
                false,
            );
            let borrow = || {
                assign(
                    place(3, reference_type),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(1, workgroup),
                    },
                )
            };
            locals.push(local(160, reference_type, SemanticLocalRoleV1::Temporary));
            let mut body = vec![assign(
                place(3, reference_type),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, reference_type))),
            )];
            if matches!(case, NominalLoopCase::ConflictingBorrow) {
                locals.push(local(161, reference_type, SemanticLocalRoleV1::Temporary));
                body[0] = assign(
                    place(4, reference_type),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, reference_type))),
                );
                body.push(borrow());
            }
            body.push(decrement());
            vec![
                block(
                    115,
                    vec![borrow()],
                    switch(SemanticOperandV1::Copy(place(2, U32)), 2, 1),
                ),
                block(
                    116,
                    body,
                    switch(SemanticOperandV1::Copy(place(2, U32)), 2, 1),
                ),
                finish(),
            ]
        }
        NominalLoopCase::Mixed | NominalLoopCase::Nested => {
            assert_eq!(
                types[workgroup.index() as usize].layout().size_bytes(),
                Some(16)
            );
            assert_eq!(
                types[workgroup.index() as usize].layout().alignment_bytes(),
                8
            );
            let pair = declaration(
                &mut types,
                SemanticTypeLayoutV1::aggregate(
                    Some(24),
                    8,
                    SemanticAggregateLayoutV1::new(vec![0, 16], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![workgroup, U32]).unwrap(),
                ),
                None,
            );
            let mut initialize = vec![assign(
                place(3, pair),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![SemanticOperandV1::Move(place(1, workgroup)), literal(41)],
                    )
                    .unwrap(),
                ),
            )];
            locals.push(local(160, pair, SemanticLocalRoleV1::Temporary));
            let (carried, ty, projection) = if matches!(case, NominalLoopCase::Nested) {
                let nested = declaration(
                    &mut types,
                    SemanticTypeLayoutV1::aggregate(
                        Some(32),
                        8,
                        SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
                    )
                    .unwrap(),
                    SemanticTypeShapeV1::Tuple(
                        SemanticAggregateTypeV1::new(vec![U32, pair]).unwrap(),
                    ),
                    None,
                );
                locals.push(local(161, nested, SemanticLocalRoleV1::Temporary));
                initialize.push(assign(
                    place(4, nested),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Tuple,
                            vec![literal(11), SemanticOperandV1::Move(place(3, pair))],
                        )
                        .unwrap(),
                    ),
                ));
                (
                    4,
                    nested,
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), pair)
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), U32).unwrap(),
                    ],
                )
            } else {
                (
                    3,
                    pair,
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), U32).unwrap(),
                    ],
                )
            };
            let scalar =
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(carried), projection, U32)
                    .unwrap();
            vec![
                block(
                    115,
                    initialize,
                    switch(SemanticOperandV1::Copy(place(2, U32)), 2, 1),
                ),
                block(
                    116,
                    vec![move_self(carried, ty), decrement()],
                    switch(SemanticOperandV1::Copy(place(2, U32)), 2, 1),
                ),
                block(
                    118,
                    vec![assign(
                        place(0, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(scalar)),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ]
        }
        NominalLoopCase::Repeated => {
            let call = |next| {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(3),
                        vec![
                            SemanticOperandV1::Move(place(1, workgroup)),
                            SemanticOperandV1::Copy(place(2, U32)),
                        ],
                        Some(SemanticCallDestinationV1::new(
                            place(1, workgroup),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(next),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                )
            };
            vec![
                block(115, vec![], call(1)),
                block(117, vec![], call(2)),
                finish(),
            ]
        }
    };
    functions[CALLBACK.index() as usize] = function(
        110,
        SemanticFunctionRoleV1::InternalHelper,
        callback.abi().clone(),
        locals,
        blocks,
    );
    if matches!(case, NominalLoopCase::Repeated) {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([131; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            2,
            vec![
                SemanticAbiArgumentV1::source(value_abi(&types, workgroup)),
                SemanticAbiArgumentV1::source(direct(U32)),
            ],
            value_abi(&types, workgroup),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
        .unwrap();
        functions[3] = function(
            130,
            SemanticFunctionRoleV1::InternalHelper,
            abi,
            vec![
                local(132, workgroup, SemanticLocalRoleV1::Return),
                local(133, workgroup, SemanticLocalRoleV1::Argument(0)),
                local(134, U32, SemanticLocalRoleV1::Argument(1)),
            ],
            vec![
                block(
                    135,
                    vec![move_self(1, workgroup), decrement()],
                    switch(SemanticOperandV1::Copy(place(2, U32)), 1, 0),
                ),
                block(
                    136,
                    vec![assign(
                        place(0, workgroup),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, workgroup))),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn one_nominal_leaf(binding: &SemanticValueBindingV1) -> &SemanticExecutionBindingV29 {
    fn append<'a>(
        binding: &'a SemanticValueBindingV1,
        leaves: &mut Vec<&'a SemanticExecutionBindingV29>,
    ) {
        match binding {
            SemanticValueBindingV1::Execution(value) => leaves.push(value),
            SemanticValueBindingV1::ExecutionBorrow(value) => leaves.push(&value.borrowed),
            SemanticValueBindingV1::Aggregate(fields) => {
                for field in fields {
                    append(field, leaves);
                }
            }
            SemanticValueBindingV1::Value { .. } | SemanticValueBindingV1::Unit => {}
            _ => panic!("unexpected nominal loop representation"),
        }
    }
    let mut leaves = Vec::new();
    append(binding, &mut leaves);
    assert_eq!(leaves.len(), 1);
    leaves[0]
}

fn observe_nominal_loops(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut cyclic = 0;
    let mut original_instances = BTreeSet::new();
    for (ordinal, source) in instances.instances().iter().enumerate() {
        let id = instances.id_at(ordinal).unwrap();
        let lowered = emitted[ordinal].as_ref().unwrap();
        assert_eq!(lowered.source_call_instance, Some(id));
        let observation = lowered.execution_observation.as_ref().unwrap();
        let types = instances.owner().source_semantic().types();
        let mut found = false;
        for block in source.ssa().plan().reverse_postorder() {
            for local in source.ssa().plan().transport_variables(*block).unwrap() {
                let ty = source.declaration().locals()[local.get() as usize].ty();
                if execution_identity_channels_v1(types, ty, budget)? == 0 {
                    continue;
                }
                let phi = SsaValueV1::BlockArgument {
                    block: *block,
                    variable: *local,
                };
                let binding = observation.bindings.get(&phi).unwrap();
                let actual = one_nominal_leaf(binding);
                if let SemanticValueBindingV1::ExecutionBorrow(reference) = binding {
                    assert_eq!(reference.occurrence.instance, id);
                    assert_eq!(reference.occurrence.block.index(), 0);
                    assert_eq!(reference.occurrence.statement, 0);
                    assert_eq!(reference.source_local.index(), 1);
                    assert_eq!(reference.destination_local.index(), 3);
                    assert_eq!(reference.kind, SemanticBorrowKindV1::Shared);
                }
                let occurrences = instances.occurrences(id).unwrap();
                let input = occurrences
                    .entry_definitions()
                    .iter()
                    .find(|entry| entry.variable().get() == 1)
                    .unwrap()
                    .value();
                let seed = if let Some(input) = input {
                    one_nominal_leaf(observation.bindings.get(&input).unwrap())
                } else {
                    assert_eq!(
                        source.declaration().locals()[1].role(),
                        SemanticLocalRoleV1::Argument(0)
                    );
                    observation.retained_seeds[1].as_ref().unwrap()
                };
                assert_eq!(actual, seed);
                assert_eq!(actual.role, SemanticExecutionRoleV29::Workgroup);
                let producer = instances.instance(actual.identity.producer.caller).unwrap();
                assert_eq!(producer.function(), HELPER);
                assert_eq!(actual.identity.producer.block.index(), 0);
                found = true;
            }
        }
        if !found {
            continue;
        }
        cyclic += 1;
        assert!(original_instances.insert(id.index()));
        let body = lowered.function.body.as_ref().unwrap();
        assert!(
            body.blocks
                .iter()
                .flat_map(|block| &block.parameters)
                .all(|parameter| !matches!(parameter.ty, Type::Execution(_)))
        );
        if source.ssa().plan().entry_arguments().is_empty() {
            assert!(lowered.invocation_entry.is_none());
        } else {
            let relation = lowered.invocation_entry.as_ref().unwrap();
            assert_eq!(relation.subject.unwrap().instance, id);
            assert!(relation.subject.unwrap().source == slots.source);
            assert!(
                relation
                    .arguments
                    .iter()
                    .any(|row| row.original.variable().get() == 1 && row.component_count == 0)
            );
            invocation_checked_prefix_v1(source, slots.source.root, lowered, budget)?;
        }
    }
    assert!(cyclic == 1 || cyclic == 2);
    REACHED.set(cyclic);
    Ok(())
}

#[test]
fn genuine_nominal_entry_and_header_loops_preserve_the_emitted_producer_identity() {
    for case in [
        NominalLoopCase::Entry,
        NominalLoopCase::Header,
        NominalLoopCase::Borrowed,
        NominalLoopCase::Repeated,
    ] {
        let result = run_suffix_owner(
            || nominal_loop_owner(case),
            observe_nominal_loops,
            LIMIT,
            LIMIT,
            |output, _, _| {
                assert_eq!(
                    output.pending.sidecars.rows.len(),
                    if matches!(case, NominalLoopCase::Repeated) {
                        5
                    } else {
                        3
                    }
                );
                let entries = output
                    .pending
                    .sidecars
                    .rows
                    .iter()
                    .filter(|row| row.invocation_entry.is_some())
                    .count();
                assert_eq!(
                    entries,
                    match case {
                        NominalLoopCase::Entry => 1,
                        NominalLoopCase::Repeated => 2,
                        _ => 0,
                    }
                );
                assert!(
                    output
                        .pending
                        .function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .flat_map(|block| &block.parameters)
                        .all(|parameter| !matches!(parameter.ty, Type::Execution(_)))
                );
                Ok(())
            },
        )
        .0;
        assert!(result.is_ok(), "{case:?}: {result:?}");
        assert_eq!(
            REACHED.get(),
            if matches!(case, NominalLoopCase::Repeated) {
                2
            } else {
                1
            }
        );
    }
}

#[test]
fn genuine_loop_with_distinct_borrow_producers_is_not_an_identity_fixed_point() {
    let result = run_suffix_owner(
        || nominal_loop_owner(NominalLoopCase::ConflictingBorrow),
        observe_nominal_loops,
        LIMIT,
        LIMIT,
        |_, _, _| panic!("conflicting nominal identity reached output"),
    )
    .0;
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "nominal identity equations differ from their original source",
                ..
            })
        ),
        "{result:?}"
    );
    assert_eq!(REACHED.get(), 0);
}

#[test]
fn genuine_mixed_and_nested_nominal_loops_require_original_logical_initialization() {
    for case in [NominalLoopCase::Mixed, NominalLoopCase::Nested] {
        let result = run_suffix_owner(
            || nominal_loop_owner(case),
            observe_nominal_loops,
            LIMIT,
            LIMIT,
            |_, _, _| Ok(()),
        )
        .0;
        assert!(result.is_ok(), "{case:?}: {result:?}");
        assert_eq!(REACHED.get(), 1);
    }
}

fn observe_checked_nominal_reconstruction(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    observe_nominal_loops(source, instances, emitted, slots, budget)?;
    with_scoped_source_test_layouts_v29(
        source,
        ProductionSemanticKirLimitsV1::default(),
        budget,
        |_, layouts, budget| {
            source_storage_v29::with_source_storage_root_v29(
                layouts,
                instances,
                budget,
                |references, root, budget| {
                    let floor = budget.storage();
                    let mut prior = None;
                    for _ in 0..2 {
                        with_execution_identity_plan_v1(
                            instances,
                            references,
                            &root,
                            budget,
                            |plan, budget| {
                                let plan = plan.unwrap();
                                assert_eq!(plan.targets.len(), 2);
                                let fingerprint = (
                                    plan.source,
                                    plan.targets.clone(),
                                    plan.mapping.clone(),
                                    plan.classes.clone(),
                                );
                                if let Some(prior) = &prior {
                                    assert!(prior == &fingerprint);
                                } else {
                                    prior = Some(fingerprint);
                                }
                                let (ordinal, block) = plan.targets[0];
                                let id = instances.id_at(ordinal).unwrap();
                                let original = instances.instance(id).unwrap();
                                let block = SemanticBlockIdV1::from_index(block);
                                let before = budget.storage();
                                let mut cursor =
                                    ExecutionAvailabilityV29::new(instances, id, budget)?
                                        .with_identity_plan_v1(Some(plan), budget)?;
                                plan.check_cursor(&cursor, budget)?;
                                assert!(
                                    plan.destination(id, &cursor.cfg, block, budget)?.is_some()
                                );

                                let foreign = instances.id_at(plan.targets[1].0).unwrap();
                                assert_eq!(
                                    instances.instance(foreign).unwrap().function(),
                                    original.function()
                                );
                                cursor.instance = foreign;
                                assert!(plan.check_cursor(&cursor, budget).is_err());
                                cursor.instance = id;
                                let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                                let mut foreign_budget = ArgumentBudgetV1::new(&mut work, LIMIT);
                                foreign_budget.reserve_storage(budget.storage())?;
                                assert!(plan.check_cursor(&cursor, &mut foreign_budget).is_err());

                                let range = cursor.cfg.ranges[block.index() as usize].clone();
                                let index = range
                                    .clone()
                                    .find(|index| !cursor.cfg.entries[*index].leaves.is_empty())
                                    .unwrap();
                                let leaves = cursor.cfg.entries[index].leaves.clone();
                                cursor.cfg.entries[index].leaves = leaves.start..leaves.start;
                                assert!(plan.destination(id, &cursor.cfg, block, budget).is_err());
                                cursor.cfg.entries[index].leaves = leaves;
                                let local = cursor.cfg.entries[index].local;
                                cursor.cfg.entries[index].local = u32::MAX;
                                assert!(plan.destination(id, &cursor.cfg, block, budget).is_err());
                                cursor.cfg.entries[index].local = local;

                                let other = ExecutionCfgV29::new(
                                    instances.owner().source_semantic().types(),
                                    original.declaration(),
                                    original.ssa(),
                                    &instances.occurrences(id).unwrap(),
                                    budget,
                                )?;
                                let proof = plan.destination(id, &cursor.cfg, block, budget)?;
                                assert!(
                                    other
                                        .enter_with_identity_v1(
                                            block,
                                            &mut cursor.current,
                                            &cursor.seen,
                                            proof,
                                            budget,
                                        )
                                        .is_err()
                                );
                                drop(other);
                                drop(cursor);
                                budget.release_storage(budget.storage() - before)?;
                                Ok(())
                            },
                        )?;
                        assert_eq!(budget.storage(), floor);
                    }
                    for panic in [false, true] {
                        let prior_failure = budget.failed_storage();
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            with_execution_identity_plan_v1(
                                instances,
                                references,
                                &root,
                                budget,
                                |plan, budget| -> Result<(), ProductionSemanticKirErrorV1> {
                                    assert!(plan.is_some());
                                    budget.reserve_storage(19)?;
                                    if panic {
                                        std::panic::panic_any(271usize);
                                    }
                                    Err(execution_identity_error_v1())
                                },
                            )
                        }));
                        if panic {
                            assert_eq!(*result.unwrap_err().downcast::<usize>().unwrap(), 271);
                        } else {
                            assert!(matches!(
                                result.unwrap(),
                                Err(ProductionSemanticKirErrorV1::Unsupported {
                                    detail: "nominal identity equations differ from their original source",
                                    ..
                                })
                            ));
                        }
                        assert_eq!(budget.storage(), floor);
                        assert_eq!(budget.failed_storage(), prior_failure);
                    }
                    Ok(())
                },
            )
        },
    )
}

#[test]
fn checked_nominal_reconstruction_rejects_foreign_missing_and_replaced_source_cursors() {
    let result = run_suffix_owner(
        || nominal_loop_owner(NominalLoopCase::Repeated),
        observe_checked_nominal_reconstruction,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    )
    .0;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(REACHED.get(), 2);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NominalCleanupFault {
    LostByte,
    ForeignLedger,
    ArenaGrowth,
    DisposableScratch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NominalCleanupExit {
    Success,
    Error,
    Panic,
}

fn nominal_cleanup_selected_error(error: &ProductionSemanticKirErrorV1) {
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "nominal cleanup selected callback failure",
                ..
            }
        ),
        "{error:?}"
    );
}

fn nominal_cleanup_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = nominal_loop_owner(NominalLoopCase::Repeated);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let reference_type = reference(&mut types, U32, SemanticMutabilityV1::Immutable, false);
    let mut functions = semantic.functions().to_vec();
    let root = &functions[ROOT.index() as usize];
    let mut locals = root.locals().to_vec();
    let reference_local = u32::try_from(locals.len()).unwrap();
    locals.push(local(162, reference_type, SemanticLocalRoleV1::Temporary));
    let mut blocks = root.blocks().to_vec();
    let first = &blocks[0];
    let mut statements = first.statements().to_vec();
    // Physical state requires an actual source storage demand, not just a
    // promoted scalar argument or a logical initialization snapshot.
    statements.push(assign(
        place(reference_local, reference_type),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(1, U32),
        },
    ));
    blocks[0] = fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1::new(
        first.identity(),
        first.source(),
        statements,
        first.terminator().clone(),
    )
    .unwrap();
    functions[ROOT.index() as usize] = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        locals,
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn nominal_cleanup_case(fault: NominalCleanupFault, exit: NominalCleanupExit, ignore: bool) {
    const CALLER: usize = 37;
    let mut owner = nominal_cleanup_owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let mut other = ArgumentBudgetV1::new(&mut other_work, LIMIT);
    budget.reserve_storage(CALLER).unwrap();
    let captured = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(captured.retained_storage()).unwrap();
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    assert!(demands.types(&owner, &mut budget).unwrap().contains(&U32));
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget).unwrap(),
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    let (lost, source_failed) = with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let table_floor = budget.storage();
        let mut after_callback = 0;
        let lost = matches!(fault, NominalCleanupFault::LostByte | NominalCleanupFault::ForeignLedger)
            || (fault == NominalCleanupFault::ArenaGrowth && exit != NominalCleanupExit::Success);
        let result = source_storage_v29::with_source_storage_root_v29(&mut layouts, instances, budget,
            |references, root, budget| {
                let source_floor = budget.storage();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_execution_identity_plan_v1(instances, references, &root, budget,
                        |plan, budget| -> Result<(), ProductionSemanticKirErrorV1> {
                            assert!(plan.is_some());
                            let full = budget.storage();
                            assert!(full > source_floor + 1);
                            // Global denial history does not poison this source
                            // owner; it must survive cleanup on either ledger.
                            assert!(budget.reserve_storage(LIMIT).is_err());
                            match fault {
                                NominalCleanupFault::LostByte => {
                                    budget.release_storage(1)?;
                                    assert!(budget.storage() > source_floor);
                                }
                                NominalCleanupFault::ForeignLedger => {
                                    other.reserve_storage(full)?;
                                    assert!(other.work_ledger_identity_v1() != budget.work_ledger_identity_v1());
                                    std::mem::swap(budget, &mut other);
                                    assert!(!root.permits_cleanup_refund(instances, &references.failure, 0, budget));
                                    assert!(!root.permits_cleanup_refund(instances, &references.failure, 0, &other));
                                    assert!(references.failure.first_error().is_none(), "cleanup observations do not select a new error");
                                }
                                NominalCleanupFault::ArenaGrowth => {
                                    let state = root.new_state(instances.root(), SemanticLocalIdV1::from_index(1), budget).unwrap();
                                    let path = root.root_path(U32, budget).unwrap();
                                    root.mutate(state, path, source_storage_v29::SourceStorageRootMutationV29::Initialize, budget).unwrap();
                                    let copy = root.copy_state(state, budget).unwrap();
                                    assert!(root.is_initialized(copy, path, budget).unwrap());
                                    assert!(budget.storage() > full);
                                }
                                NominalCleanupFault::DisposableScratch => budget.reserve_storage(19)?,
                            }
                            after_callback = budget.storage();
                            match exit {
                                NominalCleanupExit::Success => Ok(()),
                                NominalCleanupExit::Error => Err(source_reference_error_v29("nominal cleanup selected callback failure")),
                                NominalCleanupExit::Panic => std::panic::panic_any(1105usize),
                            }
                        },
                    )
                }));
                if fault == NominalCleanupFault::ForeignLedger {
                    assert_eq!(budget.storage(), after_callback);
                    assert_eq!(budget.work(), 0, "foreign work is never charged");
                    assert_eq!(budget.failed_storage(), None);
                    assert_eq!(other.storage(), after_callback);
                    std::mem::swap(budget, &mut other);
                    // Restoring the original ledger cannot undo an already
                    // denied refund on the concrete original root.
                    other.release_storage(other.storage()).unwrap();
                }
                if lost {
                    assert_eq!(budget.storage(), after_callback);
                    assert!(!root.custody_view().retains_after_refund(0, budget));
                } else if exit != NominalCleanupExit::Success {
                    assert_eq!(budget.storage(), source_floor);
                    assert!(root.custody_view().retains_custody(instances, &references.failure, budget));
                }
                match &outcome {
                    Ok(Err(error)) if exit == NominalCleanupExit::Error => nominal_cleanup_selected_error(error),
                    Ok(Err(error)) => assert!(matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))),
                    Ok(Ok(())) => assert_eq!(exit, NominalCleanupExit::Success),
                    Err(payload) => assert_eq!(payload.downcast_ref::<usize>(), Some(&1105)),
                }
                if ignore { return Ok(()); }
                match outcome {
                    Ok(result) => result.map_err(Into::into),
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            },
        );
        if ignore && lost || exit == NominalCleanupExit::Success && lost {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))), "{fault:?}/{exit:?}: {result:?}");
        } else if !ignore && exit == NominalCleanupExit::Error {
            nominal_cleanup_selected_error(result.as_ref().unwrap_err());
        } else if !ignore && exit == NominalCleanupExit::Panic {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference callback panicked", ..
            })), "{result:?}");
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
        if lost {
            assert_eq!(budget.storage(), after_callback, "outer source owner cannot refund abandoned credits");
            assert!(!layouts.permits_root_emission_refund(instances.owner(), 0, budget));
        } else {
            let kept = usize::from(exit == NominalCleanupExit::Success && fault == NominalCleanupFault::DisposableScratch) * 19;
            assert_eq!(budget.storage(), table_floor + kept);
            budget.release_storage(kept).unwrap();
        }
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>((lost, result.is_err()))
    }).unwrap();
    // The enclosing layout table outlives the call-instance reservation.
    let before_release = budget.storage();
    let first_denial = budget.failed_storage();
    assert!(first_denial.is_some());
    let release = layouts.release(&mut budget);
    if lost {
        assert!(release.is_err());
        assert_eq!(budget.storage(), before_release, "module-table release cannot recover denied root custody");
    } else if source_failed {
        assert!(release.is_err(), "consumed first error stays sticky");
    } else {
        release.unwrap();
    }
    assert_eq!(budget.failed_storage(), first_denial);
    // Only this test owns the abandoned source/module/instance budgets. Drop
    // their backing first; production cleanup above was not permitted to refund.
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(budget.storage() - CALLER).unwrap();
    assert_eq!(budget.storage(), CALLER);
    assert_eq!(other.storage(), 0);
    drop((budget, other));
    assert_eq!(work.failed_work(), None);
    assert_eq!(other_work.failed_work(), None);
}

#[test]
fn nominal_cleanup_retained_floor_loss_denies_both_inner_and_outer_refunds() {
    for exit in [
        NominalCleanupExit::Success,
        NominalCleanupExit::Error,
        NominalCleanupExit::Panic,
    ] {
        nominal_cleanup_case(NominalCleanupFault::LostByte, exit, false);
        nominal_cleanup_case(NominalCleanupFault::LostByte, exit, true);
    }
}

#[test]
fn nominal_cleanup_foreign_ledger_cannot_be_repaired_into_refund_authority() {
    for exit in [
        NominalCleanupExit::Success,
        NominalCleanupExit::Error,
        NominalCleanupExit::Panic,
    ] {
        nominal_cleanup_case(NominalCleanupFault::ForeignLedger, exit, false);
        nominal_cleanup_case(NominalCleanupFault::ForeignLedger, exit, true);
    }
}

#[test]
fn nominal_cleanup_preserves_callback_arena_growth_and_selected_failures() {
    for exit in [
        NominalCleanupExit::Success,
        NominalCleanupExit::Error,
        NominalCleanupExit::Panic,
    ] {
        nominal_cleanup_case(NominalCleanupFault::ArenaGrowth, exit, false);
        nominal_cleanup_case(NominalCleanupFault::ArenaGrowth, exit, true);
    }
}

#[test]
fn nominal_cleanup_disposable_scratch_still_settles_without_losing_denial_history() {
    for exit in [
        NominalCleanupExit::Success,
        NominalCleanupExit::Error,
        NominalCleanupExit::Panic,
    ] {
        nominal_cleanup_case(NominalCleanupFault::DisposableScratch, exit, false);
        nominal_cleanup_case(NominalCleanupFault::DisposableScratch, exit, true);
    }
}

fn nested_nominal_growth<'a, 'root, 'source, 'work>(
    instances: &'a ExecutionInstancesV29<'source>,
    references: &'a SourceReferencePlanV29<'a, 'source>,
    root: &source_storage_v29::SourceStorageRootV29<'a, 'root, 'source>,
    budget: &mut ArgumentBudgetV1<'work>,
    depth: usize,
    exit: NominalCleanupExit,
    growth_bytes: &std::cell::Cell<usize>,
    callback_storage: &std::cell::Cell<usize>,
    reached: &std::cell::Cell<usize>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_execution_identity_plan_v1(instances, references, root, budget, |plan, budget| {
        assert!(plan.is_some());
        reached.set(reached.get() + 1);
        let before = budget.storage();
        let state = root
            .new_state(instances.root(), SemanticLocalIdV1::from_index(1), budget)
            .unwrap();
        let path = root.root_path(U32, budget).unwrap();
        root.mutate(
            state,
            path,
            source_storage_v29::SourceStorageRootMutationV29::Initialize,
            budget,
        )
        .unwrap();
        let copy = root.copy_state(state, budget).unwrap();
        assert!(root.is_initialized(copy, path, budget).unwrap());
        let growth = budget.storage() - before;
        assert!(growth > 0);
        growth_bytes.set(growth_bytes.get() + growth);
        if depth != 0 {
            return nested_nominal_growth(
                instances,
                references,
                root,
                budget,
                depth - 1,
                exit,
                growth_bytes,
                callback_storage,
                reached,
            );
        }
        callback_storage.set(budget.storage());
        match exit {
            NominalCleanupExit::Success => Ok(()),
            NominalCleanupExit::Error => Err(source_reference_error_v29(
                "cushioned nominal selected failure",
            )),
            NominalCleanupExit::Panic => std::panic::panic_any(1156usize),
        }
    })
}

fn cushioned_nominal_growth_case(depth: usize, exit: NominalCleanupExit, ignore: bool) {
    const CALLER: usize = 37;
    const ANCESTOR: usize = 65_537;
    let mut owner = nominal_cleanup_owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(CALLER).unwrap();
    let captured = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(captured.retained_storage()).unwrap();
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget).unwrap(),
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    let reached = std::cell::Cell::new(0);
    let checked = std::cell::Cell::new(false);
    let denied = exit != NominalCleanupExit::Success;
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let table_floor = budget.storage();
        let callback_storage = std::cell::Cell::new(0);
        let result = source_storage_v29::with_source_storage_root_v29(
            &mut layouts,
            instances,
            budget,
            |references, root, budget| {
                let source_floor = budget.storage();
                let mut ancestor = Vec::<u8>::new();
                budget.reserve_storage(std::mem::size_of::<Vec<u8>>() + ANCESTOR)?;
                ancestor.try_reserve_exact(ANCESTOR).unwrap();
                budget.reserve_storage(ancestor.capacity() - ANCESTOR)?;
                ancestor.resize(ANCESTOR, 73);
                let ancestor_bytes = std::mem::size_of::<Vec<u8>>() + ancestor.capacity();
                let entry = budget.storage();
                assert_eq!(entry, source_floor + ancestor_bytes);
                assert!(budget.reserve_storage(LIMIT).is_err());
                let prior_denial = budget.failed_storage();
                let growth_bytes = std::cell::Cell::new(0);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    nested_nominal_growth(
                        instances,
                        references,
                        &root,
                        budget,
                        depth,
                        exit,
                        &growth_bytes,
                        &callback_storage,
                        &reached,
                    )
                }));
                assert_eq!(reached.get(), depth + 1);
                assert!(ancestor.iter().all(|byte| *byte == 73));
                assert!(growth_bytes.get() > 0 && growth_bytes.get() < ancestor_bytes);
                assert_eq!(budget.failed_storage(), prior_denial);
                if denied {
                    assert_eq!(
                        budget.storage(),
                        callback_storage.get(),
                        "neither nested scope may refund live growth"
                    );
                    assert!(!root.permits_cleanup_refund(
                        instances,
                        &references.failure,
                        0,
                        budget
                    ));
                    budget.reserve_storage(13)?;
                    budget.release_storage(13)?;
                    assert!(!root.permits_cleanup_refund(
                        instances,
                        &references.failure,
                        0,
                        budget
                    ));
                    drop(ancestor);
                } else {
                    assert_eq!(budget.storage(), entry + growth_bytes.get());
                    drop(ancestor);
                    assert!(root.permits_cleanup_refund(
                        instances,
                        &references.failure,
                        ancestor_bytes,
                        budget
                    ));
                    budget.release_storage(ancestor_bytes)?;
                    assert_eq!(budget.storage(), source_floor + growth_bytes.get());
                }
                match &outcome {
                    Ok(Ok(())) => assert!(!denied),
                    Ok(Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "cushioned nominal selected failure",
                        ..
                    })) => assert_eq!(exit, NominalCleanupExit::Error),
                    Err(payload) => {
                        assert_eq!(exit, NominalCleanupExit::Panic);
                        assert_eq!(payload.downcast_ref::<usize>(), Some(&1156));
                    }
                    other => panic!("unexpected nominal result: {other:?}"),
                }
                checked.set(true);
                if ignore {
                    return Ok(());
                }
                match outcome {
                    Ok(result) => result.map_err(Into::into),
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            },
        );
        assert!(checked.get(), "setup panic is not cleanup coverage");
        match &result {
            Ok(()) => assert!(!denied),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting,
            )) => assert!(denied && ignore),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "cushioned nominal selected failure",
                ..
            }) => assert!(denied && !ignore && exit == NominalCleanupExit::Error),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference callback panicked",
                ..
            }) => assert!(denied && !ignore && exit == NominalCleanupExit::Panic),
            other => panic!("unexpected source scope result: {other:?}"),
        }
        if denied {
            assert_eq!(budget.storage(), callback_storage.get());
            assert!(!layouts.permits_root_emission_refund(instances.owner(), 0, budget));
        } else {
            assert_eq!(budget.storage(), table_floor);
        }
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
    assert!(checked.get());
    let before = budget.storage();
    let prior_denial = budget.failed_storage();
    assert!(prior_denial.is_some());
    let released = layouts.release(&mut budget);
    assert_eq!(released.is_err(), denied);
    if denied {
        assert_eq!(budget.storage(), before);
    }
    assert_eq!(budget.failed_storage(), prior_denial);
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(budget.storage() - CALLER).unwrap();
    assert_eq!(budget.storage(), CALLER);
}

#[test]
fn original_root_growth_cannot_spend_an_independent_ancestor_or_nested_nominal_header() {
    for depth in [0, 1] {
        for exit in [
            NominalCleanupExit::Success,
            NominalCleanupExit::Error,
            NominalCleanupExit::Panic,
        ] {
            for ignore in [false, true] {
                cushioned_nominal_growth_case(depth, exit, ignore);
            }
        }
    }
}

#[test]
fn retained_growth_headers_and_prepaid_checks_have_independent_field_counts() {
    use std::mem::size_of;
    let view = 2 * size_of::<&usize>() + size_of::<usize>();
    let checkpoint = size_of::<Option<(&usize, usize, usize)>>();
    assert_eq!(
        view,
        size_of::<source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>()
    );
    assert_eq!(
        checkpoint,
        size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
    );
    assert_eq!(
        source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29,
        4 + 4
    );
}

std::thread_local! {
    static IDENTITY_CONSTRUCTION_CASE_V1: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct IdentityConstructionFaultGuard(u8);
impl IdentityConstructionFaultGuard {
    fn install(fault: u8) -> Self {
        Self(IDENTITY_CONSTRUCTION_FAULT_V1.replace(fault))
    }
}
impl Drop for IdentityConstructionFaultGuard {
    fn drop(&mut self) {
        IDENTITY_CONSTRUCTION_FAULT_V1.set(self.0);
    }
}

fn observe_identity_construction_settlement(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    _: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let case = IDENTITY_CONSTRUCTION_CASE_V1.get();
    with_scoped_source_test_layouts_v29(
        source,
        ProductionSemanticKirLimitsV1::default(),
        budget,
        |_, layouts, budget| {
            source_storage_v29::with_source_storage_root_v29(
                layouts,
                instances,
                budget,
                |references, root, budget| {
                    let floor = budget.storage();
                    assert!(floor > 37);
                    assert!(budget.reserve_storage(LIMIT).is_err());
                    assert!(budget.charge_work(LIMIT + 9).is_err());
                    let storage_denial = budget.failed_storage();
                    if case == 0 || case == 3 {
                        // Case0 accepts all eight checkpoint checks, then refuses
                        // the first target scan. Case3 refuses the prepaid batch.
                        let remaining = if case == 0 { 4 + 4 } else { 4 + 4 - 1 };
                        budget.charge_work(LIMIT - remaining - budget.work())?;
                    }
                    let before = budget.work();
                    let mut called = false;
                    let outcome = {
                        let _fault = IdentityConstructionFaultGuard::install(match case {
                            1 => 1,
                            2 => 2,
                            _ => 0,
                        });
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            with_execution_identity_plan_v1(
                                instances,
                                references,
                                &root,
                                budget,
                                |plan, _| {
                                    called = true;
                                    assert_eq!(plan.is_none(), case == 5);
                                    Ok(())
                                },
                            )
                        }))
                    };
                    assert_eq!(IDENTITY_CONSTRUCTION_FAULT_V1.get(), 0);
                    assert_eq!(called, case == 4 || case == 5);
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.failed_storage(), storage_denial);
                    assert!(root.permits_cleanup_refund(instances, &references.failure, 0, budget));
                    match case {
                        0 | 3 => {
                            assert!(matches!(outcome, Ok(Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))))), "{outcome:?}");
                            assert_eq!(budget.work() - before, if case == 0 { 4 + 4 } else { 0 });
                        }
                        1 => {
                            assert!(
                                matches!(
                                    outcome,
                                    Ok(Err(ProductionSemanticKirErrorV1::Unsupported {
                                        detail: "nominal identity equations differ from their original source",
                                        ..
                                    }))
                                ),
                                "{outcome:?}"
                            );
                            assert_eq!(budget.work() - before, 4 + 4);
                        }
                        2 => {
                            assert_eq!(outcome.unwrap_err().downcast_ref::<usize>(), Some(&1158));
                            assert_eq!(budget.work() - before, 4 + 4);
                        }
                        4 | 5 => assert!(outcome.unwrap().is_ok()),
                        _ => unreachable!(),
                    }
                    REACHED.set(300 + case);
                    // The resource cases deliberately exhaust this work ledger.
                    // Preserve this selected result rather than requiring later
                    // emission work from the enclosing actual root route.
                    Err(source_reference_error_v29(
                        "identity construction settlement probe complete",
                    )
                    .into())
                },
            )
        },
    )
}

#[test]
fn nominal_construction_errors_and_panics_drop_their_paid_growth_envelope_before_refund() {
    for case in 0..6 {
        IDENTITY_CONSTRUCTION_CASE_V1.set(case);
        let result = run_suffix_owner(
            || {
                if case == 5 {
                    lifecycle_owner(false)
                } else {
                    nominal_loop_owner(NominalLoopCase::Repeated)
                }
            },
            observe_identity_construction_settlement,
            LIMIT,
            LIMIT,
            |_, _, _| Ok(()),
        )
        .0;
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "identity construction settlement probe complete",
                    ..
                })
            ),
            "case{case}: {result:?}"
        );
        assert_eq!(
            REACHED.get(),
            300 + case,
            "caught setup panic is not construction coverage"
        );
    }
}
