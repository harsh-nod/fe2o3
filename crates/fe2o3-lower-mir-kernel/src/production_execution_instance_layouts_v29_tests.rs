use super::*;

pub(super) fn layout_root_only_owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |_, functions| {
        let root = &functions[0];
        let entry = root.kernel_entry().unwrap().clone();
        functions[0] = function(
            10,
            true,
            WORD,
            root.locals().to_vec(),
            vec![block(10, vec![unit()], SemanticTerminatorKindV1::Return)],
        )
        .with_kernel_entry(entry);
        functions.truncate(1);
    })
}

#[test]
fn instance_layout_roster_and_child_plans_match_independent_source_reconstruction() {
    for source in [
        owner(Case::Shared),
        owner(Case::Writeback),
        cells_tests::mixed_strategy_owner(),
    ] {
        let completed = std::cell::Cell::new(false);
        cells_tests::run_cells(source, |plan, budget| {
            let floor = budget.storage();
            with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
                assert_eq!(layouts.rows.len(), plan.instances.instances().len());
                assert!(layouts.rows[plan.root.index()].is_none());
                assert_eq!(layouts.calls.len(), layouts.rows.len() - 1);
                for index in 1..layouts.rows.len() {
                    let id = plan.instances.id_at(index).unwrap();
                    let row = layouts.row(id, budget)?;
                    let original = plan.instances.instance(id).unwrap();
                    let incoming = plan.instances.incoming(id).unwrap();
                    assert_eq!(row.function, original.function());
                    assert_eq!(row.incoming, incoming.occurrence());
                    assert_eq!(
                        row.original_call,
                        std::ptr::from_ref(incoming.source()) as usize
                    );
                    let before = budget.storage();
                    let independent = execution_function_signature_with_references_v29(
                        plan.instances,
                        id,
                        Some(plan),
                        budget,
                    )?;
                    source_reference_same_signature_v29(&row.signature, &independent, budget)?;
                    let placement = SemanticEmissionPlacementV1 {
                        first_block: 1000,
                        first_value: 2000,
                    };
                    let emitted = layouts.instance_plan_v29(
                        plan.instances,
                        id,
                        FunctionId::new("instance_layout_test"),
                        placement,
                        budget,
                    )?;
                    assert_eq!(emitted.semantic_function, row.function);
                    assert_eq!(emitted.parameter_declarations, row.declarations);
                    assert_eq!(emitted.parameter_types, row.signature.parameter_types);
                    assert_eq!(emitted.result_types, row.signature.result_types);
                    assert_eq!(
                        emitted.call_arguments.len(),
                        row.signature.call_arguments.len()
                    );
                    for (actual, expected) in emitted
                        .call_arguments
                        .iter()
                        .zip(&row.signature.call_arguments)
                    {
                        assert_eq!(
                            (actual.source_argument, actual.tuple_field, actual.component),
                            (
                                expected.source_argument,
                                expected.tuple_field,
                                expected.component
                            )
                        );
                    }
                    assert_eq!(
                        emitted.parameter_values.len(),
                        emitted.parameter_types.len()
                    );
                    for (ordinal, value) in emitted.parameter_values.iter().enumerate() {
                        assert_eq!(*value, ValueId(2000 + ordinal as u32));
                    }
                    drop((emitted, independent));
                    budget.release_storage(budget.storage() - before)?;
                }
                completed.set(true);
                Ok(())
            })?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
        assert!(completed.get());
    }
}

#[test]
fn instance_layouts_keep_mixed_strategies_distinct_and_require_the_actual_call() {
    let completed = std::cell::Cell::new(false);
    cells_tests::run_cells(cells_tests::mixed_strategy_owner(), |plan, budget| {
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
            let left = capture_instance(plan, 0);
            let right = capture_instance(plan, 1);
            let a = layouts.row(left, budget)?;
            let b = layouts.row(right, budget)?;
            assert_eq!(a.function, b.function);
            assert_eq!(
                a.signature.parameter_semantic_types,
                b.signature.parameter_semantic_types
            );
            assert_eq!(a.signature.parameter_types, [Type::Scalar(ScalarType::U64)]);
            assert_eq!(
                b.signature.parameter_types,
                [Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Private,
                    AccessMode::ReadWrite
                )]
            );
            assert_ne!(a.incoming.caller, b.incoming.caller);
            assert_ne!(a.original_call, b.original_call);
            assert!(layouts.row(plan.root, budget).is_err());
            for child in [left, right] {
                let incoming = plan.instances.incoming(child).unwrap();
                let caller = incoming.occurrence().caller;
                let block = incoming.occurrence().block;
                let call = incoming.source();
                let callee = plan.instances.instance(child).unwrap().function();
                with_source_reference_availability_v29(
                    plan.instances,
                    caller,
                    Some(&emission),
                    budget,
                    |mut cursor, budget| {
                        let before = budget.storage();
                        let actual =
                            layouts.signature_for_call_v29(&cursor, block, call, callee, budget)?;
                        source_reference_same_signature_v29(
                            &actual,
                            &layouts.row(child, budget)?.signature,
                            budget,
                        )?;
                        drop(actual);
                        budget.release_storage(budget.storage() - before)?;
                        let copied = call.clone();
                        assert!(
                            layouts
                                .signature_for_call_v29(&cursor, block, &copied, callee, budget)
                                .is_err()
                        );
                        assert!(
                            layouts
                                .signature_for_call_v29(
                                    &cursor,
                                    SemanticBlockIdV1::from_index(u32::MAX),
                                    call,
                                    callee,
                                    budget
                                )
                                .is_err()
                        );
                        assert!(
                            layouts
                                .signature_for_call_v29(&cursor, block, call, ROOT, budget)
                                .is_err()
                        );
                        let original = cursor.instance;
                        cursor.instance = if child == left {
                            b.incoming.caller
                        } else {
                            a.incoming.caller
                        };
                        assert!(
                            layouts
                                .signature_for_call_v29(&cursor, block, call, callee, budget)
                                .is_err()
                        );
                        cursor.instance = original;
                        let actual =
                            layouts.signature_for_call_v29(&cursor, block, call, callee, budget)?;
                        drop(actual);
                        Ok(())
                    },
                )?;
            }
            completed.set(true);
            Ok(())
        })
    })
    .unwrap();
    assert!(completed.get());
}

#[test]
fn instance_layout_root_only_has_no_synthetic_helper_signature() {
    let completed = std::cell::Cell::new(false);
    cells_tests::run_cells(layout_root_only_owner(), |plan, budget| {
        assert_eq!(plan.instances.instances().len(), 1);
        with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
            assert_eq!(layouts.rows.len(), 1);
            assert!(layouts.rows[0].is_none());
            assert!(layouts.calls.is_empty());
            assert!(layouts.row(plan.root, budget).is_err());
            completed.set(true);
            Ok(())
        })
    })
    .unwrap();
    assert!(completed.get());
}

#[test]
fn instance_layout_foreign_slot_is_denied_before_query_work() {
    let completed = std::cell::Cell::new(false);
    let result = cells_tests::run_cells(owner(Case::Shared), |plan, budget| {
        with_execution_instance_layouts_v29(plan, budget, |layouts, _| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let child = plan.instances.id_at(1).unwrap();
            let before = (foreign.work(), foreign.storage());
            assert!(matches!(
                layouts.row(child, &mut foreign),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!((foreign.work(), foreign.storage()), before);
            completed.set(true);
            Ok(())
        })
    });
    assert!(completed.get());
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}

#[test]
fn instance_layouts_preserve_dense_original_ids_and_absent_inactive_children() {
    let owner = owner_with(Case::Shared, |_, functions| {
        let original = &functions[2];
        functions[2] = function(
            30,
            false,
            CAPTURE,
            original.locals().to_vec(),
            vec![block(
                30,
                vec![],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(0),
                )),
            )],
        );
    });
    let completed = std::cell::Cell::new(false);
    cells_tests::run_cells(owner, |plan, budget| {
        assert_eq!(plan.instances.instances().len(), 5);
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
            assert_eq!(layouts.rows.len(), 5);
            assert_eq!(layouts.calls.len(), 2);
            let mut active_children = 0;
            let mut inactive_children = 0;
            for ordinal in 1..layouts.rows.len() {
                let id = plan.instances.id_at(ordinal).unwrap();
                let active = plan.instances.instance_reachable(id).unwrap();
                assert_eq!(layouts.rows[ordinal].is_some(), active);
                if !active {
                    inactive_children += 1;
                    assert!(layouts.row(id, budget).is_err());
                    continue;
                }
                active_children += 1;
                let row = layouts.row(id, budget)?;
                assert_eq!(plan.instances.instance_may_return(id), Some(false));
                assert!(plan.returns[ordinal].is_none());
                assert!(row.signature.result_types.is_empty());
                let incoming = plan.instances.incoming(id).unwrap();
                let caller = incoming.occurrence().caller;
                assert_eq!(
                    plan.instances.call_control(incoming.occurrence()),
                    Some(ProductionCallControlV1::NoNormalReturn)
                );
                with_source_reference_availability_v29(
                    plan.instances,
                    caller,
                    Some(&emission),
                    budget,
                    |cursor, budget| {
                        let signature = layouts.signature_for_call_v29(
                            &cursor,
                            incoming.occurrence().block,
                            incoming.source(),
                            row.function,
                            budget,
                        )?;
                        assert!(signature.result_types.is_empty());
                        drop(signature);
                        Ok(())
                    },
                )?;
            }
            assert_eq!((active_children, inactive_children), (2, 2));
            completed.set(true);
            Ok(())
        })
    })
    .unwrap();
    assert!(completed.get());
}
