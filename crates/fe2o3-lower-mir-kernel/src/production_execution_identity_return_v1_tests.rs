use super::*;

fn input_return_owner(two_exits: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = nominal_loop_owner(NominalLoopCase::Repeated);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    let ty = helper.abi().source_output_type();
    let normal_return = |marker| {
        block(
            marker,
            vec![assign(
                place(0, ty),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, ty))),
            )],
            SemanticTerminatorKindV1::Return,
        )
    };
    let blocks = if two_exits {
        vec![
            block(
                135,
                vec![],
                switch(SemanticOperandV1::Copy(place(2, U32)), 1, 2),
            ),
            normal_return(136),
            normal_return(137),
        ]
    } else {
        vec![normal_return(135)]
    };
    functions[3] = function(
        130,
        SemanticFunctionRoleV1::InternalHelper,
        helper.abi().clone(),
        helper.locals().to_vec(),
        blocks,
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        semantic.types().to_vec(),
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

fn aggregate_input_return_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = input_return_owner(true);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let workgroup = functions[3].abi().source_output_type();
    let pair = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(24),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![workgroup, U32]).unwrap()),
        None,
    );
    let original_callback = &functions[CALLBACK.index() as usize];
    let mut locals = original_callback.locals()[..3].to_vec();
    locals.push(local(160, pair, SemanticLocalRoleV1::Temporary));
    let call = |next| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(3),
                vec![
                    SemanticOperandV1::Move(place(3, pair)),
                    SemanticOperandV1::Copy(place(2, U32)),
                ],
                Some(SemanticCallDestinationV1::new(
                    place(3, pair),
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
    let scalar = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(3),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), U32).unwrap()],
        U32,
    )
    .unwrap();
    functions[CALLBACK.index() as usize] = function(
        110,
        SemanticFunctionRoleV1::InternalHelper,
        original_callback.abi().clone(),
        locals,
        vec![
            block(
                115,
                vec![assign(
                    place(3, pair),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Tuple,
                            vec![
                                SemanticOperandV1::Move(place(1, workgroup)),
                                SemanticOperandV1::Copy(place(2, U32)),
                            ],
                        )
                        .unwrap(),
                    ),
                )],
                call(1),
            ),
            block(117, vec![], call(2)),
            block(
                118,
                vec![assign(
                    place(0, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(scalar)),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([131; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(value_abi(&types, pair)),
            SemanticAbiArgumentV1::source(direct(U32)),
        ],
        value_abi(&types, pair),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    functions[3] = function(
        130,
        SemanticFunctionRoleV1::InternalHelper,
        abi,
        vec![
            local(132, pair, SemanticLocalRoleV1::Return),
            local(133, pair, SemanticLocalRoleV1::Argument(0)),
            local(134, U32, SemanticLocalRoleV1::Argument(1)),
        ],
        vec![
            block(
                135,
                vec![],
                switch(SemanticOperandV1::Copy(place(2, U32)), 1, 2),
            ),
            block(
                136,
                vec![assign(
                    place(0, pair),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, pair))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
            block(
                137,
                vec![assign(
                    place(0, pair),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, pair))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
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

fn observe_input_returns(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut helpers = 0;
    let mut helper_ids = Vec::new();
    for (ordinal, original) in instances.instances().iter().enumerate() {
        if original.function().index() != 3 {
            continue;
        }
        helpers += 1;
        let id = instances.id_at(ordinal).unwrap();
        helper_ids.push(id);
        let lowered = emitted[ordinal].as_ref().unwrap();
        assert_eq!(lowered.source_call_instance, Some(id));
        let observed = lowered.execution_observation.as_ref().unwrap();
        let occurrences = instances.occurrences(id).unwrap();
        let entry = occurrences
            .entry_definitions()
            .iter()
            .find(|entry| entry.variable().get() == 1)
            .unwrap()
            .value()
            .unwrap();
        let input = one_nominal_leaf(&observed.bindings[&entry]);
        let returns: Vec<_> = occurrences
            .events()
            .iter()
            .filter(|event| {
                event.operand() == ExecutionOperandV29::ReturnValue
                    && event.role() == ExecutionEventV29::BaseUse
            })
            .collect();
        assert!(returns.len() == 1 || returns.len() == 2);
        let witness = || {
            let mut observations = Vec::new();
            for event in &returns {
                assert!(event.is_promoted() && event.is_reachable());
                let Some(SsaResolvedEventV1::Use { variable, value }) = event.resolved() else {
                    panic!("normal return must resolve its original use")
                };
                assert_eq!(variable.get(), 0);
                let returned = one_nominal_leaf(&observed.bindings[&value]);
                assert_eq!(returned, input);
                let ExecutionSiteV29::Terminator { block } = event.site() else {
                    panic!("return site")
                };
                let source = SemanticBlockIdV1::from_index(block.get());
                let mapping = lowered
                    .blocks
                    .iter()
                    .find(|row| row.semantic_block == source)
                    .unwrap();
                assert_eq!(mapping.semantic_function, original.function());
                let target = lowered
                    .function
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .find(|row| row.id == mapping.kernel_ir_block)
                    .unwrap();
                let Some(Terminator::Return { values }) = &target.terminator else {
                    panic!("actual emitted normal return")
                };
                let direct_nominal = execution_cfg_nominal_kind_v29(
                    instances.owner().source_semantic().types(),
                    original.declaration().abi().source_output_type(),
                )
                .unwrap()
                .is_some();
                assert_eq!(
                    values.len(),
                    usize::from(!direct_nominal),
                    "only the mixed aggregate's scalar has a physical result"
                );
                observations.push(ExecutionIdentityReturnSlotV1 {
                    exit: ExecutionIdentityReturnExitV1 {
                        block: source,
                        local: SemanticLocalIdV1::from_index(variable.get()),
                    },
                    observation: Some(ExecutionIdentityReturnObservationV1 {
                        source,
                        target: target.id,
                        values: values.clone(),
                        nominal: vec![Some(ExecutionCfgLeafV29::Owned(returned.clone()))],
                    }),
                });
            }
            observations.sort_unstable_by_key(|slot| slot.exit.block.index());
            ExecutionIdentityReturnWitnessV1 {
                expected: vec![Some(ExecutionCfgLeafV29::Owned(input.clone()))],
                observations,
            }
        };
        let check = |witness: &ExecutionIdentityReturnWitnessV1,
                     child: ProductionCallInstanceIdV1,
                     budget: &mut ArgumentBudgetV1<'_>| {
            let output = emitted
                .get(child.index())
                .and_then(Option::as_ref)
                .ok_or_else(execution_identity_error_v1)?;
            witness.finish(child, original.function(), output, instances, budget)
        };
        let valid = witness();
        let storage = budget.storage();
        assert!(check(&valid, id, budget).is_ok());
        assert_eq!(budget.storage(), storage);
        let mut missing = witness();
        missing.observations.pop();
        assert!(check(&missing, id, budget).is_err());
        let mut replaced = witness();
        replaced.expected[0] = Some(ExecutionCfgLeafV29::Moved);
        assert!(check(&replaced, id, budget).is_err());
        let mut wrong_values = witness();
        wrong_values.observations[0]
            .observation
            .as_mut()
            .unwrap()
            .values
            .push(ValueId(u32::MAX));
        assert!(check(&wrong_values, id, budget).is_err());
        let mut wrong_block = witness();
        wrong_block.observations[0]
            .observation
            .as_mut()
            .unwrap()
            .target = BlockId(u32::MAX);
        assert!(check(&wrong_block, id, budget).is_err());
        if returns.len() == 2 {
            let mut duplicate = witness();
            duplicate.observations[1]
                .observation
                .as_mut()
                .unwrap()
                .source = duplicate.observations[0]
                .observation
                .as_ref()
                .unwrap()
                .source;
            assert!(check(&duplicate, id, budget).is_err());
        }
        let sibling = instances
            .instances()
            .iter()
            .enumerate()
            .find_map(|(index, row)| {
                let candidate = instances.id_at(index).unwrap();
                (row.function() == original.function() && candidate != id).then_some(candidate)
            })
            .unwrap();
        assert!(check(&valid, sibling, budget).is_err());
    }
    assert_eq!(helpers, 2);
    assert_ne!(helper_ids[0], helper_ids[1]);
    REACHED.set(helpers);
    Ok(())
}

fn observe_return_index_limits(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let outputs: Vec<_> = emitted.iter().map(|row| row.as_ref().unwrap()).collect();
    let count = instances.instances().len();
    assert_eq!(outputs.len(), count);
    assert!(count > 1);
    // This inert borrowed index grants no custody or source authority. Its
    // isolated ledger pays only its own header, slots and bounded visits.
    let floor = 31;
    let bytes = std::mem::size_of::<Vec<Option<&LoweredFunctionResultV1>>>()
        + count * std::mem::size_of::<Option<&LoweredFunctionResultV1>>();
    let exact_work = 3 + count + 4 * (count + 1) + count;
    for (work_limit, storage_limit, succeeds) in [
        (exact_work, floor + bytes, true),
        (exact_work - 1, floor + bytes, false),
        (exact_work, floor + bytes - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor)?;
        assert!(budget.charge_work(work_limit + 17).is_err());
        assert!(budget.reserve_storage(storage_limit + 19).is_err());
        let storage_history = budget.failed_storage();
        let result =
            execution_emitted_index_v1(instances, outputs.iter().copied().rev(), &mut budget);
        assert_eq!(result.is_ok(), succeeds, "{work_limit}/{storage_limit}");
        if let Ok(index) = result {
            assert_eq!(index.capacity(), count);
            for (ordinal, row) in index.iter().enumerate() {
                assert!(std::ptr::eq(row.unwrap(), outputs[ordinal]));
            }
            drop(index);
            budget.release_storage(bytes)?;
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), storage_history);
        assert_eq!(storage_history, Some(floor + storage_limit + 19));
        assert_eq!(
            budget.peak_storage(),
            if storage_limit < floor + bytes {
                floor + std::mem::size_of::<Vec<Option<&LoweredFunctionResultV1>>>()
            } else {
                floor + bytes
            }
        );
        drop(budget);
        assert_eq!(
            work.work(),
            if storage_limit < floor + bytes {
                3
            } else if succeeds {
                exact_work
            } else {
                exact_work - count
            }
        );
        assert_eq!(work.failed_work(), Some(work_limit + 17));
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(floor)?;
    for fault in 0..3 {
        let mut rows = outputs.clone();
        match fault {
            0 => {
                rows.pop();
            }
            1 => {
                rows[1] = rows[0];
            }
            _ => {
                rows.push(rows[0]);
            }
        }
        assert!(execution_emitted_index_v1(instances, rows.into_iter(), &mut budget).is_err());
        assert_eq!(budget.storage(), floor);
    }
    let work_before_denial = budget.work();
    assert!(budget.charge_work(LIMIT + 1).is_err());
    assert!(budget.reserve_storage(LIMIT + 1).is_err());
    let prior_storage_denial = budget.failed_storage();
    let mut position = 0;
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        execution_emitted_index_v1(
            instances,
            outputs.iter().copied().map(|row| {
                position += 1;
                if position == 2 {
                    std::panic::panic_any(271usize);
                }
                row
            }),
            &mut budget,
        )
    }));
    let Err(payload) = panicked else {
        panic!("iterator panic must propagate")
    };
    assert_eq!(*payload.downcast::<usize>().unwrap(), 271);
    assert_eq!(position, 2);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), prior_storage_denial);
    drop(outputs);
    for replacement in [None, Some(ProductionCallInstanceIdV1(usize::MAX))] {
        let saved = std::mem::replace(
            &mut emitted[0].as_mut().unwrap().source_call_instance,
            replacement,
        );
        let result = execution_emitted_index_v1(
            instances,
            emitted.iter().filter_map(Option::as_ref),
            &mut budget,
        );
        assert!(result.is_err());
        drop(result);
        emitted[0].as_mut().unwrap().source_call_instance = saved;
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(budget.failed_storage(), prior_storage_denial);
    drop(budget);
    assert_eq!(work.failed_work(), Some(work_before_denial + LIMIT + 1));
    REACHED.set(count);
    Ok(())
}

#[test]
fn actual_emitted_roster_index_has_exact_limits_and_rejects_missing_duplicate_extra_rows() {
    let result = run_suffix_owner(
        || input_return_owner(true),
        observe_return_index_limits,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    )
    .0;
    assert!(result.is_ok(), "{result:?}");
    assert!(REACHED.get() > 1);
}

#[test]
fn actual_acyclic_input_returns_check_every_original_exit_and_instance() {
    for two_exits in [false, true] {
        let result = run_suffix_owner(
            || input_return_owner(two_exits),
            observe_input_returns,
            LIMIT,
            LIMIT,
            |_, _, _| Ok(()),
        )
        .0;
        assert!(result.is_ok(), "two exits {two_exits}: {result:?}");
        assert_eq!(REACHED.get(), 2);
    }
}

#[test]
fn unchanged_repeated_cyclic_input_return_fixture_reaches_the_real_emitter() {
    let result = run_suffix_owner(
        || nominal_loop_owner(NominalLoopCase::Repeated),
        observe_input_returns,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    )
    .0;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(REACHED.get(), 2);
}

#[test]
fn actual_mixed_aggregate_returns_keep_scalar_results_and_nominal_identity_distinct() {
    let result = run_suffix_owner(
        aggregate_input_return_owner,
        observe_input_returns,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    )
    .0;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(REACHED.get(), 2);
}

#[test]
fn return_leaf_layout_distinguishes_borrow_identity_from_referent_and_physical_width() {
    let owner = nominal_loop_owner(NominalLoopCase::Borrowed);
    let types = owner.source_semantic().types();
    let callback = &owner.source_semantic().functions()[CALLBACK.index() as usize];
    for (local, width, borrowed) in [(1, 1, false), (3, 2, true)] {
        let ty = callback.locals()[local].ty();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        let leaves = execution_identity_return_leaf_layout_v1(
            types,
            ExecutionIdentitySelectionV1 {
                ty,
                first: 9,
                count: width,
            },
            &mut budget,
        )
        .unwrap();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].ty, ty);
        assert_eq!(leaves[0].first, 9);
        assert_eq!(leaves[0].borrowed, borrowed);
        assert!(
            execution_identity_return_leaf_layout_v1(
                types,
                ExecutionIdentitySelectionV1 {
                    ty,
                    first: 9,
                    count: width + 1
                },
                &mut budget
            )
            .is_err()
        );
    }
}

#[test]
fn single_return_leaf_has_independently_counted_exact_work_storage_and_cleanup() {
    let owner = nominal_loop_owner(NominalLoopCase::Entry);
    let types = owner.source_semantic().types();
    let ty = owner.source_semantic().functions()[CALLBACK.index() as usize].locals()[1].ty();
    let floor = 29;
    // Pending vector: header + one type. First leaf grows its vector to four.
    let scratch =
        std::mem::size_of::<Vec<SemanticTypeIdV1>>() + std::mem::size_of::<SemanticTypeIdV1>();
    let retained = 4 * std::mem::size_of::<ExecutionIdentityReturnLeafV1>();
    // Three allocation visits, one node, two push visits, three growth visits.
    let exact_work = 3 + 1 + 2 + 3;
    let exact_storage = floor + scratch + retained;
    for (work_limit, storage_limit, succeeds) in [
        (exact_work, exact_storage, true),
        (exact_work - 1, exact_storage, false),
        (exact_work, exact_storage - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = scoped_slot_attempt_v29(&mut budget, |budget| {
            execution_identity_return_leaf_layout_v1(
                types,
                ExecutionIdentitySelectionV1 {
                    ty,
                    first: 0,
                    count: 1,
                },
                budget,
            )
        });
        assert_eq!(result.is_ok(), succeeds);
        if let Ok(leaves) = result {
            assert_eq!(leaves.capacity(), 4);
            assert_eq!(budget.storage(), floor + retained);
            assert_eq!(budget.peak_storage(), exact_storage);
            drop(leaves);
            budget.release_storage(retained).unwrap();
        }
        assert_eq!(budget.storage(), floor);
        assert!(budget.peak_storage() <= storage_limit);
        assert_eq!(
            budget.failed_storage(),
            (!succeeds && storage_limit < exact_storage).then_some(exact_storage)
        );
        drop(budget);
        assert_eq!(
            work.work(),
            if work_limit < exact_work {
                6
            } else {
                exact_work
            }
        );
        assert_eq!(
            work.failed_work(),
            (work_limit < exact_work).then_some(exact_work)
        );
    }
}

#[test]
fn return_leaf_cleanup_keeps_prior_denial_history_on_success_error_and_panic() {
    let owner = nominal_loop_owner(NominalLoopCase::Entry);
    let types = owner.source_semantic().types();
    let ty = owner.source_semantic().functions()[CALLBACK.index() as usize].locals()[1].ty();
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(37).unwrap();
        assert!(budget.charge_work(LIMIT + 1).is_err());
        assert!(budget.reserve_storage(LIMIT).is_err());
        let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_slot_attempt_v29(&mut budget, |budget| {
                let leaves = execution_identity_return_leaf_layout_v1(
                    types,
                    ExecutionIdentitySelectionV1 {
                        ty,
                        first: 0,
                        count: 1,
                    },
                    budget,
                )?;
                let retained =
                    leaves.capacity() * std::mem::size_of::<ExecutionIdentityReturnLeafV1>();
                drop(leaves);
                match mode {
                    0 => budget.release_storage(retained).map_err(Into::into),
                    1 => Err(execution_identity_error_v1()),
                    _ => panic!("return layout consumer"),
                }
            })
        }));
        assert_eq!(observed.is_err(), mode == 2);
        if let Ok(result) = observed {
            assert_eq!(result.is_ok(), mode == 0);
        }
        assert_eq!(budget.storage(), 37);
        assert_eq!(budget.failed_storage(), Some(LIMIT + 37));
        drop(budget);
        assert_eq!(work.work(), 9);
        assert_eq!(work.failed_work(), Some(LIMIT + 1));
    }
}

fn many_exit_return_owner(exits: usize, shifted_entry: bool) -> ProductionSemanticSsaOwnerV1 {
    assert!((1..=32).contains(&exits));
    let original = input_return_owner(false);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    let ty = helper.abi().source_output_type();
    let count = exits * 2 - 1;
    let offset = usize::from(shifted_entry) * (count - 1);
    let physical = |logical| ((logical + offset) % count) as u32;
    let mut blocks = vec![None; count];
    for logical in 0..count {
        let index = physical(logical);
        let normal = logical % 2 == 1 || logical + 1 == count;
        blocks[index as usize] = Some(block(
            135 + index as u8,
            if normal {
                vec![assign(
                    place(0, ty),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, ty))),
                )]
            } else {
                vec![]
            },
            if normal {
                SemanticTerminatorKindV1::Return
            } else {
                switch(
                    SemanticOperandV1::Copy(place(2, U32)),
                    physical(logical + 1),
                    physical(logical + 2),
                )
            },
        ));
    }
    functions[3] = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        helper.source(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        SemanticBlockIdV1::from_index(physical(0)),
        blocks.into_iter().map(Option::unwrap).collect(),
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        semantic.types().to_vec(),
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

fn observe_many_exit_census(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut helpers = 0;
    for (ordinal, original) in instances.instances().iter().enumerate() {
        if original.function().index() != 3 {
            continue;
        }
        helpers += 1;
        let id = instances.id_at(ordinal).unwrap();
        let output = emitted[ordinal].as_mut().unwrap();
        assert!(output.invocation_entry.is_none());
        assert!(output.synthetic_operation_spans.is_empty());
        let observation = output.execution_observation.as_ref().unwrap();
        let occurrences = instances.occurrences(id).unwrap();
        let entry = occurrences
            .entry_definitions()
            .iter()
            .find(|row| row.variable().get() == 1)
            .unwrap()
            .value()
            .unwrap();
        let input = one_nominal_leaf(&observation.bindings[&entry]);
        let mut slots = Vec::new();
        for event in occurrences.events().iter().filter(|event| {
            event.operand() == ExecutionOperandV29::ReturnValue
                && event.role() == ExecutionEventV29::BaseUse
        }) {
            let Some(SsaResolvedEventV1::Use { variable, value }) = event.resolved() else {
                panic!("resolved original return")
            };
            assert!(event.is_promoted() && event.is_reachable());
            assert_eq!(variable.get(), 0);
            let returned = one_nominal_leaf(&observation.bindings[&value]);
            assert_eq!(returned, input);
            let ExecutionSiteV29::Terminator { block } = event.site() else {
                panic!()
            };
            let source = SemanticBlockIdV1::from_index(block.get());
            let mapping = output
                .blocks
                .iter()
                .find(|row| row.semantic_block == source)
                .unwrap();
            let block = output
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == mapping.kernel_ir_block)
                .unwrap();
            let Some(Terminator::Return { values }) = &block.terminator else {
                panic!()
            };
            assert!(values.is_empty());
            slots.push(ExecutionIdentityReturnSlotV1 {
                exit: ExecutionIdentityReturnExitV1 {
                    block: source,
                    local: SemanticLocalIdV1::from_index(variable.get()),
                },
                observation: Some(ExecutionIdentityReturnObservationV1 {
                    source,
                    target: block.id,
                    values: values.clone(),
                    nominal: vec![Some(ExecutionCfgLeafV29::Owned(returned.clone()))],
                }),
            });
        }
        slots.sort_unstable_by_key(|slot| slot.exit.block.index());
        let mut witness = ExecutionIdentityReturnWitnessV1 {
            expected: vec![Some(ExecutionCfgLeafV29::Owned(input.clone()))],
            observations: slots,
        };
        let exits = witness.observations.len();
        let blocks = original.declaration().blocks().len();
        assert_eq!(blocks, exits * 2 - 1);
        assert_eq!(original.ssa().plan().reverse_postorder().len(), blocks);
        let lookup = (exits.ilog2() as usize + 2) * 16;
        // Entry reconstruction: 4 fixed + B block visits + 2 visits per each
        // of 2(E-1) edges + original entry definitions. The remaining census
        // pays fixed 4, 2E validity/count, 5E shape, 2E exit, 3B active
        // validity/count/join, 6B mapping and two E lookups.
        let entry_work =
            4 + blocks + 4 * (exits - 1) + original.ssa().plan().entry_definitions().len();
        let exact_work =
            4 + 2 * exits + 5 * exits + 2 * exits + 9 * blocks + 2 * exits * lookup + entry_work;
        let floor = 37;
        let scratch = std::mem::size_of::<InvocationEntryPlanV1<'_>>();
        for (work_limit, storage_limit, succeeds) in [
            (exact_work, floor + scratch, true),
            (exact_work - 1, floor + scratch, false),
            (exact_work, floor + scratch - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut query = ArgumentBudgetV1::new(&mut work, storage_limit);
            query.reserve_storage(floor)?;
            let result = witness.finish(id, original.function(), output, instances, &mut query);
            assert_eq!(
                result.is_ok(),
                succeeds,
                "E={exits}, work={work_limit}, storage={storage_limit}: {result:?}"
            );
            assert_eq!(query.storage(), floor);
            assert_eq!(
                query.failed_storage(),
                (storage_limit < floor + scratch).then_some(floor + scratch)
            );
            drop(query);
            if succeeds {
                assert_eq!(work.work(), exact_work);
            }
            assert_eq!(
                work.failed_work(),
                (work_limit < exact_work).then_some(exact_work)
            );
        }
        let before = budget.storage();
        let check = |witness: &ExecutionIdentityReturnWitnessV1,
                     output: &LoweredFunctionResultV1,
                     budget: &mut ArgumentBudgetV1<'_>| {
            witness.finish(id, original.function(), output, instances, budget)
        };
        let saved = witness.observations[0].observation.take();
        assert!(check(&witness, output, budget).is_err());
        witness.observations[0].observation = saved;
        if exits > 1 {
            let original_exit = witness.observations[1].exit;
            witness.observations[1].exit = witness.observations[0].exit;
            assert!(check(&witness, output, budget).is_err());
            witness.observations[1].exit = original_exit;
            output.blocks.swap(0, 1);
            assert!(check(&witness, output, budget).is_err());
            output.blocks.swap(0, 1);
            output.function.body.as_mut().unwrap().blocks.swap(0, 1);
            assert!(check(&witness, output, budget).is_err());
            output.function.body.as_mut().unwrap().blocks.swap(0, 1);
            let previous = output.blocks[1].kernel_ir_block;
            output.blocks[1].kernel_ir_block = output.blocks[0].kernel_ir_block;
            assert!(check(&witness, output, budget).is_err());
            output.blocks[1].kernel_ir_block = previous;
        }
        check(&witness, output, budget)?;
        assert_eq!(budget.storage(), before);
    }
    REACHED.set(helpers);
    Ok(())
}

#[test]
fn actual_many_exit_and_nonzero_entry_returns_have_independent_logarithmic_census_work() {
    for exits in [1, 8, 32] {
        for shifted in [false, true] {
            let result = run_suffix_owner(
                || many_exit_return_owner(exits, shifted),
                observe_many_exit_census,
                LIMIT,
                LIMIT,
                |_, _, _| Ok(()),
            )
            .0;
            assert!(
                result.is_ok(),
                "exits={exits}, shifted={shifted}: {result:?}"
            );
            assert_eq!(REACHED.get(), 2);
        }
    }
}

fn phase_replay_return_values(
    emitted: &mut [Option<LoweredFunctionResultV1>],
    instance: usize,
    block: usize,
) -> &mut Vec<ValueId> {
    let target = &mut emitted[instance]
        .as_mut()
        .unwrap()
        .function
        .body
        .as_mut()
        .unwrap()
        .blocks[block];
    let Some(Terminator::Return { values }) = &mut target.terminator else {
        panic!("original normal return")
    };
    values
}

fn observe_return_phase_replay(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    observe_input_returns(source, instances, emitted, slots, budget)?;
    with_scoped_source_test_layouts_v29(
        source,
        ProductionSemanticKirLimitsV1::default(),
        budget,
        |_, layouts, budget| {
            source_storage_v29::with_source_storage_root_v29(
                layouts,
                instances,
                budget,
                |references, _, budget| {
                    let floor = budget.storage();
                    // Reconstruct from the original owner, independently of the
                    // source plan that installed the actual emitted arguments.
                    check_scoped_defined_call_phases_with_references_v29(
                        instances,
                        emitted,
                        Some(references),
                        budget,
                    )?;
                    assert_eq!(budget.storage(), floor);
                    assert!(matches!(
                        check_scoped_defined_call_phases_v29(instances, emitted, budget),
                        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    ));
                    assert_eq!(budget.storage(), floor);
                    let helpers: Vec<_> = instances
                        .instances()
                        .iter()
                        .enumerate()
                        .filter(|(_, row)| row.function().index() == 3)
                        .map(|(ordinal, _)| ordinal)
                        .collect();
                    assert_eq!(helpers.len(), 2);
                    let first = helpers[0];
                    let second = instances.id_at(helpers[1]).unwrap();
                    let check =
                        |emitted: &[Option<LoweredFunctionResultV1>],
                         budget: &mut ArgumentBudgetV1<'_>| {
                            check_scoped_defined_call_phases_with_references_v29(
                                instances,
                                emitted,
                                Some(references),
                                budget,
                            )
                        };
                    for replacement in [None, Some(second)] {
                        let original = std::mem::replace(
                            &mut emitted[first].as_mut().unwrap().source_call_instance,
                            replacement,
                        );
                        let rejected = check(emitted, budget);
                        emitted[first].as_mut().unwrap().source_call_instance = original;
                        assert!(matches!(
                            rejected,
                            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                        ));
                        assert_eq!(budget.storage(), floor);
                        check(emitted, budget)?;
                    }
                    emitted[first]
                        .as_mut()
                        .unwrap()
                        .function
                        .signature
                        .results
                        .push(Type::Scalar(ScalarType::U32));
                    let rejected = check(emitted, budget);
                    emitted[first]
                        .as_mut()
                        .unwrap()
                        .function
                        .signature
                        .results
                        .pop();
                    assert!(matches!(
                        rejected,
                        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    ));
                    assert_eq!(budget.storage(), floor);
                    check(emitted, budget)?;
                    let return_index = emitted[first]
                        .as_ref()
                        .unwrap()
                        .function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .position(|block| {
                            matches!(block.terminator, Some(Terminator::Return { .. }))
                        })
                        .unwrap();
                    phase_replay_return_values(emitted, first, return_index)
                        .push(ValueId(u32::MAX));
                    let rejected = check(emitted, budget);
                    phase_replay_return_values(emitted, first, return_index).pop();
                    assert!(matches!(
                        rejected,
                        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    ));
                    assert_eq!(budget.storage(), floor);
                    check(emitted, budget)?;
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            )
        },
    )
}

#[test]
fn source_aware_return_phase_replay_preserves_legacy_and_hostile_refusals() {
    for case in 0..4 {
        let result = run_suffix_owner(
            || match case {
                0 => input_return_owner(false),
                1 => input_return_owner(true),
                2 => aggregate_input_return_owner(),
                _ => nominal_loop_owner(NominalLoopCase::Repeated),
            },
            observe_return_phase_replay,
            LIMIT,
            LIMIT,
            |_, _, _| Ok(()),
        )
        .0;
        assert!(result.is_ok(), "case {case}: {result:?}");
        assert_eq!(REACHED.get(), 2);
    }
}
