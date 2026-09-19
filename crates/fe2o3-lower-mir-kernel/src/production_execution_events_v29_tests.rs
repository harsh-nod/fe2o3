use super::*;

#[test]
fn scalar_sibling_arithmetic_uses_retained_operands_and_preserves_results() {
    for shape in [
        Shape::ScalarBinary,
        Shape::ScalarUnary,
        Shape::ScalarCast,
        Shape::ScalarCheckedBinary,
    ] {
        lower_cfg_fixture(
            shape,
            |_| {},
            |_, seed, result| {
                let result = result.unwrap();
                let body = result.function.body.unwrap();
                let operations = &body.blocks[0].operations;
                let scalar = source_operand_constant_v29(operations, Constant::U32(42));
                let expected = match shape {
                    Shape::ScalarUnary => OperationKind::Unary {
                        op: UnaryOp::Not,
                        operand: scalar,
                    },
                    Shape::ScalarCast => OperationKind::Cast {
                        kind: CastKind::ZeroExtend,
                        value: scalar,
                        to: Type::Scalar(ScalarType::U64),
                    },
                    _ => OperationKind::Binary {
                        op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                        lhs: scalar,
                        rhs: scalar,
                    },
                };
                let emitted = operations.iter().find(|op| op.kind == expected).unwrap();
                assert_eq!(
                    operations.iter().filter(|op| op.kind == expected).count(),
                    1
                );
                assert_eq!(
                    emitted.results[0].ty,
                    Type::Scalar(if matches!(shape, Shape::ScalarCast) {
                        ScalarType::U64
                    } else {
                        ScalarType::U32
                    })
                );
                let observation = result.execution_observation.unwrap();
                let local = if matches!(shape, Shape::ScalarCast | Shape::ScalarCheckedBinary) {
                    5
                } else {
                    3
                };
                let binding = observation.locals[local].as_ref().unwrap();
                if matches!(shape, Shape::ScalarCheckedBinary) {
                    assert_eq!(emitted.results.len(), 2);
                    let SemanticValueBindingV1::Aggregate(fields) = binding else {
                        panic!("missing checked result and overflow flag");
                    };
                    assert_eq!(fields.len(), 2);
                    for (field, result) in fields.iter().zip(&emitted.results) {
                        assert_eq!(field.value().unwrap(), (result.id, result.ty.clone()));
                    }
                    assert_eq!(emitted.results[1].ty, Type::BOOL);
                } else {
                    assert_eq!(
                        binding.value().unwrap(),
                        (emitted.results[0].id, emitted.results[0].ty.clone())
                    );
                }
                let Some(SemanticValueBindingV1::Aggregate(fields)) = &observation.locals[4] else {
                    panic!("missing source tuple");
                };
                assert!(
                    matches!(&fields[0], SemanticValueBindingV1::Execution(value) if value == seed)
                );
                assert_eq!(
                    fields[1].value().unwrap(),
                    (scalar, Type::Scalar(ScalarType::U32))
                );
            },
        );
    }
}

#[test]
fn scalar_sibling_switch_and_assume_consume_the_selected_scalar() {
    for shape in [Shape::ScalarSwitch, Shape::ScalarAssume] {
        lower_cfg_fixture(
            shape,
            |_| {},
            |_, seed, result| {
                let result = result.unwrap();
                let body = result.function.body.unwrap();
                let entry = &body.blocks[0];
                let scalar = source_operand_constant_v29(
                    &entry.operations,
                    if matches!(shape, Shape::ScalarAssume) {
                        Constant::Bool(true)
                    } else {
                        Constant::U32(42)
                    },
                );
                if matches!(shape, Shape::ScalarSwitch) {
                    let Some(Terminator::Switch {
                        selector,
                        cases,
                        default_target,
                        ..
                    }) = &entry.terminator
                    else {
                        panic!("missing scalar sibling switch");
                    };
                    assert_eq!(*selector, scalar);
                    assert!(cases.is_empty());
                    assert_eq!(*default_target, BlockId(18));
                } else {
                    assert_eq!(entry.operations.len(), 1);
                    assert!(
                        matches!(&entry.terminator, Some(Terminator::Return { values }) if values.is_empty())
                    );
                }
                let observation = result.execution_observation.unwrap();
                assert!(observation.bindings.values().any(|binding| {
                    matches!(binding, SemanticValueBindingV1::Aggregate(fields)
                        if matches!(&fields[0], SemanticValueBindingV1::Execution(value) if value == seed)
                        && fields[1].value().unwrap().0 == scalar)
                }));
            },
        );
    }
}

#[test]
fn scalar_sibling_asserts_consume_conditions_and_discarded_diagnostics() {
    for shape in [
        Shape::AssertMessage,
        Shape::AssertMessagePair,
        Shape::AssertConstant,
        Shape::AssertCondition,
        Shape::AssertConditionFolded,
    ] {
        lower_cfg_fixture(
            shape,
            |_| {},
            |_, seed, result| {
                let result = result.unwrap();
                let body = result.function.body.unwrap();
                let entry = &body.blocks[0];
                let condition =
                    source_operand_constant_v29(&entry.operations, Constant::Bool(true));
                if matches!(shape, Shape::AssertConditionFolded) {
                    assert!(
                        matches!(&entry.terminator, Some(Terminator::Branch { target, arguments })
                        if *target == BlockId(18) && arguments.is_empty())
                    );
                    assert_eq!(body.blocks.len(), 2);
                } else {
                    assert!(matches!(&entry.terminator,
                        Some(Terminator::ConditionalBranch { condition: actual, then_target, .. })
                        if *actual == condition && *then_target == BlockId(18)));
                }
                let scalar =
                    if matches!(shape, Shape::AssertCondition | Shape::AssertConditionFolded) {
                        condition
                    } else {
                        source_operand_constant_v29(&entry.operations, Constant::U32(42))
                    };
                let observation = result.execution_observation.unwrap();
                assert!(observation.bindings.values().any(|binding| {
                    matches!(binding, SemanticValueBindingV1::Aggregate(fields)
                        if matches!(&fields[0], SemanticValueBindingV1::Execution(value) if value == seed)
                        && fields[1].value().unwrap().0 == scalar)
                }));
            },
        );
    }
}

#[test]
fn scalar_arithmetic_does_not_restore_a_moved_nominal_sibling() {
    lower_cfg_fixture(
        Shape::ScalarMovedSibling,
        |_| {},
        |_, seed, result| {
            let result = result.unwrap();
            let body = result.function.body.unwrap();
            let scalar = source_operand_constant_v29(&body.blocks[0].operations, Constant::U32(42));
            let emitted = body.blocks[0]
                .operations
                .iter()
                .find(|op| {
                    op.kind
                        == OperationKind::Binary {
                            op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                            lhs: scalar,
                            rhs: scalar,
                        }
                })
                .unwrap();
            let observation = result.execution_observation.unwrap();
            let Some(SemanticValueBindingV1::Aggregate(fields)) = &observation.locals[4] else {
                panic!("missing partially moved tuple");
            };
            assert!(matches!(fields[0], SemanticValueBindingV1::MovedExecution));
            assert_eq!(fields[1].value().unwrap().0, scalar);
            assert!(
                matches!(&observation.locals[6], Some(SemanticValueBindingV1::Execution(value)) if value == seed)
            );
            assert_eq!(
                observation.locals[3].as_ref().unwrap().value().unwrap().0,
                emitted.results[0].id
            );
        },
    );
}

#[test]
fn every_new_scalar_sibling_source_event_is_mandatory() {
    use ExecutionOperandV29 as Role;
    for (shape, expected) in [
        (
            Shape::ScalarBinary,
            vec![Role::RvalueOperand(0), Role::RvalueOperand(1)],
        ),
        (Shape::ScalarUnary, vec![Role::RvalueOperand(0)]),
        (Shape::ScalarCast, vec![Role::RvalueOperand(0)]),
        (
            Shape::ScalarCheckedBinary,
            vec![Role::RvalueOperand(0), Role::RvalueOperand(1)],
        ),
        (Shape::ScalarSwitch, vec![Role::SwitchDiscriminant]),
        (Shape::ScalarAssume, vec![Role::Assume]),
        (Shape::AssertMessage, vec![Role::AssertMessage(0)]),
        (
            Shape::AssertMessagePair,
            vec![Role::AssertMessage(0), Role::AssertMessage(1)],
        ),
        (Shape::AssertCondition, vec![Role::AssertCondition]),
        (Shape::AssertConditionFolded, vec![Role::AssertCondition]),
    ] {
        let mut required = Vec::new();
        lower_cfg_fixture_with_cursor(
            shape,
            |_| {},
            |cursor| {
                required.extend_from_slice(&cursor.events.required);
                let operands = required
                    .iter()
                    .skip(3)
                    .map(|index| {
                        let event = &cursor.occurrences.events()[*index];
                        assert_eq!(event.role(), ExecutionEventV29::BaseUse);
                        assert!(event.is_promoted() && event.resolved().is_some());
                        assert!(
                            cursor
                                .retained_operand(event.site(), event.operand())
                                .is_some()
                        );
                        assert!(
                            cursor
                                .retained_operand(event.site(), Role::RvalueOperand(2))
                                .is_none()
                        );
                        assert!(
                            cursor
                                .retained_operand(event.site(), Role::CallArgument(0))
                                .is_none()
                        );
                        event.operand()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(operands, expected, "{shape:?}");
            },
            |_, _, result| {
                result.unwrap();
            },
        );
        for omitted in required {
            lower_cfg_fixture_with_cursor(
                shape,
                |_| {},
                |cursor| {
                    cursor.skipped_event = Some(omitted);
                },
                |_, _, result| {
                    let error = result.err().expect("an omitted source event must reject");
                    assert_eq!(
                        format!("{error:?}"),
                        format!("{:?}", execution_availability_error_v29()),
                        "{shape:?}, omitted {omitted}"
                    );
                },
            );
        }
    }
}

#[test]
fn scalar_sibling_hooks_do_not_permit_owned_field_copies() {
    lower_cfg_fixture(
        Shape::CopyOwnedSibling,
        |_| {},
        |_, _, result| {
            let error = result.err().expect("owned execution copy must reject");
            assert!(format!("{error:?}").contains("owned execution roles cannot be copied"));
        },
    );
}

#[test]
fn selected_field_archive_checks_reject_moved_and_stale_nominal_values() {
    lower_cfg_fixture(
        Shape::ScalarMovedSibling,
        |_| {},
        |owner, seed, result| {
            let observation = result.unwrap().execution_observation.unwrap();
            let definition = owner
                .plan_for_function(ROOT)
                .unwrap()
                .plan()
                .resolved_events(SsaBlockIdV1::new(0))
                .unwrap()
                .iter()
                .find_map(|(_, event)| match event {
                    SsaResolvedEventV1::Define { variable, value } if variable.get() == 4 => {
                        Some(*value)
                    }
                    _ => None,
                })
                .unwrap();
            let selected = |field, ty| {
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(4),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty)
                            .unwrap(),
                    ],
                    ty,
                )
                .unwrap()
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
            check_execution_archive_v29(
                &observation.locals,
                &observation.bindings,
                &selected(1, U32),
                definition,
                &mut budget,
            )
            .unwrap();
            assert!(
                check_execution_archive_v29(
                    &observation.locals,
                    &observation.bindings,
                    &selected(0, CONTEXT),
                    definition,
                    &mut budget
                )
                .is_err()
            );
            let mut stale = observation.locals.clone();
            let Some(SemanticValueBindingV1::Aggregate(fields)) = &mut stale[4] else {
                panic!("missing partially moved tuple");
            };
            let mut changed = seed.clone();
            changed.identity.producer.block = SemanticBlockIdV1::from_index(99);
            fields[0] = SemanticValueBindingV1::Execution(changed);
            assert!(
                check_execution_archive_v29(
                    &stale,
                    &observation.bindings,
                    &selected(0, CONTEXT),
                    definition,
                    &mut budget
                )
                .is_err()
            );
            assert!(
                check_execution_archive_v29(
                    &observation.locals,
                    &BTreeMap::new(),
                    &selected(1, U32),
                    definition,
                    &mut budget
                )
                .is_err()
            );
            let SemanticValueBindingV1::Aggregate(archived) = &observation.bindings[&definition]
            else {
                panic!("missing immutable tuple archive");
            };
            assert!(
                matches!(&archived[0], SemanticValueBindingV1::Execution(value) if value == seed)
            );
        },
    );
}

fn source_operand_constant_v29(operations: &[Operation], constant: Constant) -> ValueId {
    operations
        .iter()
        .find(|op| op.kind == OperationKind::Constant(constant.clone()))
        .expect("missing source scalar constant")
        .results[0]
        .id
}

#[test]
fn scalar_assert_diagnostics_obey_exact_work_and_peak_storage_limits() {
    let run = |limits| -> Result<(usize, usize), String> {
        let mut emitted = Err("emitter was not reached".to_owned());
        let usage = lower_cfg_fixture_with_limits(
            Shape::AssertMessagePair,
            limits,
            |_| {},
            |_| {},
            |_, _, result| {
                emitted = result.map(|_| ()).map_err(|error| format!("{error:?}"));
            },
        )?;
        emitted.map(|()| usage)
    };
    // Includes captured-source setup and emission; released scratch still counts
    // toward the peak, so the storage boundary is not the final live footprint.
    let (work, storage) = run((10_000_000, 10_000_000)).unwrap();
    assert!(work > 0 && storage > 0);
    assert_eq!(run((work, storage)).unwrap(), (work, storage));
    let short_work = run((work - 1, storage)).unwrap_err();
    assert!(short_work.contains("Work("), "{short_work}");
    let short_storage = run((work, storage - 1)).unwrap_err();
    assert!(short_storage.contains("Storage("), "{short_storage}");
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
