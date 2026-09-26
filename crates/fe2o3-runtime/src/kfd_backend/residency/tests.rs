use super::*;
use crate::synthetic_cov6;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn account(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ExecutableHostImageBytes, bytes),
        records,
    )
    .unwrap()
}

fn allocate(len: usize) -> Result<Vec<u8>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    Ok(Vec::with_capacity(len))
}

fn rejected<T>(
    result: Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    kind: KfdRuntimeBackendErrorKindV1,
) {
    match result {
        Err(RuntimeBackendFailureV1::Rejected(error)) => assert_eq!(error.kind(), kind),
        Err(error) => panic!("expected rejection {kind:?}, got {error:?}"),
        Ok(_) => panic!("expected rejection {kind:?}, got success"),
    }
}

fn usage(backend: &KfdRuntimeBackendV1) -> ResourceCreditUsageV1 {
    backend.host_image_usage_v1().unwrap()
}

#[test]
fn host_image_credit_exhaustion_precedes_allocator_and_parser() {
    let image = synthetic_cov6::module();
    for records in [false, true] {
        let account = account(
            if records {
                2 * image.len() as u64
            } else {
                image.len() as u64 - 1
            },
            1,
        );
        let retained =
            records.then(|| ResidentModuleImageV1::load(&image, Some(&account)).unwrap());
        let before = account.usage();
        rejected(
            ResidentModuleImageV1::load_with(
                &image,
                Some(&account),
                |_| panic!("exhaustion must precede allocation"),
                |_| panic!("exhaustion must precede parsing"),
            ),
            KfdRuntimeBackendErrorKindV1::Capacity,
        );
        assert_eq!(account.usage(), before);
        drop(retained);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(account.usage().retained_records, 0);
    }
}

#[test]
fn host_image_allocation_validation_and_precommit_unwind_refund_provisional_credit() {
    let image = synthetic_cov6::module();
    let account = account(image.len() as u64, 1);
    let before = account.usage();
    rejected(
        ResidentModuleImageV1::load_with(
            &image,
            Some(&account),
            |_| Err(KfdRuntimeBackendV1::capacity("injected allocation failure")),
            |_| panic!("failed allocation must precede parser"),
        ),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    assert_eq!(account.usage(), before);
    rejected(
        ResidentModuleImageV1::load(b"invalid", Some(&account)),
        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
    );
    assert_eq!(account.usage(), before);
    for in_parser in [false, true] {
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = ResidentModuleImageV1::load_with(
                    &image,
                    Some(&account),
                    |len| {
                        assert!(in_parser, "injected allocator panic");
                        allocate(len)
                    },
                    |bytes| {
                        assert_eq!(bytes, image);
                        assert_eq!(account.usage().reserved_records, 1);
                        panic!("injected parser panic");
                    },
                );
            }))
            .is_err()
        );
        assert_eq!(account.usage(), before);
    }
    let loaded = ResidentModuleImageV1::load(&image, Some(&account)).unwrap();
    drop(loaded);
    assert_eq!(account.usage(), before);
}

#[test]
fn host_image_rust_capacity_mismatch_rejects_before_copy_and_parsing() {
    let image = synthetic_cov6::module();
    let account = account(image.len() as u64, 1);
    let before = account.usage();
    for case in 0..3 {
        rejected(
            ResidentModuleImageV1::load_with(
                &image,
                Some(&account),
                |len| {
                    let mut bytes = Vec::with_capacity(if case == 0 {
                        len - 1
                    } else {
                        len + usize::from(case == 1)
                    });
                    if case == 2 {
                        bytes.push(1);
                    }
                    Ok(bytes)
                },
                |_| panic!("wrong capacity must not reach parser"),
            ),
            KfdRuntimeBackendErrorKindV1::Capacity,
        );
        assert_eq!(account.usage(), before);
    }
}

#[test]
fn host_image_exact_budget_and_kernel_clones_share_one_allocation_and_credit() {
    let image = synthetic_cov6::module();
    let account = account(image.len() as u64, 1);
    let before = account.usage();
    for module_last in [false, true] {
        let module = ResidentModuleImageV1::load(&image, Some(&account)).unwrap();
        let weak = Arc::downgrade(&module.0);
        let kernel = module.bind_kernel("vecadd").unwrap();
        let clone = kernel.clone();
        assert_eq!(module.bytes().as_ptr(), kernel.image.bytes().as_ptr());
        assert!(Arc::ptr_eq(&module.0, &clone.image.0));
        let charged = account.usage();
        assert_eq!(charged.used, before.capacity);
        assert_eq!(
            (
                charged.reserved_records,
                charged.retained_records,
                charged.quarantined_records
            ),
            (0, 1, 0)
        );
        if module_last {
            drop(kernel);
            drop(clone);
            assert_eq!(account.usage(), charged);
            drop(module);
        } else {
            drop(module);
            drop(kernel);
            assert_eq!(account.usage(), charged);
            assert!(weak.upgrade().is_some());
            drop(clone);
        }
        assert!(weak.upgrade().is_none());
        assert_eq!(account.usage(), before);
    }
}

#[test]
fn host_image_backend_load_rejects_without_changing_ids_or_registries() {
    let image = synthetic_cov6::module();
    for records in [false, true] {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend
            .configure_host_image_budget_v1(
                if records {
                    2 * image.len() as u64
                } else {
                    image.len() as u64 - 1
                },
                1,
            )
            .unwrap();
        let module = records.then(|| backend.load_module_v1(7, &image).unwrap());
        let before = usage(&backend);
        let state = (
            backend.next_handle,
            backend.modules.len(),
            backend.kernels.len(),
        );
        rejected(
            backend.load_module_v1(7, &image),
            KfdRuntimeBackendErrorKindV1::Capacity,
        );
        assert_eq!(usage(&backend), before);
        assert_eq!(
            (
                backend.next_handle,
                backend.modules.len(),
                backend.kernels.len()
            ),
            state
        );
        assert!(!backend.terminal && backend.queue.is_none());
        if let Some(module) = module {
            backend.unload_module_v1(module).unwrap();
        }
        assert_eq!(usage(&backend).used, ResourceVectorV1::ZERO);
    }
}

#[test]
fn host_image_repeated_load_resolve_unload_cycles_charge_duplicate_images_separately() {
    let image = synthetic_cov6::module();
    let mut backend = KfdRuntimeBackendV1::mock();
    backend
        .configure_host_image_budget_v1(2 * image.len() as u64, 2)
        .unwrap();
    let before = usage(&backend);
    for _ in 0..4 {
        let first = backend.load_module_v1(7, &image).unwrap();
        let second = backend.load_module_v1(7, &image).unwrap();
        assert_ne!(
            backend.modules[&first].validated.bytes().as_ptr(),
            backend.modules[&second].validated.bytes().as_ptr()
        );
        let charged = usage(&backend);
        assert_eq!(charged.retained_records, 2);
        assert_eq!(charged.used, charged.capacity);
        for _ in 0..3 {
            backend.resolve_kernel_v1(first, "vecadd", [7; 32]).unwrap();
        }
        assert_eq!(usage(&backend), charged);
        rejected(
            backend.resolve_kernel_v1(first, "absent", [7; 32]),
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
        );
        assert_eq!(usage(&backend), charged);
        backend.unload_module_v1(second).unwrap();
        assert_eq!(usage(&backend).retained_records, 1);
        backend.unload_module_v1(first).unwrap();
        assert!(backend.modules.is_empty() && backend.kernels.is_empty());
        assert_eq!(usage(&backend), before);
    }
}

#[test]
fn host_image_configuration_is_atomic_immutable_and_precedes_resource_history() {
    let mut backend = KfdRuntimeBackendV1::mock();
    assert!(backend.host_image_usage_v1().is_none());
    for records in [0, usize::MAX] {
        rejected(
            backend.configure_host_image_budget_v1(100, records),
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
        );
        assert!(backend.host_image_usage_v1().is_none());
        assert_eq!(backend.next_handle, 1);
    }
    backend.configure_host_image_budget_v1(0, 1).unwrap();
    let before = usage(&backend);
    rejected(
        backend.configure_host_image_budget_v1(0, 1),
        KfdRuntimeBackendErrorKindV1::Busy,
    );
    rejected(
        backend.load_module_v1(7, b"image"),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    assert_eq!(usage(&backend), before);
    assert_eq!(backend.next_handle, 1);
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    rejected(
        backend.configure_host_image_budget_v1(100, 1),
        KfdRuntimeBackendErrorKindV1::Busy,
    );
    assert!(backend.host_image_usage_v1().is_none());
}

#[test]
fn host_image_unconfigured_module_behavior_and_backend_drop_remain_valid() {
    let image = synthetic_cov6::module();
    for configured in [false, true] {
        let mut backend = KfdRuntimeBackendV1::mock();
        if configured {
            backend
                .configure_host_image_budget_v1(image.len() as u64, 1)
                .unwrap();
        }
        let account = backend.host_image_account.clone();
        let module = backend.load_module_v1(7, &image).unwrap();
        backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        assert_eq!(backend.host_image_usage_v1().is_some(), configured);
        drop(backend);
        if let Some(account) = account {
            assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
            assert_eq!(account.usage().retained_records, 0);
        }
    }
}

fn configured_launch() -> (KfdRuntimeBackendV1, OwnedComputeLaunchV1) {
    super::super::tests::host_visible_three_binding_launch_with_configuration_v1(|backend| {
        backend
            .configure_host_image_budget_v1(synthetic_cov6::three_binding_module().len() as u64, 1)
            .unwrap();
    })
}

#[test]
fn host_image_actual_prepared_launch_keeps_charge_after_unload_and_backend_drop() {
    for clone_last in [false, true] {
        let (mut backend, launch) = configured_launch();
        let account = backend.host_image_account.clone().unwrap();
        let charged = usage(&backend);
        let module = backend.kernels[&launch.kernel].module;
        let module_alias = backend.modules[&module].validated.clone();
        let kernel_alias = backend.kernels[&launch.kernel].validated.clone();
        let prepared = backend
            .prepare_launch(launch.borrowed(), false, false)
            .unwrap();
        assert!(Arc::ptr_eq(&prepared.program.image.0, &module_alias.0));
        assert_eq!(usage(&backend), charged);
        let borrowed =
            build_program_v1(&prepared.program, prepared.signature, &prepared.abi_rows).unwrap();
        assert!(borrowed.dispatch_abi_identity().is_some());
        drop(borrowed);
        backend.unload_module_v1(module).unwrap();
        assert!(backend.modules.is_empty() && backend.kernels.is_empty());
        assert_eq!(usage(&backend), charged);
        rejected(
            backend.load_module_v1(7, &synthetic_cov6::three_binding_module()),
            KfdRuntimeBackendErrorKindV1::Capacity,
        );
        drop(backend);
        drop(module_alias);
        assert_eq!(account.usage(), charged);
        if clone_last {
            drop(prepared);
            assert_eq!(account.usage(), charged);
            drop(kernel_alias);
        } else {
            drop(kernel_alias);
            assert_eq!(account.usage(), charged);
            drop(prepared);
        }
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(account.usage().retained_records, 0);
    }
}

#[test]
fn host_image_pending_dispatch_busy_unload_preserves_credit_until_cancellation() {
    let (mut backend, launch) = configured_launch();
    let before = usage(&backend);
    let module = backend.kernels[&launch.kernel].module;
    // Model outstanding native dirtiness to defer publication. Cancellation
    // precedes synchronization, so this fixture never creates native custody.
    backend.native_available = true;
    backend.native_dirty_extents = 1;
    let id = backend.submit_v1(launch.borrowed()).unwrap();
    assert!(backend.pending_compute.contains_key(&id));
    assert_eq!(backend.compute_module_retain_counts[&module], 1);
    rejected(
        backend.unload_module_v1(module),
        KfdRuntimeBackendErrorKindV1::Busy,
    );
    assert!(backend.modules.contains_key(&module) && backend.kernels.contains_key(&launch.kernel));
    assert_eq!(usage(&backend), before);
    assert_eq!(
        backend.cancel_v1(id).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    backend.release_submission_v1(id).unwrap();
    assert!(backend.compute_module_retain_counts.is_empty());
    assert_eq!(usage(&backend), before);
    backend.unload_module_v1(module).unwrap();
    assert_eq!(usage(&backend).used, ResourceVectorV1::ZERO);
    backend.native_dirty_extents = 0;
    backend.native_available = false;
}

#[test]
fn host_image_failed_recycled_detach_keeps_module_kernel_and_credit() {
    let (mut backend, launch) = configured_launch();
    let before = usage(&backend);
    let module = backend.kernels[&launch.kernel].module;
    // Metadata-only corrupt-state fixture, not a native queue or DATA lease.
    backend.recycled_dispatch = Some(RecycledDispatchV1 {
        kernel: launch.kernel,
        dispatch_shape_sha256: [0; 32],
        descriptors: Vec::new(),
    });
    rejected(
        backend.unload_module_v1(module),
        KfdRuntimeBackendErrorKindV1::Unsupported,
    );
    assert!(!backend.terminal);
    assert!(backend.modules.contains_key(&module) && backend.kernels.contains_key(&launch.kernel));
    assert_eq!(usage(&backend), before);
    backend.unload_module_v1(module).unwrap();
    assert_eq!(usage(&backend).used, ResourceVectorV1::ZERO);
}

#[test]
fn host_image_terminal_control_release_failure_preserves_owners_and_observable_usage() {
    let (mut backend, launch) = configured_launch();
    let before = usage(&backend);
    let module = backend.kernels[&launch.kernel].module;
    // Metadata-only missing-control fixture; no native allocation is fabricated.
    let retained = RetainedPersistentDispatchV1 {
        allocation: launch.bindings[0].region.allocation,
        dispatch_shape_sha256: [0; 32],
    };
    backend.retained_persistent_dispatch = Some(retained);
    assert!(matches!(
        backend.unload_module_v1(module),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(backend.terminal);
    assert_eq!(backend.retained_persistent_dispatch, Some(retained));
    assert!(backend.modules.contains_key(&module) && backend.kernels.contains_key(&launch.kernel));
    assert_eq!(usage(&backend), before);
    let handle = backend.next_handle;
    assert!(matches!(
        backend.configure_host_image_budget_v1(1, 1),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(backend.next_handle, handle);
    assert_eq!(usage(&backend), before);
    // No native custody was created; disarm only this CPU fixture's terminal bit.
    backend.terminal = false;
    backend.retained_persistent_dispatch = None;
    backend.unload_module_v1(module).unwrap();
    assert_eq!(usage(&backend).used, ResourceVectorV1::ZERO);
}

#[test]
fn host_image_configuration_is_independent_of_native_budgets_and_preserves_capabilities() {
    let image = synthetic_cov6::module();
    for host_first in [false, true] {
        let mut backend = KfdRuntimeBackendV1::mock();
        let capabilities = backend.description.capabilities;
        let budget = Gfx942DeviceBackingBudgetV1::new(64 * 1024, 4).unwrap();
        if host_first {
            backend
                .configure_host_image_budget_v1(image.len() as u64, 1)
                .unwrap();
        }
        backend.configure_device_backing_budget_v1(budget).unwrap();
        if !host_first {
            backend
                .configure_host_image_budget_v1(image.len() as u64, 1)
                .unwrap();
        }
        assert_eq!(backend.device_backing_budget_v1(), Some(budget));
        assert_eq!(backend.device_backing_usage_v1(), None);
        assert_eq!(backend.description.capabilities, capabilities);
        assert_eq!(backend.next_handle, 1);
        assert!(backend.queue.is_none());
        let before = usage(&backend);
        rejected(
            backend.load_module_v1(u64::MAX, &image),
            KfdRuntimeBackendErrorKindV1::WrongDevice,
        );
        rejected(
            backend.unload_module_v1(u64::MAX),
            KfdRuntimeBackendErrorKindV1::UnknownHandle,
        );
        assert_eq!(usage(&backend), before);
        assert_eq!(backend.next_handle, 1);
        assert!(backend.modules.is_empty() && backend.kernels.is_empty());
        let module = backend.load_module_v1(7, &image).unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let alias = backend.kernels[&kernel].validated.clone();
        backend.unload_module_v1(module).unwrap();
        rejected(
            backend.load_module_v1(7, &image),
            KfdRuntimeBackendErrorKindV1::Capacity,
        );
        drop(alias);
        assert_eq!(usage(&backend), before);
        let reloaded = backend.load_module_v1(7, &image).unwrap();
        backend.unload_module_v1(reloaded).unwrap();
        assert_eq!(usage(&backend), before);
    }
}

#[test]
fn host_image_successful_insertion_survives_caller_unwind_and_failed_loads_refund() {
    let image = synthetic_cov6::module();
    let mut backend = KfdRuntimeBackendV1::mock();
    backend
        .configure_host_image_budget_v1(image.len() as u64, 1)
        .unwrap();
    let empty = usage(&backend);
    let handle = backend.next_handle;
    rejected(
        backend.load_module_v1(7, b"invalid"),
        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
    );
    assert_eq!(usage(&backend), empty);
    assert_eq!(backend.next_handle, handle);
    assert!(backend.modules.is_empty());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            backend.load_module_v1(7, &image).unwrap();
            panic!("caller panic after committed load");
        }))
        .is_err()
    );
    assert_eq!(backend.modules.len(), 1);
    assert_eq!(usage(&backend).retained_records, 1);
    let module = *backend.modules.keys().next().unwrap();
    backend.unload_module_v1(module).unwrap();
    assert_eq!(usage(&backend), empty);
    backend.next_handle = u64::MAX;
    rejected(
        backend.load_module_v1(7, &image),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    assert!(backend.modules.is_empty());
    assert_eq!(usage(&backend), empty);
    assert_eq!(backend.next_handle, u64::MAX);
}
