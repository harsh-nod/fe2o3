use super::*;
use crate::resource_domains::native_tests::{
    admission, budget, device_budget, observe_lifetime, root,
};
use fe2o3_resource_accounting::{ResourceCreditErrorV1, ResourceKindV1};

fn configured(
    root: &crate::Gfx942NativeBackingRootV1,
    parent: crate::Gfx942NativeBackingDeviceBudgetV1,
    budget: crate::Gfx942NativeBackingSessionBudgetV1,
) -> SharedMemoryEngine<FakeBackend> {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    engine
        .configure_native_backing_admission_v1(device, vm, admission(root, device, parent, budget))
        .unwrap();
    engine
}

fn calls(engine: &SharedMemoryEngine<FakeBackend>) -> (usize, usize, usize, usize, usize) {
    (
        engine.backend.currentness_calls,
        engine.backend.reserve_va_calls,
        engine.backend.alloc_calls,
        engine.backend.free_calls,
        engine.backend.release_va_calls,
    )
}

#[test]
fn compound_backing_mixed_session_records_reject_before_effects_and_refund_for_other_class() {
    let (device, vm) = device_vm(1);
    for device_first in [false, true] {
        let root = root(65536, 65536, 8);
        let mut engine = configured(&root, device_budget(8), budget(2, 2, 3));
        let mut host = Vec::new();
        let mut native = Vec::new();
        for _ in 0..if device_first { 1 } else { 2 } {
            host.push(engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap());
        }
        for _ in 0..if device_first { 2 } else { 1 } {
            native.push(engine.allocate_device_memory(device, vm, 1, 4096).unwrap());
        }
        let before = calls(&engine);
        let usage = root.usage_v1();
        if device_first {
            assert!(matches!(
                engine.allocate::<HostVisibleCoherentGttV1>(1),
                Err(MemorySessionError::HostVisibleBackingCredits(
                    ResourceCreditErrorV1::RecordCapacity
                ))
            ));
        } else {
            assert!(matches!(
                engine.allocate_device_memory(device, vm, 1, 4096),
                Err(MemorySessionError::DeviceBackingCredits(
                    ResourceCreditErrorV1::RecordCapacity
                ))
            ));
        }
        assert_eq!(calls(&engine), before);
        assert_eq!(root.usage_v1(), usage);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
        assert_eq!(
            engine
                .host_backing_account
                .as_ref()
                .unwrap()
                .usage()
                .used_allocation_records,
            host.len() as u64
        );
        assert_eq!(
            engine
                .device_backing_account
                .as_ref()
                .unwrap()
                .usage()
                .used_allocation_records,
            native.len() as u64
        );
        assert_eq!(
            engine
                .host_backing_account
                .as_ref()
                .unwrap()
                .session_usage()
                .unwrap()
                .used
                .get(ResourceKindV1::AllocationRecords),
            3
        );
        if device_first {
            engine.release_device_memory(native.pop().unwrap()).unwrap();
            host.push(engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap());
        } else {
            engine
                .release(host.pop().unwrap(), SharedAllocationPhaseV1::CpuWritable)
                .unwrap();
            native.push(engine.allocate_device_memory(device, vm, 1, 4096).unwrap());
        }
        for token in host {
            engine
                .release(token, SharedAllocationPhaseV1::CpuWritable)
                .unwrap();
        }
        for lease in native {
            engine.release_device_memory(lease).unwrap();
        }
        assert_eq!(
            root.usage_v1().used.get(ResourceKindV1::AllocationRecords),
            0
        );
    }
}

#[test]
fn compound_backing_per_class_ceiling_does_not_become_combined_usage() {
    let root = root(65536, 65536, 8);
    let mut engine = configured(&root, device_budget(8), budget(1, 2, 3));
    let (device, vm) = device_vm(1);
    let host = engine.allocate::<HostVisibleCoherentGttV1>(4100).unwrap();
    let before = calls(&engine);
    assert!(matches!(
        engine.allocate::<HostVisibleCoherentGttV1>(1),
        Err(MemorySessionError::HostVisibleBackingCredits(
            ResourceCreditErrorV1::RecordCapacity
        ))
    ));
    assert_eq!(calls(&engine), before);
    let a = engine
        .allocate_device_memory(device, vm, 4097, 4096)
        .unwrap();
    let b = engine.allocate_device_memory(device, vm, 1, 4096).unwrap();
    let host_usage = engine.host_backing_account.as_ref().unwrap().usage();
    let device_usage = engine.device_backing_account.as_ref().unwrap().usage();
    assert_eq!(
        (
            host_usage.used_backing_bytes,
            host_usage.used_allocation_records
        ),
        (8192, 1)
    );
    assert_eq!(
        (
            device_usage.used_backing_bytes,
            device_usage.used_allocation_records
        ),
        (12288, 2)
    );
    assert_eq!(
        root.usage_v1().used.get(ResourceKindV1::AllocationRecords),
        3
    );
    engine
        .release(host, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    engine.release_device_memory(a).unwrap();
    engine.release_device_memory(b).unwrap();
}

#[test]
fn compound_backing_root_device_and_session_record_limits_cover_both_classes() {
    let (device, vm) = device_vm(1);
    for limiting in 0..3 {
        let root = root(65536, 65536, if limiting == 0 { 2 } else { 8 });
        let mut engine = configured(
            &root,
            device_budget(if limiting == 1 { 2 } else { 8 }),
            budget(4, 4, if limiting == 2 { 2 } else { 8 }),
        );
        let host = engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
        let native = engine.allocate_device_memory(device, vm, 1, 4096).unwrap();
        let before = calls(&engine);
        let used = root.usage_v1();
        let expected = if limiting == 0 {
            ResourceCreditErrorV1::Capacity
        } else {
            ResourceCreditErrorV1::RecordCapacity
        };
        assert!(matches!(engine.allocate_device_memory(device, vm, 1, 4096),
            Err(MemorySessionError::DeviceBackingCredits(error)) if error == expected));
        assert_eq!(calls(&engine), before);
        assert_eq!(root.usage_v1(), used);
        engine
            .release(host, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        engine.release_device_memory(native).unwrap();
    }
}

#[test]
fn compound_backing_host_byte_pressure_preserves_independent_device_capacity() {
    let (device, vm) = device_vm(1);
    for limiting in 0..3 {
        let root = root(if limiting == 0 { 8192 } else { 65536 }, 65536, 8);
        let parent = crate::Gfx942NativeBackingDeviceBudgetV1::new(
            if limiting == 1 { 8192 } else { 65536 },
            65536,
            8,
        )
        .unwrap();
        let session = crate::Gfx942NativeBackingSessionBudgetV1::new(
            Gfx942HostVisibleBackingBudgetV1::new(if limiting == 2 { 8192 } else { 32768 }, 4)
                .unwrap(),
            Gfx942DeviceBackingBudgetV1::new(32768, 4).unwrap(),
            8,
        )
        .unwrap();
        let mut engine = configured(&root, parent, session);
        let host = engine.allocate::<HostVisibleCoherentGttV1>(4100).unwrap();
        let before = calls(&engine);
        assert!(matches!(
            engine.allocate::<HostVisibleCoherentGttV1>(1),
            Err(MemorySessionError::HostVisibleBackingCredits(
                ResourceCreditErrorV1::Capacity
            ))
        ));
        assert_eq!(calls(&engine), before);
        let native = engine
            .allocate_device_memory(device, vm, 4097, 4096)
            .unwrap();
        assert_eq!(
            root.usage_v1()
                .used
                .get(ResourceKindV1::ResidentHostAllocationBytes),
            8192
        );
        assert_eq!(
            root.usage_v1()
                .used
                .get(ResourceKindV1::ResidentDeviceAllocationBytes),
            8192
        );
        engine
            .release(host, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        engine.release_device_memory(native).unwrap();
    }
}

#[test]
fn compound_backing_device_byte_and_class_pressure_preserve_host_capacity() {
    let (device, vm) = device_vm(1);
    for limiting in 0..4 {
        let root = root(65536, if limiting == 0 { 8192 } else { 65536 }, 8);
        let parent = crate::Gfx942NativeBackingDeviceBudgetV1::new(
            65536,
            if limiting == 1 { 8192 } else { 65536 },
            8,
        )
        .unwrap();
        let session = crate::Gfx942NativeBackingSessionBudgetV1::new(
            Gfx942HostVisibleBackingBudgetV1::new(32768, 4).unwrap(),
            Gfx942DeviceBackingBudgetV1::new(
                if limiting == 2 { 8192 } else { 32768 },
                if limiting == 3 { 1 } else { 4 },
            )
            .unwrap(),
            8,
        )
        .unwrap();
        let mut engine = configured(&root, parent, session);
        let native = engine
            .allocate_device_memory(device, vm, 4097, 4096)
            .unwrap();
        let before = calls(&engine);
        let usage = root.usage_v1();
        let expected = if limiting == 3 {
            ResourceCreditErrorV1::RecordCapacity
        } else {
            ResourceCreditErrorV1::Capacity
        };
        assert!(matches!(engine.allocate_device_memory(device, vm, 1, 4096),
            Err(MemorySessionError::DeviceBackingCredits(error)) if error == expected));
        assert_eq!(calls(&engine), before);
        assert_eq!(root.usage_v1(), usage);
        let host = engine.allocate::<HostVisibleCoherentGttV1>(4100).unwrap();
        assert_eq!(
            root.usage_v1()
                .used
                .get(ResourceKindV1::ResidentHostAllocationBytes),
            8192
        );
        assert_eq!(
            root.usage_v1()
                .used
                .get(ResourceKindV1::ResidentDeviceAllocationBytes),
            8192
        );
        engine.release_device_memory(native).unwrap();
        assert_eq!(
            root.usage_v1().used.get(ResourceKindV1::AllocationRecords),
            1
        );
        engine
            .release(host, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        assert_eq!(
            root.usage_v1().used.get(ResourceKindV1::AllocationRecords),
            0
        );
    }
}

#[test]
fn compound_backing_model_sessions_share_device_pressure_but_not_quarantine_phase() {
    let (device, vm) = device_vm(1);
    for uncertain in [false, true] {
        let root = root(65536, 65536, 8);
        let mut first = configured(&root, device_budget(2), budget(2, 2, 4));
        let mut second = configured(&root, device_budget(2), budget(2, 2, 4));
        let host = first.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
        let native = second.allocate_device_memory(device, vm, 1, 4096).unwrap();
        for engine in [&first, &second] {
            assert_eq!(
                engine
                    .host_backing_account
                    .as_ref()
                    .unwrap()
                    .session_usage()
                    .unwrap()
                    .used
                    .get(ResourceKindV1::AllocationRecords),
                1
            );
        }
        let before = calls(&second);
        assert!(matches!(
            second.allocate::<HostVisibleCoherentGttV1>(1),
            Err(MemorySessionError::HostVisibleBackingCredits(
                ResourceCreditErrorV1::RecordCapacity
            ))
        ));
        assert_eq!(calls(&second), before);
        if uncertain {
            first.backend.fail_operation = Some("free");
            assert!(
                first
                    .release(host, SharedAllocationPhaseV1::CpuWritable)
                    .is_err()
            );
            drop(first);
            assert_eq!(root.usage_v1().quarantined_records, 1);
            assert_eq!(second.phase(), SharedMemorySessionPhaseV1::Active);
            assert!(matches!(
                second.allocate::<HostVisibleCoherentGttV1>(1),
                Err(MemorySessionError::HostVisibleBackingCredits(
                    ResourceCreditErrorV1::RecordCapacity
                ))
            ));
            second.release_device_memory(native).unwrap();
            let replacement = second.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
            second
                .release(replacement, SharedAllocationPhaseV1::CpuWritable)
                .unwrap();
            assert_eq!(
                root.usage_v1().used.get(ResourceKindV1::AllocationRecords),
                1
            );
        } else {
            first
                .release(host, SharedAllocationPhaseV1::CpuWritable)
                .unwrap();
            let replacement = second.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
            second.release_device_memory(native).unwrap();
            second
                .release(replacement, SharedAllocationPhaseV1::CpuWritable)
                .unwrap();
            assert_eq!(
                root.usage_v1().used.get(ResourceKindV1::AllocationRecords),
                0
            );
        }
    }
}

#[test]
fn compound_backing_install_is_atomic_on_identity_configuration_and_currentness_failure() {
    let (device, vm) = device_vm(1);
    for mode in 0..8 {
        let root = root(65536, 65536, 8);
        let mut engine = acquired();
        match mode {
            2 => engine
                .configure_host_visible_backing_budget_v1(device, vm, budget(4, 4, 8).host_budget())
                .unwrap(),
            3 => engine
                .configure_device_backing_budget_v1(device, vm, budget(4, 4, 8).device_budget())
                .unwrap(),
            4 => engine.host_backing_configuration_closed = true,
            5 => engine.device_backing_configuration_closed = true,
            6 => engine.host_backing_activity_started = true,
            7 => engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 1),
            _ => {}
        }
        let before_host = engine
            .host_backing_account
            .as_ref()
            .map(HostBackingAccountV1::usage);
        let before_device = engine
            .device_backing_account
            .as_ref()
            .map(DeviceBackingAccountV1::usage);
        let token = admission(
            &root,
            if mode == 0 { device_vm(2).0 } else { device },
            device_budget(8),
            budget(4, 4, 8),
        );
        let before = calls(&engine);
        assert!(
            engine
                .configure_native_backing_admission_v1(
                    device,
                    if mode == 1 { device_vm(2).1 } else { vm },
                    token
                )
                .is_err()
        );
        assert_eq!(
            engine
                .host_backing_account
                .as_ref()
                .map(HostBackingAccountV1::usage),
            before_host
        );
        assert_eq!(
            engine
                .device_backing_account
                .as_ref()
                .map(DeviceBackingAccountV1::usage),
            before_device
        );
        if mode != 7 {
            assert_eq!(calls(&engine), before);
        }
        assert_eq!(
            root.usage_v1().used.get(ResourceKindV1::AllocationRecords),
            0
        );
        let mut retry = configured(&root, device_budget(8), budget(4, 4, 8));
        assert!(
            retry
                .configure_host_visible_backing_budget_v1(device, vm, budget(4, 4, 8).host_budget())
                .is_err()
        );
        assert!(
            retry
                .configure_device_backing_budget_v1(device, vm, budget(4, 4, 8).device_budget())
                .is_err()
        );
    }
}

#[test]
fn compound_backing_mixed_disposal_error_and_unwind_keep_both_ancestor_charges() {
    let (device, vm) = device_vm(1);
    for host_failure in [false, true] {
        for panic in [false, true] {
            for boundary in 0..if host_failure { 7 } else { 5 } {
                let root = root(65536, 65536, 8);
                let observe = observe_lifetime(&root);
                let mut engine = configured(&root, device_budget(8), budget(4, 4, 8));
                let host = engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
                let native = engine.allocate_device_memory(device, vm, 1, 4096).unwrap();
                let before = root.usage_v1().used;
                drop(root);
                let operation = if boundary % 2 == 0 {
                    "currentness"
                } else if host_failure && boundary == 1 {
                    "unmap_cpu"
                } else if boundary == if host_failure { 3 } else { 1 } {
                    "free"
                } else {
                    "release_va_reservation"
                };
                if boundary % 2 == 0 {
                    let at = engine.backend.currentness_calls + boundary / 2 + 1;
                    if panic {
                        engine.backend.panic_currentness_at = Some(at);
                    } else {
                        engine.backend.fail_currentness_at = Some(at);
                    }
                } else if panic {
                    engine.backend.panic_operation = Some(operation);
                } else {
                    engine.backend.fail_operation = Some(operation);
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if host_failure {
                        engine.release(host, SharedAllocationPhaseV1::CpuWritable)
                    } else {
                        engine.release_device_memory(native)
                    }
                }));
                if panic {
                    assert_eq!(
                        result
                            .unwrap_err()
                            .downcast_ref::<(&'static str, &'static str)>(),
                        Some(&("N2 native panic", operation))
                    );
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert_eq!(observe().unwrap().used, before);
                let calls_before = calls(&engine);
                assert!(matches!(
                    engine.allocate::<HostVisibleCoherentGttV1>(1),
                    Err(MemorySessionError::SharedSessionQuarantined)
                ));
                assert!(matches!(
                    engine.allocate_device_memory(device, vm, 1, 4096),
                    Err(MemorySessionError::SharedSessionQuarantined)
                ));
                assert_eq!(calls(&engine), calls_before);
                drop(engine);
                assert_eq!(observe().unwrap().quarantined_records, 2);
                assert_eq!(observe().unwrap().used, before);
            }
        }
    }
}

#[test]
fn compound_backing_clean_disposal_keeps_registry_until_final_owner_drop() {
    let root = root(65536, 65536, 8);
    let observe = observe_lifetime(&root);
    let mut engine = configured(&root, device_budget(8), budget(4, 4, 8));
    let (device, vm) = device_vm(1);
    let host = engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
    let native = engine.allocate_device_memory(device, vm, 1, 4096).unwrap();
    drop(root);
    engine
        .release(host, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    assert_eq!(
        observe()
            .unwrap()
            .used
            .get(ResourceKindV1::AllocationRecords),
        1
    );
    engine.release_device_memory(native).unwrap();
    assert_eq!(
        observe()
            .unwrap()
            .used
            .get(ResourceKindV1::AllocationRecords),
        0
    );
    drop(engine);
    assert!(observe().is_none());
}
