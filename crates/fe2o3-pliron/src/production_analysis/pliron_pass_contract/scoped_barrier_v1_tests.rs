fn ready_barrier<'a>(
    context: &'a Context,
    function: &'a FuncOp,
) -> PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'a>> {
    let mut session = begin_production_pliron_pass_contract_session_v1(
        LivePlironStructuralIdentityProviderV1::new(context, function),
    )
    .unwrap();
    for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..5] {
        session
            .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();
    }
    assert_eq!(session.next, 5);
    session
}

#[test]
fn barrier_scope_retains_three_cells_and_exact_endpoints() {
    assert_eq!(
        std::mem::size_of::<ScopedVerifiedProgressInputV1<'_, true>>(),
        3 * std::mem::size_of::<usize>()
    );
    assert_eq!(
        std::mem::size_of::<ScopedVerifiedProgressInputV1<'_, true>>(),
        std::mem::size_of::<ScopedVerifiedProgressInputV1<'_>>()
    );
    for cycle in [false, true] {
        let (context, function, _) = fixture(cycle);
        let (foreign_context, foreign_function, _) = fixture(cycle);
        let expected = run_pliron_progress_check_v1(&context, &function);
        let mut session = ready_barrier(&context, &function);
        let actual = session
            .run_scoped_barrier_with_resource_limits_v1(limits(), |input| {
                let (owner_context, owner_function) = input.endpoints()?;
                assert!(std::ptr::eq(owner_context, &context));
                assert!(!std::ptr::eq(owner_context, &foreign_context));
                assert_eq!(owner_function.get_operation(), function.get_operation());
                assert_ne!(
                    owner_function.get_operation(),
                    foreign_function.get_operation()
                );
                let progress = run_pliron_progress_with_scoped_input_v1(input)?;
                Ok(Ok::<_, ()>(progress.report))
            })
            .unwrap()
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(session.next, 6);
    }
}

#[test]
fn barrier_scope_admission_and_stage_order_precede_callback() {
    for (work, storage) in [(52, 44), (53, 43)] {
        let (context, function, _) = fixture(false);
        let mut session = ready_barrier(&context, &function);
        let called = Cell::new(false);
        let result = session.run_scoped_barrier_with_resource_limits_v1(
            ProductionAnalysisResourceLimitsV1::new(work, storage),
            |_| {
                called.set(true);
                Ok(Ok::<_, ()>(()))
            },
        );
        assert!(
            matches!(
                result,
                Err(PlironPassPreservationErrorV1::ResourceLimit { .. })
            ),
            "{result:?}"
        );
        assert!(!called.get());
        assert_eq!(session.next, 5);
        assert!(session.pending.is_none());
    }
    let (context, function, _) = fixture(false);
    let mut session = ready(&context, &function);
    let called = Cell::new(false);
    let result = session.run_scoped_barrier_with_resource_limits_v1(limits(), |_| {
        called.set(true);
        Ok(Ok::<_, ()>(()))
    });
    assert!(
        matches!(
            result,
            Err(PlironPassPreservationErrorV1::PassOrderMismatch {
                position: 8,
                expected: KernelCheckPassKindV1::SemanticRefinement,
                observed: KernelCheckPassKindV1::BarrierConvergence,
            })
        ),
        "{result:?}"
    );
    assert!(!called.get());
}

#[test]
fn barrier_scope_restored_mutation_before_begin_or_progress_is_rejected() {
    for before_begin in [false, true] {
        let (context, function, operation) = fixture(false);
        let mut session = ready_barrier(&context, &function);
        let called = Cell::new(false);
        if before_begin {
            restored_mutation(&context, operation);
        }
        let result = session.run_scoped_barrier_with_resource_limits_v1(limits(), |input| {
            called.set(true);
            let _endpoints = input.endpoints()?;
            restored_mutation(&context, operation);
            let progress = run_pliron_progress_with_scoped_input_v1(input)?;
            Ok(Ok::<_, ()>(progress.report))
        });
        if before_begin {
            assert!(
                matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::StaleMutationEpoch {
                        pass: KernelCheckPassKindV1::BarrierConvergence,
                        ..
                    })
                ),
                "{result:?}"
            );
            assert!(!called.get());
        } else {
            assert!(
                matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::MutationAttempted {
                        pass: Some(KernelCheckPassKindV1::BarrierConvergence),
                        ..
                    })
                ),
                "{result:?}"
            );
        }
        assert_eq!(session.next, 5);
    }
}

#[test]
fn barrier_scope_analysis_error_still_checks_post_pass_epoch() {
    for mutate in [false, true] {
        let (context, function, operation) = fixture(false);
        let mut session = ready_barrier(&context, &function);
        let result = session.run_scoped_barrier_with_resource_limits_v1(limits(), |input| {
            let _endpoints = input.into_endpoints()?;
            if mutate {
                restored_mutation(&context, operation);
            }
            Ok(Err::<(), _>("barrier analysis rejected"))
        });
        if mutate {
            assert!(
                matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::MutationAttempted {
                        pass: Some(KernelCheckPassKindV1::BarrierConvergence),
                        ..
                    })
                ),
                "{result:?}"
            );
            assert_eq!(session.next, 5);
        } else {
            assert_eq!(result, Ok(Err("barrier analysis rejected")));
            assert_eq!(session.next, 6);
            assert!(session.last_checkpoint_resource_upper_bound_v1().is_some());
        }
    }
    assert!(matches!(
        require_scoped_progress_epoch_for_pass_v1(
            KernelCheckPassKindV1::BarrierConvergence,
            u64::MAX,
            Err(IrMutationAttemptEpochExhausted),
        ),
        Err(PlironPassPreservationErrorV1::MutationEpochUnavailable {
            pass: Some(KernelCheckPassKindV1::BarrierConvergence),
            ..
        }),
    ));
}

#[test]
fn barrier_scope_preserves_identity_replacement_and_exact_scope_increment() {
    let (context, function, _) = fixture(false);
    let mut plain = ready_barrier(&context, &function);
    let mut scoped = ready_barrier(&context, &function);
    plain
        .run_contiguous_pass(
            KernelCheckPassKindV1::BarrierConvergence,
            || Ok::<_, ()>(()),
        )
        .unwrap()
        .unwrap();
    scoped
        .run_scoped_barrier_with_resource_limits_v1(limits(), |input| {
            let _endpoints = input.into_endpoints()?;
            Ok(Ok::<_, ()>(()))
        })
        .unwrap()
        .unwrap();
    let old = plain.last_checkpoint_resource_upper_bound_v1().unwrap();
    let new = scoped.last_checkpoint_resource_upper_bound_v1().unwrap();
    assert_eq!(new.work_upper_bound(), old.work_upper_bound() + 53);
    assert_eq!(
        new.retained_storage_upper_bound(),
        old.retained_storage_upper_bound()
    );
    assert_eq!(
        new.peak_storage_upper_bound(),
        old.peak_storage_upper_bound().max(44)
    );
    assert_eq!(
        scoped.lineage_identity_resource_upper_bound_v1(),
        plain.lineage_identity_resource_upper_bound_v1()
    );
}
