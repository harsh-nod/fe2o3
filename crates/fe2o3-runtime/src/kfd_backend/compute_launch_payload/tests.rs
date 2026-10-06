use super::*;
use fe2o3_resource_accounting::resource_domain_bootstrap_bytes_v1;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[cfg(feature = "scale-qualification")]
#[path = "tests/scale_settlement.rs"]
mod scale_settlement;

fn account(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        records,
    )
    .unwrap()
}

fn recipe() -> OwnedComputeLaunchV1 {
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

fn bytes(recipe: &OwnedComputeLaunchV1) -> u64 {
    payload_bytes(recipe.explicit_kernarg.len(), recipe.bindings.len()).unwrap()
}

fn used(account: &ResourceCreditAccountV1) -> u64 {
    account
        .usage()
        .used
        .get(ResourceKindV1::ControlResidentBytes)
}

fn capacity<T>(result: Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>) {
    assert!(matches!(
        result,
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
}

#[test]
fn retained_launch_checked_cost_and_admission_precede_every_copy() {
    assert_eq!(payload_bytes(usize::MAX, 0), None);
    assert_eq!(payload_bytes(0, usize::MAX), None);
    assert_eq!(payload_bytes(0, 0), Some(0));
    let recipe = recipe();
    for record_refusal in [false, true] {
        let account = account(
            if record_refusal {
                bytes(&recipe) * 2
            } else {
                bytes(&recipe) - 1
            },
            1,
        );
        let sibling =
            record_refusal.then(|| account.reserve(ResourceVectorV1::ZERO).unwrap().retain());
        let before = account.usage();
        capacity(RetainedComputeLaunchV1::copy_with(
            recipe.borrowed(),
            Some(&account),
            |_| panic!("admission must precede kernarg copying"),
            |_| panic!("admission must precede binding copying"),
        ));
        assert_eq!(account.usage(), before);
        if let Some(sibling) = sibling {
            sibling.release_after_disposal().unwrap();
        }
        assert_eq!(used(&account), 0);
    }
}

#[test]
fn retained_launch_copy_failures_and_capacity_mismatch_refund_exactly() {
    let recipe = recipe();
    for failure in 0..5 {
        let account = account(bytes(&recipe), 1);
        let before = account.usage();
        let copies = Cell::new(0);
        capacity(RetainedComputeLaunchV1::copy_with(
            recipe.borrowed(),
            Some(&account),
            |len| {
                assert_eq!(used(&account), bytes(&recipe));
                assert_eq!(account.usage().reserved_records, 1);
                copies.set(copies.get() + 1);
                if failure == 0 {
                    return Err(KfdRuntimeBackendV1::capacity("injected kernarg allocation"));
                }
                if failure == 1 {
                    return allocate_exact(len + 1);
                }
                let mut values = allocate_exact(len)?;
                if failure == 2 {
                    values.push(0);
                }
                Ok(values)
            },
            |len| {
                assert_eq!(used(&account), bytes(&recipe));
                copies.set(copies.get() + 1);
                if failure == 3 {
                    Err(KfdRuntimeBackendV1::capacity("injected binding allocation"))
                } else {
                    allocate_exact(len + 1)
                }
            },
        ));
        assert_eq!(copies.get(), if failure <= 2 { 1 } else { 2 });
        assert_eq!(account.usage(), before);
    }
}

#[test]
fn retained_launch_copy_unwind_refunds_unaccepted_reservation() {
    let recipe = recipe();
    let account = account(bytes(&recipe), 1);
    let before = account.usage();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = RetainedComputeLaunchV1::copy_with(
                recipe.borrowed(),
                Some(&account),
                allocate_exact,
                |_| panic!("injected second allocation unwind"),
            );
        }))
        .is_err()
    );
    assert_eq!(account.usage(), before);
}

#[test]
fn retained_launch_arc_aliases_and_recipe_comparison_preserve_one_charge() {
    let recipe = recipe();
    let account = account(bytes(&recipe) * 2, 2);
    let owner = RetainedComputeLaunchV1::copy_from(recipe.borrowed(), Some(&account)).unwrap();
    assert_eq!(&**owner, &recipe);
    let alias = Arc::clone(&owner);
    let weak = Arc::downgrade(&owner);
    assert_eq!(used(&account), bytes(&recipe));
    assert_eq!(account.usage().retained_records, 1);
    let independent =
        RetainedComputeLaunchV1::copy_from(recipe.borrowed(), Some(&account)).unwrap();
    assert_eq!(used(&account), bytes(&recipe) * 2);
    assert!(compute_dispatch::ordinary_compute_recipes_match_v1(
        &owner,
        &independent
    ));
    drop(independent);
    drop(owner);
    assert_eq!(used(&account), bytes(&recipe));
    assert!(weak.upgrade().is_some());
    drop(alias);
    assert!(weak.upgrade().is_none());
    assert_eq!(used(&account), 0);
    assert_eq!(account.usage().retained_records, 0);
    // Weak Arc control storage is explicitly not part of the slice charge.
    drop(weak);
}

#[test]
fn retained_launch_shared_root_enforces_aggregate_and_leaf_limits() {
    let recipe = recipe();
    let charge = bytes(&recipe);
    let baseline = resource_domain_bootstrap_bytes_v1(3, 4).unwrap();
    let root = ResourceCreditAccountV1::new_root(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, baseline + 2 * charge),
        3,
        4,
    )
    .unwrap();
    let first = root
        .new_child(
            ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 2 * charge),
            2,
        )
        .unwrap();
    let second = root
        .new_child(
            ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, charge),
            2,
        )
        .unwrap();
    let a = RetainedComputeLaunchV1::copy_from(recipe.borrowed(), Some(&first)).unwrap();
    let b = RetainedComputeLaunchV1::copy_from(recipe.borrowed(), Some(&first)).unwrap();
    let before = root.usage();
    capacity(RetainedComputeLaunchV1::copy_from(
        recipe.borrowed(),
        Some(&second),
    ));
    assert_eq!(root.usage(), before);
    assert_eq!(used(&second), 0);
    drop(a);
    let c = RetainedComputeLaunchV1::copy_from(recipe.borrowed(), Some(&second)).unwrap();
    drop(b);
    // Root has room now, but this leaf still permits only one payload.
    let before = root.usage();
    capacity(RetainedComputeLaunchV1::copy_from(
        recipe.borrowed(),
        Some(&second),
    ));
    assert_eq!(root.usage(), before);
    drop(c);
    assert_eq!(used(&root), baseline);
    assert_eq!(used(&first), 0);
    assert_eq!(used(&second), 0);
}

#[test]
fn retained_launch_storage_disposal_precedes_refund_and_panic_quarantines() {
    struct Storage<'a>(&'a ResourceCreditAccountV1, &'a Cell<usize>, bool);
    impl Drop for Storage<'_> {
        fn drop(&mut self) {
            assert_eq!(used(self.0), 8);
            assert_eq!(self.0.usage().retained_records, 1);
            self.1.set(self.1.get() + 1);
            assert!(!self.2, "injected storage disposal unwind");
        }
    }
    for panic in [false, true] {
        let account = account(8, 1);
        let credits = account
            .reserve(ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 8))
            .unwrap()
            .retain();
        let drops = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            dispose_then_refund(Storage(&account, &drops, panic), Some(credits));
        }));
        assert_eq!(result.is_err(), panic);
        assert_eq!(drops.get(), 1);
        assert_eq!(used(&account), if panic { 8 } else { 0 });
        assert_eq!(account.usage().quarantined_records, usize::from(panic));
    }
}

fn pending_fixture(
    account: ResourceCreditAccountV1,
) -> (
    std::mem::ManuallyDrop<KfdRuntimeBackendV1>,
    OwnedComputeLaunchV1,
) {
    let (mut backend, launch) =
        super::super::tests::host_visible_three_binding_launch_with_configuration_v1(|backend| {
            backend.launch_payload_account = Some(account)
        });
    for binding in &launch.bindings {
        backend
            .allocations
            .get_mut(&binding.region.allocation)
            .unwrap()
            .sdma_shadow_dirty = true;
    }
    backend.native_available = true;
    (std::mem::ManuallyDrop::new(backend), launch)
}

fn dispose_fixture(mut backend: std::mem::ManuallyDrop<KfdRuntimeBackendV1>) {
    assert!(backend.queue.is_none() && backend.active.is_none());
    assert!(backend.pending_compute.is_empty() && backend.allocation_custody.is_empty());
    assert_eq!(backend.compute_completion_reservations, 0);
    backend.native_available = false;
    drop(std::mem::ManuallyDrop::into_inner(backend));
}

#[test]
fn retained_launch_pending_admission_refusal_preserves_neighbor_and_all_custody() {
    let account = account(32 + 3 * core::mem::size_of::<BackendBindingV1>() as u64, 1);
    let (mut backend, launch) = pending_fixture(account.clone());
    let id = backend.submit_v1(launch.borrowed()).unwrap();
    let next = backend.next_handle;
    let reservations = backend.compute_completion_reservations;
    let before = account.usage();
    capacity(backend.submit_v1(launch.borrowed()));
    assert_eq!(account.usage(), before);
    assert_eq!(backend.next_handle, next);
    assert_eq!(backend.compute_completion_reservations, reservations);
    assert_eq!(backend.pending_compute.len(), 1);
    assert_eq!(
        backend.pending_compute_streams[&launch.stream]
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![id]
    );
    for binding in &launch.bindings {
        let custody = &backend.allocation_custody[&binding.region.allocation];
        assert_eq!(custody.owners.len(), 1);
        assert_eq!(custody.owners[0].submission, id);
    }
    assert_eq!(
        backend.cancel_v1(id).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    backend.release_submission_v1(id).unwrap();
    assert_eq!(used(&account), 0);
    dispose_fixture(backend);
}

#[test]
fn retained_launch_later_no_effect_refusal_refunds_copied_payload() {
    let account = account(4096, 2);
    let (mut backend, launch) = pending_fixture(account.clone());
    backend.next_handle = u64::MAX;
    let before = account.usage();
    capacity(backend.submit_v1(launch.borrowed()));
    assert_eq!(account.usage(), before);
    assert_eq!(backend.next_handle, u64::MAX);
    assert!(backend.pending_compute_streams.is_empty());
    assert!(backend.compute_module_retain_counts.is_empty());
    dispose_fixture(backend);
}

#[test]
fn retained_launch_final_recipe_alias_outlives_pending_no_effect_settlement() {
    let account = account(4096, 2);
    let (mut backend, launch) = pending_fixture(account.clone());
    let id = backend.submit_v1(launch.borrowed()).unwrap();
    let retained = Arc::clone(&backend.pending_compute[&id].launch);
    assert_eq!(used(&account), bytes(&launch));
    assert_eq!(
        backend.cancel_v1(id).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    backend.release_submission_v1(id).unwrap();
    dispose_fixture(backend);
    assert_eq!(used(&account), bytes(&launch));
    drop(retained);
    assert_eq!(used(&account), 0);
}

#[cfg(feature = "scale-qualification")]
#[test]
fn retained_launch_opt_in_constructor_rejects_before_kfd_and_keeps_account() {
    let account = account(4096, 2);
    let before = account.usage();
    for (device, table_bytes, kind) in [
        (0, 4096, KfdRuntimeBackendErrorKindV1::InvalidLaunch),
        (7, 0, KfdRuntimeBackendErrorKindV1::Capacity),
    ] {
        let result = KfdRuntimeBackendV1::open_gfx942_vecadd_repeat_scale_qualification_with_launch_payload_account_v1(
            device, table_bytes, 8, account.clone(),
        );
        assert!(matches!(result, Err(error) if error.kind() == kind));
        assert_eq!(account.usage(), before);
    }
    let backend = KfdRuntimeBackendV1::mock();
    assert_eq!(
        backend.scale_qualification_launch_payload_account_usage_v1(),
        None
    );
}

#[cfg(feature = "scale-qualification")]
#[test]
fn retained_launch_async_snapshot_handoff_preserves_backend_charge_until_final_disposal() {
    use crate::qualification_gfx942_vecadd_repeat_v1::{
        Gfx942VecaddRepeatQualificationArgumentsV1 as Arguments,
        admit_gfx942_vecadd_repeat_qualification_v1,
    };
    use crate::qualification_gfx942_vecadd_v1::{
        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1 as ALIGNMENT,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as BYTES,
    };
    use crate::{RuntimeAsyncLaunchRequestV1, RuntimeContextV1};

    let payload = account(4096, 4);
    let tables = account(64 * 1024 * 1024, 32);
    let admitted = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
    let buffers = admitted.host_buffers().unwrap();
    let mut backend = scale_capacity::tests::backend(tables.clone());
    backend.launch_gate = KfdRuntimeLaunchGateV1::ExactGfx942VecaddRepeat(
        admit_gfx942_vecadd_repeat_qualification_v1().unwrap(),
    );
    backend.launch_payload_account = Some(payload.clone());
    // Synthetic-only fixture: an assertion failure must not invoke KFD's
    // fail-closed Drop on a deliberately deferred CPU record.
    let mut context = std::mem::ManuallyDrop::new(RuntimeContextV1::open(backend).unwrap());
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let module = context.load_module(device, admitted.hsaco()).unwrap();
    let kernel = Arc::new(
        context
            .resolve_kernel::<Arguments>(module, admitted.kernel_name())
            .unwrap(),
    );
    let allocations = [buffers.left(), buffers.right(), buffers.output()].map(|bytes| {
        let allocation = context
            .allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                BYTES as u64,
                ALIGNMENT,
            )
            .unwrap();
        context.write_allocation(allocation, 0, bytes).unwrap();
        allocation
    });
    {
        let backend = context.backend_mut_for_test_v1();
        assert!(backend.queue.is_none() && backend.admitted_device.is_none());
        for record in backend.allocations.values_mut() {
            assert!(matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::Synthetic
            ));
            record.sdma_backed = true;
            record.sdma_initialized = true;
            record.sdma_shadow_dirty = true;
        }
        // Dirty synthetic shadows prevent eager native publication.
        backend.native_available = true;
    }
    let arguments = Arguments::new(allocations[0], allocations[1], allocations[2]).unwrap();
    let request = RuntimeAsyncLaunchRequestV1::new(
        stream,
        kernel,
        &arguments,
        admitted.geometry(),
        Vec::new(),
    )
    .unwrap();
    let expected = payload_bytes(admitted.explicit_kernarg().len(), 3).unwrap();
    assert_eq!(used(&payload), 0);
    let mut final_recipe = None;
    let outcome = request.consume_with_handoff_inspection_for_test_v1(
        &mut context,
        |context, snapshot_bytes| {
            assert_eq!(snapshot_bytes, 0);
            assert_eq!(used(&payload), expected);
            assert_eq!(payload.usage().retained_records, 1);
            let backend = context.backend_mut_for_test_v1();
            assert!(backend.queue.is_none() && backend.admitted_device.is_none());
            assert!(backend.active.is_none() && backend.compute_pipeline.is_empty());
            assert_eq!(backend.pending_compute.len(), 1);
            assert_eq!(backend.compute_completion_reservations, 1);
            let (&id, pending) = backend.pending_compute.iter().next().unwrap();
            final_recipe = Some(Arc::clone(&pending.launch));
            assert_eq!(
                backend.cancel_v1(id).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            assert!(backend.pending_compute.is_empty() && backend.allocation_custody.is_empty());
            assert_eq!(backend.compute_completion_reservations, 0);
            assert_eq!(used(&payload), expected);
        },
    );
    assert!(matches!(
        outcome.observation,
        Ok(crate::RuntimeCompletionStatusV1::Failed(
            crate::RuntimeCompletionFailureV1::BackendCode(-2),
        ))
    ));
    context
        .release_submission(outcome.submission.expect("accepted Context submission"))
        .unwrap();
    {
        let backend = context.backend_mut_for_test_v1();
        assert!(backend.queue.is_none() && backend.active.is_none());
        backend.native_available = false;
        for record in backend.allocations.values_mut() {
            assert!(matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::Synthetic
            ));
            record.sdma_backed = false;
            record.sdma_initialized = false;
            record.sdma_shadow_dirty = false;
        }
    }
    let context = std::mem::ManuallyDrop::into_inner(context);
    let backend = context.shutdown().unwrap();
    drop(backend);
    assert_eq!(used(&tables), 0);
    assert_eq!(used(&payload), expected);
    drop(final_recipe.take());
    assert_eq!(used(&payload), 0);
    assert_eq!(payload.usage().retained_records, 0);
}
