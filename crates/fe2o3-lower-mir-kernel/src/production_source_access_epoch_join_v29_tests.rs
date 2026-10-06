#[test]
fn direct_scalar_loop_accesses_retain_converged_original_activation_sets() {
    for kill in [
        InitializationKillV29::StorageLive,
        InitializationKillV29::StorageDeadLive,
    ] {
        let mut completed = false;
        with_selected_pointer_test_plan_v29(scoped_root_tests::fixtures::initialization_owner(
            InitializationFixtureV29 { looping: true, kill: Some(kill), reinitialize: true, ..config() }),
            |plan, budget| {
                let mut helpers = 0;
                for index in 0..plan.instances.instances().len() {
                    let instance = plan.instances.id_at(index).unwrap();
                    let original = plan.instances.instance(instance).unwrap();
                    if original.function() != SemanticFunctionIdV1::from_index(3) { continue; }
                    helpers += 1;
                    let function = original.declaration();
                    let limit = 1 + function.blocks().iter().map(|block| block.statements().len()).sum::<usize>();
                    let restart = function.blocks()[2].statements().iter().position(|statement|
                        matches!(statement.kind(), SemanticStatementKindV1::StorageLive(local) if local.index() == 2)).unwrap();
                    let atom = 1 + function.blocks()[..2].iter().map(|block| block.statements().len()).sum::<usize>() + restart;
                    for (block, access) in [(2, SourceReferenceAccessV29::Write), (3, SourceReferenceAccessV29::Read)] {
                        let mut rows = plan.accesses.iter().filter(|row| row.key.site == SourceReferenceSiteV29 {
                            instance, block: SemanticBlockIdV1::from_index(block), statement: Some(0),
                        } && row.key.access == access && row.source_local.index() == 2);
                        let row = rows.next().expect("one immutable original occurrence");
                        assert!(rows.next().is_none());
                        assert!(row.loan.is_none() && row.traversed.is_empty() && row.projections.is_empty());
                        let snapshot = plan.blocks.iter().find(|entry|
                            entry.instance == instance && entry.block == row.key.site.block).unwrap();
                        assert_eq!(row.generation, plan.states[snapshot.entry][2].generation,
                            "retained occurrence must agree with the converged original block input");
                        let set = &plan.epoch_sets[row.generation as usize - limit];
                        assert_eq!((set.instance, set.local), (instance, SemanticLocalIdV1::from_index(2)));
                        assert_eq!(&plan.epoch_members[set.first..set.first + set.count], &[0, atom as u32]);
                    }
                    // The gate is shape-only. Exact source/census checks remain
                    // in retain_reference_access and immutable final replay.
                    let SemanticStatementKindV1::Store(store) = function.blocks()[2].statements()[0].kind() else { unreachable!() };
                    let source = store.destination();
                    let site = SourceReferenceSiteV29 { instance, block: SemanticBlockIdV1::from_index(2), statement: Some(0) };
                    for short in [false, true] {
                        let resolved = SourceReferencePlaceV29 {
                            instance, local: source.local(), generation: 0, value: 0,
                            representation_root: 0, node: 0, projections: vec![], selector_source: None,
                            anchor: None, loan: None, shared_path: false, traversed: vec![],
                        };
                        let mut work = CanonicalKernelIrWorkBudgetV1::new(12 - usize::from(short));
                        let mut exact = ArgumentBudgetV1::new(&mut work, 0);
                        let result = source_reference_direct_scalar_epoch_access_v29(plan, site, source,
                            SourceReferenceAccessV29::Write, &resolved, &mut exact);
                        if short {
                            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))));
                            assert_eq!(exact.work(), 0);
                        } else { assert!(result.unwrap()); assert_eq!(exact.work(), 12); }
                        assert_eq!(exact.storage(), 0);
                    }
                    for fault in 0..7 {
                        let mut resolved = SourceReferencePlaceV29 {
                            instance, local: source.local(), generation: 0, value: 0,
                            representation_root: 0, node: 0, projections: vec![], selector_source: None,
                            anchor: None, loan: None, shared_path: false, traversed: vec![],
                        };
                        let mut access = SourceReferenceAccessV29::Write;
                        match fault {
                            0 => access = SourceReferenceAccessV29::Address,
                            1 => access = SourceReferenceAccessV29::ReadDiscriminant,
                            2 => access = SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Shared),
                            3 => resolved.loan = Some(0),
                            4 => resolved.shared_path = true,
                            5 => resolved.traversed.push(0),
                            6 => resolved.local = SemanticLocalIdV1::from_index(1),
                            _ => unreachable!(),
                        }
                        assert!(!source_reference_direct_scalar_epoch_access_v29(plan, site, source, access, &resolved, budget)?);
                    }
                }
                assert_eq!(helpers, 2);
                completed = true;
                Ok(())
            }).unwrap();
        assert!(completed);
    }
}

fn old_scalar_address_across_restart_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let template = scoped_root_tests::fixtures::initialization_owner(InitializationFixtureV29 {
        looping: true,
        kill: Some(InitializationKillV29::StorageLive),
        reinitialize: true,
        address_read: true,
        ..config()
    });
    let semantic = template.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let original = &functions[3];
    let mut blocks = original.blocks().to_vec();
    let address = blocks[3].statements()[0].clone();
    assert!(
        matches!(address.kind(), SemanticStatementKindV1::Assign(assignment)
        if matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { .. }))
    );
    let mut entry = blocks[0].statements().to_vec();
    entry.push(address);
    blocks[0] = block(140, entry, blocks[0].terminator().kind().clone());
    blocks[3] = block(
        143,
        blocks[3].statements()[1..].to_vec(),
        blocks[3].terminator().kind().clone(),
    );
    functions[3] = function(
        130,
        original.role(),
        original.abi().clone(),
        original.locals().to_vec(),
        blocks,
    );
    scoped_root_tests::fixtures::build(
        semantic.types().to_vec(),
        functions,
        semantic.callables().to_vec(),
    )
}

#[test]
fn direct_scalar_epoch_union_does_not_revive_an_old_raw_address_after_restart() {
    let mut visited = false;
    let result = with_selected_pointer_test_plan_v29(
        old_scalar_address_across_restart_owner_v29(),
        |_plan, _budget| {
            visited = true;
            Ok(())
        },
    );
    assert!(!visited);
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source raw pointer outlived its storage activation",
                ..
            })
        ),
        "old original address stays invalid after scalar writeback: {result:?}"
    );
}

thread_local! {
    static RESTART_CONFIG_V29: std::cell::Cell<Option<InitializationFixtureV29>> = const { std::cell::Cell::new(None) };
    static RESTART_COMPLETE_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn observe_restarted_scalar_v29(
    original: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert!(is_stopped(&observe_initialization(
        original, instances, emitted, slots, budget
    )));
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore = Restore(SCOPED_SLOT_OBSERVER_V29.replace(None));
    let owner =
        scoped_root_tests::fixtures::initialization_owner(RESTART_CONFIG_V29.get().unwrap());
    assert_eq!(
        owner.source_semantic_sha256(),
        original.owner.source_semantic_sha256()
    );
    assert_eq!(owner.identity(), original.owner.identity());
    let launch = ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "lifecycle_fixture",
            [88; 32],
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap();
    let floor = budget.storage();
    // Separate complete re-emission of this exact unchanged original. It does
    // not accept a component-observer mutation as an admitted graph.
    let result = (|| -> Result<(), ProductionSourceOwnedViewErrorV18> {
        let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
            owner,
            launch,
            original.input,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )?;
        prepared.with_source_consumer_v18(budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        scoped_raw_admission_v29::with_checked_source_memory_v29(
                            relation,
                            0,
                            None,
                            budget,
                            |_physical, budget| -> Result<(), ProductionSourceOwnedViewErrorV18> {
                                let body = inventory.functions()[source.root(0, budget)?.1]
                                    .function
                                    .body
                                    .as_ref()
                                    .unwrap();
                                let reads: Vec<_> = body
                                    .blocks
                                    .iter()
                                    .flat_map(|block| &block.operations)
                                    .filter(|operation| {
                                        matches!(operation.kind, OperationKind::Load { .. })
                                    })
                                    .collect();
                                assert_eq!(
                                    reads.len(),
                                    2,
                                    "one original final scalar read per helper invocation"
                                );
                                for operation in reads {
                                    let OperationKind::Load { access, .. } = operation.kind else {
                                        unreachable!()
                                    };
                                    assert_eq!(access.address_space, AddressSpace::Private);
                                    assert!(!access.volatile);
                                    assert_eq!(operation.results.len(), 1);
                                    assert_eq!(
                                        operation.results[0].ty,
                                        Type::Scalar(ScalarType::U32)
                                    );
                                }
                                RESTART_COMPLETE_V29.set(true);
                                Ok(())
                            },
                        )
                    })
                })
            })
        })
    })();
    assert_eq!(budget.storage(), floor);
    result.unwrap_or_else(|error| {
        panic!("exact original restart must finish independent memory replay: {error:?}")
    });
    assert!(RESTART_COMPLETE_V29.get());
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn direct_scalar_storage_restart_loops_complete_original_source_and_final_memory_replay() {
    struct Restore(Option<InitializationFixtureV29>, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            RESTART_CONFIG_V29.set(self.0);
            RESTART_COMPLETE_V29.set(self.1);
        }
    }
    let _restore = Restore(RESTART_CONFIG_V29.get(), RESTART_COMPLETE_V29.get());
    for kill in [
        InitializationKillV29::StorageLive,
        InitializationKillV29::StorageDeadLive,
    ] {
        let configuration = InitializationFixtureV29 {
            looping: true,
            kill: Some(kill),
            reinitialize: true,
            ..config()
        };
        RESTART_CONFIG_V29.set(Some(configuration));
        RESTART_COMPLETE_V29.set(false);
        let result = init_run(configuration, observe_restarted_scalar_v29);
        assert!(is_stopped(&result), "{kill:?}: {result:?}");
        assert_eq!(OBSERVED.get(), 1);
        assert!(RESTART_COMPLETE_V29.get());
    }
}
