use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1,
    CanonicalKirOperationCoordinateV1, Constant,
    with_canonical_predicated_conditional_slice_domains_v85,
    with_canonical_predicated_global_stores_v84,
};

fn predicated_module() -> Module {
    let mut module = mixed_module();
    let body = module.functions[1].body.as_mut().unwrap();
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    body.blocks[0].operations.extend([
        Operation::new(
            vec![ValueDef::new(ValueId(200), Type::INDEX)],
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(201), Type::INDEX)],
            OperationKind::Select {
                condition: ValueId(103),
                true_value: ValueId(101),
                false_value: ValueId(200),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(202), pointer.clone())],
            OperationKind::SliceData {
                slice: ValueId(100),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(203), pointer)],
            OperationKind::GetElementPointer {
                base: ValueId(202),
                offset: ValueId(201),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(203),
                predicate: ValueId(103),
                value: ValueId(20),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    body.blocks[1].operations.remove(2);
    module
}

fn store_coordinate() -> CanonicalKirOperationCoordinateV1 {
    CanonicalKirOperationCoordinateV1 {
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(1),
            block: 0,
        },
        operation: 10,
    }
}

fn with_predicated_case(
    run: impl FnOnce(
        &mut PendingCanonicalRankedSourceRolesV18<'_, '_>,
        &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        &Globals<'_, '_>,
        &mut Budget<'_>,
    ) -> Result<(), Failure>,
) -> Result<(), Failure> {
    with_physical(&predicated_module(), |checked, physical, budget| {
        let floor = budget.storage();
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                let owner = physical.inventory().owner();
                let mut run = Some(run);
                let mut result = None;
                let admitted = with_canonical_guarded_global_reads_v18(
                    owner,
                    Default::default(),
                    budget,
                    |reads, budget| {
                        with_canonical_predicated_global_stores_v84(
                            owner,
                            Default::default(),
                            budget,
                            |stores, budget| {
                                with_canonical_predicated_conditional_slice_domains_v85(
                                    reads,
                                    stores,
                                    &[ExplicitLaunchExtent::Exact {
                                        rank: 1,
                                        extents: [64, 1, 1],
                                    }; 3],
                                    FormalIndexWidth::Bits64,
                                    budget,
                                    |globals, budget| {
                                        result = Some(run.take().unwrap()(
                                            pending, physical, globals, budget,
                                        ));
                                        Ok(())
                                    },
                                )
                            },
                        )
                    },
                )
                .map_err(Failure::ConditionalGlobalsV26)?;
                assert!(
                    admitted.is_some(),
                    "complete actual predicated formal family"
                );
                result.expect("predicated domain callback")
            },
        );
        assert_eq!(budget.storage(), floor);
        result
    })
}

#[test]
fn predicated_pending_v89_runs_nine_stages_with_exact_native_store_and_unresolved_premises() {
    let reached = Cell::new(false);
    with_predicated_case(|pending, physical, globals, budget| {
        pending
            .with_predicated_memory_observations_v89(physical, globals, budget, |view, budget| {
                let view: &crate::PendingCanonicalPredicatedMemoryPoliciesV89<'_, '_> = view;
                let owner = physical.inventory().owner();
                assert!(std::ptr::eq(view.owner(budget)?, owner));
                assert!(std::ptr::eq(view.conditional_globals(budget)?, globals));
                for function in 0..3 {
                    let report = view.report(function, budget)?.unwrap();
                    assert_eq!(report.paired_stage_count(), 9);
                    assert!(report.reports().is_clean());
                    for stage in 0..9 {
                        assert_eq!(
                            report.global_access_counts(stage),
                            Some(if function == 1 { [1, 1] } else { [0, 0] })
                        );
                        assert_eq!(report.private_access_counts(stage), Some([1, 0, 1, 1, 0]));
                    }
                }
                let actual = view
                    .global_accesses(budget)?
                    .operation(owner, store_coordinate(), budget)?
                    .unwrap();
                let expected =
                    &owner.module().functions[1].body.as_ref().unwrap().blocks[0].operations[10];
                assert!(std::ptr::eq(actual, expected));
                assert!(matches!(
                    actual.kind,
                    OperationKind::GuardedStore {
                        predicate: ValueId(103),
                        ..
                    }
                ));
                assert!(!view.source_roles_are_complete());
                assert!(!view.runtime_requirements_are_discharged());
                assert!(!view.ranked_verification_is_complete());
                assert!(!view.grants_artifact_or_launch_authority());
                reached.set(true);
                Ok(())
            })
            .map_err(|error| error.failure)
    })
    .unwrap();
    assert!(reached.get());
}

#[test]
fn predicated_pending_v89_does_not_widen_legacy_mixed_admission() {
    let reached = Cell::new(false);
    let result = with_predicated_case(|pending, physical, globals, budget| {
        pending
            .with_mixed_memory_observations_v26(physical, globals, budget, |_, _| {
                reached.set(true);
                Ok(())
            })
            .map_err(|error| error.failure)
    });
    assert!(result.is_err());
    assert!(!reached.get());
}

#[test]
fn predicated_pending_v89_refuses_identical_foreign_physical_owner_before_callback() {
    let reached = Cell::new(false);
    let result = with_predicated_case(|pending, _, globals, budget| {
        with_physical(&predicated_module(), |_, foreign, _| {
            let error = pending
                .with_predicated_memory_observations_v89(foreign, globals, budget, |_, _| {
                    reached.set(true);
                    Ok(())
                })
                .unwrap_err();
            assert!(matches!(error.failure(), Failure::ExactGraph));
            Err(error.failure)
        })
    });
    assert!(matches!(result, Err(Failure::ExactGraph)));
    assert!(!reached.get());
}

#[test]
fn predicated_pending_v89_exact_and_one_short_native_stage_limits() {
    let reached = Cell::new(false);
    with_predicated_case(|pending, physical, globals, budget| {
        let observation = pending
            .with_predicated_memory_observations_v89(physical, globals, budget, |view, budget| {
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
            let error = pending
                .with_predicated_memory_limits_v89(
                    physical,
                    globals,
                    limits,
                    budget,
                    |_, _| -> Result<(), Failure> {
                        panic!("short native allowance entered source callback")
                    },
                )
                .unwrap_err();
            assert!(error.observation().first_denial().is_some());
            assert!(
                error
                    .last_invocation()
                    .unwrap()
                    .invocation()
                    .first_denial()
                    .is_some()
            );
        }
        pending
            .with_predicated_memory_limits_v89(
                physical,
                globals,
                Limits::new(
                    observation.work_upper_bound(),
                    observation.peak_storage_units(),
                ),
                budget,
                |view, budget| {
                    assert_eq!(view.report(1, budget)?.unwrap().paired_stage_count(), 9);
                    reached.set(true);
                    Ok(())
                },
            )
            .map_err(|error| error.failure)
    })
    .unwrap();
    assert!(reached.get());
}

#[test]
fn predicated_pending_v89_callback_error_and_unwind_restore_floor_after_capture_drop() {
    struct Capture<'a>(&'a Cell<usize>);
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for panic in [false, true] {
        let drops = Cell::new(0);
        let result = with_predicated_case(|pending, physical, globals, budget| {
            let floor = budget.storage();
            let capture = Capture(&drops);
            let result = pending
                .with_predicated_memory_observations_v89(
                    physical,
                    globals,
                    budget,
                    move |view, budget| {
                        let _capture = capture;
                        assert!(view.report(1, budget)?.is_some());
                        if panic {
                            panic!("predicated native callback sentinel");
                        }
                        Err::<(), _>(Failure::Callback("predicated native callback sentinel"))
                    },
                )
                .map_err(|error| error.failure);
            assert_eq!(drops.get(), 1);
            assert_eq!(budget.storage(), floor);
            result
        });
        if panic {
            assert!(matches!(result, Err(Failure::Panicked)));
        } else {
            assert!(matches!(
                result,
                Err(Failure::Callback("predicated native callback sentinel"))
            ));
        }
    }
}
