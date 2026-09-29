//! Allocation reuse/replacement with native epoch continuity, not recipe identity
//! or a simultaneous native-depth witness.

use super::*;
use crate::qualification_gfx942_vecadd_v1::{
    GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1 as ALIGNMENT,
    GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as BYTES,
    GFX942_VECADD_QUALIFICATION_GEOMETRY_V1 as GEOMETRY,
    Gfx942VecaddQualificationArgumentsV1 as Arguments, admit_gfx942_vecadd_qualification_v1,
};
use crate::{
    RuntimeAllocationIdV1, RuntimeAsyncEngineCallErrorV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncLaunchRequestV1, RuntimeAsyncOperationResultV1, RuntimeAsyncOwnedDispositionV1,
    RuntimeAsyncOwnedEngineV1, RuntimeAsyncProgressConfigV1, RuntimeAsyncProgressHandleV1,
    RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeModuleIdV1, RuntimeStreamIdV1,
    TypedRuntimeKernelV1,
};
use std::future::Future;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

const OPERATIONS: usize = LANES * DEPTH;
// Three 2048-operation waves exceed the frozen V1 16384-event profile ceiling.
// Qualify reuse and replacement in separate processes, with two waves each.
const WAVES: usize = 2;
const PROFILE_CAPACITY: u32 = 16_384;

struct ThreadWake(thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn await_bounded<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
            return result;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "scaled owner observation deadline expired"
        );
        thread::park_timeout(remaining);
    }
}

struct ReleaseGate(SyncSender<()>);

impl Drop for ReleaseGate {
    fn drop(&mut self) {
        let _ = self.0.try_send(());
    }
}

struct Prepared {
    module: RuntimeModuleIdV1,
    kernel: Arc<TypedRuntimeKernelV1<Arguments>>,
    streams: [RuntimeStreamIdV1; LANES],
    allocations: [[RuntimeAllocationIdV1; 3]; LANES],
}

impl Prepared {
    fn request(&self, lane: usize) -> RuntimeAsyncLaunchRequestV1<Arguments> {
        let allocations = self.allocations[lane];
        RuntimeAsyncLaunchRequestV1::new(
            self.streams[lane],
            Arc::clone(&self.kernel),
            &Arguments::new(allocations[0], allocations[1], allocations[2]).unwrap(),
            GEOMETRY,
            Vec::new(),
        )
        .unwrap()
    }
}

fn allocate_triplets(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
) -> [[RuntimeAllocationIdV1; 3]; LANES] {
    let buffers = admit_gfx942_vecadd_qualification_v1()
        .unwrap()
        .host_buffers()
        .unwrap();
    let device = context.devices()[0].id();
    std::array::from_fn(|_| {
        [buffers.left(), buffers.right(), buffers.output()].map(|bytes| {
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
        })
    })
}

fn sampled_receipts(backend: &KfdRuntimeBackendV1) -> Vec<serde_json::Value> {
    let Some(queue) = &backend.queue else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for lane in 0..LANES {
        let (frontier, pipeline) = if lane == 0 {
            (backend.active.as_ref(), &backend.compute_pipeline)
        } else {
            let state = &backend.auxiliary_compute_lanes[lane - 1];
            (state.active.as_ref(), &state.pipeline)
        };
        for active in frontier.into_iter().chain(pipeline.iter()) {
            if let Some(ActiveComputeExecutionV1::Materialized(
                MaterializedCompletionReceiptV1::Published(batch),
            )) = &active.execution
            {
                let native_lane = backend.native_compute_lanes[lane].unwrap();
                let receipt = queue
                    .observe_retained_fixed_dispatch_v1(native_lane, batch)
                    .expect("published owner retains its original native receipt");
                rows.push(serde_json::json!({
                    "submission": active.id, "lane": lane, "stream": active.stream,
                    "native_receipt": receipt, "dispatch_shape": active.dispatch_shape_sha256,
                }));
            }
        }
    }
    rows
}

fn fail_owned_observation(
    engine: RuntimeAsyncOwnedEngineV1<KfdRuntimeBackendV1>,
    handle: &RuntimeAsyncProgressHandleV1<KfdRuntimeBackendV1>,
    wave: usize,
    ordinal: usize,
    mut result: RuntimeAsyncOperationResultV1<Arguments, KfdRuntimeBackendErrorV1>,
) -> ! {
    let submission = result.submission.take();
    let snapshot = handle.observer().enqueue_with_context(move |context| {
        let submission = submission.as_ref()
            .and_then(|submission| context.backend_submission_for_test_v1(submission).ok());
        let backend = context.backend();
        let first_failure = take_unpublished_compute_failure_for_test_v1().map(|failure| {
            serde_json::json!({
                "submission": failure.submission, "stream": failure.stream,
                "stage": failure.stage, "kind": format!("{:?}", failure.kind),
                "quiescent": failure.quiescent, "detail": failure.detail(),
                "detail_truncated": failure.detail_truncated,
            })
        });
        serde_json::json!({
            "submission": submission,
            "submission_published": submission.and_then(|id| backend.submissions.get(&id))
                .map(|record| record.profile_dispatch_published),
            "context_terminal": context.is_terminal(), "backend_terminal": backend.terminal,
            "pending_compute": backend.pending_compute.len(),
            "completed_successfully": backend.submissions.values()
                .filter(|record| record.status == BackendPollV1::Succeeded).count(),
            "completed_failed": backend.submissions.values()
                .filter(|record| matches!(record.status, BackendPollV1::Failed { .. })).count(),
            "active_lanes": usize::from(backend.active.is_some()) + backend.auxiliary_compute_lanes.iter()
                .filter(|lane| lane.active.is_some()).count(),
            "retained_pipeline": backend.compute_pipeline.len() + backend.auxiliary_compute_lanes.iter()
                .map(|lane| lane.pipeline.len()).sum::<usize>(),
            "native_dirty_extents": backend.native_dirty_extents,
            "allocations_without_content_hash": backend.allocations.ordinary_iter()
                .filter(|(_, allocation)| allocation.content_sha256.is_none()).count(),
            "first_unpublished_failure": first_failure,
        })
    });
    let state = match snapshot {
        Ok(snapshot) => match await_bounded(snapshot) {
            Ok(state) => state,
            Err(error) => serde_json::json!({"snapshot_error": format!("{error:?}")}),
        },
        Err(error) => serde_json::json!({"snapshot_enqueue_error": format!("{error:?}")}),
    };
    eprintln!("scale_owner_failure_json={}", serde_json::json!({
        "wave": wave, "ordinal": ordinal, "state": state,
        "observation": format!("{:?}", result.observation),
        "rejected_observations": result.rejected_observations,
        "last_rejected_observation": format!("{:?}", result.last_rejected_observation),
    }));
    // Stop and join through the owning engine. Failure never authorizes manual
    // native teardown, and a retained disposition is not cleanup success.
    let shutdown = match engine.shutdown() {
        Ok(shutdown) => serde_json::json!({
            "disposition": format!("{:?}", shutdown.disposition),
            "worker_panicked": shutdown.worker_panicked,
            "native_failure": format!("{:?}", shutdown.native_failure),
            "cleanup_complete": shutdown.cleanup.as_ref().map(|report| report.is_complete()),
            "cleanup_failure_count": shutdown.cleanup.as_ref().map(|report| report.failures().len()),
            "retained": shutdown.cleanup.as_ref().map(|report| format!("{:?}", report.retained())),
        }),
        Err(error) => serde_json::json!({"join_error": format!("{error:?}")}),
    };
    eprintln!("scale_owner_failure_shutdown_json={shutdown}");
    panic!("scaled owner observation failed at wave {wave}, ordinal {ordinal}");
}

#[test]
#[ignore = "requires an idle selected MI300X, isolated process and external process-group deadline"]
fn native_scaled_owned_two_waves_2048_operations_reuse_and_cleanup() {
    run_owned_waves(false);
}

#[test]
#[ignore = "requires an idle selected MI300X, isolated process and external process-group deadline"]
fn native_scaled_owned_two_waves_2048_operations_replacement_and_cleanup() {
    run_owned_waves(true);
}

fn run_owned_waves(replace: bool) {
    use super::super::super::retained_release_tests::{native_device, observe_shutdown};

    let unique_id = native_device();
    let config =
        RuntimeAsyncEngineConfigV1::new(OPERATIONS, OPERATIONS, 1024, 64, Duration::from_millis(1))
            .unwrap()
            .with_reply_capacity(OPERATIONS + 2)
            .unwrap();
    let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
        move || -> Result<_, Box<dyn std::error::Error + Send + Sync>> {
            let mut backend = KfdRuntimeBackendV1::open_gfx942_vecadd_scale_qualification_v1(
                unique_id,
                64 * 1024 * 1024,
                32,
            )?;
            backend
                .configure_host_visible_backing_budget_v1(NATIVE_BACKING_BUDGET)
                .map_err(|error| format!("host backing budget: {error:?}"))?;
            let mut scope = [0; 32];
            std::io::Read::read_exact(&mut std::fs::File::open("/dev/urandom")?, &mut scope)?;
            backend
                .enable_profiler_v1(
                    KfdRuntimeProfilerConfigV1::new(scope, PROFILE_CAPACITY).unwrap(),
                )
                .map_err(|error| format!("profiler: {error:?}"))?;
            reset_unpublished_compute_failure_for_test_v1();
            Ok(RuntimeContextV1::open(backend)?)
        },
        config,
        RuntimeAsyncProgressConfigV1::new(LANES, LANES).unwrap(),
    )
    .unwrap();
    let (mut prepared, account, runtime_only) = await_bounded(
        handle
            .observer()
            .enqueue_with_context(|context| {
                assert_eq!(context.devices().len(), 1);
                assert_eq!(context.devices()[0].target(), "gfx942:xnack-");
                let account = context
                    .backend()
                    .dispatch_capacity
                    .account
                    .as_ref()
                    .unwrap()
                    .clone();
                let runtime_only = account.usage();
                assert_eq!(runtime_only.retained_records, LANES);
                let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
                let device = context.devices()[0].id();
                let module = context.load_module(device, admitted.hsaco()).unwrap();
                let kernel = Arc::new(
                    context
                        .resolve_kernel::<Arguments>(module, admitted.kernel_name())
                        .unwrap(),
                );
                let streams = std::array::from_fn(|_| context.create_stream(device).unwrap());
                let allocations = allocate_triplets(context);
                (
                    Prepared {
                        module,
                        kernel,
                        streams,
                        allocations,
                    },
                    account,
                    runtime_only,
                )
            })
            .unwrap(),
    )
    .unwrap();
    let original_allocations = prepared.allocations;
    let mut wave_records = Vec::with_capacity(WAVES);
    let mut recycled_snapshots = Vec::with_capacity(WAVES);
    let mut expected_profiles = Vec::with_capacity(WAVES * OPERATIONS);
    let mut all_ids = HashSet::with_capacity(WAVES * OPERATIONS);
    let mut checks = Vec::new();
    for wave in 0..WAVES {
        if wave == 1 && !replace {
            let allocations = prepared.allocations;
            await_bounded(
                handle
                    .observer()
                    .enqueue_with_context(move |context| {
                        let buffers = admit_gfx942_vecadd_qualification_v1()
                            .unwrap()
                            .host_buffers()
                            .unwrap();
                        for triplet in allocations {
                            context
                                .write_allocation(triplet[2], 0, buffers.output())
                                .unwrap();
                        }
                    })
                    .unwrap(),
            )
            .unwrap();
        } else if wave == 1 {
            // Keep the old IDs live through replacement admission so replacement
            // cannot silently reuse their logical identities.
            let old = prepared.allocations;
            prepared.allocations = await_bounded(
                handle
                    .observer()
                    .enqueue_with_context(move |context| {
                        let replacement = allocate_triplets(context);
                        assert!(
                            replacement
                                .iter()
                                .flatten()
                                .all(|id| !old.iter().flatten().any(|prior| prior == id))
                        );
                        for allocation in old.into_iter().flatten() {
                            context.release_allocation(allocation).unwrap();
                        }
                        replacement
                    })
                    .unwrap(),
            )
            .unwrap();
        }
        let (entered_tx, entered_rx) = sync_channel(1);
        let (release_tx, release_rx) = sync_channel(1);
        let release = ReleaseGate(release_tx);
        let gate = handle
            .observer()
            .enqueue_with_context(move |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(30))
            })
            .unwrap();
        entered_rx.recv_timeout(Duration::from_secs(30)).unwrap();
        let mut operations = Vec::with_capacity(OPERATIONS);
        let mut snapshot_bytes = 0;
        for ordinal in 0..OPERATIONS {
            let request = prepared.request(ordinal % LANES);
            snapshot_bytes += request.snapshot_bytes();
            operations.push(handle.enqueue_launch_tracked(request).unwrap());
        }
        let before = (
            handle.observer().snapshot_bytes_in_use(),
            handle.observer().reply_cells_in_use(),
        );
        let rejected = handle.enqueue_launch_tracked(prepared.request(0));
        let after = (
            handle.observer().snapshot_bytes_in_use(),
            handle.observer().reply_cells_in_use(),
        );
        drop(release);
        await_bounded(gate).unwrap().unwrap();
        checks.push(matches!(
            rejected,
            Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
        ));
        checks.push(
            snapshot_bytes > 0 && before == (snapshot_bytes, OPERATIONS + 1) && after == before,
        );
        let mut returned = Vec::with_capacity(OPERATIONS);
        let mut retention_sample = Vec::new();
        for (ordinal, operation) in operations.into_iter().enumerate() {
            let result = await_bounded(operation).unwrap();
            checks.push(result.rejected_observations == 0);
            if !matches!(result.observation, Ok(RuntimeCompletionStatusV1::Succeeded)) {
                fail_owned_observation(engine, &handle, wave, ordinal, result);
            }
            let submission = result.submission.unwrap();
            checks.push(submission.stream() == prepared.streams[ordinal % LANES]);
            returned.push(submission);
            if ordinal == 0 {
                retention_sample = await_bounded(
                    handle
                        .observer()
                        .enqueue_with_context(|context| sampled_receipts(context.backend()))
                        .unwrap(),
                )
                .unwrap();
            }
        }
        checks.push(handle.observer().snapshot_bytes_in_use() == 0);
        checks.push(handle.observer().reply_cells_in_use() == 0);
        let allocations = prepared.allocations;
        let streams = prepared.streams;
        let (ids, profiles, buffers_ok, hashes, usage, settled, recycled) = await_bounded(
            handle
                .observer()
                .enqueue_with_context(move |context| {
                    let ids = returned
                        .iter()
                        .map(|submission| {
                            context.backend_submission_for_test_v1(submission).unwrap()
                        })
                        .collect::<Vec<_>>();
                    let profiles = ids
                        .iter()
                        .enumerate()
                        .map(|(ordinal, id)| {
                            let backend = context.backend();
                            let dispatch = backend
                                .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, *id)
                                .unwrap();
                            let stream = backend.submissions[id].stream;
                            assert_eq!(
                                context.query_submission(&returned[ordinal]).unwrap(),
                                RuntimeCompletionStatusV1::Succeeded
                            );
                            (
                                dispatch,
                                (
                                    backend
                                        .profile_resource_v1(
                                            KfdProfileResourceKindV1::Stream,
                                            stream,
                                        )
                                        .unwrap(),
                                    wave,
                                ),
                            )
                        })
                        .collect::<Vec<_>>();
                    let buffers = admit_gfx942_vecadd_qualification_v1()
                        .unwrap()
                        .host_buffers()
                        .unwrap();
                    let mut observed = vec![0; BYTES];
                    let mut buffers_ok = true;
                    let mut hashes = Vec::with_capacity(LANES * 3);
                    for triplet in allocations {
                        for (allocation, expected) in triplet.into_iter().zip([
                            buffers.left(),
                            buffers.right(),
                            buffers.expected_output(),
                        ]) {
                            context
                                .read_allocation(allocation, 0, &mut observed)
                                .unwrap();
                            buffers_ok &= observed == expected;
                            hashes.push(
                                Sha256::digest(&observed)
                                    .iter()
                                    .map(|byte| format!("{byte:02x}"))
                                    .collect::<String>(),
                            );
                        }
                    }
                    for submission in returned.into_iter().rev() {
                        context.release_submission(submission).unwrap();
                    }
                    let counts = context.async_drain_counts_v1();
                    let observations = streams.map(|stream| context.query_stream(stream).unwrap());
                    let backend = context.backend_mut_for_test_v1();
                    let settled = counts.total_submissions == 0
                        && observations
                            .iter()
                            .all(|observation| observation.total_submissions == 0)
                        && backend.active.is_none()
                        && backend.compute_pipeline.is_empty()
                        && backend
                            .auxiliary_compute_lanes
                            .iter()
                            .all(|lane| lane.active.is_none() && lane.pipeline.is_empty())
                        && backend.allocation_custody.is_empty()
                        && backend.compute_dependency_retain_counts.is_empty()
                        && backend.compute_completion_reservations == 0;
                    let native_lanes: [_; LANES] =
                        std::array::from_fn(|lane| backend.native_compute_lanes[lane]);
                    let queue = backend
                        .queue
                        .as_mut()
                        .expect("settled lanes retain their queue");
                    let recycled: [(u64, usize); LANES] = std::array::from_fn(|lane| {
                        queue
                            .with_compute_lane_v1(
                                native_lanes[lane].expect("each stream retains a native lane"),
                                |selected| selected.recycled_fixed_dispatch_data_shape_v1(),
                            )
                            .unwrap()
                            .unwrap()
                    });
                    (
                        ids,
                        profiles,
                        buffers_ok,
                        hashes,
                        backend.scale_qualification_host_table_usage_v1().unwrap(),
                        settled,
                        recycled,
                    )
                })
                .unwrap(),
        )
        .unwrap();
        checks.push(ids.len() == OPERATIONS && ids.iter().all(|id| all_ids.insert(*id)));
        let positions: HashMap<_, _> = ids
            .iter()
            .enumerate()
            .map(|(ordinal, &id)| (id, ordinal % LANES))
            .collect();
        let mut sampled_ids = HashSet::new();
        let mut sampled_native = HashSet::new();
        checks.push(retention_sample.iter().all(|row| {
            let id = row["submission"].as_u64().unwrap();
            let lane = row["lane"].as_u64().unwrap() as usize;
            positions.get(&id) == Some(&lane)
                && sampled_ids.insert(id)
                && sampled_native.insert(row["native_receipt"].to_string())
        }));
        checks.push(buffers_ok && settled);
        recycled_snapshots.push(recycled);
        checks
            .push(!usage.poisoned && usage.reserved_records == 0 && usage.quarantined_records == 0);
        expected_profiles.extend(profiles);
        wave_records.push(serde_json::json!({
            "wave": wave, "operations": ids.len(), "submission_ids": ids,
            "buffer_mode": if wave == 0 { "initial" } else if replace { "replacement-allocations" } else { "same-allocations" },
            "command_pressure": { "rejected": "CommandQueueFull", "snapshot_bytes": before.0, "reply_cells": before.1, "refund_unchanged": before == after },
            "retention_sample": retention_sample, "retention_sample_count": retention_sample.len(),
            "retention_sample_is_peak": false, "retention_sample_coverage": "diagnostic-not-guaranteed",
            "recycled_lanes": recycled.iter().enumerate().map(|(lane, &(generation, data_count))| {
                serde_json::json!({"lane": lane, "generation": generation, "data_count": data_count})
            }).collect::<Vec<_>>(),
            "all_six_buffers_correct": buffers_ok, "final_buffer_sha256": hashes,
            "settled": settled, "host_table_retained_records": usage.retained_records,
            "host_table_payload_bytes": usage.used.get(ResourceKindV1::ControlResidentBytes),
        }));
    }
    checks.push(if replace {
        prepared.allocations.iter().flatten().all(|id| {
            !original_allocations
                .iter()
                .flatten()
                .any(|prior| prior == id)
        })
    } else {
        prepared.allocations == original_allocations
    });
    let (host_usage, after_shutdown, profile) = await_bounded(
        handle
            .observer()
            .enqueue_with_context(move |context| {
                let Prepared {
                    module,
                    kernel,
                    streams,
                    allocations,
                } = prepared;
                drop(kernel);
                for allocation in allocations.into_iter().flatten() {
                    context.release_allocation(allocation).unwrap();
                }
                for stream in streams {
                    context.destroy_stream(stream).unwrap();
                }
                context.unload_module(module).unwrap();
                assert!(context.cleanup().is_complete());
                let backend = context.backend_mut_for_test_v1();
                let host_usage = observe_shutdown(backend);
                let after_shutdown = backend.scale_qualification_host_table_usage_v1().unwrap();
                let profile = backend.finish_profiler_v1().unwrap();
                (host_usage, after_shutdown, profile)
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
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
    assert_eq!(after_shutdown, runtime_only);
    checker::check_disposed(account.usage()).unwrap();
    profile.validate().unwrap();
    assert!(profile.coverage.complete_runtime_operation_history);
    assert_eq!(profile.coverage.dropped_events, 0);
    check_lifecycles(&profile.events, &expected_profiles, replace);
    for (wave, recycled) in recycled_snapshots.into_iter().enumerate() {
        assert_eq!(
            recycled,
            [(((wave + 1) * DEPTH) as u64, 3); LANES],
            "every settled lane preserves the exact native epoch progression"
        );
    }
    assert!(checks.into_iter().all(|passed| passed));
    println!(
        "scale_owner_waves_json={}",
        serde_json::json!({
            "schema": "fe2o3.scale-owner-waves-development.v1", "unique_id": unique_id,
            "waves": wave_records, "total_operations": WAVES * OPERATIONS, "replacement": replace,
            "unfinished_gpu_count": null, "physical_overlap": "unmeasured",
            "native_recipe_occurrence_observed": false,
            "protected_generated_execution": false, "cleanup": "complete",
            "host_table_final_records": account.usage().retained_records,
        })
    );
    println!("profile_json={}", serde_json::to_string(&profile).unwrap());
    println!("scale_owner_host_shutdown={host_usage:?}");
}

fn check_lifecycles(
    events: &[KfdRuntimeProfileEventV1],
    expected: &[(ProfileIdentityV1, (ProfileIdentityV1, usize))],
    replace: bool,
) {
    let expected: HashMap<_, _> = expected.iter().copied().collect();
    assert_eq!(expected.len(), WAVES * OPERATIONS);
    let mut published = HashMap::new();
    let mut completed = HashMap::new();
    let mut released = HashMap::new();
    let mut created = HashMap::new();
    let mut destroyed = HashMap::new();
    let mut stream_queues = HashMap::new();
    let mut wave_shapes = HashMap::new();
    for entry in events {
        match entry.event {
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue } => {
                assert!(created.insert(queue, entry.sequence).is_none());
            }
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue } => {
                assert!(destroyed.insert(queue, entry.sequence).is_none());
            }
            KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                queue,
                stream,
                dispatch_shape,
                ..
            } => {
                let &(expected_stream, wave) = expected.get(&dispatch).unwrap();
                assert_eq!(expected_stream, stream);
                assert_eq!(*stream_queues.entry(stream).or_insert(queue), queue);
                assert_eq!(
                    *wave_shapes.entry((wave, stream)).or_insert(dispatch_shape),
                    dispatch_shape
                );
                assert!(
                    published
                        .insert(dispatch, (entry.sequence, queue))
                        .is_none()
                );
            }
            KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } => {
                assert!(expected.contains_key(&dispatch));
                assert!(completed.insert(dispatch, entry.sequence).is_none());
            }
            KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch } => {
                assert!(expected.contains_key(&dispatch));
                assert!(released.insert(dispatch, entry.sequence).is_none());
            }
            _ => {}
        }
    }
    assert_eq!(published.len(), expected.len());
    assert_eq!(completed.len(), expected.len());
    assert_eq!(released.len(), expected.len());
    let mut compute_queues = HashSet::new();
    for dispatch in expected.keys() {
        let (publication, queue) = published[dispatch];
        compute_queues.insert(queue);
        assert!(created[&queue] < publication);
        assert!(publication < completed[dispatch]);
        assert!(completed[dispatch] < released[dispatch]);
        assert!(released[dispatch] < destroyed[&queue]);
    }
    assert_eq!(compute_queues.len(), LANES);
    assert_eq!(stream_queues.len(), LANES);
    assert_eq!(wave_shapes.len(), WAVES * LANES);
    for stream in stream_queues.keys() {
        assert_eq!(
            wave_shapes[&(0, *stream)] != wave_shapes[&(1, *stream)],
            replace
        );
    }
}
