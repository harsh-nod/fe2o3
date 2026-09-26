fn joined_backing_owner_v29(looping: bool, restart: bool, straight: bool) -> ProductionSemanticSsaOwnerV1 {
    let owner = scoped_root_tests::fixtures::initialization_owner(InitializationFixtureV29 {
        looping, address_read: true,
        kill: restart.then_some(InitializationKillV29::StorageDeadLive),
        reinitialize: restart, ..config()
    });
    if !straight { return owner; }
    assert!(!looping && restart);
    let semantic = owner.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let original = &functions[3];
    let mut blocks = original.blocks().to_vec();
    blocks[0] = block(140, blocks[0].statements().to_vec(), SemanticTerminatorKindV1::Goto(
        SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::Goto, SemanticBlockIdV1::from_index(2))));
    functions[3] = function(130, original.role(), original.abi().clone(), original.locals().to_vec(), blocks);
    scoped_root_tests::fixtures::build(semantic.types().to_vec(), functions, semantic.callables().to_vec())
}

#[test]
fn joined_physical_backing_keeps_complete_logical_generation_census() {
    for (looping, restart, straight) in [(false, false, false), (false, true, false), (true, true, false), (false, true, true)] {
        let mut completed = false;
        with_selected_pointer_test_plan_v29(joined_backing_owner_v29(looping, restart, straight), |plan, budget| {
            let joined = restart && !straight;
            let mut helpers = 0;
            for index in 0..plan.instances.instances().len() {
                let id = plan.instances.id_at(index).unwrap();
                let original = plan.instances.instance(id).unwrap();
                if original.function().index() != 3 { continue; }
                helpers += 1;
                let function = original.declaration();
                let limit = 1 + function.blocks().iter().map(|block| block.statements().len()).sum::<usize>();
                let rows: Vec<_> = plan.cells.rows.iter().enumerate()
                    .filter(|(_, row)| row.instance == id && row.local.index() == 2).collect();
                assert_eq!(rows.len(), if joined { 3 } else if restart { 2 } else { 1 });
                let representative = rows.iter().find(|(_, row)| row.generation == 0).unwrap().0;
                let kind = rows[0].1.kind;
                assert!(matches!(kind, SourceBackingKindV29::Object(_)));
                for &(cell, row) in &rows {
                    assert_eq!(row.kind, kind);
                    let (physical, actual, coalesced) = plan.physical_object_cell(cell, budget)?;
                    assert_eq!(coalesced, joined);
                    assert_eq!(physical, if joined { representative } else { cell });
                    assert_eq!((actual.instance, actual.local, actual.ty, actual.kind),
                        (row.instance, row.local, row.ty, kind));
                    assert_eq!(actual.generation, if joined { 0 } else { row.generation });
                    let mapped = plan.physical_object_generation(id, row.local, row.generation, budget)?;
                    assert_eq!(mapped, joined.then_some((cell, representative, plan.cells.rows[representative])));
                }
                if joined {
                    let logical = rows.iter().find(|(_, row)| row.generation as usize >= limit).unwrap().1;
                    let set = &plan.epoch_sets[logical.generation as usize - limit];
                    let restart = function.blocks()[2].statements().iter().position(|statement|
                        matches!(statement.kind(), SemanticStatementKindV1::StorageLive(local) if local.index() == 2)).unwrap();
                    let atom = 1 + function.blocks()[..2].iter().map(|block| block.statements().len()).sum::<usize>() + restart;
                    assert_eq!((set.instance, set.local), (id, SemanticLocalIdV1::from_index(2)));
                    assert_eq!(&plan.epoch_members[set.first..set.first + set.count], &[0, atom as u32]);
                    assert!(rows.iter().any(|(_, row)| row.generation == atom as u32));
                }
            }
            assert_eq!(helpers, 2);
            assert_eq!(plan.cells.physical_backings.len(), if joined { 6 } else { 0 });
            completed = true;
            Ok(())
        }).unwrap();
        assert!(completed);
    }
}

#[test]
fn joined_physical_lookup_has_independent_work_and_no_scratch_boundary() {
    for mode in 0..3 {
        let mut completed = false;
        let result = with_selected_pointer_test_plan_v29(joined_backing_owner_v29(false, true, false), |plan, budget| {
            let mapping = *plan.cells.physical_backings.last().unwrap();
            let search = plan.cells.physical_backings.len().checked_ilog2().map_or(1, |n| n as usize + 2);
            let expected = 15 + 2 * search;
            let (work, storage) = (budget.work(), budget.storage());
            let positive = plan.physical_object_cell(mapping.cell, budget)?;
            assert!(positive.2);
            assert_eq!(positive.0, mapping.representative);
            assert_eq!((budget.work() - work, budget.storage()), (expected, storage));
            if mode == 0 {
                assert!(matches!(plan.physical_object_cell(usize::MAX, budget),
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "selected backing differs from its original source object or representation", ..
                    })));
                assert_eq!(plan.physical_object_cell(mapping.cell, budget)?, positive);
                completed = true;
                return Ok(());
            }
            if mode == 1 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                let mut foreign = ArgumentBudgetV1::new(&mut work, 0);
                assert!(matches!(plan.physical_object_cell(mapping.cell, &mut foreign),
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
                assert_eq!((foreign.work(), foreign.storage()), (0, 0));
            } else {
                budget.charge_work(20_000_000 - budget.work() - (expected - 1))?;
            }
            let (work, storage) = (budget.work(), budget.storage());
            let error = plan.physical_object_cell(mapping.cell, budget).unwrap_err();
            match (&error, mode) {
                (ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting), 1) =>
                    assert_eq!(budget.work(), work),
                (ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(limit)), 2) => {
                    assert_eq!((limit.actual(), limit.limit()), (20_000_001, 20_000_000));
                    assert_eq!(budget.work() - work, 15 + search);
                }
                _ => panic!("mode {mode}: {error:?}"),
            }
            assert_eq!(budget.storage(), storage);
            let replay_work = budget.work();
            assert_eq!(format!("{:?}", plan.physical_object_cell(mapping.cell, budget).unwrap_err()), format!("{error:?}"));
            assert_eq!((budget.work(), budget.storage()), (replay_work, storage));
            completed = true;
            Err(error)
        });
        assert!(completed, "mode {mode}: {result:?}");
        match mode {
            0 => result.unwrap(),
            1 => assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting)))),
            2 => assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))))),
            _ => unreachable!(),
        }
    }
}

#[test]
fn physical_backing_lookup_equations_are_bounded_and_reject_forged_rows() {
    // Inert lookup equations only: these cannot construct a source plan or
    // authorize a slot. The consuming tests below supply the original owner.
    let row = SourceReferenceScalarCellV29 {
        instance: ProductionCallInstanceIdV1(0), local: SemanticLocalIdV1::from_index(2),
        generation: 0, ty: U32, kind: SourceBackingKindV29::Object(fe2o3_kernel_ir::StorageLayoutIdV1(0)),
    };
    for count in [1usize, 2, 32, 128, 4096] {
        let cells: Vec<_> = (0..count).map(|generation| SourceReferenceScalarCellV29 { generation: generation as u32, ..row }).collect();
        let mappings: Vec<_> = (0..count).map(|cell| SourceReferencePhysicalBackingV29 {
            key: (0, 2, cell as u32), cell, representative: 0,
        }).collect();
        let search = count.ilog2() as usize + 2;
        let expected = 10 + 2 * search;
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(expected - usize::from(short));
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = source_physical_backing_cell_v29(&cells, &mappings, count - 1, &mut budget);
            if short {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))));
                assert_eq!(budget.work(), 10 + search);
            } else {
                assert_eq!(result.unwrap(), (0, row, true));
                assert_eq!(budget.work(), expected);
            }
            assert_eq!(budget.storage(), 0);
        }
    }
    let cells = [row, SourceReferenceScalarCellV29 { generation: 9, ..row }];
    let mappings = [
        SourceReferencePhysicalBackingV29 { key: (0, 2, 0), cell: 0, representative: 0 },
        SourceReferencePhysicalBackingV29 { key: (0, 2, 9), cell: 1, representative: 0 },
    ];
    for fault in 0..10 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert_eq!(source_physical_backing_cell_v29(&cells, &mappings, 1, &mut budget).unwrap(), (0, row, true));
        let mut changed_cells = cells;
        let mut changed = mappings;
        match fault {
            0 => changed[1].cell = 0,
            1 => changed[1].representative = usize::MAX,
            2 => changed_cells[0].instance = ProductionCallInstanceIdV1(1),
            3 => changed_cells[0].local = SemanticLocalIdV1::from_index(3),
            4 => changed_cells[0].ty = UNIT,
            5 => changed_cells[0].kind = SourceBackingKindV29::Object(fe2o3_kernel_ir::StorageLayoutIdV1(1)),
            6 => changed_cells[0].generation = 10,
            7 => changed[0].representative = 1,
            8 => changed[0].cell = 1,
            9 => changed_cells[1].kind = SourceBackingKindV29::Scalar,
            _ => unreachable!(),
        }
        assert!(matches!(source_physical_backing_cell_v29(&changed_cells, &changed, 1, &mut budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "selected backing differs from its original source object or representation", ..
            })), "fault {fault}");
        assert_eq!(budget.storage(), 0);
        assert_eq!(source_physical_backing_cell_v29(&cells, &mappings, 1, &mut budget).unwrap(), (0, row, true));
    }
}

#[test]
fn joined_generation_query_cannot_hide_a_recorded_work_denial() {
    let mut completed = false;
    let result = with_selected_pointer_test_plan_v29(joined_backing_owner_v29(false, true, false), |plan, budget| {
        let mapping = *plan.cells.physical_backings.last().unwrap();
        let logical = plan.cells.rows[mapping.cell];
        assert!(plan.physical_object_generation(logical.instance, logical.local, logical.generation, budget)?.is_some());
        // The query and its source-aware debit each authenticate the owner.
        budget.charge_work(20_000_000 - budget.work() - 10)?;
        let before = budget.storage();
        let error = plan.physical_object_generation(logical.instance, logical.local, logical.generation, budget).unwrap_err();
        let search = plan.cells.physical_backings.len().ilog2() as usize + 2;
        assert!(matches!(&error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(limit))
            if (limit.actual(), limit.limit()) == (20_000_000 + search + 4, 20_000_000)));
        let work = budget.work();
        assert_eq!(format!("{:?}", plan.physical_object_generation(logical.instance, logical.local, logical.generation, budget).unwrap_err()), format!("{error:?}"));
        assert_eq!((budget.work(), budget.storage()), (work, before));
        completed = true;
        Ok(()) // The owning source scope must retain this swallowed resource error.
    });
    assert!(completed);
    assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))));
}

thread_local! {
    static JOINED_BACKING_EXPECTED_ALLOCATIONS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(1) };
    static JOINED_BACKING_OBSERVED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_joined_physical_backing_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    _budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut helpers = 0;
    for (index, lowered) in emitted.iter().enumerate() {
        let id = instances.id_at(index).unwrap();
        if instances.instance(id).unwrap().function().index() != 3 { continue; }
        helpers += 1;
        let lowered = lowered.as_ref().unwrap();
        let candidates: Vec<_> = slots.slots.iter().filter(|slot| slot.instance == id
            && matches!(slot.origin.identity, ScopedAllocationIdentityV29::OriginalObject { local: 2, .. })).collect();
        assert_eq!(candidates.len(), JOINED_BACKING_EXPECTED_ALLOCATIONS_V29.get());
        assert!(candidates.iter().all(|slot| matches!(slot.representation, ScopedSlotRepresentationV29::Object { .. })));
        let mut reads = 0;
        let mut stores = 0;
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        for anchor in &anchors.rows {
            let ScopedMemoryAnchorKindV29::Object(payload) = anchor.kind else { continue; };
            let payload = anchors.objects[payload];
            match (payload.operation, payload.role) {
                (ScopedObjectOperationV29::WriteValue { address, .. }, ScopedObjectRoleV29::WriteValue { destination, .. })
                    if matches!(destination.object, ScopedObjectIdentityV29::Local { instance, local, .. }
                        if instance == id && local.index() == 2) => {
                    assert!(candidates.iter().any(|slot| slot.origin.pointer == address));
                    stores += 1;
                }
                (ScopedObjectOperationV29::ReadValue { address, .. }, ScopedObjectRoleV29::ReadValue {
                    source, read: ScopedObjectReadOriginV29::Original(read),
                }) if read.site == (ExecutionSiteV29::Statement { block: SsaBlockIdV1::new(3), statement: 1 }) => {
                    assert_eq!(read.role, ExecutionOperandV29::RvaluePlace);
                    assert_eq!(read.prefix, 1);
                    assert!(matches!(source.object, ScopedObjectIdentityV29::Reference { .. }));
                    assert!(candidates.iter().all(|slot| slot.origin.pointer != address), "the actual fresh alias must remain distinct");
                    let actual = lowered.function.body.as_ref().unwrap().blocks.iter()
                        .find(|block| block.id == anchor.block).unwrap().operations.get(anchor.position).unwrap();
                    assert!(matches!(actual.kind, OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address: actual, .. }) if actual == address));
                    reads += 1;
                }
                _ => {},
            }
        }
        assert!(stores >= 1);
        assert_eq!(reads, 1);
    }
    assert_eq!(helpers, 2);
    JOINED_BACKING_OBSERVED_V29.set(JOINED_BACKING_OBSERVED_V29.get() + 1);
    OBSERVED.set(OBSERVED.get() + 1);
    Ok(())
}

#[test]
fn original_joined_storage_restarts_complete_fresh_alias_same_candidate_memory() {
    struct Restore(usize);
    impl Drop for Restore { fn drop(&mut self) { JOINED_BACKING_EXPECTED_ALLOCATIONS_V29.set(self.0); } }
    let _restore = Restore(JOINED_BACKING_EXPECTED_ALLOCATIONS_V29.get());
    for (looping, restart, straight) in [(false, false, false), (false, true, false), (true, true, false), (false, true, true)] {
        JOINED_BACKING_EXPECTED_ALLOCATIONS_V29.set(if straight { 2 } else { 1 });
        JOINED_BACKING_OBSERVED_V29.set(0);
        let (result, _, _, completed) = run_original_repeated_source_v29(
            || joined_backing_owner_v29(looping, restart, straight), observe_joined_physical_backing_v29,
            10_000_000, 10_000_000);
        result.unwrap();
        assert!(completed);
        assert_eq!((JOINED_BACKING_OBSERVED_V29.get(), OBSERVED.get()), (3, 3));
    }
}

#[test]
fn joined_physical_complete_source_keeps_exact_boundary_cleanup() {
    struct Restore(usize);
    impl Drop for Restore { fn drop(&mut self) { JOINED_BACKING_EXPECTED_ALLOCATIONS_V29.set(self.0); } }
    let _restore = Restore(JOINED_BACKING_EXPECTED_ALLOCATIONS_V29.replace(1));
    let factory = || joined_backing_owner_v29(true, true, false);
    let (positive, work, storage, completed) = run_original_repeated_source_v29(
        factory, observe_joined_physical_backing_v29, 10_000_000, 10_000_000);
    positive.unwrap();
    assert!(completed);
    // Whole-owner boundary executions use the observed complete transaction;
    // the independent fixed lookup debit oracle is tested above.
    for (work_limit, storage_limit, short) in [(work, storage, 0), (work - 1, storage, 1), (work, storage - 1, 2)] {
        let (result, _, _, completed) = run_original_repeated_source_v29(
            factory, observe_joined_physical_backing_v29, work_limit, storage_limit);
        if short == 0 { result.unwrap(); assert!(completed); }
        else {
            assert!(!completed);
            match (short, original_repeated_source_resource_v29(result.unwrap_err())) {
                (1, ArgumentResourceV1::Work(_)) | (2, ArgumentResourceV1::Storage(_)) => {},
                other => panic!("exact complete-owner resource refusal: {other:?}"),
            }
        }
    }
}

fn two_births_across_restart_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let owner = old_scalar_address_across_restart_owner_v29();
    let semantic = owner.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let original = &functions[3];
    let mut blocks = original.blocks().to_vec();
    let address = blocks[0].statements().last().unwrap().clone();
    assert!(matches!(address.kind(), SemanticStatementKindV1::Assign(assignment)
        if matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { .. })));
    let mut changed = blocks[2].statements().to_vec();
    // This original fixture restarts directly with StorageLive, without a
    // preceding StorageDead. Both births must precede that actual restart.
    assert!(!changed.iter().any(|statement| matches!(statement.kind(), SemanticStatementKindV1::StorageDead(_))));
    let restart = changed.iter().position(|statement| matches!(statement.kind(), SemanticStatementKindV1::StorageLive(local) if local.index() == 2)).unwrap();
    changed.insert(restart, address);
    blocks[2] = block(142, changed, blocks[2].terminator().kind().clone());
    functions[3] = function(130, original.role(), original.abi().clone(), original.locals().to_vec(), blocks);
    scoped_root_tests::fixtures::build(semantic.types().to_vec(), functions, semantic.callables().to_vec())
}

#[test]
fn joined_physical_backing_never_revives_an_old_or_loop_carried_original_birth() {
    for owner in [old_scalar_address_across_restart_owner_v29(), two_births_across_restart_owner_v29()] {
        let mut entered = false;
        let result = with_selected_pointer_test_plan_v29(owner, |_, _| { entered = true; Ok(()) });
        assert!(!entered);
        assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source raw pointer outlived its storage activation", ..
        })), "logical epoch joins never refresh an original pointer: {result:?}");
    }
}
