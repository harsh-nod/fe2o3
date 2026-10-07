use super::*;

#[test]
fn native_subrange_pending_readback_preserves_device_and_host_guards_after_event_release() {
    for journal in [false, true] {
        for published in [false, true] {
            let mut f = ReadbackFixture::with_readback_steps(
                readback_range_steps(22, 9, 17),
                journal,
                None,
                false,
            );
            let mut source = region(f.source, RuntimeAccessV1::Read);
            source.byte_offset = 5;
            source.byte_len = 31;
            let mut destination = region(f.destination, RuntimeAccessV1::Write);
            destination.byte_offset = 17;
            destination.byte_len = 31;
            let mut peer = f
                .context
                .peer_copy(f.peer_stream, source, destination, &[])
                .unwrap();
            let peer_id = f.context.backend_submission_for_test_v1(&peer).unwrap();
            let endpoints = [f.copy(peer_id).source, f.copy(peer_id).destination];
            let identities = endpoints.map(|endpoint| {
                let KfdRuntimeSdmaStorageV1::Device(owner) =
                    &f.context.backend().children[endpoint.child].allocations[&endpoint.local]
                        .sdma_storage
                else {
                    unreachable!()
                };
                owner.scripted_owner_id()
            });
            let event = f.context.record_event(&peer).unwrap();
            assert!(f.copy(peer_id).compute_xgmi.is_some());
            assert!(f.copy(peer_id).staging.is_empty());
            if published {
                f.context.flush_stream(f.peer_stream).unwrap();
                assert_eq!(
                    f.copy(peer_id).compute_xgmi.as_ref().unwrap().phase,
                    Phase::Published
                );
            }
            let before = f.snapshot();
            let mut read = region(f.destination, RuntimeAccessV1::Read);
            read.byte_offset = 16;
            read.byte_len = 17;
            let mut write = region(f.host, RuntimeAccessV1::Write);
            write.byte_offset = 9;
            write.byte_len = 17;
            let count = f.context.backend().submissions.len();
            assert!(
                f.context
                    .copy_async(f.readback_stream, read, write, &[event])
                    .is_err()
            );
            assert_eq!(f.context.backend().submissions.len(), count);
            assert_eq!(f.snapshot(), before);
            read.byte_offset = 22;
            let mut readback = f
                .context
                .copy_async(f.readback_stream, read, write, &[event])
                .unwrap();
            let readback_id = f.context.backend_submission_for_test_v1(&readback).unwrap();
            f.context.release_event(event).unwrap();
            assert_eq!(
                f.context.poll(&mut readback).unwrap(),
                RuntimePollV1::Pending
            );
            assert_eq!(
                f.context.wait(&mut readback, Duration::ZERO).unwrap(),
                RuntimePollV1::Pending
            );
            assert!(f.context.drain(&mut readback, Instant::now()).is_err());
            assert_eq!(f.snapshot(), before);
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut status = RuntimePollV1::Pending;
            for _ in 0..4 {
                status = f.context.drain(&mut readback, deadline).unwrap();
                if status == RuntimePollV1::Succeeded {
                    break;
                }
                assert_eq!(status, RuntimePollV1::Pending);
                // Backend restoration can precede the exact parent's logical observation.
                assert_eq!(f.copy(readback_id).status(), BackendPollV1::Succeeded);
                assert_eq!(f.copy(peer_id).status(), BackendPollV1::Succeeded);
            }
            assert_eq!(status, RuntimePollV1::Succeeded);
            assert_eq!(f.context.poll(&mut peer).unwrap(), RuntimePollV1::Succeeded);
            for (index, endpoint) in endpoints.into_iter().enumerate() {
                let KfdRuntimeSdmaStorageV1::Device(owner) =
                    &f.context.backend().children[endpoint.child].allocations[&endpoint.local]
                        .sdma_storage
                else {
                    unreachable!()
                };
                let mut expected = [if index == 0 { 0x53 } else { 0x17 }; BYTES];
                if index == 1 {
                    expected[17..48].fill(0x53);
                }
                assert_eq!(owner.scripted_owner_id(), identities[index]);
                assert_eq!(owner.scripted_bytes().unwrap(), expected);
            }
            let mut output = [0; BYTES];
            f.context.read_allocation(f.host, 0, &mut output).unwrap();
            let mut expected = [0x29; BYTES];
            expected[9..26].fill(0x53);
            assert_eq!(output, expected);
            assert_eq!(f.context.backend().completed_compute_xgmi_copies_v1(), 0);
            f.context.release_submission(readback).unwrap();
            f.context.release_submission(peer).unwrap();
            f.clean();
        }
    }
}
