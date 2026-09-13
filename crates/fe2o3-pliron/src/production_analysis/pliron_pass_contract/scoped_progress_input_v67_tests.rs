#[cfg(test)]
mod scoped_progress_input_v67_tests {
    use super::*;
    use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
    use crate::production_analysis::pliron_progress::{
        run_pliron_progress_check_v1, run_pliron_progress_with_scoped_input_v1,
    };
    use crate::production_analysis::pliron_semantic_refinement::{
        require_pliron_semantic_refinement_with_analyses_v1,
        require_pliron_semantic_refinement_with_scoped_input_v1,
    };
    use dialect_kernel::{BranchOp, DIALECT_NAME, ReturnOp, register_dialect};
    use pliron::{
        builtin::{attributes::UnitAttr, types::FunctionType},
        context::Ptr,
        dialect::DialectName,
        op::Op,
        operation::{Operation, verify_operation},
    };
    use std::cell::Cell;

    fn fixture(cycle: bool) -> (Context, FuncOp, Ptr<Operation>) {
        let mut context = Context::new();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "scoped_progress".try_into().unwrap(),
            function_type,
        );
        let entry = function.get_entry_block(&context);
        let terminator = if cycle {
            BranchOp::new(&mut context, entry).get_operation()
        } else {
            ReturnOp::new(&mut context).get_operation()
        };
        terminator.insert_at_back(entry, &context);
        verify_operation(function.get_operation(), &context).unwrap();
        (context, function, terminator)
    }

    fn ready<'a>(
        context: &'a Context,
        function: &'a FuncOp,
    ) -> PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'a>> {
        let mut session = begin_production_pliron_pass_contract_session_v1(
            LivePlironStructuralIdentityProviderV1::new(context, function),
        )
        .unwrap();
        for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..8] {
            session
                .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                .unwrap()
                .unwrap();
        }
        assert_eq!(session.next, 8);
        session
    }

    fn limits() -> ProductionAnalysisResourceLimitsV1 {
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling()
    }

    #[test]
    fn scoped_progress_input_has_literal_exact_and_one_under_admission() {
        let bound = scoped_progress_input_resource_upper_bound_v1().unwrap();
        // 3 guard + 1 pending + 3 fields + 1 callback + 2 epoch + 2 endpoints.
        assert_eq!(SCOPED_PROGRESS_SUCCESS_WORK_V1, 12);
        assert_eq!(SCOPED_PROGRESS_EPOCH_DETAIL_V1.len(), 35);
        assert_eq!(
            SCOPED_PROGRESS_EPOCH_DETAIL_V1,
            IrMutationAttemptEpochExhausted.to_string()
        );
        assert_eq!(bound.work_upper_bound(), 53);
        assert_eq!(bound.retained_storage_upper_bound(), 0);
        assert_eq!(bound.peak_storage_upper_bound(), 44);
        let remaining = ProductionAnalysisResourceLimitsV1::new(53, 44)
            .remaining_after_retained(ProductionAnalysisResourcePhaseV1::PassPreservation, bound)
            .unwrap();
        assert_eq!(remaining.max_work(), 0);
        assert_eq!(remaining.max_peak_storage(), 44);
        for (work, storage, resource) in [
            (52, 44, "work upper bound"),
            (53, 43, "peak storage upper bound"),
        ] {
            let error = ProductionAnalysisResourceLimitsV1::new(work, storage)
                .remaining_after_retained(
                    ProductionAnalysisResourcePhaseV1::PassPreservation,
                    bound,
                )
                .unwrap_err();
            assert_eq!(
                error.phase,
                ProductionAnalysisResourcePhaseV1::PassPreservation
            );
            assert_eq!(error.resource, resource);
            let (context, function, _) = fixture(false);
            let mut session = ready(&context, &function);
            let called = Cell::new(false);
            let result = session.run_scoped_semantic_refinement_with_resource_limits_v1(
                ProductionAnalysisResourceLimitsV1::new(work, storage),
                |_input| {
                    called.set(true);
                    Ok(Ok::<_, ()>(()))
                },
            );
            assert_eq!(
                result,
                Err(PlironPassPreservationErrorV1::ResourceLimit { resource })
            );
            assert!(!called.get());
            assert!(session.pending.is_none());
            assert!(session.lineage.is_some());
            assert_eq!(session.next, 8);
        }
    }

    #[test]
    fn scoped_progress_wrong_stage_and_pending_pass_cannot_mint_an_input() {
        let (context, function, _) = fixture(false);
        let mut session = begin_production_pliron_pass_contract_session_v1(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
        )
        .unwrap();
        let called = Cell::new(false);
        let result =
            session.run_scoped_semantic_refinement_with_resource_limits_v1(limits(), |_| {
                called.set(true);
                Ok(Ok::<_, ()>(()))
            });
        assert!(matches!(
            result,
            Err(PlironPassPreservationErrorV1::PassOrderMismatch {
                position: 0,
                expected: KernelCheckPassKindV1::TensorLayout,
                observed: KernelCheckPassKindV1::SemanticRefinement,
            })
        ));
        assert!(!called.get());
        let mut session = ready(&context, &function);
        session
            .begin_pass(KernelCheckPassKindV1::SemanticRefinement, false)
            .unwrap();
        let result =
            session.run_scoped_semantic_refinement_with_resource_limits_v1(limits(), |_| {
                called.set(true);
                Ok(Ok::<_, ()>(()))
            });
        assert!(matches!(
            result,
            Err(PlironPassPreservationErrorV1::InvalidSessionState { .. })
        ));
        assert!(!called.get());
    }

    #[test]
    fn scoped_progress_uses_the_exact_provider_endpoints_and_preserves_reports() {
        for cycle in [false, true] {
            let (context, function, _) = fixture(cycle);
            let (foreign_context, foreign_function, _) = fixture(cycle);
            let expected = run_pliron_progress_check_v1(&context, &function);
            let mut session = ready(&context, &function);
            let actual = session
                .run_scoped_semantic_refinement_with_resource_limits_v1(limits(), |input| {
                    let scoped = run_pliron_progress_with_scoped_input_v1(input)?;
                    assert!(std::ptr::eq(scoped.context, &context));
                    assert!(!std::ptr::eq(scoped.context, &foreign_context));
                    assert_eq!(scoped.function.get_operation(), function.get_operation());
                    assert_ne!(
                        scoped.function.get_operation(),
                        foreign_function.get_operation()
                    );
                    Ok(Ok::<_, ()>(scoped.report))
                })
                .unwrap()
                .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.is_clean(), !cycle);
            assert_eq!(session.next, 9);
        }
    }

    #[test]
    fn scoped_progress_semantics_share_the_private_after_progress_body() {
        for cycle in [false, true] {
            let (context, function, _) = fixture(cycle);
            let mut standalone_analyses = PlironAnalysisManagerV1::new(&function);
            let expected = require_pliron_semantic_refinement_with_analyses_v1(
                &context,
                &function,
                &mut standalone_analyses,
            );
            let mut analyses = PlironAnalysisManagerV1::new(&function);
            let mut session = ready(&context, &function);
            let actual = session
                .run_scoped_semantic_refinement_with_resource_limits_v1(limits(), |input| {
                    require_pliron_semantic_refinement_with_scoped_input_v1(input, &mut analyses)
                })
                .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.is_ok(), !cycle);
            assert_eq!(session.next, 9);
        }
    }

    #[test]
    fn scoped_progress_preserves_prior_verification_cost_and_adds_only_scope_work() {
        let (context, function, _) = fixture(false);
        let mut standalone = ready(&context, &function);
        let mut scoped = ready(&context, &function);
        standalone
            .run_contiguous_pass(
                KernelCheckPassKindV1::SemanticRefinement,
                || Ok::<_, ()>(()),
            )
            .unwrap()
            .unwrap();
        scoped
            .run_scoped_semantic_refinement_with_resource_limits_v1(limits(), |input| {
                let _endpoints = input.into_endpoints()?;
                Ok(Ok::<_, ()>(()))
            })
            .unwrap()
            .unwrap();
        let before = standalone
            .last_checkpoint_resource_upper_bound_v1()
            .unwrap();
        let after = scoped.last_checkpoint_resource_upper_bound_v1().unwrap();
        assert_eq!(after.work_upper_bound(), before.work_upper_bound() + 53);
        assert_eq!(
            after.retained_storage_upper_bound(),
            before.retained_storage_upper_bound()
        );
        assert_eq!(
            after.peak_storage_upper_bound(),
            before.peak_storage_upper_bound().max(44)
        );
        assert_eq!(
            scoped.lineage_identity_resource_upper_bound_v1(),
            standalone.lineage_identity_resource_upper_bound_v1()
        );
    }

    fn restored_mutation(context: &Context, operation: Ptr<Operation>) {
        let original = operation.deref(context).attributes.clone();
        operation
            .deref_mut(context)
            .attributes
            .set("scoped_transient".try_into().unwrap(), UnitAttr::new());
        operation.deref_mut(context).attributes = original;
    }

    #[test]
    fn scoped_progress_rejects_restore_before_begin_and_before_consumption() {
        for before_begin in [false, true] {
            let (context, function, operation) = fixture(false);
            let mut session = ready(&context, &function);
            let called = Cell::new(false);
            let rejected_at_consume = Cell::new(false);
            if before_begin {
                restored_mutation(&context, operation);
            }
            let result =
                session.run_scoped_semantic_refinement_with_resource_limits_v1(limits(), |input| {
                    called.set(true);
                    restored_mutation(&context, operation);
                    let result = input.into_endpoints();
                    rejected_at_consume.set(matches!(
                        &result,
                        Err(PlironPassPreservationErrorV1::MutationAttempted {
                            pass: Some(KernelCheckPassKindV1::SemanticRefinement),
                            ..
                        })
                    ));
                    let _endpoints = result?;
                    Ok(Ok::<_, ()>(()))
                });
            if before_begin {
                assert!(matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::StaleMutationEpoch {
                        pass: KernelCheckPassKindV1::SemanticRefinement,
                        ..
                    })
                ));
                assert!(!called.get());
            } else {
                assert!(matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::MutationAttempted {
                        pass: Some(KernelCheckPassKindV1::SemanticRefinement),
                        ..
                    })
                ));
                assert!(rejected_at_consume.get());
            }
            assert_eq!(session.next, 8);
        }
    }

    #[test]
    fn scoped_progress_failed_mutable_borrow_is_rejected_immediately() {
        let (context, function, operation) = fixture(false);
        let mut session = ready(&context, &function);
        let rejected_at_consume = Cell::new(false);
        let result =
            session.run_scoped_semantic_refinement_with_resource_limits_v1(limits(), |input| {
                let read = operation.deref(&context);
                assert!(operation.try_deref_mut(&context).is_err());
                drop(read);
                let consumed = input.into_endpoints();
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
        assert_eq!(session.next, 8);
    }

    #[test]
    fn scoped_progress_analysis_error_still_runs_post_pass_identity_and_epoch_checks() {
        for mutate in [false, true] {
            let (context, function, operation) = fixture(false);
            let mut session = ready(&context, &function);
            let result =
                session.run_scoped_semantic_refinement_with_resource_limits_v1(limits(), |input| {
                    let _endpoints = input.into_endpoints()?;
                    if mutate {
                        restored_mutation(&context, operation);
                    }
                    Ok(Err::<(), _>("analysis rejected"))
                });
            if mutate {
                assert!(matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::MutationAttempted { .. })
                ));
                assert_eq!(session.next, 8);
            } else {
                assert_eq!(result, Ok(Err("analysis rejected")));
                assert_eq!(session.next, 9);
                assert!(session.last_checkpoint_resource_upper_bound_v1().is_some());
            }
        }
    }

    #[test]
    fn scoped_progress_permanent_invalid_mutation_keeps_structural_error_precedence() {
        let (context, function, operation) = fixture(false);
        let mut session = ready(&context, &function);
        let result =
            session.run_scoped_semantic_refinement_with_resource_limits_v1(limits(), |input| {
                let _endpoints = input.into_endpoints()?;
                Operation::push_successor(operation, &context, function.get_entry_block(&context));
                Ok(Err::<(), _>("analysis rejected"))
            });
        assert!(matches!(
            result,
            Err(PlironPassPreservationErrorV1::StructuralIdentityChanged {
                pass: KernelCheckPassKindV1::SemanticRefinement,
                ..
            })
        ));
        assert_eq!(session.next, 8);
    }

    #[test]
    fn scoped_progress_panic_keeps_the_existing_terminal_session_behavior() {
        let (context, function, _) = fixture(false);
        let mut session = ready(&context, &function);
        let result = session.run_scoped_semantic_refinement_with_resource_limits_v1::<(), ()>(
            limits(),
            |input| {
                let _endpoints = input.into_endpoints()?;
                panic!("scoped callback panic");
            },
        );
        assert_eq!(
            result,
            Err(PlironPassPreservationErrorV1::AnalysisPanicked {
                pass: KernelCheckPassKindV1::SemanticRefinement
            })
        );
        assert!(session.pending.is_some());
        assert_eq!(session.next, 8);
    }

    #[test]
    fn scoped_progress_epoch_unavailable_is_never_an_analysis_result() {
        // Context exposes no safe epoch-exhaustion setter. Exercise the exact
        // adapter, not a fake provider or a forged scoped input.
        let error =
            require_scoped_progress_epoch_v1(u64::MAX, Err(IrMutationAttemptEpochExhausted))
                .unwrap_err();
        assert_eq!(
            error,
            PlironPassPreservationErrorV1::MutationEpochUnavailable {
                pass: Some(KernelCheckPassKindV1::SemanticRefinement),
                detail: "IR mutation-attempt epoch exhausted".to_owned(),
            }
        );
        assert_eq!(
            require_scoped_progress_epoch_v1(u64::MAX, Ok(u64::MAX)),
            Ok(())
        );
        assert!(matches!(
            require_scoped_progress_epoch_v1(u64::MAX, Ok(0)),
            Err(PlironPassPreservationErrorV1::MutationAttempted {
                before: u64::MAX,
                after: 0,
                ..
            })
        ));
    }

    #[test]
    fn scoped_progress_invalid_input_never_enters_the_live_session() {
        let (context, function, operation) = fixture(false);
        Operation::push_successor(operation, &context, function.get_entry_block(&context));
        assert!(verify_operation(function.get_operation(), &context).is_err());
        let result = begin_production_pliron_pass_contract_session_v1(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
        );
        assert!(matches!(
            result,
            Err(PlironPassPreservationErrorV1::IdentityUnavailable { .. })
        ));
    }

    #[test]
    fn scoped_progress_checkpoint_composition_has_literal_bounds_and_checked_overflow() {
        let scope = scoped_progress_input_resource_upper_bound_v1().unwrap();
        let phase = ProductionAnalysisResourcePhaseV1::PassPreservation;
        let checkpoint =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 17, 5, 7).unwrap();
        let combined = scope.checked_then_retain(checkpoint, phase).unwrap();
        assert_eq!(combined.work_upper_bound(), 70);
        assert_eq!(combined.retained_storage_upper_bound(), 5);
        assert_eq!(combined.peak_storage_upper_bound(), 44);
        assert!(
            ProductionAnalysisResourceLimitsV1::new(70, 44)
                .require(phase, combined)
                .is_ok()
        );
        assert!(
            ProductionAnalysisResourceLimitsV1::new(69, 44)
                .require(phase, combined)
                .is_err()
        );
        assert!(
            ProductionAnalysisResourceLimitsV1::new(70, 43)
                .require(phase, combined)
                .is_err()
        );
        let overflow =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, usize::MAX - 52, 0, 0)
                .unwrap();
        assert!(scope.checked_then_retain(overflow, phase).is_err());
    }
    include!("scoped_barrier_v1_tests.rs");
    include!("scoped_ownership_v1_tests.rs");
}
