// Inert equation fixtures only. These tests do not authenticate a source birth
// or an optimizer rewrite; the optimized transport producer must do that.
fn logical_alias_equation_run_v29(
    selected: usize,
    duplicate_edges: bool,
    holder_after_restart: bool,
    fault: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    let (mut function, slots, layouts) = typed_currentness_fixture();
    let block = &mut function.body.as_mut().unwrap().blocks[0];
    block.operations.remove(4);
    let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value, .. }) = &mut block.operations[4].kind
        else { unreachable!() };
    *value = A;
    let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { address, .. }) = &mut block.operations[6].kind
        else { unreachable!() };
    *address = A;
    let mut accesses = accesses();
    for access in &mut accesses { access.operation -= 1; }
    let mut lifetimes = [
        SourceAddressLifetimeV29 { block: BlockId(77), gap: 6, sequence: 0, slot: 0, live: false },
        SourceAddressLifetimeV29 { block: BlockId(77), gap: 6, sequence: 1, slot: 0, live: true },
    ];
    let mut aliases = vec![
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0, recipe: SourceAddressAliasRecipeV29::Birth },
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0, recipe: SourceAddressAliasRecipeV29::Birth },
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0, recipe: SourceAddressAliasRecipeV29::Birth },
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0,
            recipe: SourceAddressAliasRecipeV29::Inherit(SourceAddressAliasInputV29::Logical(1)) },
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0,
            recipe: SourceAddressAliasRecipeV29::Inherit(SourceAddressAliasInputV29::Logical(2)) },
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0,
            recipe: SourceAddressAliasRecipeV29::Select(
                SourceAddressAliasInputV29::Logical(1), SourceAddressAliasInputV29::Logical(2)) },
    ];
    let mut events = vec![SourceAddressBoundaryEventV29 {
        block: BlockId(77), gap: 4, kind: SourceAddressBoundaryKindV29::Alias(0),
    }];
    events.extend([
        SourceAddressBoundaryKindV29::Alias(1), SourceAddressBoundaryKindV29::Lifetime(0),
        SourceAddressBoundaryKindV29::Lifetime(1),
        SourceAddressBoundaryKindV29::Alias(2), SourceAddressBoundaryKindV29::Alias(3),
        SourceAddressBoundaryKindV29::Alias(4), SourceAddressBoundaryKindV29::Alias(5),
    ].map(|kind| SourceAddressBoundaryEventV29 { block: BlockId(77), gap: 6, kind }));
    let mut uses = vec![
        SourceAddressLogicalUseV29 { block: BlockId(77), operation: Some(4), successor: None,
            operand: 1, value: A, alias: 0 },
        SourceAddressLogicalUseV29 { block: BlockId(77), operation: Some(6), successor: None,
            operand: 0, value: A, alias: selected },
    ];
    if holder_after_restart {
        assert!(!duplicate_edges);
        for row in &mut lifetimes { row.gap = 4; }
        for row in &mut events { row.gap = 4; }
        uses.truncate(1);
        uses[0].alias = selected;
        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { address, .. }) = &mut block.operations[6].kind
            else { unreachable!() };
        *address = LOADED;
    }
    if duplicate_edges {
        let mut store = block.operations.pop().unwrap();
        let pointer_type = block.operations[5].results[0].ty.clone();
        let parameter = ValueId(90);
        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { address, .. }) = &mut store.kind
            else { unreachable!() };
        *address = parameter;
        block.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(40), then_target: BlockId(88), then_arguments: vec![A],
            else_target: BlockId(88), else_arguments: vec![A],
        });
        function.body.as_mut().unwrap().blocks.push(BasicBlock {
            id: BlockId(88), parameters: vec![ValueDef::new(parameter, pointer_type)],
            operations: vec![store], terminator: Some(Terminator::Return { values: vec![] }),
        });
        accesses[2].block = BlockId(88);
        accesses[2].operation = 0;
        uses[1] = SourceAddressLogicalUseV29 { block: BlockId(77), operation: None, successor: Some(0),
            operand: 0, value: A, alias: 2 };
        uses.push(SourceAddressLogicalUseV29 { block: BlockId(77), operation: None, successor: Some(1),
            operand: 0, value: A, alias: selected });
    }
    match fault {
        0 => {}
        1 => { events[1].kind = SourceAddressBoundaryKindV29::Alias(0); }
        2 => { uses[1].operand = 7; }
        3 => { uses[1].value = CELL; }
        4 => { aliases[2].slot = 1; }
        5 => { aliases[2].recipe = SourceAddressAliasRecipeV29::Inherit(SourceAddressAliasInputV29::Physical(CELL)); }
        6 => { events.swap(2, 3); }
        _ => unreachable!(),
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            &function, &slots, None, &accesses, &layouts, budget)?.solve(&slots, &accesses, &[], budget)?;
        let floor = budget.storage();
        check_source_address_currentness_transport_v29(&function, &graph, &slots, &accesses, &[],
            &[true; 3], &lifetimes, &[], &[], SourceAddressGeometryV29::Scalar,
            SourceAddressAliasTransportV29 { aliases: &aliases, uses: &uses, boundaries: Some(&events) }, budget)?;
        assert_eq!(budget.storage(), floor);
        completed = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn logical_alias_events_do_not_revive_saved_aliases_at_a_collapsed_restart_gap() {
    for selected in [2, 4] {
        let (result, _, _, completed) = logical_alias_equation_run_v29(selected, false, false, 0, LIMIT, LIMIT);
        result.unwrap();
        assert!(completed);
    }
    for selected in [0, 1, 3, 5] {
        let (result, _, _, completed) = logical_alias_equation_run_v29(selected, false, false, 0, LIMIT, LIMIT);
        assert!(!completed);
        assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
            if detail == "physical raw access crosses a storage activation or unresolved alias"));
    }
}

#[test]
fn logical_alias_output_edge_occurrences_remain_distinct_for_duplicate_destinations() {
    for selected in [1, 2] {
        let (result, _, _, completed) = logical_alias_equation_run_v29(selected, true, false, 0, LIMIT, LIMIT);
        if selected == 2 { result.unwrap(); assert!(completed); }
        else { unsupported(result); assert!(!completed); }
    }
}

#[test]
fn logical_alias_equations_reject_orphan_duplicate_wrong_role_and_cross_object_rows() {
    for fault in 1..=6 {
        let (result, _, _, completed) = logical_alias_equation_run_v29(2, false, false, fault, LIMIT, LIMIT);
        unsupported(result);
        assert!(!completed);
    }
}

#[test]
fn logical_alias_equations_have_exact_and_one_short_owned_resource_boundaries() {
    let (result, work, storage, completed) = logical_alias_equation_run_v29(2, true, false, 0, LIMIT, LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, actual_work, actual_storage, completed) = logical_alias_equation_run_v29(2, true, false, 0, work, storage);
    result.unwrap();
    assert!(completed);
    assert_eq!((actual_work, actual_storage), (work, storage));
    for (work_limit, storage_limit, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let (result, _, _, _) = logical_alias_equation_run_v29(2, true, false, 0, work_limit, storage_limit);
        let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = result else {
            panic!("exact resource refusal required: {result:?}");
        };
        if is_work { assert!(matches!(error, ArgumentResourceV1::Work(_))); }
        else { assert!(matches!(error, ArgumentResourceV1::Storage(_))); }
    }
}

#[test]
fn logical_alias_store_payload_currentness_survives_real_holder_loads() {
    for selected in [1, 2] {
        let (result, _, _, completed) = logical_alias_equation_run_v29(selected, false, true, 0, LIMIT, LIMIT);
        if selected == 2 { result.unwrap(); assert!(completed); }
        else {
            assert!(!completed);
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == "physical raw access crosses a storage activation or unresolved alias"));
        }
    }
}

#[test]
fn logical_alias_currentness_header_matches_independent_live_shapes() {
    type Transport<'a> = (&'a [SourceAddressLogicalAliasV29], &'a [SourceAddressLogicalUseV29],
        Option<&'a [SourceAddressBoundaryEventV29]>);
    type Currentness<'a, 'g> = (&'a SourceAddressMemoryV29<'g>, Vec<Option<usize>>,
        Vec<(usize, bool)>, [usize; 3], Transport<'a>);
    let expected = std::mem::size_of::<Currentness<'_, '_>>()
        + std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>()
        + 4 * std::mem::size_of::<Vec<usize>>()
        + std::mem::size_of::<[usize; 3]>()
        + std::mem::size_of::<[usize; 4]>()
        + std::mem::size_of::<Result<usize, usize>>();
    assert_eq!(source_address_currentness_headers_v29().unwrap(), expected);
    for limit in [expected, expected - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        let result = budget.reserve_storage(source_address_currentness_headers_v29().unwrap());
        if limit == expected {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        } else {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(_))));
        }
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.work(), 0);
    }
}

// The producer canonicalizes only clears at one checked output gap. This
// independent equation fixture checks that rule against the actual engine,
// including a holder Store/Load and stale-vs-fresh logical payloads.
fn logical_alias_gap_kill_run_v29(selected: usize, position: usize, after_store: bool)
    -> Result<(), ProductionSemanticKirErrorV1>
{
    let (mut function, slots, layouts) = typed_currentness_fixture();
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    operations.remove(4);
    let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value, .. }) = &mut operations[4].kind
        else { unreachable!() };
    *value = A;
    let mut accesses = accesses();
    for row in &mut accesses { row.operation -= 1; }
    let lifetimes = [
        SourceAddressLifetimeV29 { block: BlockId(77), gap: 4, sequence: 0, slot: 0, live: false },
        SourceAddressLifetimeV29 { block: BlockId(77), gap: 4, sequence: 1, slot: 0, live: true },
    ];
    let aliases = [
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0, recipe: SourceAddressAliasRecipeV29::Birth },
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0, recipe: SourceAddressAliasRecipeV29::Birth },
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0,
            recipe: SourceAddressAliasRecipeV29::Inherit(SourceAddressAliasInputV29::Logical(1)) },
        SourceAddressLogicalAliasV29 { pointer: A, slot: 0,
            recipe: SourceAddressAliasRecipeV29::Select(
                SourceAddressAliasInputV29::Logical(1), SourceAddressAliasInputV29::Logical(2)) },
    ];
    let mut events = [
        SourceAddressBoundaryKindV29::Alias(0),
        SourceAddressBoundaryKindV29::Lifetime(0),
        SourceAddressBoundaryKindV29::Lifetime(1),
        SourceAddressBoundaryKindV29::Alias(1),
        SourceAddressBoundaryKindV29::Alias(2),
        SourceAddressBoundaryKindV29::Alias(3),
    ].map(|kind| SourceAddressBoundaryEventV29 { block: BlockId(77), gap: 4, kind }).to_vec();
    let kill_gap = if after_store { 5 } else { 4 };
    let kills = [SourceAddressKillV29 { block: BlockId(77), gap: kill_gap, slot: 2 }];
    events.insert(position, SourceAddressBoundaryEventV29 {
        block: BlockId(77), gap: kill_gap, kind: SourceAddressBoundaryKindV29::Kill(0),
    });
    let uses = [SourceAddressLogicalUseV29 {
        block: BlockId(77), operation: Some(4), successor: None, operand: 1, value: A, alias: selected,
    }];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            &function, &slots, None, &accesses, &layouts, budget)?.solve(&slots, &accesses, &kills, budget)?;
        let floor = budget.storage();
        check_source_address_currentness_transport_v29(&function, &graph, &slots, &accesses, &kills,
            &[true; 3], &lifetimes, &[], &[], SourceAddressGeometryV29::Scalar,
            SourceAddressAliasTransportV29 { aliases: &aliases, uses: &uses, boundaries: Some(&events) }, budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    result
}

#[test]
fn logical_alias_same_gap_kills_commute_with_closed_virtual_events_without_reviving_aliases() {
    for position in 0..=6 {
        for selected in 0..4 {
            let result = logical_alias_gap_kill_run_v29(selected, position, false);
            if selected != 0 { result.unwrap(); }
            else {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "physical raw access crosses a storage activation or unresolved alias", ..
                })), "position {position}: {result:?}");
            }
        }
    }
}

#[test]
fn logical_alias_kills_cannot_move_across_an_actual_pointer_holder_store() {
    logical_alias_gap_kill_run_v29(1, 6, false).unwrap();
    assert!(matches!(logical_alias_gap_kill_run_v29(1, 6, true),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source raw address differs from its actual formation or memory history", ..
        })));
}
