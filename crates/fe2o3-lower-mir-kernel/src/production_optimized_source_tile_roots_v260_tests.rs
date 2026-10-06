// Inert admitted source fixtures exercise the complete lowering owner. Actual
// rustc collection/import remains a separate integration qualification.
fn multi_tile_owner(second_tile: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = super::super::super::fixtures::tile_parts_repeated_owner();
    let semantic = original.source_semantic();
    let root = &semantic.functions()[0];
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    let mut roots = vec![ROOT];
    let shared = SemanticFunctionIdV1::from_index(functions.len() as u32);
    let shared_call = SemanticCallableIdV1::from_index(callables.len() as u32);
    functions.push(function(
        230,
        SemanticFunctionRoleV1::InternalHelper,
        abi(231, false, &[]),
        vec![local(232, UNIT, SemanticLocalRoleV1::Return)],
        vec![block(233, vec![], SemanticTerminatorKindV1::Return)],
    ));
    callables.push(SemanticCallableDeclV1::defined(shared));
    let append_shared_call = |mut blocks: Vec<SemanticBasicBlockV1>| {
        let last = blocks.len() - 1;
        assert!(matches!(
            blocks[last].terminator().kind(),
            SemanticTerminatorKindV1::Return
        ));
        let next = SemanticBlockIdV1::from_index(blocks.len() as u32);
        blocks[last] = block(
            234,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    shared_call,
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        place(0, UNIT),
                        SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, next),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        );
        blocks.push(block(235, vec![], SemanticTerminatorKindV1::Return));
        blocks
    };
    let tile_blocks = append_shared_call(root.blocks().to_vec());
    functions[0] = function(
        80,
        root.role(),
        root.abi().clone(),
        root.locals().to_vec(),
        tile_blocks.clone(),
    )
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let mut add = |tag, name: &[u8], blocks| {
        let id = SemanticFunctionIdV1::from_index(functions.len() as u32);
        functions.push(
            function(
                tag,
                SemanticFunctionRoleV1::KernelRoot,
                root.abi().clone(),
                root.locals().to_vec(),
                blocks,
            )
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(name.to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([tag; 32]),
                root.kernel_entry().unwrap().source_contract(),
            )),
        );
        callables.push(SemanticCallableDeclV1::defined(id));
        roots.push(id);
    };
    if second_tile {
        add(210, b"tile_second_v260", tile_blocks);
    }
    add(
        220,
        b"scalar_root_v260",
        append_shared_call(vec![block(221, vec![], SemanticTerminatorKindV1::Return)]),
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        callables,
        roots,
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    kernel_argument_abi_v18::tests::fixture_descriptor_ownership_v18(
        ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap(),
    )
}

fn prepared_multi_tile(
    second_tile: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> ProductionPreparedSourceV18 {
    let projection = multi_tile_owner(second_tile);
    let owner = multi_tile_owner(second_tile);
    let semantic = projection.source_semantic();
    let launch_inputs: Vec<_> = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
            )
        })
        .collect();
    let launch =
        ProductionSourceLaunchRosterV1::try_new(owner.source_semantic(), &launch_inputs).unwrap();
    let mut roots = vec![root_input(&projection)];
    let mut events = vec![];
    for &root in semantic
        .roots()
        .iter()
        .take(if second_tile { 2 } else { 1 })
    {
        if root != ROOT {
            let mut input = root_input(&projection);
            input.root = root;
            input.root_identity = semantic.functions()[root.index() as usize].identity();
            roots.push(input);
        }
        events.push(crate::ProductionScopeEventCandidateV29 {
            function: root,
            block: SemanticBlockIdV1::from_index(1),
            statement_count: 0,
            kind: ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(1),
                kind: ProductionScopeCallKindV29::Provider,
            },
        });
    }
    let helper = &semantic.functions()[1];
    let SemanticTerminatorKindV1::Call(derive) = helper.blocks()[0].terminator().kind() else {
        panic!("derive");
    };
    let mut classes =
        vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    classes[1] = ProductionScopeCallableCandidateV29::Provider {
        function: HELPER,
        identity: helper.identity(),
    };
    classes[derive.callee().index() as usize] = ProductionScopeCallableCandidateV29::Derive {
        binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
        operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
        context: CONTEXT,
        workgroup: semantic.functions()[2].abi().source_input_types()[0],
    };
    for (ordinal, block) in helper.blocks().iter().enumerate() {
        let kind = match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => ProductionScopeEventKindV29::Call {
                callee: call.callee(),
                kind: if call.callee() == derive.callee() {
                    ProductionScopeCallKindV29::Derive
                } else {
                    ProductionScopeCallKindV29::Ordinary
                },
            },
            SemanticTerminatorKindV1::Return => ProductionScopeEventKindV29::Return,
            _ => panic!("provider control"),
        };
        events.push(crate::ProductionScopeEventCandidateV29 {
            function: HELPER,
            block: SemanticBlockIdV1::from_index(ordinal as u32),
            statement_count: block.statements().len(),
            kind,
        });
    }
    let profile = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&projection);
    let profile_roots = profile.roots();
    ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
        owner,
        launch,
        ProductionExecutionSourceInputV29 {
            semantic_sha256: projection.source_semantic_sha256(),
            roots: &roots,
            classes: &classes,
            events: &events,
        },
        ProductionKernelArgumentAbiInputV18 {
            roots: &profile_roots,
        },
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}

#[test]
fn complete_tile_roots_keep_mixed_rosters_and_one_whole_module() {
    for second_tile in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_multi_tile(second_tile, &mut budget);
        with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let count = if second_tile { 3 } else { 2 };
            assert_eq!(view.original_source(budget)?.root_count(budget)?, count);
            let mut layouts = vec![Some(ExecutionTileLayoutV1::Blocked); count];
            if second_tile {
                layouts[1] = Some(ExecutionTileLayoutV1::Striped);
            }
            layouts[count - 1] = None;
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let inventory = view.output_inventory(budget)?;
            let original = inventory.owner();
            let shared: Vec<_> = inventory.functions().iter().filter(|function| {
                !inventory.kernels().iter().any(|kernel| kernel.entry == function.coordinate)
                    && inventory.kernels().iter().all(|kernel| {
                        inventory.calls().iter().filter(|call| {
                            call.target == Some(function.coordinate)
                                && call.coordinate.block.function == kernel.entry
                        }).count() == 1
                    })
            }).collect();
            assert_eq!(shared.len(), 1, "one actual shared helper with an incoming call from every root must survive the neutral prefix");
            let helper = shared[0];
            let mut first_identity = None;
            for _ in 0..2 {
                let expanded = view.prepare_tile_expansion_roots_v260(&layouts, budget)?;
                assert!(!expanded.grants_artifact_or_launch_authority());
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(budget.storage() - floor, expanded.retained_storage(budget)?);
                assert_eq!(expanded.selections(budget)?.len(), count - 1);
                assert!(std::ptr::eq(expanded.neutral_source_v162(budget)?, view));
                for root in 0..count {
                    let policy = expanded.root_policy_v162(root, budget)?;
                    assert_eq!(policy.is_some(), root != count - 1);
                    if let Some((_, layout, lanes)) = policy {
                        assert_eq!((Some(layout), lanes), (layouts[root], 64));
                    }
                }
                let output = expanded.output(budget)?.module();
                assert_eq!(output.kernels, original.module().kernels);
                assert_eq!(output.functions.len(), original.module().functions.len());
                assert_eq!(output.functions.iter().filter(|function| function.id == helper.function.id).count(), 1);
                assert_eq!(&output.functions[helper.coordinate.0 as usize], helper.function);
                for kernel in &output.kernels {
                    let root = output.functions.iter().find(|function| function.id == kernel.entry).unwrap();
                    let calls = root.body.as_ref().unwrap().blocks.iter().flat_map(|block| &block.operations)
                        .filter(|operation| matches!(&operation.kind, OperationKind::Call { callee, .. } if callee == &helper.function.id)).count();
                    assert_eq!(calls, 1, "the same actual helper remains linked from every root");
                }
                for (index, function) in original.module().functions.iter().enumerate() {
                    if !expanded
                        .selections(budget)?
                        .iter()
                        .any(|row| row.function.0 as usize == index)
                    {
                        assert_eq!(&output.functions[index], function);
                    }
                }
                let identity = *expanded.output(budget)?.identity().digest();
                if let Some(first) = first_identity {
                    assert_eq!(identity, first);
                }
                first_identity = Some(identity);
                expanded.replay(budget)?;
                expanded.discard(budget)?;
                assert_eq!(budget.storage(), floor);
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn complete_tile_roots_refuse_missing_extra_omitted_and_phantom_layouts() {
    for layouts in [
        vec![],
        vec![Some(ExecutionTileLayoutV1::Blocked)],
        vec![Some(ExecutionTileLayoutV1::Blocked), None, None],
        vec![None, None],
        vec![
            Some(ExecutionTileLayoutV1::Blocked),
            Some(ExecutionTileLayoutV1::Blocked),
        ],
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_multi_tile(false, &mut budget);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let floor = budget.storage();
            let work = budget.work();
            let error = match view.prepare_tile_expansion_roots_v260(&layouts, budget) {
                Ok(_) => panic!("invalid complete root roster admitted"),
                Err(error) => error,
            };
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Binding(_)
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > work);
            Err(error)
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn complete_tile_roots_preserve_scalar_roots_without_tile_geometry_or_policy() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_optimized_module_v18(ModuleFixture::Ordinary, &mut budget);
    with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let floor = budget.storage();
        let original = view.output_inventory(budget)?.owner();
        let expanded = view.prepare_tile_expansion_roots_v260(&[None, None], budget)?;
        assert!(expanded.selections(budget)?.is_empty());
        assert!(expanded.root_policy_v162(0, budget)?.is_none());
        assert!(expanded.root_policy_v162(1, budget)?.is_none());
        assert_eq!(expanded.output(budget)?.module(), original.module());
        expanded.replay(budget)?;
        expanded.discard(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn complete_roots_resource_run(
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionSourceOptimizationErrorV18<ProductionSourceOwnedViewErrorV18>>,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = (|| {
        let prepared = prepared_tile_schedule_result_v155(&mut budget)
            .map_err(ProductionSourceOptimizationErrorV18::Source)?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let (output, (), _) =
                source.with_checked_optimization_v18(budget, |_, view, budget| {
                    let expanded = view.prepare_tile_expansion_roots_v260(
                        &[Some(ExecutionTileLayoutV1::Striped)],
                        budget,
                    )?;
                    expanded.root_policy_v162(0, budget)?;
                    let replay = expanded.replay(budget);
                    let settled = expanded.discard(budget);
                    replay?;
                    settled?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                })?;
            drop(output);
            Ok(())
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn complete_tile_root_owner_has_exact_and_one_short_work_storage_boundaries() {
    let (result, work, storage) =
        complete_roots_resource_run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    let (result, used, peak) = complete_roots_resource_run(work, storage);
    result.unwrap();
    assert_eq!((used, peak), (work, storage));
    for (work_limit, storage_limit, work_short) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _) = complete_roots_resource_run(work_limit, storage_limit);
        let error = match result {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Resource(error),
                ),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
            )) => error,
            other => panic!("expected exact resource refusal: {other:?}"),
        };
        assert!(if work_short {
            matches!(error, ArgumentResourceV1::Work(_))
        } else {
            matches!(error, ArgumentResourceV1::Storage(_))
        });
    }
}

#[test]
fn complete_tile_root_owner_preserves_original_account_and_storage_custody() {
    for foreign in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_tile_schedule_v155(&mut budget);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let expanded = view.prepare_tile_expansion_roots_v260(
                &[Some(ExecutionTileLayoutV1::Blocked)],
                budget,
            )?;
            let error = if foreign {
                let mut other_work =
                    CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
                other.reserve_storage(budget.storage())?;
                let floor = other.storage();
                let error = expanded.root_policy_v162(0, &mut other).unwrap_err();
                assert_eq!(other.storage(), floor);
                error
            } else {
                budget.release_storage(1)?;
                expanded.root_policy_v162(0, budget).unwrap_err()
            };
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            assert!(expanded.discard(budget).is_err());
            Err(error)
        });
        assert!(result.is_err());
    }
}

#[test]
fn complete_tile_root_owner_panic_drops_graph_and_policy_before_refund() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_multi_tile(true, &mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        view.test_tile_expansion_panic_after_replay_v159();
        let expanded = view.prepare_tile_expansion_roots_v260(
            &[
                Some(ExecutionTileLayoutV1::Blocked),
                Some(ExecutionTileLayoutV1::Striped),
                None,
            ],
            budget,
        )?;
        expanded.discard(budget)?;
        panic!("constructor panic injection did not run")
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "actual optimizer consumer panicked"
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
