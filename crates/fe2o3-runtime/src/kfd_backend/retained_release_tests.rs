//! Shutdown settlement tests and opt-in public native allocation workflow.

use super::*;
use crate::RuntimeOwnedShutdownBackendV1;
use std::cell::Cell;

thread_local! { static SELECTION: Cell<Option<bool>> = const { Cell::new(None) }; }

pub(super) fn observe_selection(retained: bool) {
    SELECTION.with(|selection| {
        if selection.get().is_some() {
            selection.set(Some(retained));
        }
    });
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
    let value = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_ID").expect("explicit device unique ID");
    let device = u64::from_str_radix(value.strip_prefix("0x").unwrap_or(&value), 16).unwrap();
    assert_ne!(device, 0);
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
    SELECTION.with(|selection| selection.set(Some(false)));
    backend.shutdown_owned_v1().unwrap();
    assert_eq!(SELECTION.with(Cell::get), Some(true));
    SELECTION.with(|selection| selection.set(None));
    assert!(backend.queue_retired && backend.queue.is_none() && backend.primary_teardown.is_none());
    assert!(!backend.terminal);
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
    println!(
        "native_runtime_retained_directional_release=complete allocation_workflow=public pooled_buffers=1 pooled_bytes=4096 selector=retained completed_root_drop=confirmed backend_drop=completed packets=0"
    );
}
