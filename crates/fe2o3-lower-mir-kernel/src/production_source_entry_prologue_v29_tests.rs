thread_local! {
    static ENTRY_PROLOGUE_HELPER_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static ENTRY_PROLOGUE_COMPONENTS_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ENTRY_PROLOGUE_CASE_V29: std::cell::Cell<(usize, usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0, 0)) };
    static ENTRY_PROLOGUE_MODE_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static ENTRY_PROLOGUE_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ENTRY_PROLOGUE_ROOT_ANCHORS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ENTRY_PROLOGUE_ROOT_WORK_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ENTRY_PROLOGUE_EXPECTED_V29: std::cell::Cell<Option<ArgumentResourceV1>> = const { std::cell::Cell::new(None) };
}

fn entry_prologue_source_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    if ENTRY_PROLOGUE_HELPER_V29.get() != 0 {
        return entry_prologue_helper_owner_v29(ENTRY_PROLOGUE_HELPER_V29.get() == 2);
    }
    if ENTRY_PROLOGUE_COMPONENTS_V29.get() {
        return entry_prologue_components_owner_v29();
    }
    let (arguments, entries, extra_locals, extra_anchors) = ENTRY_PROLOGUE_CASE_V29.get();
    assert!(entries <= arguments);
    assert!(arguments + entries + extra_locals + usize::from(extra_anchors != 0) < 50);
    let original = scalar_entry_original_owner_v29(SuffixCase::Root);
    if arguments + entries + extra_locals + extra_anchors == 0 {
        return original;
    }
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let raw = (entries != 0)
        .then(|| reference(&mut types, U32, SemanticMutabilityV1::Immutable, true));
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let mut inputs = root.abi().source_input_types().to_vec();
    let mut locals = root.locals().to_vec();
    let original_locals = locals.len();
    let mut before = Vec::new();
    let first_new = locals.len();
    for _ in 0..arguments {
        locals.push(local(
            200 + (locals.len() - original_locals) as u8,
            U32,
            SemanticLocalRoleV1::Argument(inputs.len() as u32),
        ));
        inputs.push(U32);
    }
    for index in 0..entries {
        let raw = raw.expect("entry dimension requires a pointer type");
        let pointer = locals.len() as u32;
        locals.push(local(
            200 + (locals.len() - original_locals) as u8,
            raw,
            SemanticLocalRoleV1::Temporary,
        ));
        before.push(assign(
            place(pointer, raw),
            SemanticRvalueKindV1::AddressOf {
                place: place((first_new + index) as u32, U32),
                mutability: SemanticMutabilityV1::Immutable,
            },
        ));
    }
    for _ in 0..extra_locals {
        let index = locals.len() as u32;
        locals.push(local(
            200 + (locals.len() - original_locals) as u8,
            U32,
            SemanticLocalRoleV1::Temporary,
        ));
        before.push(assign(
            place(index, U32),
            SemanticRvalueKindV1::Use(literal(5)),
        ));
    }
    if extra_anchors != 0 {
        let value = locals.len() as u32;
        locals.push(local(
            200 + (locals.len() - original_locals) as u8,
            U32,
            SemanticLocalRoleV1::Temporary,
        ));
        // Incoming local 1 already has typed backing. Actual reads grow its
        // anchor roster without adding parameters, slots, or entry stores.
        for _ in 0..extra_anchors {
            before.push(assign(
                place(value, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32))),
            ));
        }
    }
    let mut blocks = root.blocks().to_vec();
    before.extend_from_slice(blocks[0].statements());
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        before,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    functions[0] = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        abi(201, true, &inputs),
        locals,
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    scoped_root_tests::fixtures::build(types, functions, semantic.callables().to_vec())
}

fn inspect_entry_prologue_source_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    _: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(
        slots.retained_storage,
        slots.instances.capacity() * std::mem::size_of::<ScopedSourceSlotInstanceV29>()
            + slots.slots.capacity() * std::mem::size_of::<ScopedSourceSlotV29>()
    );
    OBSERVED.set(OBSERVED.get() + 1);
    Ok(())
}

fn inspect_entry_prologue_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.expect("genuine source plan").plan;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut constructors = 0;
    let mut helpers = 0;
    for owner in &slots.instances {
        let lowered = emitted[owner.instance.index()].as_ref().unwrap();
        let body = lowered.function.body.as_ref().unwrap();
        let mut entries = Vec::new();
        visit_scoped_slot_initializers_v29(
            instances,
            owner.instance,
            body.blocks[0].id,
            &slots.slots[owner.slots.clone()],
            budget,
            |index, _, location, _| {
                entries.push((owner.slots.start + index, location));
                Ok(())
            },
        )?;
        let Some(&(_, first)) = entries.first() else {
            continue;
        };
        let mode = ENTRY_PROLOGUE_MODE_V29.get();
        if matches!(mode, 9..=11) {
            let mut first_error = None;
            with_canonical_call_scratch_v1(budget, |budget| {
                let context =
                    SourceEntryPrologueV29::new(plan, owner.instance, lowered, first, budget)?;
                let local = SemanticLocalIdV1::from_index(
                    slots.slots[entries[0].0]
                        .origin
                        .identity
                        .original_local()
                        .unwrap(),
                );
                let query = SourceEntryQueryV29::Prologue(&context);
                query.parameter(plan, owner.instance, local, lowered, budget)?;
                query.object_anchor(plan, owner.instance, lowered, first, budget)?;
                let remaining = match mode {
                    9 => 10,
                    10 => 17,
                    11 => 20,
                    _ => unreachable!(),
                };
                budget.charge_work(LIMIT.checked_sub(budget.work() + remaining).unwrap())?;
                let context_floor = budget.storage();
                let peak = budget.peak_storage();
                let refused = match mode {
                    9 => context.check(plan, owner.instance, lowered, budget),
                    10 => query
                        .object_anchor(plan, owner.instance, lowered, first, budget)
                        .map(|_| ()),
                    11 => query
                        .parameter(plan, owner.instance, local, lowered, budget)
                        .map(|_| ()),
                    _ => unreachable!(),
                }
                .unwrap_err();
                let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(resource) =
                    refused
                else {
                    panic!("exact live query-work denial required: {refused:?}")
                };
                let ArgumentResourceV1::Work(error) = resource else {
                    panic!("Work required: {resource:?}")
                };
                assert_eq!((error.actual(), error.limit()), (LIMIT + 1, LIMIT));
                let unspent = match mode {
                    9 => 5,
                    10 => 6,
                    11 => 4,
                    _ => unreachable!(),
                };
                assert_eq!(budget.work(), LIMIT - unspent);
                assert_eq!(plan.failure.get(), Some(resource));
                assert_eq!(budget.failed_work(), Some(LIMIT + 1));
                let before = budget.work();
                for replay in [
                    context.check(plan, owner.instance, lowered, budget),
                    query
                        .object_anchor(plan, owner.instance, lowered, first, budget)
                        .map(|_| ()),
                    query
                        .parameter(plan, owner.instance, local, lowered, budget)
                        .map(|_| ()),
                    plan.charge(1, budget),
                ] {
                    assert!(
                        matches!(replay, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(actual)) if actual == resource)
                    );
                }
                assert_eq!(budget.work(), before);
                assert_eq!(
                    (budget.storage(), budget.peak_storage()),
                    (context_floor, peak)
                );
                assert_eq!(budget.failed_storage(), None);
                first_error = Some(resource);
                Ok(())
            })?;
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            ENTRY_PROLOGUE_EXPECTED_V29.set(first_error);
            ENTRY_PROLOGUE_VISITS_V29.set(ENTRY_PROLOGUE_VISITS_V29.get() + 1);
            return Ok(());
        }
        if matches!(mode, 6 | 7) {
            let (index, _) = entries[0];
            inspect_entry_prologue_live_exit_v29(
                mode,
                plan,
                owner.instance,
                lowered,
                &slots.slots[index],
                first,
                budget,
            )?;
            assert_eq!(budget.storage(), floor);
        }
        if mode == 8 {
            with_canonical_call_scratch_v1(budget, |budget| {
                let context =
                    SourceEntryPrologueV29::new(plan, owner.instance, lowered, first, budget)?;
                context.check(plan, owner.instance, lowered, budget)?;
                assert!(budget.storage() > floor);
                budget.release_storage(1)?;
                let denied = context
                    .check(plan, owner.instance, lowered, budget)
                    .unwrap_err();
                assert!(matches!(
                    denied,
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ));
                assert_eq!(plan.failure.get(), Some(ArgumentResourceV1::Accounting));
                let before = budget.work();
                let replay = context
                    .check(plan, owner.instance, lowered, budget)
                    .unwrap_err();
                assert!(matches!(
                    replay,
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ));
                assert_eq!(budget.work(), before);
                Ok(())
            })?;
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            ENTRY_PROLOGUE_EXPECTED_V29.set(Some(ArgumentResourceV1::Accounting));
            ENTRY_PROLOGUE_VISITS_V29.set(ENTRY_PROLOGUE_VISITS_V29.get() + 1);
            return Ok(());
        }
        if mode == 5 {
            with_canonical_call_scratch_v1(budget, |budget| {
                let context =
                    SourceEntryPrologueV29::new(plan, owner.instance, lowered, first, budget)?;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
                let before = budget.work();
                let denied = context
                    .check(plan, owner.instance, lowered, &mut foreign)
                    .unwrap_err();
                assert!(matches!(
                    denied,
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ));
                assert_eq!(
                    (foreign.work(), foreign.storage(), foreign.peak_storage()),
                    (0, 0, 0)
                );
                assert_eq!(
                    (foreign.failed_work(), foreign.failed_storage()),
                    (None, None)
                );
                assert_eq!(plan.failure.get(), Some(ArgumentResourceV1::Accounting));
                let replay = context
                    .check(plan, owner.instance, lowered, budget)
                    .unwrap_err();
                assert!(matches!(
                    replay,
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ));
                assert_eq!(budget.work(), before);
                Ok(())
            })?;
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            ENTRY_PROLOGUE_EXPECTED_V29.set(Some(ArgumentResourceV1::Accounting));
            ENTRY_PROLOGUE_VISITS_V29.set(ENTRY_PROLOGUE_VISITS_V29.get() + 1);
            return Ok(());
        }
        if matches!(mode, 2 | 3) {
            let first_header =
                source_reference_emission_headers_v29::<SourceEntryPrologueV29<'_>>()?;
            let second_header =
                source_reference_emission_headers_v29::<Option<SourceEntryPrologueV29<'_>>>()?;
            let padding = if mode == 3 {
                let remaining = first_header + second_header - 1;
                let padding = budget
                    .storage_limit()
                    .checked_sub(budget.storage() + remaining)
                    .unwrap();
                budget.reserve_storage(padding)?;
                padding
            } else {
                budget.charge_work(LIMIT.checked_sub(budget.work() + 1).unwrap())?;
                0
            };
            let padded_floor = budget.storage();
            let peak = budget.peak_storage();
            let denied = with_canonical_call_scratch_v1(budget, |budget| {
                let _context =
                    SourceEntryPrologueV29::new(plan, owner.instance, lowered, first, budget)?;
                Ok(())
            })
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))
            .unwrap_err();
            let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(resource) = denied
            else {
                panic!("exact prologue resource denial required: {denied:?}");
            };
            match resource {
                ArgumentResourceV1::Work(error) if mode == 2 => {
                    assert_eq!((error.actual(), error.limit()), (LIMIT + 1, LIMIT));
                    assert_eq!(budget.work(), LIMIT - 1);
                    assert_eq!(budget.peak_storage(), peak);
                }
                ArgumentResourceV1::Storage(error) if mode == 3 => {
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (budget.storage_limit() + 1, budget.storage_limit())
                    );
                    assert_eq!(budget.peak_storage(), peak.max(padded_floor + first_header));
                }
                _ => panic!("unexpected first resource: {resource:?}"),
            }
            assert_eq!(budget.storage(), padded_floor);
            assert_eq!(plan.failure.get(), Some(resource));
            budget.release_storage(padding)?;
            assert_eq!(budget.storage(), floor);
            let before_work = budget.work();
            let replay = with_canonical_call_scratch_v1(budget, |budget| {
                let _context =
                    SourceEntryPrologueV29::new(plan, owner.instance, lowered, first, budget)?;
                Ok(())
            })
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))
            .unwrap_err();
            assert!(
                matches!(replay, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(actual) if actual == resource)
            );
            // The outer scratch debit can itself fail after a work denial, but
            // source-plan replay must preserve the original transaction failure.
            assert_eq!(plan.failure.get(), Some(resource));
            assert_eq!(budget.work() - before_work, if mode == 2 { 0 } else { 2 });
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            ENTRY_PROLOGUE_EXPECTED_V29.set(Some(resource));
            ENTRY_PROLOGUE_VISITS_V29.set(ENTRY_PROLOGUE_VISITS_V29.get() + 1);
            return Ok(());
        }
        let mut constructor_work = None;
        let mut query_work = None;
        let mut peak = None;
        for _ in 0..2 {
            with_canonical_call_scratch_v1(budget, |budget| {
                let before = budget.work();
                let context = SourceEntryPrologueV29::new(plan, owner.instance, lowered, first, budget)?;
                let actual = budget.work() - before;
                if let Some(expected) = constructor_work { assert_eq!(actual, expected); }
                constructor_work = Some(actual);
                let context_floor = budget.storage();
                if ENTRY_PROLOGUE_HELPER_V29.get() != 0 && owner.instance != instances.root() {
                    assert_eq!(&context.parameters[..4], &[None, Some(0), None, Some(1)]);
                    if ENTRY_PROLOGUE_HELPER_V29.get() == 2 { assert_eq!(context.parameters[4], None); }
                    assert_eq!(body.parameters.len(), 2);
                    assert_ne!(body.parameters[0], body.parameters[1]);
                }
                if ENTRY_PROLOGUE_COMPONENTS_V29.get() && owner.instance == instances.root() {
                    let locals = instances.instance(owner.instance).unwrap().declaration().locals().len();
                    assert_eq!(context.parameters[locals - 4], None);
                    assert_eq!(context.parameters[locals - 3], None);
                    assert_eq!(context.parameters[locals - 2], Some(body.parameters.len() - 1));
                    assert_eq!(body.parameters.len(), 4);
                }
                if mode == 4 {
                    inspect_entry_prologue_anchor_index_v29(lowered, first, &context, plan, owner.instance, budget)?;
                }
                for &(index, location) in &entries {
                    let slot = &slots.slots[index];
                    let local = SemanticLocalIdV1::from_index(slot.origin.identity.original_local().unwrap());
                    let before = budget.work();
                    let parameter = SourceEntryQueryV29::Prologue(&context)
                        .parameter(plan, owner.instance, local, lowered, budget)?;
                    let actual = budget.work() - before;
                    if let Some(expected) = query_work { assert_eq!(actual, expected); }
                    query_work = Some(actual);
                    let operation = &body.blocks[0].operations[location.operation];
                    match slot.representation {
                        ScopedSlotRepresentationV29::Object { .. } => {
                            source_reference_object_entry_store_with_prologue_v29(
                                SourceEntryQueryV29::Prologue(&context), plan, owner.instance, slot, lowered,
                                location, operation, budget)?;
                            assert!(matches!(operation.kind, OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value, .. }) if value == parameter));
                        }
                        ScopedSlotRepresentationV29::ScalarArray(_) => {
                            source_reference_retained_scalar_entry_store_with_prologue_v29(
                                SourceEntryQueryV29::Prologue(&context), plan, owner.instance, slot.origin,
                                lowered, operation, budget)?;
                        }
                    }
                    assert_eq!(budget.storage(), context_floor);
                }
                assert!(SourceEntryQueryV29::Prologue(&context).parameter(
                    plan, owner.instance, SemanticLocalIdV1::from_index(0), lowered, budget).is_err());
                if mode == 1 {
                    let function = instances.instance(owner.instance).unwrap().function();
                    let (foreign_index, foreign) = emitted.iter().enumerate()
                        .find(|(index, row)| *index != owner.instance.index() && row.is_some()
                            && instances.instance(instances.id_at(*index).unwrap()).unwrap().function() == function)
                        .or_else(|| emitted.iter().enumerate()
                            .find(|(index, row)| *index != owner.instance.index() && row.is_some())).unwrap();
                    let foreign_instance = instances.id_at(foreign_index).unwrap();
                    if owner.instance != instances.root() {
                        assert_eq!(instances.instance(foreign_instance).unwrap().function(), function);
                    }
                    let foreign = foreign.as_ref().unwrap();
                    assert!(context.check(plan, foreign_instance, lowered, budget).is_err());
                    assert!(context.check(plan, owner.instance, foreign, budget).is_err());
                    assert!(context.check(plan, foreign_instance, foreign, budget).is_err());
                    context.check(plan, owner.instance, lowered, budget)?;
                    assert_eq!(plan.failure.get(), None);
                }
                Ok(())
            }).inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
            assert_eq!(budget.storage(), floor);
            if let Some(expected) = peak {
                assert_eq!(budget.peak_storage(), expected);
            }
            peak = Some(budget.peak_storage());
        }
        constructors += 1;
        if owner.instance == instances.root() {
            ENTRY_PROLOGUE_ROOT_WORK_V29.set(constructor_work.unwrap());
            ENTRY_PROLOGUE_ROOT_ANCHORS_V29.set(lowered.scoped_memory_anchors.as_ref().unwrap().rows.len());
            assert_eq!(entries.len(), 1 + ENTRY_PROLOGUE_CASE_V29.get().1);
        } else {
            helpers += 1;
            assert_eq!(entries.len(), 2);
        }
    }
    assert_eq!((constructors, helpers), (3, 2));
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!((budget.storage(), plan.failure.get()), (floor, None));
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
    ENTRY_PROLOGUE_VISITS_V29.set(ENTRY_PROLOGUE_VISITS_V29.get() + 1);
    Ok(())
}

fn run_entry_prologue_v29(mode: u8, case: (usize, usize, usize, usize)) -> (usize, usize, usize) {
    struct Restore(
        Option<ScopedSlotCustodyObserverV29>,
        u8,
        (usize, usize, usize, usize),
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            ENTRY_PROLOGUE_MODE_V29.set(self.1);
            ENTRY_PROLOGUE_CASE_V29.set(self.2);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(inspect_entry_prologue_v29)),
        ENTRY_PROLOGUE_MODE_V29.replace(mode),
        ENTRY_PROLOGUE_CASE_V29.replace(case),
    );
    ENTRY_PROLOGUE_VISITS_V29.set(0);
    ENTRY_PROLOGUE_ROOT_WORK_V29.set(0);
    ENTRY_PROLOGUE_ROOT_ANCHORS_V29.set(0);
    ENTRY_PROLOGUE_EXPECTED_V29.set(None);
    let (result, work, peak, completed) = run_original_source_fixture_v29(
        entry_prologue_source_owner_v29,
        false,
        true,
        3,
        inspect_entry_prologue_source_v29,
        LIMIT,
        LIMIT,
    );
    if !matches!(mode, 2 | 3 | 5 | 8..=11) {
        result.unwrap();
        assert!(completed);
        assert_eq!((ENTRY_PROLOGUE_VISITS_V29.get(), OBSERVED.get()), (3, 3));
        assert_eq!(ENTRY_PROLOGUE_EXPECTED_V29.get(), None);
    } else {
        assert!(!completed);
        assert_eq!((ENTRY_PROLOGUE_VISITS_V29.get(), OBSERVED.get()), (1, 1));
        assert_eq!(
            original_repeated_source_resource_v29(result.unwrap_err()),
            ENTRY_PROLOGUE_EXPECTED_V29
                .get()
                .expect("completed exact resource assertions")
        );
    }
    (ENTRY_PROLOGUE_ROOT_WORK_V29.get(), work, peak)
}

#[test]
fn original_entry_prologue_reuses_root_and_repeated_helper_rosters_with_exact_cleanup() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
}

#[test]
fn original_entry_prologue_rejects_foreign_instance_and_candidate_then_recovers() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    run_entry_prologue_v29(1, (0, 0, 0, 0));
}

#[test]
fn original_entry_prologue_source_scale_is_affine_in_arguments_entries_locals_and_anchors() {
    for dimension in 0..4 {
        let mut measured = Vec::new();
        let mut anchors = Vec::new();
        for count in [4usize, 8, 12] {
            let case = match dimension {
                0 => (count, 0, 0, 0),
                1 => (12, count, 0, 0),
                2 => (0, 0, count, 0),
                3 => (0, 0, 0, count),
                _ => unreachable!(),
            };
            measured.push(run_entry_prologue_v29(0, case).0);
            anchors.push(ENTRY_PROLOGUE_ROOT_ANCHORS_V29.get());
        }
        if dimension == 3 {
            assert!(anchors[0] < anchors[1], "actual anchor roster: {anchors:?}");
            assert_eq!(anchors[2] - anchors[1], anchors[1] - anchors[0]);
            assert_eq!(measured[1] - measured[0], 8 * (anchors[1] - anchors[0]));
        }
        assert!(
            measured[0] < measured[1],
            "dimension {dimension}: {measured:?}"
        );
        assert_eq!(
            measured[2] - measured[1],
            measured[1] - measured[0],
            "only lexical constructor work, not whole-pipeline complexity: dimension {dimension}: {measured:?}"
        );
    }
}

#[test]
fn original_entry_prologue_scope_work_denial_preserves_first_error_and_caller_floor() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    run_entry_prologue_v29(2, (0, 0, 0, 0));
}

#[test]
fn original_entry_prologue_foreign_ledger_is_uncharged_and_first_accounting_error_replays() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    run_entry_prologue_v29(5, (0, 0, 0, 0));
}

#[test]
fn original_entry_prologue_partial_header_storage_denial_preserves_first_error_and_caller_floor() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    run_entry_prologue_v29(3, (0, 0, 0, 0));
}

fn inspect_entry_prologue_anchor_index_v29(
    lowered: &LoweredFunctionResultV1,
    first: PrivateArrayPhysicalLocationV1,
    context: &SourceEntryPrologueV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let body = lowered.function.body.as_ref().unwrap();
    let source = lowered.scoped_memory_anchors.as_ref().unwrap();
    let original = *context.anchor(plan, instance, lowered, first, None, budget)?;
    assert_eq!(original.source, None);
    let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
        address,
        value,
        access,
    }) = body.blocks[0].operations[first.operation].kind
    else {
        panic!("genuine typed entry")
    };
    let end = first.operation + context.anchors.len();
    let floor = budget.storage();
    let check = |rows: &[ScopedMemoryAnchorV29],
                 operations: &[Operation],
                 budget: &mut ArgumentBudgetV1<'_>| {
        let mut facts = None;
        with_canonical_call_scratch_v1(budget, |budget| {
            let index = source_entry_anchor_index_v29(first, end, operations, rows, budget)?;
            facts = Some(index[0]);
            Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
        Ok::<_, ProductionSemanticKirErrorV1>(facts.unwrap())
    };
    let original_index = check(&source.rows, &body.blocks[0].operations, budget)?;
    assert!(original_index.any.is_some() && !original_index.any_duplicate);

    let mut rows = source.rows.clone();
    rows.push(original);
    let duplicate = check(&rows, &body.blocks[0].operations, budget)?;
    assert!(duplicate.any_duplicate);
    rows.retain(|row| row.block != first.block || row.position != first.operation);
    let missing = check(&rows, &body.blocks[0].operations, budget)?;
    assert!(missing.any.is_none() && !missing.any_duplicate);
    let mut swapped = original;
    swapped.position = end;
    rows.push(swapped);
    assert!(
        check(&rows, &body.blocks[0].operations, budget)?
            .any
            .is_none()
    );
    rows.push(original);
    rows.reverse();
    let unsorted = check(&rows, &body.blocks[0].operations, budget)?;
    assert!(unsorted.any.is_some() && !unsorted.any_duplicate);

    // Test the legacy predicate against an actual Store form, retaining the
    // genuine address/value/access. Other same-gap kinds and pointers are not
    // duplicates of its Access row.
    let mut operations = body.blocks[0].operations.clone();
    operations[first.operation].kind = OperationKind::Store {
        pointer: address,
        value,
        access,
    };
    let access_row = ScopedMemoryAnchorV29 {
        kind: ScopedMemoryAnchorKindV29::Access {
            pointer: address,
            payload: None,
        },
        ..original
    };
    let unrelated = ScopedMemoryAnchorV29 {
        kind: ScopedMemoryAnchorKindV29::Access {
            pointer: value,
            payload: None,
        },
        ..original
    };
    assert_ne!(address, value);
    let gap = ScopedMemoryAnchorV29 {
        kind: ScopedMemoryAnchorKindV29::FailureRead { event: 0, local: 0 },
        ..original
    };
    let rows = [unrelated, access_row, gap];
    let legacy = check(&rows, &operations, budget)?;
    assert!(legacy.any_duplicate);
    assert_eq!(legacy.access, Some(1));
    assert!(!legacy.access_duplicate);
    let missing = check(&[unrelated, gap], &operations, budget)?;
    assert!(missing.access.is_none() && !missing.access_duplicate);
    assert!(check(&[access_row, gap, access_row], &operations, budget)?.access_duplicate);

    let mut deltas = Vec::new();
    for copies in [4usize, 8, 12] {
        let rows = vec![swapped; copies];
        let before = budget.work();
        let facts = check(&rows, &operations, budget)?;
        assert!(facts.any.is_none() && facts.access.is_none());
        deltas.push(budget.work() - before);
    }
    assert_eq!(deltas[2] - deltas[1], 8 * 4);
    assert_eq!(deltas[1] - deltas[0], 8 * 4);
    assert_eq!(budget.storage(), floor);
    assert_eq!(plan.failure.get(), None);
    Ok(())
}

#[test]
fn original_entry_prologue_anchor_index_preserves_missing_duplicate_swapped_and_same_gap_rules() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    run_entry_prologue_v29(4, (0, 0, 0, 0));
}

#[test]
fn original_entry_prologue_rebuild_after_observer_rejects_stale_values_and_positions() {
    for fault in [1, 9] {
        let (positive, _, _, completed) = run_scalar_entries_v29(SuffixCase::Root, 0, LIMIT, LIMIT);
        positive.unwrap();
        assert!(completed);
        let (refused, _, _, completed) =
            run_scalar_entries_v29(SuffixCase::Root, fault, LIMIT, LIMIT);
        assert!(
            refused.is_err(),
            "fresh mandatory rederivation must reject fault {fault}"
        );
        assert!(!completed);
        assert_eq!(SCALAR_ENTRY_VISITS_V29.get(), 1);
        assert!(SCALAR_ENTRY_MUTATIONS_V29.get() >= 1);
    }
}

fn entry_prologue_components_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let original = scalar_entry_original_owner_v29(SuffixCase::Root);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let array = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            4,
            SemanticFieldsShapeV1::array(4, 2),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: U32,
            length: 2,
        },
        None,
    );
    let raw = reference(&mut types, U32, SemanticMutabilityV1::Immutable, true);
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let prior = root.abi();
    assert_eq!(prior.source_input_types(), &[U32]);
    let mut arguments = prior.arguments().to_vec();
    let mode = SemanticAbiPassModeV1::Indirect {
        attributes: SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesNone),
                true,
                false,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            8,
            Some(4),
        )
        .unwrap(),
        metadata_attributes: None,
        on_stack: false,
    };
    arguments.extend([
        SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(array, mode)),
        SemanticAbiArgumentV1::source(ignored(UNIT)),
        SemanticAbiArgumentV1::source(direct(U32)),
    ]);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([218; 32]),
        prior.layout_identity(),
        prior.canon_abi(),
        prior.extern_abi(),
        prior.c_variadic(),
        prior.can_unwind(),
        prior.fixed_count() + 3,
        arguments,
        prior.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 4])
    .unwrap();
    let mut locals = root.locals().to_vec();
    let scalar = locals.len() as u32 + 2;
    let pointer = scalar + 1;
    locals.extend([
        local(200, array, SemanticLocalRoleV1::Argument(1)),
        local(201, UNIT, SemanticLocalRoleV1::Argument(2)),
        local(202, U32, SemanticLocalRoleV1::Argument(3)),
        local(203, raw, SemanticLocalRoleV1::Temporary),
    ]);
    let mut blocks = root.blocks().to_vec();
    let mut statements = vec![assign(
        place(pointer, raw),
        SemanticRvalueKindV1::AddressOf {
            place: place(scalar, U32),
            mutability: SemanticMutabilityV1::Immutable,
        },
    )];
    statements.extend_from_slice(blocks[0].statements());
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        statements,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    functions[0] = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        abi,
        locals,
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    scoped_root_tests::fixtures::build(types, functions, semantic.callables().to_vec())
}

#[test]
fn original_entry_prologue_advances_unqueried_array_and_zero_width_root_components() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENTRY_PROLOGUE_COMPONENTS_V29.set(self.0);
        }
    }
    let _restore = Restore(ENTRY_PROLOGUE_COMPONENTS_V29.replace(true));
    run_entry_prologue_v29(0, (1, 1, 0, 0));
}

fn entry_prologue_helper_owner_v29(rust_call: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = scalar_entry_original_owner_v29(SuffixCase::Root);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let raw = reference(&mut types, U32, SemanticMutabilityV1::Immutable, true);
    let unit = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ))
    };
    let mut functions = semantic.functions().to_vec();
    let tuple = if rust_call {
        let SemanticBackendReprV1::Scalar(scalar) =
            *types[U32.index() as usize].layout().backend_repr()
        else {
            panic!("u32 fixture scalar")
        };
        Some(declaration(
            &mut types,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                4,
                SemanticBackendReprV1::scalar_pair(scalar, scalar),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 4, 4], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, UNIT, U32]).unwrap()),
            None,
        ))
    } else {
        None
    };
    let abi = if let Some(tuple) = tuple {
        SemanticFunctionAbiV1::from_rustc_with_source_signature(
            SemanticAbiIdentityV1::from_sha256([228; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::RustCall,
            false,
            false,
            1,
            vec![UNIT, tuple],
            U32,
            vec![
                SemanticAbiArgumentV1::source(ignored(UNIT)),
                SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(U32)),
                SemanticAbiArgumentV1::rust_call_tuple_field(1, ignored(UNIT)),
                SemanticAbiArgumentV1::rust_call_tuple_field(2, direct(U32)),
            ],
            direct(U32),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
        .unwrap()
    } else {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([228; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            3,
            vec![
                SemanticAbiArgumentV1::source(direct(U32)),
                SemanticAbiArgumentV1::source(ignored(UNIT)),
                SemanticAbiArgumentV1::source(direct(U32)),
            ],
            direct(U32),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 3])
        .unwrap()
    };
    let role = |field| {
        if rust_call {
            SemanticLocalRoleV1::RustCallTupleField { argument: 1, field }
        } else {
            SemanticLocalRoleV1::Argument(field)
        }
    };
    let mut locals = vec![
        local(132, U32, SemanticLocalRoleV1::Return),
        local(133, U32, role(2)),
        local(134, UNIT, role(1)),
        local(135, U32, role(0)),
    ];
    if rust_call {
        locals.push(local(136, UNIT, SemanticLocalRoleV1::Argument(0)));
    }
    let pointer = locals.len() as u32;
    locals.push(local(137, raw, SemanticLocalRoleV1::Temporary));
    locals.push(local(138, raw, SemanticLocalRoleV1::Temporary));
    let helper = &functions[3];
    let helper_blocks = vec![block(
        140,
        vec![
            assign(
                place(pointer, raw),
                SemanticRvalueKindV1::AddressOf {
                    place: place(1, U32),
                    mutability: SemanticMutabilityV1::Immutable,
                },
            ),
            assign(
                place(pointer + 1, raw),
                SemanticRvalueKindV1::AddressOf {
                    place: place(3, U32),
                    mutability: SemanticMutabilityV1::Immutable,
                },
            ),
            store(place(1, U32), SemanticOperandV1::Copy(place(1, U32))),
            store(place(3, U32), SemanticOperandV1::Copy(place(3, U32))),
            assign(
                place(0, U32),
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Subtract,
                    left: SemanticOperandV1::Copy(place(3, U32)),
                    right: SemanticOperandV1::Copy(place(1, U32)),
                },
            ),
        ],
        SemanticTerminatorKindV1::Return,
    )];
    functions[3] = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        helper.source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        helper_blocks,
    )
    .unwrap();
    for function in &mut functions[..3] {
        let mut locals = function.locals().to_vec();
        let mut blocks = Vec::new();
        let mut changed = false;
        for (ordinal, source) in function.blocks().iter().enumerate() {
            let SemanticTerminatorKindV1::Call(call) = source.terminator().kind() else {
                blocks.push(source.clone());
                continue;
            };
            if call.callee().index() != 3 {
                blocks.push(source.clone());
                continue;
            }
            assert_eq!(call.arguments().len(), 2);
            let mut statements = source.statements().to_vec();
            let arguments = if let Some(tuple) = tuple {
                let local_index = locals.len() as u32;
                locals.push(local(
                    210 + ordinal as u8,
                    tuple,
                    SemanticLocalRoleV1::Temporary,
                ));
                statements.push(assign(
                    place(local_index, tuple),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Tuple,
                            vec![
                                call.arguments()[0].clone(),
                                unit(),
                                call.arguments()[1].clone(),
                            ],
                        )
                        .unwrap(),
                    ),
                ));
                vec![unit(), SemanticOperandV1::Move(place(local_index, tuple))]
            } else {
                vec![
                    call.arguments()[0].clone(),
                    unit(),
                    call.arguments()[1].clone(),
                ]
            };
            let terminator = SemanticTerminatorV1::new(
                source.terminator().source(),
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        call.callee(),
                        arguments,
                        call.destination().cloned(),
                        call.unwind(),
                    )
                    .unwrap(),
                ),
            );
            blocks.push(
                SemanticBasicBlockV1::new(
                    source.identity(),
                    source.source(),
                    statements,
                    terminator,
                )
                .unwrap(),
            );
            changed = true;
        }
        if changed {
            let mut rebuilt = SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                function.abi().clone(),
                locals,
                function.entry(),
                blocks,
            )
            .unwrap();
            if let Some(entry) = function.kernel_entry() {
                rebuilt = rebuilt.with_kernel_entry(entry.clone());
            }
            *function = rebuilt;
        }
    }
    scoped_root_tests::fixtures::build(types, functions, semantic.callables().to_vec())
}

#[test]
fn original_entry_prologue_helper_cursor_preserves_interleaved_zero_width_arguments() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    struct Restore(u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENTRY_PROLOGUE_HELPER_V29.set(self.0);
        }
    }
    let _restore = Restore(ENTRY_PROLOGUE_HELPER_V29.replace(1));
    run_entry_prologue_v29(0, (0, 0, 0, 0));
}

#[test]
fn original_entry_prologue_helper_cursor_preserves_reversed_rust_call_fields_and_zero_width_groups()
{
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    struct Restore(u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENTRY_PROLOGUE_HELPER_V29.set(self.0);
        }
    }
    let _restore = Restore(ENTRY_PROLOGUE_HELPER_V29.replace(2));
    run_entry_prologue_v29(0, (0, 0, 0, 0));
}

fn inspect_entry_prologue_live_exit_v29(
    mode: u8,
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    slot: &ScopedSourceSlotV29,
    location: PrivateArrayPhysicalLocationV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let before = budget.work();
    let operation =
        &lowered.function.body.as_ref().unwrap().blocks[0].operations[location.operation];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_canonical_call_scratch_v1(budget, |budget| {
            let context = SourceEntryPrologueV29::new(plan, instance, lowered, location, budget)?;
            source_reference_object_entry_store_with_prologue_v29(
                SourceEntryQueryV29::Prologue(&context),
                plan,
                instance,
                slot,
                lowered,
                location,
                operation,
                budget,
            )?;
            if mode == 6 {
                SourceEntryQueryV29::Prologue(&context).object_anchor(
                    plan,
                    instance,
                    lowered,
                    PrivateArrayPhysicalLocationV1 {
                        operation: usize::MAX,
                        ..location
                    },
                    budget,
                )?;
                panic!("impossible initializer coordinate accepted");
            }
            panic!("test unwind while exact prologue context is live");
            #[allow(unreachable_code)]
            Ok::<(), ProductionSemanticKirErrorV1>(())
        })
    }));
    if mode == 6 {
        let error = result
            .expect("semantic refusal must not unwind")
            .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "typed object source payload differs from its actual operation",
                ..
            }
        ));
    } else {
        assert_eq!(mode, 7);
        let payload = result.expect_err("the live-context unwind must actually run");
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"test unwind while exact prologue context is live")
        );
    }
    assert!(budget.work() > before);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
    assert_eq!(plan.failure.get(), None);
    Ok(())
}

#[test]
fn original_entry_prologue_live_semantic_failure_drops_context_and_recovers() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    run_entry_prologue_v29(6, (0, 0, 0, 0));
}

#[test]
fn original_entry_prologue_live_unwind_drops_context_and_recovers() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    run_entry_prologue_v29(7, (0, 0, 0, 0));
}

#[test]
fn original_entry_prologue_retained_floor_loss_refuses_and_replays_first_accounting_error() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    run_entry_prologue_v29(8, (0, 0, 0, 0));
}

#[test]
fn original_entry_prologue_live_query_work_denials_latch_before_swallowed_callback_replay() {
    run_entry_prologue_v29(0, (0, 0, 0, 0));
    for mode in 9..=11 {
        run_entry_prologue_v29(mode, (0, 0, 0, 0));
    }
}
