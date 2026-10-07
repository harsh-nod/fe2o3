//! Scripted runtime outcome tests, not native host-allocation or DMA evidence.

use super::*;
use crate::{RuntimePeerCopySegmentV1, RuntimePeerCopySegmentsBackendV1};

fn root_mut(f: &mut Fixture, submission: u64) -> &mut Root {
    let RoutedSubmissionV1::CooperativeCopy(copy) =
        f.backend.submissions.get_mut(&submission).unwrap()
    else {
        panic!("cooperative root")
    };
    copy.compute_xgmi.as_deref_mut().unwrap()
}

fn submit(f: &mut Fixture, segmented: bool, dependencies: &[u64]) -> u64 {
    if segmented {
        f.backend
            .peer_copy_segments_v1(
                f.stream,
                f.source,
                f.destination,
                &[
                    RuntimePeerCopySegmentV1 {
                        source_offset: 1,
                        destination_offset: 3,
                        byte_len: 17,
                    },
                    RuntimePeerCopySegmentV1 {
                        source_offset: 9,
                        destination_offset: 11,
                        byte_len: 13,
                    },
                ],
                dependencies,
            )
            .unwrap()
    } else {
        f.submit(dependencies)
    }
}

fn finish(f: &mut Fixture, submission: u64) -> BackendPollV1 {
    for _ in 0..16 {
        let status = f.backend.progress_cooperative_copy(submission).unwrap();
        if status != BackendPollV1::Pending {
            return status;
        }
    }
    panic!("bounded scripted completion");
}

#[cfg(feature = "hardware-qualification")]
#[test]
fn qualification_denial_rejects_scripted_missing_and_wrong_submissions_without_progress() {
    for segmented in [false, true] {
        let mut f = Fixture::new(None, false);
        let copy = submit(&mut f, segmented, &[]);
        let (producer, event, _) = f.producer(BackendPollV1::Succeeded);
        for id in [copy, producer, u64::MAX] {
            assert!(matches!(
                f.backend
                    .reject_native_xgmi_host_preparation_once_for_qualification_v1(id),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ));
            assert!(!f.backend.terminal);
        }
        assert!(
            !f.backend
                .native_xgmi_host_preparation_rejected_for_qualification_v1(copy)
                .unwrap()
        );
        assert!(
            !f.backend
                .native_xgmi_host_preparation_rejected_for_qualification_v1(producer)
                .unwrap()
        );
        assert!(matches!(
            f.backend
                .native_xgmi_host_preparation_rejected_for_qualification_v1(u64::MAX),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(
            f.root(copy).qualification_host_preparation,
            QualificationHostPreparationV1::Unrequested
        );
        assert!(f.root(copy).trace.is_empty());
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
        f.backend.release_event_v1(event).unwrap();
        f.backend.release_submission_v1(producer).unwrap();
        f.backend.cancel_v1(copy).unwrap();
        f.clean();
    }
}

#[cfg(feature = "hardware-qualification")]
#[test]
fn qualification_denial_cannot_certify_scripted_rejection_or_unused_cancel_request() {
    for cancel in [false, true] {
        let mut f = Fixture::new(None, false);
        let copy = f.submit(&[]);
        // Explicit private-state test only: an armed marker on the scripted
        // route must never take the native entrypoint or certify its outcome.
        root_mut(&mut f, copy).qualification_host_preparation =
            QualificationHostPreparationV1::Armed;
        root_mut(&mut f, copy).reject_host_preparation = true;
        if cancel {
            assert_eq!(
                f.backend.cancel_v1(copy).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            assert!(f.root(copy).no_effect_error.is_none());
            assert!(f.root(copy).trace.is_empty());
        } else {
            assert!(matches!(finish(&mut f, copy), BackendPollV1::Failed { .. }));
            assert!(f.root(copy).no_effect_error.is_some());
            assert_eq!(f.root(copy).trace, [Stage::Create]);
        }
        assert_eq!(
            f.root(copy).qualification_host_preparation,
            QualificationHostPreparationV1::Armed
        );
        assert!(
            !f.backend
                .native_xgmi_host_preparation_rejected_for_qualification_v1(copy)
                .unwrap()
        );
        assert!(matches!(
            f.backend
                .reject_native_xgmi_host_preparation_once_for_qualification_v1(copy),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(f.root(copy).is_quiescent());
        assert!(!f.backend.terminal);
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert_eq!(f.owner(true).scripted_bytes().unwrap(), &[0x53; BYTES]);
        assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x17; BYTES]);
        f.clean();
    }
}

#[test]
fn no_effect_creation_restores_metadata_refunds_retains_and_allows_fresh_retry() {
    for segmented in [false, true] {
        let mut f = Fixture::new(None, false);
        let original_owners = [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id(),
        ];
        for source in [false, true] {
            let bytes: Arc<[u8]> = Arc::from(f.owner(source).scripted_bytes().unwrap());
            let record = f.record_mut(source);
            record.bytes = Arc::clone(&bytes);
            record.last_full_host_write = Some((bytes, record.content_sha256.unwrap()));
            record.sdma_shadow_dirty = false;
        }
        let source_bytes = Arc::clone(&f.record(true).bytes);
        let destination_bytes = Arc::clone(&f.record(false).bytes);
        let source_digest = f.record(true).content_sha256;
        let destination_digest = f.record(false).content_sha256;
        let source_write = f.record(true).last_full_host_write.clone();
        let destination_write = f.record(false).last_full_host_write.clone();
        let (producer, event, _) = f.producer(BackendPollV1::Succeeded);
        let copy = submit(&mut f, segmented, &[event]);
        root_mut(&mut f, copy).reject_host_preparation = true;
        f.backend.release_event_v1(event).unwrap();
        assert!(
            f.backend
                .cooperative_dependency_retain_counts
                .contains_key(&producer)
        );
        assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
        assert!(f.root(copy).trace.is_empty());
        assert!(matches!(finish(&mut f, copy), BackendPollV1::Failed { .. }));
        assert_eq!(f.root(copy).trace, [Stage::Create]);
        assert!(matches!(
            f.root(copy).no_effect_error,
            Some(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                "scripted host preparation rejection"
            ))
        ));
        assert!(f.root(copy).is_quiescent());
        assert_eq!(f.root(copy).phase, Phase::Retired);
        assert_eq!(
            [
                f.owner(true).scripted_owner_id(),
                f.owner(false).scripted_owner_id()
            ],
            original_owners
        );
        assert!(Arc::ptr_eq(&f.record(true).bytes, &source_bytes));
        assert!(Arc::ptr_eq(&f.record(false).bytes, &destination_bytes));
        assert_eq!(f.owner(true).scripted_bytes().unwrap(), &*source_bytes);
        assert_eq!(
            f.owner(false).scripted_bytes().unwrap(),
            &*destination_bytes
        );
        assert_eq!(f.record(true).content_sha256, source_digest);
        assert_eq!(f.record(false).content_sha256, destination_digest);
        assert_eq!(f.record(true).last_full_host_write, source_write);
        assert_eq!(f.record(false).last_full_host_write, destination_write);
        for source in [false, true] {
            assert!(f.record(source).sdma_initialized);
            assert!(!f.record(source).sdma_shadow_dirty);
            assert!(f.record(source).native_dirty.is_empty());
        }
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert!(f.backend.cooperative_dependency_retain_counts.is_empty());
        assert!(!f.backend.terminal);
        assert!(f.backend.children.iter().all(|child| !child.terminal));
        assert_eq!(f.backend.cooperative_staging_bytes, 0);
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        f.backend.assert_cooperative_indexes_consistent();
        f.backend.release_submission_v1(copy).unwrap();
        f.backend.release_submission_v1(producer).unwrap();

        // Failed stream ordering is not reset. Retry is a genuinely new result
        // on a fresh stream after all old allocation/dependency retains refund.
        f.stream = f.backend.create_stream_v1(8).unwrap();
        let retry = f.submit(&[]);
        assert_eq!(finish(&mut f, retry), BackendPollV1::Succeeded);
        assert_eq!(f.owner(false).scripted_bytes().unwrap(), &*source_bytes);
        f.clean();
    }
}

#[test]
fn no_effect_creation_does_not_poison_published_disjoint_pair_or_quantum() {
    let mut f = Fixture::configured(None, false, 0, 4);
    let allocation = |child| {
        *f.backend
            .allocations
            .iter()
            .find(|(_, route)| route.child == child)
            .unwrap()
            .0
    };
    let other_source = BackendMemoryRegionV1 {
        allocation: allocation(2),
        ..f.source
    };
    let other_destination = BackendMemoryRegionV1 {
        allocation: allocation(3),
        ..f.destination
    };
    let other_stream = f.backend.create_stream_v1(10).unwrap();
    let other = f
        .backend
        .peer_copy_v1(other_stream, other_source, other_destination, &[])
        .unwrap();
    for _ in 0..4 {
        if f.root(other).phase == Phase::Published {
            break;
        }
        f.backend.progress_cooperative_copy(other).unwrap();
    }
    assert_eq!(f.root(other).phase, Phase::Published);
    let other_trace = f.root(other).trace.clone();
    let failed = f.submit(&[]);
    root_mut(&mut f, failed).reject_host_preparation = true;
    for _ in 0..4 {
        let outcome = f.backend.progress_stream_v1(f.stream);
        assert!(outcome.is_ok() || matches!(outcome, Err(RuntimeBackendFailureV1::Quiescent(_))));
        assert!(f.backend.cooperative_progress_quantum.is_none());
        if f.copy(failed).phase == CooperativeCopyPhaseV1::Failed {
            break;
        }
    }
    assert_eq!(f.copy(failed).phase, CooperativeCopyPhaseV1::Failed);
    assert_eq!(f.root(other).trace, other_trace);
    assert_eq!(
        &f.backend.compute_xgmi_children[2..],
        &[Some(other), Some(other)]
    );
    assert!(!f.backend.terminal);
    assert_eq!(finish(&mut f, other), BackendPollV1::Succeeded);
    let route = f.backend.allocations[&other_destination.allocation];
    let KfdRuntimeSdmaStorageV1::Device(owner) =
        &f.backend.children[route.child].allocations[&route.local].sdma_storage
    else {
        panic!("restored disjoint destination")
    };
    assert_eq!(owner.scripted_bytes().unwrap(), &[0x53; BYTES]);
    f.clean();
}

#[test]
fn cancellation_before_creation_never_consumes_the_host_rejection_or_changes_bytes() {
    for metadata_step in [false, true] {
        let mut f = Fixture::new(None, false);
        let copy = f.submit(&[]);
        root_mut(&mut f, copy).reject_host_preparation = true;
        if metadata_step {
            assert_eq!(
                f.backend.progress_cooperative_copy(copy).unwrap(),
                BackendPollV1::Pending
            );
            assert!(f.root(copy).trace.is_empty());
        }
        assert_eq!(
            f.backend.cancel_v1(copy).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert!(f.root(copy).reject_host_preparation);
        assert!(f.root(copy).no_effect_error.is_none());
        assert!(f.root(copy).trace.is_empty());
        assert!(f.root(copy).is_quiescent());
        assert_eq!(f.owner(true).scripted_bytes().unwrap(), &[0x53; BYTES]);
        assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x17; BYTES]);
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        f.clean();
    }
}

#[test]
fn no_effect_recovery_validates_both_outputs_before_restoring_either() {
    for missing in 0..2 {
        let mut f = Fixture::new(None, false);
        let copy = f.submit(&[]);
        root_mut(&mut f, copy).reject_host_preparation = true;
        let shell = root_mut(&mut f, copy).shells[missing].take();
        let mut terminal = false;
        for _ in 0..4 {
            if matches!(
                f.backend.progress_cooperative_copy(copy),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ) {
                terminal = true;
                break;
            }
        }
        assert!(terminal && f.backend.terminal);
        assert!(f.root(copy).no_effect_error.is_some());
        assert!(!f.root(copy).is_quiescent());
        assert!(f.root(copy).scripted_owners.iter().all(Option::is_some));
        assert_eq!(f.backend.compute_xgmi_children, [Some(copy), Some(copy)]);
        for source in [false, true] {
            assert!(matches!(f.record(source).sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(id)) if id == copy));
        }
        drop(shell);
        // Deliberately retain the terminal fixture, exactly as existing native
        // fault tests do; no success or cleanup is fabricated after corruption.
    }
}

#[test]
fn unclassified_creation_and_postpublication_faults_keep_fail_stop_custody() {
    for stage in [Stage::Create, Stage::Copy, Stage::Poll, Stage::Retire] {
        for unwind in [false, true] {
            let mut f = Fixture::new(Some(stage), unwind);
            let copy = f.submit(&[]);
            let result = catch_unwind(AssertUnwindSafe(|| {
                for _ in 0..8 {
                    f.backend.progress_cooperative_copy(copy)?;
                }
                Ok::<(), Failure>(())
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(RuntimeBackendFailureV1::Terminal(_))
                ));
            }
            assert!(f.backend.terminal);
            assert!(f.root(copy).no_effect_error.is_none());
            assert!(!f.root(copy).is_quiescent());
            assert!(f.root(copy).scripted_owners.iter().all(Option::is_some));
            assert_eq!(f.backend.compute_xgmi_children, [Some(copy), Some(copy)]);
            assert_eq!(f.copy(copy).phase, CooperativeCopyPhaseV1::Read);
        }
    }
}
