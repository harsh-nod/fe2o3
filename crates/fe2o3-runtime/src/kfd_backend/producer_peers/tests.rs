use super::*;
mod authentication;

fn backend() -> KfdMultiDeviceRuntimeBackendV1 {
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    KfdMultiDeviceRuntimeBackendV1::from_backends(vec![KfdRuntimeBackendV1::mock(), right]).unwrap()
}

#[test]
fn peer_launch_custody_rolls_back_only_definite_admission_failures() {
    for mode in 0..4 {
        let mut backend = backend();
        backend.peer_launch_retains.prepare(&[40]).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend.with_peer_launch_custody_v1(41, vec![40], |backend| {
                assert!(backend.peer_launch_retains.retains(40));
                let error = KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Native,
                    "exact diagnostic",
                );
                match mode {
                    0 => Err(RuntimeBackendFailureV1::Rejected(error)),
                    1 => Err(RuntimeBackendFailureV1::Quiescent(error)),
                    2 => Err(RuntimeBackendFailureV1::Terminal(error)),
                    _ => std::panic::panic_any(1234_u32),
                }
            })
        }));
        if mode == 3 {
            assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 1234);
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(backend.terminal, mode >= 2);
        assert_eq!(backend.peer_launch_retains.retains(40), mode >= 2);
        // Scripted bookkeeping only: no native resources or uncertain owners.
        backend.terminal = false;
        backend.peer_launch_retains.release(41);
        backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn peer_launch_observation_retires_once_only_after_exact_quiescence() {
    for mode in 0..6 {
        let mut backend = backend();
        for consumer in [41, 42] {
            backend.peer_launch_retains.prepare(&[40]).unwrap();
            backend
                .with_peer_launch_custody_v1(consumer, vec![40], |_| Ok(consumer))
                .unwrap();
        }
        let error = KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Native,
            "exact observation",
        );
        let result = match mode {
            0 => Ok(BackendPollV1::Pending),
            1 => Ok(BackendPollV1::Succeeded),
            2 => Ok(BackendPollV1::Failed { code: -1 }),
            3 => Err(RuntimeBackendFailureV1::Rejected(error)),
            4 => Err(RuntimeBackendFailureV1::Quiescent(error)),
            _ => Err(RuntimeBackendFailureV1::Terminal(error)),
        };
        let _ = backend
            .observe_peer_launch_result_v1(41, result, |status| *status != BackendPollV1::Pending);
        let retired = matches!(mode, 1 | 2);
        assert_eq!(
            backend.peer_launch_retains.producers[&40],
            if retired { 1 } else { 2 }
        );
        backend.peer_launch_retains.release(41);
        backend.peer_launch_retains.release(41);
        assert_eq!(backend.peer_launch_retains.producers[&40], 1);
        backend.peer_launch_retains.release(42);
        assert!(backend.peer_launch_retains.is_empty());
        backend.terminal = false;
        backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn peer_launch_retain_overflow_rejects_before_acquisition() {
    let mut retains = PeerLaunchRetainsV1::default();
    retains.producers.insert(40, usize::MAX);
    assert!(
        matches!(retains.prepare(&[40]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    assert!(retains.consumers.is_empty());
    assert_eq!(retains.producers[&40], usize::MAX);
}

#[test]
fn flush_retires_only_the_matching_conclusive_native_consumers() {
    let mut backend = backend();
    let stream = backend.create_stream_v1(7).unwrap();
    let other = backend.create_stream_v1(7).unwrap();
    let local = backend.streams[&stream].local;
    for (id, owner, status) in [
        (41, stream, BackendPollV1::Succeeded),
        (42, other, BackendPollV1::Succeeded),
        (43, stream, BackendPollV1::Pending),
    ] {
        backend.peer_launch_retains.prepare(&[40]).unwrap();
        backend
            .with_peer_launch_custody_v1(id, vec![40], |_| Ok(id))
            .unwrap();
        backend.children[0].submissions.insert(
            id,
            SubmissionRecordV1 {
                stream: local,
                status,
                profile_dispatch_published: false,
            },
        );
        backend.submissions.insert(
            id,
            RoutedSubmissionV1::Native {
                route: RoutedHandleV1 {
                    child: 0,
                    local: id,
                },
                stream: owner,
            },
        );
    }
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(backend.peer_launch_retains.producers[&40], 2);
    assert!(!backend.peer_launch_retains.consumers.contains_key(&41));
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(backend.peer_launch_retains.producers[&40], 2);
    for id in [41, 42, 43] {
        backend.peer_launch_retains.release(id);
        backend.submissions.remove(&id);
        backend.children[0].submissions.remove(&id);
    }
    backend.destroy_stream_v1(stream).unwrap();
    backend.destroy_stream_v1(other).unwrap();
    backend.shutdown_native_v1().unwrap();
}
