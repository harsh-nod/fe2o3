use super::*;
use fe2o3_kernel_ir::{
    CanonicalGuardedGlobalReadErrorV1 as DomainError,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CheckedCanonicalSelectedSliceDomainsV30 as Selected,
    FormalGuardedMemoryResourceErrorV1 as DomainResource,
    with_canonical_selected_slice_domains_v30,
};

fn selected_module(parallel: bool, recurrence: bool) -> Module {
    let mut module = mixed_module();
    let function = &mut module.functions[1];
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let generic = Type::pointer(scalar.clone(), AddressSpace::Generic, AccessMode::ReadWrite);
    function.signature.parameters.extend([
        Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
        Type::BOOL,
    ]);
    let body = function.body.as_mut().unwrap();
    body.parameters.extend([ValueId(200), ValueId(201)]);
    let guard_ops = body.blocks[0].operations.split_off(4);
    assert_eq!(guard_ops.len(), 2, "length and exact bounds predicate");
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(201),
        then_target: BlockId(23),
        then_arguments: vec![],
        else_target: BlockId(24),
        else_arguments: vec![],
    });
    let mut guard_a = BasicBlock::new(BlockId(23));
    guard_a.operations = guard_ops;
    guard_a.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(103),
        then_target: BlockId(21),
        then_arguments: vec![],
        else_target: BlockId(22),
        else_arguments: vec![],
    });
    let mut guard_b = BasicBlock::new(BlockId(24));
    guard_b.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(202), Type::INDEX),
            OperationKind::SliceLength {
                slice: ValueId(200),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(203), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(101),
                rhs: ValueId(202),
            },
        ),
    ];
    guard_b.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(203),
        then_target: BlockId(26),
        then_arguments: vec![],
        else_target: BlockId(22),
        else_arguments: vec![],
    });
    let inject_a = &mut body.blocks[1];
    assert_eq!(inject_a.id, BlockId(21));
    inject_a.operations.truncate(2);
    inject_a.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(106), generic.clone()),
        OperationKind::Cast {
            kind: fe2o3_kernel_ir::CastKind::PointerToGeneric,
            value: ValueId(105),
            to: generic.clone(),
        },
    ));
    inject_a.terminator = Some(if parallel {
        Terminator::ConditionalBranch {
            condition: ValueId(201),
            then_target: BlockId(25),
            then_arguments: vec![ValueId(106)],
            else_target: BlockId(25),
            else_arguments: vec![ValueId(106)],
        }
    } else {
        Terminator::Branch {
            target: BlockId(25),
            arguments: vec![ValueId(106)],
        }
    });
    let mut inject_b = BasicBlock::new(BlockId(26));
    inject_b.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(204), pointer.clone()),
            OperationKind::SliceData {
                slice: ValueId(200),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(205), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(204),
                offset: ValueId(101),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(206), generic.clone()),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::PointerToGeneric,
                value: ValueId(205),
                to: generic.clone(),
            },
        ),
    ];
    inject_b.terminator = Some(Terminator::Branch {
        target: BlockId(25),
        arguments: vec![ValueId(206)],
    });
    let mut join = BasicBlock::new(BlockId(25));
    join.parameters.push(ValueDef::new(ValueId(300), generic));
    join.operations = vec![
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(300),
                value: ValueId(20),
                access: MemoryAccess::new(AddressSpace::Generic, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(301), scalar),
            OperationKind::Load {
                pointer: ValueId(300),
                access: MemoryAccess::new(AddressSpace::Generic, 4),
            },
        ),
    ];
    let mut latch = None;
    if recurrence {
        for (id, value) in [(400, 0), (401, 1), (402, 2)] {
            body.blocks[0].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(value)),
            ));
        }
        let add_seed = |terminator: &mut Option<Terminator>| match terminator.as_mut().unwrap() {
            Terminator::Branch { arguments, .. } => arguments.push(ValueId(400)),
            Terminator::ConditionalBranch {
                then_arguments,
                else_arguments,
                ..
            } => {
                then_arguments.push(ValueId(400));
                else_arguments.push(ValueId(400));
            }
            _ => panic!("selected injection branch"),
        };
        add_seed(&mut body.blocks[1].terminator);
        add_seed(&mut inject_b.terminator);
        join.parameters
            .push(ValueDef::new(ValueId(403), Type::Scalar(ScalarType::U32)));
        join.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(404), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(403),
                rhs: ValueId(402),
            },
        ));
        join.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(404),
            then_target: BlockId(27),
            then_arguments: vec![],
            else_target: BlockId(22),
            else_arguments: vec![],
        });
        let mut backedge = BasicBlock::new(BlockId(27));
        backedge.operations.push(Operation::new(
            vec![
                ValueDef::new(ValueId(405), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(406), Type::BOOL),
            ],
            OperationKind::Binary {
                op: fe2o3_kernel_ir::BinaryOp::Checked(fe2o3_kernel_ir::CheckedBinaryOperator::Add),
                lhs: ValueId(403),
                rhs: ValueId(401),
            },
        ));
        backedge.terminator = Some(Terminator::Branch {
            target: BlockId(25),
            arguments: vec![ValueId(300), ValueId(405)],
        });
        latch = Some(backedge);
    } else {
        join.terminator = Some(Terminator::Return { values: vec![] });
    }
    body.blocks.extend([guard_a, guard_b, inject_b, join]);
    if let Some(latch) = latch {
        body.blocks.push(latch);
    }
    module
}

fn with_selected_case(
    module: &Module,
    mut run: impl FnMut(
        &mut PendingCanonicalRankedSourceRolesV18<'_, '_>,
        &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        &Selected<'_, '_>,
        &mut Budget<'_>,
    ) -> Result<(), Failure>,
) -> Result<(), Failure> {
    with_physical(module, |checked, physical, budget| {
        let floor = budget.storage();
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                let mut returned = None;
                let domains = with_canonical_selected_slice_domains_v30(
                    physical.inventory().owner(),
                    &[ExplicitLaunchExtent::Exact {
                        rank: 1,
                        extents: [64, 1, 1],
                    }; 3],
                    FormalIndexWidth::Bits64,
                    Default::default(),
                    budget,
                    |domains, budget| {
                        returned = Some(run(pending, physical, domains, budget));
                        Ok(())
                    },
                )
                .map_err(Failure::ConditionalGlobalsV26)?;
                assert!(domains.is_some(), "complete selected whole-module family");
                returned.expect("selected domain callback")
            },
        );
        assert_eq!(budget.storage(), floor);
        result
    })
}

#[test]
fn selected_native_v30_preserves_ordered_diamond_parallel_and_recurrent_choices() {
    for (parallel, recurrence) in [(false, false), (true, false), (true, true)] {
        let complete = Cell::new(false);
        with_selected_case(
            &selected_module(parallel, recurrence),
            |pending, physical, domains, budget| {
                let obligations = pending.obligations(budget)?.to_vec();
                pending
                    .with_selected_memory_observations_v30(
                        physical,
                        domains,
                        budget,
                        |view, budget| {
                            let view: &crate::PendingCanonicalSelectedMemoryPoliciesV30<'_, '_> =
                                view;
                            assert!(std::ptr::eq(
                                view.owner(budget)?,
                                physical.inventory().owner()
                            ));
                            assert!(std::ptr::eq(view.selected_domains(budget)?, domains));
                            assert_eq!(view.obligations(budget)?, obligations);
                            assert_eq!(view.function_count(budget)?, 3);
                            for function in 0..3 {
                                let report = view.report(function, budget)?.unwrap();
                                assert_eq!(report.paired_stage_count(), 9);
                                assert!(report.reports().is_clean());
                                for stage in 0..9 {
                                    assert_eq!(
                                        report.global_access_counts(stage),
                                        Some(if function == 1 { [1, 1] } else { [0, 0] })
                                    );
                                    assert_eq!(
                                        report.private_access_counts(stage),
                                        Some([1, 0, 1, 1, 0])
                                    );
                                }
                                assert_eq!(
                                    view.history(function, budget)?.unwrap().function(),
                                    function
                                );
                            }
                            let coordinate = FunctionCoordinate(1);
                            let incoming = domains
                                .incoming_edges(coordinate, budget)
                                .map_err(Failure::ConditionalGlobalsV26)?;
                            assert_eq!(
                                incoming.len(),
                                2 + usize::from(parallel) + usize::from(recurrence)
                            );
                            let accesses = domains
                                .accesses(coordinate, budget)
                                .map_err(Failure::ConditionalGlobalsV26)?;
                            assert_eq!(accesses.len(), 2);
                            for access in accesses {
                                let choices = domains
                                    .choices_at(access.operation(), budget)
                                    .map_err(Failure::ConditionalGlobalsV26)?
                                    .unwrap();
                                assert_eq!(choices.len(), 2 + usize::from(parallel));
                                assert_eq!(
                                    choices
                                        .iter()
                                        .filter(|choice| choice.domain.slice() == ValueId(100))
                                        .count(),
                                    1 + usize::from(parallel)
                                );
                                assert_eq!(
                                    choices
                                        .iter()
                                        .filter(|choice| choice.domain.slice() == ValueId(200))
                                        .count(),
                                    1
                                );
                            }
                            for parameter in [1usize, 2] {
                                let row = domains
                                    .parameters(coordinate, budget)
                                    .map_err(Failure::ConditionalGlobalsV26)?[parameter]
                                    .as_ref()
                                    .unwrap();
                                assert!(row.requires_valid_aligned_extent());
                                assert!(row.requires_initialized_extent());
                                assert!(row.requires_exclusive_runtime_binding());
                                assert!(row.requires_exact_launch_binding());
                            }
                            assert!(!view.source_roles_are_complete());
                            assert!(!view.runtime_requirements_are_discharged());
                            assert!(!view.ranked_verification_is_complete());
                            assert!(!view.grants_artifact_or_launch_authority());
                            complete.set(true);
                            Ok(())
                        },
                    )
                    .map_err(|error| error.failure)
            },
        )
        .unwrap();
        assert!(complete.get());
    }
    assert_ne!(
        std::any::TypeId::of::<crate::PendingCanonicalSelectedMemoryPoliciesV30<'static, 'static>>(
        ),
        std::any::TypeId::of::<crate::PendingCanonicalMixedMemoryPoliciesV26<'static, 'static>>()
    );
}

#[test]
fn selected_native_v30_censuses_selected_singleton_and_private_effects_together() {
    let mut module = selected_module(true, true);
    let body = module.functions[1].body.as_mut().unwrap();
    let block = body
        .blocks
        .iter_mut()
        .find(|block| block.id == BlockId(21))
        .unwrap();
    block.operations.extend([
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(105),
                value: ValueId(20),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(107), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(105),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    with_selected_case(&module, |pending, physical, domains, budget| {
        assert_eq!(
            domains
                .accesses(FunctionCoordinate(1), budget)
                .map_err(Failure::ConditionalGlobalsV26)?
                .len(),
            4
        );
        pending
            .with_selected_memory_observations_v30(physical, domains, budget, |view, budget| {
                let report = view.report(1, budget)?.unwrap();
                for stage in 0..9 {
                    assert_eq!(report.global_access_counts(stage), Some([2, 2]));
                    assert_eq!(report.private_access_counts(stage), Some([1, 0, 1, 1, 0]));
                }
                assert!(!view.source_roles_are_complete());
                assert!(!view.runtime_requirements_are_discharged());
                assert!(!view.grants_artifact_or_launch_authority());
                Ok(())
            })
            .map_err(|error| error.failure)
    })
    .unwrap();
}

#[test]
fn selected_native_v30_keeps_exact_and_one_short_nine_stage_analysis_limits() {
    with_selected_case(
        &selected_module(true, true),
        |pending, physical, domains, budget| {
            let observation = pending
                .with_selected_memory_observations_v30(physical, domains, budget, |view, budget| {
                    view.observation(budget)
                })
                .map_err(|error| error.failure)?;
            for (work, storage) in [
                (
                    observation.work_upper_bound() - 1,
                    observation.peak_storage_units(),
                ),
                (
                    observation.work_upper_bound(),
                    observation.peak_storage_units() - 1,
                ),
            ] {
                let called = Cell::new(false);
                let error = pending
                    .with_selected_memory_limits_v30(
                        physical,
                        domains,
                        Limits::new(work, storage),
                        budget,
                        |_, _| {
                            called.set(true);
                            Ok(())
                        },
                    )
                    .unwrap_err();
                assert!(!called.get());
                assert!(error.observation().first_denial().is_some());
                let invocation = error.last_invocation().unwrap();
                assert!(invocation.function() < 3);
                assert!(invocation.invocation().first_denial().is_some());
            }
            pending
                .with_selected_memory_limits_v30(
                    physical,
                    domains,
                    Limits::new(
                        observation.work_upper_bound(),
                        observation.peak_storage_units(),
                    ),
                    budget,
                    |view, budget| {
                        assert_eq!(view.report(1, budget)?.unwrap().paired_stage_count(), 9);
                        Ok(())
                    },
                )
                .map_err(|error| error.failure)
        },
    )
    .unwrap();
}

#[test]
fn selected_native_v30_refuses_foreign_physical_owner_before_callback() {
    let module = selected_module(false, false);
    let called = Cell::new(false);
    let result = with_selected_case(&module, |pending, _, domains, budget| {
        with_physical(&module, |_, foreign, _| {
            let error = pending
                .with_selected_memory_observations_v30(foreign, domains, budget, |_, _| {
                    called.set(true);
                    Ok(())
                })
                .unwrap_err();
            assert!(matches!(error.failure(), Failure::ExactGraph));
            Err(error.failure)
        })
    });
    assert!(matches!(result, Err(Failure::ExactGraph)));
    assert!(!called.get());
}

#[test]
fn selected_native_v30_ignored_query_refusal_retains_first_error_and_drops_callback_output() {
    let dropped = Cell::new(false);
    struct Payload<'a>(&'a Cell<bool>);
    impl Drop for Payload<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let result = with_selected_case(
        &selected_module(true, false),
        |pending, physical, domains, budget| {
            let error = pending
                .with_selected_memory_observations_v30(physical, domains, budget, |view, budget| {
                    assert!(matches!(
                        view.report(3, budget),
                        Err(Failure::InvalidQuery { function: 3 })
                    ));
                    Ok(Payload(&dropped))
                })
                .err()
                .unwrap();
            assert!(dropped.get());
            assert!(matches!(
                error.failure(),
                Failure::InvalidQuery { function: 3 }
            ));
            Err(error.failure)
        },
    );
    assert!(matches!(result, Err(Failure::InvalidQuery { function: 3 })));
    assert!(dropped.get());
}

#[test]
fn selected_native_v30_raw_accounting_denies_credit_before_all_scope_cleanup() {
    for first_invalid in [false, true] {
        with_physical(
            &selected_module(true, false),
            |checked, physical, budget| {
                let floor = budget.storage();
                let retained = Cell::new(None);
                let callback_count = Cell::new(0);
                let result = with_pending_canonical_ranked_source_roles_v18(
                    checked,
                    LAYOUTS,
                    budget,
                    |pending, budget| {
                        let mut first = None;
                        let domains = with_canonical_selected_slice_domains_v30(
                            physical.inventory().owner(),
                            &[ExplicitLaunchExtent::Exact {
                                rank: 1,
                                extents: [64, 1, 1],
                            }; 3],
                            FormalIndexWidth::Bits64,
                            Default::default(),
                            budget,
                            |domains, budget| {
                                let result: Result<(), _> = pending
                                    .with_selected_memory_observations_v30(
                                        physical,
                                        domains,
                                        budget,
                                        |view, budget| {
                                            callback_count.set(callback_count.get() + 1);
                                            if first_invalid {
                                                assert!(matches!(
                                                    view.report(3, budget),
                                                    Err(Failure::InvalidQuery { function: 3 })
                                                ));
                                            }
                                            retained.set(Some(budget.storage()));
                                            Err(Failure::Resource(Resource::Accounting))
                                        },
                                    )
                                    .map_err(|error| error.failure);
                                assert_eq!(Some(budget.storage()), retained.get());
                                if first_invalid {
                                    assert!(matches!(
                                        result,
                                        Err(Failure::InvalidQuery { function: 3 })
                                    ));
                                } else {
                                    assert!(matches!(
                                        result,
                                        Err(Failure::Resource(Resource::Accounting))
                                    ));
                                }
                                first = Some(result.unwrap_err());
                                Ok(())
                            },
                        );
                        assert!(matches!(
                            domains,
                            Err(DomainError::Resource(DomainResource::Accounting))
                        ));
                        assert_eq!(Some(budget.storage()), retained.get());
                        Err::<(), _>(first.expect("native callback refused"))
                    },
                );
                assert_eq!(callback_count.get(), 1);
                assert_eq!(Some(budget.storage()), retained.get());
                if first_invalid {
                    assert!(matches!(result, Err(Failure::InvalidQuery { function: 3 })));
                } else {
                    assert!(matches!(
                        result,
                        Err(Failure::Resource(Resource::Accounting))
                    ));
                }
                // Test-only credit disposal occurs after all genuine native/formal
                // scopes have refused cleanup without changing the retained amount.
                budget.release_storage(budget.storage() - floor).unwrap();
            },
        );
    }
}

#[test]
fn selected_native_v30_empty_and_uniform_families_do_not_discharge_source_or_runtime() {
    let mut empty = mixed_module();
    let function = &mut empty.functions[1];
    function.signature.parameters.truncate(1);
    let body = function.body.as_mut().unwrap();
    body.parameters.truncate(1);
    body.blocks.truncate(1);
    body.blocks[0].operations.truncate(3);
    body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
    for (module, count) in [(empty, 0usize), (mixed_module(), 2)] {
        with_selected_case(&module, |pending, physical, domains, budget| {
            pending
                .with_selected_memory_observations_v30(physical, domains, budget, |view, budget| {
                    assert_eq!(
                        domains
                            .accesses(FunctionCoordinate(1), budget)
                            .map_err(Failure::ConditionalGlobalsV26)?
                            .len(),
                        count
                    );
                    assert_eq!(view.function_count(budget)?, 3);
                    assert!(!view.source_roles_are_complete());
                    assert!(!view.runtime_requirements_are_discharged());
                    assert!(!view.grants_artifact_or_launch_authority());
                    Ok(())
                })
                .map_err(|error| error.failure)
        })
        .unwrap();
    }
}

#[test]
fn selected_native_v30_ordinary_callback_refusal_keeps_normal_refund() {
    let result = with_selected_case(
        &selected_module(false, false),
        |pending, physical, domains, budget| {
            let floor = budget.storage();
            let result: Result<(), _> = pending
                .with_selected_memory_observations_v30(physical, domains, budget, |_, _| {
                    Err(Failure::Callback("selected ordinary callback refusal"))
                })
                .map_err(|error| error.failure);
            assert!(matches!(
                result,
                Err(Failure::Callback("selected ordinary callback refusal"))
            ));
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                domains
                    .function_count(budget)
                    .map_err(Failure::ConditionalGlobalsV26)?,
                3
            );
            result
        },
    );
    assert!(matches!(
        result,
        Err(Failure::Callback("selected ordinary callback refusal"))
    ));
}

#[test]
fn selected_native_v30_keeps_real_unranked_cycle_progress_refusal() {
    let mut module = selected_module(true, true);
    let header = module.functions[1]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == BlockId(25))
        .unwrap();
    let Some(Terminator::ConditionalBranch { condition, .. }) = &mut header.terminator else {
        panic!("selected recurrent header");
    };
    *condition = ValueId(201);
    let reached = Cell::new(false);
    let result = with_selected_case(&module, |pending, physical, domains, budget| {
        pending
            .with_selected_memory_observations_v30(physical, domains, budget, |_, _| {
                reached.set(true);
                Ok(())
            })
            .map_err(|error| error.failure)
    });
    let Err(Failure::Analysis {
        function: 1,
        cause: crate::ProductionPlironPreloweringErrorV2::Semantic(cause),
    }) = result
    else {
        panic!("expected selected progress refusal: {result:?}");
    };
    assert!(!reached.get());
    assert!(
        cause
            .report()
            .progress()
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                crate::PlironProgressFindingV1::ProgressIncomplete { .. }
            ))
    );
}
