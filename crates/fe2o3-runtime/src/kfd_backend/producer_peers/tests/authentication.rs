use super::*;

#[test]
fn cooperative_launch_parent_requires_exact_identity_destination_and_success() {
    for outcome in 0..3 {
        let mut backend = backend();
        let stream = backend.create_stream_v1(8).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let copy = backend
            .peer_copy_v1(
                stream,
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 8,
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 8,
                },
                &[],
            )
            .unwrap();
        let event = backend.record_event_v1(stream, copy).unwrap();
        let dependency = BackendLaunchProducerV1 {
            event,
            producer_submission: copy,
        };
        assert!(
            matches!(backend.exact_launch_dependency_for_child(dependency, 1), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        match outcome {
            0 => backend.flush_stream_v1(stream).unwrap(),
            1 => {
                assert_eq!(
                    backend.cancel_v1(copy).unwrap(),
                    crate::BackendCancellationV1::Cancelled
                );
            }
            _ => {
                backend.finish_cooperative_copy(copy, CooperativeCopyPhaseV1::Failed);
            }
        }
        assert!(
            matches!(backend.exact_launch_dependency_for_child(BackendLaunchProducerV1 { producer_submission: copy + 1, ..dependency }, 1), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch)
        );
        assert!(
            matches!(backend.exact_launch_dependency_for_child(dependency, 0), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::WrongDevice)
        );
        // Event branding alone must not hide the copy's actual destination.
        backend.events.insert(
            event,
            RoutedEventV1::CooperativeCopy {
                submission: copy,
                child: 0,
            },
        );
        assert!(
            matches!(backend.exact_launch_dependency_for_child(dependency, 0), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::WrongDevice)
        );
        backend.events.insert(
            event,
            RoutedEventV1::CooperativeCopy {
                submission: copy,
                child: 1,
            },
        );
        if outcome == 0 {
            assert_eq!(
                backend
                    .exact_launch_dependency_for_child(dependency, 1)
                    .unwrap(),
                None
            );
        } else {
            assert!(
                matches!(backend.exact_launch_dependency_for_child(dependency, 1), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch)
            );
        }
        assert!(backend.peer_launch_retains.is_empty());
        backend.release_event_v1(event).unwrap();
        backend.release_submission_v1(copy).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }
}
