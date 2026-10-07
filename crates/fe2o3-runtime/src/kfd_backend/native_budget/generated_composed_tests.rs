use super::*;
use crate::{RuntimeResourceKindV1 as Kind, RuntimeResourceVectorV1 as Vector};

fn root() -> Gfx942ComposedBackingRootV1 {
    // Exactly one device and one session plus its three class leaves.
    let capacity = Vector::ZERO
        .with(
            Kind::ControlResidentBytes,
            Gfx942ComposedBackingRootV1::bootstrap_bytes_v1(1, 6, 32).unwrap(),
        )
        .with(Kind::RequestedAllocationBytes, 64)
        .with(Kind::ResidentHostAllocationBytes, 65536)
        .with(Kind::ResidentDeviceAllocationBytes, 65536)
        .with(Kind::AllocationRecords, 24);
    Gfx942ComposedBackingRootV1::new(capacity, 1, 6, 32).unwrap()
}

fn budgets() -> (
    Gfx942ComposedBackingDeviceBudgetV1,
    Gfx942ComposedBackingSessionBudgetV1,
) {
    (
        Gfx942ComposedBackingDeviceBudgetV1::new(64, 65536, 65536, 24).unwrap(),
        Gfx942ComposedBackingSessionBudgetV1::new(
            fe2o3_kfd::Gfx942AllocationRequestBudgetV1::new(64, 8).unwrap(),
            Gfx942HostVisibleBackingBudgetV1::new(65536, 8).unwrap(),
            Gfx942DeviceBackingBudgetV1::new(65536, 8).unwrap(),
            24,
        )
        .unwrap(),
    )
}

#[test]
fn generated_composed_zero_device_rejects_before_root_or_native_admission() {
    let root = root();
    let (device_budget, session_budget) = budgets();
    let before = root.usage_v1();
    let error = KfdRuntimeBackendV1::open_worker_v3_generated_only_with_composed_backing_root_v1(
        0,
        &root,
        device_budget,
        session_budget,
    )
    .unwrap_err();
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    assert_eq!(root.usage_v1(), before);
    assert!(root.device_usage_v1(0).unwrap().is_none());
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires idle MI300X and FE2O3_TEST_NATIVE_UNIQUE_ID; opens checked device only"]
fn native_generated_composed_admission_pressure_gate_and_refund() {
    let raw = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_ID").expect("explicit physical device UID");
    let uid = u64::from_str_radix(raw.trim_start_matches("0x"), 16).unwrap();
    let root = root();
    let (device_budget, session_budget) = budgets();
    let baseline = root.usage_v1();
    let checked = KfdRuntimeBackendV1::open_checked_device_v1(uid).unwrap();
    let blocker = root
        .admit_session_v1(&checked, device_budget, session_budget)
        .unwrap();
    let blocked_usage = root.usage_v1();
    let failure = KfdRuntimeBackendV1::from_checked_device_worker_v3_generated_only_with_composed_backing_root_v1(
        checked, &root, device_budget, session_budget,
    ).unwrap_err();
    assert_eq!(failure.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
    assert_eq!(root.usage_v1(), blocked_usage);
    drop(blocker);
    assert_eq!(root.usage_v1(), baseline);

    // The canonical device ceiling is immutable, even after an unused session drops.
    let mismatched = Gfx942ComposedBackingDeviceBudgetV1::new(65, 65536, 65536, 24).unwrap();
    let failure = KfdRuntimeBackendV1::open_worker_v3_generated_only_with_composed_backing_root_v1(
        uid,
        &root,
        mismatched,
        session_budget,
    )
    .unwrap_err();
    assert_eq!(failure.kind(), KfdRuntimeBackendErrorKindV1::Terminal);
    assert_eq!(root.usage_v1(), baseline);

    let mut backend =
        KfdRuntimeBackendV1::open_worker_v3_generated_only_with_composed_backing_root_v1(
            uid,
            &root,
            device_budget,
            session_budget,
        )
        .unwrap();
    assert!(matches!(
        backend.launch_gate,
        KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly
    ));
    assert!(!backend.description.capabilities.typed_async_launch);
    let capabilities = backend.execution_capabilities_v1(uid);
    assert!(!capabilities.atomics && !capabilities.collectives && !capabilities.concurrent_compute);
    assert!(capabilities.native_async_copy && capabilities.memory_pool);
    assert_eq!(
        backend.host_visible_backing_budget_v1(),
        Some(session_budget.host_budget())
    );
    assert_eq!(
        backend.device_backing_budget_v1(),
        Some(session_budget.device_budget())
    );
    assert!(matches!(backend.allocation_admission_profile_v1().unwrap(),
        crate::RuntimeAllocationAdmissionProfileV1::Required(ref entries) if entries.len() == 1));
    assert!(
        matches!(backend.allocate_v1(uid, RuntimeMemoryKindV1::HostVisible, 4, 4),
        Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported)
    );
    let request = match &backend.rooted_backing {
        Some(RootedBackingV1::Composed(Some(admission))) => admission.request_account_v1().clone(),
        _ => panic!("constructor must retain the composed admission"),
    };
    let next = backend.next_handle;
    let pressure = request.reserve_v1(64).unwrap();
    let before = root.usage_v1();
    let mut context = crate::RuntimeContextV1::open_with_version_journal_v1(backend, 8, 8).unwrap();
    let device = context.devices()[0].id();
    let usage = request.usage_v1();
    assert_eq!(
        context.allocation_admission_usage_v1(device).unwrap(),
        Some(crate::RuntimeResourceCreditUsageV1 {
            device,
            capacity: usage.capacity,
            used: usage.used,
            reserved_records: usage.reserved_records,
            retained_records: usage.retained_records,
            quarantined_records: usage.quarantined_records,
            record_capacity: usage.record_capacity,
            poisoned: usage.poisoned,
        })
    );
    assert!(matches!(
        context.allocate(device, RuntimeMemoryKindV1::HostVisible, 4, 4),
        Err(crate::RuntimeErrorV1::Validation(
            crate::RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(root.usage_v1(), before);
    assert!(
        context
            .configure_allocation_admission_v1(device, 128, 8)
            .is_err()
    );
    assert_eq!(root.usage_v1(), before);
    drop(pressure);
    // A complete roster can again reserve the original request leaf. This does
    // not claim generated shell registration or native backing allocation.
    let roster = request.reserve_batch_v1(&[16, 20, 24]).unwrap();
    assert_eq!(root.usage_v1().reserved_records, 3);
    drop(roster);
    assert_eq!(root.usage_v1(), baseline);
    let mut backend = context.shutdown().unwrap();
    assert_eq!(backend.next_handle, next);
    assert!(backend.allocations.is_empty() && backend.queue.is_none());
    assert!(backend.terminal_memory.is_none() && backend.admitted_device.is_some());
    assert!(matches!(
        backend.allocation_admission_profile_v1().unwrap(),
        crate::RuntimeAllocationAdmissionProfileV1::Required(_)
    ));
    backend.shutdown_native_v1().unwrap();
    drop(backend);
    drop(request);
    assert_eq!(root.usage_v1(), baseline);

    // Reopening succeeds only because no VM was acquired in this test.
    let backend = KfdRuntimeBackendV1::open_worker_v3_generated_only_with_composed_backing_root_v1(
        uid,
        &root,
        device_budget,
        session_budget,
    )
    .unwrap();
    drop(backend);
    assert_eq!(root.usage_v1(), baseline);
    println!(
        "generated_composed uid={uid:#x} domain_pressure=passed request_pressure=passed gate=generated_only refund=bootstrap no_vm_or_dispatch=true"
    );
}
