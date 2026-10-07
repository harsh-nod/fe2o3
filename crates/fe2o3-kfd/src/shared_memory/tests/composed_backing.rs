use super::*;
use crate::resource_domains::composed_tests::{
    admission_for_device, budget, device_budget, observer, root,
};
use fe2o3_resource_accounting::{ResourceCreditErrorV1, ResourceKindV1 as K};

fn admission(
    root: &crate::Gfx942ComposedBackingRootV1,
    records: usize,
) -> crate::Gfx942ComposedBackingAdmissionV1 {
    admission_for_device(root, device_vm(1).0, device_budget(), budget(records))
}

fn configured(
    admission: crate::Gfx942ComposedBackingAdmissionV1,
) -> SharedMemoryEngine<FakeBackend> {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    engine
        .configure_composed_backing_admission_v1(device, vm, admission)
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

fn uninstalled(engine: &SharedMemoryEngine<FakeBackend>) {
    assert!(engine.composed_request_account.is_none());
    assert!(engine.host_backing_account.is_none());
    assert!(engine.device_backing_account.is_none());
}

#[test]
fn composed_backing_accepts_healthy_reserved_and_retained_requests() {
    for state in 0..3 {
        let root = root();
        let observe = observer(&root);
        let token = admission(&root, 8);
        let request = token.request_account_v1().clone();
        let reserved = (state == 1).then(|| request.reserve_v1(97).unwrap());
        let retained = (state == 2).then(|| request.reserve_v1(97).unwrap().retain());
        let before = root.usage_v1();
        let engine = configured(token);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
        assert_eq!(root.usage_v1(), before);
        assert!(engine.composed_request_account.is_some());
        assert!(engine.host_backing_account.is_some());
        assert!(engine.device_backing_account.is_some());
        drop(root);
        drop(request);
        drop(reserved);
        if let Some(credit) = retained {
            credit.release_after_rejection().unwrap();
        }
        assert_eq!(observe().unwrap().used.get(K::AllocationRecords), 0);
        drop(engine);
        assert!(observe().is_none());
    }
}

#[test]
fn composed_backing_combined_records_contend_and_refund_independently_in_all_orders() {
    let (device, vm) = device_vm(1);
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let root = root();
        let token = admission(&root, 3);
        let account = token.request_account_v1().clone();
        let mut engine = configured(token);
        let (mut request, mut host, mut native) = (None, None, None);
        for class in order {
            match class {
                0 => request = Some(account.reserve_v1(17).unwrap().retain()),
                1 => host = Some(engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap()),
                2 => native = Some(engine.allocate_device_memory(device, vm, 17, 4096).unwrap()),
                _ => unreachable!(),
            }
        }
        let before = calls(&engine);
        let usage = root.usage_v1();
        assert!(matches!(
            account.reserve_v1(1),
            Err(ResourceCreditErrorV1::RecordCapacity)
        ));
        assert!(matches!(
            engine.allocate::<HostVisibleCoherentGttV1>(1),
            Err(MemorySessionError::HostVisibleBackingCredits(
                ResourceCreditErrorV1::RecordCapacity
            ))
        ));
        assert!(matches!(
            engine.allocate_device_memory(device, vm, 1, 4096),
            Err(MemorySessionError::DeviceBackingCredits(
                ResourceCreditErrorV1::RecordCapacity
            ))
        ));
        assert_eq!(calls(&engine), before);
        assert_eq!(root.usage_v1(), usage);
        assert_eq!(account.session_usage_v1().used.get(K::AllocationRecords), 3);
        assert_eq!(usage.used.get(K::RequestedAllocationBytes), 17);
        assert_eq!(usage.used.get(K::ResidentHostAllocationBytes), 4096);
        assert_eq!(usage.used.get(K::ResidentDeviceAllocationBytes), 4096);
        for class in order {
            match class {
                0 => request.take().unwrap().release_after_rejection().unwrap(),
                1 => engine
                    .release(host.take().unwrap(), SharedAllocationPhaseV1::CpuWritable)
                    .unwrap(),
                2 => engine
                    .release_device_memory(native.take().unwrap())
                    .unwrap(),
                _ => unreachable!(),
            }
            let usage = root.usage_v1();
            assert_eq!(
                usage.used.get(K::RequestedAllocationBytes),
                if request.is_some() { 17 } else { 0 }
            );
            assert_eq!(
                usage.used.get(K::ResidentHostAllocationBytes),
                if host.is_some() { 4096 } else { 0 }
            );
            assert_eq!(
                usage.used.get(K::ResidentDeviceAllocationBytes),
                if native.is_some() { 4096 } else { 0 }
            );
            let replacement = account.reserve_v1(1).unwrap();
            drop(replacement);
            assert_eq!(root.usage_v1(), usage);
        }
    }
}

#[test]
fn composed_backing_device_and_root_record_pressure_cross_sessions() {
    use fe2o3_resource_accounting::ResourceVectorV1;

    let (device, vm) = device_vm(1);
    for device_limit in [false, true] {
        let root = crate::Gfx942ComposedBackingRootV1::new(
            ResourceVectorV1::ZERO
                .with(K::ControlResidentBytes, 1 << 20)
                .with(K::RequestedAllocationBytes, 65536)
                .with(K::ResidentHostAllocationBytes, 65536)
                .with(K::ResidentDeviceAllocationBytes, 65536)
                .with(K::AllocationRecords, if device_limit { 32 } else { 3 }),
            2,
            24,
            40,
        )
        .unwrap();
        let parent = crate::Gfx942ComposedBackingDeviceBudgetV1::new(
            65536,
            65536,
            65536,
            if device_limit { 3 } else { 16 },
        )
        .unwrap();
        let token = admission_for_device(&root, device, parent, budget(8));
        let request = token.request_account_v1().reserve_v1(31).unwrap().retain();
        let mut first = configured(token);
        let mut second = configured(admission_for_device(&root, device, parent, budget(8)));
        let host = first.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
        let native = second.allocate_device_memory(device, vm, 1, 4096).unwrap();
        let before = calls(&second);
        let usage = root.usage_v1();
        let expected = if device_limit {
            ResourceCreditErrorV1::RecordCapacity
        } else {
            ResourceCreditErrorV1::Capacity
        };
        assert!(matches!(second.allocate::<HostVisibleCoherentGttV1>(1),
            Err(MemorySessionError::HostVisibleBackingCredits(error)) if error == expected));
        assert_eq!(calls(&second), before);
        assert_eq!(root.usage_v1(), usage);
        request.release_after_rejection().unwrap();
        let replacement = second.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
        first
            .release(host, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        second
            .release(replacement, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        second.release_device_memory(native).unwrap();
        assert_eq!(root.usage_v1().used.get(K::AllocationRecords), 0);
    }
}

#[test]
fn composed_backing_install_failure_preserves_request_and_existing_configuration() {
    let (device, vm) = device_vm(1);
    for mode in 0..10 {
        let root = root();
        let mut engine = acquired();
        match mode {
            2 => engine
                .configure_host_visible_backing_budget_v1(device, vm, budget(8).host_budget())
                .unwrap(),
            3 => engine
                .configure_device_backing_budget_v1(device, vm, budget(8).device_budget())
                .unwrap(),
            4 => engine.host_backing_configuration_closed = true,
            5 => engine.device_backing_configuration_closed = true,
            6 => engine.host_backing_activity_started = true,
            7 => engine.device_backing_activity_started = true,
            8 => engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 1),
            9 => engine.backend.panic_currentness_at = Some(engine.backend.currentness_calls + 1),
            _ => {}
        }
        let token = admission_for_device(
            &root,
            if mode == 0 { device_vm(2).0 } else { device },
            device_budget(),
            budget(8),
        );
        let account = token.request_account_v1().clone();
        let credit = account.reserve_v1(31).unwrap().retain();
        let before = root.usage_v1();
        let before_calls = calls(&engine);
        let host = engine
            .host_backing_account
            .as_ref()
            .map(HostBackingAccountV1::usage);
        let native = engine
            .device_backing_account
            .as_ref()
            .map(DeviceBackingAccountV1::usage);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.configure_composed_backing_admission_v1(
                device,
                if mode == 1 { device_vm(2).1 } else { vm },
                token,
            )
        }));
        if mode == 9 {
            assert_eq!(
                result
                    .unwrap_err()
                    .downcast_ref::<(&'static str, &'static str)>(),
                Some(&("N2 native panic", "currentness"))
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(engine.composed_request_account.is_none());
        assert_eq!(
            engine
                .host_backing_account
                .as_ref()
                .map(HostBackingAccountV1::usage),
            host
        );
        assert_eq!(
            engine
                .device_backing_account
                .as_ref()
                .map(DeviceBackingAccountV1::usage),
            native
        );
        assert_eq!(root.usage_v1(), before);
        assert!(account.matches_retained_charge_v1(&credit, 31));
        if mode < 8 {
            assert_eq!(calls(&engine), before_calls);
        } else {
            assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
            assert_eq!(engine.backend.alloc_calls, 0);
        }
        credit.release_after_rejection().unwrap();
    }
}

#[test]
fn composed_backing_quarantine_before_and_during_install_never_commits_partial_accounts() {
    let (device, vm) = device_vm(1);
    for during in [false, true] {
        let root = root();
        let token = admission(&root, 8);
        let credit = token.request_account_v1().reserve_v1(63).unwrap().retain();
        let mut engine = acquired();
        let before = calls(&engine);
        if during {
            engine.backend.quarantine_request_at =
                Some((engine.backend.currentness_calls + 1, credit));
        } else {
            credit.quarantine();
        }
        let result = engine.configure_composed_backing_admission_v1(device, vm, token);
        uninstalled(&engine);
        assert_eq!(engine.backend.alloc_calls, 0);
        assert_eq!(engine.backend.reserve_va_calls, 0);
        if during {
            assert!(matches!(
                result,
                Err(MemorySessionError::SharedSessionQuarantined)
            ));
            assert_eq!(
                calls(&engine),
                (before.0 + 1, before.1, before.2, before.3, before.4)
            );
            assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        } else {
            assert!(matches!(
                result,
                Err(MemorySessionError::DeviceBackingCredits(
                    ResourceCreditErrorV1::Invariant
                ))
            ));
            assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
            assert_eq!(calls(&engine), before);
        }
        assert_eq!(root.usage_v1().quarantined_records, 1);
        assert_eq!(root.usage_v1().used.get(K::RequestedAllocationBytes), 63);
    }
}

#[test]
fn composed_backing_request_quarantine_seals_selected_session_not_sibling() {
    let (device, vm) = device_vm(1);
    let root = root();
    let token = admission(&root, 8);
    let credit = token.request_account_v1().reserve_v1(31).unwrap().retain();
    let mut engine = configured(token);
    let mut sibling = configured(admission(&root, 8));
    credit.quarantine();
    let before = calls(&engine);
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert!(matches!(
        engine.allocate::<HostVisibleCoherentGttV1>(1),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert!(matches!(
        engine.allocate_device_memory(device, vm, 1, 4096),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert_eq!(calls(&engine), before);
    assert_eq!(sibling.phase(), SharedMemorySessionPhaseV1::Active);
    let host = sibling.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
    let native = sibling.allocate_device_memory(device, vm, 1, 4096).unwrap();
    sibling
        .release(host, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    sibling.release_device_memory(native).unwrap();
    assert_eq!(root.usage_v1().used.get(K::AllocationRecords), 1);
}

#[test]
fn composed_backing_outstanding_request_can_outlive_clean_native_engine() {
    for retained in [false, true] {
        let root = root();
        let observe = observer(&root);
        let token = admission(&root, 8);
        let reservation = token.request_account_v1().reserve_v1(31).unwrap();
        let engine = configured(token);
        drop(root);
        drop(engine);
        assert_eq!(observe().unwrap().used.get(K::RequestedAllocationBytes), 31);
        if retained {
            reservation.retain().release_after_rejection().unwrap();
        } else {
            drop(reservation);
        }
        assert!(observe().is_none());
    }
}

#[test]
fn composed_backing_failed_install_without_external_credit_releases_registry() {
    let (device, vm) = device_vm(1);
    for panic in [false, true] {
        let root = root();
        let observe = observer(&root);
        let token = admission(&root, 8);
        drop(root);
        let mut engine = acquired();
        let at = engine.backend.currentness_calls + 1;
        if panic {
            engine.backend.panic_currentness_at = Some(at);
        } else {
            engine.backend.fail_currentness_at = Some(at);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.configure_composed_backing_admission_v1(device, vm, token)
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        uninstalled(&engine);
        assert!(observe().is_none());
    }
}

#[test]
fn composed_backing_engine_drop_quarantines_native_records_and_retains_registry() {
    let (device, vm) = device_vm(1);
    let root = root();
    let observe = observer(&root);
    let mut engine = configured(admission(&root, 8));
    let _host = engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
    let _native = engine.allocate_device_memory(device, vm, 1, 4096).unwrap();
    let before = root.usage_v1().used;
    drop(root);
    drop(engine);
    let after = observe().unwrap();
    assert_eq!(after.used, before);
    assert_eq!(after.quarantined_records, 2);
    assert_eq!(after.used.get(K::RequestedAllocationBytes), 0);
    assert_eq!(after.used.get(K::ResidentHostAllocationBytes), 4096);
    assert_eq!(after.used.get(K::ResidentDeviceAllocationBytes), 4096);
}

#[test]
fn composed_backing_queue_owned_and_loaned_configuration_reject_without_effects() {
    for loaned in [false, true] {
        let mut fixture = BackingConstructorFixture::new(None);
        let mut queue = fixture.transfer(&[]).unwrap();
        let loan = loaned.then(|| {
            fixture
                .ownership
                .loan_foundation(
                    fixture.engine.session_id,
                    &mut fixture.foundation,
                    &mut queue,
                    fixture.device,
                    fixture.vm,
                )
                .unwrap()
        });
        let root = root();
        let token = admission_for_device(
            &root,
            fixture.device.model_key(),
            device_budget(),
            budget(8),
        );
        let before = calls(&fixture.engine);
        let usage = root.usage_v1();
        assert!(matches!(
            fixture.ownership.configure_composed_backing(
                &mut fixture.engine,
                fixture.device,
                fixture.vm,
                token,
            ),
            Err(MemorySessionError::DeviceBackingBudgetConfiguration(_))
        ));
        assert_eq!(calls(&fixture.engine), before);
        assert_eq!(root.usage_v1(), usage);
        uninstalled(&fixture.engine);
        if let Some(loan) = loan {
            fixture
                .ownership
                .reclaim_foundation(
                    fixture.engine.session_id,
                    &mut fixture.foundation,
                    &mut queue,
                    fixture.device,
                    fixture.vm,
                    loan,
                )
                .unwrap();
        }
    }
}

#[test]
fn composed_backing_native_disposal_error_and_unwind_preserve_all_custody() {
    let (device, vm) = device_vm(1);
    for host_failure in [false, true] {
        for panic in [false, true] {
            for boundary in 0..if host_failure { 7 } else { 5 } {
                let root = root();
                let observe = observer(&root);
                let token = admission(&root, 8);
                let request = token.request_account_v1().reserve_v1(31).unwrap().retain();
                let mut engine = configured(token);
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
                    assert!(result.is_err());
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
                assert_eq!(observe().unwrap().used, before);
                drop(request);
                drop(engine);
                assert_eq!(observe().unwrap().quarantined_records, 3);
                assert_eq!(observe().unwrap().used, before);
            }
        }
    }
}
