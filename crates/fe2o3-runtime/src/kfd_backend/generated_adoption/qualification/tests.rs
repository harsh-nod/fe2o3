use super::*;
use crate::kfd_backend::counted_allocations_for_test_v1 as counted;

fn publications() -> (PublicationV1, PublicationV1) {
    // Data-only recorder inputs. These do not manufacture native receipts.
    (
        PublicationV1 {
            kind: KfdGeneratedCopyPublicationKindV1::GeneratedCompute,
            submission: 11,
            native: [1; 32],
            membership: [2; 32],
        },
        PublicationV1 {
            kind: KfdGeneratedCopyPublicationKindV1::DirectionalCopy,
            submission: 12,
            native: [3; 32],
            membership: [4; 32],
        },
    )
}

fn record(
    recorder: &mut RecorderV1,
    value: PublicationV1,
    pair: Option<(PublicationV1, PublicationV1, usize)>,
) {
    assert!(recorder.next());
    recorder.record(value, pair, 99);
}

#[test]
fn qualification_recorder_requires_exact_second_publication_in_either_order_without_allocation() {
    let (compute, copy) = publications();
    for (first, second) in [(compute, copy), (copy, compute)] {
        let ((witness, duplicate), allocations) = counted(|| {
            let mut recorder = RecorderV1::new();
            record(&mut recorder, first, None);
            assert!(recorder.witness.is_none());
            record(&mut recorder, second, Some((compute, copy, 3)));
            let witness = recorder.take().unwrap();
            (witness, recorder.take())
        });
        assert_eq!(allocations, 0);
        assert_eq!(witness.device_unique_id(), 99);
        assert_eq!(witness.first_publication(), first.kind);
        assert_eq!(witness.compute_receipt_sha256(), compute.native);
        assert_eq!(witness.compute_membership_sha256(), compute.membership);
        assert_eq!(witness.copy_receipt_sha256(), copy.native);
        assert_eq!(witness.copy_membership_sha256(), copy.membership);
        assert_eq!(witness.copy_packets(), 3);
        assert_eq!(
            duplicate,
            Err(KfdGeneratedCopyCoexistenceFailureV1::NotArmed)
        );
    }
}

#[test]
fn qualification_recorder_missing_completed_or_replaced_first_receipt_cannot_pass() {
    use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
    let (compute, copy) = publications();
    let mut empty = RecorderV1::new();
    assert_eq!(empty.take(), Err(Failure::MissingPublication));
    for mutation in 0..9 {
        let mut recorder = RecorderV1::new();
        record(&mut recorder, compute, None);
        let (mut observed_compute, mut observed_copy) = (compute, copy);
        match mutation {
            0 => observed_compute.submission += 1,
            1 => observed_compute.native[0] ^= 1,
            2 => observed_compute.membership[0] ^= 1,
            3 => observed_copy.submission += 1,
            4 => observed_copy.native[0] ^= 1,
            5 => observed_copy.membership[0] ^= 1,
            6 => observed_compute.kind = copy.kind,
            7 => observed_copy.kind = compute.kind,
            8 => {}
            _ => unreachable!(),
        }
        let pair = if mutation == 8 {
            None
        } else {
            Some((observed_compute, observed_copy, 1))
        };
        let ((), allocations) = counted(|| record(&mut recorder, copy, pair));
        assert_eq!(allocations, 0);
        assert_eq!(
            recorder.take(),
            Err(if mutation == 8 {
                Failure::MissingPublication
            } else {
                Failure::PublicationIdentity
            })
        );
    }
}

#[test]
fn qualification_recorder_repeated_classes_zero_packets_and_overflow_fail_closed() {
    use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
    let (compute, copy) = publications();
    for first in [compute, copy] {
        let mut recorder = RecorderV1::new();
        record(&mut recorder, first, None);
        record(&mut recorder, first, Some((compute, copy, 1)));
        assert_eq!(recorder.take(), Err(Failure::RepeatedPublicationClass));
    }
    let mut recorder = RecorderV1::new();
    record(&mut recorder, compute, None);
    record(&mut recorder, copy, Some((compute, copy, 0)));
    assert_eq!(recorder.take(), Err(Failure::PublicationIdentity));

    let mut recorder = RecorderV1::new();
    record(&mut recorder, compute, None);
    record(&mut recorder, copy, Some((compute, copy, 1)));
    let (next, allocations) = counted(|| recorder.next());
    assert!(!next);
    assert_eq!(allocations, 0);
    assert_eq!(recorder.take(), Err(Failure::PublicationOverflow));
}

#[test]
fn qualification_recorder_refusal_is_sticky_and_taken_capture_is_inert() {
    use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
    let (compute, copy) = publications();
    for failure in [
        Failure::UnsupportedCopy,
        Failure::Terminal,
        Failure::AliasedOwners,
    ] {
        let mut recorder = RecorderV1::new();
        record(&mut recorder, compute, None);
        recorder.reject(failure);
        recorder.reject(Failure::PublicationOverflow);
        assert!(!recorder.next());
        assert_eq!(recorder.take(), Err(failure));
    }
    let mut recorder = RecorderV1::new();
    record(&mut recorder, compute, None);
    record(&mut recorder, copy, Some((compute, copy, 1)));
    let captured = recorder.take().unwrap();
    recorder.reject(Failure::Terminal);
    assert!(!recorder.next());
    assert_eq!(recorder.take(), Err(Failure::NotArmed));
    assert_eq!(captured.copy_packets(), 1);
}

#[test]
fn qualification_recorder_rejects_zero_submission_or_device_identity() {
    for (submission, device) in [(0, 99), (11, 0)] {
        let (mut compute, _) = publications();
        compute.submission = submission;
        let mut recorder = RecorderV1::new();
        assert!(recorder.next());
        recorder.record(compute, None, device);
        assert_eq!(
            recorder.take(),
            Err(KfdGeneratedCopyCoexistenceFailureV1::PublicationIdentity)
        );
    }
}

#[test]
fn qualification_public_arm_preserves_launch_gate_and_rejects_other_profiles_or_pending_work() {
    use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
    let mut unsupported = KfdRuntimeBackendV1::mock();
    assert_eq!(
        unsupported.arm_generated_copy_coexistence_qualification_v1(),
        Err(Failure::UnsupportedProfile)
    );
    assert!(unsupported.generated_copy_coexistence.is_none());
    let mut backend = KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1();
    let (result, allocations) =
        counted(|| backend.arm_generated_copy_coexistence_qualification_v1());
    assert_eq!(result, Ok(()));
    assert_eq!(allocations, 0);
    assert!(matches!(
        backend.launch_gate,
        KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly
    ));
    assert!(!backend.description.capabilities.typed_async_launch);
    assert_eq!(
        backend.arm_generated_copy_coexistence_qualification_v1(),
        Err(Failure::AlreadyArmed)
    );
    assert_eq!(
        backend.take_generated_copy_coexistence_qualification_v1(),
        Err(Failure::MissingPublication)
    );
    assert_eq!(
        backend.arm_generated_copy_coexistence_qualification_v1(),
        Err(Failure::AlreadyArmed)
    );
    let mut busy = KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1();
    busy.generated_submissions.insert(1, 2);
    assert_eq!(
        busy.arm_generated_copy_coexistence_qualification_v1(),
        Err(Failure::Busy)
    );
    busy.generated_submissions.clear();
}

#[test]
fn qualification_hooks_do_not_accept_scripted_or_missing_native_receipts() {
    use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
    for (kind, failure) in [
        (
            KfdGeneratedCopyPublicationKindV1::GeneratedCompute,
            Failure::GeneratedSource,
        ),
        (
            KfdGeneratedCopyPublicationKindV1::DirectionalCopy,
            Failure::UnsupportedCopy,
        ),
    ] {
        let mut backend = KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1();
        backend
            .arm_generated_copy_coexistence_qualification_v1()
            .unwrap();
        let ((), allocations) = counted(|| backend.record_generated_copy_publication_v1(kind, 17));
        assert_eq!(allocations, 0);
        assert_eq!(
            backend.take_generated_copy_coexistence_qualification_v1(),
            Err(failure)
        );
        assert!(!backend.terminal);
        assert!(backend.generated_submissions.is_empty());
        assert!(backend.active_sdma.is_empty());
    }
}

#[test]
fn qualification_disabled_hooks_are_inert_and_terminal_evidence_is_unavailable() {
    use KfdGeneratedCopyCoexistenceFailureV1 as Failure;
    let mut backend = KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1();
    let ((), allocations) = counted(|| {
        backend.record_generated_copy_publication_v1(
            KfdGeneratedCopyPublicationKindV1::GeneratedCompute,
            7,
        );
        backend.reject_generated_copy_witness_v1(Failure::UnsupportedCopy);
    });
    assert_eq!(allocations, 0);
    assert!(backend.generated_copy_coexistence.is_none());
    backend
        .arm_generated_copy_coexistence_qualification_v1()
        .unwrap();
    let (compute, copy) = publications();
    let recorder = backend.generated_copy_coexistence.as_mut().unwrap();
    record(recorder, compute, None);
    record(recorder, copy, Some((compute, copy, 1)));
    backend.terminal = true;
    let refused = backend.take_generated_copy_coexistence_qualification_v1();
    // Only the mock terminal bit was injected; no native custody was created.
    // Restore it before asserting so ordinary fail-closed Drop remains intact.
    backend.terminal = false;
    assert_eq!(refused, Err(Failure::Terminal));
}

#[test]
fn qualification_publication_hooks_exclude_retry_poll_and_recycle() {
    let issue = include_str!("../issue.rs");
    let (publication, progress) = issue
        .split_once("pub(crate) fn progress_generated_submission_v1(")
        .unwrap();
    assert_eq!(
        publication
            .matches("self.record_generated_copy_publication_v1(")
            .count(),
        1
    );
    assert!(!progress.contains("record_generated_copy_publication_v1"));
    assert!(publication.contains(".published()"));
    let typed = include_str!("../typed_receipt.rs");
    let published = typed
        .split_once("fn published(&self) -> bool")
        .unwrap()
        .1
        .split_once("fn recycled")
        .unwrap()
        .0;
    assert!(published.contains("Self::Singleton(ReceiptV1::Published(_))"));
    assert!(published.contains("Self::Cohort3(ReceiptV1::Published(_))"));
    for excluded in [
        "Ready",
        "RetryReady",
        "Completed",
        "Recycled",
        "HandedToLower",
    ] {
        assert!(!published.contains(excluded));
    }
    assert!(
        publication
            .rfind("self.finish_generated_native_call_v1(result)?;")
            .unwrap()
            < publication
                .find("self.record_generated_copy_publication_v1(")
                .unwrap()
    );
    let copy = include_str!("../../sdma_publication.rs");
    assert_eq!(
        copy.matches("self.record_generated_copy_publication_v1(")
            .count(),
        1
    );
    assert_eq!(
        copy.matches("self.reject_generated_copy_witness_v1(")
            .count(),
        1
    );
    assert!(
        copy.find(".phase = ActiveSdmaPhaseV1::DirectionalPublished")
            .unwrap()
            < copy
                .find("self.record_generated_copy_publication_v1(")
                .unwrap()
    );
    assert!(
        !include_str!("../../sdma_observation.rs").contains("record_generated_copy_publication_v1")
    );
}
