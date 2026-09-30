use super::super::tests::pipelined_active_for_test_v1;
use super::*;

#[path = "tests/preallocation.rs"]
mod preallocation;

#[path = "tests/repeat_profile.rs"]
mod repeat_profile;

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
fn scaled_exact_vecadd_output_metadata_denial_settles_and_refunds_without_native_work() {
    use crate::qualification_gfx942_vecadd_v1::{
        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1, GFX942_VECADD_QUALIFICATION_SIGNATURE_V1,
        admit_gfx942_vecadd_qualification_v1, gfx942_vecadd_qualification_bindings_v1,
        gfx942_vecadd_qualification_explicit_kernarg_v1,
    };

    for truthful_completed_output in [false, true] {
        let account = account(64 * 1024 * 1024, 32);
        let mut backend = backend(account.clone());
        let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
        let buffers = admitted.host_buffers().unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let module = backend.load_module_v1(7, admitted.hsaco()).unwrap();
        let kernel = backend
            .resolve_kernel_v1(
                module,
                admitted.kernel_name(),
                GFX942_VECADD_QUALIFICATION_SIGNATURE_V1,
            )
            .unwrap();
        let allocations = [buffers.left(), buffers.right(), buffers.output()].map(|bytes| {
            let allocation = backend
                .allocate_v1(
                    7,
                    RuntimeMemoryKindV1::HostVisible,
                    GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                    GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
                )
                .unwrap();
            backend.write_allocation_v1(allocation, 0, bytes).unwrap();
            allocation
        });
        let launch = OwnedComputeLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: gfx942_vecadd_qualification_explicit_kernarg_v1().into(),
            bindings: gfx942_vecadd_qualification_bindings_v1(allocations)
                .unwrap()
                .into(),
            geometry: admitted.geometry(),
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        };
        for reuse_bound_recipe in [false, true] {
            backend
                .prepare_launch(launch.borrowed(), false, reuse_bound_recipe)
                .unwrap();
        }
        if truthful_completed_output {
            backend
                .write_allocation_v1(allocations[2], 0, buffers.expected_output())
                .unwrap();
        } else {
            // Model only the host metadata invalidation from logical writeback.
            // No native completion, queue, DATA lease or receipt is fabricated.
            backend
                .allocations
                .get_mut(&allocations[2])
                .unwrap()
                .content_sha256 = None;
        }
        let expected_detail = "direct KFD launch authority denied the exact invocation";
        for reuse_bound_recipe in [false, true] {
            assert!(matches!(
                backend.prepare_launch(launch.borrowed(), false, reuse_bound_recipe),
                Err(RuntimeBackendFailureV1::Rejected(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
                        && error.detail() == expected_detail
            ));
        }
        // Match the existing CPU three-binding fixture's region admission;
        // storage remains Synthetic and no native backing is claimed.
        for allocation in allocations {
            let record = backend.allocations.get_mut(&allocation).unwrap();
            assert!(matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::Synthetic
            ));
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        let before = account.usage();
        reset_unpublished_compute_failure_for_test_v1();
        // This CPU-only switch permits host admission. The exact gate must
        // reject before publication can obtain any native resource.
        backend.native_available = true;
        let first = backend.submit_v1(launch.borrowed()).unwrap();
        let second = backend.submit_v1(launch.borrowed()).unwrap();
        for submission in [first, second] {
            assert_eq!(
                backend.poll_v1(submission).unwrap(),
                BackendPollV1::Failed { code: -1 }
            );
            assert!(!backend.submissions[&submission].profile_dispatch_published);
        }
        let diagnostic = take_unpublished_compute_failure_for_test_v1().unwrap();
        assert_eq!((diagnostic.submission, diagnostic.stream), (first, stream));
        assert_eq!(diagnostic.stage, "preparation-or-publication");
        assert_eq!(diagnostic.kind, KfdRuntimeBackendErrorKindV1::Unsupported);
        assert_eq!(diagnostic.detail(), expected_detail);
        assert!(!diagnostic.quiescent && !diagnostic.detail_truncated);
        assert!(take_unpublished_compute_failure_for_test_v1().is_none());
        assert!(backend.pending_compute.is_empty() && backend.pending_compute_streams.is_empty());
        assert!(
            backend.allocation_custody.is_empty()
                && backend.compute_module_retain_counts.is_empty()
        );
        assert!(
            backend.compute_dependency_retain_counts.is_empty()
                && backend.stream_compute_lanes.is_empty()
        );
        assert_eq!(backend.compute_completion_reservations, 0);
        assert_eq!(account.usage(), before);
        assert!(
            !backend.terminal && backend.active.is_none() && backend.compute_pipeline.is_empty()
        );
        assert!(
            backend.queue.is_none()
                && backend.admitted_device.is_none()
                && backend.terminal_memory.is_none()
        );
        assert!(backend.native_compute_lanes.iter().all(Option::is_none));
        for submission in [second, first] {
            backend.release_submission_v1(submission).unwrap();
        }
        assert!(backend.submissions.is_empty() && backend.stream_submission_tails.is_empty());
        backend.native_available = false;
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        backend.unload_module_v1(module).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
        drop(backend);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(account.usage().retained_records, 0);
    }
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

fn shared_host_payload_recipe() -> OwnedComputeLaunchV1 {
    OwnedComputeLaunchV1 {
        stream: 7,
        kernel: 11,
        explicit_kernarg: vec![0x5a; 32].into_boxed_slice(),
        bindings: vec![BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 19,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 64,
            },
            kernarg_byte_offset: 0,
        }]
        .into_boxed_slice(),
        geometry: crate::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        },
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    }
}

fn shared_host_payload_bytes(launch: &OwnedComputeLaunchV1) -> u64 {
    (launch.explicit_kernarg.len()
        + launch.bindings.len() * core::mem::size_of::<BackendBindingV1>()) as u64
}

#[test]
fn shared_host_account_constructor_refuses_before_kfd_and_refunds_partial_tables() {
    for (device, bytes, records, kind) in [
        (0, u64::MAX, 4, KfdRuntimeBackendErrorKindV1::InvalidLaunch),
        (1, 0, 2, KfdRuntimeBackendErrorKindV1::Capacity),
        (
            1,
            2 * pipeline_bytes() - 1,
            2,
            KfdRuntimeBackendErrorKindV1::Capacity,
        ),
        (1, u64::MAX, 1, KfdRuntimeBackendErrorKindV1::Capacity),
    ] {
        let account = account(bytes, records);
        let before = account.usage();
        let result = KfdRuntimeBackendV1::open_gfx942_vecadd_repeat_scale_qualification_with_shared_host_account_v1(
            device, account.clone(),
        );
        assert!(matches!(result, Err(error) if error.kind() == kind));
        assert_eq!(account.usage(), before);
    }
}

#[test]
fn shared_host_account_payload_and_tables_share_identity_bytes_and_final_alias_refund() {
    let launch = shared_host_payload_recipe();
    let payload_bytes = shared_host_payload_bytes(&launch);
    let account = account(2 * pipeline_bytes() + payload_bytes, 4);
    let backend = backend(account.clone());
    assert!(backend.launch_payload_account.is_none());
    let backend = backend.with_shared_host_payload_account_v1();
    assert!(
        backend
            .dispatch_capacity
            .account
            .as_ref()
            .unwrap()
            .shares_ledger_with(&account)
    );
    assert!(
        backend
            .launch_payload_account
            .as_ref()
            .unwrap()
            .shares_ledger_with(&account)
    );
    let payload = RetainedComputeLaunchV1::copy_from(
        launch.borrowed(),
        backend.launch_payload_account.as_ref(),
    )
    .unwrap();
    let alias = Arc::clone(&payload);
    let charged = account.usage();
    assert_eq!(
        charged.used.get(ResourceKindV1::ControlResidentBytes),
        2 * pipeline_bytes() + payload_bytes
    );
    assert_eq!(charged.retained_records, 3);
    assert_eq!(
        backend.scale_qualification_host_table_usage_v1(),
        Some(charged)
    );
    assert_eq!(
        backend.scale_qualification_launch_payload_account_usage_v1(),
        Some(charged)
    );
    rejected(
        RetainedComputeLaunchV1::copy_from(
            launch.borrowed(),
            backend.launch_payload_account.as_ref(),
        ),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    assert_eq!(account.usage(), charged);
    drop(backend);
    assert_eq!(
        account
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        payload_bytes
    );
    assert_eq!(account.usage().retained_records, 1);
    drop(payload);
    assert_eq!(account.usage().retained_records, 1);
    drop(alias);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(account.usage().retained_records, 0);
}

#[test]
fn shared_host_account_payload_competes_with_epoch_and_custody_tables_before_native_work() {
    let launch = shared_host_payload_recipe();
    let account = account(u64::MAX, 3);
    let mut backend = backend(account.clone()).with_shared_host_payload_account_v1();
    let baseline = account.usage();
    let payload = RetainedComputeLaunchV1::copy_from(
        launch.borrowed(),
        backend.launch_payload_account.as_ref(),
    )
    .unwrap();
    let charged = account.usage();
    rejected(
        backend.preallocate_native_binding_v1(false),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    assert!(RuntimeAllocationCustodyV1::try_new(&backend.dispatch_capacity).is_err());
    assert_eq!(account.usage(), charged);
    assert!(!backend.terminal && backend.queue.is_none() && backend.admitted_device.is_none());
    assert!(backend.native_compute_lanes.iter().all(Option::is_none));
    drop(payload);
    assert_eq!(account.usage(), baseline);

    // This allocates only the real epoch host table, not a queue or native backing.
    let epoch = backend
        .preallocate_native_binding_v1(false)
        .unwrap()
        .unwrap();
    rejected(
        RetainedComputeLaunchV1::copy_from(
            launch.borrowed(),
            backend.launch_payload_account.as_ref(),
        ),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    drop(epoch);
    assert_eq!(account.usage(), baseline);
    let custody = RuntimeAllocationCustodyV1::try_new(&backend.dispatch_capacity).unwrap();
    rejected(
        RetainedComputeLaunchV1::copy_from(
            launch.borrowed(),
            backend.launch_payload_account.as_ref(),
        ),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    drop(custody);
    assert_eq!(account.usage(), baseline);
    assert!(backend.queue.is_none() && backend.admitted_device.is_none());
    drop(backend);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn shared_host_account_sibling_charges_enforce_root_byte_and_record_limits() {
    let launch = shared_host_payload_recipe();
    let payload_bytes = shared_host_payload_bytes(&launch);
    for record_limit in [false, true] {
        let records = if record_limit { 3 } else { 4 };
        let bootstrap =
            fe2o3_resource_accounting::resource_domain_bootstrap_bytes_v1(3, records).unwrap();
        let root = ResourceCreditAccountV1::new_root(
            ResourceVectorV1::ZERO.with(
                ResourceKindV1::ControlResidentBytes,
                if record_limit {
                    u64::MAX
                } else {
                    bootstrap + 2 * pipeline_bytes() + payload_bytes
                },
            ),
            3,
            records,
        )
        .unwrap();
        let before = root.usage();
        let child = root
            .new_child(
                ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, u64::MAX),
                records,
            )
            .unwrap();
        let sibling = root
            .new_child(
                ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, u64::MAX),
                records,
            )
            .unwrap();
        let competing = sibling
            .reserve(
                ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, payload_bytes),
            )
            .unwrap()
            .retain();
        let backend = backend(child.clone()).with_shared_host_payload_account_v1();
        let payload_account = backend.launch_payload_account.as_ref().unwrap();
        assert!(payload_account.shares_ledger_with(&child));
        assert!(payload_account.shares_root_with(&sibling));
        assert!(!payload_account.shares_ledger_with(&sibling));
        let charged = root.usage();
        rejected(
            RetainedComputeLaunchV1::copy_from(launch.borrowed(), Some(payload_account)),
            KfdRuntimeBackendErrorKindV1::Capacity,
        );
        assert_eq!(root.usage(), charged);
        competing.release_after_disposal().unwrap();
        let payload =
            RetainedComputeLaunchV1::copy_from(launch.borrowed(), Some(payload_account)).unwrap();
        assert_eq!(root.usage().retained_records, 3);
        drop(payload);
        drop(backend);
        assert_eq!(child.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(root.usage(), before);
    }
}

#[test]
fn shared_host_account_child_keeps_intermediate_ancestor_limits() {
    let launch = shared_host_payload_recipe();
    for record_limit in [false, true] {
        let root = ResourceCreditAccountV1::new_root(
            ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, u64::MAX),
            3,
            4,
        )
        .unwrap();
        let before = root.usage();
        let parent = root
            .new_child(
                ResourceVectorV1::ZERO.with(
                    ResourceKindV1::ControlResidentBytes,
                    if record_limit {
                        u64::MAX
                    } else {
                        2 * pipeline_bytes() + shared_host_payload_bytes(&launch) - 1
                    },
                ),
                if record_limit { 2 } else { 4 },
            )
            .unwrap();
        let child = parent
            .new_child(
                ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, u64::MAX),
                4,
            )
            .unwrap();
        let backend = backend(child.clone()).with_shared_host_payload_account_v1();
        let charged = root.usage();
        rejected(
            RetainedComputeLaunchV1::copy_from(
                launch.borrowed(),
                backend.launch_payload_account.as_ref(),
            ),
            KfdRuntimeBackendErrorKindV1::Capacity,
        );
        assert_eq!(root.usage(), charged);
        drop(backend);
        let payload = RetainedComputeLaunchV1::copy_from(launch.borrowed(), Some(&child)).unwrap();
        assert_eq!(parent.usage().retained_records, 1);
        drop(payload);
        assert_eq!(child.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(root.usage(), before);
    }
}

#[test]
fn shared_host_account_quarantined_credit_is_not_recovered_by_payload_or_table_refusal() {
    let launch = shared_host_payload_recipe();
    let account = account(u64::MAX, 3);
    let mut backend = backend(account.clone()).with_shared_host_payload_account_v1();
    account
        .reserve(ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1))
        .unwrap()
        .retain()
        .quarantine();
    let charged = account.usage();
    assert_eq!(charged.quarantined_records, 1);
    assert!(!charged.poisoned);
    rejected(
        RetainedComputeLaunchV1::copy_from(
            launch.borrowed(),
            backend.launch_payload_account.as_ref(),
        ),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    rejected(
        backend.preallocate_native_binding_v1(false),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    assert_eq!(account.usage(), charged);
    drop(backend);
    assert_eq!(
        account
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        1
    );
    assert_eq!(account.usage().quarantined_records, 1);
    assert_eq!(account.usage().retained_records, 0);
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
            ".create_compute_aql_queue_with_preallocated_fixed_dispatch_v1(",
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
