//! Public wrapper validation on a real multi-backend Context with mock children.
//! These tests do not manufacture native allocations or a certified native result.

use super::*;

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;

fn pending(context: &mut Context) -> RuntimeSubmissionV1<RuntimePeerCopyV1> {
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let stream = context.create_stream(devices[1]).unwrap();
    let source = context
        .allocate(devices[0], RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    let destination = context
        .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    context.write_allocation(source, 0, &[0x53; 64]).unwrap();
    context
        .write_allocation(destination, 0, &[0x17; 64])
        .unwrap();
    context
        .peer_copy(
            stream,
            RuntimeMemoryRegionV1 {
                allocation: source,
                byte_offset: 0,
                byte_len: 64,
                access: RuntimeAccessV1::Read,
            },
            RuntimeMemoryRegionV1 {
                allocation: destination,
                byte_offset: 0,
                byte_len: 64,
                access: RuntimeAccessV1::Write,
            },
            &[],
        )
        .unwrap()
}

#[test]
fn qualification_xgmi_context_rejects_nonnative_without_progress_or_journal_change() {
    for journal in [false, true] {
        let backend = KfdMultiDeviceRuntimeBackendV1::qualification_xgmi_test_backend_v1();
        let mut context = if journal {
            Context::open_with_version_journal_v1(backend, 16, 16).unwrap()
        } else {
            Context::open(backend).unwrap()
        };
        let mut submission = pending(&mut context);
        let event = context.record_event(&submission).unwrap();
        let usage = context.version_journal_usage_v1();
        for _ in 0..2 {
            assert!(matches!(
                context.reject_native_xgmi_host_preparation_once_for_qualification_v1(&submission),
                Err(RuntimeErrorV1::BackendRejected(_))
            ));
            assert!(
                !context
                    .native_xgmi_host_preparation_rejected_for_qualification_v1(&submission)
                    .unwrap()
            );
            assert_eq!(
                context.query_submission(&submission).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(context.version_journal_usage_v1(), usage);
            assert!(!context.terminal);
        }
        context.release_event(event).unwrap();
        assert_eq!(
            context.cancel(&mut submission).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        assert!(
            !context
                .native_xgmi_host_preparation_rejected_for_qualification_v1(&submission)
                .unwrap()
        );
        assert!(matches!(
            context.reject_native_xgmi_host_preparation_once_for_qualification_v1(&submission),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::Unsupported
            ))
        ));
        context.release_submission(submission).unwrap();
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn qualification_xgmi_context_authenticates_handle_and_reservation_before_backend() {
    let mut context = Context::qualification_xgmi_test_context_v1();
    let mut other = Context::qualification_xgmi_test_context_v1();
    let mut foreign = pending(&mut other);
    let token = context.reserve_graph_v1(1).unwrap();
    assert!(matches!(
        context.reject_native_xgmi_host_preparation_once_for_qualification_v1(&foreign),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert!(matches!(
        context.native_xgmi_host_preparation_rejected_for_qualification_v1(&foreign),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    context.close_graph_issue_v1(token).unwrap();
    context.release_graph_v1(token).unwrap();
    let mut submission = pending(&mut context);
    assert!(matches!(
        context.reject_native_xgmi_host_preparation_once_for_qualification_v1(&foreign),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownSubmission
        ))
    ));
    assert!(matches!(
        context.native_xgmi_host_preparation_rejected_for_qualification_v1(&foreign),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownSubmission
        ))
    ));
    let original = submission.backend_submission;
    submission.backend_submission += 1;
    assert!(matches!(
        context.reject_native_xgmi_host_preparation_once_for_qualification_v1(&submission),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownSubmission
        ))
    ));
    submission.backend_submission = original;
    assert_eq!(
        context.query_submission(&submission).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(
        other.query_submission(&foreign).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(
        context.cancel(&mut submission).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(
        other.cancel(&mut foreign).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    context.release_submission(submission).unwrap();
    other.release_submission(foreign).unwrap();
    assert!(context.cleanup().is_complete());
    assert!(other.cleanup().is_complete());
}

#[test]
fn qualification_xgmi_context_requires_live_stream_and_exact_operation_custody() {
    let mut context = Context::qualification_xgmi_test_context_v1();
    let submission = pending(&mut context);
    let stream = context.streams.remove(&submission.stream).unwrap();
    assert!(matches!(
        context.reject_native_xgmi_host_preparation_once_for_qualification_v1(&submission),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    context.streams.insert(submission.stream, stream);
    let mut context = core::mem::ManuallyDrop::new(context);
    // Contradictory retained-profile metadata must quarantine rather than reach
    // a backend hook. No native owners exist in this intentionally retained mock.
    context
        .submissions
        .get_mut(&submission.id)
        .unwrap()
        .segmented_peer_copy = true;
    assert!(
        context
            .reject_native_xgmi_host_preparation_once_for_qualification_v1(&submission)
            .is_err()
    );
    assert!(context.terminal);
    assert!(context.submissions.contains_key(&submission.id));
}
