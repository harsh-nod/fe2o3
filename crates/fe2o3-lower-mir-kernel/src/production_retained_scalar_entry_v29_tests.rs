use super::*;

const LIMIT: usize = 10_000_000;
thread_local! {
    static ENTRY_OBSERVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn retained_reference_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = super::super::fixtures::repeated_reference_owner();
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    assert_eq!(helper.blocks().len(), 1);
    assert_eq!(helper.locals().len(), 3);
    let original = &helper.blocks()[0];
    assert_eq!(original.statements().len(), 2);
    // Preserve both original operations, but keep the shared holder live over
    // a real CFG edge. The same-block promotion does not remove this backing.
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
        helper.entry(),
        vec![
            SemanticBasicBlockV1::new(
                original.identity(),
                original.source(),
                original.statements()[..1].to_vec(),
                SemanticTerminatorV1::new(original.terminator().source(),
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto, SemanticBlockIdV1::from_index(1),
                    ))),
            ).unwrap(),
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([135; 32]),
                original.source(),
                original.statements()[1..].to_vec(),
                original.terminator().clone(),
            ).unwrap(),
        ],
    ).unwrap();
    let owner = super::super::fixtures::build(
        semantic.types().to_vec(), functions, semantic.callables().to_vec(),
    );
    assert_cross_cfg_retention(&owner, 3, 2);
    owner
}

fn assert_cross_cfg_retention(owner: &ProductionSemanticSsaOwnerV1, function: u32, holder: u32) {
    let id = SemanticFunctionIdV1::from_index(function);
    let declaration = &owner.source_semantic().functions()[function as usize];
    assert_eq!(declaration.blocks().len(), 2);
    assert_eq!(declaration.blocks()[0].statements().len(), 1);
    let SemanticStatementKindV1::Assign(assignment) = declaration.blocks()[0].statements()[0].kind()
        else { panic!("original shared borrow") };
    assert_eq!(assignment.destination().local().index(), holder);
    assert!(matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared, place,
    } if place.local().index() == 1 && place.projections().is_empty()));
    assert!(matches!(declaration.blocks()[0].terminator().kind(),
        SemanticTerminatorKindV1::Goto(edge) if edge.target().index() == 1));
    let plan = owner.plan_for_function(id).unwrap().plan();
    assert!(plan.promoted_variables().iter().all(|variable| variable.get() != 1),
        "the ordinary argument must still require backing");
    assert!(plan.live_in(fe2o3_mir_model::SsaBlockIdV1::new(1)).unwrap()
        .iter().any(|variable| variable.get() == holder),
        "the original reference holder must remain live across the edge");
}

fn stopped_original_source(result: &SourceOwnedResultV18<()>) -> bool {
    matches!(result, Err(ProductionSourceOwnedViewErrorV18::Source(
        ProductionPendingScopedSourceErrorV29::Source(
            ProductionSemanticKirErrorV1::Unsupported { detail: STOP, .. }
        )
    )))
}

fn ordinary_slots(
    instances: &ExecutionInstancesV29<'_>,
    receipt: &OwnedScopedSourceSlotsV29,
) -> Vec<ScopedSourceSlotV29> {
    let selected: Vec<_> = receipt
        .slots
        .iter()
        .copied()
        .filter(|slot| {
            instances
                .instance(slot.instance)
                .unwrap()
                .function()
                .index()
                == 3
                && slot.legacy_local().unwrap() == 1
        })
        .collect();
    assert_eq!(
        selected.len(),
        2,
        "both original helper invocations retain their argument"
    );
    assert_ne!(selected[0].instance, selected[1].instance);
    assert_ne!(selected[0].origin.pointer, selected[1].origin.pointer);
    selected
}

fn check_noncell(
    plan: &SourceReferencePlanV29<'_, '_>,
    slot: ScopedSourceSlotV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut found = false;
    for (index, loan) in plan.loans.iter().enumerate() {
        let origin = &plan.origins[loan.origin];
        if origin.instance == slot.instance && origin.local.index() == slot.legacy_local()? {
            assert_eq!(
                loan.representation,
                SourceReferenceRepresentationV29::StableReferent
            );
            assert!(plan.scalar_cell(index, budget)?.is_none());
            found = true;
        }
    }
    assert!(found);
    assert!(
        !plan.cells.rows.iter().any(|cell| {
            cell.instance == slot.instance && cell.local.index() == slot.legacy_local().unwrap()
        })
    );
    Ok(())
}

fn entry_operation(lowered: &LoweredFunctionResultV1, slot: ScopedSourceSlotV29) -> &Operation {
    &lowered.function.body.as_ref().unwrap().blocks[slot.allocation.block_ordinal].operations
        [slot.allocation.operation + 1]
}

#[test]
fn ordinary_retained_helper_arguments_complete_full_root_emission() {
    fn observe(
        _source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let slots = ordinary_slots(instances, receipt);
        with_source_reference_storage_plan_v29(
            instances,
            SourceReferenceStorageV29::ScalarCells,
            budget,
            |plan, budget| {
                for slot in slots {
                    check_noncell(plan, slot, budget)?;
                    let lowered = emitted[slot.instance.index()].as_ref().unwrap();
                    source_reference_retained_scalar_entry_store_v29(
                        plan,
                        slot.instance,
                        slot.origin,
                        lowered,
                        entry_operation(lowered, slot),
                        budget,
                    )?;
                    ENTRY_OBSERVED.set(ENTRY_OBSERVED.get() + 1);
                }
                Ok(())
            },
        )
    }
    ENTRY_OBSERVED.set(0);
    let (result, _, _, completed) = run_original_repeated_source_v29(
        retained_reference_owner,
        observe,
        LIMIT,
        LIMIT,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(completed, "the retained helper entries must reach final physical admission");
    assert_eq!(ENTRY_OBSERVED.get(), 6, "two helpers in each of three mandatory replays");
}

#[test]
fn complete_noncell_entry_census_reconstructs_its_original_abi_without_omissions() {
    fn observe(
        _source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        ordinary_slots(instances, receipt);
        with_canonical_call_scratch_v1(budget, |budget| {
            for (index, lowered) in emitted.iter().enumerate() {
                let instance = instances.id_at(index).unwrap();
                let row = instances.instance(instance).unwrap();
                let candidates = scoped_slot_candidates_v29(row.declaration(), row.ssa(), budget)?;
                assert_eq!(
                    candidates.iter().filter(|&&flag| flag == 2).count(),
                    lowered
                        .as_ref()
                        .unwrap()
                        .scoped_slot_origins
                        .as_ref()
                        .unwrap()
                        .len(),
                    "this regression must exercise the complete-census branch",
                );
            }
            Ok(())
        })?;
        let floor = budget.storage();
        let fresh = derive_scoped_source_slots_with_demanded_plan_v29(instances, emitted, 1024, budget)?;
        assert_eq!(fresh.slots, receipt.slots);
        assert_eq!(fresh.instances, receipt.instances);
        let retained = fresh.retained_storage;
        drop(fresh);
        budget.release_storage(retained)?;
        assert_eq!(budget.storage(), floor);
        ENTRY_OBSERVED.set(ENTRY_OBSERVED.get() + 1);
        OBSERVED.set(OBSERVED.get() + 1);
        Ok(())
    }
    ENTRY_OBSERVED.set(0);
    let (result, _, _, completed) = run_original_repeated_source_v29(
        retained_reference_owner,
        observe,
        LIMIT,
        LIMIT,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(completed, "the complete original census must reach final physical admission");
    assert_eq!(ENTRY_OBSERVED.get(), 3);
    assert_eq!(OBSERVED.get(), 3);
}

#[test]
fn ordinary_entry_validation_does_not_grant_scalar_cell_membership() {
    fn observe(
        _source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let slots = ordinary_slots(instances, receipt);
        with_source_reference_storage_plan_v29(
            instances,
            SourceReferenceStorageV29::ScalarCells,
            budget,
            |plan, budget| {
                for slot in slots {
                    check_noncell(plan, slot, budget)?;
                    let lowered = emitted[slot.instance.index()].as_ref().unwrap();
                    let operation = entry_operation(lowered, slot);
                    source_reference_retained_scalar_entry_store_v29(
                        plan,
                        slot.instance,
                        slot.origin,
                        lowered,
                        operation,
                        budget,
                    )?;
                    assert!(
                        source_reference_cell_entry_store_v29(
                            plan,
                            slot.instance,
                            slot.origin,
                            lowered,
                            operation,
                            budget,
                        )
                        .is_err()
                    );
                    ENTRY_OBSERVED.set(ENTRY_OBSERVED.get() + 1);
                }
                Ok(())
            },
        )?;
        Err(unsupported(0, None, None, STOP))
    }
    ENTRY_OBSERVED.set(0);
    let (result, _, _, completed) = run_original_repeated_source_v29(
        retained_reference_owner,
        observe,
        LIMIT,
        LIMIT,
    );
    assert!(stopped_original_source(&result), "{result:?}");
    assert!(!completed);
    assert_eq!(ENTRY_OBSERVED.get(), 2);
}

#[test]
fn complete_ordinary_entry_census_rejects_initializer_and_instance_tampering() {
    fn observe(
        _source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let slots = ordinary_slots(instances, receipt);
        let slot = slots[0];
        for fault in 0..6 {
            let floor = budget.storage();
            let positive = derive_scoped_source_slots_with_demanded_plan_v29(instances, emitted, 1024, budget)?;
            assert_eq!(positive.slots, receipt.slots);
            assert_eq!(positive.instances, receipt.instances);
            let retained = positive.retained_storage;
            drop(positive);
            budget.release_storage(retained)?;
            assert_eq!(budget.storage(), floor);
            let lowered = emitted[slot.instance.index()].as_mut().unwrap();
            let original_instance = lowered.source_call_instance;
            let original_origins = lowered.scoped_slot_origins.as_ref().unwrap().clone();
            let operations = &mut lowered.function.body.as_mut().unwrap().blocks
                [slot.allocation.block_ordinal]
                .operations;
            let position = slot.allocation.operation + 1;
            let original = operations[position].clone();
            match fault {
                0 => {
                    let OperationKind::Store { value, .. } = &mut operations[position].kind else {
                        panic!("entry Store")
                    };
                    *value = slot.origin.pointer;
                }
                1 => {
                    let OperationKind::Store { pointer, .. } = &mut operations[position].kind
                    else {
                        panic!("entry Store")
                    };
                    *pointer = slots[1].origin.pointer;
                }
                2 => {
                    let OperationKind::Store { access, .. } = &mut operations[position].kind else {
                        panic!("entry Store")
                    };
                    *access = MemoryAccess::new(AddressSpace::Global, 4);
                }
                3 => lowered.source_call_instance = Some(slots[1].instance),
                4 => lowered.scoped_slot_origins.as_mut().unwrap()[0].semantic_type = UNIT,
                5 => lowered.scoped_slot_origins.as_mut().unwrap()[0].identity = ScopedAllocationIdentityV29::LegacyLocal(0),
                _ => unreachable!(),
            }
            let floor = budget.storage();
            let refused = derive_scoped_source_slots_with_demanded_plan_v29(
                instances, emitted, 1024, budget,
            );
            assert!(
                matches!(
                    refused.as_ref().err(),
                    Some(ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "scoped source-slot allocation census is incomplete or mismatched",
                    })
                ),
                "initializer fault {fault} must fail the exact source-slot census: {:?}",
                refused.as_ref().err(),
            );
            drop(refused);
            assert_eq!(budget.storage(), floor);
            let lowered = emitted[slot.instance.index()].as_mut().unwrap();
            lowered.source_call_instance = original_instance;
            lowered.scoped_slot_origins = Some(original_origins);
            lowered.function.body.as_mut().unwrap().blocks[slot.allocation.block_ordinal]
                .operations[position] = original;
            ENTRY_OBSERVED.set(ENTRY_OBSERVED.get() + 1);
        }
        Err(unsupported(0, None, None, STOP))
    }
    ENTRY_OBSERVED.set(0);
    let (result, _, _, completed) = run_original_repeated_source_v29(
        retained_reference_owner,
        observe,
        LIMIT,
        LIMIT,
    );
    assert!(stopped_original_source(&result), "{result:?}");
    assert!(!completed);
    assert_eq!(ENTRY_OBSERVED.get(), 6);
}

fn root_argument_owner() -> ProductionSemanticSsaOwnerV1 {
    let template = lifecycle_owner(false);
    let mut types = template.source_semantic().types()[..2].to_vec();
    let shared = reference(&mut types, U32, SemanticMutabilityV1::Immutable, false);
    let dereference = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(3),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    let root = function(
        201,
        SemanticFunctionRoleV1::KernelRoot,
        abi(202, true, &[U32, U32]),
        vec![
            local(203, UNIT, SemanticLocalRoleV1::Return),
            local(204, U32, SemanticLocalRoleV1::Argument(0)),
            local(205, U32, SemanticLocalRoleV1::Argument(1)),
            local(206, shared, SemanticLocalRoleV1::Temporary),
            local(207, U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(208, vec![assign(
                    place(3, shared),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(1, U32),
                    },
                )], SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto, SemanticBlockIdV1::from_index(1),
                ))),
            block(209, vec![
                assign(
                    place(4, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(dereference)),
                ),
                assign(
                    place(0, UNIT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(UNIT, SemanticConstantValueV1::ZeroSized),
                    )),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .with_kernel_entry(
        template.source_semantic().functions()[0]
            .kernel_entry()
            .unwrap()
            .clone(),
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert_cross_cfg_retention(&owner, 0, 3);
    owner
}

fn with_root_lowered(
    inspect: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut LoweredFunctionResultV1,
        ScopedSlotOriginV29,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut owner = root_argument_owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage())?;
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                with_source_reference_storage_plan_v29(
                    instances,
                    SourceReferenceStorageV29::ScalarCells,
                    budget,
                    |plan, budget| {
                        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
                        let semantic = owner.source_semantic();
                        let root = kernel_entry_plan_v1(
                            semantic,
                            ROOT,
                            ROOT,
                            FunctionId::new("retained_entry_component"),
                            16_384,
                            &mut ReachableClosureBudgetV1::new(16_384),
                        )?;
                        let mut private = PrivateArrayLazyBudgetV1::new(1, 16_384);
                        let mut lowered = with_source_reference_availability_v29(
                            instances,
                            instances.root(),
                            Some(&emission),
                            budget,
                            |cursor, budget| {
                                lower_one_semantic_function_with_calls_v29(
                                    semantic,
                                    &root,
                                    instances.instance(instances.root()).unwrap().ssa(),
                                    &BTreeMap::new(),
                                    &BTreeMap::new(),
                                    None,
                                    BTreeSet::new(),
                                    1,
                                    true,
                                    16_384,
                                    None,
                                    &mut private,
                                    None,
                                    budget,
                                    SemanticEmissionPlacementV1::default(),
                                    Some(cursor),
                                    None,
                                    None,
                                )
                            },
                        )?;
                        emission.finish(budget)?;
                        assert!(plan.cells.rows.is_empty());
                        assert!(!plan.loans.is_empty());
                        assert!(plan.loans.iter().all(|loan| loan.representation
                            == SourceReferenceRepresentationV29::StableReferent));
                        let origins = lowered.scoped_slot_origins.as_ref().unwrap();
                        assert_eq!(origins.len(), 1);
                        let origin = origins[0];
                        assert_eq!((origin.legacy_local().unwrap(), origin.semantic_type), (1, U32));
                        inspect(plan, &mut lowered, origin, budget)
                    },
                ),
            )
        })
        .unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    result
}

#[test]
fn ordinary_retained_root_argument_has_its_original_abi_initializer_but_is_not_a_cell() {
    with_root_lowered(|plan, lowered, origin, budget| {
        let operation = &lowered.function.body.as_ref().unwrap().blocks[0].operations[1];
        source_reference_retained_scalar_entry_store_v29(
            plan,
            plan.instances.root(),
            origin,
            lowered,
            operation,
            budget,
        )?;
        assert!(
            source_reference_cell_entry_store_v29(
                plan,
                plan.instances.root(),
                origin,
                lowered,
                operation,
                budget,
            )
            .is_err()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn ordinary_retained_root_entry_rejects_same_type_parameter_substitution_and_forged_origin() {
    with_root_lowered(|plan, lowered, origin, budget| {
        let body = lowered.function.body.as_ref().unwrap();
        let original = body.blocks[0].operations[1].clone();
        let parameter = source_reference_cell_initial_parameter_v29(
            plan,
            plan.instances.root(),
            SemanticLocalIdV1::from_index(origin.legacy_local()?),
            lowered,
            budget,
        )?;
        assert_ne!(parameter, body.parameters[1]);
        for fault in 0..7 {
            let mut operation = original.clone();
            let mut changed_origin = origin;
            let original_instance = lowered.source_call_instance;
            match fault {
                0 => {
                    let other = lowered.function.body.as_ref().unwrap().parameters[1];
                    let OperationKind::Store { value, .. } = &mut operation.kind else {
                        panic!("entry Store")
                    };
                    *value = other;
                }
                1 => {
                    let OperationKind::Store { pointer, .. } = &mut operation.kind else {
                        panic!("entry Store")
                    };
                    *pointer = parameter;
                }
                2 => {
                    let OperationKind::Store { access, .. } = &mut operation.kind else {
                        panic!("entry Store")
                    };
                    *access = MemoryAccess::new(AddressSpace::Private, 1);
                }
                3 => changed_origin.semantic_type = UNIT,
                4 => changed_origin.identity = ScopedAllocationIdentityV29::LegacyLocal(0),
                5 => lowered.source_call_instance = None,
                6 => operation.results.push(ValueDef::new(
                    ValueId(lowered.next_value),
                    Type::Scalar(ScalarType::U32),
                )),
                _ => unreachable!(),
            }
            assert!(
                source_reference_retained_scalar_entry_store_v29(
                    plan,
                    plan.instances.root(),
                    changed_origin,
                    lowered,
                    &operation,
                    budget,
                )
                .is_err(),
                "fault {fault}"
            );
            lowered.source_call_instance = original_instance;
        }
        Ok(())
    })
    .unwrap();
}
