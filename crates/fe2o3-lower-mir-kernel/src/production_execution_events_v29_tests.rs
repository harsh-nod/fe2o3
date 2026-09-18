use super::*;

#[test]
fn unhooked_scalar_siblings_and_assert_diagnostics_fail_closed() {
    lower_cfg_fixture(
        Shape::AssertConstant,
        |_| {},
        |_, _, result| {
            result.unwrap();
        },
    );
    for shape in [
        Shape::ScalarBinary,
        Shape::ScalarSwitch,
        Shape::AssertMessage,
    ] {
        lower_cfg_fixture(
            shape,
            |_| {},
            |_, _, result| {
                let error = result.err().expect("unconsumed nominal-root read");
                assert_eq!(
                    format!("{error:?}"),
                    format!("{:?}", execution_availability_error_v29())
                );
            },
        );
    }
}

#[test]
fn ordinary_index_only_call_destination_does_not_invent_a_base_read() {
    let original = cfg_owner(Shape::Independent);
    let mut types = original.source_semantic().types().to_vec();
    let array = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([241; 32]),
        SemanticLayoutIdentityV1::from_sha256([241; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::array(0, 8),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: UNIT,
            length: 8,
        },
    ));
    let integer = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([242; 32]),
        SemanticLayoutIdentityV1::from_sha256([242; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    let edge = SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::CallReturn,
        SemanticBlockIdV1::from_index(1),
    );
    let root = function(
        243,
        SemanticFunctionRoleV1::KernelRoot,
        abi(243, true, &[]),
        vec![
            local(240, UNIT, SemanticLocalRoleV1::Return),
            local(241, array, SemanticLocalRoleV1::Temporary),
            local(242, integer, SemanticLocalRoleV1::Temporary),
            local(243, CONTEXT, SemanticLocalRoleV1::Temporary),
            local(245, U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                241,
                vec![assign(
                    place(2, integer),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            integer,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0, 8).unwrap(),
                            ),
                        ),
                    )),
                )],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(1),
                                vec![
                                    SemanticProjectionV1::new(
                                        SemanticProjectionKindV1::Index(
                                            SemanticLocalIdV1::from_index(2),
                                        ),
                                        UNIT,
                                    )
                                    .unwrap(),
                                ],
                                UNIT,
                            )
                            .unwrap(),
                            edge,
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(242, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(
        original.source_semantic().functions()[0]
            .kernel_entry()
            .unwrap()
            .clone(),
    );
    let helper = function(
        244,
        SemanticFunctionRoleV1::InternalHelper,
        abi(244, false, &[]),
        vec![local(244, UNIT, SemanticLocalRoleV1::Return)],
        vec![block(244, vec![], SemanticTerminatorKindV1::Return)],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        original.source_semantic().target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000_000);
    let captured = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(captured.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |cursor, _| {
            assert_eq!(cursor.cfg.nominal_locals[3], 1);
            assert!(cursor.events.required.is_empty());
            let events = cursor.occurrences.events();
            assert_eq!(events.len(), 2);
            assert_eq!(
                events[1].operand(),
                ExecutionOperandV29::CallDestinationAddress
            );
            assert_eq!(events[1].role(), ExecutionEventV29::ProjectionIndexUse(0));
            assert!(events[1].is_promoted() && events[1].resolved().is_some());
            Ok(())
        })
        .unwrap();
        Ok::<(), production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
}

#[test]
fn shared_emitter_consumes_storage_and_ordinary_siblings_of_nominal_roots() {
    lower_cfg_fixture_with_cursor(
        Shape::Storage,
        |_| {},
        |cursor| {
            let ordinals = cursor
                .events
                .required
                .iter()
                .map(|index| cursor.occurrences.events()[*index].ordinal())
                .collect::<Vec<_>>();
            assert_eq!(ordinals, [0, 1, 2, 3, 4, 5, 6, 8, 9]);
            assert!(matches!(
                cursor.occurrences.events()[0].resolved(),
                Some(SsaResolvedEventV1::Kill { previous: None, .. })
            ));
            assert!(matches!(
                cursor.occurrences.events()[9].resolved(),
                Some(SsaResolvedEventV1::Kill { previous: None, .. })
            ));
        },
        |_, seed, result| {
            let result = result.unwrap();
            let body = result.function.body.unwrap();
            let scalar = body.blocks[0]
                .operations
                .iter()
                .find(|op| op.kind == OperationKind::Constant(Constant::U32(42)))
                .unwrap()
                .results[0]
                .id;
            let observation = result.execution_observation.unwrap();
            assert!(observation.locals[1].is_none());
            assert!(observation.locals[4].is_none());
            assert!(
                matches!(&observation.locals[6], Some(SemanticValueBindingV1::Execution(value)) if value == seed)
            );
            assert!(
                matches!(observation.locals[3], Some(SemanticValueBindingV1::Value { id, .. }) if id == scalar)
            );
        },
    );
}

#[test]
fn shared_emitter_rejects_every_omitted_nominal_event_including_the_last() {
    for omitted in [0, 1, 2, 3, 4, 5, 6, 8, 9] {
        lower_cfg_fixture_with_cursor(
            Shape::Storage,
            |_| {},
            |cursor| {
                cursor.skipped_event = Some(omitted);
            },
            |_, _, result| {
                let error = result
                    .err()
                    .expect("a missing source event cannot produce a lowered function");
                assert_eq!(
                    format!("{error:?}"),
                    format!("{:?}", execution_availability_error_v29()),
                    "omitted {omitted}"
                );
            },
        );
    }
}

#[test]
fn independent_live_inputs_cannot_be_consumed_out_of_source_order() {
    let mut owner = cfg_owner(Shape::Independent);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000_000);
    let captured = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(captured.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            cursor.begin_block(block, budget)?;
            let initial = cursor.current.clone();
            assert!(initial[1].is_some() && initial[2].is_some());
            assert!(
                cursor
                    .use_place(
                        execution_site_v29(block, Some(1)),
                        ExecutionOperandV29::RvalueOperand(0),
                        &place(2, CONTEXT),
                        true,
                        budget
                    )
                    .is_err()
            );
            assert_eq!(cursor.current, initial);
            assert!(cursor.claimed.iter().all(|claimed| !claimed));
            assert!(cursor.finish_block(budget).is_err());
            for (statement, from, to) in [(0, 1, 4), (1, 2, 6)] {
                cursor.use_place(
                    execution_site_v29(block, Some(statement)),
                    ExecutionOperandV29::RvalueOperand(0),
                    &place(from, CONTEXT),
                    true,
                    budget,
                )?;
                cursor.define(
                    execution_site_v29(block, Some(statement)),
                    SemanticLocalIdV1::from_index(to),
                    source_definition(&cursor, 0, statement),
                    budget,
                )?;
            }
            cursor.finish_block(budget)?;
            cursor.finish(budget)?;
            assert!(cursor.finish(budget).is_err());
            assert!(cursor.begin_block(block, budget).is_err());
            Ok(())
        })
        .unwrap();
        Ok::<(), production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
    lower_cfg_fixture(
        Shape::Independent,
        |_| {},
        |_, _, result| {
            result.unwrap();
        },
    );
}

#[test]
fn completion_requires_edges_and_unvisited_eventless_blocks() {
    captured_execution(Flow::Linear, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            assert!(cursor.finish(budget).is_err());
            let block = SemanticBlockIdV1::from_index(0);
            cursor.begin_block(block, budget)?;
            assert!(cursor.events.pending.is_empty());
            assert!(cursor.finish_block(budget).is_err());
            assert!(
                cursor
                    .begin_block(SemanticBlockIdV1::from_index(4), budget)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            cursor.begin_block(SemanticBlockIdV1::from_index(4), budget)?;
            cursor.finish_block(budget)?;
            assert!(cursor.finish(budget).is_err());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn event_completion_has_exact_work_storage_and_ledger_boundaries() {
    captured_execution(Flow::Linear, |plan, _| {
        let helper = plan.calls(plan.root()).unwrap()[1].child().unwrap();
        let run = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(37).unwrap();
            let result =
                with_execution_availability_v29(plan, helper, &mut budget, |mut cursor, budget| {
                    let bytes = budget.storage();
                    let block = SemanticBlockIdV1::from_index(0);
                    cursor.begin_block(block, budget)?;
                    cursor.use_place(
                        execution_site_v29(block, Some(0)),
                        ExecutionOperandV29::RvalueOperand(0),
                        &place(1, CONTEXT),
                        true,
                        budget,
                    )?;
                    cursor.define(
                        execution_site_v29(block, Some(0)),
                        SemanticLocalIdV1::from_index(3),
                        source_definition(&cursor, 0, 0),
                        budget,
                    )?;
                    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(1000);
                    let mut other = ArgumentBudgetV1::new(&mut other_work, 1000);
                    assert!(cursor.finish_block(&mut other).is_err());
                    assert!(cursor.finish(&mut other).is_err());
                    assert_eq!(other.work(), 0);
                    cursor.finish_block(budget)?;
                    cursor.finish(budget)?;
                    Ok((budget.work(), bytes))
                });
            assert_eq!(budget.storage(), 37);
            result
        };
        let (work, storage) = run(10_000_000, 10_000_000).unwrap();
        assert_eq!(run(work, storage).unwrap(), (work, storage));
        assert!(run(work - 1, storage).is_err());
        assert!(run(work, storage - 1).is_err());
    });
}
