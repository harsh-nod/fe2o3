use super::*;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn stable() -> R28PersistentStableBindingV1 {
    R28PersistentStableBindingV1 {
        queue: QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(28),
                    generation: DeviceGenerationV1(29),
                },
                id: VmIdV1(30),
            },
            id: QueueInstanceIdV1(31),
            generation: QueueGenerationV1(32),
        },
        memory_session_id: 33,
        control_identity: digest(34),
        storage_identity: digest(35),
        first_attachment_generation: 1,
        initial_predecessor_dispatch_generation: 40,
    }
}

fn attempt(
    stable: R28PersistentStableBindingV1,
    attachment_generation: u64,
    predecessor_dispatch_generation: u64,
) -> R28PersistentAttemptBindingV1 {
    R28PersistentAttemptBindingV1 {
        stable,
        attachment_generation,
        predecessor_dispatch_generation,
        dispatch_generation: predecessor_dispatch_generation + 1,
    }
}

fn model() -> R28PersistentHotCurrentnessScopeModelV1 {
    R28PersistentHotCurrentnessScopeModelV1::new_model_only(stable()).unwrap()
}

fn opened() -> (
    R28PersistentHotCurrentnessScopeModelV1,
    R28PersistentAttemptBindingV1,
) {
    let mut model = model();
    let first = attempt(stable(), 1, 40);
    model
        .open_model_only(first, R28ContractedCurrentnessOutcomeV1::Current)
        .unwrap();
    (model, first)
}

fn submit(
    model: &mut R28PersistentHotCurrentnessScopeModelV1,
    receipt: R28PersistentAttemptBindingV1,
) {
    model
        .submit_model_only(
            receipt,
            R28SubmitCurrentnessV1::all_current(),
            R28SubmitDispositionV1::Published,
        )
        .unwrap();
}

fn finish_attempt(
    model: &mut R28PersistentHotCurrentnessScopeModelV1,
    receipt: R28PersistentAttemptBindingV1,
) {
    submit(model, receipt);
    model
        .observe_completion_model_only(
            receipt,
            R28CompletionCurrentnessV1::all_current(),
            R28CompletionDispositionV1::Completed,
        )
        .unwrap();
    model
        .recycle_model_only(
            receipt,
            R28RecycleCurrentnessV1::all_current(),
            R28RecycleDispositionV1::Recycled,
        )
        .unwrap();
    model
        .detach_recycled_model_only(receipt, R28DetachDispositionV1::Detached)
        .unwrap();
}

fn assert_valid(model: &R28PersistentHotCurrentnessScopeModelV1) {
    model.validate_global_invariants().unwrap();
}

#[test]
fn exact_checkpoint_budget_is_seven_initial_and_eight_replay() {
    let (mut model, first) = opened();
    finish_attempt(&mut model, first);
    assert_eq!(model.snapshot().operational_checkpoint_count, 7);

    let replay = attempt(stable(), 2, 41);
    model
        .prepare_replay_model_only(replay, R28ContractedCurrentnessOutcomeV1::Current)
        .unwrap();
    assert_eq!(model.snapshot().operational_checkpoint_count, 8);
    finish_attempt(&mut model, replay);
    assert_eq!(model.snapshot().operational_checkpoint_count, 15);

    model
        .close_model_only(
            stable(),
            R28ContractedCurrentnessOutcomeV1::Current,
            R28CloseDispositionV1::ReleasedAndRetaken,
        )
        .unwrap();
    let closed = model.snapshot();
    assert_eq!(closed.full_open_audit_count, 1);
    assert_eq!(closed.full_close_audit_count, 1);
    assert_eq!(closed.completed_attempt_count, 2);
    assert_eq!(closed.scope_phase, R28PersistentScopePhaseV1::Closed);
    assert_valid(&model);
}

#[test]
fn occupancy_is_retryable_only_before_side_effect_and_preserves_exact_receipt() {
    for occupancy in [
        R28RingOccupancyV1::Full,
        R28RingOccupancyV1::InsufficientSpace,
    ] {
        let (mut model, first) = opened();
        assert_eq!(
            model.submit_model_only(
                first,
                R28SubmitCurrentnessV1::all_current(),
                R28SubmitDispositionV1::Occupancy(occupancy),
            ),
            Err(R28PersistentHotCurrentnessErrorV1::RetryableOccupancy(
                occupancy
            ))
        );
        let retry = model.snapshot();
        assert_eq!(retry.active_attempt, Some(first));
        assert_eq!(
            retry.attempt_custody,
            R28AttemptAuthorityCustodyV1::Prepared
        );
        assert_eq!(retry.operational_checkpoint_count, 2);
        submit(&mut model, first);
        assert_eq!(model.snapshot().operational_checkpoint_count, 5);
        assert_valid(&model);
    }
}

#[test]
fn submit_currentness_loss_is_classified_before_or_after_effect() {
    let cases = [
        (
            R28SubmitCurrentnessV1 {
                before_counter: R28ContractedCurrentnessOutcomeV1::Lost,
                ..R28SubmitCurrentnessV1::all_current()
            },
            R28TerminalReasonV1::SubmitCurrentnessLostBeforeEffect,
            1,
        ),
        (
            R28SubmitCurrentnessV1 {
                before_side_effect: R28ContractedCurrentnessOutcomeV1::Lost,
                ..R28SubmitCurrentnessV1::all_current()
            },
            R28TerminalReasonV1::SubmitCurrentnessLostBeforeEffect,
            2,
        ),
        (
            R28SubmitCurrentnessV1 {
                after_publication: R28ContractedCurrentnessOutcomeV1::Lost,
                ..R28SubmitCurrentnessV1::all_current()
            },
            R28TerminalReasonV1::SubmitCurrentnessLostAfterPublication,
            3,
        ),
    ];
    for (currentness, reason, checkpoints) in cases {
        let (mut model, first) = opened();
        assert_eq!(
            model.submit_model_only(first, currentness, R28SubmitDispositionV1::Published),
            Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
        );
        assert_eq!(model.snapshot().terminal_reason, Some(reason));
        assert_eq!(model.snapshot().operational_checkpoint_count, checkpoints);
        assert_eq!(
            model.snapshot().terminal_custody,
            Some(R28TerminalNativeCustodyStageV1::Attached)
        );
        assert_valid(&model);
    }
}

#[test]
fn completion_and_recycle_have_two_exact_checkpoints() {
    for (before, reason, checkpoints) in [
        (
            true,
            R28TerminalReasonV1::CompletionCurrentnessLostBeforeObservation,
            4,
        ),
        (
            false,
            R28TerminalReasonV1::CompletionCurrentnessLostAfterObservation,
            5,
        ),
    ] {
        let (mut model, first) = opened();
        submit(&mut model, first);
        let currentness = R28CompletionCurrentnessV1 {
            before_observation: if before {
                R28ContractedCurrentnessOutcomeV1::Lost
            } else {
                R28ContractedCurrentnessOutcomeV1::Current
            },
            after_observation: if before {
                R28ContractedCurrentnessOutcomeV1::Current
            } else {
                R28ContractedCurrentnessOutcomeV1::Lost
            },
        };
        assert_eq!(
            model.observe_completion_model_only(
                first,
                currentness,
                R28CompletionDispositionV1::Completed,
            ),
            Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
        );
        assert_eq!(model.snapshot().terminal_reason, Some(reason));
        assert_eq!(model.snapshot().operational_checkpoint_count, checkpoints);
        assert_eq!(
            model.snapshot().terminal_custody,
            Some(R28TerminalNativeCustodyStageV1::Published)
        );
    }

    for (before, reason, checkpoints) in [
        (
            true,
            R28TerminalReasonV1::RecycleCurrentnessLostBeforeReset,
            6,
        ),
        (
            false,
            R28TerminalReasonV1::RecycleCurrentnessLostAfterReset,
            7,
        ),
    ] {
        let (mut model, first) = opened();
        submit(&mut model, first);
        model
            .observe_completion_model_only(
                first,
                R28CompletionCurrentnessV1::all_current(),
                R28CompletionDispositionV1::Completed,
            )
            .unwrap();
        let currentness = R28RecycleCurrentnessV1 {
            before_reset: if before {
                R28ContractedCurrentnessOutcomeV1::Lost
            } else {
                R28ContractedCurrentnessOutcomeV1::Current
            },
            after_reset: if before {
                R28ContractedCurrentnessOutcomeV1::Current
            } else {
                R28ContractedCurrentnessOutcomeV1::Lost
            },
        };
        assert_eq!(
            model.recycle_model_only(first, currentness, R28RecycleDispositionV1::Recycled),
            Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
        );
        assert_eq!(model.snapshot().terminal_reason, Some(reason));
        assert_eq!(model.snapshot().operational_checkpoint_count, checkpoints);
        assert_eq!(
            model.snapshot().terminal_custody,
            Some(R28TerminalNativeCustodyStageV1::Completed)
        );
    }
}

#[test]
fn completion_and_recycle_failures_bind_checkpoint_and_native_custody_stage() {
    for (disposition, checkpoints, custody) in [
        (
            R28CompletionDispositionV1::TerminalFailureAfterFirstCheckpoint,
            4,
            R28TerminalNativeCustodyStageV1::Published,
        ),
        (
            R28CompletionDispositionV1::CompletionLedgerFailureAfterSecondCheckpoint,
            5,
            R28TerminalNativeCustodyStageV1::Completed,
        ),
    ] {
        let (mut model, first) = opened();
        submit(&mut model, first);
        assert_eq!(
            model.observe_completion_model_only(
                first,
                R28CompletionCurrentnessV1::all_current(),
                disposition,
            ),
            Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
        );
        let terminal = model.snapshot();
        assert_eq!(terminal.operational_checkpoint_count, checkpoints);
        assert_eq!(terminal.terminal_custody, Some(custody));
    }

    for (disposition, checkpoints, custody) in [
        (
            R28RecycleDispositionV1::TerminalFailureAfterFirstCheckpoint,
            6,
            R28TerminalNativeCustodyStageV1::Completed,
        ),
        (
            R28RecycleDispositionV1::RecycleLedgerFailureAfterSecondCheckpoint,
            7,
            R28TerminalNativeCustodyStageV1::Recycled,
        ),
    ] {
        let (mut model, first) = opened();
        submit(&mut model, first);
        model
            .observe_completion_model_only(
                first,
                R28CompletionCurrentnessV1::all_current(),
                R28CompletionDispositionV1::Completed,
            )
            .unwrap();
        assert_eq!(
            model.recycle_model_only(first, R28RecycleCurrentnessV1::all_current(), disposition),
            Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
        );
        let terminal = model.snapshot();
        assert_eq!(terminal.operational_checkpoint_count, checkpoints);
        assert_eq!(terminal.terminal_custody, Some(custody));
    }
}

#[test]
fn submission_failures_preserve_exact_checkpoint_and_terminal_custody_stage() {
    for (disposition, reason, checkpoints, custody) in [
        (
            R28SubmitDispositionV1::StructuralFailureBeforeFirstCheckpoint,
            R28TerminalReasonV1::StructuralSubmissionFailure,
            0,
            R28TerminalNativeCustodyStageV1::Attached,
        ),
        (
            R28SubmitDispositionV1::TerminalBeforeSideEffectAfterFirstCheckpoint,
            R28TerminalReasonV1::StructuralSubmissionFailure,
            1,
            R28TerminalNativeCustodyStageV1::Attached,
        ),
        (
            R28SubmitDispositionV1::TerminalBeforeSideEffectAfterSecondCheckpoint,
            R28TerminalReasonV1::StructuralSubmissionFailure,
            2,
            R28TerminalNativeCustodyStageV1::Attached,
        ),
        (
            R28SubmitDispositionV1::FailureAfterPossibleSideEffectBeforeFinalCheckpoint,
            R28TerminalReasonV1::PossibleSubmissionSideEffect,
            2,
            R28TerminalNativeCustodyStageV1::Attached,
        ),
        (
            R28SubmitDispositionV1::FailureAfterFinalCheckpoint,
            R28TerminalReasonV1::PossibleSubmissionSideEffect,
            3,
            R28TerminalNativeCustodyStageV1::Attached,
        ),
        (
            R28SubmitDispositionV1::PublicationLedgerFailureAfterFinalCheckpoint,
            R28TerminalReasonV1::PublicationLedgerFailure,
            3,
            R28TerminalNativeCustodyStageV1::Published,
        ),
    ] {
        let (mut model, first) = opened();
        assert_eq!(
            model.submit_model_only(first, R28SubmitCurrentnessV1::all_current(), disposition),
            Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
        );
        let snapshot = model.snapshot();
        assert_eq!(snapshot.terminal_reason, Some(reason));
        assert_eq!(snapshot.terminal_custody, Some(custody));
        assert_eq!(snapshot.operational_checkpoint_count, checkpoints);
        assert_eq!(snapshot.stable_custody, R28StableAuthorityCustodyV1::Opaque);
        assert_eq!(
            snapshot.attempt_custody,
            R28AttemptAuthorityCustodyV1::Opaque
        );
        assert_valid(&model);
    }
}

#[test]
fn public_receipt_mismatch_is_atomic_but_post_authenticated_substitution_is_terminal() {
    let (mut model, first) = opened();
    let foreign = R28PersistentAttemptBindingV1 {
        attachment_generation: 2,
        ..first
    };
    let before = model.snapshot();
    assert_eq!(
        model.submit_model_only(
            foreign,
            R28SubmitCurrentnessV1::all_current(),
            R28SubmitDispositionV1::Published,
        ),
        Err(R28PersistentHotCurrentnessErrorV1::ReceiptMismatch)
    );
    assert_eq!(model.snapshot(), before);
    assert_eq!(
        model.absorb_post_authenticated_substitution_model_only(first, foreign),
        Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
    );
    assert_eq!(
        model.snapshot().terminal_reason,
        Some(R28TerminalReasonV1::PostAuthenticatedAttemptSubstitution)
    );
    assert_eq!(
        model.snapshot().terminal_custody,
        Some(R28TerminalNativeCustodyStageV1::Attached)
    );
}

#[test]
fn cancel_consumes_control_and_precludes_full_close() {
    let (mut model, first) = opened();
    model
        .cancel_prepared_model_only(first, R28CancelDispositionV1::Cancelled)
        .unwrap();
    let cancelled = model.snapshot();
    assert_eq!(cancelled.scope_phase, R28PersistentScopePhaseV1::Cancelled);
    assert_eq!(
        cancelled.stable_custody,
        R28StableAuthorityCustodyV1::Released
    );
    assert_eq!(
        cancelled.control_state,
        R28PersistentControlStateV1::Ordinary
    );
    assert_eq!(
        cancelled.attempt_custody,
        R28AttemptAuthorityCustodyV1::Available
    );
    assert_eq!(
        model.close_model_only(
            stable(),
            R28ContractedCurrentnessOutcomeV1::Current,
            R28CloseDispositionV1::ReleasedAndRetaken,
        ),
        Err(R28PersistentHotCurrentnessErrorV1::IllegalPhase)
    );
    assert_valid(&model);
}

#[test]
fn only_data_detached_scope_can_close() {
    let (mut model, first) = opened();
    let substituted = R28PersistentStableBindingV1 {
        memory_session_id: stable().memory_session_id + 1,
        ..stable()
    };
    let attached = model.snapshot();
    assert_eq!(
        model.close_model_only(
            substituted,
            R28ContractedCurrentnessOutcomeV1::Current,
            R28CloseDispositionV1::ReleasedAndRetaken,
        ),
        Err(R28PersistentHotCurrentnessErrorV1::AttemptStillAttached)
    );
    assert_eq!(model.snapshot(), attached);
    finish_attempt(&mut model, first);
    assert_eq!(
        model.close_model_only(
            substituted,
            R28ContractedCurrentnessOutcomeV1::Current,
            R28CloseDispositionV1::ReleasedAndRetaken,
        ),
        Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
    );
    assert_eq!(
        model.snapshot().terminal_custody,
        Some(R28TerminalNativeCustodyStageV1::RetainedControl)
    );
}

#[test]
fn detach_cancel_and_post_audit_close_failures_are_terminal_opaque() {
    for (disposition, reason, custody) in [
        (
            R28DetachDispositionV1::PreflightFailure,
            R28TerminalReasonV1::DetachPreflightFailure,
            R28TerminalNativeCustodyStageV1::Attached,
        ),
        (
            R28DetachDispositionV1::ReleaseFailureAttached,
            R28TerminalReasonV1::DetachReleaseFailure,
            R28TerminalNativeCustodyStageV1::Attached,
        ),
        (
            R28DetachDispositionV1::StorageSubstitution,
            R28TerminalReasonV1::DetachStorageSubstitution,
            R28TerminalNativeCustodyStageV1::DataDetached,
        ),
        (
            R28DetachDispositionV1::NativeRestoreFailure,
            R28TerminalReasonV1::DetachNativeRestoreFailure,
            R28TerminalNativeCustodyStageV1::StorageDetached,
        ),
        (
            R28DetachDispositionV1::SettlementFailure,
            R28TerminalReasonV1::DetachSettlementFailure,
            R28TerminalNativeCustodyStageV1::Restored,
        ),
    ] {
        let (mut model, first) = opened();
        submit(&mut model, first);
        model
            .observe_completion_model_only(
                first,
                R28CompletionCurrentnessV1::all_current(),
                R28CompletionDispositionV1::Completed,
            )
            .unwrap();
        model
            .recycle_model_only(
                first,
                R28RecycleCurrentnessV1::all_current(),
                R28RecycleDispositionV1::Recycled,
            )
            .unwrap();
        assert_eq!(
            model.detach_recycled_model_only(first, disposition),
            Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
        );
        assert_eq!(model.snapshot().terminal_reason, Some(reason));
        assert_eq!(model.snapshot().terminal_custody, Some(custody));
    }

    for (disposition, reason, custody) in [
        (
            R28CancelDispositionV1::ReleaseFailureAttached,
            R28TerminalReasonV1::CancelReleaseFailure,
            R28TerminalNativeCustodyStageV1::Attached,
        ),
        (
            R28CancelDispositionV1::ReleaseFailureDataDetached,
            R28TerminalReasonV1::CancelReleaseFailure,
            R28TerminalNativeCustodyStageV1::DataDetached,
        ),
        (
            R28CancelDispositionV1::StorageSubstitution,
            R28TerminalReasonV1::CancelStorageSubstitution,
            R28TerminalNativeCustodyStageV1::DataDetached,
        ),
        (
            R28CancelDispositionV1::NativeRestoreFailure,
            R28TerminalReasonV1::CancelNativeRestoreFailure,
            R28TerminalNativeCustodyStageV1::StorageDetached,
        ),
        (
            R28CancelDispositionV1::LedgerFailure,
            R28TerminalReasonV1::CancelLedgerFailure,
            R28TerminalNativeCustodyStageV1::Restored,
        ),
    ] {
        let (mut model, first) = opened();
        assert_eq!(
            model.cancel_prepared_model_only(first, disposition),
            Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
        );
        assert_eq!(model.snapshot().terminal_reason, Some(reason));
        assert_eq!(model.snapshot().terminal_custody, Some(custody));
    }

    for (audit, disposition, reason, custody) in [
        (
            R28ContractedCurrentnessOutcomeV1::Lost,
            R28CloseDispositionV1::ReleasedAndRetaken,
            R28TerminalReasonV1::FullCloseCurrentnessLost,
            R28TerminalNativeCustodyStageV1::RetainedControl,
        ),
        (
            R28ContractedCurrentnessOutcomeV1::Current,
            R28CloseDispositionV1::ControlReleaseFailure,
            R28TerminalReasonV1::CloseControlReleaseFailure,
            R28TerminalNativeCustodyStageV1::RetainedControl,
        ),
        (
            R28ContractedCurrentnessOutcomeV1::Current,
            R28CloseDispositionV1::ModelRetakeFailure,
            R28TerminalReasonV1::CloseModelRetakeFailure,
            R28TerminalNativeCustodyStageV1::ControlReleased,
        ),
    ] {
        let (mut model, first) = opened();
        finish_attempt(&mut model, first);
        assert_eq!(
            model.close_model_only(stable(), audit, disposition,),
            Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
        );
        let snapshot = model.snapshot();
        assert_eq!(snapshot.full_close_audit_count, 1);
        assert_eq!(snapshot.terminal_reason, Some(reason));
        assert_eq!(snapshot.terminal_custody, Some(custody));
        assert_eq!(snapshot.stable_custody, R28StableAuthorityCustodyV1::Opaque);
    }
}

#[test]
fn bind_consumption_advances_attachment_generation_even_when_audit_fails() {
    let mut model = model();
    let first = attempt(stable(), 1, 40);
    assert_eq!(
        model.open_model_only(first, R28ContractedCurrentnessOutcomeV1::Lost),
        Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
    );
    assert_eq!(model.snapshot().next_attachment_generation, 2);

    let (mut replay_model, first) = opened();
    finish_attempt(&mut replay_model, first);
    let replay = attempt(stable(), 2, 41);
    assert_eq!(
        replay_model.prepare_replay_model_only(replay, R28ContractedCurrentnessOutcomeV1::Lost,),
        Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
    );
    assert_eq!(replay_model.snapshot().next_attachment_generation, 3);
    assert_eq!(replay_model.snapshot().operational_checkpoint_count, 8);
}

#[test]
fn generation_boundaries_reject_missing_dispatch_successor() {
    let invalid = R28PersistentStableBindingV1 {
        initial_predecessor_dispatch_generation: u64::MAX - 1,
        ..stable()
    };
    assert!(!invalid.is_valid());
    assert!(matches!(
        R28PersistentHotCurrentnessScopeModelV1::new_model_only(invalid),
        Err(R28PersistentHotCurrentnessErrorV1::InvalidStableBinding)
    ));

    let mut model = model();
    let dispatch_max = R28PersistentAttemptBindingV1 {
        stable: stable(),
        attachment_generation: 1,
        predecessor_dispatch_generation: u64::MAX - 1,
        dispatch_generation: u64::MAX,
    };
    assert!(!dispatch_max.is_structurally_valid());
    let before = model.snapshot();
    assert_eq!(
        model.open_model_only(dispatch_max, R28ContractedCurrentnessOutcomeV1::Current),
        Err(R28PersistentHotCurrentnessErrorV1::GenerationExhausted)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn model_observation_counters_saturate_without_changing_production_phase() {
    let (mut checkpoint_model, first) = opened();
    checkpoint_model.set_counters_for_test_only(u64::MAX, 0);
    checkpoint_model
        .submit_model_only(
            first,
            R28SubmitCurrentnessV1::all_current(),
            R28SubmitDispositionV1::Published,
        )
        .unwrap();
    assert_eq!(
        checkpoint_model.snapshot().operational_checkpoint_count,
        u64::MAX
    );
    assert_eq!(
        checkpoint_model.snapshot().attempt_custody,
        R28AttemptAuthorityCustodyV1::Published
    );
    assert_eq!(checkpoint_model.snapshot().terminal_reason, None);

    let (mut completion_model, first) = opened();
    submit(&mut completion_model, first);
    completion_model
        .observe_completion_model_only(
            first,
            R28CompletionCurrentnessV1::all_current(),
            R28CompletionDispositionV1::Completed,
        )
        .unwrap();
    completion_model
        .recycle_model_only(
            first,
            R28RecycleCurrentnessV1::all_current(),
            R28RecycleDispositionV1::Recycled,
        )
        .unwrap();
    completion_model.set_counters_for_test_only(7, u64::MAX);
    completion_model
        .detach_recycled_model_only(first, R28DetachDispositionV1::Detached)
        .unwrap();
    assert_eq!(
        completion_model.snapshot().completed_attempt_count,
        u64::MAX
    );
    assert_eq!(completion_model.snapshot().terminal_reason, None);
    assert_eq!(
        completion_model.snapshot().control_state,
        R28PersistentControlStateV1::DataDetached
    );
}

#[test]
fn full_audit_loss_and_terminal_absorption_are_sticky() {
    let mut model = model();
    let first = attempt(stable(), 1, 40);
    assert_eq!(
        model.open_model_only(first, R28ContractedCurrentnessOutcomeV1::Lost),
        Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
    );
    let terminal = model.snapshot();
    assert_eq!(terminal.full_open_audit_count, 1);
    assert_eq!(
        terminal.terminal_reason,
        Some(R28TerminalReasonV1::FullOpenCurrentnessLost)
    );
    assert_eq!(
        terminal.terminal_custody,
        Some(R28TerminalNativeCustodyStageV1::Attached)
    );
    assert_eq!(
        model.submit_model_only(
            first,
            R28SubmitCurrentnessV1::all_current(),
            R28SubmitDispositionV1::Published,
        ),
        Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
    );
    assert_eq!(model.snapshot(), terminal);
}

#[test]
fn replay_bind_requires_its_distinct_operational_outcome() {
    let (mut model, first) = opened();
    finish_attempt(&mut model, first);
    let replay = attempt(stable(), 2, 41);
    assert_eq!(
        model.prepare_replay_model_only(replay, R28ContractedCurrentnessOutcomeV1::Lost),
        Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
    );
    assert_eq!(
        model.snapshot().terminal_reason,
        Some(R28TerminalReasonV1::ReplayBindCurrentnessLost)
    );
}
