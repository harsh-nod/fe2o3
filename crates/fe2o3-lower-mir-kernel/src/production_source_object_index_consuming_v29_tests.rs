thread_local! {
    static OBJECT_INDEX_FAULT_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OBJECT_INDEX_MUTATIONS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OBJECT_INDEX_GUARD_READS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn object_index_original_owner_v29(branches: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = mixed_selected_array_object_owner_v29(branches);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    for (index, tag, entry) in [(1, 100, 90), (2, 110, 115)] {
        let helper = &functions[index];
        let mut locals = helper.locals().to_vec();
        let raw = locals[locals.len() - 2].ty();
        let pointer = locals.len() as u32;
        locals.push(local(145, raw, SemanticLocalRoleV1::Temporary));
        let mut statements = helper.blocks()[0].statements().to_vec();
        let SemanticStatementKindV1::Store(store) = statements[2].kind() else {
            panic!("original retained index Store");
        };
        let index_place = store.destination().clone();
        assert!(index_place.projections().is_empty());
        statements.insert(
            2,
            assign(
                place(pointer, raw),
                SemanticRvalueKindV1::AddressOf {
                    place: index_place,
                    mutability: SemanticMutabilityV1::Mutable,
                },
            ),
        );
        let mut blocks = helper.blocks().to_vec();
        blocks[0] = block(entry, statements, blocks[0].terminator().kind().clone());
        functions[index] = function(tag, helper.role(), helper.abi().clone(), locals, blocks);
    }
    super::super::fixtures::build(
        semantic.types().to_vec(),
        functions,
        semantic.callables().to_vec(),
    )
}

fn with_original_object_index_plan_v29(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    const MODULE_LIMIT: usize = 20_000_000;
    const MODULE_FLOOR: usize = 23;
    let mut owner = factory();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR)?;
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
    budget.reserve_storage(capture.retained_storage())?;
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let credit = layouts.capture_emission_credit(&owner, &mut budget)?;
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &mut budget,
        |instances, budget| {
            let lens = demands.root_lens(&owner, 0, budget).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                with_source_reference_descriptor_demands_scope_v29(
                    instances,
                    SourceReferenceStorageV29::ScalarCells,
                    Some(&mut layouts),
                    None,
                    Some(lens),
                    budget,
                    |plan, _, budget| consume(plan, budget).map_err(Into::into),
                ),
            )
        },
    )
    .unwrap();
    let scratch = credit.into_root_credit(&layouts, &budget).unwrap();
    assert!(layouts.permits_root_emission_refund(&owner, scratch, &budget));
    budget.release_storage(scratch)?;
    let cleanup = layouts.release(&mut budget);
    let demand_cleanup = demands.discard(&mut budget);
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    assert_eq!(budget.storage(), MODULE_FLOOR);
    result.and(cleanup).and(demand_cleanup)
}

fn object_index_read_only_join_owner_v29(looping: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = object_index_original_owner_v29(false);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    assert_eq!(helper.blocks().len(), 1);
    let mut before = helper.blocks()[0].statements().to_vec();
    let selected = before.iter().position(|statement| matches!(statement.kind(),
        SemanticStatementKindV1::Assign(assignment) if matches!(assignment.destination().projections(),
            [projection] if matches!(projection.kind(), SemanticProjectionKindV1::Index(_))))).unwrap();
    let SemanticStatementKindV1::Assign(assignment) = before[selected].kind() else {
        unreachable!()
    };
    let SemanticProjectionKindV1::Index(index) = assignment.destination().projections()[0].kind()
    else {
        unreachable!()
    };
    let after = before.split_off(selected);
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let zero = SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
    ));
    let mut blocks = vec![
        block(
            115,
            before,
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(index.index(), U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            148,
            vec![
                SemanticStatementV1::new(source(), SemanticStatementKindV1::StorageDead(index)),
                SemanticStatementV1::new(source(), SemanticStatementKindV1::StorageLive(index)),
                assign(place(index.index(), U32), SemanticRvalueKindV1::Use(zero)),
            ],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            149,
            vec![],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            150,
            after,
            if looping {
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(index.index(), U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 4),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                    )
                    .unwrap(),
                }
            } else {
                helper.blocks()[0].terminator().kind().clone()
            },
        ),
    ];
    if looping {
        blocks.push(block(
            151,
            vec![],
            helper.blocks()[0].terminator().kind().clone(),
        ));
    }
    functions[2] = function(
        110,
        helper.role(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        blocks,
    );
    super::super::fixtures::build(
        semantic.types().to_vec(),
        functions,
        semantic.callables().to_vec(),
    )
}

#[test]
fn original_selector_only_generation_joins_use_existing_physical_backing_and_currentness() {
    OBJECT_INDEX_FAULT_V29.set(0);
    let (positive, _, _, completed) = run_object_indices_v29(false, 10_000_000, 10_000_000);
    assert!(positive.is_ok() && completed, "{positive:?}");
    for looping in [false, true] {
        let factory: fn() -> ProductionSemanticSsaOwnerV1 = if looping {
            || object_index_read_only_join_owner_v29(true)
        } else {
            || object_index_read_only_join_owner_v29(false)
        };
        let mut observed = false;
        with_original_object_index_plan_v29(factory, |plan, budget| {
            let mut joins = 0;
            for selector in &plan.selectors {
                let instance = plan.instances.instance(selector.instance).unwrap();
                if instance.function().index() != 2 {
                    continue;
                }
                let SourceReferenceSelectorValueV29::Retained { event } = selector.value else {
                    panic!("retained index");
                };
                let occurrences = plan.instances.occurrences(selector.instance).unwrap();
                let occurrence = &occurrences.events()[event];
                let read = ScopedMemoryIndexReadV29 {
                    site: occurrence.site(),
                    role: occurrence.operand(),
                    event,
                    projection: selector.projection as u32,
                    local: selector.local,
                    ty: U32,
                };
                let generation =
                    source_retained_index_generation_v29(plan, selector.instance, read, budget)?;
                let cells: Vec<_> = plan
                    .cells
                    .rows
                    .iter()
                    .filter(|cell| {
                        cell.instance == selector.instance && cell.local == selector.local
                    })
                    .collect();
                assert_eq!(
                    cells.len(),
                    3,
                    "two original atomic activations and the selector-only union"
                );
                assert_eq!(
                    cells
                        .iter()
                        .filter(|cell| cell.generation == generation)
                        .count(),
                    1
                );
                let (logical, representative, physical) = plan
                    .physical_object_generation(
                        selector.instance,
                        selector.local,
                        generation,
                        budget,
                    )?
                    .expect("same original family");
                assert_eq!(plan.cells.rows[logical].generation, generation);
                assert_eq!(plan.cells.rows[representative], physical);
                assert_eq!(physical.generation, 0);
                assert_ne!(logical, representative);
                joins += 1;
            }
            assert_eq!(joins, 1);
            observed = true;
            Ok(())
        })
        .unwrap_or_else(|error| {
            panic!("original selector generation join looping={looping}: {error:?}")
        });
        assert!(
            observed,
            "original selector generation join looping={looping}"
        );
        let (result, work, peak, completed) = run_original_source_fixture_v29(
            factory,
            false,
            false,
            1,
            inspect_joined_original_object_indices_v29,
            10_000_000,
            10_000_000,
        );
        assert!(result.is_ok() && completed, "looping={looping}: {result:?}");
        assert_eq!(OBSERVED.get(), 3);
        let (exact, _, _, completed) = run_original_source_fixture_v29(
            factory,
            false,
            false,
            1,
            inspect_joined_original_object_indices_v29,
            work,
            peak,
        );
        assert!(exact.is_ok() && completed, "{exact:?}");
        assert_eq!(OBSERVED.get(), 3);
        for (work_limit, storage_limit, storage_fault) in
            [(work - 1, peak, false), (work, peak - 1, true)]
        {
            let (refused, _, _, completed) = run_original_source_fixture_v29(
                factory,
                false,
                false,
                1,
                inspect_joined_original_object_indices_v29,
                work_limit,
                storage_limit,
            );
            assert!(!completed);
            match original_repeated_source_resource_v29(refused.unwrap_err()) {
                ArgumentResourceV1::Work(error) if !storage_fault => {
                    assert_eq!(error.limit(), work_limit);
                    assert!(error.actual() > work_limit);
                }
                ArgumentResourceV1::Storage(error) if storage_fault => {
                    assert_eq!(error.limit(), storage_limit);
                    assert!(error.actual() > storage_limit);
                }
                error => panic!("joined holder resource refusal: {error:?}"),
            }
        }
        let (positive, _, _, completed) = run_original_source_fixture_v29(
            factory,
            false,
            false,
            1,
            inspect_joined_original_object_indices_v29,
            10_000_000,
            10_000_000,
        );
        assert!(positive.is_ok() && completed, "{positive:?}");
        OBJECT_INDEX_FAULT_V29.set(4);
        OBJECT_INDEX_MUTATIONS_V29.set(0);
        let (refused, _, _, completed) = run_original_source_fixture_v29(
            factory,
            false,
            false,
            1,
            inspect_joined_original_object_indices_v29,
            10_000_000,
            10_000_000,
        );
        OBJECT_INDEX_FAULT_V29.set(0);
        assert!(!completed);
        assert!(OBSERVED.get() > 0);
        assert_eq!(
            OBJECT_INDEX_MUTATIONS_V29.get(),
            OBSERVED.get(),
            "each completed source pass substitutes only the joined helper's exact logical generation"
        );
        assert!(
            matches!(
                refused,
                Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            function: 0,
                            detail: "source raw address differs from its actual formation or memory history",
                            ..
                        }
                    )
                ))
            ),
            "{refused:?}"
        );
    }
}

#[test]
fn original_object_index_generation_query_rejoins_its_original_use() {
    let mut completed = false;
    with_original_object_index_plan_v29(|| object_index_original_owner_v29(false), |plan, budget| {
        let mut checked = 0;
        for selector in &plan.selectors {
            let SourceReferenceSelectorValueV29::Retained { event } = selector.value else { continue; };
            let instance = plan.instances.instance(selector.instance).unwrap();
            let occurrences = plan.instances.occurrences(selector.instance).unwrap();
            let occurrence = &occurrences.events()[event];
            let read = ScopedMemoryIndexReadV29 {
                site: occurrence.site(), role: occurrence.operand(), event,
                projection: selector.projection as u32, local: selector.local,
                ty: instance.declaration().locals()[selector.local.index() as usize].ty(),
            };
            let cells: Vec<_> = plan.cells.rows.iter().filter(|cell|
                cell.instance == selector.instance && cell.local == selector.local).collect();
            assert_eq!(cells.len(), 1, "supported fixture has one original scalar activation");
            assert!(matches!(cells[0].kind, SourceBackingKindV29::Object(_)));
            assert_eq!(cells[0].generation, 0);
            let floor = budget.storage();
            with_canonical_call_scratch_v1(budget, |budget| {
                let before = (budget.work(), budget.storage());
                assert_eq!(source_retained_index_generation_v29(plan, selector.instance, read, budget)?, cells[0].generation);
                // Four original-owner checks, occurrence 14, selector 12,
                // final equality 4, and the existing bounded map lookup.
                let work = 50 + 16 * (plan.selector_sites.len().checked_ilog2().unwrap_or(0) as usize + 2);
                let headers = std::mem::size_of::<u32>()
                    + 2 * std::mem::size_of::<Result<u32, ProductionSemanticKirErrorV1>>()
                    + std::mem::size_of::<Option<(usize, SourceReferenceSelectorV29)>>()
                    + 2 * std::mem::size_of::<Result<Option<(usize, SourceReferenceSelectorV29)>, ProductionSemanticKirErrorV1>>();
                assert_eq!(budget.work() - before.0, work);
                assert_eq!(budget.storage() - before.1, headers);
                for fault in 0..7 {
                    let mut changed = read;
                    let mut owner = selector.instance;
                    match fault {
                        0 => changed.event = usize::MAX,
                        1 => changed.site = execution_site_v29(SemanticBlockIdV1::from_index(0), Some(0)),
                        2 => changed.role = ExecutionOperandV29::RvalueOperand(0),
                        3 => changed.projection = u32::MAX,
                        4 => changed.local = SemanticLocalIdV1::from_index(u32::MAX),
                        5 => changed.ty = selector.array,
                        6 => owner = plan.instances.root(),
                        _ => unreachable!(),
                    }
                    let error = source_retained_index_generation_v29(plan, owner, changed, budget).unwrap_err();
                    assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
                        function: 0, detail: "scoped memory anchors differ from their source instance", ..
                    }), "fault {fault}: {error:?}");
                }
                Ok(())
            })?;
            assert_eq!(budget.storage(), floor);
            checked += 1;
        }
        assert_eq!(checked, 2, "one implicit index read in each original helper");
        completed = true;
        Ok(())
    }).unwrap();
    assert!(completed);
}

fn inspect_original_object_indices_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    inspect_original_object_index_shape_v29(false, source, instances, emitted, slots, budget)
}

fn inspect_joined_original_object_indices_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    inspect_original_object_index_shape_v29(true, source, instances, emitted, slots, budget)
}

fn inspect_original_object_index_shape_v29(
    joined: bool,
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    if joined {
        for (ordinal, instance) in instances.instances().iter().enumerate() {
            if instance.function().index() != 2 || instance.declaration().blocks().len() != 5 {
                continue;
            }
            let lowered = emitted[ordinal].as_ref().unwrap();
            let mapped = |source| {
                let rows: Vec<_> = lowered
                    .blocks
                    .iter()
                    .filter(|row| {
                        row.semantic_function == instance.function()
                            && row.semantic_block.index() == source
                    })
                    .collect();
                assert_eq!(rows.len(), 1);
                rows[0].kernel_ir_block
            };
            let restart = mapped(1);
            let use_block = mapped(3);
            let body = lowered.function.body.as_ref().unwrap();
            let successors = |id| {
                body.blocks
                    .iter()
                    .find(|block| block.id == id)
                    .unwrap()
                    .terminator
                    .as_ref()
                    .unwrap()
                    .successors()
            };
            assert_eq!(successors(restart), vec![use_block]);
            assert!(
                successors(use_block).contains(&restart),
                "actual backedge survives original lowering"
            );
        }
    }
    let mut counts = [0; 3];
    let mut changed = 0;
    for slot in &slots.slots {
        let instance = instances.instance(slot.instance).unwrap();
        assert!(matches!(instance.function().index(), 1 | 2));
        let lowered = emitted[slot.instance.index()].as_mut().unwrap();
        if matches!(
            slot.origin.source,
            ScopedAllocationSourceV29::OriginalArray { .. }
        ) {
            assert!(matches!(
                slot.representation,
                ScopedSlotRepresentationV29::ScalarArray(_)
            ));
            counts[0] += 1;
            continue;
        }
        let ScopedAllocationIdentityV29::OriginalObject { local, generation } =
            slot.origin.identity
        else {
            panic!("both retained scalar holders must use original typed object backing");
        };
        let ScopedSlotRepresentationV29::Object {
            schema,
            bytes: 4,
            alignment: 4,
        } = slot.representation
        else {
            panic!("whole scalar Object");
        };
        let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
        let mut index_reads = 0;
        let occurrences = instances.occurrences(slot.instance).unwrap();
        for payload in &mut anchors.objects {
            let ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::ProjectionIndex(read),
            } = &mut payload.role
            else {
                continue;
            };
            if read.local.index() != local {
                continue;
            }
            check_scoped_index_read_v29(instance.declaration(), &occurrences, *read, budget)?;
            let expected_generation = if joined && instance.function().index() == 2 {
                let ScopedObjectIdentityV29::Local {
                    instance: owner,
                    local,
                    generation: logical,
                } = source.object
                else {
                    panic!("original logical index holder");
                };
                assert_eq!((owner, local), (slot.instance, read.local));
                assert_ne!(
                    logical, generation,
                    "the read uses the original joined epoch, not physical representative"
                );
                logical
            } else {
                generation
            };
            assert_eq!(
                source.object,
                ScopedObjectIdentityV29::Local {
                    instance: slot.instance,
                    local: read.local,
                    generation: expected_generation,
                }
            );
            assert_eq!(source.source, ScopedObjectSourceV29::ProjectionIndex(*read));
            assert_eq!(
                (source.root_schema, source.projected_schema),
                (schema, schema)
            );
            assert_eq!((source.source_path.count, source.path.count), (0, 0));
            assert!(
                matches!(payload.operation, ScopedObjectOperationV29::ReadValue { address, access }
                if address == slot.origin.pointer && !access.volatile && access.alignment == 4)
            );
            assert!(payload.result.is_some());
            index_reads += 1;
            if OBJECT_INDEX_FAULT_V29.get() != 0
                && (OBJECT_INDEX_FAULT_V29.get() != 4 || instance.function().index() == 2)
            {
                match OBJECT_INDEX_FAULT_V29.get() {
                    1 => {
                        // Change both inert roles, never the immutable event.
                        read.event = usize::MAX;
                        source.source = ScopedObjectSourceV29::ProjectionIndex(*read);
                    }
                    2 => {
                        source.object = ScopedObjectIdentityV29::Local {
                            instance: slot.instance,
                            local: read.local,
                            generation: u32::MAX,
                        }
                    }
                    3 => {
                        source.object = ScopedObjectIdentityV29::Local {
                            instance: ProductionCallInstanceIdV1(usize::MAX),
                            local: read.local,
                            generation,
                        }
                    }
                    4 => {
                        source.object = ScopedObjectIdentityV29::Local {
                            instance: slot.instance,
                            local: read.local,
                            generation,
                        }
                    }
                    _ => unreachable!(),
                }
                changed += 1;
            }
        }
        if index_reads == 0 {
            counts[1] += 1;
        } else {
            assert_eq!(
                index_reads, 1,
                "one original selected array write per helper"
            );
            counts[2] += 1;
        }
    }
    assert_eq!(
        counts,
        [2, 2, 2],
        "array, disjoint Object, typed index Object per helper"
    );
    if OBJECT_INDEX_FAULT_V29.get() != 0 {
        assert_eq!(
            changed,
            if OBJECT_INDEX_FAULT_V29.get() == 4 {
                1
            } else {
                2
            }
        );
    }
    OBJECT_INDEX_MUTATIONS_V29.set(OBJECT_INDEX_MUTATIONS_V29.get() + changed);
    Ok(())
}

fn run_object_indices_v29(
    branches: bool,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    run_original_source_fixture_v29(
        || object_index_original_owner_v29(branches),
        branches,
        false,
        1,
        inspect_original_object_indices_v29,
        work,
        storage,
    )
}

fn object_index_guard_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let original = object_index_original_owner_v29(false);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let boolean = if let Some(index) = types.iter().position(|row| {
        matches!(
            row.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
        )
    }) {
        SemanticTypeIdV1::from_index(index as u32)
    } else {
        declaration(
            &mut types,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(1),
                1,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 8, 1),
                    SemanticScalarValidityRangeV1::new(0, 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
            None,
        )
    };
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    assert_eq!(helper.blocks().len(), 1);
    let mut locals = helper.locals().to_vec();
    let condition = locals.len() as u32;
    locals.push(local(146, boolean, SemanticLocalRoleV1::Temporary));
    let mut before = helper.blocks()[0].statements().to_vec();
    let selected = before
        .iter()
        .position(|statement| {
            matches!(statement.kind(),
        SemanticStatementKindV1::Assign(assign) if matches!(assign.destination().projections(),
            [projection] if matches!(projection.kind(), SemanticProjectionKindV1::Index(_))))
        })
        .unwrap();
    let SemanticStatementKindV1::Assign(assignment) = before[selected].kind() else {
        unreachable!()
    };
    let SemanticProjectionKindV1::Index(index) = assignment.destination().projections()[0].kind()
    else {
        unreachable!()
    };
    let index = place(index.index(), U32);
    let after = before.split_off(selected);
    let length = SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 4).unwrap()),
    ));
    before.push(assign(
        place(condition, boolean),
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: SemanticOperandV1::Copy(index.clone()),
            right: length.clone(),
        },
    ));
    functions[2] = function(
        110,
        helper.role(),
        helper.abi().clone(),
        locals,
        vec![
            block(
                115,
                before,
                SemanticTerminatorKindV1::Assert {
                    condition: SemanticOperandV1::Copy(place(condition, boolean)),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length,
                        index: SemanticOperandV1::Copy(index),
                    },
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        SemanticBlockIdV1::from_index(1),
                    ),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(147, after, helper.blocks()[0].terminator().kind().clone()),
        ],
    );
    super::super::fixtures::build(types, functions, semantic.callables().to_vec())
}

fn inspect_object_index_guard_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    inspect_original_object_indices_v29(source, instances, emitted, slots, budget)?;
    let mut found = 0;
    for (ordinal, instance) in instances.instances().iter().enumerate() {
        let declaration = instance.declaration();
        if instance.function().index() != 2 {
            continue;
        }
        let SemanticTerminatorKindV1::Assert {
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck { .. },
            ..
        } = declaration.blocks()[0].terminator().kind()
        else {
            panic!("original bounds guard");
        };
        let comparison = declaration.blocks()[0].statements().len() - 1;
        let SemanticStatementKindV1::Assign(original) =
            declaration.blocks()[0].statements()[comparison].kind()
        else {
            panic!("comparison");
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: SemanticOperandV1::Copy(place),
            ..
        } = original.value().kind()
        else {
            panic!("original guarded index read");
        };
        let lowered = emitted[ordinal].as_ref().unwrap();
        for payload in &lowered.scoped_memory_anchors.as_ref().unwrap().objects {
            let ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::Original(read),
            } = payload.role
            else {
                continue;
            };
            if read.site
                != (ExecutionSiteV29::Statement {
                    block: SsaBlockIdV1::new(0),
                    statement: comparison as u32,
                })
                || read.role != ExecutionOperandV29::RvalueOperand(0)
            {
                continue;
            }
            assert_eq!(read.prefix, 0);
            assert!(matches!(
                read.occurrence,
                ScopedMemoryOccurrenceV29::Retained { .. }
            ));
            assert!(
                matches!(source.source, ScopedObjectSourceV29::Place { local, .. } if local == place.local())
            );
            assert!(matches!(
                payload.operation,
                ScopedObjectOperationV29::ReadValue { .. }
            ));
            found += 1;
        }
    }
    assert_eq!(found, 1);
    OBJECT_INDEX_GUARD_READS_V29.set(OBJECT_INDEX_GUARD_READS_V29.get() + found);
    Ok(())
}

#[test]
fn original_typed_guard_comparison_remains_pending_after_unrelated_object_support() {
    OBJECT_INDEX_FAULT_V29.set(0);
    let (positive, _, _, completed) = run_object_indices_v29(false, 10_000_000, 10_000_000);
    assert!(positive.is_ok() && completed, "{positive:?}");
    OBJECT_INDEX_GUARD_READS_V29.set(0);
    let (refused, _, _, completed) = run_original_source_fixture_v29(
        object_index_guard_owner_v29,
        false,
        false,
        1,
        inspect_object_index_guard_v29,
        10_000_000,
        10_000_000,
    );
    assert!(!completed);
    assert!(OBSERVED.get() > 0);
    assert_eq!(
        OBJECT_INDEX_GUARD_READS_V29.get(),
        OBSERVED.get(),
        "one exact typed comparison per completed original-source pass"
    );
    assert!(
        matches!(
            refused,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        detail: "typed object effects require actual same-candidate source and physical currentness",
                        ..
                    }
                )
            ))
        ),
        "{refused:?}"
    );
}

#[test]
fn original_object_index_holders_reach_same_candidate_memory_versions_and_history() {
    OBJECT_INDEX_FAULT_V29.set(0);
    for branches in [false, true] {
        let (result, work, storage, completed) =
            run_object_indices_v29(branches, 10_000_000, 10_000_000);
        assert!(
            result.is_ok() && completed,
            "branches={branches}: {result:?}"
        );
        assert_eq!(OBSERVED.get(), 3);
        let (exact, _, _, completed) = run_object_indices_v29(branches, work, storage);
        assert!(exact.is_ok() && completed, "{exact:?}");
        for (limit, bytes, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
            let (refused, _, _, completed) = run_object_indices_v29(branches, limit, bytes);
            assert!(!completed);
            let error = original_repeated_source_resource_v29(refused.unwrap_err());
            match (is_work, error) {
                (true, ArgumentResourceV1::Work(error)) => {
                    assert_eq!(error.limit(), limit);
                    assert!(error.actual() > limit, "{error:?}");
                }
                (false, ArgumentResourceV1::Storage(error)) => {
                    assert_eq!(error.limit(), bytes);
                    assert!(error.actual() > bytes, "{error:?}");
                }
                (_, error) => panic!("wrong whole-transaction resource refusal: {error:?}"),
            }
        }
    }
}

#[test]
fn original_object_index_roles_cannot_replace_the_implicit_original_use() {
    for fault in 1..=3 {
        OBJECT_INDEX_FAULT_V29.set(0);
        let (positive, _, _, completed) = run_object_indices_v29(false, 10_000_000, 10_000_000);
        assert!(positive.is_ok() && completed, "{positive:?}");
        OBJECT_INDEX_FAULT_V29.set(fault);
        OBJECT_INDEX_MUTATIONS_V29.set(0);
        let (refused, _, _, completed) = run_object_indices_v29(false, 10_000_000, 10_000_000);
        OBJECT_INDEX_FAULT_V29.set(0);
        assert!(!completed);
        assert!(OBSERVED.get() > 0);
        assert_eq!(
            OBJECT_INDEX_MUTATIONS_V29.get(),
            2 * OBSERVED.get(),
            "every completed source pass mutates both exact original helper occurrences"
        );
        let expected = match fault {
            1 => "scoped memory anchors differ from their source instance",
            2 => "source raw address differs from its actual formation or memory history",
            3 => "typed object source payload differs from its actual operation",
            _ => unreachable!(),
        };
        assert!(
            matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
                detail, ..
            }))) if detail == expected),
            "fault={fault}: {refused:?}"
        );
    }
}

thread_local! {
    static OBJECT_INDEX_CENSUS_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static OBJECT_INDEX_CENSUS_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn alter_original_object_index_census_v29(
    source_index: &SourceAddressSourceIndexV29<'_>,
    rows: &mut Vec<SourceAddressAccessSourceV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<[Option<usize>; 2]>(budget)?;
    source_reference_emission_prepay_v29::<[SourceAddressAccessSourceV29; 2]>(budget)?;
    let mut selected = [None; 2];
    let mut count = 0;
    for (index, source) in rows.iter().enumerate() {
        budget.charge_work(2)?;
        let sidecar = source_index.sidecar(source.instance, budget)?;
        let anchors = sidecar.scoped_memory_anchors.as_ref().unwrap();
        let row = &anchors.rows[source.anchor];
        if let Some((endpoint, _)) = source_address_object_payload_v29(anchors, row, budget)?
            && matches!(endpoint.source, ScopedObjectSourceV29::ProjectionIndex(_))
        {
            assert!(count < selected.len());
            selected[count] = Some(index);
            count += 1;
        }
    }
    assert_eq!(count, 2, "two genuine original helper selector reads");
    let left = selected[0].unwrap();
    let right = selected[1].unwrap();
    assert_ne!(rows[left].instance, rows[right].instance);
    assert_ne!(rows[left].physical.slot, rows[right].physical.slot);
    budget.charge_work(6)?;
    match OBJECT_INDEX_CENSUS_FAULT_V29.get() {
        1 => {
            budget.charge_work(rows.len())?;
            rows.remove(left);
        }
        2 => {
            assert!(rows.len() < rows.capacity());
            rows.push(rows[left]);
        }
        3 => {
            let a = rows[left];
            let b = rows[right];
            rows[left].instance = b.instance;
            rows[left].anchor = b.anchor;
            rows[right].instance = a.instance;
            rows[right].anchor = a.anchor;
        }
        _ => panic!("unknown selector census fault"),
    }
    OBJECT_INDEX_CENSUS_VISITS_V29.set(OBJECT_INDEX_CENSUS_VISITS_V29.get() + 1);
    Ok(())
}

#[test]
fn original_object_index_census_requires_one_exact_read_for_each_typed_selector() {
    struct Restore(Option<SourceObjectEffectCensusObserverV29>, u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29.set(self.0);
            OBJECT_INDEX_CENSUS_FAULT_V29.set(self.1);
        }
    }
    for branches in [false, true] {
        for fault in 1..=3 {
            OBJECT_INDEX_FAULT_V29.set(0);
            let (positive, _, _, completed) =
                run_object_indices_v29(branches, 10_000_000, 10_000_000);
            assert!(positive.is_ok() && completed, "{positive:?}");
            let restore = Restore(
                SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29
                    .replace(Some(alter_original_object_index_census_v29)),
                OBJECT_INDEX_CENSUS_FAULT_V29.replace(fault),
            );
            OBJECT_INDEX_CENSUS_VISITS_V29.set(0);
            let (refused, _, _, completed) =
                run_object_indices_v29(branches, 10_000_000, 10_000_000);
            let visits = OBJECT_INDEX_CENSUS_VISITS_V29.get();
            drop(restore);
            assert!(!completed);
            assert!(visits > 0, "the genuine source census was not visited");
            let expected = match fault {
                1 => "original typed selector read is missing from the actual candidate",
                2 => "original typed selector read is duplicated in the actual candidate",
                3 => "typed object source payload differs from its actual operation",
                _ => unreachable!(),
            };
            assert!(
                matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0, detail, ..
                }))) if detail == expected),
                "branches={branches}, fault={fault}: {refused:?}"
            );
        }
    }
}
