#[cfg(test)]
mod observation_tests {
    use super::*;
    use crate::production_analysis::pliron_ir_identity::{
        BuiltIdentityV1, LivePlironStructuralIdentityProviderV1,
    };
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use crate::production_analysis::pliron_resource_envelope::{
        ProductionAnalysisResourceLimitsV1 as Limits, ProductionAnalysisResourcePhaseV1 as Phase,
    };
    use pliron::{
        builtin::{attributes::UnitAttr, ops::FuncOp, types::FunctionType},
        context::Context,
        op::Op,
    };
    use std::{cell::Cell, rc::Rc};

    const PHASE: Phase = Phase::PassPreservation;

    fn fixture() -> (Context, FuncOp) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "session_observation".try_into().unwrap(),
            signature,
        );
        dialect_kernel::ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(function.get_entry_block(&context), &context);
        (context, function)
    }

    #[test]
    fn observed_session_matches_ordinary_with_live_floor_and_exact_limits() {
        let hard = Limits::production_hard_ceiling();
        for short in [0, 1, 2] {
            let (context, function) = fixture();
            let floor_owner = PlironPassContractSessionV1::new(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
                hard,
            )
            .unwrap();
            let bound = floor_owner.initial_identity_resource_upper_bound_v1();
            let total = bound.checked_then_retain(bound, PHASE).unwrap();
            let limits = Limits::new(
                total.work_upper_bound() - usize::from(short == 1),
                total.peak_storage_upper_bound() - usize::from(short == 2),
            );
            {
                let mut receipt = Receipt::new(bound, limits).unwrap();
                let phase = receipt.phase(PHASE, 0).unwrap();
                let result = begin_production_pliron_pass_contract_session_with_observation_v1(
                    LivePlironStructuralIdentityProviderV1::new(&context, &function),
                    hard,
                    Some(&phase.observer(&Ok)),
                );
                if short == 0 {
                    let owner = result.unwrap();
                    assert_eq!(owner.initial_identity_resource_upper_bound_v1(), bound);
                    assert_eq!(owner.input_census_v1(), floor_owner.input_census_v1());
                    assert_eq!(owner.input_identity, floor_owner.input_identity);
                    assert!(
                        owner
                            .provider
                            .require_exact_identity(
                                owner.lineage.as_ref().unwrap(),
                                floor_owner.lineage.as_ref().unwrap()
                            )
                            .is_ok()
                    );
                    phase.commit(bound).unwrap();
                    assert_eq!(receipt.complete(), Ok(bound));
                    let released = receipt
                        .drop_owner(PHASE, owner, bound.retained_storage_upper_bound())
                        .unwrap();
                    assert_eq!(released.retained_storage_upper_bound(), 0);
                    assert_eq!(released.work_upper_bound(), bound.work_upper_bound());
                    assert_eq!(
                        released.peak_storage_upper_bound(),
                        bound.peak_storage_upper_bound()
                    );
                } else {
                    let Err(PlironPassPreservationErrorV1::ResourceLimit { resource }) = result
                    else {
                        panic!("expected cumulative limit refusal");
                    };
                    drop(phase);
                    let state = receipt.snapshot();
                    let error = state.first_denial.unwrap();
                    assert_eq!(
                        resource,
                        if short == 1 {
                            "work upper bound"
                        } else {
                            "peak storage upper bound"
                        }
                    );
                    assert_eq!(error.resource, resource);
                    assert!(state.committed.work_upper_bound() > 1);
                    assert!(!state.caught_panic);
                    limits
                        .require(
                            PHASE,
                            bound.checked_then_retain(state.committed, PHASE).unwrap(),
                        )
                        .unwrap();
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
                }
            }
            drop(floor_owner);
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Fault {
        Healthy,
        FirstEpoch,
        SecondEpoch,
        SecondEpochChanged,
        EpochPanic,
        CaptureDenied,
        DeniedThenPanic,
        LabelPanic,
        ComparisonPanic,
        EpochChanged,
        RetainPanic,
        RetainWrongLength,
        SnapshotDropPanic,
        ProviderDropPanic,
    }

    #[derive(Default)]
    struct Calls {
        epochs: Cell<usize>,
        captures: Cell<usize>,
        comparisons: Cell<usize>,
        drops: Cell<usize>,
        snapshot_drops: Cell<usize>,
        retains: Cell<usize>,
        panic_next_snapshot_drop: Cell<bool>,
    }

    struct ObservedSnapshot {
        inner: Option<BuiltIdentityV1>,
        calls: Rc<Calls>,
    }

    impl Drop for ObservedSnapshot {
        fn drop(&mut self) {
            drop(self.inner.take());
            self.calls
                .snapshot_drops
                .set(self.calls.snapshot_drops.get() + 1);
            assert!(
                !self.calls.panic_next_snapshot_drop.replace(false),
                "injected snapshot destructor panic"
            );
        }
    }

    struct FaultProvider<'a> {
        live: LivePlironStructuralIdentityProviderV1<'a>,
        fault: Fault,
        calls: Rc<Calls>,
    }

    impl FaultProvider<'_> {
        fn capture(
            &mut self,
            limits: Limits,
            observer: Option<&InvocationObserverV1<'_, '_>>,
        ) -> Result<BoundedPlironIdentityCaptureV1<ObservedSnapshot>, IdentityCaptureFailureV1>
        {
            self.calls.captures.set(self.calls.captures.get() + 1);
            let deny = matches!(self.fault, Fault::CaptureDenied | Fault::DeniedThenPanic);
            let limits = if deny {
                Limits::new(0, usize::MAX)
            } else {
                limits
            };
            let result = self
                .live
                .capture_with_resource_observation_v1(limits, observer);
            if deny {
                assert!(matches!(
                    result,
                    Err(IdentityCaptureFailureV1::ResourceLimit(_))
                ));
                assert_ne!(
                    self.fault,
                    Fault::DeniedThenPanic,
                    "panic after real capture quota refusal"
                );
                return Err(IdentityCaptureFailureV1::Unavailable {
                    source_code: "test-converted-resource-denial",
                    detail: "callback converted its real capture refusal".to_owned(),
                });
            }
            result.map(|capture| BoundedPlironIdentityCaptureV1 {
                snapshot: ObservedSnapshot {
                    inner: Some(capture.snapshot),
                    calls: Rc::clone(&self.calls),
                },
                input_census: capture.input_census,
                resource_upper_bound: capture.resource_upper_bound,
            })
        }
    }

    impl Drop for FaultProvider<'_> {
        fn drop(&mut self) {
            self.calls.drops.set(self.calls.drops.get() + 1);
            assert_ne!(
                self.fault,
                Fault::ProviderDropPanic,
                "injected provider destructor panic"
            );
        }
    }

    impl PlironStructuralIdentityProviderV1 for FaultProvider<'_> {
        type Snapshot = ObservedSnapshot;
        fn mutation_epoch(&self) -> Result<u64, MutationEpochCaptureFailureV1> {
            let count = self.calls.epochs.get() + 1;
            self.calls.epochs.set(count);
            assert_ne!(
                self.fault,
                Fault::EpochPanic,
                "injected epoch callback panic"
            );
            if matches!(
                (self.fault, count),
                (Fault::FirstEpoch, 1) | (Fault::SecondEpoch, 2)
            ) {
                return Err(MutationEpochCaptureFailureV1::new(
                    "injected epoch callback failure",
                ));
            }
            self.live.mutation_epoch().map(|epoch| {
                epoch
                    + u64::from(
                        self.fault == Fault::EpochChanged
                            || (self.fault == Fault::SecondEpochChanged && count == 2),
                    )
            })
        }
        fn capture_with_resource_limits_v1(
            &mut self,
            limits: Limits,
        ) -> Result<BoundedPlironIdentityCaptureV1<Self::Snapshot>, IdentityCaptureFailureV1>
        {
            self.capture(limits, None)
        }
        fn capture_with_resource_observation_v1(
            &mut self,
            limits: Limits,
            observer: Option<&InvocationObserverV1<'_, '_>>,
        ) -> Result<BoundedPlironIdentityCaptureV1<Self::Snapshot>, IdentityCaptureFailureV1>
        {
            self.capture(limits, observer)
        }
        fn label(&self, snapshot: &Self::Snapshot) -> PlironStructuralIdentityLabelV1 {
            assert_ne!(
                self.fault,
                Fault::LabelPanic,
                "injected label callback panic"
            );
            self.live.label(snapshot.inner.as_ref().unwrap())
        }
        fn require_exact_identity(
            &self,
            expected: &Self::Snapshot,
            observed: &Self::Snapshot,
        ) -> Result<(), IdentityComparisonFailureV1> {
            self.calls.comparisons.set(self.calls.comparisons.get() + 1);
            assert_ne!(
                self.fault,
                Fault::ComparisonPanic,
                "injected exact comparison panic"
            );
            self.live.require_exact_identity(
                expected.inner.as_ref().unwrap(),
                observed.inner.as_ref().unwrap(),
            )
        }
        fn retain_exact_identity(&self, mut snapshot: Self::Snapshot) -> Arc<[u8]> {
            self.calls.retains.set(self.calls.retains.get() + 1);
            assert_ne!(
                self.fault,
                Fault::RetainPanic,
                "injected retention callback panic"
            );
            let retained = self
                .live
                .retain_exact_identity(snapshot.inner.take().unwrap());
            if self.fault == Fault::RetainWrongLength {
                Arc::from([])
            } else {
                retained
            }
        }
    }

    #[test]
    fn observed_setup_refusal_precedes_provider_callbacks() {
        let (context, function) = fixture();
        for limits in [
            Limits::new(0, usize::MAX),
            Limits::new(usize::MAX, MAX_PLIRON_PASS_CONTRACTS_V1),
        ] {
            let calls = Rc::new(Calls::default());
            let provider = FaultProvider {
                live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                fault: Fault::Healthy,
                calls: Rc::clone(&calls),
            };
            let mut receipt =
                Receipt::new(Default::default(), Limits::production_hard_ceiling()).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            assert!(matches!(
                begin_production_pliron_pass_contract_session_with_observation_v1(
                    provider,
                    limits,
                    Some(&phase.observer(&Ok))
                ),
                Err(PlironPassPreservationErrorV1::ResourceLimit { .. })
            ));
            drop(phase);
            assert_eq!(
                (calls.epochs.get(), calls.captures.get(), calls.drops.get()),
                (0, 0, 1)
            );
            let state = receipt.snapshot();
            assert_eq!(state.committed, Default::default());
            assert_eq!(state.first_denial.unwrap().phase, PHASE);
            assert!(!state.caught_panic);
        }
        let floor_owner = PlironPassContractSessionV1::new(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
            Limits::production_hard_ceiling(),
        )
        .unwrap();
        let floor = floor_owner.initial_identity_resource_upper_bound_v1();
        let calls = Rc::new(Calls::default());
        {
            let mut receipt =
                Receipt::new(floor, Limits::new(floor.work_upper_bound(), usize::MAX)).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            assert!(matches!(
                begin_production_pliron_pass_contract_session_with_observation_v1(
                    FaultProvider {
                        live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                        fault: Fault::Healthy,
                        calls: Rc::clone(&calls),
                    },
                    Limits::production_hard_ceiling(),
                    Some(&phase.observer(&Ok)),
                ),
                Err(PlironPassPreservationErrorV1::ResourceLimit { .. })
            ));
            assert_eq!(
                (calls.epochs.get(), calls.captures.get(), calls.drops.get()),
                (0, 0, 1)
            );
            drop(phase);
            let state = receipt.snapshot();
            assert_eq!(state.committed, Default::default());
            assert_eq!(state.first_denial.unwrap().phase, PHASE);
            assert_eq!(state.first_denial.unwrap().resource, "work upper bound");
            assert!(!state.caught_panic);
        }
        drop(floor_owner);
    }

    #[test]
    fn observed_constructor_callbacks_preserve_prefix_denial_and_panic() {
        let hard = Limits::production_hard_ceiling();
        for fault in [
            Fault::FirstEpoch,
            Fault::SecondEpoch,
            Fault::SecondEpochChanged,
            Fault::EpochPanic,
            Fault::CaptureDenied,
            Fault::DeniedThenPanic,
            Fault::LabelPanic,
        ] {
            let (context, function) = fixture();
            let baseline = PlironPassContractSessionV1::new(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
                hard,
            )
            .unwrap();
            let bound = baseline.initial_identity_resource_upper_bound_v1();
            drop(baseline);
            let calls = Rc::new(Calls::default());
            let provider = FaultProvider {
                live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                fault,
                calls: Rc::clone(&calls),
            };
            let mut receipt = Receipt::new(Default::default(), hard).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                begin_production_pliron_pass_contract_session_with_observation_v1(
                    provider,
                    hard,
                    Some(&phase.observer(&Ok)),
                )
            }));
            match fault {
                Fault::SecondEpochChanged => assert!(matches!(
                    result,
                    Ok(Err(PlironPassPreservationErrorV1::MutationAttempted { .. }))
                )),
                Fault::FirstEpoch | Fault::SecondEpoch => assert!(matches!(
                    result,
                    Ok(Err(
                        PlironPassPreservationErrorV1::MutationEpochUnavailable { .. }
                    ))
                )),
                Fault::CaptureDenied => assert!(matches!(
                    result,
                    Ok(Err(
                        PlironPassPreservationErrorV1::IdentityUnavailable { .. }
                    ))
                )),
                _ => assert!(result.is_err()),
            }
            drop(phase);
            assert_eq!(calls.drops.get(), 1);
            let state = receipt.snapshot();
            let complete_capture = matches!(
                fault,
                Fault::SecondEpoch | Fault::SecondEpochChanged | Fault::LabelPanic
            );
            assert_eq!(
                state.committed.work_upper_bound(),
                if complete_capture {
                    bound.work_upper_bound()
                } else {
                    1
                }
            );
            assert_eq!(
                state.committed.peak_storage_upper_bound(),
                if complete_capture {
                    bound.peak_storage_upper_bound()
                } else {
                    MAX_PLIRON_PASS_CONTRACTS_V1 + 1
                }
            );
            let denied = matches!(fault, Fault::CaptureDenied | Fault::DeniedThenPanic);
            assert_eq!(state.first_denial.is_some(), denied);
            assert_eq!(
                state.caught_panic,
                matches!(
                    fault,
                    Fault::EpochPanic | Fault::DeniedThenPanic | Fault::LabelPanic
                )
            );
            if let Some(error) = state.first_denial {
                assert_eq!(error.phase, Phase::StructuralIdentity);
                assert_eq!(error.resource, "work upper bound");
                assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
            } else if state.caught_panic {
                assert_eq!(receipt.complete(), Err(ReceiptFailure::CaughtPanic));
            }
        }
    }

    #[test]
    fn observed_checkpoint_replaces_actual_owner_at_exact_and_short_limits() {
        let hard = Limits::production_hard_ceiling();
        let pass = PRODUCTION_PLIRON_PASS_CONTRACTS_V1[0].pass();
        for short in [0, 1, 2] {
            let (context, function) = fixture();
            let mut baseline = PlironPassContractSessionV1::new(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
                hard,
            )
            .unwrap();
            let initial = baseline.initial_identity_resource_upper_bound_v1();
            let old_storage = baseline
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            baseline.begin_pass(pass, false).unwrap();
            baseline.end_pass(pass).unwrap();
            let checkpoint = baseline.last_checkpoint_resource_upper_bound_v1().unwrap();
            let expected = initial
                .checked_then_replace_retained(old_storage, checkpoint, PHASE)
                .unwrap();
            let total = expected.checked_then_retain(expected, PHASE).unwrap();
            let limits = Limits::new(
                total.work_upper_bound() - usize::from(short == 1),
                total.peak_storage_upper_bound() - usize::from(short == 2),
            );
            // The independently executed ordinary session remains alive as Q.
            {
                let mut receipt = Receipt::new(expected, limits).unwrap();
                let calls = Rc::new(Calls::default());
                let phase = receipt.phase(PHASE, 0).unwrap();
                let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                    FaultProvider {
                        live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                        fault: Fault::Healthy,
                        calls: Rc::clone(&calls),
                    },
                    hard,
                    Some(&phase.observer(&Ok)),
                )
                .unwrap();
                phase.commit(initial).unwrap();
                owner.begin_pass(pass, false).unwrap();
                let phase = receipt.phase(PHASE, old_storage).unwrap();
                let result =
                    owner.end_pass_with_observation_v1(pass, hard, Some(&phase.observer(&Ok)));
                // The old real identity owner is gone before retained credit can
                // be replaced. The wrapper delegates capture and exact comparison.
                assert!(calls.snapshot_drops.get() >= 1);
                if short == 0 {
                    result.unwrap();
                    assert_eq!(calls.snapshot_drops.get(), 1);
                    assert_eq!(
                        owner.last_checkpoint_resource_upper_bound_v1(),
                        Some(checkpoint)
                    );
                    assert_eq!(owner.certificates, baseline.certificates);
                    assert_eq!(owner.lineage_identity, baseline.lineage_identity);
                    assert!(
                        owner
                            .provider
                            .live
                            .require_exact_identity(
                                owner.lineage.as_ref().unwrap().inner.as_ref().unwrap(),
                                baseline.lineage.as_ref().unwrap(),
                            )
                            .is_ok()
                    );
                    assert_eq!(phase.commit(checkpoint).unwrap(), expected);
                    assert_eq!(receipt.complete(), Ok(expected));
                    let released = receipt
                        .drop_owner(PHASE, owner, expected.retained_storage_upper_bound())
                        .unwrap();
                    assert_eq!(calls.snapshot_drops.get(), 2);
                    assert_eq!(calls.drops.get(), 1);
                    assert_eq!(released.retained_storage_upper_bound(), 0);
                    assert_eq!(released.work_upper_bound(), expected.work_upper_bound());
                    assert_eq!(
                        released.peak_storage_upper_bound(),
                        expected.peak_storage_upper_bound()
                    );
                } else {
                    assert!(matches!(
                        result,
                        Err(PlironPassPreservationErrorV1::ResourceLimit { .. })
                    ));
                    assert_eq!(owner.next, 0);
                    assert!(owner.certificates.is_empty());
                    assert!(owner.lineage.is_none());
                    assert!(owner.pending.is_none());
                    drop(phase);
                    let state = receipt.snapshot();
                    let denial = state.first_denial.unwrap();
                    assert_eq!(
                        denial.resource,
                        if short == 1 {
                            "work upper bound"
                        } else {
                            "peak storage upper bound"
                        }
                    );
                    assert_eq!(
                        state.committed.retained_storage_upper_bound(),
                        state.committed.peak_storage_upper_bound()
                    );
                    assert!(!state.caught_panic);
                    limits
                        .require(
                            PHASE,
                            expected
                                .checked_then_retain(state.committed, PHASE)
                                .unwrap(),
                        )
                        .unwrap();
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
                    drop(owner);
                }
            }
            drop(baseline);
        }
    }

    #[test]
    fn observed_checkpoint_callback_errors_keep_prefix_and_drop_owners() {
        let hard = Limits::production_hard_ceiling();
        let pass = PRODUCTION_PLIRON_PASS_CONTRACTS_V1[0].pass();
        for fault in [
            Fault::FirstEpoch,
            Fault::EpochPanic,
            Fault::CaptureDenied,
            Fault::DeniedThenPanic,
            Fault::LabelPanic,
            Fault::ComparisonPanic,
            Fault::EpochChanged,
            Fault::SnapshotDropPanic,
        ] {
            let (context, function) = fixture();
            let mut baseline = PlironPassContractSessionV1::new(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
                hard,
            )
            .unwrap();
            let initial = baseline.initial_identity_resource_upper_bound_v1();
            let old_storage = baseline
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            baseline.begin_pass(pass, false).unwrap();
            baseline.end_pass(pass).unwrap();
            let checkpoint = baseline.last_checkpoint_resource_upper_bound_v1().unwrap();
            let floor = initial
                .checked_then_replace_retained(old_storage, checkpoint, PHASE)
                .unwrap();
            {
                let mut receipt = Receipt::new(floor, hard).unwrap();
                let calls = Rc::new(Calls::default());
                let phase = receipt.phase(PHASE, 0).unwrap();
                let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                    FaultProvider {
                        live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                        fault: Fault::Healthy,
                        calls: Rc::clone(&calls),
                    },
                    hard,
                    Some(&phase.observer(&Ok)),
                )
                .unwrap();
                phase.commit(initial).unwrap();
                owner.begin_pass(pass, false).unwrap();
                owner.provider.fault = fault;
                calls.epochs.set(0);
                calls
                    .panic_next_snapshot_drop
                    .set(fault == Fault::SnapshotDropPanic);
                let phase = receipt.phase(PHASE, old_storage).unwrap();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    owner.end_pass_with_observation_v1(pass, hard, Some(&phase.observer(&Ok)))
                }));
                match fault {
                    Fault::FirstEpoch => assert!(matches!(
                        result,
                        Ok(Err(
                            PlironPassPreservationErrorV1::MutationEpochUnavailable { .. }
                        ))
                    )),
                    Fault::CaptureDenied => assert!(matches!(
                        result,
                        Ok(Err(
                            PlironPassPreservationErrorV1::StructuralIdentityChanged { .. }
                        ))
                    )),
                    Fault::EpochChanged => assert!(matches!(
                        result,
                        Ok(Err(PlironPassPreservationErrorV1::MutationAttempted { .. }))
                    )),
                    _ => assert!(result.is_err()),
                }
                assert_eq!(owner.next, 0);
                assert!(owner.certificates.is_empty());
                assert!(owner.lineage.is_none());
                assert!(owner.pending.is_none());
                let denied = matches!(fault, Fault::CaptureDenied | Fault::DeniedThenPanic);
                assert_eq!(calls.snapshot_drops.get(), if denied { 1 } else { 2 });
                drop(phase);
                let state = receipt.snapshot();
                let local_prefix = if denied {
                    ProductionAnalysisResourceUpperBoundV1::checked_phase(
                        PHASE,
                        old_storage + 1,
                        1,
                        old_storage,
                    )
                    .unwrap()
                } else {
                    checkpoint
                };
                let expected = initial
                    .checked_then_replace_retained(old_storage, local_prefix, PHASE)
                    .unwrap();
                assert_eq!(
                    state.committed.work_upper_bound(),
                    expected.work_upper_bound()
                );
                assert_eq!(
                    state.committed.peak_storage_upper_bound(),
                    expected.peak_storage_upper_bound()
                );
                assert_eq!(
                    state.committed.retained_storage_upper_bound(),
                    expected.peak_storage_upper_bound()
                );
                assert_eq!(state.first_denial.is_some(), denied);
                assert_eq!(
                    state.caught_panic,
                    matches!(
                        fault,
                        Fault::SnapshotDropPanic
                            | Fault::EpochPanic
                            | Fault::DeniedThenPanic
                            | Fault::LabelPanic
                            | Fault::ComparisonPanic
                    )
                );
                if let Some(denial) = state.first_denial {
                    assert_eq!(denial.phase, Phase::StructuralIdentity);
                    assert_eq!(denial.resource, "work upper bound");
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
                } else if state.caught_panic {
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::CaughtPanic));
                }
                hard.require(
                    PHASE,
                    floor.checked_then_retain(state.committed, PHASE).unwrap(),
                )
                .unwrap();
                drop(owner);
                assert_eq!(calls.drops.get(), 1);
            }
            drop(baseline);
        }
    }

    #[test]
    fn observed_checkpoints_match_all_nine_ordinary_preservation_boundaries() {
        let hard = Limits::production_hard_ceiling();
        let (context, function) = fixture();
        let mut baseline = PlironPassContractSessionV1::new(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
            hard,
        )
        .unwrap();
        let initial = baseline.initial_identity_resource_upper_bound_v1();
        let mut checkpoints = Vec::new();
        let mut expected = initial;
        for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
            let replaced = baseline
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            baseline.begin_pass(contract.pass(), false).unwrap();
            baseline.end_pass(contract.pass()).unwrap();
            let bound = baseline.last_checkpoint_resource_upper_bound_v1().unwrap();
            expected = expected
                .checked_then_replace_retained(replaced, bound, PHASE)
                .unwrap();
            checkpoints.push((replaced, bound, expected));
        }
        let total = expected.checked_then_retain(expected, PHASE).unwrap();
        let limits = Limits::new(total.work_upper_bound(), total.peak_storage_upper_bound());
        {
            let mut receipt = Receipt::new(expected, limits).unwrap();
            let calls = Rc::new(Calls::default());
            let phase = receipt.phase(PHASE, 0).unwrap();
            let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                FaultProvider {
                    live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                    fault: Fault::Healthy,
                    calls: Rc::clone(&calls),
                },
                hard,
                Some(&phase.observer(&Ok)),
            )
            .unwrap();
            phase.commit(initial).unwrap();
            // This checks preservation boundaries, not execution of the analyses.
            for (position, (contract, (replaced, bound, cumulative))) in
                PRODUCTION_PLIRON_PASS_CONTRACTS_V1
                    .into_iter()
                    .zip(checkpoints)
                    .enumerate()
            {
                owner.begin_pass(contract.pass(), false).unwrap();
                let phase = receipt.phase(PHASE, replaced).unwrap();
                owner
                    .end_pass_with_observation_v1(contract.pass(), hard, Some(&phase.observer(&Ok)))
                    .unwrap();
                assert_eq!(calls.snapshot_drops.get(), position + 1);
                assert_eq!(owner.last_checkpoint_resource_upper_bound_v1(), Some(bound));
                assert_eq!(phase.commit(bound).unwrap(), cumulative);
            }
            assert_eq!(owner.next, MAX_PLIRON_PASS_CONTRACTS_V1);
            assert_eq!(owner.certificates, baseline.certificates);
            assert_eq!(receipt.complete(), Ok(expected));
            let released = receipt
                .drop_owner(PHASE, owner, expected.retained_storage_upper_bound())
                .unwrap();
            assert_eq!(calls.snapshot_drops.get(), MAX_PLIRON_PASS_CONTRACTS_V1 + 1);
            assert_eq!(released.retained_storage_upper_bound(), 0);
            assert_eq!(released.work_upper_bound(), expected.work_upper_bound());
            assert_eq!(
                released.peak_storage_upper_bound(),
                expected.peak_storage_upper_bound()
            );
        }
        drop(baseline);
    }

    #[test]
    fn observed_checkpoint_real_mutation_preserves_semantic_error_precedence() {
        let hard = Limits::production_hard_ceiling();
        let pass = PRODUCTION_PLIRON_PASS_CONTRACTS_V1[0].pass();
        for restored in [false, true] {
            let (context, function) = fixture();
            let calls = Rc::new(Calls::default());
            let mut receipt = Receipt::new(Default::default(), hard).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                FaultProvider {
                    live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                    fault: Fault::Healthy,
                    calls: Rc::clone(&calls),
                },
                hard,
                Some(&phase.observer(&Ok)),
            )
            .unwrap();
            let initial = owner.initial_identity_resource_upper_bound_v1();
            phase.commit(initial).unwrap();
            let replaced = owner
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            owner.begin_pass(pass, false).unwrap();
            let operation = function.get_operation();
            let original = operation.deref(&context).attributes.clone();
            operation
                .deref_mut(&context)
                .attributes
                .set("observed_change".try_into().unwrap(), UnitAttr::new());
            if restored {
                operation.deref_mut(&context).attributes = original;
            }
            let phase = receipt.phase(PHASE, replaced).unwrap();
            let result = owner.end_pass_with_observation_v1(pass, hard, Some(&phase.observer(&Ok)));
            if restored {
                assert!(matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::MutationAttempted { .. })
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::StructuralIdentityChanged { .. })
                ));
            }
            assert_eq!(calls.captures.get(), 2);
            assert_eq!(calls.snapshot_drops.get(), 2);
            assert!(owner.certificates.is_empty());
            assert!(owner.last_checkpoint_resource_upper_bound_v1().is_none());
            drop(phase);
            let state = receipt.snapshot();
            assert!(state.committed.work_upper_bound() > initial.work_upper_bound());
            assert_eq!(
                state.committed.retained_storage_upper_bound(),
                state.committed.peak_storage_upper_bound()
            );
            assert_eq!(state.first_denial, None);
            assert!(!state.caught_panic);
            drop(owner);
            assert_eq!(calls.drops.get(), 1);
        }
    }

    #[test]
    fn observed_checkpoint_header_refusal_precedes_capture() {
        let hard = Limits::production_hard_ceiling();
        let (context, function) = fixture();
        for limits in [Limits::new(0, usize::MAX), Limits::new(usize::MAX, 0)] {
            let calls = Rc::new(Calls::default());
            let mut receipt = Receipt::new(Default::default(), hard).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                FaultProvider {
                    live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                    fault: Fault::Healthy,
                    calls: Rc::clone(&calls),
                },
                hard,
                Some(&phase.observer(&Ok)),
            )
            .unwrap();
            let initial = owner.initial_identity_resource_upper_bound_v1();
            phase.commit(initial).unwrap();
            let replaced = owner
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            let pass = PRODUCTION_PLIRON_PASS_CONTRACTS_V1[0].pass();
            owner.begin_pass(pass, false).unwrap();
            calls.epochs.set(0);
            let phase = receipt.phase(PHASE, replaced).unwrap();
            assert!(matches!(
                owner.end_pass_with_observation_v1(pass, limits, Some(&phase.observer(&Ok))),
                Err(PlironPassPreservationErrorV1::ResourceLimit { .. })
            ));
            assert_eq!(calls.captures.get(), 1);
            assert_eq!(calls.epochs.get(), 0);
            assert_eq!(calls.snapshot_drops.get(), 1);
            assert!(owner.certificates.is_empty());
            drop(phase);
            let state = receipt.snapshot();
            assert_eq!(state.committed, initial);
            assert_eq!(state.first_denial.unwrap().phase, PHASE);
            assert!(!state.caught_panic);
            drop(owner);
            assert_eq!(calls.drops.get(), 1);
        }
    }

    #[test]
    fn observed_finish_preserves_exact_transfer_limits_and_callback_failures() {
        let hard = Limits::production_hard_ceiling();
        for (fault, short) in [
            (Fault::Healthy, 0),
            (Fault::Healthy, 1),
            (Fault::Healthy, 2),
            (Fault::Healthy, 3),
            (Fault::FirstEpoch, 0),
            (Fault::EpochPanic, 0),
            (Fault::EpochChanged, 0),
            (Fault::RetainPanic, 0),
            (Fault::RetainWrongLength, 0),
            (Fault::ProviderDropPanic, 0),
        ] {
            let (context, function) = fixture();
            let mut baseline = PlironPassContractSessionV1::new(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
                hard,
            )
            .unwrap();
            let mut floor = baseline.initial_identity_resource_upper_bound_v1();
            for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
                let old = baseline
                    .lineage_identity_resource_upper_bound_v1()
                    .retained_storage_upper_bound();
                baseline.begin_pass(contract.pass(), false).unwrap();
                baseline.end_pass(contract.pass()).unwrap();
                floor = floor
                    .checked_then_replace_retained(
                        old,
                        baseline.last_checkpoint_resource_upper_bound_v1().unwrap(),
                        PHASE,
                    )
                    .unwrap();
            }
            let replaced = baseline
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            let baseline = baseline.finish().unwrap();
            let finish_bound = baseline.resource_upper_bound_v1();
            floor = floor
                .checked_then_replace_retained(replaced, finish_bound, PHASE)
                .unwrap();
            let calls = Rc::new(Calls::default());
            let total = floor.checked_then_retain(floor, PHASE).unwrap();
            let receipt_limits = Limits::new(
                total.work_upper_bound() - usize::from(short == 3),
                total.peak_storage_upper_bound(),
            );
            {
                let mut receipt = Receipt::new(floor, receipt_limits).unwrap();
                let phase = receipt.phase(PHASE, 0).unwrap();
                let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                    FaultProvider {
                        live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                        fault: Fault::Healthy,
                        calls: Rc::clone(&calls),
                    },
                    hard,
                    Some(&phase.observer(&Ok)),
                )
                .unwrap();
                let initial = owner.initial_identity_resource_upper_bound_v1();
                phase.commit(initial).unwrap();
                for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
                    let replaced = owner
                        .lineage_identity_resource_upper_bound_v1()
                        .retained_storage_upper_bound();
                    owner.begin_pass(contract.pass(), false).unwrap();
                    let phase = receipt.phase(PHASE, replaced).unwrap();
                    owner
                        .end_pass_with_observation_v1(
                            contract.pass(),
                            hard,
                            Some(&phase.observer(&Ok)),
                        )
                        .unwrap();
                    phase
                        .commit(owner.last_checkpoint_resource_upper_bound_v1().unwrap())
                        .unwrap();
                }
                let before = receipt.snapshot().committed;
                owner.provider.fault = fault;
                calls.epochs.set(0);
                let limits = Limits::new(
                    finish_bound.work_upper_bound() - usize::from(short == 1),
                    finish_bound.peak_storage_upper_bound() - usize::from(short == 2),
                );
                let phase = receipt.phase(PHASE, replaced).unwrap();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    owner.finish_with_observation_v1(limits, Some(&phase.observer(&Ok)))
                }));
                assert_eq!(calls.drops.get(), 1);
                assert_eq!(calls.snapshot_drops.get(), MAX_PLIRON_PASS_CONTRACTS_V1 + 1);
                let expected = before
                    .checked_then_replace_retained(replaced, finish_bound, PHASE)
                    .unwrap();
                if fault == Fault::Healthy && short == 0 {
                    let report = result.unwrap().unwrap();
                    assert_eq!(report, baseline);
                    assert_eq!(calls.retains.get(), 1);
                    assert_eq!(phase.commit(finish_bound).unwrap(), expected);
                    assert_eq!(receipt.complete(), Ok(expected));
                    let released = receipt
                        .drop_owner(PHASE, report, expected.retained_storage_upper_bound())
                        .unwrap();
                    assert_eq!(released.retained_storage_upper_bound(), 0);
                    assert_eq!(released.work_upper_bound(), expected.work_upper_bound());
                    assert_eq!(
                        released.peak_storage_upper_bound(),
                        expected.peak_storage_upper_bound()
                    );
                } else {
                    if short != 0 {
                        assert!(matches!(
                            result,
                            Ok(Err(PlironPassPreservationErrorV1::ResourceLimit { .. }))
                        ));
                        assert_eq!(calls.epochs.get(), 0);
                        assert_eq!(calls.retains.get(), 0);
                    } else {
                        match fault {
                            Fault::FirstEpoch => assert!(matches!(
                                result,
                                Ok(Err(
                                    PlironPassPreservationErrorV1::MutationEpochUnavailable { .. }
                                ))
                            )),
                            Fault::EpochChanged => assert!(matches!(
                                result,
                                Ok(Err(PlironPassPreservationErrorV1::MutationAttempted { .. }))
                            )),
                            Fault::RetainWrongLength => assert!(matches!(
                                result,
                                Ok(Err(
                                    PlironPassPreservationErrorV1::InvalidSessionState { .. }
                                ))
                            )),
                            _ => assert!(result.is_err()),
                        }
                        assert_eq!(
                            calls.retains.get(),
                            usize::from(matches!(
                                fault,
                                Fault::RetainPanic
                                    | Fault::RetainWrongLength
                                    | Fault::ProviderDropPanic
                            ))
                        );
                    }
                    drop(phase);
                    let state = receipt.snapshot();
                    assert_eq!(
                        state.committed.work_upper_bound(),
                        if short == 0 {
                            expected.work_upper_bound()
                        } else {
                            before.work_upper_bound()
                        }
                    );
                    assert_eq!(
                        state.committed.peak_storage_upper_bound(),
                        if short == 0 {
                            expected.peak_storage_upper_bound()
                        } else {
                            before.peak_storage_upper_bound()
                        }
                    );
                    if short != 0 {
                        assert_eq!(state.committed, before);
                        let denial = state.first_denial.unwrap();
                        assert_eq!(denial.phase, PHASE);
                        assert_eq!(
                            denial.resource,
                            if short == 1 || short == 3 {
                                "work upper bound"
                            } else {
                                "peak storage upper bound"
                            }
                        );
                        assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
                    } else {
                        assert_eq!(state.first_denial, None);
                        assert_eq!(
                            state.committed.retained_storage_upper_bound(),
                            state.committed.peak_storage_upper_bound()
                        );
                    }
                    assert_eq!(
                        state.caught_panic,
                        matches!(
                            fault,
                            Fault::EpochPanic | Fault::RetainPanic | Fault::ProviderDropPanic
                        )
                    );
                    hard.require(
                        PHASE,
                        floor.checked_then_retain(state.committed, PHASE).unwrap(),
                    )
                    .unwrap();
                }
            }
            drop(baseline);
        }
    }

    include!("begin_observation_v1_tests.rs");
    include!("scoped_observation_v1_tests.rs");
}
