use super::*;

#[path = "production_instance_coordinates_scalar_v1_tests.rs"]
mod scalar_tests;

const LIMIT: usize = 1_000_000;

fn snapshot(map: &ProductionInstanceCorrespondenceV1<'_, '_>) -> [usize; 17] {
    [
        map.storage,
        usize::from(map.transferred),
        map.seeds.rows.len(),
        map.seeds.rows.as_ptr() as usize,
        map.spans.rows.len(),
        map.spans.rows.as_ptr() as usize,
        map.controls.rows.len(),
        map.controls.rows.as_ptr() as usize,
        map.anchors.rows.len(),
        map.anchors.rows.as_ptr() as usize,
        map.returns.rows.len(),
        map.returns.rows.as_ptr() as usize,
        map.components.rows.len(),
        map.components.rows.as_ptr() as usize,
        map.values.rows.len(),
        map.values.rows.as_ptr() as usize,
        usize::from(map.failed),
    ]
}

fn with_expanded(
    test: impl FnOnce(
        &mut ProductionInstanceCorrespondenceV1<'_, '_>,
        &mut Function,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    with_plan(|plan, budget| {
        let floor = budget.storage();
        let (caller, callee, lowered_storage) = lower_pair(plan, budget);
        with_production_instance_correspondence_v1(plan, budget, |map, budget| {
            let call = &plan.calls(plan.root()).unwrap()[0];
            map.append_lowered(plan.root(), &caller, budget)?;
            map.append_lowered(call.child().unwrap(), &callee, budget)?;
            let mut expanded = map.splice(
                call,
                caller.function,
                callee.function,
                BlockId(18),
                BlockId(19),
                budget,
            )?;
            test(map, &mut expanded.caller, budget);
            let storage = expanded.additional_storage_bytes;
            drop(expanded);
            budget.release_storage(storage)?;
            Ok::<_, InstanceCorrespondenceErrorV1>(())
        })
        .unwrap();
        budget.release_storage(lowered_storage).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn owned_coordinates_escape_both_planning_scopes_and_transfer_storage_once() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(7).unwrap();
    let owned = {
        let mut ssa = ProductionSemanticSsaOwnerV1::try_new(
            resource_tests::helper_closure_semantic_owner(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let capture = ssa
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let root = ssa.source_semantic().roots()[0];
        let owned = with_production_call_instances_v1(&ssa, root, &mut budget, |plan, budget| {
            let (caller, callee, lowered_storage) = lower_pair(plan, budget);
            let owned = with_production_instance_correspondence_v1(plan, budget, |map, budget| {
                let call = &plan.calls(plan.root()).unwrap()[0];
                map.append_lowered(plan.root(), &caller, budget)?;
                map.append_lowered(call.child().unwrap(), &callee, budget)?;
                let expanded = map.splice(
                    call,
                    caller.function,
                    callee.function,
                    BlockId(18),
                    BlockId(19),
                    budget,
                )?;
                let old = snapshot(map);
                let before = budget.storage();
                let owned = map.take_owned_coordinates_v1(&expanded.caller, budget)?;
                let added = owned.sources.capacity * std::mem::size_of::<OwnedInstanceSourceV1>();
                assert_eq!(owned.retained_storage(), old[0] + added);
                assert_eq!(budget.storage(), before + added);
                assert_eq!(owned.seeds.rows.as_ptr() as usize, old[3]);
                assert_eq!(owned.spans.rows.as_ptr() as usize, old[5]);
                assert_eq!(owned.controls.rows.as_ptr() as usize, old[7]);
                assert_eq!(owned.anchors.rows.as_ptr() as usize, old[9]);
                assert_eq!(owned.returns.rows.as_ptr() as usize, old[11]);
                assert_eq!(owned.components.rows.as_ptr() as usize, old[13]);
                assert_eq!(owned.values.rows.as_ptr() as usize, old[15]);
                assert_eq!(map.storage, 0);
                assert!(map.transferred);
                assert!(matches!(
                    map.take_owned_coordinates_v1(&expanded.caller, budget),
                    Err(InstanceCorrespondenceErrorV1::Source)
                ));
                assert!(matches!(
                    map.check_coordinates(plan.root(), &expanded.caller, budget),
                    Err(InstanceCorrespondenceErrorV1::Source)
                ));
                owned.check_source_plan(plan, budget)?;
                let storage = expanded.additional_storage_bytes;
                drop(expanded);
                budget.release_storage(storage)?;
                Ok::<_, InstanceCorrespondenceErrorV1>(owned)
            })
            .unwrap();
            budget.release_storage(lowered_storage).unwrap();
            Ok::<_, ProductionCallInstanceErrorV1>(owned)
        })
        .unwrap();
        assert_eq!(owned.semantic_sha256, *ssa.source_semantic_sha256());
        assert!(owned.ssa == ssa.identity());
        drop(ssa);
        budget.release_storage(capture.retained_storage()).unwrap();
        owned
    };
    assert_eq!(owned.sources.rows.len(), 2);
    assert_eq!(owned.seeds.rows.len(), 2);
    assert_eq!(owned.anchors.rows.len(), 1);
    assert_eq!(owned.returns.rows.len(), 2);
    assert!(owned.sources.rows[0].incoming.is_none());
    assert_eq!(
        owned.sources.rows[1].incoming.unwrap().caller,
        owned.sources.rows[0].instance
    );
    assert_eq!(budget.storage(), 7 + owned.retained_storage());
    let storage = owned.retained_storage();
    drop(owned);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), 7);
}

#[test]
fn owned_coordinates_refuse_incomplete_or_changed_expansion_without_moving_rows() {
    for fault in 0..9 {
        with_expanded(|map, function, budget| {
            let root = map.plan.root();
            let child = map.plan.calls(root).unwrap()[0].child().unwrap();
            match fault {
                0 => {
                    map.seeds.rows.pop().unwrap();
                }
                1 => {
                    let index = map.seed_index(child, budget).unwrap();
                    map.seeds.rows[index].container = child;
                }
                2 => {
                    map.returns.rows.pop().unwrap();
                }
                3 => map.anchors.rows[0].removed = false,
                4 => {
                    map.controls.rows.pop().unwrap();
                }
                5 => {
                    let block = function
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .iter_mut()
                        .find(|block| block.id == BlockId(18))
                        .unwrap();
                    block.parameters.push(ValueDef {
                        id: ValueId(999),
                        ty: Type::Scalar(ScalarType::U32),
                    });
                }
                6 => function
                    .body
                    .as_mut()
                    .unwrap()
                    .parameters
                    .push(ValueId(999)),
                7 => map.controls.rows[1].physical_block = map.controls.rows[0].physical_block,
                8 => {
                    let control = map
                        .controls
                        .rows
                        .iter_mut()
                        .find(|row| row.expected_branch.is_some())
                        .unwrap();
                    control.expected_branch.as_mut().unwrap().1 = 0..usize::MAX;
                }
                _ => unreachable!(),
            }
            let before = snapshot(map);
            let storage = budget.storage();
            assert!(
                matches!(
                    map.take_owned_coordinates_v1(function, budget),
                    Err(InstanceCorrespondenceErrorV1::Source
                        | InstanceCorrespondenceErrorV1::CallAnchor
                        | InstanceCorrespondenceErrorV1::Control)
                ),
                "fault {fault}"
            );
            assert_eq!(snapshot(map), before);
            assert_eq!(budget.storage(), storage);
        });
    }
    for append_child in [false, true] {
        with_plan(|plan, budget| {
            let floor = budget.storage();
            let (caller, callee, storage) = lower_pair(plan, budget);
            with_production_instance_correspondence_v1(plan, budget, |map, budget| {
                map.append_lowered(plan.root(), &caller, budget)?;
                if append_child {
                    map.append_lowered(
                        plan.calls(plan.root()).unwrap()[0].child().unwrap(),
                        &callee,
                        budget,
                    )?;
                }
                map.check_coordinates(plan.root(), &caller.function, budget)?;
                let before = snapshot(map);
                assert!(matches!(
                    map.take_owned_coordinates_v1(&caller.function, budget),
                    Err(InstanceCorrespondenceErrorV1::Source)
                ));
                assert_eq!(snapshot(map), before);
                Ok::<_, InstanceCorrespondenceErrorV1>(())
            })
            .unwrap();
            drop((caller, callee));
            budget.release_storage(storage).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn refused_owned_transfer_keeps_the_original_donor_usable() {
    with_expanded(|map, function, budget| {
        let storage = budget.storage();
        map.anchors.rows[0].removed = false;
        let before = snapshot(map);
        assert!(matches!(
            map.take_owned_coordinates_v1(function, budget),
            Err(InstanceCorrespondenceErrorV1::CallAnchor)
        ));
        assert_eq!(snapshot(map), before);
        assert_eq!(budget.storage(), storage);
        assert!(!map.failed);
        map.anchors.rows[0].removed = true;
        let owned = map.take_owned_coordinates_v1(function, budget).unwrap();
        owned.check_source_plan(map.plan, budget).unwrap();
        let storage = owned.retained_storage();
        drop(owned);
        budget.release_storage(storage).unwrap();
    });
}

#[test]
fn instance_map_entry_points_refuse_foreign_ledgers_without_charges() {
    with_plan(|plan, budget| {
        let floor = budget.storage();
        let (caller, callee, storage) = lower_pair(plan, budget);
        with_production_instance_correspondence_v1(plan, budget, |map, budget| {
            let call = &plan.calls(plan.root()).unwrap()[0];
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
            let accounting = Err(InstanceCorrespondenceErrorV1::Resource(
                ArgumentResourceV1::Accounting,
            ));
            assert_eq!(
                map.append_lowered(plan.root(), &caller, &mut foreign),
                accounting
            );
            assert_eq!(map.storage, 0);
            map.append_lowered(plan.root(), &caller, budget)?;
            map.append_lowered(call.child().unwrap(), &callee, budget)?;
            let before = snapshot(map);
            let charged = budget.storage();
            assert!(matches!(
                map.splice(
                    call,
                    caller.function.clone(),
                    callee.function.clone(),
                    BlockId(18),
                    BlockId(19),
                    &mut foreign
                ),
                Err(InstanceCorrespondenceErrorV1::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(
                map.check_coordinates(plan.root(), &caller.function, &mut foreign),
                accounting
            );
            assert!(matches!(
                map.take_owned_coordinates_v1(&caller.function, &mut foreign),
                Err(InstanceCorrespondenceErrorV1::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(snapshot(map), before);
            assert_eq!(budget.storage(), charged);
            assert!(!map.failed);
            assert_eq!(foreign.work(), 0);
            assert_eq!(foreign.storage(), 0);
            Ok::<_, InstanceCorrespondenceErrorV1>(())
        })
        .unwrap();
        drop((caller, callee));
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn owned_coordinate_transfer_preserves_exact_work_and_storage_boundaries() {
    let mut exact_work = 0;
    let mut added_storage = 0;
    with_expanded(|map, function, budget| {
        let before_work = budget.work();
        let before_storage = budget.storage();
        let owned = map.take_owned_coordinates_v1(function, budget).unwrap();
        exact_work = budget.work() - before_work;
        added_storage = budget.storage() - before_storage;
        let storage = owned.retained_storage();
        drop(owned);
        budget.release_storage(storage).unwrap();
    });
    assert!(exact_work > 1 && added_storage > 1);
    for work_boundary in [false, true] {
        for short in [false, true] {
            with_expanded(|map, function, budget| {
                let before = snapshot(map);
                let reserve = if work_boundary {
                    budget
                        .charge_work(LIMIT - budget.work() - exact_work + usize::from(short))
                        .unwrap();
                    0
                } else {
                    let reserve = LIMIT - budget.storage() - added_storage + usize::from(short);
                    budget.reserve_storage(reserve).unwrap();
                    reserve
                };
                let storage = budget.storage();
                match map.take_owned_coordinates_v1(function, budget) {
                    Ok(owned) => {
                        assert!(!short);
                        let retained = owned.retained_storage();
                        drop(owned);
                        budget.release_storage(retained).unwrap();
                    }
                    Err(error) => {
                        assert!(short);
                        if work_boundary {
                            assert!(matches!(
                                error,
                                InstanceCorrespondenceErrorV1::Resource(ArgumentResourceV1::Work(
                                    _
                                ))
                            ));
                        } else {
                            assert!(matches!(
                                error,
                                InstanceCorrespondenceErrorV1::Resource(
                                    ArgumentResourceV1::Storage(_)
                                )
                            ));
                        }
                        assert_eq!(snapshot(map), before);
                        assert_eq!(budget.storage(), storage);
                    }
                }
                budget.release_storage(reserve).unwrap();
            });
        }
    }
}

#[test]
fn owned_coordinates_reject_a_fresh_source_plan_with_matching_function_identities() {
    with_expanded(|map, function, budget| {
        let owned = map.take_owned_coordinates_v1(function, budget).unwrap();
        owned.check_source_plan(map.plan, budget).unwrap();
        let mut changed = ProductionSemanticSsaOwnerV1::try_new(
            statement_order_owner(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(
            changed.source_semantic().functions()[0].identity(),
            map.plan.owner().source_semantic().functions()[0].identity()
        );
        let capture = changed
            .try_capture_occurrences_with_budget_v1(budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        with_production_call_instances_v1(
            &changed,
            changed.source_semantic().roots()[0],
            budget,
            |plan, budget| {
                assert_eq!(
                    owned.check_source_plan(plan, budget),
                    Err(InstanceCorrespondenceErrorV1::Source)
                );
                Ok::<_, ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
        drop(changed);
        budget.release_storage(capture.retained_storage()).unwrap();
        let storage = owned.retained_storage();
        drop(owned);
        budget.release_storage(storage).unwrap();
    });
}
