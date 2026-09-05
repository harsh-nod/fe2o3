use super::*;
use std::format;

fn binding(effect: R35EffectV1) -> R35ReplayBindingV1 {
    R35ReplayBindingV1 {
        queue_id: 11,
        queue_generation: 12,
        attachment_generation: 13,
        next_attachment_generation: 14,
        storage_identity: 15,
        predecessor_generation: 16,
        effect,
    }
}

fn loan(open_succeeded: bool, retake_succeeded: bool) -> R35LoanOutcomeV1 {
    R35LoanOutcomeV1 {
        open_succeeded,
        retake_succeeded,
    }
}

fn success_observations() -> R35ReplayObservationsV1 {
    R35ReplayObservationsV1 {
        admission: R35AdmissionObservationV1::Admitted,
        preparation: R35PreparationObservationV1::Prepared,
        former_mapped_facts_loan: loan(true, true),
        former_retain_loan: loan(true, true),
        fused_loan: loan(true, true),
        mapped_facts_succeeded: true,
        detach_succeeded: true,
        authenticated_construction_succeeded: true,
        retain_succeeded: true,
        final_audit_succeeded: true,
        cancellation_succeeded: true,
        session_healthy: true,
        quarantine_succeeded: true,
    }
}

fn run_former(observations: R35ReplayObservationsV1) -> R35ReplaySnapshotV1 {
    R35RetainedControlReplayModelV1::new_model_only(binding(R35EffectV1::ReadWrite))
        .unwrap()
        .run_former_model_only(observations)
}

fn run_fused(observations: R35ReplayObservationsV1) -> R35ReplaySnapshotV1 {
    R35RetainedControlReplayModelV1::new_model_only(binding(R35EffectV1::ReadWrite))
        .unwrap()
        .run_fused_model_only(observations)
}

#[test]
fn binding_requires_exact_monotonic_generation_and_nonzero_coordinates() {
    assert!(binding(R35EffectV1::Read).is_valid());
    for invalid in [
        R35ReplayBindingV1 {
            queue_id: 0,
            ..binding(R35EffectV1::Write)
        },
        R35ReplayBindingV1 {
            queue_generation: 0,
            ..binding(R35EffectV1::Write)
        },
        R35ReplayBindingV1 {
            attachment_generation: 0,
            ..binding(R35EffectV1::Write)
        },
        R35ReplayBindingV1 {
            next_attachment_generation: 15,
            ..binding(R35EffectV1::Write)
        },
        R35ReplayBindingV1 {
            storage_identity: 0,
            ..binding(R35EffectV1::Write)
        },
        R35ReplayBindingV1 {
            predecessor_generation: 0,
            ..binding(R35EffectV1::Write)
        },
    ] {
        assert_eq!(
            R35RetainedControlReplayModelV1::new_model_only(invalid),
            Err(R35ModelErrorV1::InvalidBinding)
        );
    }
}

#[test]
fn success_commits_every_exact_binding_field_and_clears_recycled_roster() {
    for effect in [
        R35EffectV1::Read,
        R35EffectV1::Write,
        R35EffectV1::ReadWrite,
    ] {
        let expected = binding(effect);
        let snapshot = R35RetainedControlReplayModelV1::new_model_only(expected)
            .unwrap()
            .run_fused_model_only(success_observations());
        assert_eq!(snapshot.outcome, R35OutcomeV1::Prepared);
        assert_eq!(snapshot.custody, R35CustodyV1::PreparedAttachment);
        assert!(!snapshot.terminal_poisoned);
        assert!(snapshot.dispatch_retained);
        assert_eq!(
            snapshot.next_attachment_generation,
            expected.next_attachment_generation
        );
        assert_eq!(snapshot.detached_data_count, 0);
        assert_eq!(snapshot.detached_generation, None);
        assert_eq!(snapshot.detached_identity_count, 0);
        assert_eq!(snapshot.detached_next_insertion_index, None);
        assert_eq!(
            snapshot.attachment,
            Some(R35AttachmentV1 {
                queue_id: expected.queue_id,
                queue_generation: expected.queue_generation,
                attachment_generation: expected.attachment_generation,
                storage_identity: expected.storage_identity,
                predecessor_generation: expected.predecessor_generation,
                effect,
                authority_state: R35PreparedAuthorityStateV1::Prepared,
                terminal_custody: None,
            })
        );
    }
}

#[test]
fn successful_fusion_preserves_projected_custody_and_commit_while_reducing_loans() {
    let observations = success_observations();
    let former = run_former(observations);
    let fused = run_fused(observations);
    assert!(former.same_projected_custody_and_commit_semantics(&fused));
    assert_eq!(former.foundation_loan_attempts, 2);
    assert_eq!(fused.foundation_loan_attempts, 1);
    assert_eq!(former.currentness_observations, 2);
    assert_eq!(fused.currentness_observations, 2);
    assert!(observations.loan_equivalence_premise());
}

#[test]
fn projection_excludes_only_the_documented_model_observables() {
    let left = run_fused(success_observations());
    let mut right = left;
    right.terminal_stage = Some(R35TerminalStageV1::Admission);
    right.admission_event_index = 21;
    right.preparation_event_index = Some(22);
    right.detach_event_index = Some(23);
    right.commit_event_index = Some(24);
    right.foundation_loan_attempts = 25;
    right.currentness_observations = 26;
    right.attachment.as_mut().unwrap().authority_state = R35PreparedAuthorityStateV1::Quarantined;
    assert!(left.same_projected_custody_and_commit_semantics(&right));

    right.attachment.as_mut().unwrap().storage_identity += 1;
    assert!(!left.same_projected_custody_and_commit_semantics(&right));
}

#[test]
fn admission_and_preparation_precede_every_foundation_loan_and_detach() {
    for admission in [
        R35AdmissionObservationV1::RetryableFailure,
        R35AdmissionObservationV1::TerminalFailure,
        R35AdmissionObservationV1::Admitted,
    ] {
        for preparation in [
            R35PreparationObservationV1::UseRequestRejected,
            R35PreparationObservationV1::ReserveRejected,
            R35PreparationObservationV1::PrepareRejected,
        ] {
            let mut observations = success_observations();
            observations.admission = admission;
            observations.preparation = preparation;
            let snapshot = run_fused(observations);
            assert_eq!(snapshot.foundation_loan_attempts, 0);
            assert_eq!(snapshot.detach_event_index, None);
            if admission == R35AdmissionObservationV1::Admitted {
                assert_eq!(snapshot.preparation_event_index, Some(2));
            }
        }
    }
    let snapshot = run_fused(success_observations());
    assert!(snapshot.admission_event_index < snapshot.preparation_event_index.unwrap());
    assert!(snapshot.preparation_event_index.unwrap() < snapshot.detach_event_index.unwrap());
    assert!(snapshot.detach_event_index.unwrap() < snapshot.commit_event_index.unwrap());
}

#[test]
fn before_detach_failure_retries_only_after_clean_loan_cancel_and_healthy_session() {
    for loan_succeeded in [false, true] {
        for cancellation_succeeded in [false, true] {
            for session_healthy in [false, true] {
                let mut observations = success_observations();
                observations.mapped_facts_succeeded = false;
                observations.fused_loan = loan(true, loan_succeeded);
                observations.cancellation_succeeded = cancellation_succeeded;
                observations.session_healthy = session_healthy;
                let snapshot = run_fused(observations);
                let retryable = loan_succeeded && cancellation_succeeded && session_healthy;
                assert_eq!(snapshot.outcome == R35OutcomeV1::Retryable, retryable);
                assert_eq!(
                    snapshot.custody,
                    if retryable {
                        R35CustodyV1::RetryableInput
                    } else if cancellation_succeeded {
                        R35CustodyV1::TerminalInput
                    } else {
                        R35CustodyV1::TerminalAttached
                    }
                );
            }
        }
    }
}

#[test]
fn post_detach_failures_preserve_exact_storage_data_or_attached_custody() {
    for (mut observations, custody, stage) in [
        {
            let mut observations = success_observations();
            observations.authenticated_construction_succeeded = false;
            (
                observations,
                R35CustodyV1::TerminalStorage,
                R35TerminalStageV1::AuthenticatedConstruction,
            )
        },
        {
            let mut observations = success_observations();
            observations.retain_succeeded = false;
            (
                observations,
                R35CustodyV1::TerminalData,
                R35TerminalStageV1::Retain,
            )
        },
        {
            let mut observations = success_observations();
            observations.final_audit_succeeded = false;
            (
                observations,
                R35CustodyV1::TerminalAttached,
                R35TerminalStageV1::FinalAudit,
            )
        },
    ] {
        observations.cancellation_succeeded = true;
        observations.session_healthy = true;
        let snapshot = run_fused(observations);
        assert_eq!(snapshot.outcome, R35OutcomeV1::Terminal);
        assert_eq!(snapshot.custody, custody);
        assert_eq!(snapshot.terminal_stage, Some(stage));
        assert!(snapshot.terminal_poisoned);
        assert_eq!(snapshot.attachment.unwrap().terminal_custody, Some(custody));
    }
}

#[test]
fn no_post_detach_or_ready_retake_failure_is_retryable() {
    for cut in 0..4 {
        let mut observations = success_observations();
        match cut {
            0 => observations.authenticated_construction_succeeded = false,
            1 => observations.retain_succeeded = false,
            2 => observations.final_audit_succeeded = false,
            _ => observations.fused_loan.retake_succeeded = false,
        }
        let snapshot = run_fused(observations);
        assert_eq!(snapshot.outcome, R35OutcomeV1::Terminal);
        assert_ne!(snapshot.custody, R35CustodyV1::RetryableInput);
        assert!(snapshot.attachment.is_some());
    }
}

#[test]
fn failed_quarantine_preserves_prepared_authority_in_every_terminal_attachment() {
    for cut in 0..4 {
        let mut observations = success_observations();
        observations.quarantine_succeeded = false;
        match cut {
            0 => {
                observations.mapped_facts_succeeded = false;
                observations.cancellation_succeeded = false;
            }
            1 => observations.authenticated_construction_succeeded = false,
            2 => observations.retain_succeeded = false,
            _ => observations.final_audit_succeeded = false,
        }
        let snapshot = run_fused(observations);
        assert_eq!(
            snapshot.attachment.unwrap().authority_state,
            R35PreparedAuthorityStateV1::Prepared
        );
    }
}

#[test]
fn successful_quarantine_marks_terminal_attachment_quarantined() {
    let mut observations = success_observations();
    observations.retain_succeeded = false;
    assert_eq!(
        run_fused(observations).attachment.unwrap().authority_state,
        R35PreparedAuthorityStateV1::Quarantined
    );
}

#[test]
fn ready_plus_failed_fused_retake_is_terminal_attached_with_exact_commit_fields() {
    let mut observations = success_observations();
    observations.fused_loan.retake_succeeded = false;
    observations.quarantine_succeeded = false;
    let snapshot = run_fused(observations);
    assert_eq!(snapshot.custody, R35CustodyV1::TerminalAttached);
    assert_eq!(
        snapshot.terminal_stage,
        Some(R35TerminalStageV1::FusedLoanRetake)
    );
    let attachment = snapshot.attachment.unwrap();
    assert_eq!(attachment.queue_id, snapshot.binding.queue_id);
    assert_eq!(
        attachment.queue_generation,
        snapshot.binding.queue_generation
    );
    assert_eq!(
        attachment.attachment_generation,
        snapshot.binding.attachment_generation
    );
    assert_eq!(
        attachment.storage_identity,
        snapshot.binding.storage_identity
    );
    assert_eq!(
        attachment.predecessor_generation,
        snapshot.binding.predecessor_generation
    );
    assert_eq!(attachment.effect, snapshot.binding.effect);
    assert_eq!(
        attachment.authority_state,
        R35PreparedAuthorityStateV1::Prepared
    );
}

#[test]
fn premise_is_input_only_and_does_not_invoke_either_runner() {
    let source = include_str!("r35_fused_retained_control_replay.rs");
    let premise = source
        .split("pub const fn loan_equivalence_premise")
        .nth(1)
        .unwrap()
        .split("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum R35OutcomeV1")
        .next()
        .unwrap();
    assert!(!premise.contains("run_former_model_only"));
    assert!(!premise.contains("run_fused_model_only"));
    assert!(!premise.contains("same_projected_custody_and_commit_semantics"));
}

#[test]
fn replay_authority_carriers_are_private_and_move_only() {
    let source = include_str!("r35_fused_retained_control_replay.rs");
    for carrier in [
        "R35DetachedReplayAuthorityV1",
        "R35ReplayDataAuthorityV1",
        "R35AttachedReplayAuthorityV1",
    ] {
        assert!(source.contains(&format!("struct {carrier}")));
        assert!(!source.contains(&format!("pub struct {carrier}")));
        assert!(!source.contains(&format!("#[derive(Clone)]\nstruct {carrier}")));
        assert!(!source.contains(&format!("#[derive(Clone, Copy)]\nstruct {carrier}")));
        assert!(!source.contains(&format!("impl Clone for {carrier}")));
        assert!(!source.contains(&format!("impl Copy for {carrier}")));
    }
}

#[test]
fn exhaustive_finite_premise_implies_projected_custody_and_commit_equivalence() {
    let admissions = [
        R35AdmissionObservationV1::RetryableFailure,
        R35AdmissionObservationV1::TerminalFailure,
        R35AdmissionObservationV1::Admitted,
    ];
    let preparations = [
        R35PreparationObservationV1::UseRequestRejected,
        R35PreparationObservationV1::ReserveRejected,
        R35PreparationObservationV1::PrepareRejected,
        R35PreparationObservationV1::Prepared,
    ];
    let loans = [
        loan(false, false),
        loan(false, true),
        loan(true, false),
        loan(true, true),
    ];
    let mut admitted = 0_u64;
    for admission in admissions {
        for preparation in preparations {
            for former_mapped_facts_loan in loans {
                for former_retain_loan in loans {
                    for fused_loan in loans {
                        for mask in 0_u16..256 {
                            let bit = |index: u16| mask & (1 << index) != 0;
                            let observations = R35ReplayObservationsV1 {
                                admission,
                                preparation,
                                former_mapped_facts_loan,
                                former_retain_loan,
                                fused_loan,
                                mapped_facts_succeeded: bit(0),
                                detach_succeeded: bit(1),
                                authenticated_construction_succeeded: bit(2),
                                retain_succeeded: bit(3),
                                final_audit_succeeded: bit(4),
                                cancellation_succeeded: bit(5),
                                session_healthy: bit(6),
                                quarantine_succeeded: bit(7),
                            };
                            if !observations.loan_equivalence_premise() {
                                continue;
                            }
                            admitted += 1;
                            let former = run_former(observations);
                            let fused = run_fused(observations);
                            assert!(
                                former.same_projected_custody_and_commit_semantics(&fused),
                                "premised input diverged: {observations:?}\nformer={former:?}\nfused={fused:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    assert_eq!(admitted, 186_288);
}
