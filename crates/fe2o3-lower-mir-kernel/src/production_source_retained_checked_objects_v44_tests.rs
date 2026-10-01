use super::*;

mod scalar_range_tests_v45 {
    use super::*;
    include!("production_source_scalar_ranges_owner_v45_tests.rs");
}

thread_local! {
    static CHECKED_OBJECT_FAULT_V44: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static CHECKED_OBJECT_MUTATIONS_V44: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn alter_checked_object_values_v44(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let fault = CHECKED_OBJECT_FAULT_V44.get();
    if fault > 2 {
        return Ok(());
    }
    for lowered in emitted.iter_mut().flatten() {
        let Some(anchors) = lowered.scoped_memory_anchors.as_ref() else {
            continue;
        };
        let mut fields = [None, None];
        for (anchor, row) in anchors.rows.iter().enumerate() {
            budget.charge_work(1)?;
            let ScopedMemoryAnchorKindV29::Object(index) = row.kind else {
                continue;
            };
            let payload = &anchors.objects[index];
            if let ScopedObjectRoleV29::WriteValue { destination, .. } = payload.role
                && let ScopedObjectSourceV29::RvalueComponent { site, result, .. } =
                    destination.source
            {
                assert!(result < 2);
                let ScopedObjectOperationV29::WriteValue { value, .. } = payload.operation else {
                    panic!("genuine computed field write");
                };
                assert!(
                    fields[result as usize]
                        .replace((anchor, index, site, value))
                        .is_none()
                );
            }
        }
        if fields.iter().all(Option::is_none) {
            continue;
        }
        let [Some(first), Some(second)] = fields else {
            panic!("both computed field rows");
        };
        assert_eq!(first.2, second.2);
        assert_ne!(first.3, second.3);
        let archived = lowered.execution_observation.as_ref().unwrap();
        let key = execution_rvalue_key_v30(first.2)?;
        let operands = archived
            .rvalues
            .get(&key)
            .unwrap()
            .checked_operands
            .unwrap();
        assert_ne!(operands[0], first.3);
        let replacements = match fault {
            0 => [first.3, second.3],
            1 => [operands[0], second.3],
            2 => [second.3, first.3],
            _ => unreachable!(),
        };
        for (entry, replacement) in [first, second].into_iter().zip(replacements) {
            budget.charge_work(2)?;
            let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
            let row = anchors.rows[entry.0];
            let payload = &mut anchors.objects[entry.1];
            let ScopedObjectOperationV29::WriteValue { value, .. } = &mut payload.operation else {
                unreachable!()
            };
            *value = replacement;
            let block = lowered
                .function
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .find(|block| block.id == row.block)
                .unwrap();
            let operation = &mut block.operations[row.position];
            assert!(matches!(operation.kind, OperationKind::Storage(
                ScopedObjectOperationV29::WriteValue { value, .. }) if value == entry.3));
            // Change the actual operation and its inert anchor together. The
            // unchanged original archive must still reject a false computation.
            operation.kind = OperationKind::Storage(payload.operation);
        }
        CHECKED_OBJECT_MUTATIONS_V44.set(CHECKED_OBJECT_MUTATIONS_V44.get() + 1);
    }
    Ok(())
}

fn alter_checked_object_census_v44(
    source_index: &SourceAddressSourceIndexV29<'_>,
    rows: &mut Vec<SourceAddressAccessSourceV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let fault = CHECKED_OBJECT_FAULT_V44.get();
    if fault < 3 {
        return Ok(());
    }
    let mut fields = [None, None];
    for (index, source) in rows.iter().enumerate() {
        budget.charge_work(2)?;
        let sidecar = source_index.sidecar(source.instance, budget)?;
        let anchors = sidecar.scoped_memory_anchors.as_ref().unwrap();
        let row = &anchors.rows[source.anchor];
        if !matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)) {
            continue;
        }
        let payload = anchors.object_payload(row, budget)?;
        if let ScopedObjectRoleV29::WriteValue { destination, .. } = payload.role
            && let ScopedObjectSourceV29::RvalueComponent { result, .. } = destination.source
        {
            assert!(result < 2);
            fields[result as usize].get_or_insert(index);
        }
    }
    let [Some(first), Some(second)] = fields else {
        panic!("complete original computed fields");
    };
    assert_ne!(first, second);
    match fault {
        3 => {
            budget.charge_work(rows.len())?;
            rows.remove(second);
        }
        4 => {
            let duplicate = rows[first];
            emission_push_v1(rows, duplicate, budget)?;
        }
        _ => unreachable!(),
    }
    CHECKED_OBJECT_MUTATIONS_V44.set(CHECKED_OBJECT_MUTATIONS_V44.get() + 1);
    Ok(())
}

#[test]
fn retained_checked_object_consumer_rejects_wrong_results_and_incomplete_field_census() {
    struct Restore(
        Option<ScopedSlotObserverV29>,
        Option<SourceObjectEffectCensusObserverV29>,
        u8,
        usize,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
            SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29.set(self.1);
            CHECKED_OBJECT_FAULT_V44.set(self.2);
            CHECKED_OBJECT_MUTATIONS_V44.set(self.3);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_OBSERVER_V29.replace(Some(alter_checked_object_values_v44)),
        SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29.replace(Some(alter_checked_object_census_v44)),
        CHECKED_OBJECT_FAULT_V44.get(),
        CHECKED_OBJECT_MUTATIONS_V44.get(),
    );
    for fault in 0..=4 {
        CHECKED_OBJECT_FAULT_V44.set(fault);
        CHECKED_OBJECT_MUTATIONS_V44.set(0);
        let (result, _, _, completed) = run(
            SemanticCheckedBinaryOpV1::Add,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(
            CHECKED_OBJECT_MUTATIONS_V44.get() > 0,
            "fault={fault}: {result:?}"
        );
        assert_eq!(result.is_ok(), fault == 0, "fault={fault}: {result:?}");
        assert_eq!(
            completed,
            usize::from(fault == 0),
            "fault={fault}: {result:?}"
        );
        if fault == 1 {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("checked field changed its exact archived result")
            );
        } else if fault == 3 {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("original checked result write is missing or duplicated")
            );
        } else if fault != 0 {
            let error = result.unwrap_err();
            let mut current: Option<&(dyn std::error::Error + 'static)> = Some(&error);
            while let Some(part) = current {
                assert!(
                    part.downcast_ref::<ArgumentResourceV1>().is_none(),
                    "{error:?}"
                );
                current = part.source();
            }
        }
    }
}

fn retained_checked_owner(operation: SemanticCheckedBinaryOpV1) -> ProductionSemanticSsaOwnerV1 {
    let base = retained_projected_failure_loop();
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let mut blocks = original.blocks().to_vec();
    let body = &blocks[2];
    let mut statements = body.statements().to_vec();
    let SemanticStatementKindV1::Assign(assignment) = statements[0].kind() else {
        panic!("original retained checked assignment");
    };
    let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
        panic!("original checked operands");
    };
    let left = checked.left().clone();
    let right = checked.right().clone();
    statements[0] = assign(
        assignment.destination().clone(),
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            operation,
            left.clone(),
            right.clone(),
        )),
    );
    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        target,
        unwind,
        ..
    } = body.terminator().kind()
    else {
        panic!("original overflow guard");
    };
    // This positive exercises the checked write with ordinary original scalar
    // diagnostics. The separate retained projected diagnostic test stays closed.
    let term = SemanticTerminatorKindV1::Assert {
        condition: condition.clone(),
        expected: *expected,
        message: SemanticAssertMessageV1::Overflow {
            operation: match operation {
                SemanticCheckedBinaryOpV1::Add => SemanticBinaryOpV1::Add,
                SemanticCheckedBinaryOpV1::Subtract => SemanticBinaryOpV1::Subtract,
                SemanticCheckedBinaryOpV1::Multiply => SemanticBinaryOpV1::Multiply,
            },
            left,
            right,
        },
        target: *target,
        unwind: *unwind,
    };
    blocks[2] = SemanticBasicBlockV1::new(
        body.identity(),
        body.source(),
        statements,
        SemanticTerminatorV1::new(body.terminator().source(), term),
    )
    .unwrap();
    let mut functions = source.functions().to_vec();
    functions[0] = rebuild_root(original, original.locals().to_vec(), blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
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

fn run(
    operation: SemanticCheckedBinaryOpV1,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, usize) {
    run_with_invalid_site(operation, work, storage, false)
}

fn run_with_invalid_site(
    operation: SemanticCheckedBinaryOpV1,
    work: usize,
    storage: usize,
    invalid_site: bool,
) -> (SourceOwnedResultV18<()>, usize, usize, usize) {
    let mut ledger = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut ledger, storage);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut visited = 0;
    let projection = retained_checked_owner(operation);
    let owner = retained_checked_owner(operation);
    let result = (|| -> SourceOwnedResultV18<()> {
        let (_, launch) =
            with_module_fixture_view(&owner, ModuleFixture::Ordinary, &mut budget, |_, _| ())?;
        with_module_fixture_view(&projection, ModuleFixture::Ordinary, &mut budget,
        |input, budget| -> SourceOwnedResultV18<()> {
            assert!(
                !owner.plans()[0]
                    .plan()
                    .promoted_variables()
                    .iter()
                    .any(|local| local.get() == 4)
            );
            assert!(owner.source_semantic().functions()[0].blocks()[2].statements().iter()
                .any(|row| matches!(row.kind(), SemanticStatementKindV1::Assign(assignment)
                    if matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { place, .. }
                        if place.local().index() == 4 && place.projections().is_empty()))));
            let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
            let roots = fixture.roots();
            let prepared =
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    input.input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )?;
            prepared.with_source_consumer_v18(budget, |source, budget| {
                source.with_analysis_v18(budget, |scope| {
                    scope.with_inventory_v1(|inventory, budget| {
                        source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                            source_scalar_normalization_scratch_v18(source.cleanup, budget, 0, |budget| {
                              scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 0, None, budget, |_memory, budget| -> SourceOwnedResultV18<()> {
                                let definitions = relation.checked_assignment_definitions_v44(
                                    0, 0, execution_site_v29(SemanticBlockIdV1::from_index(2), Some(0)), budget,
                                )?;
                                let values = definitions.map(|index| inventory.definitions()[index].value.unwrap());
                                assert_ne!(values[0], values[1]);
                                let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                                    operation: checked, result: 0,
                                } = inventory.definitions()[definitions[0]].coordinate else {
                                    panic!("exact original checked value result");
                                };
                                let mut stores = [0; 2];
                                for row in inventory.operations() {
                                    budget.charge_work(1)?;
                                    if row.coordinate.block.function != checked.block.function {
                                        continue;
                                    }
                                    if let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value, .. }) =
                                        row.operation.kind {
                                        for index in 0..2 {
                                            if value == values[index] { stores[index] += 1; }
                                        }
                                    }
                                }
                                assert_eq!(stores, [1, 1], "each exact checked result has one original destination field");
                                visited += 1;
                                if invalid_site {
                                    let error = relation.checked_assignment_definitions_v44(
                                        0, 0, execution_site_v29(SemanticBlockIdV1::from_index(2), None), budget,
                                    ).unwrap_err();
                                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(
                                        "checked result requires an original assignment")));
                                    let before = (budget.work(), budget.storage());
                                    assert!(matches!(relation.checked_assignment_definitions_v44(
                                        0, 0, execution_site_v29(SemanticBlockIdV1::from_index(2), Some(0)), budget,
                                    ), Err(ProductionSourceOwnedViewErrorV18::Binding(
                                        "checked result requires an original assignment"))));
                                    assert_eq!((budget.work(), budget.storage()), before);
                                    return Err(error);
                                }
                                Ok(())
                              })
                            })
                        })
                    })
                })
            })
        })?.0
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), visited)
}

#[test]
fn retained_checked_result_query_latches_non_assignment_origin_after_a_real_positive() {
    let observed = run_with_invalid_site(
        SemanticCheckedBinaryOpV1::Add,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        true,
    );
    assert_eq!(observed.3, 1);
    assert!(matches!(
        observed.0,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "checked result requires an original assignment"
        ))
    ));
}

#[test]
fn retained_checked_original_type_classifier_rejects_result_and_operand_substitution() {
    let owner = retained_checked_owner(SemanticCheckedBinaryOpV1::Add);
    let semantic = owner.source_semantic();
    let SemanticStatementKindV1::Assign(assignment) =
        semantic.functions()[0].blocks()[2].statements()[0].kind()
    else {
        panic!("exact original checked assignment");
    };
    let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
        panic!("original checked rvalue");
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let fields = source_object_checked_types_v44(
        semantic.types(),
        assignment.value().result_type(),
        checked,
        &mut budget,
    )
    .unwrap();
    assert_eq!(fields, &[U32, semantic.functions()[0].locals()[3].ty()]);
    let wrong_operand = SemanticCheckedBinaryRvalueV1::new(
        checked.operation(),
        SemanticOperandV1::Copy(place(3, fields[1])),
        checked.right().clone(),
    );
    for (result, operation) in [
        (U32, checked),
        (assignment.value().result_type(), &wrong_operand),
    ] {
        assert!(
            source_object_checked_types_v44(semantic.types(), result, operation, &mut budget)
                .is_err()
        );
    }
}

#[test]
fn retained_checked_pairs_reach_original_memory_consumer_with_both_exact_results() {
    for operation in [
        SemanticCheckedBinaryOpV1::Add,
        SemanticCheckedBinaryOpV1::Subtract,
        SemanticCheckedBinaryOpV1::Multiply,
    ] {
        let observed = run(operation, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        observed.0.unwrap();
        assert_eq!(observed.3, 1);
    }
}

#[test]
fn retained_checked_pair_complete_transaction_has_exact_resource_boundaries() {
    let operation = SemanticCheckedBinaryOpV1::Multiply;
    let measured = run(operation, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    measured.0.unwrap();
    assert_eq!(measured.3, 1);
    let exact = run(operation, measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, measured.2, 1));
    for (work, storage, is_work) in [
        (measured.1 - 1, measured.2, true),
        (measured.1, measured.2 - 1, false),
    ] {
        let result = run(operation, work, storage).0;
        let mut error: &(dyn std::error::Error + 'static) = result.as_ref().unwrap_err();
        loop {
            if let Some(resource) = error.downcast_ref::<ArgumentResourceV1>() {
                match resource {
                    ArgumentResourceV1::Work(limit) if is_work => {
                        assert_eq!(limit.limit(), work);
                        assert_eq!(limit.actual(), measured.1);
                    }
                    ArgumentResourceV1::Storage(limit) if !is_work => {
                        assert_eq!(limit.limit(), storage);
                        assert_eq!(limit.actual(), measured.2);
                    }
                    other => panic!("wrong resource: {other:?}"),
                }
                break;
            }
            error = error
                .source()
                .unwrap_or_else(|| panic!("missing resource: {result:?}"));
        }
    }
}
