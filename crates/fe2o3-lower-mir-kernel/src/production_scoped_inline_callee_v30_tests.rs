include!("production_scoped_lane_query_v29_tests.rs");

thread_local! {
    static SCOPED_INLINE_TEST_FAULT_V30: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SCOPED_INLINE_TEST_VISITS_V30: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

const SCOPED_INLINE_TEST_STOP_V30: &str = "scoped inline callee inspection completed";

struct ScopedInlineObserverResetV30(Option<ScopedSlotObserverV29>);

impl Drop for ScopedInlineObserverResetV30 {
    fn drop(&mut self) {
        SCOPED_SLOT_OBSERVER_V29.set(self.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn probe_inline_callee_v30(
    map: &ProductionInstanceCorrespondenceV1<'_, '_>,
    sidecars: &[PendingInstanceSidecarsV29],
    active: &PendingActiveInstanceIndexV1,
    container: ProductionCallInstanceIdV1,
    function: &Function,
    check_body: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), CallInstanceEmissionErrorV1> {
    let floor = budget.storage();
    let mut scratch = 0usize;
    let result = (|| {
        let view = ScopedDeferredScalarViewV29::for_container(
            map, sidecars, active, container, function, budget,
        )
        .map_err(|error| match error {
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
            _ => CallInstanceEmissionErrorV1::CalleeInlineAssembly,
        })?;
        let index = call_splice_index_v1(function, budget, &mut scratch)?;
        let before = scratch;
        let permit = view.inline_callee_v30(function, &index, budget, &mut scratch)?;
        let count = function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::InlineAssembly(_)))
            .count();
        let expected = if count == 0 {
            std::mem::size_of::<ScopedInlineCalleeV30<'_>>()
        } else {
            std::mem::size_of::<ScopedInlineCalleeV30<'_>>()
                + std::mem::size_of::<Vec<ScopedInlineActualV30<'_>>>()
                + std::mem::size_of::<[(ValueId, Option<ScalarType>); 2]>()
                + count * std::mem::size_of::<ScopedInlineActualV30<'_>>()
        };
        assert_eq!(scratch - before, expected);
        if check_body && count > 0 {
            permit.check(function, budget)?;
            let duplicate = function.clone();
            assert_eq!(
                permit.check(&duplicate, budget),
                Err(CallInstanceEmissionErrorV1::CalleeInlineAssembly)
            );
            assert_eq!(
                call_splice_check_body_v1(function, &index, true, budget),
                Err(CallInstanceEmissionErrorV1::CalleeInlineAssembly)
            );
            assert!(
                call_splice_check_body_with_scoped_inline_v30(
                    function,
                    &index,
                    true,
                    None,
                    Some(&permit),
                    budget,
                )? > 0
            );
        }
        Ok(())
    })();
    budget.release_storage(scratch).unwrap();
    assert_eq!(budget.storage(), floor);
    result
}

fn inline_operation_mut_v30(function: &mut Function) -> &mut Operation {
    function
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::InlineAssembly(_)))
        .unwrap()
}

fn inspect_inline_callees_v30(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    SCOPED_INLINE_TEST_VISITS_V30.set(SCOPED_INLINE_TEST_VISITS_V30.get() + 1);
    let mut next_block = pending_scope_preflight_v29(
        instances,
        emitted,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )?;
    let floor = budget.storage();
    with_canonical_call_scratch_v1(budget, |budget| {
        with_production_instance_correspondence_v1(instances, budget, |map, budget| {
            for row in emitted.iter().flatten() {
                map.append_lowered(row.source_call_instance.unwrap(), row, budget)?;
            }
            let (functions, sidecars): (Vec<_>, Vec<_>) = emitted
                .iter_mut()
                .map(|row| PendingInstanceSidecarsV29::split(row.take().unwrap()))
                .unzip();
            let active = pending_active_instance_index_v1(instances, &sidecars, budget)
                .map_err(instance_anchor_error_v1)?;
            assert_eq!(
                map.inline.rows.len(),
                2,
                "independent original intrinsic census"
            );
            let children: Vec<_> = functions
                .iter()
                .enumerate()
                .filter(|(_, function)| {
                    function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .any(|operation| matches!(operation.kind, OperationKind::InlineAssembly(_)))
                })
                .map(|(ordinal, _)| (sidecars[ordinal].source_call_instance.unwrap(), ordinal))
                .collect();
            assert_eq!(
                children.len(),
                2,
                "both authentic repeated source instances were emitted"
            );
            for &(child, ordinal) in &children {
                probe_inline_callee_v30(
                    map,
                    &sidecars,
                    &active,
                    child,
                    &functions[ordinal],
                    true,
                    budget,
                )?;
            }
            let (child, ordinal) = children[0];
            let original = &functions[ordinal];
            let root = instances.root();
            probe_inline_callee_v30(
                map,
                &sidecars,
                &active,
                root,
                &functions[active.rows[root.index()].unwrap()],
                false,
                budget,
            )?;
            for fault in 0..7 {
                let mut changed = original.clone();
                let operation = inline_operation_mut_v30(&mut changed);
                if fault == 5 {
                    operation.kind = OperationKind::Constant(Constant::U32(42));
                } else if fault == 6 {
                    let mut extra = operation.clone();
                    extra.results[0].id = ValueId(1_000_000);
                    changed.body.as_mut().unwrap().blocks[0]
                        .operations
                        .push(extra);
                } else {
                    let OperationKind::InlineAssembly(assembly) = &mut operation.kind else {
                        unreachable!()
                    };
                    match fault {
                        0 => assembly.mnemonic = "v_sub_u32".into(),
                        1 => assembly.source.statement[0] ^= 1,
                        2 => {
                            assembly.operands[1].kind =
                                fe2o3_kernel_ir::AssemblyOperandKind::Input(ValueId(u32::MAX))
                        }
                        3 => {
                            assembly
                                .options
                                .insert(fe2o3_kernel_ir::AssemblyOption::Pure);
                        }
                        4 => {
                            assembly
                                .declared_effects
                                .insert(fe2o3_kernel_ir::AssemblyEffect::ReadGlobal);
                        }
                        _ => unreachable!(),
                    }
                }
                assert!(
                    probe_inline_callee_v30(
                        map, &sidecars, &active, child, &changed, false, budget
                    )
                    .is_err(),
                    "inline fault {fault} bypassed the scoped permit"
                );
                probe_inline_callee_v30(map, &sidecars, &active, child, original, true, budget)?;
            }
            let spans = map.spans.rows.clone();
            let target = spans
                .iter()
                .position(|row| {
                    row.instance == child
                        && matches!(row.source, InstanceSpanSourceV1::Terminator(span)
                    if span.semantic_block.index() == 0)
                })
                .unwrap();
            for fault in 0..6 {
                match fault {
                    0 => map.spans.rows[target].instance = children[1].0,
                    1 => map.spans.rows[target].segments = [None, None],
                    2 => map.spans.rows[target].segments[1] = spans[target].segments[0],
                    3 => {
                        map.spans.rows[target].removed_call =
                            Some(instances.incoming(child).unwrap().occurrence())
                    }
                    4 => {
                        let InstanceSpanSourceV1::Terminator(mut row) = spans[target].source else {
                            unreachable!()
                        };
                        row.semantic_block = SemanticBlockIdV1::from_index(1);
                        map.spans.rows[target].source = InstanceSpanSourceV1::Terminator(row);
                    }
                    5 => map.spans.rows[target].segments[0].as_mut().unwrap().count = 0,
                    _ => unreachable!(),
                }
                let result = probe_inline_callee_v30(
                    map, &sidecars, &active, child, original, false, budget,
                );
                map.spans.rows[target] = spans[target];
                assert!(
                    result.is_err(),
                    "transported source fault {fault} was admitted"
                );
                probe_inline_callee_v30(map, &sidecars, &active, child, original, true, budget)?;
            }
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
            foreign.reserve_storage(37).unwrap();
            assert_eq!(
                probe_inline_callee_v30(
                    map,
                    &sidecars,
                    &active,
                    child,
                    original,
                    false,
                    &mut foreign
                ),
                Err(CallInstanceEmissionErrorV1::Resource(
                    ArgumentResourceV1::Accounting
                ))
            );
            assert_eq!(foreign.storage(), 37);
            let seed = map
                .seeds
                .rows
                .iter()
                .position(|row| row.instance == child)
                .unwrap();
            let sibling = map
                .seeds
                .rows
                .iter()
                .position(|row| row.instance == children[1].0)
                .unwrap();
            let chain = map.seeds.rows[seed].inline;
            let first = chain.first.unwrap();
            let source = map.inline.rows[first];
            for fault in 0..5 {
                match fault {
                    0 => map.seeds.rows[seed].inline.first = None,
                    1 => map.seeds.rows[seed].inline.count = 0,
                    2 => map.seeds.rows[seed].inline = map.seeds.rows[sibling].inline,
                    3 => map.inline.rows[first].next = Some(first),
                    4 => map.inline.rows[first].span = usize::MAX,
                    _ => unreachable!(),
                }
                let result = probe_inline_callee_v30(
                    map, &sidecars, &active, child, original, false, budget,
                );
                map.seeds.rows[seed].inline = chain;
                map.inline.rows[first] = source;
                assert!(result.is_err(), "source index fault {fault} was admitted");
            }
            if SCOPED_INLINE_TEST_FAULT_V30.get() >= 3 {
                inline_permit_boundaries_v30(
                    map,
                    &sidecars,
                    &active,
                    child,
                    original,
                    SCOPED_INLINE_TEST_FAULT_V30.get() == 4,
                    budget,
                );
                return Ok(());
            }
            let mut expanded: Vec<_> = functions.into_iter().map(Some).collect();
            let mut joined = 0;
            for &(child, ordinal) in children.iter().rev() {
                let call = instances.incoming(child).unwrap();
                let caller = call.occurrence().caller;
                let caller_ordinal = active.rows[caller.index()].unwrap();
                let spliced = map.splice_with_scoped_parts_v29(
                    call,
                    expanded[caller_ordinal].take().unwrap(),
                    expanded[ordinal].take().unwrap(),
                    BlockId(next_block),
                    BlockId(next_block + 1),
                    None,
                    &sidecars,
                    &active,
                    budget,
                )?;
                next_block += 2;
                expanded[caller_ordinal] = Some(spliced.caller);
                joined += 1;
                assert_eq!(
                    map.seeds
                        .rows
                        .iter()
                        .find(|row| row.instance == caller)
                        .unwrap()
                        .inline
                        .count,
                    joined
                );
                assert_eq!(
                    map.seeds
                        .rows
                        .iter()
                        .find(|row| row.instance == child)
                        .unwrap()
                        .inline,
                    ScopedInlineChainV30::default()
                );
                probe_inline_callee_v30(
                    map,
                    &sidecars,
                    &active,
                    caller,
                    expanded[caller_ordinal].as_ref().unwrap(),
                    false,
                    budget,
                )?;
            }
            Ok(())
        })
        .map_err(pending_scope_correspondence_error_v29)
    })?;
    assert_eq!(budget.storage(), floor);
    Err(unsupported(0, None, None, SCOPED_INLINE_TEST_STOP_V30))
}

#[test]
fn scoped_inline_callee_permit_rejects_instruction_source_map_census_and_ledger_forgery() {
    SCOPED_INLINE_TEST_FAULT_V30.set(0);
    SCOPED_INLINE_TEST_VISITS_V30.set(0);
    let reset = ScopedInlineObserverResetV30(
        SCOPED_SLOT_OBSERVER_V29.replace(Some(inspect_inline_callees_v30)),
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(repeated_inline_owner_v18, &mut budget);
    let result =
        prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            panic!("inspection sentinel must stop before final consumer")
        });
    drop(reset);
    assert_eq!(SCOPED_INLINE_TEST_VISITS_V30.get(), 1);
    assert!(
        format!("{result:?}").contains(SCOPED_INLINE_TEST_STOP_V30),
        "{result:?}"
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[allow(clippy::too_many_arguments)]
fn inline_permit_boundaries_v30(
    map: &ProductionInstanceCorrespondenceV1<'_, '_>,
    sidecars: &[PendingInstanceSidecarsV29],
    active: &PendingActiveInstanceIndexV1,
    child: ProductionCallInstanceIdV1,
    function: &Function,
    short_work: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) {
    let floor = budget.storage();
    let available = budget.storage_limit() - floor;
    let filler = available.checked_sub(1_000_000).unwrap();
    let old_peak = budget.peak_storage();
    budget.reserve_storage(filler).unwrap();
    let entry = budget.storage();
    assert!(entry > old_peak);
    let before = budget.work();
    probe_inline_callee_v30(map, sidecars, active, child, function, false, budget).unwrap();
    let work = budget.work() - before;
    let peak = budget.peak_storage() - entry;
    assert!(work > 0 && peak > 0);
    budget.release_storage(filler).unwrap();
    for (allowance, success) in [(peak, true), (peak - 1, false)] {
        let filler = available - allowance;
        budget.reserve_storage(filler).unwrap();
        let result = probe_inline_callee_v30(map, sidecars, active, child, function, false, budget);
        match (result, success) {
            (Ok(()), true) => {}
            (
                Err(CallInstanceEmissionErrorV1::Resource(ArgumentResourceV1::Storage(error))),
                false,
            ) => {
                assert_eq!(error.limit(), budget.storage_limit());
                assert_eq!(error.actual(), budget.storage_limit() + 1);
            }
            (other, _) => panic!("inline storage boundary: {other:?}"),
        }
        assert_eq!(budget.storage(), floor + filler);
        budget.release_storage(filler).unwrap();
    }
    let allowance = work - usize::from(short_work);
    budget
        .charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - allowance)
        .unwrap();
    let result = probe_inline_callee_v30(map, sidecars, active, child, function, false, budget);
    match (result, short_work) {
        (Ok(()), false) => assert_eq!(budget.work(), OPTIMIZED_SOURCE_WORK_LIMIT_V18),
        (Err(CallInstanceEmissionErrorV1::Resource(ArgumentResourceV1::Work(error))), true) => {
            assert_eq!(error.limit(), OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            assert_eq!(error.actual(), OPTIMIZED_SOURCE_WORK_LIMIT_V18 + 1);
        }
        (other, _) => panic!("inline work boundary: {other:?}"),
    }
    assert_eq!(budget.storage(), floor);
}

#[test]
fn scoped_inline_callee_exact_resource_boundaries_restore_the_caller_floor() {
    for fault in [3, 4] {
        SCOPED_INLINE_TEST_FAULT_V30.set(fault);
        SCOPED_INLINE_TEST_VISITS_V30.set(0);
        let reset = ScopedInlineObserverResetV30(
            SCOPED_SLOT_OBSERVER_V29.replace(Some(inspect_inline_callees_v30)),
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(repeated_inline_owner_v18, &mut budget);
        let result =
            prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
                panic!("resource observer sentinel must stop before final consumer")
            });
        drop(reset);
        assert_eq!(SCOPED_INLINE_TEST_VISITS_V30.get(), 1);
        assert!(
            format!("{result:?}").contains(SCOPED_INLINE_TEST_STOP_V30),
            "{result:?}"
        );
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

fn corrupt_inline_emission_v30(
    _source: &ExecutionLifecycleSourceV29<'_>,
    _instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _receipt: &OwnedScopedSourceSlotsV29,
    _budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // Corrupt only the retained candidate. The required final reconstruction
    // must use the unchanged original source, not repeat this test mutation.
    if SCOPED_INLINE_TEST_VISITS_V30.get() != 0 {
        return Ok(());
    }
    let Some(lowered) = emitted.iter_mut().flatten().find(|row| {
        row.function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::InlineAssembly(_)))
    }) else {
        return Ok(());
    };
    SCOPED_INLINE_TEST_VISITS_V30.set(SCOPED_INLINE_TEST_VISITS_V30.get() + 1);
    let operation = inline_operation_mut_v30(&mut lowered.function);
    let OperationKind::InlineAssembly(assembly) = &mut operation.kind else {
        unreachable!()
    };
    match SCOPED_INLINE_TEST_FAULT_V30.get() {
        0 => assembly.operands[1] = assembly.operands[2].clone(),
        1 => assembly.source.contract[0] ^= 1,
        2 => operation.kind = OperationKind::Constant(Constant::U32(42)),
        _ => unreachable!(),
    }
    Ok(())
}

#[test]
fn scoped_inline_movement_keeps_final_source_operand_and_instruction_replay_required() {
    for fault in 0..3 {
        SCOPED_INLINE_TEST_FAULT_V30.set(fault);
        SCOPED_INLINE_TEST_VISITS_V30.set(0);
        let reset = ScopedInlineObserverResetV30(
            SCOPED_SLOT_OBSERVER_V29.replace(Some(corrupt_inline_emission_v30)),
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(repeated_inline_owner_v18, &mut budget);
        let called = std::cell::Cell::new(false);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |_, _| {
            called.set(true);
            Ok(())
        });
        drop(reset);
        assert_eq!(SCOPED_INLINE_TEST_VISITS_V30.get(), 1);
        assert!(
            result.is_err(),
            "source corruption {fault} reached optimized admission"
        );
        assert!(!called.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn scoped_inline_repeated_helpers_reach_genuine_optimized_admission() {
    run_optimized_source_v18(repeated_inline_owner_v18, |view, budget| {
        let inventory = view.output_inventory(budget)?;
        let actual = inventory
            .operations()
            .iter()
            .filter(|row| matches!(row.operation.kind, OperationKind::InlineAssembly(_)))
            .count();
        assert_eq!(
            actual, 2,
            "both distinct source instances survive actual optimized replay"
        );
        Ok(())
    });
}
