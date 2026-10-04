use super::*;

mod slot_query_tests {
    include!("production_execution_identity_retained_slot_v1_tests.rs");
}

mod call_effect_tests {
    include!("production_execution_identity_retained_calls_v1_tests.rs");
}

fn retained_insert_callable(
    functions: &mut [SemanticFunctionDeclV1],
    callables: &mut Vec<SemanticCallableDeclV1>,
) -> SemanticCallableIdV1 {
    let boundary = functions.len() as u32;
    let original = callables.clone();
    callables.insert(
        boundary as usize,
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(boundary)),
    );
    for function in functions {
        let blocks = function
            .blocks()
            .iter()
            .map(|block| {
                let kind = match block.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) if call.callee().index() >= boundary => {
                        assert!(matches!(
                            original[call.callee().index() as usize],
                            SemanticCallableDeclV1::CompilerIntrinsic { .. }
                        ));
                        assert!(call.variadic_argument_abis().is_empty());
                        assert!(call.inline_assembly_source_v30().is_none());
                        assert!(call.ordered_region_source_v31().is_none());
                        assert!(call.ordered_program_source_v32().is_none());
                        let target = SemanticCallableIdV1::from_index(call.callee().index() + 1);
                        assert_eq!(
                            callables[target.index() as usize],
                            original[call.callee().index() as usize]
                        );
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable(
                                target,
                                call.arguments().to_vec(),
                                call.destination().cloned(),
                                call.unwind(),
                            )
                            .unwrap(),
                        )
                    }
                    kind => kind.clone(),
                };
                SemanticBasicBlockV1::new(
                    block.identity(),
                    block.source(),
                    block.statements().to_vec(),
                    SemanticTerminatorV1::new(block.terminator().source(), kind),
                )
                .unwrap()
            })
            .collect();
        let mut replacement = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            function.abi().clone(),
            function.locals().to_vec(),
            function.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = function.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        assert_eq!(replacement.identity(), function.identity());
        assert_eq!(replacement.abi(), function.abi());
        assert_eq!(replacement.locals(), function.locals());
        assert_eq!(replacement.entry(), function.entry());
        assert_eq!(replacement.kernel_entry(), function.kernel_entry());
        for (after, before) in replacement.blocks().iter().zip(function.blocks()) {
            assert_eq!(after.identity(), before.identity());
            assert_eq!(after.source(), before.source());
            assert_eq!(after.statements(), before.statements());
            match (after.terminator().kind(), before.terminator().kind()) {
                (SemanticTerminatorKindV1::Call(after), SemanticTerminatorKindV1::Call(before)) => {
                    assert_eq!(after.arguments(), before.arguments());
                    assert_eq!(after.destination(), before.destination());
                    assert_eq!(after.unwind(), before.unwind());
                    assert_eq!(
                        callables[after.callee().index() as usize],
                        original[before.callee().index() as usize]
                    );
                }
                (after, before) => assert_eq!(after, before),
            }
        }
        *function = replacement;
    }
    SemanticCallableIdV1::from_index(boundary)
}

fn retained_owner(cyclic: bool, repeated: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = nominal_loop_owner(NominalLoopCase::Borrowed);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    if !cyclic {
        let callback = &functions[CALLBACK.index() as usize];
        let mut blocks = callback.blocks().to_vec();
        blocks[1] = block(
            116,
            blocks[1].statements().to_vec(),
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(2),
            )),
        );
        functions[CALLBACK.index() as usize] = function(
            110,
            SemanticFunctionRoleV1::InternalHelper,
            callback.abi().clone(),
            callback.locals().to_vec(),
            blocks,
        );
    }
    if repeated {
        let worker_callable = retained_insert_callable(&mut functions, &mut callables);
        let preserved_root = functions[ROOT.index() as usize].clone();
        let preserved_provider = functions[HELPER.index() as usize].clone();
        let callback = &functions[CALLBACK.index() as usize];
        let worker = function(
            130,
            SemanticFunctionRoleV1::InternalHelper,
            callback.abi().clone(),
            callback.locals().to_vec(),
            callback
                .blocks()
                .iter()
                .enumerate()
                .map(|(index, original)| {
                    block(
                        135 + index as u8,
                        original.statements().to_vec(),
                        original.terminator().kind().clone(),
                    )
                })
                .collect(),
        );
        let workgroup = callback.locals()[1].ty();
        let call = || {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    worker_callable,
                    vec![
                        SemanticOperandV1::Move(place(1, workgroup)),
                        SemanticOperandV1::Copy(place(2, U32)),
                    ],
                    Some(SemanticCallDestinationV1::new(
                        place(0, U32),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(3),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        functions[CALLBACK.index() as usize] = function(
            110,
            SemanticFunctionRoleV1::InternalHelper,
            callback.abi().clone(),
            callback.locals()[..3].to_vec(),
            vec![
                block(
                    115,
                    vec![],
                    switch(SemanticOperandV1::Copy(place(2, U32)), 1, 2),
                ),
                block(116, vec![], call()),
                block(117, vec![], call()),
                block(118, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        functions.push(worker);
        assert_eq!(functions[ROOT.index() as usize], preserved_root);
        assert_eq!(functions[HELPER.index() as usize], preserved_provider);
    }
    retained_rebuild(semantic.types().to_vec(), functions, callables)
}

fn retained_rebuild(
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
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

fn observe_retained_entries(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut inputs = Vec::new();
    let count = instances.owner().source_semantic().functions().len();
    assert!(count == 3 || count == 4);
    let expected_function = SemanticFunctionIdV1::from_index(if count == 4 { 3 } else { 2 });
    for (ordinal, original) in instances.instances().iter().enumerate() {
        if original.function() != expected_function {
            continue;
        }
        let id = instances.id_at(ordinal).unwrap();
        let lowered = emitted[ordinal].as_ref().unwrap();
        assert_eq!(lowered.source_call_instance, Some(id));
        let observed = lowered.execution_observation.as_ref().unwrap();
        let seed = observed.retained_seeds[1].as_ref().unwrap();
        assert_eq!(seed.role, SemanticExecutionRoleV29::Workgroup);
        assert_eq!(
            instances
                .instance(seed.identity.producer.caller)
                .unwrap()
                .function(),
            HELPER
        );
        let occurrences = instances.occurrences(id).unwrap();
        assert!(
            occurrences
                .entry_definitions()
                .iter()
                .find(|entry| entry.variable().get() == 1)
                .unwrap()
                .value()
                .is_none()
        );
        let use_event = occurrences
            .events()
            .iter()
            .find(|event| event.event().variable().get() == 1)
            .unwrap();
        assert!(!use_event.is_promoted());
        assert!(use_event.resolved().is_none());
        assert_eq!(use_event.role(), ExecutionEventV29::BaseUse);
        let borrowed = observed
            .bindings
            .values()
            .find_map(|binding| match binding {
                SemanticValueBindingV1::ExecutionBorrow(binding)
                    if binding.occurrence.instance == id =>
                {
                    Some(binding)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(&borrowed.borrowed, seed);
        assert_eq!(borrowed.source_local.index(), 1);
        assert_eq!(borrowed.destination_local.index(), 3);
        assert_eq!(borrowed.kind, SemanticBorrowKindV1::Shared);
        assert_eq!(borrowed.occurrence.block.index(), 0);
        assert_eq!(borrowed.occurrence.statement, 0);
        inputs.push((id, seed.clone()));
    }
    assert!(inputs.len() == 1 || inputs.len() == 2);
    if inputs.len() == 2 {
        assert_ne!(inputs[0].0, inputs[1].0);
        // The source borrow occurrences differ; their real referent is equal.
        assert_eq!(inputs[0].1, inputs[1].1);
        assert_eq!(inputs[0].1.identity.producer.block.index(), 0);
        assert_eq!(inputs[1].1.identity.producer.block.index(), 0);
    }
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
                    with_execution_identity_plan_v1(
                        instances,
                        references,
                        &root,
                        budget,
                        |plan, budget| {
                            let plan = plan.unwrap();
                            assert_eq!(plan.index.retained.len(), inputs.len());
                            let before = budget.storage();
                            let replayed = derive_scoped_source_slots_with_identities_v1(
                                instances,
                                emitted,
                                ProductionSemanticKirLimitsV1::default().max_operations,
                                Some(references),
                                Some(plan),
                                budget,
                            )?;
                            assert_eq!(replayed.instances, slots.instances);
                            assert_eq!(replayed.slots, slots.slots);
                            let retained = replayed.retained_storage;
                            drop(replayed);
                            budget.release_storage(retained)?;
                            assert_eq!(budget.storage(), before);
                            assert!(matches!(derive_scoped_source_slots_with_references_v29(
                                instances, emitted, ProductionSemanticKirLimitsV1::default().max_operations,
                                Some(references), budget,
                            ), Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                                if detail == "scoped source-slot allocation census is incomplete or mismatched"));
                            assert_eq!(budget.storage(), before);
                            assert!(
                                derive_scoped_source_slots_with_identities_v1(
                                    instances,
                                    emitted,
                                    ProductionSemanticKirLimitsV1::default().max_operations,
                                    None,
                                    Some(plan),
                                    budget,
                                )
                                .is_err()
                            );
                            for (id, seed) in &inputs {
                                let original = instances.instance(*id).unwrap();
                                let local = SemanticLocalIdV1::from_index(1);
                                let row = plan.index.retained_entry(*id, local, budget)?.unwrap();
                                assert!(matches!(
                                    row.kind,
                                    ExecutionIdentitySourceKindV1::RetainedEntry(_)
                                ));
                                assert!(row.value.is_none());
                                assert_eq!(row.selection.count, 1);
                                assert!(plan.retained_slot_omission_v1(
                                    instances, references, *id, 1, budget
                                )?);
                                assert!(!plan.retained_slot_omission_v1(
                                    instances, references, *id, 0, budget
                                )?);
                                assert!(!slots.slots.iter().any(|slot| slot.instance == *id
                                    && slot.legacy_local().unwrap() == 1));
                                production_call_instances_v1::with_production_call_instances_v1(
                                    instances.owner(), ROOT, budget, |foreign, budget| {
                                        assert!(plan.retained_slot_omission_v1(
                                            foreign, references, *id, 1, budget,
                                        ).is_err());
                                        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
                                    },
                                ).map_err(|_| execution_identity_error_v1())?;
                                let site = ExecutionSiteV29::Statement {
                                    block: SsaBlockIdV1::new(0),
                                    statement: 0,
                                };
                                let role = ExecutionOperandV29::RvaluePlace;
                                let place = execution_identity_retained_place_v1(
                                    original.declaration(),
                                    site,
                                    role,
                                    budget,
                                )?
                                .unwrap();
                                // Actual production emission above installed this seed. The
                                // replay below isolates cursor ordering and archive checks.
                                for hostile in 0..9 {
                                    let before = budget.storage();
                                    let mut cursor = ExecutionAvailabilityV29::new_with_identity(
                                        instances,
                                        *id,
                                        None,
                                        Some(plan),
                                        budget,
                                    )?;
                                    let mut locals =
                                        vec![None; original.declaration().locals().len()];
                                    locals[1] =
                                        Some(SemanticValueBindingV1::Execution(seed.clone()));
                                    cursor.install_retained_seeds_v1(
                                        &[(1, SemanticValueBindingV1::Execution(seed.clone()))],
                                        budget,
                                    )?;
                                    cursor.begin_block(SemanticBlockIdV1::from_index(0), budget)?;
                                    let cloned = place.clone();
                                    match hostile {
                                        0 => {
                                            assert!(cursor.use_retained_place_v1(
                                                site, role, place, &locals, budget
                                            )?);
                                            assert!(
                                                cursor
                                                    .use_retained_place_v1(
                                                        site, role, place, &locals, budget
                                                    )
                                                    .is_err()
                                            );
                                        }
                                        1 => {
                                            assert!(cursor.events.complete(budget).is_err());
                                        }
                                        2 => {
                                            assert!(
                                                cursor
                                                    .use_retained_place_v1(
                                                        site, role, &cloned, &locals, budget
                                                    )
                                                    .is_err()
                                            );
                                        }
                                        3 => {
                                            cursor.retained_seeds[1] = None;
                                            assert!(
                                                cursor
                                                    .use_retained_place_v1(
                                                        site, role, place, &locals, budget
                                                    )
                                                    .is_err()
                                            );
                                        }
                                        4 => {
                                            locals[1] = None;
                                            assert!(
                                                cursor
                                                    .use_retained_place_v1(
                                                        site, role, place, &locals, budget
                                                    )
                                                    .is_err()
                                            );
                                        }
                                        5 => {
                                            cursor.index.clear();
                                            assert!(
                                                cursor
                                                    .use_retained_place_v1(
                                                        site, role, place, &locals, budget
                                                    )
                                                    .is_err()
                                            );
                                        }
                                        6 => {
                                            let wrong_site = ExecutionSiteV29::Statement {
                                                block: SsaBlockIdV1::new(1),
                                                statement: 0,
                                            };
                                            assert!(
                                                cursor
                                                    .use_retained_place_v1(
                                                        wrong_site, role, place, &locals, budget
                                                    )
                                                    .is_err()
                                            );
                                        }
                                        7 => {
                                            cursor.instance = instances.root();
                                            assert!(
                                                cursor
                                                    .use_retained_place_v1(
                                                        site, role, place, &locals, budget
                                                    )
                                                    .is_err()
                                            );
                                        }
                                        _ => {
                                            let Some(SemanticValueBindingV1::Execution(current)) =
                                                &mut locals[1]
                                            else {
                                                panic!()
                                            };
                                            current.identity.value = ValueId(u32::MAX);
                                            assert!(
                                                cursor
                                                    .use_retained_place_v1(
                                                        site, role, place, &locals, budget
                                                    )
                                                    .is_err()
                                            );
                                        }
                                    }
                                    assert_eq!(
                                        cursor.claimed.iter().filter(|claimed| **claimed).count(),
                                        usize::from(hostile == 0)
                                    );
                                    drop(cursor);
                                    budget.release_storage(budget.storage() - before)?;
                                }
                                if inputs.len() == 2 {
                                    let before = budget.storage();
                                    let other =
                                        inputs.iter().find(|(other, _)| other != id).unwrap();
                                    let mut cursor = ExecutionAvailabilityV29::new_with_identity(
                                        instances,
                                        *id,
                                        None,
                                        Some(plan),
                                        budget,
                                    )?;
                                    let mut locals =
                                        vec![None; original.declaration().locals().len()];
                                    locals[1] =
                                        Some(SemanticValueBindingV1::Execution(other.1.clone()));
                                    cursor.install_retained_seeds_v1(
                                        &[(1, SemanticValueBindingV1::Execution(seed.clone()))],
                                        budget,
                                    )?;
                                    cursor.begin_block(SemanticBlockIdV1::from_index(0), budget)?;
                                    // Equivalent source referents are not a
                                    // wrong seed. The instance substitution is.
                                    assert_eq!(other.1, *seed);
                                    cursor.instance = other.0;
                                    assert!(
                                        cursor
                                            .use_retained_place_v1(
                                                site, role, place, &locals, budget
                                            )
                                            .is_err()
                                    );
                                    drop(cursor);
                                    budget.release_storage(budget.storage() - before)?;
                                }
                                let before = budget.storage();
                                assert!(
                                    ExecutionAvailabilityV29::new(instances, *id, budget).is_err(),
                                    "legacy cursor must not omit retained required events"
                                );
                                budget.release_storage(budget.storage() - before)?;
                            }
                            Ok(())
                        },
                    )?;
                    assert_eq!(budget.storage(), floor);
                    for panic in [false, true] {
                        let history = budget.failed_storage();
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            with_execution_identity_plan_v1(
                                instances,
                                references,
                                &root,
                                budget,
                                |plan, budget| -> Result<(), ProductionSemanticKirErrorV1> {
                                    assert_eq!(plan.unwrap().index.retained.len(), inputs.len());
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
                        assert_eq!(budget.failed_storage(), history);
                    }
                    Ok(())
                },
            )
        },
    )?;
    REACHED.set(inputs.len());
    Ok(())
}

#[test]
fn actual_retained_entry_borrows_cover_acyclic_cyclic_and_repeated_instances() {
    for cyclic in [false, true] {
        for repeated in [false, true] {
            let result = run_suffix_owner(
                || retained_owner(cyclic, repeated),
                observe_retained_entries,
                LIMIT,
                LIMIT,
                |_, _, _| Ok(()),
            )
            .0;
            assert!(
                result.is_ok(),
                "cyclic={cyclic}, repeated={repeated}: {result:?}"
            );
            assert_eq!(REACHED.get(), if repeated { 2 } else { 1 });
        }
    }
}

#[test]
fn original_retained_entry_write_kill_and_lifetime_events_cannot_receive_a_certificate() {
    for fault in 0..4 {
        let make = || {
            let original = retained_owner(true, false);
            let semantic = original.source_semantic();
            let mut functions = semantic.functions().to_vec();
            let callback = &functions[CALLBACK.index() as usize];
            let ty = callback.locals()[1].ty();
            let changed = match fault {
                0 => assign(
                    place(1, ty),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, ty))),
                ),
                1 => fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
                ),
                2 => fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1)),
                ),
                _ => fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::Deinitialize(place(1, ty)),
                ),
            };
            let mut blocks = callback.blocks().to_vec();
            let first = &blocks[0];
            let mut statements = first.statements().to_vec();
            statements.push(changed);
            blocks[0] = fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1::new(
                first.identity(),
                first.source(),
                statements,
                first.terminator().clone(),
            )
            .unwrap();
            functions[CALLBACK.index() as usize] = function(
                110,
                SemanticFunctionRoleV1::InternalHelper,
                callback.abi().clone(),
                callback.locals().to_vec(),
                blocks,
            );
            retained_rebuild(
                semantic.types().to_vec(),
                functions,
                semantic.callables().to_vec(),
            )
        };
        let owner = make();
        let callback = &owner.source_semantic().functions()[CALLBACK.index() as usize];
        assert!(matches!(
            callback.blocks()[0].statements()[1].kind(),
            SemanticStatementKindV1::Assign(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::Deinitialize(_)
        ));
        let result = run_suffix_owner(
            make,
            |_, _, _, _, _| panic!("changed retained entry must not emit"),
            LIMIT,
            LIMIT,
            |_, _, _| Ok(()),
        )
        .0;
        assert!(result.is_err(), "fault={fault}");
    }
}

fn retained_call_escape_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = retained_owner(true, false);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    let helper_callable = retained_insert_callable(&mut functions, &mut callables);
    let callback = &functions[CALLBACK.index() as usize];
    let reference_type = callback.locals()[3].ty();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([173; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(value_abi(
            semantic.types(),
            reference_type,
        ))],
        direct(U32),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    let helper = function(
        174,
        SemanticFunctionRoleV1::InternalHelper,
        abi,
        vec![
            local(175, U32, SemanticLocalRoleV1::Return),
            local(176, reference_type, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(
            177,
            vec![assign(place(0, U32), SemanticRvalueKindV1::Use(literal(7)))],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let mut blocks = callback.blocks().to_vec();
    blocks[0] = block(
        115,
        blocks[0].statements().to_vec(),
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                helper_callable,
                vec![SemanticOperandV1::Copy(place(3, reference_type))],
                Some(SemanticCallDestinationV1::new(
                    place(0, U32),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    functions[CALLBACK.index() as usize] = function(
        110,
        SemanticFunctionRoleV1::InternalHelper,
        callback.abi().clone(),
        callback.locals().to_vec(),
        blocks,
    );
    functions.push(helper);
    retained_rebuild(semantic.types().to_vec(), functions, callables)
}

#[test]
fn retained_shared_alias_can_enter_a_body_proved_ignored_borrow_callee() {
    call_effect_tests::prove_ignored_borrow_callee();
}

#[test]
fn retained_source_place_has_exact_work_and_no_unpaid_projection_or_copy_authority() {
    let owner = retained_owner(true, false);
    let function = &owner.source_semantic().functions()[CALLBACK.index() as usize];
    for limit in [4, 3] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 23);
        budget.reserve_storage(23).unwrap();
        let result = execution_identity_retained_place_v1(
            function,
            ExecutionSiteV29::Statement {
                block: SsaBlockIdV1::new(0),
                statement: 0,
            },
            ExecutionOperandV29::RvaluePlace,
            &mut budget,
        );
        assert_eq!(result.is_ok(), limit == 4);
        if let Ok(Some(place)) = result {
            let SemanticStatementKindV1::Assign(assignment) =
                function.blocks()[0].statements()[0].kind()
            else {
                panic!()
            };
            let SemanticRvalueKindV1::Borrow {
                place: original, ..
            } = assignment.value().kind()
            else {
                panic!()
            };
            assert!(std::ptr::eq(place, original));
        }
        assert_eq!(budget.storage(), 23);
        assert_eq!(budget.peak_storage(), 23);
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.work(), if limit == 4 { 4 } else { 0 });
        assert_eq!(work.failed_work(), (limit == 3).then_some(4));
    }
}

#[test]
fn retained_seed_storage_has_independent_exact_and_one_short_limits() {
    for count in [1, 4] {
        let floor = 31;
        // The Vec header is already part of the cursor's separately paid
        // header. This helper pays allocation visits and each initialized slot.
        let work_required = 3 + count;
        let bytes = count * std::mem::size_of::<Option<SemanticExecutionBindingV29>>();
        for (work_limit, storage_limit, succeeds) in [
            (work_required, floor + bytes, true),
            (work_required - 1, floor + bytes, false),
            (work_required, floor + bytes - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = scoped_slot_attempt_v29(&mut budget, |budget| {
                execution_identity_retained_seed_slots_v1(count, budget)
            });
            assert_eq!(result.is_ok(), succeeds);
            if let Ok(seeds) = result {
                assert_eq!(seeds.capacity(), count);
                assert!(seeds.iter().all(Option::is_none));
                assert_eq!(budget.storage(), floor + bytes);
                drop(seeds);
                budget.release_storage(bytes).unwrap();
            }
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                budget.failed_storage(),
                (storage_limit < floor + bytes).then_some(floor + bytes)
            );
            assert_eq!(
                budget.peak_storage(),
                if storage_limit < floor + bytes {
                    floor
                } else {
                    floor + bytes
                }
            );
            drop(budget);
            assert_eq!(work.work(), if succeeds { work_required } else { 3 });
            assert_eq!(
                work.failed_work(),
                (work_limit < work_required).then_some(work_required)
            );
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    assert!(
        execution_identity_retained_seed_slots_v1(0, &mut budget)
            .unwrap()
            .is_empty()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn retained_seed_cleanup_preserves_floor_and_first_denial_through_error_and_panic() {
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(37).unwrap();
        assert!(budget.charge_work(LIMIT + 1).is_err());
        assert!(budget.reserve_storage(LIMIT).is_err());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_slot_attempt_v29(&mut budget, |budget| {
                let seeds = execution_identity_retained_seed_slots_v1(4, budget)?;
                let bytes =
                    seeds.capacity() * std::mem::size_of::<Option<SemanticExecutionBindingV29>>();
                drop(seeds);
                match mode {
                    0 => budget.release_storage(bytes).map_err(Into::into),
                    1 => Err(execution_identity_error_v1()),
                    _ => std::panic::panic_any(271usize),
                }
            })
        }));
        assert_eq!(result.is_err(), mode == 2);
        if let Ok(result) = result {
            assert_eq!(result.is_ok(), mode == 0);
        }
        assert_eq!(budget.storage(), 37);
        assert_eq!(budget.failed_storage(), Some(LIMIT + 37));
        drop(budget);
        assert_eq!(work.work(), 7);
        assert_eq!(work.failed_work(), Some(LIMIT + 1));
    }
}
