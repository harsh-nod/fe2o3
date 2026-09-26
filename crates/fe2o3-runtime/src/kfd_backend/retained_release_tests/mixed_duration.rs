//! Fixed-work artifact correctness before owner-thread scheduling qualification.

use super::*;
use crate::qualification_gfx942_mixed_duration_v1::{
    GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1 as BYTES,
    GFX942_MIXED_DURATION_QUALIFICATION_GEOMETRY_V1 as GEOMETRY,
    Gfx942MixedDurationQualificationArgumentsV1 as Arguments,
    Gfx942MixedDurationQualificationVariantV1 as Variant,
    gfx942_mixed_duration_qualification_initial_v1 as initial,
};
use crate::{RuntimeContextV1, RuntimePollV1};
use std::future::Future;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

mod observers;

struct ThreadWake(thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn await_bounded<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
            return result;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "mixed-duration owner deadline expired"
        );
        thread::park_timeout(remaining);
    }
}

fn long_receipt(backend: &KfdRuntimeBackendV1) -> Option<NativePendingReceipt> {
    let active = backend
        .active
        .iter()
        .chain(
            backend
                .auxiliary_compute_lanes
                .iter()
                .filter_map(|lane| lane.active.as_ref()),
        )
        .find(|active| {
            let kernel = &backend.kernels[&active.kernel];
            kernel.validated.selected_kernel().name() == Variant::Long.kernel_name()
                && backend.modules[&kernel.module].image_sha256 == Variant::Long.hsaco_sha256()
        })?;
    let Some(ActiveComputeExecutionV1::Materialized(batch)) = &active.execution else {
        return None;
    };
    let lane = backend.active_compute_lane_v1(active.id)?;
    let native_lane = backend.native_compute_lanes[lane]?;
    let receipt = backend
        .queue
        .as_ref()?
        .observe_retained_fixed_dispatch_v1(native_lane, batch)?;
    Some(NativePendingReceipt {
        id: active.id,
        lane,
        stream: active.stream,
        kernel: active.kernel,
        allocations: active.allocations.clone(),
        receipt,
        recipe: Arc::as_ptr(active.ordinary_recipe.as_ref()?) as usize,
        published_at: active.published_at,
    })
}

#[test]
#[ignore = "requires qualified short/long profiles and an isolated MI300X process"]
fn native_owned_later_short_completes_while_earlier_long_signal_is_pending() {
    use crate::{
        RuntimeAsyncEngineConfigV1, RuntimeAsyncOwnedDispositionV1, RuntimeAsyncOwnedEngineV1,
        RuntimeAsyncProgressConfigV1, RuntimeCompletionStatusV1,
    };

    let unique_id = native_device();
    let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
        move || -> Result<_, Box<dyn std::error::Error + Send + Sync>> {
            let mut backend =
                KfdRuntimeBackendV1::open_gfx942_mixed_duration_qualification_v1(unique_id)?;
            backend
                .configure_host_visible_backing_budget_v1(
                    Gfx942HostVisibleBackingBudgetV1::new(64 * 1024 * 1024, 128).unwrap(),
                )
                .map_err(|error| format!("host backing budget: {error:?}"))?;
            backend
                .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x78; 32], 128).unwrap())
                .map_err(|error| format!("profiler: {error:?}"))?;
            Ok(RuntimeContextV1::open(backend)?)
        },
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let prepared = await_bounded(
        handle
            .observer()
            .enqueue_with_context(|context| {
                let device = context.devices()[0].id();
                [Variant::Long, Variant::Short].map(|variant| {
                    let module = context.load_module(device, variant.hsaco()).unwrap();
                    let kernel = Arc::new(
                        context
                            .resolve_kernel::<Arguments>(module, variant.kernel_name())
                            .unwrap(),
                    );
                    let stream = context.create_stream(device).unwrap();
                    let allocation = context
                        .allocate(device, RuntimeMemoryKindV1::HostVisible, BYTES as u64, 4)
                        .unwrap();
                    context.write_allocation(allocation, 0, &initial()).unwrap();
                    (module, kernel, stream, allocation)
                })
            })
            .unwrap(),
    )
    .unwrap();
    let long = handle
        .launch_tracked(
            prepared[0].2,
            Arc::clone(&prepared[0].1),
            Arguments::new(prepared[0].3),
            GEOMETRY,
            Vec::new(),
        )
        .unwrap();
    let prefix_deadline = Instant::now() + Duration::from_secs(10);
    let prefix = loop {
        let receipt = await_bounded(
            handle
                .observer()
                .enqueue_with_context(|context| long_receipt(context.backend()))
                .unwrap(),
        )
        .unwrap();
        if receipt.is_some()
            || long.control().phase() == crate::RuntimeAsyncOperationPhaseV1::ObservationFinished
            || Instant::now() >= prefix_deadline
        {
            break receipt;
        }
    };
    let short = handle
        .launch_tracked(
            prepared[1].2,
            Arc::clone(&prepared[1].1),
            Arguments::new(prepared[1].3),
            GEOMETRY,
            Vec::new(),
        )
        .unwrap();
    assert!(!long.control().same_operation(&short.control()));
    let short_result = await_bounded(short).unwrap();
    assert_eq!(short_result.rejected_observations, 0);
    assert_eq!(
        short_result.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    let short_submission = short_result.submission.unwrap();
    assert_eq!(short_submission.stream(), prepared[1].2);
    let pending = await_bounded(
        handle
            .observer()
            .enqueue_with_context(|context| {
                let backend = context.backend_mut_for_test_v1();
                let before = long_receipt(backend);
                let status = before
                    .as_ref()
                    .map(|receipt| backend.poll_compute_submission_v1(receipt.lane, receipt.id));
                let after = long_receipt(backend);
                (before, status, after)
            })
            .unwrap(),
    )
    .unwrap();
    // A timing miss still drains and releases normally before the final assertion.
    let long_result = await_bounded(long).unwrap();
    assert_eq!(long_result.rejected_observations, 0);
    assert_eq!(
        long_result.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    let long_submission = long_result.submission.unwrap();
    assert_eq!(long_submission.stream(), prepared[0].2);
    let (ids, outputs, usage, profile_ids, profile) = await_bounded(
        handle
            .observer()
            .enqueue_with_context(move |context| {
                let ids = [
                    context
                        .backend_submission_for_test_v1(&long_submission)
                        .unwrap(),
                    context
                        .backend_submission_for_test_v1(&short_submission)
                        .unwrap(),
                ];
                let profile_ids = ids.map(|id| {
                    context
                        .backend()
                        .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id)
                        .unwrap()
                });
                let outputs = prepared.each_ref().map(|(_, _, _, allocation)| {
                    let mut observed = [0; BYTES];
                    context
                        .read_allocation(*allocation, 0, &mut observed)
                        .unwrap();
                    observed
                });
                context.release_submission(short_submission).unwrap();
                context.release_submission(long_submission).unwrap();
                for (module, kernel, stream, allocation) in prepared {
                    drop(kernel);
                    context.release_allocation(allocation).unwrap();
                    context.destroy_stream(stream).unwrap();
                    context.unload_module(module).unwrap();
                }
                assert!(context.cleanup().is_complete());
                let backend = context.backend_mut_for_test_v1();
                let usage = observe_shutdown(backend);
                assert_retired_shutdown_is_inert(backend);
                let profile = backend.finish_profiler_v1().unwrap();
                (ids, outputs, usage, profile_ids, profile)
            })
            .unwrap(),
    )
    .unwrap();
    let shutdown = engine.shutdown().unwrap();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert!(shutdown.cleanup.unwrap().is_complete());
    assert!(!shutdown.worker_panicked && shutdown.native_failure.is_none());
    for (variant, observed) in [Variant::Long, Variant::Short].into_iter().zip(outputs) {
        assert!(variant.validate_output(&observed));
        let hex = observed
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        println!("mixed_duration_owner variant={variant:?} observed_hex={hex}");
    }
    println!(
        "mixed_duration_owner ids={ids:?} prefix={prefix:?} after_short={pending:?} shutdown={usage:?} physical_overlap=not_measured"
    );
    println!(
        "mixed_duration_owner_profile_json={}",
        serde_json::to_string(&profile).unwrap()
    );
    assert!(profile.coverage.complete_runtime_operation_history);
    assert_eq!(profile.coverage.dropped_events, 0);
    assert_ne!(ids[0], ids[1]);
    let prefix = prefix.expect("long must be natively published before short enqueue");
    assert_eq!(prefix.id, ids[0]);
    assert_eq!(pending.0, Some(prefix));
    assert!(
        matches!(pending.1, Some(Ok(BackendPollV1::Pending))),
        "actual long completion signal must still be pending after short success"
    );
    assert_eq!(
        pending.0, pending.2,
        "the native pending poll preserves exact receipt custody"
    );
    let published = profile
        .events
        .iter()
        .filter_map(|entry| match entry.event {
            KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch, queue, ..
            } => Some((entry.sequence, dispatch, queue)),
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
    assert_eq!(published.len(), 2);
    assert_eq!(completed.len(), 2);
    assert_eq!([published[0].1, published[1].1], profile_ids);
    assert_eq!([completed[1].1, completed[0].1], profile_ids);
    assert_ne!(
        published[0].2, published[1].2,
        "distinct physical queue identities"
    );
    assert!(
        published[0].0 < published[1].0
            && published[1].0 < completed[0].0
            && completed[0].0 < completed[1].0
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
    assert_eq!(created.len(), 2);
    assert_eq!(destroyed.len(), 2);
    for (published_at, dispatch, queue) in published {
        let created_at = created.iter().find(|(_, id)| *id == queue).unwrap().0;
        let completed_at = completed.iter().find(|(_, id)| *id == dispatch).unwrap().0;
        let destroyed_at = destroyed.iter().find(|(_, id)| *id == queue).unwrap().0;
        assert!(
            created_at < published_at && published_at < completed_at && completed_at < destroyed_at
        );
    }
}

#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn native_mixed_duration_profiles_preserve_full_output_and_refund_backing() {
    for variant in [Variant::Short, Variant::Long] {
        let mut backend =
            KfdRuntimeBackendV1::open_gfx942_mixed_duration_qualification_v1(native_device())
                .unwrap();
        backend
            .configure_host_visible_backing_budget_v1(
                Gfx942HostVisibleBackingBudgetV1::new(64 * 1024 * 1024, 128).unwrap(),
            )
            .unwrap();
        let mut context = RuntimeContextV1::open(backend).unwrap();
        assert_eq!(context.devices().len(), 1);
        assert_eq!(context.devices()[0].target(), "gfx942:xnack-");
        let device = context.devices()[0].id();
        let module = context.load_module(device, variant.hsaco()).unwrap();
        let kernel = context
            .resolve_kernel::<Arguments>(module, variant.kernel_name())
            .unwrap();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, BYTES as u64, 4)
            .unwrap();
        context.write_allocation(allocation, 0, &initial()).unwrap();
        let arguments = Arguments::new(allocation);
        let mut submission = context
            .launch(stream, &kernel, &arguments, GEOMETRY, &[])
            .unwrap();
        context.flush_stream(stream).unwrap();
        assert_eq!(
            context
                .wait(&mut submission, Duration::from_secs(10))
                .unwrap(),
            RuntimePollV1::Succeeded
        );
        let mut observed = [0; BYTES];
        context
            .read_allocation(allocation, 0, &mut observed)
            .unwrap();
        assert!(
            variant.validate_output(&observed),
            "full output including guards"
        );
        let hex = observed
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        println!("mixed_duration variant={variant:?} observed_hex={hex}");
        context.release_submission(submission).unwrap();
        context.release_allocation(allocation).unwrap();
        context.destroy_stream(stream).unwrap();
        context.unload_module(module).unwrap();
        let mut backend = context.shutdown().unwrap();
        let usage = observe_shutdown(&mut backend);
        assert_retired_shutdown_is_inert(&mut backend);
        println!(
            "mixed_duration variant={variant:?} shutdown={usage:?} physical_overlap=not_measured"
        );
    }
}
