#[test]
fn bounded_call_stack_headers_are_independent_named_envelopes_and_checked_arithmetic() {
    let output = std::mem::size_of::<Vec<Option<LoweredFunctionResultV1>>>()
        + std::mem::size_of::<(Vec<Option<LoweredFunctionResultV1>>, PrivateArrayPayloadV1)>()
        + std::mem::size_of::<
            Result<
                (Vec<Option<LoweredFunctionResultV1>>, PrivateArrayPayloadV1),
                ProductionSemanticKirErrorV1,
            >,
        >();
    assert_eq!(scoped_function_stack_output_headers_v1().unwrap(), output);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, 29 + output - usize::from(short));
        budget.reserve_storage(29).unwrap();
        assert_eq!(
            budget
                .reserve_storage(scoped_function_stack_output_headers_v1().unwrap())
                .is_ok(),
            !short
        );
        if !short {
            budget.release_storage(output).unwrap();
        }
        assert_eq!(budget.storage(), 29);
        assert_eq!(budget.failed_storage(), short.then_some(29 + output));
    }
    let fixed = std::mem::size_of::<Vec<Option<ExecutionLifecycleProducerV29<'static>>>>()
        + std::mem::size_of::<Vec<ScopedWaitingFunctionV1<'static>>>()
        + std::mem::size_of::<ScopedActiveFunctionV1>();
    let per_instance = std::mem::size_of::<ScopedFunctionBoundaryV1<'static>>()
        + std::mem::size_of::<
            Result<ScopedFunctionBoundaryV1<'static>, ProductionSemanticKirErrorV1>,
        >()
        + std::mem::size_of::<
            std::thread::Result<
                Result<ScopedFunctionBoundaryV1<'static>, ProductionSemanticKirErrorV1>,
            >,
        >();
    for count in [0, 1, 2, 7, 31] {
        let bytes = fixed + count * per_instance;
        assert_eq!(scoped_function_stack_headers_v1(count).unwrap(), bytes);
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = ArgumentBudgetV1::new(&mut work, 41 + bytes - usize::from(short));
            budget.reserve_storage(41).unwrap();
            assert!(budget.reserve_storage(bytes + 123).is_err());
            let history = budget.failed_storage();
            let result = budget.reserve_storage(scoped_function_stack_headers_v1(count).unwrap());
            assert_eq!(result.is_ok(), !short);
            if !short {
                budget.release_storage(bytes).unwrap();
            }
            assert_eq!(budget.storage(), 41);
            assert_eq!(budget.failed_storage(), history);
            assert_eq!(budget.peak_storage(), if short { 41 } else { 41 + bytes });
        }
    }
    assert!(matches!(
        scoped_function_stack_headers_v1(usize::MAX),
        Err(ArgumentResourceV1::Arithmetic)
    ));
}

fn progress_resource_observer_v1(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let id = instances.id_at(instances.instances().len() - 1).unwrap();
    let output = emitted[id.index()].as_ref().unwrap();
    let placement = output.lifecycle_events.as_ref().unwrap().placement;
    let bytes = std::mem::size_of::<EmissionSubtreeProgressV1>()
        + std::mem::size_of::<Result<EmissionSubtreeProgressV1, ProductionSemanticKirErrorV1>>()
        + std::mem::size_of::<CheckedChildEmissionProgressV1>()
        + std::mem::size_of::<Result<CheckedChildEmissionProgressV1, ProductionSemanticKirErrorV1>>(
        );
    // Three active-instance/placement checks then the original 20-work source identity.
    for (work_limit, storage_limit, succeeds) in [
        (23, 47 + bytes, true),
        (22, 47 + bytes, false),
        (23, 46 + bytes, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(47)?;
        let result = scoped_slot_attempt_v29(&mut budget, |budget| {
            let progress = EmissionSubtreeProgressV1::new(instances, id, placement, budget)?;
            assert_eq!(progress.credit, bytes);
            assert_eq!(progress.next_value, placement.first_value);
            assert_eq!(progress.descendant_operations, 0);
            drop(progress);
            budget.release_storage(bytes)?;
            Ok(())
        });
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(budget.storage(), 47);
        assert_eq!(
            budget.failed_storage(),
            (storage_limit == 46 + bytes).then_some(47 + bytes)
        );
        drop(budget);
        assert_eq!(work.work(), if work_limit == 22 { 3 } else { 23 });
        assert_eq!(work.failed_work(), (work_limit == 22).then_some(23));
    }
    FRESH_SOURCE_OBSERVED_V1.set((3, 1));
    Ok(())
}

#[test]
fn child_progress_construction_has_exact_inclusive_work_and_storage_limits() {
    FRESH_SOURCE_OBSERVED_V1.set((0, 0));
    let (result, _, _) = run_profiled_suffix_owner(
        || fresh_tile_return_owner_v1(FreshReturnCaseV1::Straight),
        progress_resource_observer_v1,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(FRESH_SOURCE_OBSERVED_V1.get(), (3, 1));
}
fn actual_child_progress_observer_v1(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let id = instances
        .instances()
        .iter()
        .enumerate()
        .find_map(|(ordinal, row)| {
            (row.function().index() == 4).then(|| instances.id_at(ordinal).unwrap())
        })
        .unwrap();
    let incoming = instances.incoming(id).unwrap();
    let output = emitted[id.index()].as_mut().unwrap();
    let placement = output.lifecycle_events.as_ref().unwrap().placement;
    let floor = budget.storage();
    for fault in 0..6 {
        let saved_instance = output.source_call_instance;
        let saved_next = output.next_value;
        let saved_count = output.emitted_operations;
        let saved_placement = output.private_arrays.placement;
        let saved_lifecycle = output.lifecycle_events.as_ref().unwrap().placement;
        let mut entered = false;
        let result = scoped_slot_attempt_v29(budget, |budget| {
            let progress = EmissionSubtreeProgressV1::new(instances, id, placement, budget)?;
            match fault {
                0 => {}
                1 => output.source_call_instance = Some(instances.root()),
                2 => output.next_value = placement.first_value.saturating_sub(1),
                3 => output.emitted_operations += 1,
                4 => output.private_arrays.placement.first_block += 1,
                5 => {
                    output
                        .lifecycle_events
                        .as_mut()
                        .unwrap()
                        .placement
                        .first_value += 1
                }
                _ => unreachable!(),
            }
            let mut private = PrivateArrayLazyBudgetV1::new(1, LIMIT);
            entered = true;
            let checked = progress.finish(output, instances, &mut private, budget)?;
            let credit = checked.credit;
            assert_eq!(checked.instance, id);
            assert_eq!(checked.caller, Some(incoming.occurrence()));
            assert_eq!(checked.next_value, output.next_value);
            assert_eq!(
                checked.operations,
                output.emitted_operations + output.lifecycle_events.as_ref().unwrap().rows.len()
            );
            drop(checked);
            budget.release_storage(credit)?;
            Ok(())
        });
        output.source_call_instance = saved_instance;
        output.next_value = saved_next;
        output.emitted_operations = saved_count;
        output.private_arrays.placement = saved_placement;
        output.lifecycle_events.as_mut().unwrap().placement = saved_lifecycle;
        assert!(entered);
        assert_eq!(
            result.is_ok(),
            fault == 0,
            "actual child progress fault {fault}: {result:?}"
        );
        assert_eq!(budget.storage(), floor);
    }
    // Request checking is a coordinate/credit check only. It consumes the
    // actual finished leaf above; the scheduler separately authenticates the
    // original operands, current cursor, return archive and complete exits.
    let mut private = PrivateArrayLazyBudgetV1::new(1, LIMIT);
    let progress = EmissionSubtreeProgressV1::new(instances, id, placement, budget)?;
    let mut checked = progress.finish(output, instances, &mut private, budget)?;
    let request = ExecutionCallRequestV1 {
        source: ExecutionCallSourceV29::from_instances(instances, budget)?,
        occurrence: incoming.occurrence(),
        original_call: std::ptr::from_ref(incoming.source()) as usize,
        child: id,
        callee: instances.instance(id).unwrap().function(),
        normal_return: true,
        target: output.function.id.clone(),
        arguments: vec![],
        credit: 0,
    };
    checked.check_request(&request, budget)?;
    let original = checked.instance;
    checked.instance = incoming.occurrence().caller;
    assert!(checked.check_request(&request, budget).is_err());
    checked.instance = original;
    let original = checked.caller;
    checked.caller = None;
    assert!(checked.check_request(&request, budget).is_err());
    checked.caller = original;
    let original = checked.slot;
    checked.slot ^= 1;
    assert!(checked.check_request(&request, budget).is_err());
    checked.slot = original;
    let original = checked.source.root;
    checked.source.root = instances.instance(id).unwrap().function();
    assert!(checked.check_request(&request, budget).is_err());
    checked.source.root = original;
    checked.check_request(&request, budget)?;
    let credit = checked.credit;
    drop(checked);
    budget.release_storage(credit)?;
    assert_eq!(budget.storage(), floor);
    FRESH_SOURCE_OBSERVED_V1.set((6, 4));
    Ok(())
}

#[test]
fn actual_completed_child_progress_rejects_foreign_ownership_regression_and_changed_census() {
    FRESH_SOURCE_OBSERVED_V1.set((0, 0));
    let (result, _, _) = run_profiled_suffix_owner(
        || fresh_tile_return_owner_v1(FreshReturnCaseV1::Straight),
        actual_child_progress_observer_v1,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(FRESH_SOURCE_OBSERVED_V1.get(), (6, 4));
}
