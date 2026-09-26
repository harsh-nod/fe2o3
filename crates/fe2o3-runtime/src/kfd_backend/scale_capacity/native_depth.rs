//! Internal qualification only. Retained receipts are not pending GPU signals.

use super::super::compute_state::RuntimeComputePipelineIdentityV1;
use super::*;
use fe2o3_profiler_protocol::{KfdRuntimeProfileEventV1, ProfileIdentityV1};
use fe2o3_resource_accounting::ResourceCreditUsageV1;

mod checker;
mod native_capacity;
mod tests;

const DEPTH: usize = 1024;
const LANES: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReceiptRow {
    id: u64,
    lane: usize,
    stream: u64,
    kernel: u64,
    allocations: [u64; 3],
    predecessor: Option<u64>,
    phase: Option<RuntimeComputePipelinePhaseV1>,
    native: [u8; 32],
    shape: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Custody {
    owners: VecDeque<RuntimeAllocationCustodyOwnerV1>,
    sole_stream: Option<u64>,
    counts: [usize; 2],
    capacity: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DepthSnapshot {
    rows: Vec<ReceiptRow>,
    membership: [u8; 32],
    pipeline_lengths: [usize; LANES],
    custody: HashMap<u64, Custody>,
    leases: HashMap<u64, usize>,
    tails: HashMap<u64, u64>,
    modules: HashMap<u64, usize>,
    dependencies: HashMap<u64, usize>,
    reservations: usize,
    usage: ResourceCreditUsageV1,
}

#[derive(Debug, PartialEq)]
struct NativeCut {
    depth: DepthSnapshot,
    runtime_identities: Vec<Option<RuntimeComputePipelineIdentityV1>>,
    recipes: Vec<usize>,
    published_at: Vec<Instant>,
    publications: Vec<KfdRuntimeProfileEventKindV1>,
    events: Vec<KfdRuntimeProfileEventV1>,
    next_handle: u64,
    host_backing: Option<fe2o3_kfd::Gfx942HostVisibleBackingUsageV1>,
    device_backing: Option<fe2o3_kfd::Gfx942DeviceBackingUsageV1>,
}

#[derive(Clone, Debug)]
struct Expected {
    ids: [Vec<u64>; LANES],
    streams: [u64; LANES],
    allocations: [[u64; 3]; LANES],
    kernel: u64,
    module: u64,
}

fn native_cut(backend: &KfdRuntimeBackendV1) -> NativeCut {
    assert!(backend.dispatch_capacity.is_scaled() && !backend.terminal);
    assert!(backend.sdma_allocation_ready_v1());
    assert!(backend.pending_compute.is_empty() && backend.pending_compute_streams.is_empty());
    assert!(backend.submissions.is_empty() && backend.active_sdma.is_empty());
    assert!(
        backend.active_sdma_streams.is_empty() && backend.published_sdma_submissions.is_empty()
    );
    assert!(backend.generated_shells.is_empty() && backend.generated_submissions.is_empty());
    assert!(backend.event_submission_retain_counts.is_empty());
    assert!(backend.sdma_dependency_retain_counts.is_empty());
    assert_eq!(backend.sdma_completion_reservations, 0);
    assert_eq!(backend.selected_compute_lane, 0);
    assert_eq!(backend.auxiliary_compute_lanes.len(), LANES - 1);
    let queue = backend.queue.as_ref().unwrap();
    let mut rows = Vec::with_capacity(LANES * DEPTH);
    let mut runtime_identities = Vec::with_capacity(LANES * DEPTH);
    let mut recipes = Vec::with_capacity(LANES * DEPTH);
    let mut published_at = Vec::with_capacity(LANES * DEPTH);
    let mut publications = Vec::with_capacity(LANES * DEPTH);
    let mut pipeline_lengths = [0; LANES];
    for (lane, pipeline_length) in pipeline_lengths.iter_mut().enumerate() {
        let (frontier, pipeline) = if lane == 0 {
            (backend.active.as_ref(), &backend.compute_pipeline)
        } else {
            let state = &backend.auxiliary_compute_lanes[lane - 1];
            (state.active.as_ref(), &state.pipeline)
        };
        *pipeline_length = pipeline.len();
        assert_eq!(pipeline.iter().count(), pipeline.len());
        let native_lane = backend.native_compute_lanes[lane].unwrap();
        assert_eq!(native_lane.ordinal(), lane);
        for active in frontier.into_iter().chain(pipeline.iter()) {
            let Some(ActiveComputeExecutionV1::Materialized(batch)) = &active.execution else {
                panic!("scale depth requires every original published native receipt");
            };
            let native = queue
                .observe_retained_fixed_dispatch_v1(native_lane, batch)
                .unwrap();
            for other in backend
                .native_compute_lanes
                .iter()
                .flatten()
                .filter(|other| **other != native_lane)
            {
                assert!(
                    queue
                        .observe_retained_fixed_dispatch_v1(*other, batch)
                        .is_none()
                );
            }
            let recipe = active.ordinary_recipe.as_ref().unwrap();
            assert_eq!(
                dispatch_shape_sha256_v1(&recipe.borrowed(), recipe.semantic_launch),
                active.dispatch_shape_sha256
            );
            let allocations: [u64; 3] = recipe
                .bindings
                .iter()
                .map(|binding| binding.region.allocation)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            assert_eq!(active.allocations, allocations.into_iter().collect());
            assert_eq!(active.stream, recipe.stream);
            assert_eq!(active.kernel, recipe.kernel);
            assert_eq!(
                active.deferred_ordered_predecessor_retain,
                active.ordered_predecessor.is_some()
            );
            assert_eq!(backend.active_compute_lane_v1(active.id), Some(lane));
            rows.push(ReceiptRow {
                id: active.id,
                lane,
                stream: active.stream,
                kernel: active.kernel,
                allocations,
                predecessor: active.ordered_predecessor,
                phase: pipeline.phase(active.id),
                native,
                shape: active.dispatch_shape_sha256,
            });
            runtime_identities.push(pipeline.identity_for_submission_v1(active.id));
            recipes.push(Arc::as_ptr(recipe) as usize);
            published_at.push(active.published_at);
            publications.push(KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch: backend
                    .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, active.id)
                    .unwrap(),
                queue: backend
                    .profile_resource_v1(
                        KfdProfileResourceKindV1::NativeQueue,
                        KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + lane as u64,
                    )
                    .unwrap(),
                stream: backend
                    .profile_resource_v1(KfdProfileResourceKindV1::Stream, recipe.stream)
                    .unwrap(),
                kernel: backend
                    .profile_resource_v1(KfdProfileResourceKindV1::Kernel, recipe.kernel)
                    .unwrap(),
                dispatch_shape: backend
                    .profile_content_v1(&active.dispatch_shape_sha256)
                    .unwrap(),
                launch: KfdProfileLaunchV1 {
                    grid: recipe.geometry.grid,
                    workgroup: recipe.geometry.workgroup,
                    dynamic_shared_bytes: recipe.geometry.dynamic_shared_bytes,
                },
                bindings: backend
                    .prepare_profile_bindings_v1(&recipe.bindings)
                    .unwrap()
                    .unwrap(),
            });
        }
    }
    let membership = checker::membership(&rows);
    NativeCut {
        depth: DepthSnapshot {
            rows,
            membership,
            pipeline_lengths,
            custody: backend
                .allocation_custody
                .iter()
                .map(|(&id, custody)| {
                    (
                        id,
                        Custody {
                            owners: custody.owners.clone(),
                            sole_stream: custody.sole_stream,
                            counts: custody.owner_counts,
                            capacity: custody.owners.capacity(),
                        },
                    )
                })
                .collect(),
            leases: backend.stream_compute_lanes.clone(),
            tails: backend.stream_submission_tails.clone(),
            modules: backend.compute_module_retain_counts.clone(),
            dependencies: backend.compute_dependency_retain_counts.clone(),
            reservations: backend.compute_completion_reservations,
            usage: backend.scale_qualification_host_table_usage_v1().unwrap(),
        },
        runtime_identities,
        recipes,
        published_at,
        publications,
        events: backend
            .profiler
            .as_ref()
            .unwrap()
            .recorded_events_for_test_v1()
            .to_vec(),
        next_handle: backend.next_handle,
        host_backing: queue.host_visible_backing_usage_v1(),
        device_backing: queue.device_backing_usage_v1(),
    }
}

#[test]
#[ignore = "requires an explicitly selected idle MI300X and isolated process; scale-qualification only"]
fn native_scaled_two_lane_2048_retained_receipts_and_cleanup() {
    use super::super::retained_release_tests::{native_device, observe_shutdown};
    use crate::qualification_gfx942_vecadd_v1::{
        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1 as ALIGNMENT,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as BYTES,
        Gfx942VecaddQualificationArgumentsV1 as Arguments, admit_gfx942_vecadd_qualification_v1,
    };
    use crate::{RuntimeCompletionStatusV1, RuntimeContextV1, RuntimePollV1};
    use std::io::Read;

    let unique_id = native_device();
    let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
    let buffers = admitted.host_buffers().unwrap();
    let mut backend = KfdRuntimeBackendV1::open_gfx942_vecadd_scale_qualification_v1(
        unique_id,
        64 * 1024 * 1024,
        32,
    )
    .unwrap();
    let account = backend.dispatch_capacity.account.as_ref().unwrap().clone();
    let runtime_only = account.usage();
    assert_eq!(runtime_only.retained_records, LANES);
    backend
        .configure_host_visible_backing_budget_v1(
            Gfx942HostVisibleBackingBudgetV1::new(128 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();
    let mut scope = [0; 32];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut scope)
        .unwrap();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new(scope, 16_384).unwrap())
        .unwrap();
    let mut context = RuntimeContextV1::open(backend).unwrap();
    assert_eq!(context.devices().len(), 1);
    assert_eq!(context.devices()[0].target(), "gfx942:xnack-");
    let device = context.devices()[0].id();
    let module = context.load_module(device, admitted.hsaco()).unwrap();
    let kernel = context
        .resolve_kernel::<Arguments>(module, admitted.kernel_name())
        .unwrap();
    assert_eq!(context.backend().kernels.len(), 1);
    assert_eq!(context.backend().modules.len(), 1);
    let mut expected = Expected {
        ids: std::array::from_fn(|_| Vec::with_capacity(DEPTH)),
        streams: [0; LANES],
        allocations: [[0; 3]; LANES],
        kernel: *context.backend().kernels.keys().next().unwrap(),
        module: *context.backend().modules.keys().next().unwrap(),
    };
    let mut prepared = Vec::with_capacity(LANES);
    for lane in 0..LANES {
        let stream = context.create_stream(device).unwrap();
        let new_streams: Vec<_> = context
            .backend()
            .streams
            .keys()
            .copied()
            .filter(|id| !expected.streams.contains(id))
            .collect();
        assert_eq!(new_streams.len(), 1);
        expected.streams[lane] = new_streams[0];
        let mut allocations = Vec::with_capacity(3);
        for (index, bytes) in [buffers.left(), buffers.right(), buffers.output()]
            .into_iter()
            .enumerate()
        {
            let allocation = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::HostVisible,
                    BYTES as u64,
                    ALIGNMENT,
                )
                .unwrap();
            context.write_allocation(allocation, 0, bytes).unwrap();
            let new_allocations: Vec<_> = context
                .backend()
                .allocations
                .keys()
                .copied()
                .filter(|id| {
                    !expected
                        .allocations
                        .iter()
                        .flatten()
                        .any(|prior| prior == id)
                })
                .collect();
            assert_eq!(new_allocations.len(), 1);
            expected.allocations[lane][index] = new_allocations[0];
            allocations.push(allocation);
        }
        let allocations: [_; 3] = allocations.try_into().unwrap();
        let arguments = Arguments::new(allocations[0], allocations[1], allocations[2]).unwrap();
        prepared.push((stream, allocations, arguments));
    }
    let mut submissions: [Vec<_>; LANES] = std::array::from_fn(|_| Vec::with_capacity(DEPTH));
    for _ in 0..DEPTH {
        for lane in 0..LANES {
            let submission = context
                .launch(
                    prepared[lane].0,
                    &kernel,
                    &prepared[lane].2,
                    admitted.geometry(),
                    &[],
                )
                .unwrap();
            context.flush_stream(prepared[lane].0).unwrap();
            expected.ids[lane].push(context.backend_submission_for_test_v1(&submission).unwrap());
            submissions[lane].push(submission);
        }
    }
    // No wait, completion poll or readback may precede this full retained cut.
    let cut = native_cut(context.backend());
    let depth_check = checker::check_depth(&cut.depth, &expected);
    let context_cut = context.snapshot_async_drain_v1().unwrap();
    let context_counts = context.async_drain_counts_v1();
    let initially_pending = submissions.iter().flatten().all(|submission| {
        context.query_submission(submission).unwrap() == RuntimeCompletionStatusV1::Pending
    });
    let mut rejection_checks = Vec::with_capacity(LANES);
    let mut unexpected_runtime = Vec::with_capacity(LANES);
    for (stream, _, arguments) in &prepared {
        let rejected = match context.launch(*stream, &kernel, arguments, admitted.geometry(), &[]) {
            Ok(submission) => {
                unexpected_runtime.push(submission);
                false
            }
            Err(crate::RuntimeErrorV1::BackendRejected(error)) => {
                error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
                    && error.detail() == "KFD per-allocation custody owner capacity exceeded"
            }
            Err(error) => panic!("runtime capacity probe lost nonterminal admission: {error:?}"),
        };
        if !unexpected_runtime.is_empty() {
            rejection_checks.push((false, false, false));
            continue;
        }
        let backend_unchanged = native_cut(context.backend()) == cut;
        let context_unchanged = context.snapshot_async_drain_v1().unwrap() == context_cut
            && context.async_drain_counts_v1() == context_counts;
        let tokens_unchanged = submissions.iter().zip(&expected.ids).all(|(tokens, ids)| {
            tokens.iter().zip(ids).all(|(token, &id)| {
                context.backend_submission_for_test_v1(token) == Ok(id)
                    && context.query_submission(token).unwrap()
                        == RuntimeCompletionStatusV1::Pending
            })
        });
        rejection_checks.push((
            rejected && backend_unchanged,
            context_unchanged,
            tokens_unchanged,
        ));
    }
    let mut native_rejection_checks = Vec::with_capacity(LANES);
    if unexpected_runtime.is_empty() {
        for lane in 0..LANES {
            let rejected = native_capacity::probe(context.backend_mut_for_test_v1(), lane);
            native_rejection_checks.push((
                rejected,
                native_cut(context.backend()) == cut,
                context.snapshot_async_drain_v1().unwrap() == context_cut
                    && context.async_drain_counts_v1() == context_counts,
            ));
        }
    }
    let profile_ids = expected.ids.each_ref().map(|ids| {
        ids.iter()
            .map(|&id| {
                context
                    .backend()
                    .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id)
                    .unwrap()
            })
            .collect::<Vec<_>>()
    });
    let queues = [0, 1].map(|lane| {
        context
            .backend()
            .profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + lane,
            )
            .unwrap()
    });
    let streams = expected.streams.map(|id| {
        context
            .backend()
            .profile_resource_v1(KfdProfileResourceKindV1::Stream, id)
            .unwrap()
    });
    for lane_submissions in &mut submissions {
        assert_eq!(
            context
                .wait(
                    lane_submissions.last_mut().unwrap(),
                    Duration::from_secs(60)
                )
                .unwrap(),
            RuntimePollV1::Succeeded
        );
        for submission in lane_submissions {
            assert_eq!(context.poll(submission).unwrap(), RuntimePollV1::Succeeded);
        }
    }
    for submission in &mut unexpected_runtime {
        assert_eq!(
            context.wait(submission, Duration::from_secs(60)).unwrap(),
            RuntimePollV1::Succeeded
        );
        assert_eq!(context.poll(submission).unwrap(), RuntimePollV1::Succeeded);
    }
    let mut observed = vec![0; BYTES];
    let mut hashes = Vec::with_capacity(6);
    let mut output_checks = Vec::with_capacity(6);
    for (_, allocations, _) in &prepared {
        for (allocation, expected_bytes) in
            allocations
                .iter()
                .zip([buffers.left(), buffers.right(), buffers.expected_output()])
        {
            context
                .read_allocation(*allocation, 0, &mut observed)
                .unwrap();
            output_checks.push(observed == expected_bytes);
            hashes.push(
                Sha256::digest(&observed)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            );
        }
    }
    for submission in unexpected_runtime.into_iter().rev() {
        context.release_submission(submission).unwrap();
    }
    for lane_submissions in submissions {
        for submission in lane_submissions.into_iter().rev() {
            context.release_submission(submission).unwrap();
        }
    }
    for (stream, allocations, _) in prepared {
        for allocation in allocations {
            context.release_allocation(allocation).unwrap();
        }
        context.destroy_stream(stream).unwrap();
    }
    context.unload_module(module).unwrap();
    let mut backend = context.shutdown().unwrap();
    let runtime_empty = backend.allocation_custody.is_empty()
        && backend.compute_dependency_retain_counts.is_empty();
    let host_usage = observe_shutdown(&mut backend);
    let after_shutdown = account.usage();
    let profile = backend.finish_profiler_v1().unwrap();
    drop(backend);
    let disposed = account.usage();
    // Routine evidence failures must not skip successful native teardown.
    depth_check.unwrap();
    assert!(initially_pending);
    assert_eq!(context_counts.total_submissions, LANES * DEPTH);
    assert_eq!(context_counts.pending, LANES * DEPTH);
    assert!(
        rejection_checks
            .iter()
            .all(|checks| *checks == (true, true, true))
    );
    assert!(
        native_rejection_checks
            .iter()
            .all(|checks| *checks == (true, true, true)),
        "native capacity rejection checks: {native_rejection_checks:?}"
    );
    assert_eq!(native_rejection_checks.len(), LANES);
    assert!(output_checks.into_iter().all(|correct| correct));
    assert!(runtime_empty);
    assert_eq!(after_shutdown, runtime_only);
    profile.validate().unwrap();
    assert!(
        profile.coverage.complete_runtime_operation_history && profile.coverage.dropped_events == 0
    );
    assert_eq!(
        profile.events.get(..cut.events.len()),
        Some(cut.events.as_slice())
    );
    checker::check_profile(
        &profile.events,
        &profile_ids,
        queues,
        streams,
        &cut.publications,
    )
    .unwrap();
    checker::check_disposed(disposed).unwrap();
    let receipt_rows = cut
        .depth
        .rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "submission": row.id, "lane": row.lane, "stream": row.stream, "kernel": row.kernel,
                "allocations": row.allocations, "ordered_predecessor": row.predecessor,
                "pipeline_phase": row.phase.map(|phase| match phase {
                    RuntimeComputePipelinePhaseV1::Published => "published",
                    RuntimeComputePipelinePhaseV1::Completed => "completed",
                    RuntimeComputePipelinePhaseV1::PhysicallyRetired => "physically-retired",
                    RuntimeComputePipelinePhaseV1::Quarantined => "quarantined",
                }),
                "native_receipt": row.native, "dispatch_shape": row.shape,
            })
        })
        .collect::<Vec<_>>();
    println!(
        "scale_depth_receipts_json={}",
        serde_json::json!({
            "schema": "fe2o3.scale-retained-depth-development.v1", "unique_id": unique_id,
            "hsaco_sha256": admitted.hsaco_sha256(), "depth_per_lane": DEPTH,
            "native_retained": cut.depth.rows.len(), "membership": cut.depth.membership,
            "receipts": receipt_rows, "host_table_peak_records": cut.depth.usage.retained_records,
            "host_table_peak_payload_bytes": cut.depth.usage.used.get(ResourceKindV1::ControlResidentBytes),
            "final_buffer_sha256": hashes, "runtime_capacity_negative": "unchanged-retained-cut",
            "native_capacity_negative": "rejected-before-side-effect-1024-each-lane",
            "unfinished_gpu_count": null,
            "physical_overlap": "unmeasured", "host_table_final_records": disposed.retained_records,
            "cleanup": "complete",
        })
    );
    println!("profile_json={}", serde_json::to_string(&profile).unwrap());
    println!("scale_host_shutdown={host_usage:?}");
}
