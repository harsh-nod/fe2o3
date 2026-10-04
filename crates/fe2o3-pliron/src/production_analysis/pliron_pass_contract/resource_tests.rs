#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, collections::VecDeque, rc::Rc};

    include!("replacement_limits_v1_tests.rs");
    include!("comparison_admission_v1_tests.rs");

    struct ScriptedIdentityProviderV1 {
        snapshots: VecDeque<Result<Vec<u8>, IdentityCaptureFailureV1>>,
        mutation_epoch: Rc<Cell<u64>>,
        epoch_unavailable: Rc<Cell<bool>>,
    }

    impl PlironStructuralIdentityProviderV1 for ScriptedIdentityProviderV1 {
        type Snapshot = Vec<u8>;

        fn mutation_epoch(&self) -> Result<u64, MutationEpochCaptureFailureV1> {
            if self.epoch_unavailable.get() {
                Err(MutationEpochCaptureFailureV1::new(
                    "scripted epoch exhausted",
                ))
            } else {
                Ok(self.mutation_epoch.get())
            }
        }

        fn capture_with_resource_limits_v1(
            &mut self,
            limits: ProductionAnalysisResourceLimitsV1,
        ) -> Result<BoundedPlironIdentityCaptureV1<Self::Snapshot>, IdentityCaptureFailureV1>
        {
            let snapshot = self.snapshots.pop_front().unwrap_or(Err(
                IdentityCaptureFailureV1::Unavailable {
                    source_code: "FE2O3-PRESERVE-005",
                    detail: "script exhausted".to_owned(),
                },
            ))?;
            let upper_bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
                ProductionAnalysisResourcePhaseV1::StructuralIdentity,
                snapshot.len(),
                snapshot.len(),
                0,
            )
            .map_err(IdentityCaptureFailureV1::ResourceLimit)?;
            limits
                .require(
                    ProductionAnalysisResourcePhaseV1::StructuralIdentity,
                    upper_bound,
                )
                .map_err(IdentityCaptureFailureV1::ResourceLimit)?;
            Ok(BoundedPlironIdentityCaptureV1 {
                input_census: ProductionAnalysisInputCensusV1 {
                    canonical_bytes: snapshot.len(),
                    ..ProductionAnalysisInputCensusV1::default()
                },
                snapshot,
                resource_upper_bound: upper_bound,
            })
        }

        fn label(&self, snapshot: &Self::Snapshot) -> PlironStructuralIdentityLabelV1 {
            let mut label = [0_u8; 32];
            for (index, byte) in snapshot.iter().copied().enumerate() {
                label[index % label.len()] ^= byte;
            }
            PlironStructuralIdentityLabelV1::new(label, snapshot.len())
        }

        fn require_exact_identity(
            &self,
            expected: &Self::Snapshot,
            observed: &Self::Snapshot,
        ) -> Result<(), IdentityComparisonFailureV1> {
            if expected == observed {
                return Ok(());
            }
            let difference = expected
                .iter()
                .zip(observed)
                .position(|(expected, observed)| expected != observed)
                .unwrap_or(expected.len().min(observed.len()));
            Err(IdentityComparisonFailureV1::new(
                "FE2O3-PRESERVE-010",
                format!("first changed component at canonical byte {difference}"),
            ))
        }

        fn retain_exact_identity(&self, snapshot: Self::Snapshot) -> Arc<[u8]> {
            Arc::from(snapshot)
        }
    }

    fn provider(values: &[u8]) -> ScriptedIdentityProviderV1 {
        provider_with_epoch(values).0
    }

    fn provider_with_epoch(
        values: &[u8],
    ) -> (ScriptedIdentityProviderV1, Rc<Cell<u64>>, Rc<Cell<bool>>) {
        let mutation_epoch = Rc::new(Cell::new(0));
        let epoch_unavailable = Rc::new(Cell::new(false));
        (
            ScriptedIdentityProviderV1 {
                snapshots: values.iter().map(|value| Ok(vec![*value; 4])).collect(),
                mutation_epoch: Rc::clone(&mutation_epoch),
                epoch_unavailable: Rc::clone(&epoch_unavailable),
            },
            mutation_epoch,
            epoch_unavailable,
        )
    }

    #[test]
    fn fixed_contract_order_is_exact_and_identity_only() {
        assert_eq!(PRODUCTION_PLIRON_PASS_CONTRACTS_V1.len(), 9);
        assert_eq!(
            PRODUCTION_PLIRON_PASS_CONTRACTS_V1.map(|contract| contract.pass()),
            crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
        );
        assert!(PRODUCTION_PLIRON_PASS_CONTRACTS_V1.iter().all(|contract| {
            contract.allowed_effect() == PlironPassAllowedEffectV1::PreserveExactStructuralIdentity
        }));

        let mut session = begin_production_pliron_pass_contract_session_v1(provider(&[1])).unwrap();
        assert!(matches!(
            session.run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || Ok::<_, ()>(())),
            Err(PlironPassPreservationErrorV1::PassOrderMismatch {
                expected: KernelCheckPassKindV1::TensorLayout,
                observed: KernelCheckPassKindV1::MemoryBounds,
                ..
            })
        ));
    }

    #[test]
    fn identity_and_checkpoint_bounds_reject_at_one_under_before_acceptance() {
        let exact_initial = ProductionAnalysisResourceLimitsV1::new(5, 14);
        let mut session = begin_production_pliron_pass_contract_session_with_resource_limits_v1(
            provider(&[1, 1]),
            exact_initial,
        )
        .unwrap();
        let initial = session.initial_identity_resource_upper_bound_v1();
        assert_eq!(initial.work_upper_bound(), 5);
        assert_eq!(initial.retained_storage_upper_bound(), 14);
        assert_eq!(initial.peak_storage_upper_bound(), 14);
        assert_eq!(session.input_census_v1().canonical_bytes, 4);

        session
            .run_contiguous_pass_with_resource_limits_v1(
                KernelCheckPassKindV1::TensorLayout,
                ProductionAnalysisResourceLimitsV1::new(13, 9),
                || Ok::<_, ()>(()),
            )
            .unwrap()
            .unwrap();
        let checkpoint = session.last_checkpoint_resource_upper_bound_v1().unwrap();
        assert_eq!(checkpoint.work_upper_bound(), 13);
        assert_eq!(checkpoint.retained_storage_upper_bound(), 5);
        assert_eq!(checkpoint.peak_storage_upper_bound(), 9);

        let mut rejected = begin_production_pliron_pass_contract_session_with_resource_limits_v1(
            provider(&[1, 1]),
            exact_initial,
        )
        .unwrap();
        let error = rejected
            .run_contiguous_pass_with_resource_limits_v1(
                KernelCheckPassKindV1::TensorLayout,
                ProductionAnalysisResourceLimitsV1::new(12, 9),
                || Ok::<_, ()>(()),
            )
            .unwrap_err();
        assert_eq!(error.code(), "FE2O3-PRESERVE-030");

        let initial_error = begin_production_pliron_pass_contract_session_with_resource_limits_v1(
            provider(&[1]),
            ProductionAnalysisResourceLimitsV1::new(4, 14),
        )
        .err()
        .unwrap();
        assert_eq!(initial_error.code(), "FE2O3-PRESERVE-030");
    }

    #[test]
    fn changed_identity_and_stale_input_wrap_the_exact_mismatch() {
        let mut changed =
            begin_production_pliron_pass_contract_session_v1(provider(&[1, 1, 2])).unwrap();
        changed
            .begin_pass(KernelCheckPassKindV1::TensorLayout, true)
            .unwrap();
        let error = changed
            .end_pass(KernelCheckPassKindV1::TensorLayout)
            .unwrap_err();
        assert_eq!(error.code(), "FE2O3-PRESERVE-025");
        assert!(error.to_string().contains("FE2O3-PRESERVE-010"));

        let mut stale =
            begin_production_pliron_pass_contract_session_v1(provider(&[1, 2])).unwrap();
        let error = stale
            .begin_pass(KernelCheckPassKindV1::TensorLayout, true)
            .unwrap_err();
        assert_eq!(error.code(), "FE2O3-PRESERVE-026");
        assert!(error.to_string().contains("FE2O3-PRESERVE-010"));
    }

    #[test]
    fn clean_fixed_pipeline_returns_one_compact_certificate_per_pass() {
        let values = vec![7; 1 + (2 * MAX_PLIRON_PASS_CONTRACTS_V1)];
        let mut session =
            begin_production_pliron_pass_contract_session_v1(provider(&values)).unwrap();
        for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
            session.begin_pass(contract.pass(), true).unwrap();
            session.end_pass(contract.pass()).unwrap();
        }
        let report = session.finish().unwrap();
        assert!(report.is_exact_identity());
        assert!(report.detects_persistent_structural_mutation());
        assert!(report.detects_transient_mutation_attempts());
        assert!(report.enforces_analysis_only_ir_immutability());
        assert!(!report.grants_analysis_result_soundness_authority());
        assert!(!report.grants_read_only_or_analysis_soundness_authority());
        assert_eq!(report.certificates().len(), 9);
        assert_eq!(report.input_identity().canonical_len(), 4);
        assert!(report.exactly_matches_retained_output(&report.clone()));
        let mut stale_certificate = report.clone();
        stale_certificate.certificates[3].mutation_epoch += 1;
        assert!(!stale_certificate.is_exact_identity());

        assert_eq!(
            report
                .certificates()
                .iter()
                .map(PlironPassPreservationCertificateV1::pass)
                .collect::<Vec<_>>(),
            PRODUCTION_PLIRON_PASS_CONTRACTS_V1
                .iter()
                .map(PlironPassContractV1::pass)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn contiguous_pipeline_uses_one_initial_and_one_post_pass_snapshot() {
        let values = vec![9; MAX_PLIRON_PASS_CONTRACTS_V1 + 1];
        let mut session =
            begin_production_pliron_pass_contract_session_v1(provider(&values)).unwrap();
        for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
            session
                .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                .unwrap()
                .unwrap();
        }
        assert!(session.finish().unwrap().is_exact_identity());
    }

    #[test]
    fn report_equality_retains_bytes_when_compact_labels_collide() {
        fn report(value: u8) -> PlironPassPreservationReportV1 {
            let mut snapshot = vec![0; 64];
            snapshot[0] = value;
            snapshot[32] = value;
            let provider = ScriptedIdentityProviderV1 {
                snapshots: (0..=MAX_PLIRON_PASS_CONTRACTS_V1)
                    .map(|_| Ok(snapshot.clone()))
                    .collect(),
                mutation_epoch: Rc::new(Cell::new(0)),
                epoch_unavailable: Rc::new(Cell::new(false)),
            };
            let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
            for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
                session
                    .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                    .unwrap()
                    .unwrap();
            }
            session.finish().unwrap()
        }

        let first = report(1);
        let second = report(2);
        assert_eq!(first.output_identity(), second.output_identity());
        assert!(!first.exactly_matches_retained_output(&second));
        assert_ne!(first, second);
    }

    #[test]
    fn omitted_execution_and_identity_failures_fail_closed() {
        let values = vec![3; 1 + (2 * MAX_PLIRON_PASS_CONTRACTS_V1)];
        let session = begin_production_pliron_pass_contract_session_v1(provider(&values)).unwrap();
        assert!(matches!(
            session.finish(),
            Err(PlironPassPreservationErrorV1::OmittedPassDeclaration {
                pass: KernelCheckPassKindV1::TensorLayout,
                ..
            })
        ));

        let provider = ScriptedIdentityProviderV1 {
            snapshots: VecDeque::from([Err(IdentityCaptureFailureV1::Unavailable {
                source_code: "FE2O3-PRESERVE-002",
                detail: "error[FE2O3-PRESERVE-002]: canonical identity is too large".to_owned(),
            })]),
            mutation_epoch: Rc::new(Cell::new(0)),
            epoch_unavailable: Rc::new(Cell::new(false)),
        };
        let error = begin_production_pliron_pass_contract_session_v1(provider)
            .err()
            .expect("identity resource failure must reject the session");
        assert_eq!(error.code(), "FE2O3-PRESERVE-028");
        assert!(error.to_string().contains("FE2O3-PRESERVE-002"));

        let provider = ScriptedIdentityProviderV1 {
            snapshots: VecDeque::from([
                Ok(vec![1; 4]),
                Err(IdentityCaptureFailureV1::Unavailable {
                    source_code: "FE2O3-PRESERVE-001",
                    detail: "unsupported post-pass operation".to_owned(),
                }),
            ]),
            mutation_epoch: Rc::new(Cell::new(0)),
            epoch_unavailable: Rc::new(Cell::new(false)),
        };
        let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = session
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || Ok::<_, ()>(()))
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::StructuralIdentityChanged {
                pass: KernelCheckPassKindV1::TensorLayout,
                source_code: "FE2O3-PRESERVE-001",
                ..
            }
        ));
        assert!(error.to_string().contains("post-pass structural identity"));
    }

    #[test]
    fn transient_mutation_and_mutation_before_error_are_rejected() {
        let (provider, epoch, _) = provider_with_epoch(&[1, 1]);
        let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = session
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                epoch.set(epoch.get() + 2);
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::MutationAttempted {
                pass: Some(KernelCheckPassKindV1::TensorLayout),
                before: 0,
                after: 2,
            }
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-020");

        let (provider, epoch, _) = provider_with_epoch(&[1, 1]);
        let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = session
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                epoch.set(epoch.get() + 1);
                Err::<(), _>("analysis rejected after mutation")
            })
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::MutationAttempted {
                pass: Some(KernelCheckPassKindV1::TensorLayout),
                ..
            }
        ));
    }

    #[test]
    fn stale_epoch_panics_and_epoch_exhaustion_fail_closed() {
        let (stale_provider, epoch, _) = provider_with_epoch(&[1, 1, 1]);
        let mut session = begin_production_pliron_pass_contract_session_v1(stale_provider).unwrap();
        session
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();
        epoch.set(1);
        let error = session
            .run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || Ok::<_, ()>(()))
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::StaleMutationEpoch {
                pass: KernelCheckPassKindV1::MemoryBounds,
                expected: 0,
                observed: 1,
            }
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-021");

        let mut panicking =
            begin_production_pliron_pass_contract_session_v1(provider(&[1, 1])).unwrap();
        let error = panicking
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || -> Result<(), ()> {
                panic!("analysis panic")
            })
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::AnalysisPanicked {
                pass: KernelCheckPassKindV1::TensorLayout,
            }
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-027");

        let (provider, _, unavailable) = provider_with_epoch(&[1]);
        unavailable.set(true);
        let error = begin_production_pliron_pass_contract_session_v1(provider)
            .err()
            .expect("exhausted mutation epoch rejects session construction");
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::MutationEpochUnavailable { pass: None, .. }
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-023");
    }
}
