use super::*;

fn assertion_calls_owner(expected: bool, diamond: bool) -> ProductionSemanticMirOwnerV1 {
    let original = scalar_calls_owner(true);
    let semantic = original.semantic();
    let mut types = semantic.types().to_vec();
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([251; 32]),
        SemanticLayoutIdentityV1::from_sha256([251; 32]),
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
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[1];
    let source = helper.source();
    let scalar = SemanticTypeIdV1::from_index(1);
    let place =
        |local| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], scalar).unwrap();
    let operand = |local| SemanticOperandV1::Copy(place(local));
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let block = |index, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([219 + index; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let assertion = |target| SemanticTerminatorKindV1::Assert {
        condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
            boolean,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(u128::from(expected), 1).unwrap(),
            ),
        )),
        expected,
        message: SemanticAssertMessageV1::NullPointerDereference,
        target: edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    let call = |argument, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(2),
                vec![operand(argument)],
                Some(SemanticCallDestinationV1::new(
                    place(0),
                    edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let mut locals = helper.locals().to_vec();
    let blocks = if diamond {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([250; 32]),
            scalar,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
        let assign = |value| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(2),
                    SemanticRvalueV1::new(scalar, SemanticRvalueKindV1::Use(value)),
                )),
            )
        };
        vec![
            block(
                0,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: operand(1),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            3,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(1, vec![assign(operand(1))], assertion(3)),
            block(
                2,
                vec![assign(SemanticOperandV1::Constant(
                    SemanticConstantV1::new(
                        scalar,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(11, 4).unwrap()),
                    ),
                ))],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(3, vec![], call(2, 4)),
            block(4, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![
            block(0, vec![], assertion(1)),
            block(1, vec![], call(1, 2)),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    functions[1] = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        source,
        helper.abi().clone(),
        locals,
        helper.entry(),
        blocks,
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn with_assert_pending(
    expected: bool,
    diamond: bool,
    test: impl FnOnce(
        &mut PendingScopedRootEmissionV29,
        &ProductionCallInstancePlanV1<'_>,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    with_plan_owner(
        assertion_calls_owner(expected, diamond),
        |instances, budget| {
            let floor = budget.storage();
            let lowered = lower_scalar_instances(instances, budget);
            let input_storage: usize = lowered
                .iter()
                .map(|row| {
                    row.call_returns.requested_bytes().unwrap()
                        + row.instance_assert_origins.as_ref().unwrap().storage
                })
                .sum();
            let pointers: Vec<_> = lowered
                .iter()
                .map(|row| {
                    let capture = row.instance_assert_origins.as_ref().unwrap();
                    (capture.records.as_ptr(), capture.arguments.as_ptr())
                })
                .collect();
            assert_eq!(budget.storage(), floor + input_storage);
            let mut slots: Vec<_> = lowered.into_iter().map(Some).collect();
            let mut pending = assemble_pending_scoped_root_v29(
                instances,
                &mut slots,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap();
            assert!(slots.iter().all(Option::is_none));
            for (row, pointers) in pending.sidecars.rows.iter().zip(pointers) {
                let capture = row.instance_assert_origins.as_ref().unwrap();
                assert_eq!(
                    (capture.records.as_ptr(), capture.arguments.as_ptr()),
                    pointers
                );
            }
            test(&mut pending, instances, budget);
            let added = pending.additional_storage_bytes;
            drop(pending);
            drop(slots);
            budget.release_storage(added + input_storage).unwrap();
            assert_eq!(budget.storage(), floor);
        },
    );
}

fn capture(pending: &PendingScopedRootEmissionV29, index: usize) -> &InstanceAssertCaptureV1 {
    pending.sidecars.rows[index]
        .instance_assert_origins
        .as_ref()
        .unwrap()
}

#[test]
fn repeated_assertions_keep_instance_identity_entry_targets_and_merge_arguments() {
    for expected in [false, true] {
        for diamond in [false, true] {
            with_assert_pending(expected, diamond, |pending, instances, budget| {
                let a = capture(pending, 1);
                let b = capture(pending, 2);
                assert_eq!(a.records.len(), 1);
                assert_eq!(b.records.len(), 1);
                assert_ne!(a.instance, b.instance);
                let (a, b) = (&a.records[0], &b.records[0]);
                assert_eq!(a.site, b.site);
                assert_eq!(a.emitted_function, b.emitted_function);
                assert_ne!(a.block, b.block);
                assert_eq!(a.argument_count, usize::from(diamond));
                assert_eq!(b.argument_count, usize::from(diamond));
                let PendingAssertOutcomeV1::Emitted {
                    condition: ca,
                    failure: fa,
                } = a.outcome
                else {
                    panic!("not emitted")
                };
                let PendingAssertOutcomeV1::Emitted {
                    condition: cb,
                    failure: fb,
                } = b.outcome
                else {
                    panic!("not emitted")
                };
                assert_ne!(ca, cb);
                assert_ne!(fa, fb);
                for index in [1, 2] {
                    let origin = capture(pending, index);
                    let assertion = &origin.records[0];
                    let continuation = pending
                        .coordinates
                        .controls
                        .rows
                        .iter()
                        .find(|control| {
                            control.instance == origin.instance
                                && control.semantic_block == Some(assertion.semantic_success)
                                && control.origin == InstanceControlOriginV1::Retained
                        })
                        .unwrap();
                    assert_ne!(continuation.physical_block, assertion.physical_success);
                }
                let mut module = Module::new("expanded_assertions");
                let mut declarations = BTreeMap::new();
                for row in &pending.sidecars.rows {
                    declarations.extend(row.diagnostic_declarations.clone());
                }
                module.functions.extend(declarations.into_values());
                module.functions.push(pending.function.clone());
                module.kernels.push(Kernel::new(
                    "scalar_instance_root",
                    "scalar_instance_root",
                    LaunchDomain::D1 {
                        x: LaunchExtent::Static(64),
                    },
                ));
                verify_module(&module).unwrap();
                let before = budget.storage();
                replay_pending_instance_asserts_v1(pending, instances, budget).unwrap();
                assert_eq!(budget.storage(), before);
            });
        }
    }
}

#[test]
fn expanded_assertion_replay_refuses_identity_and_attachment_substitutions() {
    for fault in 0..18 {
        with_assert_pending(false, true, |pending, instances, budget| {
            let sibling = &capture(pending, 2).records[0];
            let PendingAssertOutcomeV1::Emitted {
                condition: sibling_condition,
                failure: sibling_failure,
            } = sibling.outcome
            else {
                unreachable!()
            };
            let sibling_success = sibling.physical_success;
            let own = capture(pending, 1);
            let block_id = own.records[0].block;
            let continuation = pending
                .coordinates
                .controls
                .rows
                .iter()
                .find(|control| {
                    control.instance == own.instance
                        && control.semantic_block == Some(own.records[0].semantic_success)
                        && control.origin == InstanceControlOriginV1::Retained
                })
                .unwrap()
                .physical_block;
            let own = pending.sidecars.rows[1]
                .instance_assert_origins
                .as_mut()
                .unwrap();
            match fault {
                0 => own.instance = instances.id_at(2).unwrap(),
                1 => own.source.semantic[0] ^= 1,
                2 => own.source.root = SemanticFunctionIdV1::from_index(1),
                3 => own.failed = true,
                4 => {
                    own.records.pop();
                }
                5 => own.records[0].argument_count += 1,
                6 => own.records[0].expected = true,
                7 => own.records[0].physical_success = continuation,
                8 => own.records[0].emitted_function.replace_range(0..1, "X"),
                9 => own.placement.first_block += 1,
                10 => own.records[0].outcome = PendingAssertOutcomeV1::ElidedByExistingRule,
                11..=14 => {
                    let block = pending
                        .function
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .iter_mut()
                        .find(|block| block.id == block_id)
                        .unwrap();
                    let Some(Terminator::ConditionalBranch {
                        condition,
                        then_target,
                        else_target,
                        else_arguments,
                        ..
                    }) = &mut block.terminator
                    else {
                        unreachable!()
                    };
                    match fault {
                        11 => *condition = sibling_condition,
                        12 => *then_target = sibling_failure,
                        13 => *else_target = sibling_success,
                        14 => else_arguments[0] = ValueId(u32::MAX),
                        _ => unreachable!(),
                    }
                }
                15 => {
                    let PendingAssertOutcomeV1::Emitted { failure, .. } = own.records[0].outcome
                    else {
                        unreachable!()
                    };
                    let block = pending
                        .function
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .iter_mut()
                        .find(|block| block.id == failure)
                        .unwrap();
                    block.terminator = Some(Terminator::Return { values: vec![] });
                }
                16 => own.records[0].first_operation += 1,
                17 => own.records[0].physical_success = sibling_success,
                _ => unreachable!(),
            }
            let floor = budget.storage();
            assert!(
                replay_pending_instance_asserts_v1(pending, instances, budget).is_err(),
                "fault {fault}"
            );
            assert_eq!(budget.storage(), floor, "fault {fault}");
        });
    }
}

#[test]
fn instance_assertion_replay_resource_boundaries_leave_no_scratch_charge() {
    let mut work_needed = 0;
    with_assert_pending(true, true, |pending, instances, budget| {
        let before = budget.work();
        replay_pending_instance_asserts_v1(pending, instances, budget).unwrap();
        work_needed = budget.work() - before;
    });
    for allowance in [0, 1, work_needed / 2, work_needed - 1, work_needed] {
        with_assert_pending(true, true, |pending, instances, budget| {
            budget
                .charge_work(LIMIT - budget.work() - allowance)
                .unwrap();
            let floor = budget.storage();
            let result = replay_pending_instance_asserts_v1(pending, instances, budget);
            assert_eq!(result.is_ok(), allowance == work_needed, "{result:?}");
            assert_eq!(budget.storage(), floor);
        });
    }
    with_assert_pending(true, true, |pending, instances, budget| {
        let reserve = LIMIT - budget.storage();
        budget.reserve_storage(reserve).unwrap();
        let floor = budget.storage();
        assert!(replay_pending_instance_asserts_v1(pending, instances, budget).is_err());
        assert_eq!(budget.storage(), floor);
        budget.release_storage(reserve).unwrap();
    });
}

#[test]
fn instance_assertion_identity_rejects_foreign_ledger_without_charging_it() {
    with_assert_pending(true, false, |pending, instances, _| {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
        assert!(matches!(
            capture(pending, 1).check_identity(
                instances,
                instances.id_at(1).unwrap(),
                &mut foreign
            ),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(foreign.work(), 0);
        assert_eq!(foreign.storage(), 0);
    });
}

fn empty_capture_like(original: &InstanceAssertCaptureV1) -> InstanceAssertCaptureV1 {
    InstanceAssertCaptureV1 {
        ledger: original.ledger,
        source: original.source,
        instance: original.instance,
        function: original.function,
        placement: original.placement,
        records: Vec::new(),
        arguments: Vec::new(),
        storage: 0,
        failed: false,
    }
}

#[test]
fn failed_instance_assert_append_keeps_grown_buffers_paid_and_poisoned() {
    with_assert_pending(true, true, |pending, instances, budget| {
        let old = capture(pending, 1);
        let mut empty = empty_capture_like(old);
        let recorded = &old.records[0];
        let span = SemanticKirTerminatorOperationSpanV1 {
            correspondence_owner: recorded.site.correspondence_owner,
            semantic_function: recorded.site.semantic_function,
            semantic_block: recorded.site.semantic_block,
            kernel_ir_block: recorded.block,
            first_operation_ordinal: recorded.first_operation,
            operation_count: recorded.operation_count,
        };
        let row = instances.instance(old.instance).unwrap();
        let source = row.declaration().blocks()[span.semantic_block.index() as usize]
            .terminator()
            .kind();
        let block = pending
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == span.kernel_ir_block)
            .unwrap();
        let name = FunctionId::new(recorded.emitted_function.clone());
        // Four scoped checks, source admission, two initial vector growths,
        // then the two-unit argument append charge. Name copying must fail.
        budget.charge_work(LIMIT - budget.work() - 9).unwrap();
        let floor = budget.storage();
        assert!(matches!(
            empty.record(
                span,
                &name,
                source,
                block.terminator.as_ref().unwrap(),
                false,
                budget
            ),
            Err(ProductionSemanticKirErrorV1::AssertOrigin(
                SemanticKirAssertOriginErrorV1::Resource(AssertOriginResourceV1::Work(_))
            ))
        ));
        assert!(empty.failed);
        assert!(empty.records.is_empty());
        assert!(empty.arguments.is_empty());
        assert_eq!(empty.records.capacity(), 4);
        assert_eq!(empty.arguments.capacity(), 4);
        assert_eq!(empty.storage, budget.storage() - floor);
        assert_eq!(
            empty.storage,
            4 * std::mem::size_of::<PendingAssertOriginV1>() + 4 * std::mem::size_of::<ValueId>()
        );
        let storage = empty.storage;
        drop(empty);
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn foreign_ledger_cannot_record_or_poison_an_instance_assert_capture() {
    with_assert_pending(true, true, |pending, instances, _| {
        let old = capture(pending, 1);
        let mut empty = empty_capture_like(old);
        let recorded = &old.records[0];
        let span = SemanticKirTerminatorOperationSpanV1 {
            correspondence_owner: recorded.site.correspondence_owner,
            semantic_function: recorded.site.semantic_function,
            semantic_block: recorded.site.semantic_block,
            kernel_ir_block: recorded.block,
            first_operation_ordinal: recorded.first_operation,
            operation_count: recorded.operation_count,
        };
        let source = instances
            .instance(old.instance)
            .unwrap()
            .declaration()
            .blocks()[span.semantic_block.index() as usize]
            .terminator()
            .kind();
        let emitted = pending
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == span.kernel_ir_block)
            .unwrap()
            .terminator
            .as_ref()
            .unwrap();
        let name = FunctionId::new(recorded.emitted_function.clone());
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
        assert!(matches!(
            empty.record(span, &name, source, emitted, false, &mut foreign),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((foreign.work(), foreign.storage()), (0, 0));
        assert_eq!(
            (empty.records.len(), empty.arguments.len(), empty.storage),
            (0, 0, 0)
        );
        assert!(!empty.failed);
    });
}
