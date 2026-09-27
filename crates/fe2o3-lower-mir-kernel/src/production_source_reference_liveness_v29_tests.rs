// Genuine source owners and their captured occurrences, not detached live sets.
fn completed_statement_owner_v29(mode: usize) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Writeback, |_, functions| {
        let referent = || {
            projected(
                1,
                &[
                    (SemanticProjectionKindV1::Field(0), REFERENCE),
                    (SemanticProjectionKindV1::Dereference, WORD),
                ],
            )
        };
        let mut statements = vec![assign(
            place(2, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: referent(),
            },
        )];
        let holder = if mode == 2 {
            statements.push(assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, REFERENCE))),
            ));
            4
        } else if mode == 3 {
            statements.push(assign(
                place(5, CAPTURE),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![SemanticOperandV1::Move(place(2, REFERENCE))],
                    )
                    .unwrap(),
                ),
            ));
            5
        } else {
            2
        };
        let read = || {
            if holder == 5 {
                projected(
                    5,
                    &[
                        (SemanticProjectionKindV1::Field(0), REFERENCE),
                        (SemanticProjectionKindV1::Dereference, WORD),
                    ],
                )
            } else {
                projected(holder, &[(SemanticProjectionKindV1::Dereference, WORD)])
            }
        };
        if mode == 4 {
            // The RHS move kills the old SSA definition, but the later Define
            // installs a new holder which the following read still needs.
            statements.push(assign(
                place(2, REFERENCE),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, REFERENCE))),
            ));
        }
        statements.push(assign(
            place(3, WORD),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read())),
        ));
        statements.push(assign(referent(), fixture_word(47)));
        if mode == 1 {
            statements.push(assign(
                place(3, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read())),
            ));
        }
        statements.push(unit());
        functions[2] = function(
            30,
            false,
            CAPTURE,
            vec![
                local(30, UNIT, SemanticLocalRoleV1::Return),
                local(31, CAPTURE, SemanticLocalRoleV1::Argument(0)),
                local(32, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(33, WORD, SemanticLocalRoleV1::Temporary),
                local(34, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(35, CAPTURE, SemanticLocalRoleV1::Temporary),
            ],
            vec![block(130, statements, SemanticTerminatorKindV1::Return)],
        );
    })
}

#[test]
fn completed_statement_liveness_releases_only_finished_direct_move_and_static_holders() {
    for mode in [0, 2, 3, 4] {
        cells_tests::run_cells(completed_statement_owner_v29(mode), |plan, _| {
            let mut helpers = 0;
            for (ordinal, row) in plan.instances.instances().iter().enumerate() {
                if row.function().index() != 2 {
                    continue;
                }
                helpers += 1;
                let instance = plan.instances.id_at(ordinal).unwrap();
                assert!(plan.loans.iter().any(|loan| loan.site.instance == instance
                    && loan.site.statement == Some(0)
                    && loan.parent.is_some()));
                assert!(
                    plan.accesses
                        .iter()
                        .any(|access| access.key.site.instance == instance
                            && access.key.access == SourceReferenceAccessV29::Write)
                );
                assert!(
                    plan.boundary_values
                        .iter()
                        .any(|value| value.site.instance == instance
                            && value.role == SourceReferenceBoundaryRoleV29::Return)
                );
                assert_eq!(plan.nodes[plan.returns[ordinal].unwrap()].ty, UNIT);
            }
            assert_eq!(helpers, 2);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn completed_statement_liveness_preserves_future_use_suspended_parent_and_escape_refusals() {
    let error = cells_tests::run_cells(completed_statement_owner_v29(1), |_, _| {
        panic!("a future child use was erased before its conflicting parent write")
    })
    .unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                function: 2,
                block: Some(0),
                statement: Some(2),
                detail: "source reference access bypasses a live loan",
            }
        ),
        "{error:?}"
    );
    let error = cells_tests::run_cells(cells_tests::return_alias_owner(true), |_, _| {
        panic!("dead-holder optimization allowed a referent-frame escape")
    })
    .unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference return escapes its referent call frame",
                ..
            }
        ),
        "{error:?}"
    );
    let error = cells_tests::run_cells(distinct_exit_loan_owner_v29(true, false), |_, _| {
        panic!("a future successor use was removed")
    })
    .unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference CFG merge changes loan identity",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn completed_statement_liveness_joins_original_events_and_keeps_same_statement_definition() {
    for mode in [0, 2, 3, 4] {
        let owner = completed_statement_owner_v29(mode);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
                let instance = instances.id_at(2).unwrap();
                let floor = budget.storage();
                with_source_reference_liveness_v29(
                    &mut builder,
                    instance,
                    budget,
                    |builder, context, budget| {
                        let site = |statement| SourceReferenceSiteV29 {
                            instance,
                            block: SemanticBlockIdV1::from_index(0),
                            statement: Some(statement),
                        };
                        assert!(
                            !context
                                .completed(&builder.plan, site(0), budget)
                                .unwrap()
                                .contains(&SemanticLocalIdV1::from_index(2))
                        );
                        let read = usize::from(mode != 0);
                        let holder = if mode == 2 {
                            4
                        } else if mode == 3 {
                            5
                        } else {
                            2
                        };
                        if mode == 4 {
                            assert!(
                                !context
                                    .completed(&builder.plan, site(1), budget)
                                    .unwrap()
                                    .contains(&SemanticLocalIdV1::from_index(2)),
                                "RHS Kill must not expire the new later Define"
                            );
                        }
                        assert!(
                            context
                                .completed(&builder.plan, site(read + 1), budget)
                                .unwrap()
                                .contains(&SemanticLocalIdV1::from_index(holder))
                        );
                        assert!(
                            context
                                .completed(
                                    &builder.plan,
                                    SourceReferenceSiteV29 {
                                        statement: None,
                                        ..site(0)
                                    },
                                    budget
                                )
                                .is_err()
                        );
                        assert!(
                            context
                                .completed(
                                    &builder.plan,
                                    SourceReferenceSiteV29 {
                                        instance: instances.root(),
                                        ..site(0)
                                    },
                                    budget
                                )
                                .is_err()
                        );
                        let repeated = instances.id_at(4).unwrap();
                        assert_eq!(
                            instances.instance(repeated).unwrap().function(),
                            instances.instance(instance).unwrap().function()
                        );
                        assert!(
                            context
                                .completed(
                                    &builder.plan,
                                    SourceReferenceSiteV29 {
                                        instance: repeated,
                                        ..site(0)
                                    },
                                    budget
                                )
                                .is_err(),
                            "equal helper syntax is not equal original call-instance identity"
                        );
                        // Semantic locator refusals do not poison the genuine context.
                        assert!(builder.plan.failure.first_error().is_none());
                        context
                            .completed(&builder.plan, site(read + 1), budget)
                            .unwrap();
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(budget.storage(), floor);
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
    }
}

#[test]
fn completed_statement_liveness_reconstructs_authentic_loop_and_return_edge_sets() {
    for owner in [
        loop_owner(LoopCase::Invariant),
        cells_tests::return_alias_owner(false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
                for ordinal in 0..instances.instances().len() {
                    let instance = instances.id_at(ordinal).unwrap();
                    let before = budget.storage();
                    with_source_reference_liveness_v29(
                        &mut builder,
                        instance,
                        budget,
                        |builder, context, budget| {
                            let function = instances.instance(instance).unwrap().declaration();
                            assert_eq!(context.blocks.len(), function.blocks().len());
                            for (block, row) in function.blocks().iter().enumerate() {
                                for statement in 0..row.statements().len() {
                                    context
                                        .completed(
                                            &builder.plan,
                                            SourceReferenceSiteV29 {
                                                instance,
                                                block: SemanticBlockIdV1::from_index(block as u32),
                                                statement: Some(statement),
                                            },
                                            budget,
                                        )
                                        .unwrap();
                                }
                            }
                            assert!(
                                context.deaths.len()
                                    <= instances.occurrences(instance).unwrap().events().len()
                            );
                            Ok(())
                        },
                    )
                    .unwrap();
                    assert_eq!(budget.storage(), before);
                }
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
    }
}

#[test]
fn completed_statement_liveness_scope_preserves_consumer_storage_and_original_generations() {
    let owner = completed_statement_owner_v29(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            for ordinal in 0..instances.instances().len() {
                let instance = instances.id_at(ordinal).unwrap();
                let count = instances
                    .instance(instance)
                    .unwrap()
                    .declaration()
                    .locals()
                    .len();
                let mut state = source_reference_scratch_v29(count, budget).unwrap();
                budget.charge_work(count).unwrap();
                state.resize(count, SourceReferenceLocalV29::default());
                let index = builder.plan.states.len();
                emission_push_v1(&mut builder.plan.states, state, budget).unwrap();
                builder.frames[ordinal] = Some(index);
            }
            let instance = instances.id_at(2).unwrap();
            let before = budget.storage();
            let mut allocated = None;
            with_source_reference_liveness_v29(
                &mut builder,
                instance,
                budget,
                |builder, _, budget| {
                    allocated = Some(source_reference_scratch_v29::<u64>(7, budget)?);
                    let local = builder.local(instance, SemanticLocalIdV1::from_index(2))?;
                    assert_eq!(local.generation, 0);
                    assert!(local.storage.is_none());
                    Ok(())
                },
            )
            .unwrap();
            let expected = std::mem::size_of::<Vec<u64>>()
                + allocated.as_ref().unwrap().capacity() * std::mem::size_of::<u64>();
            assert_eq!(budget.storage(), before + expected);
            drop(allocated);
            budget.release_storage(expected).unwrap();
            assert_eq!(budget.storage(), before);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[test]
fn completed_statement_liveness_query_work_is_exact_and_sticky_after_a_swallowed_refusal() {
    let owner = completed_statement_owner_v29(0);
    let mut outer_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut outer = ArgumentBudgetV1::new(&mut outer_work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut outer,
        |instances, _| {
            for slack in [0, 1] {
                const LIMIT: usize = 10_000_000;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                let mut builder = SourceReferenceBuilderV29::new(instances, &mut budget).unwrap();
                let instance = instances.id_at(2).unwrap();
                let floor = budget.storage();
                let result = with_source_reference_liveness_v29(
                    &mut builder,
                    instance,
                    &mut budget,
                    |builder, context, budget| {
                        let site = SourceReferenceSiteV29 {
                            instance,
                            block: SemanticBlockIdV1::from_index(0),
                            statement: Some(0),
                        };
                        budget
                            .charge_work(LIMIT - budget.work() - (11 - slack))
                            .unwrap();
                        let before = budget.work();
                        let query = context.completed(&builder.plan, site, budget);
                        if slack == 0 {
                            query.unwrap();
                            assert_eq!(budget.work(), before + 11);
                        } else {
                            let error = query.unwrap_err();
                            assert!(matches!(
                                error,
                                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                    ArgumentResourceV1::Work(_)
                                )
                            ));
                            assert_eq!(budget.work(), before + 5);
                            assert!(matches!(
                                builder.plan.failure.first_error(),
                                Some(
                                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                        ArgumentResourceV1::Work(_)
                                    )
                                )
                            ));
                            let replay = budget.work();
                            assert_eq!(
                                format!(
                                    "{:?}",
                                    context.completed(&builder.plan, site, budget).unwrap_err()
                                ),
                                format!("{error:?}")
                            );
                            assert_eq!(budget.work(), replay);
                        }
                        Ok(())
                    },
                );
                assert_eq!(budget.storage(), floor);
                if slack == 0 {
                    result.unwrap();
                } else {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                }
            }
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[test]
fn completed_statement_liveness_foreign_budget_refuses_without_debit_then_replays() {
    let owner = completed_statement_owner_v29(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            let instance = instances.id_at(2).unwrap();
            let floor = budget.storage();
            let result = with_source_reference_liveness_v29(
                &mut builder,
                instance,
                budget,
                |builder, context, budget| {
                    let site = SourceReferenceSiteV29 {
                        instance,
                        block: SemanticBlockIdV1::from_index(0),
                        statement: Some(0),
                    };
                    context.completed(&builder.plan, site, budget).unwrap();
                    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
                    assert!(matches!(
                        context.completed(&builder.plan, site, &mut foreign),
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                    let before = (budget.work(), budget.storage());
                    assert!(matches!(
                        context.completed(&builder.plan, site, budget),
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!((budget.work(), budget.storage()), before);
                    Ok(())
                },
            );
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[test]
fn completed_statement_liveness_partial_constructor_storage_returns_the_exact_caller_floor() {
    let owner = completed_statement_owner_v29(0);
    let mut outer_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut outer = ArgumentBudgetV1::new(&mut outer_work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut outer, |instances, _| {
        for partial in [false, true] {
            const LIMIT: usize = 10_000_000;
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
            let mut builder = SourceReferenceBuilderV29::new(instances, &mut budget).unwrap();
            let instance = instances.id_at(2).unwrap();
            assert_eq!(instances.instance(instance).unwrap().declaration().blocks().len(), 1);
            // Captureless unit consumer. Partial construction reserves the
            // first Vec header and one block range, then denies header two.
            let headers = source_reference_liveness_headers_v29::<()>().unwrap();
            let available = if partial { headers + 2 * std::mem::size_of::<Vec<std::ops::Range<usize>>>()
                + std::mem::size_of::<std::ops::Range<usize>>() - 1 } else { headers - 1 };
            budget.reserve_storage(LIMIT - budget.storage() - available).unwrap();
            let floor = budget.storage();
            let before_work = budget.work();
            let error = with_source_reference_liveness_v29(&mut builder, instance, &mut budget, |_, _, _|
                Ok(())).unwrap_err();
            assert!(matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))
                if error.actual() == LIMIT + 1 && error.limit() == LIMIT), "{error:?}");
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work() - before_work, if partial { 22 } else { 5 });
            let before_work = budget.work();
            assert!(builder.plan.check_owner(instances, &mut budget).is_err());
            assert_eq!(budget.work(), before_work);
        }
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    }).unwrap();
}

#[test]
fn completed_statement_liveness_scope_cleans_semantic_error_and_unwind_but_not_lost_custody() {
    let owner = completed_statement_owner_v29(0);
    let mut outer_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut outer = ArgumentBudgetV1::new(&mut outer_work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut outer,
        |instances, _| {
            for fault in 0..4 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                let mut builder = SourceReferenceBuilderV29::new(instances, &mut budget).unwrap();
                let floor = budget.storage();
                let instance = instances.id_at(2).unwrap();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_source_reference_liveness_v29(
                        &mut builder,
                        instance,
                        &mut budget,
                        |_, context, budget| {
                            assert!(!context.statements.is_empty());
                            if fault >= 2 {
                                budget.release_storage(1)?;
                            }
                            if fault == 1 || fault == 3 {
                                panic!("completed statement scratch sentinel");
                            }
                            Err::<(), _>(source_reference_error_v29(
                                "completed statement semantic sentinel",
                            ))
                        },
                    )
                }));
                if fault < 2 {
                    assert_eq!(budget.storage(), floor);
                    assert!(builder.plan.failure.first_error().is_none());
                } else {
                    assert!(budget.storage() > floor);
                    assert!(matches!(
                        builder.plan.failure.first_error(),
                        Some(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                }
                if fault == 1 || fault == 3 {
                    assert_eq!(
                        outcome.unwrap_err().downcast_ref::<&str>(),
                        Some(&"completed statement scratch sentinel")
                    );
                } else {
                    assert!(matches!(
                        outcome.unwrap(),
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "completed statement semantic sentinel",
                            ..
                        })
                    ));
                }
            }
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[test]
fn completed_statement_liveness_unrelated_scalar_prefix_has_independent_linear_work_increment() {
    let mut observed = Vec::new();
    for prefix in [0usize, 8, 32] {
        let owner = owner_with(Case::Writeback, |_, functions| {
            let helper = &functions[2];
            assert_eq!(helper.blocks().len(), 1);
            let mut statements: Vec<_> = (0..prefix)
                .map(|_| assign(place(3, WORD), fixture_word(19)))
                .collect();
            statements.extend_from_slice(helper.blocks()[0].statements());
            functions[2] = function(
                30,
                false,
                CAPTURE,
                helper.locals().to_vec(),
                vec![block(
                    30,
                    statements,
                    helper.blocks()[0].terminator().kind().clone(),
                )],
            );
        });
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
                let instance = instances.id_at(2).unwrap();
                let before = (budget.work(), budget.storage());
                with_source_reference_liveness_v29(
                    &mut builder,
                    instance,
                    budget,
                    |_, context, budget| {
                        assert_eq!(context.blocks.len(), 1);
                        assert!(context.deaths.len() >= prefix);
                        for statement in 0..prefix {
                            assert_eq!(
                                &context.deaths[context.statements[statement].clone()],
                                &[SemanticLocalIdV1::from_index(3)]
                            );
                        }
                        let rows = context.blocks.capacity()
                            * std::mem::size_of::<std::ops::Range<usize>>()
                            + context.statements.capacity()
                                * std::mem::size_of::<std::ops::Range<usize>>()
                            + context.deaths.capacity() * std::mem::size_of::<SemanticLocalIdV1>();
                        assert!(budget.storage() >= before.1 + rows);
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(budget.storage(), before.1);
                observed.push(budget.work() - before.0);
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
    }
    // Per inserted Define: resize1 + source join10 + statement4 + reverse
    // grouping1 + death visit5 + reverse transfer4. No extra loan/CFG work.
    assert_eq!(observed[1] - observed[0], 8 * 25);
    assert_eq!(observed[2] - observed[0], 32 * 25);
}

#[test]
fn completed_statement_liveness_failure_move_keeps_normal_successor_and_edge_results() {
    let owner = owner_with(Case::Shared, |types, functions| {
        let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([91; 32]),
            SemanticLayoutIdentityV1::from_sha256([91; 32]),
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
        ));
        let helper = &functions[2];
        let first = block(
            30,
            helper.blocks()[0].statements().to_vec(),
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    boolean,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
                )),
                expected: true,
                message: SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Move(place(
                    3, WORD,
                ))),
                target: SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::AssertSuccess,
                    SemanticBlockIdV1::from_index(1),
                ),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        );
        functions[2] = function(
            30,
            false,
            CAPTURE,
            helper.locals().to_vec(),
            vec![
                first,
                block(
                    31,
                    vec![assign(
                        place(3, WORD),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
    });
    for (owner, assertion) in [
        (owner, true),
        (cells_tests::return_alias_owner(false), false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            let mut found = false;
            for ordinal in 0..instances.instances().len() {
                let instance = instances.id_at(ordinal).unwrap();
                let occurrences = instances.occurrences(instance).unwrap();
                if assertion && instances.instance(instance).unwrap().function().index() == 2 {
                    let first = occurrences.terminal_failure_start(fe2o3_mir_model::SsaBlockIdV1::new(0)).unwrap();
                    assert!(occurrences.events().iter().any(|event| event.ordinal() as usize >= first
                        && matches!(event.event(), fe2o3_mir_model::SsaEventV1::Kill(variable) if variable.get() == 3)));
                    assert!(instances.instance(instance).unwrap().ssa().plan().live_in(fe2o3_mir_model::SsaBlockIdV1::new(1)).unwrap()
                        .iter().any(|variable| variable.get() == 3));
                    found = true;
                } else if !assertion {
                    for definition in occurrences.edge_definitions() {
                        let edge = occurrences.successors().iter().find(|edge| edge.id() == definition.edge()).unwrap();
                        if instances.instance(instance).unwrap().ssa().plan().live_in(fe2o3_mir_model::SsaBlockIdV1::new(edge.edge().target().index()))
                            .is_some_and(|live| live.contains(&definition.variable()))
                        { found = true; }
                    }
                }
                with_source_reference_liveness_v29(&mut builder, instance, budget, |_, _, _| Ok(())).unwrap();
            }
            assert!(found, "fixture must exercise its required failure/edge definition boundary");
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        }).unwrap();
        cells_tests::run_cells(owner, |_, _| Ok(())).unwrap();
    }
}

#[test]
fn completed_statement_liveness_changes_only_node_not_generation_storage_or_observations() {
    let owner = owner_with(Case::Shared, |_, functions| {
        let helper = &functions[2];
        let mut statements = vec![statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(2),
        ))];
        statements.extend_from_slice(helper.blocks()[0].statements());
        functions[2] = function(
            30,
            false,
            CAPTURE,
            helper.locals().to_vec(),
            vec![block(
                30,
                statements,
                helper.blocks()[0].terminator().kind().clone(),
            )],
        );
    });
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            builder.function(instances.root(), None, budget).unwrap();
            let instance = instances.id_at(2).unwrap();
            let local = SemanticLocalIdV1::from_index(2);
            let frame = builder
                .plan
                .states
                .iter()
                .position(|state| state.get(2).is_some_and(|local| local.generation == 1))
                .unwrap();
            builder.frames[instance.index()] = Some(frame);
            let original = builder.local(instance, local).unwrap();
            assert_eq!(original.generation, 1);
            assert!(original.storage.is_none());
            let node = builder
                .plan
                .nodes
                .iter()
                .position(|node| {
                    matches!(node.kind, SourceReferenceNodeKindV29::Loan(_)) && node.ty == REFERENCE
                })
                .unwrap();
            let loan_count = builder.plan.loans.len();
            let origin_count = builder.plan.origins.len();
            let activation_count = builder.plan.storage_activations.len();
            let access_count = builder.plan.accesses.len();
            with_source_reference_liveness_v29(
                &mut builder,
                instance,
                budget,
                |builder, context, budget| {
                    let site = SourceReferenceSiteV29 {
                        instance,
                        block: SemanticBlockIdV1::from_index(0),
                        statement: Some(2),
                    };
                    assert!(
                        context
                            .completed(&builder.plan, site, budget)?
                            .contains(&local)
                    );
                    builder.set_local(
                        instance,
                        local,
                        SourceReferenceLocalV29 {
                            node: Some(node),
                            ..original
                        },
                    )?;
                    builder.expire_completed_statement_v29(context, site, budget)?;
                    assert_eq!(
                        builder.local(instance, local)?,
                        SourceReferenceLocalV29 {
                            node: None,
                            ..original
                        }
                    );
                    // A retained-memory marker is an explicit negative: it grants no
                    // storage authority and must not be consulted or erased here.
                    let retained = SourceReferenceLocalV29 {
                        node: Some(node),
                        storage: Some(usize::MAX),
                        ..original
                    };
                    builder.set_local(instance, local, retained)?;
                    builder.expire_completed_statement_v29(context, site, budget)?;
                    assert_eq!(builder.local(instance, local)?, retained);
                    for kind in [
                        SourceReferenceNodeKindV29::Address(usize::MAX),
                        SourceReferenceNodeKindV29::EnumView(usize::MAX),
                    ] {
                        let copied = SourceReferenceNodeV29 {
                            kind,
                            ..builder.plan.nodes[node]
                        };
                        let index = builder.plan.nodes.len();
                        emission_push_v1(&mut builder.plan.nodes, copied, budget)?;
                        let opaque = SourceReferenceLocalV29 {
                            node: Some(index),
                            ..original
                        };
                        builder.set_local(instance, local, opaque)?;
                        builder.expire_completed_statement_v29(context, site, budget)?;
                        assert_eq!(builder.local(instance, local)?, opaque);
                    }
                    assert_eq!(
                        (
                            builder.plan.loans.len(),
                            builder.plan.origins.len(),
                            builder.plan.storage_activations.len(),
                            builder.plan.accesses.len()
                        ),
                        (loan_count, origin_count, activation_count, access_count)
                    );
                    Ok(())
                },
            )
            .unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}
