//! Actual PUBLIC allocation/accounting driver with scripted native leaves.

use super::*;

fn configured(bytes: u64, records: usize) -> SharedMemoryEngine<FakeBackend> {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    engine
        .configure_device_backing_budget_v1(
            device,
            vm,
            Gfx942DeviceBackingBudgetV1::new(bytes, records).unwrap(),
        )
        .unwrap();
    engine
}

fn allocate(
    engine: &mut SharedMemoryEngine<FakeBackend>,
    bytes: u64,
) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>, Gfx942XgmiAllocationFailureV1>
{
    let (device, vm) = device_vm(1);
    engine.allocate_xgmi_device_memory_classified_v1(device, vm, bytes, 1)
}

fn usage(engine: &SharedMemoryEngine<FakeBackend>) -> Gfx942DeviceBackingUsageV1 {
    engine.device_backing_account.as_ref().unwrap().usage()
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
fn xgmi_backing_pressure_preserves_existing_owner_and_allows_release_retry() {
    for (bytes, records, next_bytes) in [(4096, 2, 1), (8192, 1, 1), (8192, 2, 4097)] {
        let mut engine = configured(bytes, records);
        let first = allocate(&mut engine, 1).unwrap();
        let first_id = first.id;
        assert_eq!(first.layout.backing_bytes, 4096);
        assert_eq!(
            first.layout.uapi_flags,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC.bits()
        );
        let before = usage(&engine);
        let before_calls = calls(&engine);
        let failure = allocate(&mut engine, next_bytes).unwrap_err();
        assert_eq!(
            failure.disposition(),
            Gfx942XgmiAllocationDispositionV1::RejectedCapacity
        );
        assert!(matches!(
            failure.error(),
            MemorySessionError::DeviceBackingCredits(
                fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity
            )
        ));
        assert_eq!(usage(&engine), before);
        assert_eq!(calls(&engine), before_calls);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
        assert_eq!(engine.device_memory.len(), 1);
        assert_eq!(engine.device_memory[0].id, first_id);
        assert_eq!(engine.next_device_memory_id, first_id + 1);
        engine.release_device_memory(first).unwrap();
        assert_eq!(usage(&engine).used_backing_bytes, 0);
        let next = allocate(&mut engine, next_bytes).unwrap();
        assert!(next.id > first_id);
        engine.release_device_memory(next).unwrap();
        assert_eq!(usage(&engine).used_allocation_records, 0);
    }
}

#[test]
fn xgmi_backing_endpoint_pressure_does_not_charge_or_stall_peer() {
    for selected in 0..2 {
        let mut endpoints = [configured(4096, 1), configured(8192, 2)];
        let mut owned = [Vec::new(), Vec::new()];
        for _ in 0..=selected {
            owned[selected].push(allocate(&mut endpoints[selected], 1).unwrap());
        }
        let peer = 1 - selected;
        let peer_before = usage(&endpoints[peer]);
        let peer_calls = calls(&endpoints[peer]);
        assert_eq!(
            allocate(&mut endpoints[selected], 1)
                .unwrap_err()
                .disposition(),
            Gfx942XgmiAllocationDispositionV1::RejectedCapacity
        );
        assert_eq!(usage(&endpoints[peer]), peer_before);
        assert_eq!(calls(&endpoints[peer]), peer_calls);
        owned[peer].push(allocate(&mut endpoints[peer], 1).unwrap());
        let selected_before = usage(&endpoints[selected]);
        endpoints[peer]
            .release_device_memory(owned[peer].pop().unwrap())
            .unwrap();
        assert_eq!(usage(&endpoints[selected]), selected_before);
        for index in 0..2 {
            for lease in owned[index].drain(..) {
                endpoints[index].release_device_memory(lease).unwrap();
            }
        }
    }
}

#[test]
fn xgmi_backing_fixed_profile_capacity_is_pre_effect() {
    let mut engine = acquired();
    let huge = allocate(&mut engine, MAX_GFX942_DEVICE_MEMORY_BYTES_V1).unwrap();
    let before = calls(&engine);
    let failure = allocate(&mut engine, 1).unwrap_err();
    assert_eq!(
        failure.disposition(),
        Gfx942XgmiAllocationDispositionV1::RejectedCapacity
    );
    assert!(matches!(
        failure.error(),
        MemorySessionError::DeviceMemoryByteCapacity { .. }
    ));
    assert_eq!(calls(&engine), before);
    engine.release_device_memory(huge).unwrap();

    let mut engine = configured(
        MAX_GFX942_DEVICE_MEMORY_BYTES_V1,
        MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1,
    );
    let mut leases = Vec::new();
    for _ in 0..MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1 {
        leases.push(allocate(&mut engine, 1).unwrap());
    }
    let before = calls(&engine);
    let debit = usage(&engine);
    let failure = allocate(&mut engine, 1).unwrap_err();
    assert_eq!(
        failure.disposition(),
        Gfx942XgmiAllocationDispositionV1::RejectedCapacity
    );
    assert!(matches!(
        failure.error(),
        MemorySessionError::DeviceMemoryAllocationCapacity { .. }
    ));
    assert_eq!(calls(&engine), before);
    assert_eq!(usage(&engine), debit);
    let prior = leases.pop().unwrap();
    let prior_id = prior.id;
    engine.release_device_memory(prior).unwrap();
    let next = allocate(&mut engine, 1).unwrap();
    assert!(next.id > prior_id);
    leases.push(next);
    for lease in leases {
        engine.release_device_memory(lease).unwrap();
    }
    assert_eq!(usage(&engine).used_backing_bytes, 0);
}

#[test]
fn xgmi_backing_capacity_shaped_currentness_and_native_failures_never_retry() {
    for operation in ["currentness", "reserve_va", "alloc"] {
        let mut engine = configured(8192, 2);
        engine.backend.capacity_error_operation = Some(operation);
        let failure = allocate(&mut engine, 1).unwrap_err();
        assert!(matches!(
            failure.error(),
            MemorySessionError::DeviceBackingCredits(
                fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity
            )
        ));
        assert_eq!(
            failure.disposition(),
            Gfx942XgmiAllocationDispositionV1::ProcessTeardown
        );
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(
            usage(&engine).used_backing_bytes,
            if operation == "currentness" { 0 } else { 4096 }
        );
        assert_eq!(
            engine.backend.reserve_va_calls,
            usize::from(operation != "currentness")
        );
        assert_eq!(
            engine.backend.alloc_calls,
            usize::from(operation == "alloc")
        );
        let before = calls(&engine);
        engine.backend.capacity_error_operation = None;
        assert_eq!(
            allocate(&mut engine, 1).unwrap_err().disposition(),
            Gfx942XgmiAllocationDispositionV1::ProcessTeardown
        );
        assert_eq!(calls(&engine), before);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }
}

#[test]
fn xgmi_backing_geometry_domain_and_identity_failures_are_not_capacity() {
    let (device, vm) = device_vm(1);
    for (bytes, alignment) in [(0, 1), (1, 0), (1, 3), (1, 8192), (u64::MAX, 4096)] {
        let mut engine = configured(8192, 2);
        let before = calls(&engine);
        let failure = engine
            .allocate_xgmi_device_memory_classified_v1(device, vm, bytes, alignment)
            .unwrap_err();
        assert_eq!(
            failure.disposition(),
            Gfx942XgmiAllocationDispositionV1::ProcessTeardown
        );
        assert_eq!(calls(&engine), before);
        assert_eq!(usage(&engine).used_backing_bytes, 0);
    }
    let mut engine = configured(8192, 2);
    let (_, foreign_vm) = device_vm(2);
    let before = calls(&engine);
    let failure = engine
        .allocate_xgmi_device_memory_classified_v1(device, foreign_vm, 1, 1)
        .unwrap_err();
    assert!(matches!(
        failure.into_error(),
        MemorySessionError::InvalidDeviceMemoryAuthority
    ));
    assert_eq!(calls(&engine), before);
    engine.next_device_memory_id = u64::MAX;
    let failure = allocate(&mut engine, 1).unwrap_err();
    assert_eq!(
        failure.disposition(),
        Gfx942XgmiAllocationDispositionV1::ProcessTeardown
    );
    assert!(matches!(
        failure.into_error(),
        MemorySessionError::SizeOverflow
    ));
    assert_eq!(calls(&engine), before);
}

#[test]
fn xgmi_backing_failed_disposal_retains_charge_without_retry() {
    for failure in 0..3 {
        let mut engine = configured(4096, 1);
        let lease = allocate(&mut engine, 1).unwrap();
        let debit = usage(&engine);
        match failure {
            0 => engine.backend.fail_operation = Some("free"),
            1 => engine.backend.fail_operation = Some("release_va_reservation"),
            _ => engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 3),
        }
        assert!(engine.release_device_memory(lease).is_err());
        assert_eq!(usage(&engine), debit);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        let before = calls(&engine);
        assert_eq!(
            allocate(&mut engine, 1).unwrap_err().disposition(),
            Gfx942XgmiAllocationDispositionV1::ProcessTeardown
        );
        assert_eq!(calls(&engine), before);
    }
}

#[test]
fn xgmi_backing_peer_mapping_keeps_single_home_charge() {
    for selected in 0..2 {
        let mut endpoints = [configured(4096, 1), configured(8192, 2)];
        let lease = allocate(&mut endpoints[selected], 1).unwrap();
        let before = endpoints.each_ref().map(usage);
        let mapped = endpoints[selected]
            .map_device_memory_to_gpus(lease, Box::new([7, 9]))
            .ok()
            .unwrap();
        assert_eq!(endpoints.each_ref().map(usage), before);
        let lease = endpoints[selected]
            .unmap_device_memory_from_gpus(mapped)
            .ok()
            .unwrap();
        assert_eq!(endpoints.each_ref().map(usage), before);
        endpoints[selected].release_device_memory(lease).unwrap();
        assert_eq!(usage(&endpoints[selected]).used_allocation_records, 0);
        assert_eq!(usage(&endpoints[1 - selected]), before[1 - selected]);
    }
}

#[test]
fn xgmi_backing_allocation_unwind_preserves_payload_and_retained_debit() {
    for operation in ["currentness", "reserve_va", "alloc", "closing_currentness"] {
        let mut engine = configured(8192, 2);
        if operation == "closing_currentness" {
            engine.backend.panic_currentness_at = Some(engine.backend.currentness_calls + 2);
        } else {
            engine.backend.panic_operation = Some(operation);
        }
        let payload =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| allocate(&mut engine, 1)))
                .unwrap_err();
        let expected = if operation == "closing_currentness" {
            "currentness"
        } else {
            operation
        };
        assert_eq!(
            payload.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", expected))
        );
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(
            usage(&engine).used_backing_bytes,
            if operation == "currentness" { 0 } else { 4096 }
        );
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }
}

#[test]
fn xgmi_backing_malformed_output_and_closing_failure_retain_native_record() {
    for failure in 0..3 {
        let mut engine = configured(8192, 2);
        match failure {
            0 => engine.backend.corrupt_flags = true,
            1 => engine.backend.alloc_oom = true,
            _ => engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 2),
        }
        let error = allocate(&mut engine, 1).unwrap_err();
        assert_eq!(
            error.disposition(),
            Gfx942XgmiAllocationDispositionV1::ProcessTeardown
        );
        match failure {
            0 => assert!(matches!(
                error.error(),
                MemorySessionError::KernelResultMalformed(_)
            )),
            1 => assert!(matches!(
                error.error(),
                MemorySessionError::Syscall {
                    operation: "AMDKFD_IOC_ALLOC_MEMORY_OF_GPU",
                    source: rustix::io::Errno::NOMEM,
                }
            )),
            _ => assert!(matches!(
                error.error(),
                MemorySessionError::Injected("currentness")
            )),
        }
        assert_eq!(usage(&engine).used_backing_bytes, 4096);
        assert_eq!(usage(&engine).retained_records, 1);
        assert_eq!(engine.device_memory.len(), 1);
        assert!(engine.device_memory[0].reservation.is_some());
        assert!(engine.device_memory[0].backing_charge.is_some());
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        let before = calls(&engine);
        assert_eq!(
            allocate(&mut engine, 1).unwrap_err().disposition(),
            Gfx942XgmiAllocationDispositionV1::ProcessTeardown
        );
        assert_eq!(calls(&engine), before);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }
}
