use super::super::tests::pipelined_active_for_test_v1;
use super::*;

fn account(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        records,
    )
    .unwrap()
}

fn pipeline_bytes() -> u64 {
    let account = account(u64::MAX, 1);
    let pipeline = RuntimeComputePipelineV1::try_vacant(
        Gfx942FixedDispatchCapacityProfileV1::Qualification1024,
        Some(&account),
    )
    .unwrap();
    let bytes = account
        .usage()
        .used
        .get(ResourceKindV1::ControlResidentBytes);
    drop(pipeline);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    bytes
}

fn custody_bytes() -> u64 {
    host_metadata_table_payload_bytes_v1::<RuntimeAllocationCustodyOwnerV1>(1024).unwrap()
}

pub(in crate::kfd_backend) fn backend(account: ResourceCreditAccountV1) -> KfdRuntimeBackendV1 {
    let dispatch =
        RuntimeDispatchStateV1::try_new(RuntimeDispatchCapacityV1::qualification_1024(account))
            .unwrap();
    let admitted =
        crate::qualification_gfx942_vecadd_v1::admit_gfx942_vecadd_qualification_v1().unwrap();
    KfdRuntimeBackendV1::new_with_dispatch_state_v1(
        BackendDeviceDescriptionV1 {
            backend_device: 7,
            name: "scaled qualification test".to_owned(),
            target: "gfx942:xnack-".to_owned(),
            global_memory_bytes: 0,
            capabilities: kfd_capabilities_v1(),
        },
        None,
        KfdRuntimeLaunchGateV1::ExactGfx942Vecadd(admitted),
        StagingBudgetsV1 {
            max_allocation_bytes: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
            max_context_bytes: KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1,
        },
        dispatch,
    )
}

fn retain(backend: &mut KfdRuntimeBackendV1, allocation: u64, submission: u64) {
    let entries = backend
        .reserve_allocation_custody_v1(&[allocation])
        .unwrap();
    backend.retain_allocation_custody_v1(
        &[allocation],
        RuntimeAllocationCustodyOwnerV1 {
            submission,
            stream: 7,
            kind: RuntimeAllocationCustodyKindV1::Compute,
        },
        entries,
    );
}

fn rejected<T>(
    result: Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    kind: KfdRuntimeBackendErrorKindV1,
) {
    assert!(
        matches!(result, Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == kind)
    );
}

#[test]
fn scaled_startup_refunds_first_table_on_second_table_byte_or_record_failure() {
    for (bytes, records) in [(pipeline_bytes() * 2 - 1, 2), (pipeline_bytes() * 2, 1)] {
        let account = account(bytes, records);
        let before = account.usage();
        assert!(
            RuntimeDispatchStateV1::try_new(RuntimeDispatchCapacityV1::qualification_1024(
                account.clone()
            ))
            .is_err()
        );
        assert_eq!(account.usage(), before);
    }
}

#[test]
fn scaled_public_constructor_rejects_bad_capacity_before_opening_kfd() {
    for (bytes, records) in [(0, 2), (u64::MAX, 0), (pipeline_bytes(), 2)] {
        let result =
            KfdRuntimeBackendV1::open_gfx942_vecadd_scale_qualification_v1(1, bytes, records);
        assert!(
            matches!(result, Err(error) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
        );
    }
}

#[test]
fn scaled_capacity_is_forwarded_at_both_native_startup_sites() {
    // Source routing is not evidence of native construction or publication.
    for (source, method) in [
        (
            include_str!("../../kfd_backend.rs"),
            ".create_compute_aql_queue_with_backing_budgets_and_capacity_v1(",
        ),
        (
            include_str!("../compute_dispatch.rs"),
            ".create_compute_aql_queue_with_fixed_dispatch_and_capacity_v1(",
        ),
    ] {
        let call = source
            .split(method)
            .nth(1)
            .unwrap()
            .split(".map_err(")
            .next()
            .unwrap();
        assert!(call.contains("self.dispatch_capacity.native().clone(),"));
    }
}

#[test]
fn scaled_backend_has_two_accounted_1024_slot_lanes_and_preserves_debits_on_swaps() {
    let account = account(2 * pipeline_bytes(), 2);
    let mut backend = backend(account.clone());
    let usage = account.usage();
    assert_eq!(usage.retained_records, 2);
    assert_eq!(
        usage.used.get(ResourceKindV1::ControlResidentBytes),
        2 * pipeline_bytes()
    );
    for lane in 0..2 {
        backend.with_compute_lane_state_v1(lane, |backend| {
            backend.active = Some(pipelined_active_for_test_v1(1 + lane as u64 * 1024));
            for id in 2..=1024 {
                backend
                    .compute_pipeline
                    .insert_published(pipelined_active_for_test_v1(id + lane as u64 * 1024))
                    .unwrap();
            }
            assert_eq!(backend.compute_pipeline.len(), 1023);
            assert!(!backend.compute_pipeline.has_successor_capacity());
        });
        assert_eq!(account.usage(), usage);
    }
    for lane in 0..2 {
        backend.with_compute_lane_state_v1(lane, |backend| {
            assert_eq!(backend.active.take().unwrap().id, 1 + lane as u64 * 1024);
            for id in 2..=1024 {
                assert_eq!(
                    backend
                        .compute_pipeline
                        .take_commit_frontier()
                        .unwrap()
                        .1
                        .id,
                    id + lane as u64 * 1024
                );
            }
        });
    }
    assert_eq!(
        backend.scale_qualification_host_table_usage_v1(),
        Some(usage)
    );
    assert!(!backend.description.capabilities.atomics);
    assert!(!backend.description.capabilities.collectives);
    drop(backend);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn scaled_custody_accepts_1024_rejects_1025_and_refunds_only_after_disposal() {
    let account = account(2 * pipeline_bytes() + custody_bytes(), 3);
    let mut backend = backend(account.clone());
    let baseline = account.usage();
    retain(&mut backend, 11, 1);
    assert_eq!(backend.allocation_custody[&11].owners.capacity(), 1024);
    let full_debit = account.usage();
    assert_eq!(full_debit.retained_records, baseline.retained_records + 1);
    assert_eq!(
        full_debit.used.get(ResourceKindV1::ControlResidentBytes),
        2 * pipeline_bytes() + custody_bytes()
    );
    for id in 2..=1024 {
        retain(&mut backend, 11, id);
    }
    let custody = &backend.allocation_custody[&11];
    assert_eq!(custody.owners.len(), 1024);
    assert_eq!(custody.owners.capacity(), 1024);
    assert_eq!(custody.sole_stream, Some(7));
    assert_eq!(custody.owner_counts, [1024, 0]);
    let owners = custody.owners.clone();
    let usage = account.usage();
    assert_eq!(usage, full_debit);
    assert_eq!(usage.retained_records, baseline.retained_records + 1);
    assert_eq!(
        usage.used.get(ResourceKindV1::ControlResidentBytes),
        2 * pipeline_bytes() + custody_bytes()
    );
    rejected(
        backend.reserve_allocation_custody_v1(&[11]),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    assert_eq!(backend.allocation_custody[&11].owners, owners);
    assert_eq!(backend.allocation_custody[&11].owner_counts, [1024, 0]);
    assert_eq!(backend.allocation_custody[&11].sole_stream, Some(7));
    assert_eq!(account.usage(), usage);
    let custody = backend.allocation_custody.remove(&11).unwrap();
    assert_eq!(account.usage(), usage);
    backend.allocation_custody.insert(11, custody);
    for id in 1..1024 {
        backend.release_allocation_custody_v1(11, id);
    }
    assert_eq!(backend.allocation_custody[&11].owners.capacity(), 1024);
    assert_eq!(backend.allocation_custody[&11].owners.len(), 1);
    assert_eq!(account.usage(), full_debit);
    backend.release_allocation_custody_v1(11, 1024);
    assert!(backend.allocation_custody.is_empty());
    assert_eq!(account.usage(), baseline);
}

#[test]
fn scaled_custody_multi_allocation_preflight_refunds_byte_and_record_failures() {
    for (bytes, records) in [(custody_bytes(), 4), (custody_bytes() * 2, 3)] {
        let account = account(2 * pipeline_bytes() + bytes, records);
        let mut backend = backend(account.clone());
        let baseline = account.usage();
        rejected(
            backend.reserve_allocation_custody_v1(&[11, 12]),
            KfdRuntimeBackendErrorKindV1::Capacity,
        );
        assert!(backend.allocation_custody.is_empty());
        assert_eq!(account.usage(), baseline);
        let entries = backend.reserve_allocation_custody_v1(&[11, 11]).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(account.usage().retained_records, 3);
        drop(entries);
        assert_eq!(account.usage(), baseline);
    }
}

#[test]
fn scaled_custody_new_then_full_existing_rejection_preserves_roster_and_refunds() {
    let account = account(2 * pipeline_bytes() + 2 * custody_bytes(), 4);
    let mut backend = backend(account.clone());
    for id in 1..=1024 {
        retain(&mut backend, 12, id);
    }
    let usage = account.usage();
    let owners = backend.allocation_custody[&12].owners.clone();
    rejected(
        backend.reserve_allocation_custody_v1(&[11, 12]),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    assert_eq!(account.usage(), usage);
    assert!(!backend.allocation_custody.contains_key(&11));
    assert_eq!(backend.allocation_custody[&12].owners, owners);
    assert_eq!(backend.allocation_custody[&12].owner_counts, [1024, 0]);
    assert_eq!(backend.allocation_custody[&12].sole_stream, Some(7));
    assert_eq!(backend.allocation_custody[&12].owners.capacity(), 1024);
    for id in 1..=1024 {
        backend.release_allocation_custody_v1(12, id);
    }
    let baseline = account.usage();
    drop(backend.reserve_allocation_custody_v1(&[11, 12]).unwrap());
    assert_eq!(account.usage(), baseline);
    assert!(backend.allocation_custody.is_empty());
}

#[test]
fn scaled_custody_mixed_kinds_and_non_fifo_removal_preserve_counters() {
    let account = account(2 * pipeline_bytes() + custody_bytes(), 3);
    let mut backend = backend(account.clone());
    let baseline = account.usage();
    retain(&mut backend, 11, 1);
    for (submission, stream) in [(2, 8), (3, 7)] {
        let entries = backend.reserve_allocation_custody_v1(&[11]).unwrap();
        backend.retain_allocation_custody_v1(
            &[11],
            RuntimeAllocationCustodyOwnerV1 {
                submission,
                stream,
                kind: RuntimeAllocationCustodyKindV1::Sdma,
            },
            entries,
        );
    }
    assert_eq!(backend.allocation_custody[&11].sole_stream, None);
    backend.release_allocation_custody_v1(11, 2);
    assert_eq!(backend.allocation_custody[&11].sole_stream, Some(7));
    assert_eq!(backend.allocation_custody[&11].owner_counts, [1, 1]);
    backend.release_allocation_custody_v1(11, 3);
    assert_eq!(backend.allocation_custody[&11].owner_counts, [1, 0]);
    backend.release_allocation_custody_v1(11, 1);
    assert_eq!(account.usage(), baseline);
}

#[test]
fn scaled_unsupported_paths_reject_before_native_lookup_callbacks_or_mutation() {
    let account = account(2 * pipeline_bytes(), 2);
    let mut backend = backend(account.clone());
    backend.native_available = true;
    let before = account.usage();
    rejected(
        backend.allocate_with_outcome_v1(7, RuntimeMemoryKindV1::DeviceLocal, 4096, 4096),
        KfdRuntimeBackendErrorKindV1::Unsupported,
    );
    rejected(
        backend.with_retained_preparation_device_v1(7, |_| panic!("callback must not run")),
        KfdRuntimeBackendErrorKindV1::Unsupported,
    );
    rejected(
        backend.generated_lane_ready_v1(),
        KfdRuntimeBackendErrorKindV1::Unsupported,
    );
    let launch = BackendLaunchV1 {
        stream: 0,
        kernel: 0,
        explicit_kernarg: &[],
        bindings: &[],
        dependencies: &[],
        geometry: crate::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        },
        semantic_launch: BackendSemanticLaunchV1::Ordinary,
    };
    rejected(
        backend.prepare_launch(launch, true, false),
        KfdRuntimeBackendErrorKindV1::Unsupported,
    );
    let mut default = KfdRuntimeBackendV1::mock();
    rejected(
        default.prepare_launch(launch, true, false),
        KfdRuntimeBackendErrorKindV1::UnknownHandle,
    );
    assert!(!backend.terminal);
    assert_eq!(backend.next_handle, 1);
    assert_eq!(backend.staged_context_bytes, 0);
    assert!(backend.queue.is_none());
    assert!(backend.allocations.is_empty());
    assert_eq!(account.usage(), before);
    backend.native_available = false;
    let host = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 4096, 4096)
        .unwrap();
    backend.release_allocation_v1(host).unwrap();
    assert_eq!(account.usage(), before);
    assert!(default.scale_qualification_host_table_usage_v1().is_none());
}
