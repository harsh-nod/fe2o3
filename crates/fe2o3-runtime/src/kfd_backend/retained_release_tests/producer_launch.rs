//! Native producer-first reconciliation under the unchanged two-launch R57 authority.

use super::*;
use crate::qualification_gfx942_r57_n3_v1::{
    GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1, GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1,
    GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1, GFX942_R57_N3_QUALIFICATION_KERNEL_V1,
    Gfx942R57N3QualificationArgumentsV1, admit_gfx942_r57_n3_qualification_v1,
};
use crate::{
    RuntimeAllocationIdV1, RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeMemoryRegionV1,
    RuntimePollV1,
};

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
    }
}

fn assert_persistent(performance: KfdRuntimeLaunchPerformanceV1) {
    assert_eq!(
        performance.data_path(),
        KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
    );
    assert_eq!(performance.user_data_materializations(), 0);
    assert!(!performance.persistent_control_reused());
}

fn native_producer_chain(published: bool) {
    let selector = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_ID").ok();
    let isolation = std::env::var("FE2O3_TEST_NATIVE_ISOLATED").ok();
    let uid = cold_allocation::cold_probe_device(selector.as_deref(), isolation.as_deref())
        .expect("explicit native identity and isolation acknowledgement");
    let admitted = admit_gfx942_r57_n3_qualification_v1().unwrap();
    let [a, b, c_initial, d_initial, expected_c, expected_d] =
        admitted.host_buffers().unwrap().into_parts();
    let (backend, authority) =
        KfdRuntimeBackendV1::open_gfx942_r57_n3_qualification_v1(uid).unwrap();
    let mut context = RuntimeContextV1::open_with_version_journal_v1(backend, 5, 4).unwrap();
    assert_eq!(context.devices().len(), 1);
    assert_eq!(context.devices()[0].target(), "gfx942:xnack-");
    assert!(context.backend().native_available && context.backend().scripted_sdma.is_none());
    let device = context.devices()[0].id();
    let producer_stream = context.create_stream(device).unwrap();
    let consumer_stream = context.create_stream(device).unwrap();
    let module = context.load_module(device, admitted.hsaco()).unwrap();
    let kernel = context
        .resolve_kernel::<Gfx942R57N3QualificationArgumentsV1>(
            module,
            GFX942_R57_N3_QUALIFICATION_KERNEL_V1,
        )
        .unwrap();
    let upload = context
        .allocate(
            device,
            RuntimeMemoryKindV1::HostVisible,
            GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
            GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1,
        )
        .unwrap();
    let allocations: [_; 4] = core::array::from_fn(|_| {
        context
            .allocate(
                device,
                RuntimeMemoryKindV1::DeviceLocal,
                GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1,
            )
            .unwrap()
    });
    let mut retained_upload = None;
    for (index, (allocation, bytes)) in allocations
        .into_iter()
        .zip([&a, &b, &c_initial, &d_initial])
        .enumerate()
    {
        context.write_allocation(upload, 0, bytes).unwrap();
        let mut copy = context
            .copy_async(
                producer_stream,
                region(upload, RuntimeAccessV1::Read),
                region(allocation, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        context.flush_stream(producer_stream).unwrap();
        assert_eq!(
            context.wait(&mut copy, Duration::from_secs(30)).unwrap(),
            RuntimePollV1::Succeeded
        );
        if index == 3 {
            // A real completed predecessor prevents submit-time publication.
            retained_upload = Some(copy);
        } else {
            context.release_submission(copy).unwrap();
        }
    }
    let mut producer = context
        .launch_producer_aware_v1(
            producer_stream,
            &kernel,
            &Gfx942R57N3QualificationArgumentsV1::new(
                allocations[0],
                allocations[1],
                allocations[2],
            )
            .unwrap(),
            GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
            &[],
        )
        .unwrap();
    assert_eq!(context.backend().pending_compute.len(), 1);
    assert!(context.backend().active.is_none());
    assert_eq!(authority.authorization_calls_v1(), 0);
    let (&producer_id, pending) = context.backend().pending_compute.iter().next().unwrap();
    let parent_allocations = pending.retained_allocations.clone();
    assert_eq!(parent_allocations.len(), 3);
    assert!(pending.ordered_predecessor.is_some());
    let deadline = Instant::now() + Duration::from_secs(30);
    if published {
        context.flush_stream(producer_stream).unwrap();
        loop {
            let active = context.backend().active.as_ref().unwrap();
            assert_eq!(active.id, producer_id);
            match active.execution.as_ref().unwrap() {
                ActiveComputeExecutionV1::ThreeBindingPersistent { .. } => break,
                ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { .. } => {
                    assert!(Instant::now() < deadline, "producer publication deadline");
                    assert_eq!(context.poll(&mut producer).unwrap(), RuntimePollV1::Pending);
                }
                _ => panic!("producer must use native three-binding persistent ownership"),
            }
        }
        for allocation in &parent_allocations {
            assert!(
                matches!(context.backend().allocations[allocation].sdma_storage,
                KfdRuntimeSdmaStorageV1::ComputeInFlight(owner) if owner == producer_id)
            );
        }
        assert_eq!(authority.authorization_calls_v1(), 1);
    }

    let event = context.record_event(&producer).unwrap();
    let mut consumer = context
        .launch_producer_aware_v1(
            consumer_stream,
            &kernel,
            &Gfx942R57N3QualificationArgumentsV1::new(
                allocations[2],
                allocations[1],
                allocations[3],
            )
            .unwrap(),
            GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
            &[event],
        )
        .unwrap();
    let (&consumer_id, pending) = context
        .backend()
        .pending_compute
        .iter()
        .find(|(id, _)| **id != producer_id)
        .unwrap();
    assert_eq!(&*pending.explicit_success_dependencies, &[producer_id]);
    assert_eq!(pending.explicit_dependency_cursor, 0);
    assert_eq!(context.version_journal_read_records_v1(), Some(4));
    assert_eq!(context.version_journal_writer_records_v1(), Some(2));
    context.release_event(event).unwrap();
    assert!(context.backend().events.is_empty());
    assert_eq!(
        context.query_submission(&producer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(
        context.query_submission(&consumer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(authority.authorization_calls_v1(), u64::from(published));

    if !published {
        assert!(context.backend().pending_compute.contains_key(&producer_id));
        assert!(context.backend().active.is_none());
        context.flush_stream(producer_stream).unwrap();
    }
    // Backend retirement restores inputs; logical reconciliation follows child completion.
    loop {
        match context
            .backend()
            .submissions
            .get(&producer_id)
            .map(|row| row.status)
        {
            Some(BackendPollV1::Succeeded) => break,
            Some(BackendPollV1::Failed { code }) => panic!("producer failed: {code}"),
            None | Some(BackendPollV1::Pending) => {}
        }
        assert!(
            Instant::now() < deadline,
            "consumer-driven producer completion deadline"
        );
        assert_eq!(context.poll(&mut consumer).unwrap(), RuntimePollV1::Pending);
    }
    assert_persistent(context.backend().last_launch_performance_v1().unwrap());
    assert_eq!(authority.authorization_calls_v1(), 1);
    assert_eq!(
        context.query_submission(&producer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert!(context.backend().pending_compute.contains_key(&consumer_id));
    context.flush_stream(consumer_stream).unwrap();
    let mut retained_child_success = false;
    loop {
        assert!(Instant::now() < deadline, "consumer completion deadline");
        let observation = context.poll(&mut consumer).unwrap();
        if context
            .backend()
            .submissions
            .get(&consumer_id)
            .is_some_and(|row| row.status == BackendPollV1::Succeeded)
            && context.query_submission(&producer).unwrap() == RuntimeCompletionStatusV1::Pending
        {
            assert_eq!(observation, RuntimePollV1::Pending);
            retained_child_success = true;
        }
        match observation {
            RuntimePollV1::Succeeded => break,
            RuntimePollV1::Pending => std::thread::yield_now(),
            status => panic!("consumer did not succeed: {status:?}"),
        }
    }
    assert!(
        retained_child_success,
        "child device success preceded logical reconciliation"
    );
    assert_persistent(context.backend().last_launch_performance_v1().unwrap());
    assert_eq!(authority.authorization_calls_v1(), 2);
    assert_eq!(
        context.query_submission(&producer).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        context.query_submission(&consumer).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(context.version_journal_read_records_v1(), Some(0));
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    assert!(context.backend().pending_compute.is_empty());
    assert!(context.backend().active.is_none());
    assert!(context.backend().allocation_custody.is_empty());
    assert!(!context.backend().any_compute_active_v1());
    assert!(context.backend().compute_module_retain_counts.is_empty());
    assert_eq!(context.backend().compute_completion_reservations, 0);
    assert_eq!(context.backend().sdma_completion_reservations, 0);
    assert!(
        context
            .backend()
            .compute_dependency_retain_counts
            .is_empty()
    );

    let mut digests = Vec::new();
    for (index, (allocation, expected)) in allocations
        .into_iter()
        .zip([&a, &b, &expected_c, &expected_d])
        .enumerate()
    {
        let mut observed = vec![0; GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1];
        context
            .read_allocation(allocation, 0, &mut observed)
            .unwrap();
        assert_eq!(observed.len(), expected.len());
        assert_eq!(
            observed
                .iter()
                .zip(expected)
                .position(|(left, right)| left != right),
            None,
            "complete-buffer mismatch in allocation {index}"
        );
        digests.push(
            Sha256::digest(&observed)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        );
    }
    context.release_submission(consumer).unwrap();
    context.release_submission(producer).unwrap();
    context
        .release_submission(retained_upload.unwrap())
        .unwrap();
    for allocation in allocations.into_iter().rev() {
        context.release_allocation(allocation).unwrap();
    }
    context.release_allocation(upload).unwrap();
    context.unload_module(module).unwrap();
    context.destroy_stream(consumer_stream).unwrap();
    context.destroy_stream(producer_stream).unwrap();
    assert!(context.cleanup().is_complete());
    let mut backend = context.shutdown().unwrap();
    backend.shutdown_native_v1().unwrap();
    assert!(backend.queue_retired && backend.queue.is_none() && !backend.terminal);
    drop(backend);
    println!(
        "native_producer_launch=complete published={published} authority_calls=2 launches=2 checked_bytes=1048576 peak_readers=4 terminal_readers=0 terminal_writers=0 public_event=released observation=consumer_first data_path=PersistentDeviceReused materializations=0 a_sha256={} b_sha256={} c_sha256={} d_sha256={} cleanup=complete",
        digests[0], digests[1], digests[2], digests[3]
    );
}

#[test]
#[ignore = "requires an explicitly selected isolated gfx942 GPU and external pre/postflight"]
fn native_runtime_producer_launch_queued_chain() {
    native_producer_chain(false);
}

#[test]
#[ignore = "requires an explicitly selected isolated gfx942 GPU and external pre/postflight"]
fn native_runtime_producer_launch_published_chain() {
    native_producer_chain(true);
}
