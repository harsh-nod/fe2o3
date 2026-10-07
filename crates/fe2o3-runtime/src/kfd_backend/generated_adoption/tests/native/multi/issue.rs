//! Three real dispatches exercise exact-child readback, retirement and lane reuse.

use super::*;

fn assert_ready(
    backend: &mut KfdMultiDeviceRuntimeBackendV1,
    plan: &GeneratedShellPlanV1,
    id: u64,
    roster: &GeneratedHostRosterV1,
) {
    assert_eq!(backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        backend.wait_v1(id, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(
        backend.drain_v1(id, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    backend
        .flush_stream_v1(plan.binding.backend_stream)
        .unwrap();
    backend
        .progress_stream_quantum_v1(plan.binding.backend_stream)
        .unwrap();
    assert_eq!(backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
    assert!(backend.generated_submission_can_retire_v1(id));
    assert!(backend.generated_data_unpublished_v1(plan, Some(id)));
    assert!(!backend.generated_data_unpublished_v1(plan, None));
    assert!(
        backend
            .record_event_v1(plan.binding.backend_stream, id)
            .is_err()
    );
    assert!(backend.cancel_v1(id).is_err());
    assert!(backend.release_submission_v1(id).is_err());
    let mut destinations = destinations(roster);
    let before = destinations.clone();
    assert!(
        backend
            .read_generated_submission_v1(plan, id, roster, &mut destinations)
            .is_err()
    );
    assert_eq!(destinations, before);
    assert!(backend.submissions.is_empty() && backend.events.is_empty());
    assert!(!backend.terminal);
}

fn complete(backend: &mut KfdMultiDeviceRuntimeBackendV1, plan: &GeneratedShellPlanV1, id: u64) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !backend.advance_generated_issue_v1(plan, id).unwrap() {
        assert!(
            Instant::now() < deadline,
            "bounded native generated completion"
        );
        std::thread::yield_now();
    }
    assert!(!backend.generated_data_unpublished_v1(plan, Some(id)));
    for result in [
        backend.poll_v1(id),
        backend.wait_v1(id, Instant::now()),
        backend.drain_v1(id, Instant::now()),
    ] {
        assert!(matches!(result, Err(RuntimeBackendFailureV1::Quiescent(_))));
    }
}

fn read_full_roster(
    backend: &mut KfdMultiDeviceRuntimeBackendV1,
    plan: &GeneratedShellPlanV1,
    id: u64,
    roster: &GeneratedHostRosterV1,
) {
    let (left, right, _, mut output) = admit_gfx942_vecadd_qualification_v1()
        .unwrap()
        .host_buffers()
        .unwrap()
        .into_parts();
    output.extend_from_slice(&[0x7b; 16]);
    let expected = [left, right, output, vec![0x5a; 80]];
    let mut destinations = destinations(roster);
    let pointers: Vec<_> = destinations
        .iter()
        .map(|(_, bytes)| (bytes.as_ptr(), bytes.capacity()))
        .collect();
    let mut foreign = roster.clone();
    foreign.source_identity = Arc::new(());
    let before = destinations.clone();
    assert!(matches!(
        backend.read_generated_submission_v1(plan, id, &foreign, &mut destinations),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(destinations, before);
    backend
        .read_generated_submission_v1(plan, id, roster, &mut destinations)
        .unwrap();
    for (ordinal, ((_, bytes), expected)) in destinations.iter().zip(expected).enumerate() {
        assert_eq!(bytes, &expected, "complete DATA ordinal {ordinal}");
        assert_eq!((bytes.as_ptr(), bytes.capacity()), pointers[ordinal]);
    }
    assert!(!backend.terminal);
}

fn retire_issue(
    backend: &mut KfdMultiDeviceRuntimeBackendV1,
    plan: &GeneratedShellPlanV1,
    id: u64,
) {
    backend.retire_generated_data_v1(plan).unwrap();
    assert!(!backend.validate_generated_shell_disposal_v1(plan));
    backend.release_submission_v1(id).unwrap();
    dispose_retired_multi(backend, plan);
    assert!(matches!(
        backend.poll_v1(id),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
}

#[test]
#[ignore = "requires an idle MI300X pair, FE2O3_TEST_NATIVE_ISOLATED=1 and FE2O3_TEST_NATIVE_UNIQUE_IDS"]
fn generated_native_multi_issue_readback_retire_and_rebind() {
    assert_eq!(
        std::env::var("FE2O3_TEST_NATIVE_ISOLATED").as_deref(),
        Ok("1")
    );
    let raw = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_IDS").expect("two explicit device UIDs");
    let uids: Vec<_> = raw
        .split(',')
        .map(|uid| u64::from_str_radix(uid.strip_prefix("0x").unwrap_or(uid), 16).unwrap())
        .collect();
    let [first, second] = <[u64; 2]>::try_from(uids).expect("exactly two devices");
    assert_ne!(first, second);
    let children = [first, second]
        .map(|uid| KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(uid).unwrap());
    let mut backend = std::mem::ManuallyDrop::new(
        KfdMultiDeviceRuntimeBackendV1::from_backends(Vec::from(children)).unwrap(),
    );
    let streams = [first, second].map(|uid| backend.create_stream_v1(uid).unwrap());
    let (a, ra) = install_multi_with_roster(&mut backend, first, streams[0], true);
    let (b, rb) = install_multi_with_roster(&mut backend, second, streams[1], true);
    let ia = backend.prepare_generated_issue_v1(&a, &ra).unwrap();
    let ib = backend.prepare_generated_issue_v1(&b, &rb).unwrap();
    assert_ne!(ia, ib);
    assert_eq!(backend.generated_submissions.len(), 2);
    assert_eq!(backend.generated_allocations.len(), 8);
    assert_ready(&mut backend, &a, ia, &ra);
    assert_ready(&mut backend, &b, ib, &rb);
    let primary = backend.children[0].native_compute_lanes[0].unwrap();
    complete(&mut backend, &a, ia);
    complete(&mut backend, &b, ib);
    read_full_roster(&mut backend, &a, ia, &ra);
    read_full_roster(&mut backend, &b, ib, &rb);
    retire_issue(&mut backend, &a, ia);
    assert!(backend.generated_submission_can_retire_v1(ib));
    let (rebound, rr) = install_multi_with_roster(&mut backend, first, streams[0], true);
    assert_eq!(backend.children[0].native_compute_lanes[0], Some(primary));
    let ir = backend.prepare_generated_issue_v1(&rebound, &rr).unwrap();
    assert!(ir != ia && ir != ib);
    assert_ready(&mut backend, &rebound, ir, &rr);
    complete(&mut backend, &rebound, ir);
    read_full_roster(&mut backend, &rebound, ir, &rr);
    read_full_roster(&mut backend, &b, ib, &rb);
    retire_issue(&mut backend, &b, ib);
    retire_issue(&mut backend, &rebound, ir);
    assert!(backend.generated_submissions.is_empty());
    assert!(backend.generated_shells.is_empty() && backend.generated_allocations.is_empty());
    for stream in streams {
        backend.destroy_stream_v1(stream).unwrap();
    }
    backend.shutdown_native_v1().unwrap();
    assert_unpublished(&backend);
    assert!(backend.children.iter().all(|child| child.queue.is_none()));
    drop(std::mem::ManuallyDrop::into_inner(backend));
    eprintln!(
        "MULTI ISSUE PASS: two exact devices, 3 dispatches, full-roster readback, independent retirement, primary rebound, 12 DATA releases, shutdown complete"
    );
}
