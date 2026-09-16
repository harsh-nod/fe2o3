//! Shutdown settlement tests and opt-in public native allocation workflow.

use super::*;
use crate::RuntimeOwnedShutdownBackendV1;
use fe2o3_kfd::Gfx942HostVisibleBackingUsageV1;
use std::cell::Cell;

thread_local! { static SELECTION: Cell<Option<bool>> = const { Cell::new(None) }; }

#[derive(Clone, Copy, Debug, Default)]
struct PrimaryHostUsage {
    before: Option<Gfx942HostVisibleBackingUsageV1>,
    completed: Option<Gfx942HostVisibleBackingUsageV1>,
    observations: [usize; 2],
}

thread_local! {
    static PRIMARY_HOST_USAGE: Cell<Option<PrimaryHostUsage>> = const { Cell::new(None) };
}

pub(super) fn observe_primary_host_usage(owner: &PrimaryQueueReleaseCustodyV1, completed: bool) {
    PRIMARY_HOST_USAGE.with(|slot| {
        if let Some(mut usage) = slot.get() {
            let observation = owner.host_visible_backing_usage_v1();
            if completed {
                usage.completed = observation;
            } else {
                usage.before = observation;
            }
            usage.observations[usize::from(completed)] += 1;
            slot.set(Some(usage));
        }
    });
}

fn native_device() -> u64 {
    let value = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_ID").expect("explicit device unique ID");
    let device = u64::from_str_radix(value.strip_prefix("0x").unwrap_or(&value), 16).unwrap();
    assert_ne!(device, 0);
    device
}

fn observe_shutdown(backend: &mut KfdRuntimeBackendV1) -> PrimaryHostUsage {
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

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_typed_dispatch_shutdown_refunds_and_profiles_retained_primary() {
    native_typed_dispatch_retained_release(1);
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_runtime_two_stream_dispatch_uses_primary_and_auxiliary_then_refunds() {
    native_typed_dispatch_retained_release(2);
}

#[cfg(feature = "hardware-qualification")]
fn native_typed_dispatch_retained_release(stream_count: usize) {
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
    let live = context.backend().host_visible_backing_usage_v1().unwrap();
    assert!(live.used_backing_bytes >= 3 * stream_count as u64 * BYTES as u64);
    assert!(live.used_allocation_records >= 3 * stream_count as u64);
    assert!(!live.poisoned);
    for (stream, allocations, submission) in launches.into_iter().rev() {
        context.release_submission(submission).unwrap();
        for allocation in allocations.into_iter().rev() {
            context.release_allocation(allocation).unwrap();
        }
        context.destroy_stream(stream).unwrap();
    }
    context.unload_module(module).unwrap();
    let mut backend = context.shutdown().unwrap();
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
    assert_eq!(allocation_identities.len(), 3 * stream_count);
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
            .map(|allocation| (
                allocation,
                0,
                KfdProfileHostContentV1::RangeOnly {
                    byte_len: BYTES as u64,
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
        "native_runtime_typed_dispatch_retained_release=complete selector=retained kernel=vecadd compute_ordinals={compute_ordinals:?} primary_execution=confirmed packets={stream_count} readbacks={} output_sha256={output_sha256} host_account_refund=complete queue_profile=matched completed_primary_root_drop=confirmed backend_drop=completed",
        3 * stream_count,
    );
}
