#[test]
fn original_loop_address_recipe_replays_first_and_converged_visits() {
    let mut completed = false;
    with_selected_pointer_test_plan_v29(joined_backing_owner_v29(true, true, false), |plan, budget| {
        let mut helpers = 0;
        let mut earlier = 0;
        let mut exact = 0;
        for ordinal in 0..plan.instances.instances().len() {
            let instance = plan.instances.id_at(ordinal).unwrap();
            let original = plan.instances.instance(instance).unwrap();
            if original.function().index() != 3 { continue; }
            helpers += 1;
            let site = SourceReferenceSiteV29 { instance, block: SemanticBlockIdV1::from_index(3), statement: Some(0) };
            let SemanticStatementKindV1::Assign(assignment) = original.declaration().blocks()[3].statements()[0].kind()
                else { panic!("genuine original address assignment"); };
            let SemanticRvalueKindV1::AddressOf { place, mutability } = assignment.value().kind()
                else { panic!("genuine original AddressOf"); };
            let source = source_reference_access_at_v29(plan, site, place, SourceReferenceAccessV29::Address, budget)?;
            let snapshot = plan.blocks.iter().find(|block| block.instance == instance && block.block == site.block).unwrap();
            assert_eq!(source.generation, plan.states[snapshot.entry][place.local().index() as usize].generation);
            let current = source_reference_original_epoch_members_v29(plan, instance, place.local(), source.generation, budget)?.unwrap();
            assert_eq!(current.len(), 2);
            assert_eq!(current[0], 0);
            let expected = assignment.value().result_type();
            let key = source_reference_access_key_v29(site, assignment.destination(), SourceReferenceAccessV29::Write);
            for &index in plan.representation_demand_sites.get(&key).unwrap() {
                let node = plan.representation_demands[index].node;
                let row = &plan.nodes[node];
                assert_eq!(row.ty, expected);
                assert!(row.descriptor.is_none());
                let SourceReferenceNodeKindV29::Address(set) = row.kind else { panic!("source address node"); };
                let canonical = plan.raw_nodes[&(expected.index(), set)];
                source_reference_address_value_origin_v29(&plan.nodes, node, canonical, budget)?;
                let choices = plan.raw_sets[set];
                assert_eq!(choices.ty, place.ty());
                assert_eq!(choices.mutable, *mutability == SemanticMutabilityV1::Mutable);
                assert!(plan.raw_projection_range(set, place.ty(), budget)?.is_empty());
                let schema = plan.selected_storage[node].unwrap();
                let layouts = plan.storage_root.as_ref().unwrap().source_layouts(plan.instances, budget)?;
                layouts.check_selected_schema(plan.instances.owner(), expected, schema, budget)?;
                assert!(matches!(layouts.rows(plan.instances.owner(), budget)?[schema.0 as usize].kind,
                    fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer)
                        if pointer.value_space == AddressSpace::Private));
                for choice in &plan.raw_choices[choices.first..choices.first + choices.count] {
                    let origin = &plan.raw_origins[choice.origin];
                    assert_eq!(origin.site, site);
                    assert_eq!(origin.source, place as *const SemanticPlaceV1 as usize);
                    assert_eq!(origin.pointer_type, expected);
                    assert_eq!(plan.raw_origin_sites[&(instance.index(), 3, 0, origin.generation)], choice.origin);
                    assert_eq!((source.instance, source.local, source.ty, source.loan),
                        (origin.instance, origin.local, origin.ty, origin.parent));
                    if source.generation == origin.generation { exact += 1; }
                    else {
                        earlier += 1;
                        assert_eq!(origin.generation, 0, "the retained first visit predates the fixed point");
                        let (cell, current, _) = plan.physical_object_generation(instance, place.local(), source.generation, budget)?.unwrap();
                        let (captured, same, _) = plan.physical_object_generation(instance, place.local(), origin.generation, budget)?.unwrap();
                        assert_ne!(cell, captured, "logical source identities stay distinct");
                        assert_eq!(current, same, "only their authenticated physical backing is shared");
                    }
                    if source.generation != origin.generation {
                        assert!(source_reference_address_epoch_matches_v29(plan, site, place, source, choice.origin, budget)?);
                    }
                }
                let selected = source_reference_selected_pointer_type_v29(plan, node, expected, budget)?.unwrap();
                assert!(matches!(selected, Type::Pointer(pointer)
                    if pointer.address_space == AddressSpace::Private && matches!(*pointer.pointee, Type::StorageObject(_))));
            }
        }
        assert_eq!(helpers, 2);
        assert!(earlier >= 2 && exact >= 2, "both original visits must survive in each helper");
        completed = true;
        Ok(())
    }).unwrap();
    assert!(completed);
}

fn cloned_address_epoch_access_v29(
    row: &SourceReferenceAccessRecordV29,
) -> SourceReferenceAccessRecordV29 {
    SourceReferenceAccessRecordV29 {
        key: row.key,
        source_local: row.source_local,
        ty: row.ty,
        instance: row.instance,
        local: row.local,
        generation: row.generation,
        projections: row.projections.clone(),
        loan: row.loan,
        traversed: row.traversed.clone(),
        shared_path: row.shared_path,
    }
}

#[test]
fn original_address_epoch_reconciliation_refuses_changed_recipe_and_foreign_backing() {
    let mut completed = false;
    with_selected_pointer_test_plan_v29(
        joined_backing_owner_v29(true, true, false),
        |plan, budget| {
            let (index, origin) = plan
                .raw_origins
                .iter()
                .enumerate()
                .find(|(_, row)| row.generation == 0 && row.site.block.index() == 3)
                .unwrap();
            let declaration = plan
                .instances
                .instance(origin.site.instance)
                .unwrap()
                .declaration();
            let SemanticStatementKindV1::Assign(assignment) =
                declaration.blocks()[3].statements()[0].kind()
            else {
                unreachable!()
            };
            let SemanticRvalueKindV1::AddressOf { place, .. } = assignment.value().kind() else {
                unreachable!()
            };
            let source = source_reference_access_at_v29(
                plan,
                origin.site,
                place,
                SourceReferenceAccessV29::Address,
                budget,
            )?;
            assert_ne!(source.generation, origin.generation);
            assert!(source_reference_address_epoch_matches_v29(
                plan,
                origin.site,
                place,
                source,
                index,
                budget
            )?);
            for fault in 0..12 {
                let mut changed = cloned_address_epoch_access_v29(source);
                match fault {
                    0 => changed.key.source = 0,
                    1 => changed.key.access = SourceReferenceAccessV29::Read,
                    2 => changed.key.site.block = SemanticBlockIdV1::from_index(2),
                    3 => changed.instance = plan.instances.root(),
                    4 => changed.local = SemanticLocalIdV1::from_index(0),
                    5 => changed.ty = UNIT,
                    6 => changed.loan = Some(0),
                    7 => changed.shared_path = true,
                    8 => changed.projections = 0..1,
                    9 => changed.traversed = 0..1,
                    10 => {
                        changed.generation = source_reference_original_epoch_members_v29(
                            plan,
                            source.instance,
                            source.local,
                            source.generation,
                            budget,
                        )?
                        .unwrap()[1]
                    }
                    11 => changed.generation = u32::MAX,
                    _ => unreachable!(),
                }
                assert!(
                    !source_reference_address_epoch_matches_v29(
                        plan,
                        origin.site,
                        place,
                        &changed,
                        index,
                        budget
                    )?,
                    "fault {fault}"
                );
            }
            let other = plan
                .raw_origins
                .iter()
                .position(|row| row.site.instance != origin.site.instance && row.generation == 0)
                .unwrap();
            assert!(!source_reference_address_epoch_matches_v29(
                plan,
                origin.site,
                place,
                source,
                other,
                budget
            )?);
            let clone = place.clone();
            assert!(!source_reference_address_epoch_matches_v29(
                plan,
                origin.site,
                &clone,
                source,
                index,
                budget
            )?);
            assert!(source_reference_address_epoch_matches_v29(
                plan,
                origin.site,
                place,
                source,
                index,
                budget
            )?);
            completed = true;
            Ok(())
        },
    )
    .unwrap();
    assert!(completed);
}

#[test]
fn original_address_epoch_query_preserves_custody_exact_work_and_no_retained_storage() {
    for mode in 0..4 {
        let mut completed = false;
        let result = with_selected_pointer_test_plan_v29(
            joined_backing_owner_v29(true, true, false),
            |plan, budget| {
                let (index, origin) = plan
                    .raw_origins
                    .iter()
                    .enumerate()
                    .find(|(_, row)| row.generation == 0 && row.site.block.index() == 3)
                    .unwrap();
                let original = plan
                    .instances
                    .instance(origin.site.instance)
                    .unwrap()
                    .declaration();
                let SemanticStatementKindV1::Assign(assignment) =
                    original.blocks()[3].statements()[0].kind()
                else {
                    unreachable!()
                };
                let SemanticRvalueKindV1::AddressOf { place, .. } = assignment.value().kind()
                else {
                    unreachable!()
                };
                let source = source_reference_access_at_v29(
                    plan,
                    origin.site,
                    place,
                    SourceReferenceAccessV29::Address,
                    budget,
                )?;
                let before = (budget.work(), budget.storage(), budget.peak_storage());
                assert!(source_reference_address_epoch_matches_v29(
                    plan,
                    origin.site,
                    place,
                    source,
                    index,
                    budget
                )?);
                let expected = budget.work() - before.0;
                assert!(expected > 24);
                assert_eq!(
                    (budget.storage(), budget.peak_storage()),
                    (before.1, before.2)
                );
                if mode == 0 {
                    for _ in 0..32 {
                        let work = budget.work();
                        assert!(source_reference_address_epoch_matches_v29(
                            plan,
                            origin.site,
                            place,
                            source,
                            index,
                            budget
                        )?);
                        assert_eq!(budget.work() - work, expected);
                        assert_eq!(
                            (budget.storage(), budget.peak_storage()),
                            (before.1, before.2)
                        );
                    }
                    completed = true;
                    return Ok(());
                }
                if mode == 1 {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                    let mut foreign = ArgumentBudgetV1::new(&mut work, 0);
                    let error = source_reference_address_epoch_matches_v29(
                        plan,
                        origin.site,
                        place,
                        source,
                        index,
                        &mut foreign,
                    )
                    .unwrap_err();
                    assert!(matches!(
                        error,
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    ));
                    assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                    let work = budget.work();
                    assert!(matches!(
                        source_reference_address_epoch_matches_v29(
                            plan,
                            origin.site,
                            place,
                            source,
                            index,
                            budget
                        ),
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!(budget.work(), work);
                    completed = true;
                    return Err(error);
                }
                budget
                    .charge_work(20_000_000 - budget.work() - expected + usize::from(mode == 3))?;
                let before = (budget.work(), budget.storage());
                let query = source_reference_address_epoch_matches_v29(
                    plan,
                    origin.site,
                    place,
                    source,
                    index,
                    budget,
                );
                if mode == 2 {
                    assert!(query?);
                    assert_eq!(budget.work() - before.0, expected);
                    assert_eq!(budget.storage(), before.1);
                    completed = true;
                    Ok(())
                } else {
                    let error = query.unwrap_err();
                    assert!(matches!(
                        &error,
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    ));
                    let work = budget.work();
                    assert_eq!(
                        format!(
                            "{:?}",
                            source_reference_address_epoch_matches_v29(
                                plan,
                                origin.site,
                                place,
                                source,
                                index,
                                budget
                            )
                            .unwrap_err()
                        ),
                        format!("{error:?}")
                    );
                    assert_eq!((budget.work(), budget.storage()), (work, before.1));
                    completed = true;
                    Err(error)
                }
            },
        );
        assert!(completed, "mode {mode}: {result:?}");
        match mode {
            0 | 2 => result.unwrap(),
            1 => assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            )),
            3 => assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            )),
            _ => unreachable!(),
        }
    }
}

#[test]
fn address_epoch_subset_equations_have_exact_linear_work_and_no_storage() {
    // Inert set equations, not original-source or pointer authority.
    for count in [2usize, 3, 32, 128, 4096] {
        let current: Vec<u32> = (0..count as u32).collect();
        // 2 header + N current order + N captured order + N merge steps.
        let expected = 2 + 3 * count;
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(expected - usize::from(short));
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = source_reference_epoch_subset_v29(&current, &current, &mut budget);
            if short {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(limit)))
                    if (limit.actual(), limit.limit()) == (expected, expected - 1))
                );
                assert_eq!(budget.work(), expected - 1);
            } else {
                assert!(result.unwrap());
                assert_eq!(budget.work(), expected);
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
        }
    }
    for (current, captured, expected) in [
        (&[0, 7][..], &[0][..], true),
        (&[0, 7][..], &[7][..], true),
        (&[0, 7][..], &[8][..], false),
        (&[0, 7][..], &[0, 4][..], false),
        (&[0, 7][..], &[0, 7, 8][..], false),
        (&[0, 7][..], &[][..], false),
        (&[0][..], &[0][..], false),
        (&[7, 0][..], &[0][..], false),
        (&[0, 0][..], &[0][..], false),
        (&[0, 7][..], &[0, 0][..], false),
        (&[0, u32::MAX][..], &[0][..], false),
        (&[0, 7][..], &[u32::MAX][..], false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert_eq!(
            source_reference_epoch_subset_v29(current, captured, &mut budget).unwrap(),
            expected
        );
        assert_eq!(budget.storage(), 0);
    }
}
