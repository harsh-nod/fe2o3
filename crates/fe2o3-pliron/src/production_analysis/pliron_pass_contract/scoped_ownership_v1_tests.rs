#[test]
fn ownership_scope_failed_mutable_borrow_is_rejected_immediately() {
    let (context, function, operation) = fixture(false);
    let mut session = ready_ownership(&context, &function);
    let rejected_at_consume = Cell::new(false);
    let result = session.run_scoped_ownership_with_resource_limits_v1(limits(), |input| {
        let read = operation.deref(&context);
        assert!(operation.try_deref_mut(&context).is_err());
        drop(read);
        let consumed = input.snapshot();
        rejected_at_consume.set(matches!(
            &consumed,
            Err(PlironPassPreservationErrorV1::MutationAttempted { .. })
        ));
        let _endpoints = consumed?;
        Ok(Ok::<_, ()>(()))
    });
    assert!(rejected_at_consume.get());
    assert!(matches!(
        result,
        Err(PlironPassPreservationErrorV1::MutationAttempted { .. })
    ));
    assert_eq!(session.next, 4);
}

#[test]
fn ownership_scope_permanent_invalid_mutation_keeps_structural_error_precedence() {
    let (context, function, operation) = fixture(false);
    let mut session = ready_ownership(&context, &function);
    let result = session.run_scoped_ownership_with_resource_limits_v1(limits(), |input| {
        let _endpoints = input.snapshot()?;
        Operation::push_successor(operation, &context, function.get_entry_block(&context));
        Ok(Err::<(), _>("analysis rejected"))
    });
    assert!(matches!(
        result,
        Err(PlironPassPreservationErrorV1::StructuralIdentityChanged {
            pass: KernelCheckPassKindV1::HierarchicalOwnership,
            ..
        })
    ));
    assert_eq!(session.next, 4);
}

#[test]
fn ownership_scope_panic_keeps_the_existing_terminal_session_behavior() {
    let (context, function, _) = fixture(false);
    let mut session = ready_ownership(&context, &function);
    let result =
        session.run_scoped_ownership_with_resource_limits_v1::<(), ()>(limits(), |input| {
            let _endpoints = input.snapshot()?;
            panic!("scoped callback panic");
        });
    assert_eq!(
        result,
        Err(PlironPassPreservationErrorV1::AnalysisPanicked {
            pass: KernelCheckPassKindV1::HierarchicalOwnership
        })
    );
    assert!(session.pending.is_some());
    assert_eq!(session.next, 4);
}

#[test]
fn ownership_scope_exact_bound_and_pending_session_are_checked() {
    let bound = scoped_ownership_input_resource_upper_bound_v1().unwrap();
    assert_eq!(bound.work_upper_bound(), 57);
    assert_eq!(bound.retained_storage_upper_bound(), 0);
    assert_eq!(bound.peak_storage_upper_bound(), 45);
    assert!(ProductionAnalysisResourceLimitsV1::new(57, 45).admits(bound));
    let phase = ProductionAnalysisResourcePhaseV1::PassPreservation;
    let overflowing =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, usize::MAX - 56, 0, 0)
            .unwrap();
    assert!(bound.checked_then_retain(overflowing, phase).is_err());
    let (context, function, _) = fixture(false);
    let mut session = ready_ownership(&context, &function);
    session
        .begin_pass(KernelCheckPassKindV1::HierarchicalOwnership, false)
        .unwrap();
    let called = Cell::new(false);
    let result = session.run_scoped_ownership_with_resource_limits_v1(limits(), |_| {
        called.set(true);
        Ok(Ok::<_, ()>(()))
    });
    assert!(matches!(
        result,
        Err(PlironPassPreservationErrorV1::InvalidSessionState { .. })
    ));
    assert!(!called.get());
    assert_eq!(session.next, 4);
}

#[test]
fn ownership_scope_empty_report_has_no_conditional_record() {
    let (context, function, _) = fixture(false);
    let mut session = ready_ownership(&context, &function);
    let mut analyses = PlironAnalysisManagerV1::new(&function);
    let report = session
        .run_scoped_ownership_with_resource_limits_v1(limits(), |input| {
            crate::production_analysis::require_pliron_hierarchical_ownership_with_scoped_input_v1(
                input,
                &mut analyses,
            )
        })
        .unwrap()
        .unwrap();
    assert!(report.is_clean());
    assert!(report.conditional_prefix_coverage().is_none());
    assert!(report.conditional_prefix_failure().is_none());
}

fn ready_ownership<'a>(
    context: &'a Context,
    function: &'a FuncOp,
) -> PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'a>> {
    let mut session = begin_production_pliron_pass_contract_session_v1(
        LivePlironStructuralIdentityProviderV1::new(context, function),
    )
    .unwrap();
    for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..4] {
        session
            .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();
    }
    assert_eq!(session.next, 4);
    session
}

#[test]
fn ownership_scope_retains_four_cells_and_exact_snapshot() {
    assert_eq!(
        std::mem::size_of::<ScopedVerifiedOwnershipInputV1<'_>>(),
        4 * std::mem::size_of::<usize>()
    );
    assert_eq!(
        std::mem::size_of::<ScopedVerifiedOwnershipInputV1<'_>>(),
        std::mem::size_of::<ScopedVerifiedProgressInputV1<'_>>() + std::mem::size_of::<usize>()
    );
    for cycle in [false, true] {
        let (context, function, _) = fixture(cycle);
        let (foreign_context, foreign_function, _) = fixture(cycle);
        let expected = crate::production_analysis::pliron_ir_identity::derive_pliron_ir_structural_identity_v1(&context, &function).unwrap();
        let mut session = ready_ownership(&context, &function);
        let actual = session
            .run_scoped_ownership_with_resource_limits_v1(limits(), |input| {
                let (owner_context, owner_function) = input.endpoints()?;
                assert!(std::ptr::eq(owner_context, &context));
                assert!(!std::ptr::eq(owner_context, &foreign_context));
                assert_eq!(owner_function.get_operation(), function.get_operation());
                assert_ne!(
                    owner_function.get_operation(),
                    foreign_function.get_operation()
                );
                let (snapshot, epoch) = input.snapshot()?;
                assert!(std::ptr::eq(snapshot, input.snapshot()?.0));
                assert_eq!(epoch, context.ir_mutation_attempt_epoch().unwrap().value());
                Ok(Ok::<_, ()>(snapshot.clone()))
            })
            .unwrap()
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(session.next, 5);
    }
}

#[test]
fn ownership_scope_admission_and_stage_order_precede_callback() {
    for (work, storage) in [(56, 45), (57, 44)] {
        let (context, function, _) = fixture(false);
        let mut session = ready_ownership(&context, &function);
        let called = Cell::new(false);
        let result = session.run_scoped_ownership_with_resource_limits_v1(
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
        assert_eq!(session.next, 4);
        assert!(session.pending.is_none());
    }
    let (context, function, _) = fixture(false);
    let mut session = ready(&context, &function);
    let called = Cell::new(false);
    let result = session.run_scoped_ownership_with_resource_limits_v1(limits(), |_| {
        called.set(true);
        Ok(Ok::<_, ()>(()))
    });
    assert!(
        matches!(
            result,
            Err(PlironPassPreservationErrorV1::PassOrderMismatch {
                position: 8,
                expected: KernelCheckPassKindV1::SemanticRefinement,
                observed: KernelCheckPassKindV1::HierarchicalOwnership,
            })
        ),
        "{result:?}"
    );
    assert!(!called.get());
}

#[test]
fn ownership_scope_restored_mutation_before_begin_or_progress_is_rejected() {
    for before_begin in [false, true] {
        let (context, function, operation) = fixture(false);
        let mut session = ready_ownership(&context, &function);
        let called = Cell::new(false);
        if before_begin {
            restored_mutation(&context, operation);
        }
        let result = session.run_scoped_ownership_with_resource_limits_v1(limits(), |input| {
            called.set(true);
            let _endpoints = input.endpoints()?;
            restored_mutation(&context, operation);
            let _snapshot = input.snapshot()?;
            Ok(Ok::<_, ()>(()))
        });
        if before_begin {
            assert!(
                matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::StaleMutationEpoch {
                        pass: KernelCheckPassKindV1::HierarchicalOwnership,
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
                        pass: Some(KernelCheckPassKindV1::HierarchicalOwnership),
                        ..
                    })
                ),
                "{result:?}"
            );
        }
        assert_eq!(session.next, 4);
    }
}

#[test]
fn ownership_scope_analysis_error_still_checks_post_pass_epoch() {
    for mutate in [false, true] {
        let (context, function, operation) = fixture(false);
        let mut session = ready_ownership(&context, &function);
        let result = session.run_scoped_ownership_with_resource_limits_v1(limits(), |input| {
            let _endpoints = input.endpoints()?;
            if mutate {
                restored_mutation(&context, operation);
            }
            Ok(Err::<(), _>("ownership analysis rejected"))
        });
        if mutate {
            assert!(
                matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::MutationAttempted {
                        pass: Some(KernelCheckPassKindV1::HierarchicalOwnership),
                        ..
                    })
                ),
                "{result:?}"
            );
            assert_eq!(session.next, 4);
        } else {
            assert_eq!(result, Ok(Err("ownership analysis rejected")));
            assert_eq!(session.next, 5);
            assert!(session.last_checkpoint_resource_upper_bound_v1().is_some());
        }
    }
    assert!(matches!(
        require_scoped_progress_epoch_for_pass_v1(
            KernelCheckPassKindV1::HierarchicalOwnership,
            u64::MAX,
            Err(IrMutationAttemptEpochExhausted),
        ),
        Err(PlironPassPreservationErrorV1::MutationEpochUnavailable {
            pass: Some(KernelCheckPassKindV1::HierarchicalOwnership),
            ..
        }),
    ));
}

#[test]
fn ownership_scope_preserves_identity_replacement_and_exact_scope_increment() {
    let (context, function, _) = fixture(false);
    let mut plain = ready_ownership(&context, &function);
    let mut scoped = ready_ownership(&context, &function);
    plain
        .run_contiguous_pass(KernelCheckPassKindV1::HierarchicalOwnership, || {
            Ok::<_, ()>(())
        })
        .unwrap()
        .unwrap();
    scoped
        .run_scoped_ownership_with_resource_limits_v1(limits(), |input| {
            let _endpoints = input.endpoints()?;
            Ok(Ok::<_, ()>(()))
        })
        .unwrap()
        .unwrap();
    let old = plain.last_checkpoint_resource_upper_bound_v1().unwrap();
    let new = scoped.last_checkpoint_resource_upper_bound_v1().unwrap();
    assert_eq!(new.work_upper_bound(), old.work_upper_bound() + 57);
    assert_eq!(
        new.retained_storage_upper_bound(),
        old.retained_storage_upper_bound()
    );
    assert_eq!(
        new.peak_storage_upper_bound(),
        old.peak_storage_upper_bound().max(45)
    );
    assert_eq!(
        scoped.lineage_identity_resource_upper_bound_v1(),
        plain.lineage_identity_resource_upper_bound_v1()
    );
}
