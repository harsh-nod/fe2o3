use super::*;
#[path = "canonical_ranked_selected_pending_v30_tests.rs"]
mod selected_memory_v30;
use fe2o3_kernel_ir::{
    Axis, CheckedCanonicalConditionalSliceDomainsV26 as Globals, ComparePredicate,
    ExplicitLaunchExtent, FormalIndexWidth, FunctionRole, IntrinsicOperation,
    with_canonical_conditional_slice_domains_v26, with_canonical_guarded_global_reads_v18,
    with_canonical_guarded_global_stores_v24,
};

fn mixed_module() -> Module {
    let mut module = module(ScalarType::U32, 4, 4, false);
    let mut global = module.functions[0].clone();
    global.id = "mixed".into();
    global.signature.parameters.push(Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = global.body.as_mut().unwrap();
    body.parameters.push(ValueId(100));
    let entry = &mut body.blocks[0];
    entry.operations.extend([
        Operation::new(
            vec![ValueDef::new(ValueId(101), Type::INDEX)],
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(102), Type::INDEX)],
            OperationKind::SliceLength {
                slice: ValueId(100),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(103), Type::BOOL)],
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(101),
                rhs: ValueId(102),
            },
        ),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(103),
        then_target: BlockId(21),
        then_arguments: vec![],
        else_target: BlockId(22),
        else_arguments: vec![],
    });
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut access = BasicBlock::new(BlockId(21));
    access.operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(104), pointer.clone())],
            OperationKind::SliceData {
                slice: ValueId(100),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(105), pointer)],
            OperationKind::GetElementPointer {
                base: ValueId(104),
                offset: ValueId(101),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(105),
                value: ValueId(20),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(106), Type::Scalar(ScalarType::U32))],
            OperationKind::Load {
                pointer: ValueId(105),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    access.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(22));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([access, exit]);
    let mut helper = module.functions[0].clone();
    helper.id = "private_helper".into();
    helper.role = FunctionRole::InternalHelper;
    module.functions.extend([global, helper]);
    module.kernels.push(Kernel::new(
        "mixed",
        "mixed",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn with_mixed_case(
    module: &Module,
    mut run: impl FnMut(
        &mut PendingCanonicalRankedSourceRolesV18<'_, '_>,
        &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        &Globals<'_, '_>,
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
                let owner = physical.inventory().owner();
                let mut result = None;
                let domains = with_canonical_guarded_global_reads_v18(
                    owner,
                    Default::default(),
                    budget,
                    |reads, budget| {
                        with_canonical_guarded_global_stores_v24(
                            owner,
                            Default::default(),
                            budget,
                            |stores, budget| {
                                with_canonical_conditional_slice_domains_v26(
                                    reads,
                                    stores,
                                    &[ExplicitLaunchExtent::Exact {
                                        rank: 1,
                                        extents: [64, 1, 1],
                                    }; 3],
                                    FormalIndexWidth::Bits64,
                                    budget,
                                    |globals, budget| {
                                        result = Some(run(pending, physical, globals, budget));
                                        Ok(())
                                    },
                                )
                            },
                        )
                    },
                )
                .map_err(Failure::ConditionalGlobalsV26)?;
                assert!(domains.is_some(), "whole-module family");
                result.expect("conditional family callback")
            },
        );
        assert_eq!(budget.storage(), floor);
        result
    })
}

#[test]
fn mixed_pending_v26_checks_two_roots_and_private_only_helper_without_losing_requirements() {
    let finished = Cell::new(false);
    with_mixed_case(&mixed_module(), |pending, physical, globals, budget| {
        let owner = physical.inventory().owner();
        let before = pending.obligations(budget)?.to_vec();
        pending
            .with_mixed_memory_observations_v26(physical, globals, budget, |view, budget| {
                let view: &crate::PendingCanonicalMixedMemoryPoliciesV26<'_, '_> = view;
                assert!(std::ptr::eq(view.owner(budget)?, owner));
                assert_eq!(view.function_count(budget)?, 3);
                assert_eq!(view.obligations(budget)?, before.as_slice());
                for function in 0..3 {
                    let report = view.report(function, budget)?.expect("real definition");
                    assert_eq!(report.paired_stage_count(), 9);
                    assert!(report.reports().is_clean());
                    for stage in 0..9 {
                        assert_eq!(
                            report.global_access_counts(stage),
                            Some(if function == 1 { [1, 1] } else { [0, 0] })
                        );
                        assert_eq!(report.private_access_counts(stage), Some([1, 0, 1, 1, 0]));
                    }
                    assert_eq!(
                        view.history(function, budget)?.unwrap().function(),
                        function
                    );
                }
                let batch = view.conditional_globals(budget)?;
                assert!(std::ptr::eq(batch, globals));
                let requirements = batch
                    .parameter(
                        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(1),
                        1,
                        budget,
                    )
                    .map_err(Failure::ConditionalGlobalsV26)?
                    .unwrap();
                assert!(requirements.requires_valid_aligned_extent());
                assert!(requirements.requires_initialized_extent());
                assert!(requirements.requires_exclusive_runtime_binding());
                assert!(requirements.requires_exact_launch_binding());
                assert_eq!(requirements.invocation_axis(), Some(Axis::X));
                view.global_accesses(budget)?.check_owner(owner, budget)?;
                assert!(!view.source_roles_are_complete());
                assert!(!view.runtime_requirements_are_discharged());
                assert!(!view.ranked_verification_is_complete());
                assert!(!view.grants_artifact_or_launch_authority());
                finished.set(true);
                Ok(())
            })
            .map_err(|error| error.failure)
    })
    .unwrap();
    assert!(finished.get());
}

#[test]
fn mixed_pending_v26_ignored_invalid_query_cannot_publish_callback_value() {
    let dropped = Cell::new(false);
    struct Payload<'a>(&'a Cell<bool>);
    impl Drop for Payload<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let finished = Cell::new(false);
    let result = with_mixed_case(&mixed_module(), |pending, physical, globals, budget| {
        let error = pending
            .with_mixed_memory_observations_v26(physical, globals, budget, |view, budget| {
                assert!(matches!(
                    view.report(3, budget),
                    Err(Failure::InvalidQuery { function: 3 })
                ));
                finished.set(true);
                Ok(Payload(&dropped))
            })
            .err()
            .expect("ignored denial is sticky");
        assert!(matches!(
            error.failure(),
            Failure::InvalidQuery { function: 3 }
        ));
        assert!(dropped.get());
        Err(error.failure)
    });
    assert!(matches!(result, Err(Failure::InvalidQuery { function: 3 })));
    assert!(finished.get());
}

#[test]
fn mixed_pending_v26_foreign_physical_owner_never_runs_source_callback() {
    let finished = Cell::new(false);
    let module = mixed_module();
    with_mixed_case(&module, |pending, physical, globals, budget| {
        with_physical(&module, |_, foreign, _| {
            assert!(!std::ptr::eq(
                foreign.inventory().owner(),
                physical.inventory().owner()
            ));
            let error = pending
                .with_mixed_memory_observations_v26(
                    foreign,
                    globals,
                    budget,
                    |_, _| -> Result<(), Failure> { panic!("foreign physical owner admitted") },
                )
                .unwrap_err();
            assert!(matches!(error.failure(), Failure::ExactGraph));
            finished.set(true);
            Err(error.failure)
        })
    })
    .unwrap_err();
    assert!(finished.get());
}

#[test]
fn mixed_pending_v26_exact_and_one_short_analysis_limits_cover_all_definitions() {
    let finished = Cell::new(false);
    with_mixed_case(&mixed_module(), |pending, physical, globals, budget| {
        let observation = pending
            .with_mixed_memory_observations_v26(physical, globals, budget, |view, budget| {
                view.observation(budget)
            })
            .map_err(|error| error.failure)?;
        for limits in [
            Limits::new(
                observation.work_upper_bound() - 1,
                observation.peak_storage_units(),
            ),
            Limits::new(
                observation.work_upper_bound(),
                observation.peak_storage_units() - 1,
            ),
        ] {
            let called = Cell::new(false);
            let error = pending
                .with_mixed_memory_limits_v26(physical, globals, limits, budget, |_, _| {
                    called.set(true);
                    Ok(())
                })
                .unwrap_err();
            assert!(!called.get());
            assert!(error.observation().first_denial().is_some());
            let history = error.last_invocation().expect("attempt history retained");
            assert!(history.function() < 3);
            assert!(history.invocation().first_denial().is_some());
        }
        pending
            .with_mixed_memory_limits_v26(
                physical,
                globals,
                Limits::new(
                    observation.work_upper_bound(),
                    observation.peak_storage_units(),
                ),
                budget,
                |view, budget| {
                    for function in 0..3 {
                        assert_eq!(
                            view.report(function, budget)?.unwrap().paired_stage_count(),
                            9
                        );
                    }
                    finished.set(true);
                    Ok(())
                },
            )
            .map_err(|error| error.failure)
    })
    .unwrap();
    assert!(finished.get());
}

struct PanickingPayload<'a>(&'a Cell<bool>);
impl Drop for PanickingPayload<'_> {
    fn drop(&mut self) {
        self.0.set(true);
        panic!("mixed wrapper hostile destructor");
    }
}

#[test]
fn mixed_pending_v26_returned_destructor_cannot_replace_first_query_denial() {
    let dropped = Cell::new(false);
    let reached = Cell::new(false);
    let result = with_mixed_case(&mixed_module(), |pending, physical, globals, budget| {
        let error = pending
            .with_mixed_memory_observations_v26(physical, globals, budget, |view, budget| {
                assert!(matches!(
                    view.report(3, budget),
                    Err(Failure::InvalidQuery { function: 3 })
                ));
                reached.set(true);
                Ok(PanickingPayload(&dropped))
            })
            .err()
            .expect("sticky first denial");
        assert!(dropped.get());
        assert!(matches!(
            error.failure(),
            Failure::InvalidQuery { function: 3 }
        ));
        Err(error.failure)
    });
    assert!(matches!(result, Err(Failure::InvalidQuery { function: 3 })));
    assert!(reached.get());
    assert!(dropped.get());
}

#[test]
fn mixed_pending_v26_uncalled_capture_destructor_preserves_analysis_denial() {
    let dropped = Cell::new(false);
    let called = Cell::new(false);
    let finished = Cell::new(false);
    with_mixed_case(&mixed_module(), |pending, physical, globals, budget| {
        let capture = PanickingPayload(&dropped);
        let called = &called;
        let error = pending
            .with_mixed_memory_limits_v26(
                physical,
                globals,
                Limits::new(0, 0),
                budget,
                move |_, _| {
                    called.set(true);
                    drop(capture);
                    Ok(())
                },
            )
            .unwrap_err();
        assert!(!called.get());
        assert!(dropped.get());
        assert!(error.observation().first_denial().is_some());
        assert!(!matches!(error.failure(), Failure::Panicked));
        finished.set(true);
        Ok(())
    })
    .unwrap();
    assert!(finished.get());
}

#[test]
fn mixed_pending_v26_lost_higher_floor_never_refunds_ancestor_credits() {
    for disposition in 0..5 {
        let reached = Cell::new(false);
        with_physical(&mixed_module(), |checked, physical, budget| {
            let owner = physical.inventory().owner();
            let floor = budget.storage();
            let lost = Cell::new(None);
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    let parent = budget.storage();
                    let mut first = None;
                    let reads_result = with_canonical_guarded_global_reads_v18(
                        owner,
                        Default::default(),
                        budget,
                        |reads, budget| {
                            let stores_result = with_canonical_guarded_global_stores_v24(
                                owner,
                                Default::default(),
                                budget,
                                |stores, budget| {
                                    let batch_result = with_canonical_conditional_slice_domains_v26(
                                        reads,
                                        stores,
                                        &[ExplicitLaunchExtent::Exact {
                                            rank: 1,
                                            extents: [64, 1, 1],
                                        }; 3],
                                        FormalIndexWidth::Bits64,
                                        budget,
                                        |globals, budget| {
                                            let result = pending
                                                .with_mixed_memory_observations_v26(
                                                    physical,
                                                    globals,
                                                    budget,
                                                    |view, budget| {
                                                        if disposition == 3 {
                                                            assert!(matches!(
                                                                view.report(3, budget),
                                                                Err(Failure::InvalidQuery {
                                                                    function: 3
                                                                })
                                                            ));
                                                        }
                                                        budget.release_storage(1)?;
                                                        assert!(budget.storage() > parent);
                                                        lost.set(Some(budget.storage()));
                                                        if disposition == 4 {
                                                            assert!(matches!(
                                                                view.function_count(budget),
                                                                Err(Failure::Resource(
                                                                    Resource::Accounting
                                                                ))
                                                            ));
                                                        }
                                                        match disposition {
                                                            0 | 4 => Ok(()),
                                                            1 => Err(Failure::Callback(
                                                                "mixed higher floor sentinel",
                                                            )),
                                                            _ => panic!("mixed higher floor panic"),
                                                        }
                                                    },
                                                )
                                                .map_err(|error| error.failure);
                                            if disposition == 3 {
                                                assert!(
                                                    matches!(
                                                        result,
                                                        Err(Failure::InvalidQuery { function: 3 })
                                                    ),
                                                    "disposition={disposition}: {result:?}"
                                                );
                                            } else {
                                                assert!(
                                                    matches!(
                                                        result,
                                                        Err(Failure::Resource(
                                                            Resource::Accounting
                                                        ))
                                                    ),
                                                    "disposition={disposition}: {result:?}"
                                                );
                                            }
                                            assert_eq!(Some(budget.storage()), lost.get());
                                            first = Some(result.unwrap_err());
                                            reached.set(true);
                                            Ok(())
                                        },
                                    );
                                    assert!(matches!(batch_result, Err(fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Accounting))));
                                    assert_eq!(Some(budget.storage()), lost.get());
                                    Ok(())
                                },
                            );
                            assert!(matches!(
                                stores_result,
                                Err(
                                    fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(
                                        fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Accounting
                                    )
                                )
                            ));
                            assert_eq!(Some(budget.storage()), lost.get());
                            Ok(())
                        },
                    );
                    assert!(matches!(
                        reads_result,
                        Err(
                            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(
                                fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Accounting
                            )
                        )
                    ));
                    assert_eq!(Some(budget.storage()), lost.get());
                    Err::<(), _>(first.expect("mixed child refused"))
                },
            );
            if disposition == 3 {
                assert!(
                    matches!(result, Err(Failure::InvalidQuery { function: 3 })),
                    "{result:?}"
                );
            } else {
                assert!(
                    matches!(result, Err(Failure::Resource(Resource::Accounting))),
                    "{result:?}"
                );
            }
            assert_eq!(Some(budget.storage()), lost.get());
            // Only the fixture drains credit, after every native and formal
            // view that refused custody has unwound without refunding it.
            budget.release_storage(budget.storage() - floor).unwrap();
        });
        assert!(reached.get());
    }
}
