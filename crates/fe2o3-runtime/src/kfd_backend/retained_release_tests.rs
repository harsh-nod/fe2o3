//! Shutdown settlement tests and opt-in public native allocation workflow.

use super::*;
use crate::RuntimeOwnedShutdownBackendV1;
use fe2o3_kfd::{Gfx942DeviceBackingUsageV1, Gfx942HostVisibleBackingUsageV1};
use std::cell::Cell;

mod cold_allocation;
#[cfg(feature = "hardware-qualification")]
mod copy_accounting;
#[cfg(feature = "hardware-qualification")]
mod mixed_duration;
#[cfg(all(test, feature = "hardware-qualification"))]
pub(super) mod primary_envelope;
#[cfg(feature = "hardware-qualification")]
mod producer_launch;

thread_local! { static SELECTION: Cell<Option<bool>> = const { Cell::new(None) }; }
thread_local! { static PRIMARY_TEARDOWN_CAPACITY: Cell<bool> = const { Cell::new(false) }; }

pub(super) fn reject_primary_teardown_shell() -> bool {
    PRIMARY_TEARDOWN_CAPACITY.with(|slot| slot.replace(false))
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PrimaryHostUsage {
    before: Option<Gfx942HostVisibleBackingUsageV1>,
    completed: Option<Gfx942HostVisibleBackingUsageV1>,
    device_before: Option<Gfx942DeviceBackingUsageV1>,
    device_completed: Option<Gfx942DeviceBackingUsageV1>,
    observations: [usize; 2],
}

thread_local! {
    static PRIMARY_HOST_USAGE: Cell<Option<PrimaryHostUsage>> = const { Cell::new(None) };
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MaterializationRetention {
    calls: usize,
    spec_count: usize,
    retained_count: usize,
    layouts: [Option<fe2o3_kfd::Gfx942FixedDispatchDataLayoutV1>; 3],
    failure_usage: Option<Gfx942HostVisibleBackingUsageV1>,
}

thread_local! {
    static MATERIALIZATION_RETENTION: Cell<Option<MaterializationRetention>> = const { Cell::new(None) };
}

pub(super) fn observe_materialization_failure_usage(
    usage: Option<Gfx942HostVisibleBackingUsageV1>,
) {
    MATERIALIZATION_RETENTION.with(|slot| {
        if let Some(mut observation) = slot.get() {
            observation.failure_usage = usage;
            slot.set(Some(observation));
        }
    });
}

pub(super) fn observe_materialization_retention(
    specs: &[DataSpecV1],
    data: &[Gfx942FixedDispatchDataV1],
) {
    MATERIALIZATION_RETENTION.with(|slot| {
        if let Some(mut observation) = slot.get() {
            observation.calls = observation.calls.saturating_add(1);
            observation.spec_count = specs.len();
            observation.retained_count = data.len();
            for (layout, data) in observation.layouts.iter_mut().zip(data) {
                *layout = Some(data.layout());
            }
            slot.set(Some(observation));
        }
    });
}

pub(super) fn observe_primary_host_usage(owner: &PrimaryQueueReleaseCustodyV1, completed: bool) {
    PRIMARY_HOST_USAGE.with(|slot| {
        if let Some(mut usage) = slot.get() {
            let observation = owner.host_visible_backing_usage_v1();
            let device = owner.device_backing_usage_v1();
            if completed {
                usage.completed = observation;
                usage.device_completed = device;
            } else {
                usage.before = observation;
                usage.device_before = device;
            }
            usage.observations[usize::from(completed)] += 1;
            slot.set(Some(usage));
        }
    });
}

pub(super) fn native_device() -> u64 {
    let value = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_ID").expect("explicit device unique ID");
    let device = u64::from_str_radix(value.strip_prefix("0x").unwrap_or(&value), 16).unwrap();
    assert_ne!(device, 0);
    device
}

pub(super) fn observe_shutdown(backend: &mut KfdRuntimeBackendV1) -> PrimaryHostUsage {
    SELECTION.with(|selection| selection.set(Some(false)));
    PRIMARY_HOST_USAGE.with(|slot| slot.set(Some(PrimaryHostUsage::default())));
    backend.shutdown_owned_v1().unwrap();
    assert_eq!(SELECTION.with(Cell::get), Some(true));
    SELECTION.with(|selection| selection.set(None));
    let usage = PRIMARY_HOST_USAGE.with(|slot| slot.take()).unwrap();
    assert_eq!(usage.observations, [1, 1]);
    let before = usage.before.expect("configured account before release");
    let completed = usage
        .completed
        .expect("account inspected before completed root Drop");
    assert!(before.used_backing_bytes > 0 && before.used_allocation_records > 0);
    assert!(!before.poisoned && !completed.poisoned);
    assert_eq!(completed.budget, before.budget);
    assert_eq!(
        (
            completed.used_backing_bytes,
            completed.used_allocation_records
        ),
        (0, 0)
    );
    assert_eq!(
        (
            completed.reserved_records,
            completed.retained_records,
            completed.quarantined_records
        ),
        (0, 0, 0)
    );
    assert!(backend.queue_retired && backend.queue.is_none() && backend.primary_teardown.is_none());
    assert!(!backend.terminal);
    usage
}

fn assert_retired_shutdown_is_inert(backend: &mut KfdRuntimeBackendV1) {
    SELECTION.with(|selection| selection.set(Some(false)));
    PRIMARY_HOST_USAGE.with(|slot| slot.set(Some(PrimaryHostUsage::default())));
    backend.shutdown_owned_v1().unwrap();
    assert_eq!(SELECTION.with(Cell::get), Some(false));
    let usage = PRIMARY_HOST_USAGE.with(|slot| slot.take()).unwrap();
    SELECTION.with(|selection| selection.set(None));
    assert_eq!(usage.observations, [0, 0]);
    assert!(usage.before.is_none() && usage.completed.is_none());
    assert!(
        backend.queue_retired
            && !backend.terminal
            && backend.queue.is_none()
            && backend.primary_teardown.is_none()
    );
}

pub(super) fn observe_selection(retained: bool) {
    SELECTION.with(|selection| {
        if selection.get().is_some() {
            selection.set(Some(retained));
        }
    });
}

#[test]
fn runtime_retired_shutdown_is_inert_without_poison_or_native_work() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    for sdma_enabled in [true, false] {
        let mut backend = std::mem::ManuallyDrop::new(KfdRuntimeBackendV1::mock());
        backend.shutdown_owned_v1().unwrap();
        // Match the completed native SDMA route's flags without inventing a queue.
        backend.sdma_enabled = sdma_enabled;
        let result = catch_unwind(AssertUnwindSafe(|| backend.shutdown_owned_v1()));
        let terminal = backend.terminal;
        assert!(
            backend.queue_retired && backend.queue.is_none() && backend.primary_teardown.is_none()
        );
        assert_eq!(backend.sdma_enabled, sdma_enabled);
        assert!(
            backend.allocations.is_empty()
                && backend.native_compute_lanes.iter().all(Option::is_none)
        );
        // Preserve a diagnostic test failure on the old panic path, not abort-on-Drop.
        backend.terminal = false;
        drop(std::mem::ManuallyDrop::into_inner(backend));
        assert!(matches!(result, Ok(Ok(()))));
        assert!(!terminal);
    }
}

#[test]
fn runtime_multi_device_shutdown_retries_after_a_child_rejection() {
    let mut left = KfdRuntimeBackendV1::mock();
    let stream = left.create_stream_v1(7).unwrap();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    assert!(
        matches!(backend.shutdown_owned_v1(), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
    assert!(
        !backend.terminal
            && !backend.children[0].queue_retired
            && backend.children[1].queue_retired
    );
    // The already completed child can retain its historical SDMA-enabled flag.
    backend.children[1].sdma_enabled = true;
    backend.children[0].destroy_stream_v1(stream).unwrap();
    backend.shutdown_owned_v1().unwrap();
    backend.shutdown_owned_v1().unwrap();
    assert!(!backend.terminal);
    assert!(
        backend
            .children
            .iter()
            .all(|child| child.queue_retired && !child.terminal && child.queue.is_none())
    );
}

#[test]
fn runtime_auxiliary_teardown_errors_and_panics_seal_reentry_without_destroyed_events() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Debug)]
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    for panic in [false, true] {
        let mut backend = std::mem::ManuallyDrop::new(KfdRuntimeBackendV1::mock());
        backend
            .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x87; 32], 32).unwrap())
            .unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let payload = Box::new(Payload(drops.clone()));
        let pointer = core::ptr::from_ref(payload.as_ref());
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.release_auxiliary_for_shutdown_v1(1, |_| {
                if panic {
                    std::panic::resume_unwind(payload);
                }
                Err(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                    "auxiliary fault",
                ))
            })
        }));
        assert!(backend.terminal && !backend.queue_retired);
        assert!(matches!(
            backend
                .release_auxiliary_for_shutdown_v1(1, |_| panic!("terminal retry entered cleanup")),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.shutdown_owned_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.create_stream_v1(7),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        if panic {
            let caught = result.unwrap_err().downcast::<Payload>().unwrap();
            assert_eq!(core::ptr::from_ref(caught.as_ref()), pointer);
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            drop(caught);
        } else {
            assert!(
                matches!(result.unwrap(), Err(RuntimeBackendFailureV1::Terminal(error)) if error.detail().contains("auxiliary fault"))
            );
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(
            !backend
                .profiler
                .as_ref()
                .unwrap()
                .recorded_events_for_test_v1()
                .iter()
                .any(|event| matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { .. }
                ))
        );
        // Adapter settlement only: this fixture has no native queue to discard.
        assert!(backend.queue.is_none() && backend.allocations.is_empty());
        backend.terminal = false;
        drop(std::mem::ManuallyDrop::into_inner(backend));
    }
}

#[test]
fn runtime_pool_trim_error_and_panic_terminalize_before_returning() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug)]
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    for panic in [false, true] {
        let mut backend = std::mem::ManuallyDrop::new(KfdRuntimeBackendV1::mock());
        let drops = Arc::new(AtomicUsize::new(0));
        let payload = Box::new(Payload(drops.clone()));
        let pointer = core::ptr::from_ref(payload.as_ref());
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.trim_sdma_pool_for_shutdown_v1(|_| {
                if panic {
                    std::panic::resume_unwind(payload);
                }
                Err(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                    "trim fault",
                ))
            })
        }));
        assert!(backend.terminal && !backend.queue_retired && backend.primary_teardown.is_none());
        assert!(matches!(
            backend.require_live(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.shutdown_owned_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.create_stream_v1(7),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        if panic {
            let caught = result.unwrap_err().downcast::<Payload>().unwrap();
            assert_eq!(core::ptr::from_ref(caught.as_ref()), pointer);
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            drop(caught);
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        // This fixture owns no native queue or allocations; disarm only its Drop guard.
        assert!(backend.queue.is_none() && backend.allocations.is_empty());
        backend.terminal = false;
        drop(std::mem::ManuallyDrop::into_inner(backend));
    }
}

#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_allocation_shutdown_selects_retained_directional_release() {
    let device = native_device();
    // Any accidental compute authorization panics; no kernel authority is issued.
    let mut backend = KfdRuntimeBackendV1::open_default(device, TestPanickingAuthorityV1).unwrap();
    backend
        .configure_host_visible_backing_budget_v1(
            Gfx942HostVisibleBackingBudgetV1::new(16 * 1024 * 1024, 32).unwrap(),
        )
        .unwrap();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x76; 32], 64).unwrap())
        .unwrap();
    let devices = backend.enumerate_devices_v1().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].backend_device, device);
    let stream = backend.create_stream_v1(device).unwrap();
    let allocation = backend
        .allocate_v1(device, RuntimeMemoryKindV1::HostVisible, 4096, 4096)
        .unwrap();
    assert!(backend.sdma_enabled && backend.queue.is_some());
    assert!(
        backend
            .host_visible_backing_usage_v1()
            .unwrap()
            .used_backing_bytes
            >= 4096
    );
    backend.release_allocation_v1(allocation).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    let pool = backend
        .queue
        .as_ref()
        .unwrap()
        .sdma_memory_pool_observation()
        .unwrap();
    assert_eq!(pool.checked_out_buffers, 0);
    assert_eq!(pool.retained_free_buffers, 1);
    assert_eq!(pool.retained_free_bytes, 4096);
    // Observe the real shutdown selector AFTER its own pool trim, without changing it.
    let usage = observe_shutdown(&mut backend);
    assert_retired_shutdown_is_inert(&mut backend);
    assert!(matches!(
        backend.create_stream_v1(device),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(matches!(
        backend.allocate_v1(device, RuntimeMemoryKindV1::HostVisible, 4096, 4096),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    let profile = backend.finish_profiler_v1().unwrap();
    profile.validate().unwrap();
    assert_eq!(profile.coverage.dropped_events, 0);
    assert!(profile.coverage.complete_runtime_operation_history);
    // An SDMA bootstrap queue has no logical compute lane and emits no false lane destruction.
    assert!(!profile.events.iter().any(|event| matches!(
        event.event,
        KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
            | KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { .. }
    )));
    drop(backend);
    println!("primary_host_usage={usage:?}");
    println!(
        "native_runtime_retained_directional_release=complete allocation_workflow=public pooled_buffers=1 pooled_bytes=4096 selector=retained completed_root_drop=confirmed backend_drop=completed packets=0"
    );
}

#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_device_promotion_roundtrip_and_retained_shutdown() {
    native_device_roundtrip_retained_shutdown(false);
}

#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_zero_capacity_recycle_disposes_before_trim() {
    native_device_roundtrip_retained_shutdown(true);
}

fn native_device_roundtrip_retained_shutdown(zero_cache: bool) {
    let device = native_device();
    let mut backend = KfdRuntimeBackendV1::open_default(device, TestPanickingAuthorityV1).unwrap();
    backend
        .configure_host_visible_backing_budget_v1(
            Gfx942HostVisibleBackingBudgetV1::new(16 * 1024 * 1024, 32).unwrap(),
        )
        .unwrap();
    backend
        .configure_device_backing_budget_v1(
            Gfx942DeviceBackingBudgetV1::new(1024 * 1024, 8).unwrap(),
        )
        .unwrap();
    if zero_cache {
        backend
            .configure_host_pool_limits_v1(fe2o3_kfd::Gfx942HostPoolLimitsV1::new(0, 0).unwrap())
            .unwrap();
        backend
            .configure_device_pool_limits_v1(Gfx942DevicePoolLimitsV1::new(0, 0).unwrap())
            .unwrap();
    }
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x84; 32], 128).unwrap())
        .unwrap();
    let mut allocations = Vec::new();
    for bytes in [17_usize, 4097] {
        let allocation = backend
            .allocate_v1(device, RuntimeMemoryKindV1::DeviceLocal, bytes as u64, 4096)
            .unwrap();
        let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&allocation].sdma_storage
        else {
            panic!("promoted device allocation required");
        };
        let DirectionalSdmaDeviceOwnerV1::Native(owner) = &**owner else {
            panic!("native promotion required");
        };
        assert_eq!(owner.byte_len(), bytes as u64);
        assert_eq!(
            owner.physical_byte_len(),
            (bytes as u64).next_multiple_of(4096)
        );
        let mut readback = vec![0xa5; bytes];
        backend
            .read_allocation_v1(allocation, 0, &mut readback)
            .unwrap();
        assert_eq!(readback, vec![0; bytes]);
        let expected = (0..bytes)
            .map(|index| ((index * 17 + 3) % 251) as u8)
            .collect::<Vec<_>>();
        backend
            .write_allocation_v1(allocation, 0, &expected)
            .unwrap();
        readback.fill(0xa5);
        // The direct download oracle requires the native path to report that it handled the read.
        assert!(
            backend
                .download_sdma_range_v1(allocation, 0, &mut readback)
                .unwrap()
        );
        assert_eq!(readback, expected);
        println!(
            "native_device_promotion logical_bytes={bytes} physical_bytes={} readback_sha256={}",
            (bytes as u64).next_multiple_of(4096),
            Sha256::digest(&readback)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        allocations.push(allocation);
    }
    let allocated = backend.device_backing_usage_v1().unwrap();
    assert_eq!(
        (
            allocated.used_backing_bytes,
            allocated.used_allocation_records
        ),
        (12288, 2)
    );
    for allocation in allocations {
        backend.release_allocation_v1(allocation).unwrap();
    }
    let pool = backend
        .queue
        .as_ref()
        .unwrap()
        .sdma_memory_pool_observation()
        .unwrap();
    assert_eq!(pool.checked_out_buffers, 0);
    let before_trim = backend.device_backing_usage_v1().unwrap();
    if zero_cache {
        assert_eq!(
            (pool.retained_free_buffers, pool.retained_free_bytes),
            (0, 0)
        );
        assert_eq!(
            (
                before_trim.used_backing_bytes,
                before_trim.used_allocation_records
            ),
            (0, 0)
        );
        let host = backend.host_visible_backing_usage_v1().unwrap();
        assert_eq!(
            (host.used_backing_bytes, host.used_allocation_records),
            (532480, 3)
        );
    } else {
        assert!(pool.retained_free_buffers > 0);
        assert_eq!(
            (
                before_trim.used_backing_bytes,
                before_trim.used_allocation_records
            ),
            (12288, 2)
        );
    }
    let trimmed = backend
        .queue
        .as_mut()
        .unwrap()
        .trim_sdma_memory_pool()
        .unwrap();
    if zero_cache {
        assert_eq!(trimmed, 0);
    }
    let refunded = backend.device_backing_usage_v1().unwrap();
    assert_eq!(
        (
            refunded.used_backing_bytes,
            refunded.used_allocation_records
        ),
        (0, 0)
    );
    assert_eq!(
        (
            refunded.reserved_records,
            refunded.retained_records,
            refunded.quarantined_records
        ),
        (0, 0, 0)
    );
    assert!(!refunded.poisoned);
    let host_usage = observe_shutdown(&mut backend);
    assert_retired_shutdown_is_inert(&mut backend);
    let profile = backend.finish_profiler_v1().unwrap();
    profile.validate().unwrap();
    assert_eq!(profile.coverage.dropped_events, 0);
    assert!(profile.coverage.complete_runtime_operation_history);
    drop(backend);
    println!(
        "native_device_promotion=complete zero_cache={zero_cache} pool_before_trim={pool:?} device_before_trim={before_trim:?} device_before={allocated:?} device_after={refunded:?} host_usage={host_usage:?} profile_events={}",
        profile.events.len()
    );
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_typed_dispatch_shutdown_refunds_and_profiles_retained_primary() {
    native_typed_dispatch_retained_release(1, false, false);
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_two_stream_dispatch_uses_primary_and_auxiliary_then_refunds() {
    native_typed_dispatch_retained_release(2, false, false);
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_auxiliary_shutdown_retries_after_primary_capacity_rejection() {
    native_typed_dispatch_retained_release(2, false, true);
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_allocates_while_primary_compute_is_pending() {
    native_typed_dispatch_retained_release(1, true, false);
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_allocates_while_primary_and_auxiliary_compute_are_pending() {
    native_typed_dispatch_retained_release(2, true, false);
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID; terminal retention until process exit"]
fn native_runtime_auxiliary_budget_failure_retains_initialized_prefix() {
    use crate::qualification_gfx942_vecadd_v1::{
        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1 as ALIGNMENT,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as BYTES, Gfx942VecaddQualificationArgumentsV1,
        admit_gfx942_vecadd_qualification_v1,
    };
    use crate::{RuntimeContextV1, RuntimeErrorV1};

    let prefix: usize = std::env::var("FE2O3_TEST_NATIVE_INITIALIZED_PREFIX")
        .unwrap_or_else(|_| "2".to_owned())
        .parse()
        .unwrap();
    assert!(prefix <= 2);
    let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
    let buffers = admitted.host_buffers().unwrap();
    let mut backend =
        KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(native_device()).unwrap();
    backend
        .configure_host_visible_backing_budget_v1(
            Gfx942HostVisibleBackingBudgetV1::new((37 + 4 * prefix as u64) * 1024 * 1024, 128)
                .unwrap(),
        )
        .unwrap();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x78; 32], 128).unwrap())
        .unwrap();
    // Terminal retention intentionally lasts until this isolated test process exits.
    // Do not run ordinary shutdown or Drop on the terminal context, even on assertion failure.
    let mut context = core::mem::ManuallyDrop::new(RuntimeContextV1::open(backend).unwrap());
    let device = context.devices()[0].id();
    let module = context.load_module(device, admitted.hsaco()).unwrap();
    let kernel = context
        .resolve_kernel::<Gfx942VecaddQualificationArgumentsV1>(module, admitted.kernel_name())
        .unwrap();
    let mut prepared = Vec::with_capacity(2);
    for _ in 0..2 {
        let stream = context.create_stream(device).unwrap();
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
        prepared.push((
            stream,
            Gfx942VecaddQualificationArgumentsV1::new(
                allocations[0],
                allocations[1],
                allocations[2],
            )
            .unwrap(),
        ));
    }
    let (first_stream, first_args) = &prepared[0];
    let _first = context
        .launch(*first_stream, &kernel, first_args, admitted.geometry(), &[])
        .unwrap();
    context.flush_stream(*first_stream).unwrap();
    let before = context.backend().host_visible_backing_usage_v1().unwrap();
    let primary = context.backend().native_compute_lanes[0].unwrap();
    assert_eq!(primary.ordinal(), 0);
    assert!(context.backend().native_compute_lanes[1].is_none());
    let active = context
        .backend()
        .active
        .iter()
        .chain(context.backend().compute_pipeline.iter())
        .next()
        .unwrap();
    let first_identity = (active.id, active.stream, active.allocations.clone());
    let second_backend_stream = *context
        .backend()
        .streams
        .keys()
        .find(|stream| **stream != active.stream)
        .unwrap();
    let publications_before = context
        .backend()
        .profiler
        .as_ref()
        .unwrap()
        .recorded_events_for_test_v1()
        .iter()
        .filter(|event| {
            matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
                    | KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        (before.used_backing_bytes, before.used_allocation_records),
        (38_281_216, 12)
    );
    assert_eq!(
        (
            before.reserved_records,
            before.retained_records,
            before.quarantined_records,
            before.poisoned
        ),
        (0, 12, 0, false)
    );
    assert!(
        before.used_backing_bytes + prefix as u64 * BYTES as u64
            <= before.budget.max_backing_bytes()
    );
    assert!(
        before.used_backing_bytes + (prefix as u64 + 1) * BYTES as u64
            > before.budget.max_backing_bytes()
    );

    MATERIALIZATION_RETENTION.with(|slot| slot.set(Some(MaterializationRetention::default())));
    // Leave primary work logically pending so this launch must select AUX, not rebind lane 0.
    let (second_stream, second_args) = &prepared[1];
    let result = context
        .launch(
            *second_stream,
            &kernel,
            second_args,
            admitted.geometry(),
            &[],
        )
        .and_then(|_| context.flush_stream(*second_stream));
    let error = match result {
        Err(RuntimeErrorV1::BackendTerminal(error)) => error,
        other => panic!("expected terminal native budget rejection, got {other:?}"),
    };
    assert_eq!(
        error.detail(),
        "KFD host-visible initialization: host-visible backing resource credits: runtime resource credit error: Capacity"
    );
    assert!(context.is_terminal() && context.backend().terminal);
    assert_eq!(context.backend().native_compute_lanes[0], Some(primary));
    assert!(context.backend().native_compute_lanes[1].is_none());
    assert_eq!(
        context
            .backend()
            .queue
            .as_ref()
            .unwrap()
            .auxiliary_compute_lane_count_v1(),
        0
    );
    assert_eq!(context.backend().allocations.len(), 6);
    assert_eq!(context.backend().pending_compute.len(), 1);
    assert_eq!(
        context
            .backend()
            .pending_compute
            .values()
            .next()
            .unwrap()
            .launch
            .stream,
        second_backend_stream
    );
    let active = context
        .backend()
        .active
        .iter()
        .chain(context.backend().compute_pipeline.iter())
        .find(|active| active.id == first_identity.0)
        .unwrap();
    assert_eq!(
        (active.id, active.stream, &active.allocations),
        (first_identity.0, first_identity.1, &first_identity.2)
    );
    assert!(active.execution.is_some());
    let observation = MATERIALIZATION_RETENTION.with(Cell::get).unwrap();
    assert_eq!(
        (
            observation.calls,
            observation.spec_count,
            observation.retained_count
        ),
        (1, 3, prefix)
    );
    for layout in &observation.layouts[..prefix] {
        let layout = layout.unwrap();
        assert_eq!(
            layout.kind(),
            fe2o3_kfd::Gfx942FixedDispatchDataKindV1::HostVisibleCoherent
        );
        assert_eq!(layout.requested_bytes(), BYTES as u64);
        assert_eq!(layout.alignment(), 4096);
    }
    assert!(observation.layouts[prefix..].iter().all(Option::is_none));
    let failure_usage = observation.failure_usage.unwrap();
    assert_eq!(failure_usage.budget, before.budget);
    assert_eq!(
        failure_usage.used_backing_bytes,
        before.used_backing_bytes + prefix as u64 * BYTES as u64
    );
    assert_eq!(
        failure_usage.used_allocation_records,
        before.used_allocation_records + prefix as u64
    );
    assert_eq!(
        failure_usage.retained_records,
        before.retained_records + prefix
    );
    assert_eq!(
        (
            failure_usage.reserved_records,
            failure_usage.quarantined_records
        ),
        (before.reserved_records, before.quarantined_records)
    );
    // This is an inert pre-retake snapshot; the lower constructor subsequently retains its parent.
    assert!(!failure_usage.poisoned);
    let events = context
        .backend()
        .profiler
        .as_ref()
        .unwrap()
        .recorded_events_for_test_v1();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
            ))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
            ))
            .count(),
        1
    );
    let event_count = events.len();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
                    | KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
            ))
            .cloned()
            .collect::<Vec<_>>(),
        publications_before
    );
    assert!(context.flush_stream(*second_stream).is_err());
    assert_eq!(MATERIALIZATION_RETENTION.with(Cell::get), Some(observation));
    assert_eq!(
        context
            .backend()
            .profiler
            .as_ref()
            .unwrap()
            .recorded_events_for_test_v1()
            .len(),
        event_count
    );
    MATERIALIZATION_RETENTION.with(|slot| slot.set(None));
    println!(
        "before_auxiliary={before:?} retained_materialization={observation:?} terminal_error={error:?}"
    );
    println!(
        "native_auxiliary_budget_failure=retained initialized_prefix={prefix} unpublished_auxiliary=confirmed retry=inert reclamation=process_exit_only"
    );
}

#[cfg(feature = "hardware-qualification")]
#[derive(Debug, PartialEq)]
struct NativePendingReceipt {
    id: u64,
    lane: usize,
    stream: u64,
    kernel: u64,
    allocations: HashSet<u64>,
    receipt: [u8; 32],
    recipe: usize,
    published_at: Instant,
}

#[cfg(feature = "hardware-qualification")]
#[derive(Debug, PartialEq)]
struct NativePendingSnapshot {
    receipts: Vec<NativePendingReceipt>,
    leases: HashMap<u64, usize>,
    module_retains: HashMap<u64, usize>,
    dependency_retains: HashMap<u64, usize>,
    event_retains: HashMap<u64, usize>,
    tails: HashMap<u64, u64>,
    reservations: usize,
    compute_events: Vec<fe2o3_profiler_protocol::KfdRuntimeProfileEventV1>,
}

#[cfg(feature = "hardware-qualification")]
fn native_pending_snapshot(backend: &KfdRuntimeBackendV1, count: usize) -> NativePendingSnapshot {
    assert!(backend.sdma_allocation_ready_v1() && !backend.terminal);
    assert!(backend.pending_compute.is_empty() && backend.submissions.is_empty());
    assert!(backend.compute_pipeline.is_empty());
    assert!(
        backend
            .auxiliary_compute_lanes
            .iter()
            .all(|lane| lane.pipeline.is_empty())
    );
    let queue = backend.queue.as_ref().unwrap();
    let receipts = backend
        .active
        .iter()
        .chain(
            backend
                .auxiliary_compute_lanes
                .iter()
                .filter_map(|lane| lane.active.as_ref()),
        )
        .map(|active| {
            let Some(ActiveComputeExecutionV1::Materialized(batch)) = &active.execution else {
                panic!("native probe requires an original published ordinary receipt");
            };
            let lane = backend.active_compute_lane_v1(active.id).unwrap();
            let native_lane = backend.native_compute_lanes[lane].unwrap();
            let receipt = queue
                .observe_retained_fixed_dispatch_v1(native_lane, batch)
                .unwrap();
            for other in backend
                .native_compute_lanes
                .iter()
                .flatten()
                .copied()
                .filter(|other| *other != native_lane)
            {
                assert!(
                    queue
                        .observe_retained_fixed_dispatch_v1(other, batch)
                        .is_none()
                );
            }
            NativePendingReceipt {
                id: active.id,
                lane,
                stream: active.stream,
                kernel: active.kernel,
                allocations: active.allocations.clone(),
                receipt,
                recipe: Arc::as_ptr(active.ordinary_recipe.as_ref().unwrap()) as usize,
                published_at: active.published_at,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), count);
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| receipt.lane)
            .collect::<Vec<_>>(),
        (0..count).collect::<Vec<_>>()
    );
    NativePendingSnapshot {
        receipts,
        leases: backend.stream_compute_lanes.clone(),
        module_retains: backend.compute_module_retain_counts.clone(),
        dependency_retains: backend.compute_dependency_retain_counts.clone(),
        event_retains: backend.event_submission_retain_counts.clone(),
        tails: backend.stream_submission_tails.clone(),
        reservations: backend.compute_completion_reservations,
        compute_events: backend
            .profiler
            .as_ref()
            .unwrap()
            .recorded_events_for_test_v1()
            .iter()
            .filter(|event| {
                matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
                        | KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { .. }
                        | KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
                        | KfdRuntimeProfileEventKindV1::DispatchCompleted { .. }
                        | KfdRuntimeProfileEventKindV1::SubmissionReleased { .. }
                )
            })
            .cloned()
            .collect(),
    }
}

#[cfg(feature = "hardware-qualification")]
fn native_typed_dispatch_retained_release(
    stream_count: usize,
    pending_allocations: bool,
    retry_shutdown: bool,
) {
    use crate::qualification_gfx942_vecadd_v1::{
        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1 as ALIGNMENT,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as BYTES, Gfx942VecaddQualificationArgumentsV1,
        admit_gfx942_vecadd_qualification_v1,
    };
    use crate::{RuntimeContextV1, RuntimePollV1};

    let device = native_device();
    let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
    let buffers = admitted.host_buffers().unwrap();
    let mut backend = KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(device).unwrap();
    backend
        .configure_host_visible_backing_budget_v1(
            Gfx942HostVisibleBackingBudgetV1::new(64 * 1024 * 1024, 128).unwrap(),
        )
        .unwrap();
    if pending_allocations {
        backend
            .configure_device_backing_budget_v1(
                fe2o3_kfd::Gfx942DeviceBackingBudgetV1::new(1024 * 1024, 8).unwrap(),
            )
            .unwrap();
    }
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x77; 32], 128).unwrap())
        .unwrap();
    let mut context = RuntimeContextV1::open(backend).unwrap();
    assert_eq!(context.devices().len(), 1);
    assert_eq!(context.devices()[0].target(), "gfx942:xnack-");
    let device_id = context.devices()[0].id();
    let module = context.load_module(device_id, admitted.hsaco()).unwrap();
    let kernel = context
        .resolve_kernel::<Gfx942VecaddQualificationArgumentsV1>(module, admitted.kernel_name())
        .unwrap();
    let mut prepared = Vec::with_capacity(stream_count);
    for _ in 0..stream_count {
        let stream = context.create_stream(device_id).unwrap();
        let allocations = [buffers.left(), buffers.right(), buffers.output()].map(|bytes| {
            let allocation = context
                .allocate(
                    device_id,
                    RuntimeMemoryKindV1::HostVisible,
                    BYTES as u64,
                    ALIGNMENT,
                )
                .unwrap();
            context.write_allocation(allocation, 0, bytes).unwrap();
            allocation
        });
        let arguments = Gfx942VecaddQualificationArgumentsV1::new(
            allocations[0],
            allocations[1],
            allocations[2],
        )
        .unwrap();
        prepared.push((stream, allocations, arguments));
    }
    let mut launches = Vec::with_capacity(stream_count);
    for (stream, allocations, arguments) in prepared {
        let submission = context
            .launch(stream, &kernel, &arguments, admitted.geometry(), &[])
            .unwrap();
        context.flush_stream(stream).unwrap();
        launches.push((stream, allocations, submission));
    }
    let mut added = Vec::new();
    if pending_allocations {
        // Logical native custody is pending; this kernel has no minimum physical duration.
        let before = native_pending_snapshot(context.backend(), stream_count);
        for kind in [
            RuntimeMemoryKindV1::HostVisible,
            RuntimeMemoryKindV1::DeviceLocal,
        ] {
            let candidate = context.backend().next_handle;
            added.push(context.allocate(device_id, kind, 4096, 4096).unwrap());
            let record = &context.backend().allocations[&candidate];
            assert!(record.sdma_backed && record.sdma_initialized);
            assert_eq!(
                (record.kind, record.bytes.len(), record.alignment),
                (kind, 4096, 4096)
            );
            match (&record.sdma_storage, kind) {
                (
                    KfdRuntimeSdmaStorageV1::Host(SdmaBufferOwnerV1::Native(_)),
                    RuntimeMemoryKindV1::HostVisible,
                ) => {}
                (KfdRuntimeSdmaStorageV1::Device(owner), RuntimeMemoryKindV1::DeviceLocal) => {
                    let DirectionalSdmaDeviceOwnerV1::Native(native) = &**owner else {
                        panic!("expected native device owner")
                    };
                    assert_eq!(
                        (native.byte_len(), native.physical_byte_len()),
                        (4096, 4096)
                    );
                    let usage = context.backend().device_backing_usage_v1().unwrap();
                    assert_eq!(
                        (usage.used_backing_bytes, usage.used_allocation_records),
                        (4096, 1)
                    );
                    assert!(!usage.poisoned);
                }
                _ => panic!("pending allocation must retain the requested native owner"),
            }
            assert_eq!(
                native_pending_snapshot(context.backend(), stream_count),
                before
            );
        }
        println!(
            "native_pending_receipts={before:?} new_allocations=2 physical_overlap=not_measured"
        );
    }
    let mut observed = vec![0; BYTES];
    for (_, allocations, submission) in &mut launches {
        assert_eq!(
            context.wait(submission, Duration::from_secs(10)).unwrap(),
            RuntimePollV1::Succeeded,
        );
        for (allocation, expected) in allocations.iter().copied().zip([
            buffers.left(),
            buffers.right(),
            buffers.expected_output(),
        ]) {
            context
                .read_allocation(allocation, 0, &mut observed)
                .unwrap();
            assert_eq!(observed, expected, "full-byte input/output comparison");
        }
    }
    let output_sha256 = Sha256::digest(&observed)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        output_sha256,
        "79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3"
    );
    for &allocation in &added {
        let mut zeros = [0xa5; 4096];
        context.read_allocation(allocation, 0, &mut zeros).unwrap();
        assert_eq!(zeros, [0; 4096]);
    }
    let live = context.backend().host_visible_backing_usage_v1().unwrap();
    assert!(live.used_backing_bytes >= 3 * stream_count as u64 * BYTES as u64);
    assert!(live.used_allocation_records >= 3 * stream_count as u64);
    assert!(!live.poisoned);
    for allocation in added.into_iter().rev() {
        context.release_allocation(allocation).unwrap();
    }
    for (stream, allocations, submission) in launches.into_iter().rev() {
        context.release_submission(submission).unwrap();
        for allocation in allocations.into_iter().rev() {
            context.release_allocation(allocation).unwrap();
        }
        context.destroy_stream(stream).unwrap();
    }
    context.unload_module(module).unwrap();
    let mut backend = context.shutdown().unwrap();
    if pending_allocations {
        backend
            .queue
            .as_mut()
            .unwrap()
            .trim_sdma_memory_pool()
            .unwrap();
        let usage = backend.device_backing_usage_v1().unwrap();
        assert_eq!(
            (usage.used_backing_bytes, usage.used_allocation_records),
            (0, 0)
        );
        assert_eq!(
            (
                usage.reserved_records,
                usage.retained_records,
                usage.quarantined_records
            ),
            (0, 0, 0)
        );
        assert!(!usage.poisoned);
    }
    assert!(backend.sdma_enabled && backend.queue.is_some());
    let compute_lanes = backend
        .native_compute_lanes
        .iter()
        .filter_map(|lane| *lane)
        .collect::<Vec<_>>();
    assert_eq!(compute_lanes.len(), stream_count);
    let compute_ordinals = compute_lanes
        .iter()
        .map(|lane| lane.ordinal())
        .collect::<Vec<_>>();
    assert_eq!(compute_ordinals, (0..stream_count).collect::<Vec<_>>());
    assert_eq!(
        compute_lanes[0],
        backend.queue.as_ref().unwrap().primary_compute_lane_v1()
    );
    assert_eq!(
        backend
            .queue
            .as_ref()
            .unwrap()
            .auxiliary_compute_lane_count_v1(),
        stream_count - 1
    );
    if retry_shutdown {
        assert_eq!(stream_count, 2);
        let primary = backend.native_compute_lanes[0].unwrap();
        let auxiliary = backend.native_compute_lanes[1].unwrap();
        let auxiliary_profile = backend
            .profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + 1,
            )
            .unwrap();
        PRIMARY_TEARDOWN_CAPACITY.with(|slot| {
            assert!(!slot.replace(true));
        });
        assert!(
            matches!(backend.shutdown_owned_v1(), Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity && error.detail() == "primary KFD teardown storage")
        );
        assert!(!PRIMARY_TEARDOWN_CAPACITY.with(Cell::get));
        assert!(!backend.terminal && !backend.queue_retired && backend.primary_teardown.is_none());
        assert_eq!(backend.native_compute_lanes[0], Some(primary));
        assert!(backend.native_compute_lanes[1].is_none());
        assert!(backend.stream_compute_lanes.is_empty());
        let queue = backend.queue.as_mut().unwrap();
        assert_eq!(queue.primary_compute_lane_v1(), primary);
        assert!(queue.with_compute_lane_v1(primary, |_| ()).is_ok());
        assert_eq!(queue.auxiliary_compute_lane_count_v1(), 0);
        assert!(queue.with_compute_lane_v1(auxiliary, |_| ()).is_err());
        let state = &backend.auxiliary_compute_lanes[0];
        assert!(
            state.owner_stream.is_none()
                && state.active.is_none()
                && state.pipeline.is_empty()
                && state.resident_data.is_none()
                && state.recycled_dispatch.is_none()
        );
        let destroyed: Vec<_> = backend
            .profiler
            .as_ref()
            .unwrap()
            .recorded_events_for_test_v1()
            .iter()
            .filter_map(|e| match e.event {
                KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue } => Some(queue),
                _ => None,
            })
            .collect();
        assert_eq!(destroyed, vec![auxiliary_profile]);
    }
    let usage = observe_shutdown(&mut backend);
    assert!(matches!(
        backend.create_stream_v1(device),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_retired_shutdown_is_inert(&mut backend);
    let profile = backend.finish_profiler_v1().unwrap();
    profile.validate().unwrap();
    assert!(profile.coverage.complete_runtime_operation_history);
    assert_eq!(profile.coverage.dropped_events, 0);
    let allocation_identities = profile
        .events
        .iter()
        .filter_map(|entry| match entry.event {
            KfdRuntimeProfileEventKindV1::AllocationCreated { allocation, .. } => Some(allocation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        allocation_identities.len(),
        3 * stream_count + 2 * usize::from(pending_allocations)
    );
    let reads = profile
        .events
        .iter()
        .filter_map(|entry| match entry.event {
            KfdRuntimeProfileEventKindV1::HostRead {
                allocation,
                byte_offset,
                content,
            } => Some((allocation, byte_offset, content)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reads,
        allocation_identities
            .into_iter()
            .enumerate()
            .map(|(index, allocation)| (
                allocation,
                0,
                KfdProfileHostContentV1::RangeOnly {
                    byte_len: if index < 3 * stream_count {
                        BYTES as u64
                    } else {
                        4096
                    },
                }
            ))
            .collect::<Vec<_>>()
    );
    let created = profile
        .events
        .iter()
        .filter_map(|entry| match entry.event {
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue } => {
                Some((entry.sequence, queue))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let destroyed = profile
        .events
        .iter()
        .filter_map(|entry| match entry.event {
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue } => {
                Some((entry.sequence, queue))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(created.len(), stream_count);
    assert_eq!(destroyed.len(), stream_count);
    assert_eq!(
        created
            .iter()
            .map(|(_, queue)| queue)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        stream_count
    );
    let published = profile
        .events
        .iter()
        .filter_map(|entry| match entry.event {
            KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                queue,
                stream,
                ..
            } => Some((entry.sequence, dispatch, queue, stream)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let completed = profile
        .events
        .iter()
        .filter_map(|entry| match entry.event {
            KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } => {
                Some((entry.sequence, dispatch))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(published.len(), stream_count);
    assert_eq!(completed.len(), stream_count);
    let streams = profile
        .events
        .iter()
        .filter_map(|entry| match entry.event {
            KfdRuntimeProfileEventKindV1::StreamCreated { stream } => Some(stream),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(streams.len(), stream_count);
    assert_eq!(
        streams
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        stream_count
    );
    for ((created_at, queue), stream) in created.iter().zip(&streams) {
        let (destroyed_at, _) = destroyed
            .iter()
            .find(|(_, identity)| identity == queue)
            .unwrap();
        let (published_at, dispatch, _, dispatch_stream) = published
            .iter()
            .find(|(_, _, identity, _)| identity == queue)
            .unwrap();
        assert_eq!(dispatch_stream, stream);
        let (completed_at, _) = completed
            .iter()
            .find(|(_, identity)| identity == dispatch)
            .unwrap();
        assert!(
            created_at < published_at && published_at < completed_at && completed_at < destroyed_at
        );
    }
    drop(backend);
    println!("profile_json={}", serde_json::to_string(&profile).unwrap());
    println!("primary_host_usage={usage:?} live_host_usage={live:?}");
    println!(
        "native_runtime_typed_dispatch_retained_release=complete selector=retained kernel=vecadd compute_ordinals={compute_ordinals:?} primary_execution=confirmed pending_allocations={pending_allocations} packets={stream_count} readbacks={} output_sha256={output_sha256} host_account_refund=complete queue_profile=matched completed_primary_root_drop=confirmed backend_drop=completed",
        3 * stream_count + 2 * usize::from(pending_allocations),
    );
}
