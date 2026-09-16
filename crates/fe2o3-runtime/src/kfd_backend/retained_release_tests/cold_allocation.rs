//! Opt-in native settlement probes; isolation acknowledgement is not exclusivity proof.

use super::*;
use crate::{RuntimeContextV1, RuntimeErrorV1, RuntimeResourceKindV1, RuntimeResourceVectorV1};

const PAGE_BYTES: u64 = 4096;
const HOST_BUDGET_BYTES: u64 = 16 * 1024 * 1024;

fn cold_probe_device(selector: Option<&str>, isolation: Option<&str>) -> Result<u64, &'static str> {
    if isolation != Some("1") {
        return Err("FE2O3_TEST_NATIVE_ISOLATED=1 acknowledgement required");
    }
    let selector = selector.ok_or("explicit device unique ID required")?;
    let digits = selector.strip_prefix("0x").unwrap_or(selector);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("device unique ID must be hexadecimal");
    }
    let device = u64::from_str_radix(digits, 16).map_err(|_| "device unique ID overflows u64")?;
    if device == 0 {
        return Err("device unique ID must be nonzero");
    }
    Ok(device)
}

#[test]
fn cold_allocation_probe_guard_requires_explicit_identity_and_isolation_acknowledgement() {
    for acknowledgement in [
        None,
        Some(""),
        Some("0"),
        Some("true"),
        Some(" 1"),
        Some("1 "),
    ] {
        assert_eq!(
            cold_probe_device(Some("0x1234"), acknowledgement),
            Err("FE2O3_TEST_NATIVE_ISOLATED=1 acknowledgement required")
        );
    }
    for selector in [
        None,
        Some(""),
        Some("0"),
        Some("0x0"),
        Some("0x"),
        Some("-1"),
        Some("+1"),
        Some(" 1"),
        Some("1 "),
        Some("0xgg"),
        Some("10000000000000000"),
    ] {
        assert!(
            cold_probe_device(selector, Some("1")).is_err(),
            "selector={selector:?}"
        );
    }
    for (selector, expected) in [
        ("1234", 0x1234),
        ("0x000A", 10),
        ("AbCd", 0xabcd),
        ("ffffffffffffffff", u64::MAX),
    ] {
        assert_eq!(cold_probe_device(Some(selector), Some("1")), Ok(expected));
    }
}

fn assert_empty_request_state(backend: &KfdRuntimeBackendV1) {
    assert!(backend.native_available && backend.scripted_sdma.is_none());
    assert!(!backend.terminal && !backend.queue_retired);
    assert!(backend.allocations.is_empty());
    assert_eq!(backend.staged_context_bytes, 0);
    assert!(backend.terminal_memory.is_none());
    assert!(backend.terminal_sdma_custody.is_none());
    assert!(backend.primary_teardown.is_none());
}

fn pool_observation(backend: &KfdRuntimeBackendV1) -> Gfx942SdmaMemoryPoolObservationV1 {
    backend
        .queue
        .as_ref()
        .unwrap()
        .sdma_memory_pool_observation()
        .unwrap()
}

fn native_cold_allocation_settlement(kind: RuntimeMemoryKindV1) {
    let selector = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_ID").ok();
    let isolation = std::env::var("FE2O3_TEST_NATIVE_ISOLATED").ok();
    let native_device = cold_probe_device(selector.as_deref(), isolation.as_deref())
        .expect("native cold-allocation probe guard");
    let mut backend =
        KfdRuntimeBackendV1::open_default(native_device, TestPanickingAuthorityV1).unwrap();
    backend
        .configure_host_visible_backing_budget_v1(
            Gfx942HostVisibleBackingBudgetV1::new(HOST_BUDGET_BYTES, 32).unwrap(),
        )
        .unwrap();
    backend
        .configure_device_backing_budget_v1(
            Gfx942DeviceBackingBudgetV1::new(PAGE_BYTES, 1).unwrap(),
        )
        .unwrap();
    backend
        .configure_host_pool_limits_v1(fe2o3_kfd::Gfx942HostPoolLimitsV1::new(0, 0).unwrap())
        .unwrap();
    backend
        .configure_device_pool_limits_v1(Gfx942DevicePoolLimitsV1::new(0, 0).unwrap())
        .unwrap();
    let rejected_bytes = match kind {
        RuntimeMemoryKindV1::DeviceLocal => PAGE_BYTES + 1,
        RuntimeMemoryKindV1::HostVisible => HOST_BUDGET_BYTES + PAGE_BYTES,
    };
    assert!(rejected_bytes <= backend.staging_budgets.max_allocation_bytes);
    assert!(rejected_bytes <= backend.staging_budgets.max_context_bytes);
    let mut context = RuntimeContextV1::open(backend).unwrap();
    let device = context.devices()[0].id();
    context
        .configure_allocation_admission_v1(device, rejected_bytes, 1)
        .unwrap();
    let empty = context
        .allocation_admission_usage_v1(device)
        .unwrap()
        .unwrap();
    assert_eq!(empty.used, RuntimeResourceVectorV1::ZERO);
    assert_eq!(
        (
            empty.reserved_records,
            empty.retained_records,
            empty.quarantined_records
        ),
        (0, 0, 0)
    );
    assert!(!empty.poisoned);
    assert_empty_request_state(context.backend());
    assert!(context.backend().queue.is_none() && !context.backend().sdma_enabled);
    assert!(context.backend().host_visible_backing_usage_v1().is_none());
    assert!(context.backend().device_backing_usage_v1().is_none());
    let first_id = context.backend().next_handle;

    let Err(RuntimeErrorV1::BackendQuiescent(cold_error)) =
        context.allocate(device, kind, rejected_bytes, PAGE_BYTES)
    else {
        panic!("cold backing capacity must settle with a quiescent public error");
    };
    assert_eq!(cold_error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
    assert!(!context.is_terminal());
    assert_eq!(
        context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap(),
        empty
    );
    let backend = context.backend();
    assert_empty_request_state(backend);
    assert!(backend.sdma_allocation_ready_v1());
    assert_eq!(backend.next_handle, first_id + 1);
    let primary = backend.queue.as_ref().unwrap().observation();
    let lane = backend.queue.as_ref().unwrap().primary_compute_lane_v1();
    let host_baseline = backend.host_visible_backing_usage_v1().unwrap();
    let device_baseline = backend.device_backing_usage_v1().unwrap();
    assert!(host_baseline.used_backing_bytes > 0 && host_baseline.used_allocation_records > 0);
    assert_eq!(host_baseline.reserved_records, 0);
    assert_eq!(host_baseline.quarantined_records, 0);
    assert_eq!(
        host_baseline.retained_records as u64,
        host_baseline.used_allocation_records
    );
    assert!(!host_baseline.poisoned && !device_baseline.poisoned);
    assert_eq!(
        (
            device_baseline.used_backing_bytes,
            device_baseline.used_allocation_records,
            device_baseline.reserved_records,
            device_baseline.retained_records,
            device_baseline.quarantined_records
        ),
        (0, 0, 0, 0, 0)
    );
    let empty_pool = pool_observation(backend);
    assert_eq!(
        (
            empty_pool.checked_out_buffers,
            empty_pool.retained_free_buffers,
            empty_pool.retained_free_bytes,
            empty_pool.reuse_count
        ),
        (0, 0, 0, 0)
    );

    let Err(RuntimeErrorV1::BackendRejected(warm_error)) =
        context.allocate(device, kind, rejected_bytes, PAGE_BYTES)
    else {
        panic!("established-route backing capacity must be rejected before allocation");
    };
    assert_eq!(warm_error, cold_error);
    assert!(!context.is_terminal());
    assert_eq!(
        context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap(),
        empty
    );
    let backend = context.backend();
    assert_empty_request_state(backend);
    assert!(backend.sdma_allocation_ready_v1());
    assert_eq!(backend.next_handle, first_id + 2);
    assert_eq!(backend.queue.as_ref().unwrap().observation(), primary);
    assert_eq!(
        backend.queue.as_ref().unwrap().primary_compute_lane_v1(),
        lane
    );
    assert_eq!(backend.host_visible_backing_usage_v1(), Some(host_baseline));
    assert_eq!(backend.device_backing_usage_v1(), Some(device_baseline));
    assert_eq!(pool_observation(backend), empty_pool);
    assert!(context.cleanup().is_complete());

    let local = context.backend().next_handle;
    let allocation = context
        .allocate(device, kind, PAGE_BYTES, PAGE_BYTES)
        .unwrap();
    let usage = context
        .allocation_admission_usage_v1(device)
        .unwrap()
        .unwrap();
    assert_eq!(
        usage.used,
        RuntimeResourceVectorV1::ZERO
            .with(RuntimeResourceKindV1::RequestedAllocationBytes, PAGE_BYTES)
            .with(RuntimeResourceKindV1::AllocationRecords, 1)
    );
    assert_eq!(
        (
            usage.reserved_records,
            usage.retained_records,
            usage.quarantined_records
        ),
        (0, 1, 0)
    );
    assert!(!usage.poisoned);
    let backend = context.backend();
    assert_eq!(backend.allocations.len(), 1);
    assert_eq!(backend.staged_context_bytes, PAGE_BYTES);
    let record = &backend.allocations[&local];
    assert!(record.sdma_backed && record.sdma_initialized);
    assert_eq!(
        (record.kind, record.alignment, record.bytes.len()),
        (kind, PAGE_BYTES, PAGE_BYTES as usize)
    );
    match (&record.sdma_storage, kind) {
        (
            KfdRuntimeSdmaStorageV1::Host(SdmaBufferOwnerV1::Native(owner)),
            RuntimeMemoryKindV1::HostVisible,
        ) => {
            assert_eq!(owner.requested_bytes(), PAGE_BYTES);
        }
        (KfdRuntimeSdmaStorageV1::Device(owner), RuntimeMemoryKindV1::DeviceLocal) => {
            let DirectionalSdmaDeviceOwnerV1::Native(owner) = &**owner else {
                panic!("native device owner required");
            };
            assert_eq!(
                (owner.byte_len(), owner.physical_byte_len()),
                (PAGE_BYTES, PAGE_BYTES)
            );
        }
        _ => panic!("native owner of the requested memory kind required"),
    }
    let mut allocated_host = host_baseline;
    let mut allocated_device = device_baseline;
    if kind == RuntimeMemoryKindV1::HostVisible {
        allocated_host.used_backing_bytes += PAGE_BYTES;
        allocated_host.used_allocation_records += 1;
        allocated_host.retained_records += 1;
    } else {
        allocated_device.used_backing_bytes += PAGE_BYTES;
        allocated_device.used_allocation_records += 1;
        allocated_device.retained_records += 1;
    }
    assert_eq!(
        backend.host_visible_backing_usage_v1(),
        Some(allocated_host)
    );
    assert_eq!(backend.device_backing_usage_v1(), Some(allocated_device));
    let mut allocated_pool = empty_pool;
    allocated_pool.checked_out_buffers = 1;
    assert_eq!(pool_observation(backend), allocated_pool);

    let mut readback = vec![0xa5; PAGE_BYTES as usize];
    context
        .read_allocation(allocation, 0, &mut readback)
        .unwrap();
    assert_eq!(readback, vec![0; PAGE_BYTES as usize]);
    readback.fill(0xa5);
    assert!(
        context
            .backend_mut_for_test_v1()
            .download_sdma_range_v1(local, 0, &mut readback)
            .unwrap()
    );
    assert_eq!(readback, vec![0; PAGE_BYTES as usize]);
    let expected = (0..PAGE_BYTES as usize)
        .map(|index| ((index * 17 + 3) % 251) as u8)
        .collect::<Vec<_>>();
    context.write_allocation(allocation, 0, &expected).unwrap();
    readback.fill(0xa5);
    context
        .read_allocation(allocation, 0, &mut readback)
        .unwrap();
    assert_eq!(readback, expected);
    // A controlled adapter-only write makes native storage differ from the
    // retained shadow. This tests public read routing, not an all-facade workflow.
    let native_only = expected.iter().map(|byte| byte ^ 0x5a).collect::<Vec<_>>();
    context
        .backend_mut_for_test_v1()
        .upload_sdma_range_v1(local, 0, &native_only)
        .unwrap();
    assert_eq!(&*context.backend().allocations[&local].bytes, expected);
    readback.fill(0xa5);
    context
        .read_allocation(allocation, 0, &mut readback)
        .unwrap();
    assert_eq!(readback, native_only);
    // Independently require native-adapter handling; a shadow fallback returns false.
    readback.fill(0xa5);
    assert!(
        context
            .backend_mut_for_test_v1()
            .download_sdma_range_v1(local, 0, &mut readback)
            .unwrap()
    );
    assert_eq!(readback, native_only);
    context
        .write_allocation(allocation, 0, &native_only)
        .unwrap();
    assert_eq!(&*context.backend().allocations[&local].bytes, native_only);
    assert_eq!(
        context.backend().host_visible_backing_usage_v1(),
        Some(allocated_host)
    );
    assert_eq!(
        context.backend().device_backing_usage_v1(),
        Some(allocated_device)
    );
    assert_eq!(pool_observation(context.backend()), allocated_pool);

    context.release_allocation(allocation).unwrap();
    assert_eq!(
        context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap(),
        empty
    );
    assert_empty_request_state(context.backend());
    assert_eq!(
        context.backend().host_visible_backing_usage_v1(),
        Some(host_baseline)
    );
    assert_eq!(
        context.backend().device_backing_usage_v1(),
        Some(device_baseline)
    );
    assert_eq!(pool_observation(context.backend()), empty_pool);
    assert!(context.cleanup().is_complete());
    let mut backend = context.shutdown().unwrap();
    let shutdown = observe_shutdown(&mut backend);
    assert_eq!(shutdown.before, Some(host_baseline));
    assert_eq!(shutdown.device_before, Some(device_baseline));
    assert_eq!(shutdown.device_completed, Some(device_baseline));
    assert_retired_shutdown_is_inert(&mut backend);
    drop(backend);
    println!(
        "native_cold_allocation_settlement=complete kind={kind:?} rejected_bytes={rejected_bytes} retry_bytes={PAGE_BYTES} cold={cold_error:?} host_baseline={host_baseline:?} device_baseline={device_baseline:?} shutdown={shutdown:?} native_readback_sha256={}",
        Sha256::digest(&readback)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
}

#[test]
#[ignore = "requires isolated MI300X, FE2O3_TEST_NATIVE_UNIQUE_ID and FE2O3_TEST_NATIVE_ISOLATED=1"]
fn native_runtime_cold_host_capacity_refunds_context_and_retries() {
    native_cold_allocation_settlement(RuntimeMemoryKindV1::HostVisible);
}

#[test]
#[ignore = "requires isolated MI300X, FE2O3_TEST_NATIVE_UNIQUE_ID and FE2O3_TEST_NATIVE_ISOLATED=1"]
fn native_runtime_cold_device_capacity_refunds_context_and_retries() {
    native_cold_allocation_settlement(RuntimeMemoryKindV1::DeviceLocal);
}
