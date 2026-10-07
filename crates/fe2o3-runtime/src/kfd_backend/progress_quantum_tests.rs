//! Real Context/router/owner scheduling over scripted native storage. These
//! tests prove neither GPU execution nor simultaneous physical DMA.

use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::ScriptedExecutionOutcomeV1;
use crate::{
    RuntimeAllocationIdV1, RuntimeAsyncCurrentThreadOwnedEngineV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncOwnedDispositionV1, RuntimeAsyncProgressConfigV1, RuntimeAsyncProgressHandleV1,
    RuntimeCancellationV1, RuntimeCompletionFailureV1, RuntimeCompletionStatusV1, RuntimeContextV1,
    RuntimeErrorV1, RuntimeMemoryRegionV1, RuntimePeerCopyV1, RuntimePollV1, RuntimeStreamIdV1,
    RuntimeSubmissionV1,
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll, Waker};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Copy = RuntimeSubmissionV1<RuntimePeerCopyV1>;
type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<KfdMultiDeviceRuntimeBackendV1>;
type Handle = RuntimeAsyncProgressHandleV1<KfdMultiDeviceRuntimeBackendV1>;

fn root(context: &Context, id: u64) -> &Root {
    let RoutedSubmissionV1::CooperativeCopy(copy) = &context.backend().submissions[&id] else {
        panic!("the original cooperative peer root must remain indexed")
    };
    assert!(copy.staging.is_empty());
    assert_eq!(copy.scratch_byte_len, 0);
    copy.compute_xgmi.as_deref().unwrap()
}

fn leaves(trace: &[Stage]) -> usize {
    trace
        .iter()
        .filter(|stage| {
            matches!(
                stage,
                Stage::Copy | Stage::Poll | Stage::NextPacket | Stage::Finish
            )
        })
        .count()
}

fn ready<F: Future>(future: Pin<&mut F>) -> F::Output {
    match future.poll(&mut TaskContext::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("the preceding owner tick must have serviced this command"),
    }
}

fn release_steps(bytes: usize) -> Vec<ScriptedSdmaStepV1> {
    let mut steps = Vec::new();
    let cap = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
    for offset in (0..bytes).step_by(cap) {
        let len = (bytes - offset).min(cap);
        steps.extend([
            ScriptedSdmaStepV1::Allocate {
                kind: ScriptedBufferKindV1::Host,
                byte_len: len,
            },
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: len,
            },
            ScriptedSdmaStepV1::Submit {
                direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
                host_offset: 0,
                device_offset: offset as u64,
                copy_bytes: len as u32,
                outcome: ScriptedFailureModeV1::Success,
            },
            ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
    }
    steps.extend([
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    steps
}

struct Layout {
    allocations: [RuntimeAllocationIdV1; 4],
    streams: [RuntimeStreamIdV1; 4],
    routes: [RoutedHandleV1; 4],
    owners: [Option<u64>; 4],
    initial: [Vec<u8>; 4],
    lengths: [usize; 2],
}

impl Layout {
    fn region(
        &self,
        child: usize,
        offset: u64,
        len: usize,
        access: RuntimeAccessV1,
    ) -> RuntimeMemoryRegionV1 {
        RuntimeMemoryRegionV1 {
            allocation: self.allocations[child],
            access,
            byte_offset: offset,
            byte_len: len as u64,
        }
    }

    fn pair(&self, index: usize) -> [RuntimeMemoryRegionV1; 2] {
        [
            self.region(2 * index, 7, self.lengths[index], RuntimeAccessV1::Read),
            self.region(
                2 * index + 1,
                19,
                self.lengths[index],
                RuntimeAccessV1::Write,
            ),
        ]
    }

    fn expected(&self, cancelled: Option<usize>) -> [Vec<u8>; 4] {
        let mut expected = self.initial.clone();
        for pair in 0..2 {
            if cancelled != Some(pair) {
                let len = self.lengths[pair];
                expected[2 * pair + 1][19..19 + len]
                    .copy_from_slice(&self.initial[2 * pair][7..7 + len]);
            }
        }
        expected
    }

    fn assert_bytes(&self, context: &Context, expected: &[Vec<u8>; 4]) {
        for (child, route) in self.routes.iter().enumerate() {
            let KfdRuntimeSdmaStorageV1::Device(owner) =
                &context.backend().children[child].allocations[&route.local].sdma_storage
            else {
                panic!("the original whole-allocation owner must be restored")
            };
            assert_eq!(owner.scripted_owner_id(), self.owners[child]);
            assert_eq!(owner.scripted_bytes().unwrap(), expected[child]);
        }
        assert!(
            context
                .backend()
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(context.backend().cooperative_staging_bytes, 0);
        assert_eq!(context.backend().completed_compute_xgmi_copies, 0);
        context.backend().assert_cooperative_indexes_consistent();
    }
}

struct Four {
    context: ManuallyDrop<Context>,
    layout: Layout,
}

impl Four {
    fn new(first_bytes: usize, pending_samples: usize) -> Self {
        let children = (0..4)
            .map(|index| {
                let mut child = KfdRuntimeBackendV1::mock();
                child.description.backend_device = 7 + index;
                child
            })
            .collect();
        let mut context = ManuallyDrop::new(
            Context::open(KfdMultiDeviceRuntimeBackendV1::from_backends(children).unwrap())
                .unwrap(),
        );
        let lengths = [first_bytes, 37];
        let initial = std::array::from_fn(|child| {
            (0..lengths[child / 2] + 64)
                .map(|byte| ((byte as u64).wrapping_mul(29) ^ ((child as u64 + 1) * 71)) as u8)
                .collect::<Vec<_>>()
        });
        let devices: [_; 4] = std::array::from_fn(|child| context.devices()[child].id());
        let streams = std::array::from_fn(|child| context.create_stream(devices[child]).unwrap());
        let allocations = std::array::from_fn(|child| {
            let allocation = context
                .allocate(
                    devices[child],
                    RuntimeMemoryKindV1::DeviceLocal,
                    initial[child].len() as u64,
                    8,
                )
                .unwrap();
            context
                .write_allocation(allocation, 0, &initial[child])
                .unwrap();
            allocation
        });
        let backend = context.backend_mut_for_test_v1();
        let routes = std::array::from_fn(|child| {
            *backend
                .allocations
                .values()
                .find(|route| route.child == child)
                .unwrap()
        });
        let owners = std::array::from_fn(|index| {
            let child = &mut backend.children[index];
            assert_eq!(child.allocations.len(), 1);
            let driver = ScriptedSdmaDriverV1::new(release_steps(initial[index].len()));
            let mut owner = driver.test_device_owner(initial[index].len());
            owner
                .scripted_bytes_mut()
                .unwrap()
                .copy_from_slice(&initial[index]);
            let identity = owner.scripted_owner_id();
            let record = child.allocations.get_mut(&routes[index].local).unwrap();
            record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(owner));
            record.sdma_backed = true;
            record.sdma_initialized = true;
            record.sdma_shadow_dirty = true;
            child.native_available = true;
            child.sdma_enabled = true;
            child.peer_visible_device_allocations = true;
            child.scripted_sdma = Some(driver);
            identity
        });
        for pair in [(0, 1), (2, 3)] {
            backend.compute_xgmi_routes.insert(
                pair,
                Route::Scripted {
                    failure: None,
                    unwind: false,
                    pending_samples,
                },
            );
        }
        Self {
            context,
            layout: Layout {
                allocations,
                streams,
                routes,
                owners,
                initial,
                lengths,
            },
        }
    }

    fn submit(&mut self) -> (Vec<Copy>, [u64; 2]) {
        let mut copies = Vec::new();
        let ids = std::array::from_fn(|pair| {
            let [source, destination] = self.layout.pair(pair);
            let copy = self
                .context
                .peer_copy(self.layout.streams[2 * pair + 1], source, destination, &[])
                .unwrap();
            let id = self.context.backend_submission_for_test_v1(&copy).unwrap();
            assert!(root(&self.context, id).trace.is_empty());
            copies.push(copy);
            id
        });
        (copies, ids)
    }

    fn into_engine(self) -> (Engine, Handle, Layout) {
        let (engine, handle) = Engine::new_with_progress(
            || Ok::<_, ()>(ManuallyDrop::into_inner(self.context)),
            RuntimeAsyncEngineConfigV1::new(8, 8, 8, 8, Duration::from_micros(1)).unwrap(),
            RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
        )
        .unwrap();
        (engine, handle, self.layout)
    }

    fn clean<A>(mut self, copies: Vec<RuntimeSubmissionV1<A>>, expected: [Vec<u8>; 4]) {
        self.layout.assert_bytes(&self.context, &expected);
        for copy in copies.into_iter().rev() {
            self.context.release_submission(copy).unwrap();
        }
        cleanup(&mut self.context);
        self.context
            .backend_mut_for_test_v1()
            .shutdown_native_v1()
            .unwrap();
        drop(ManuallyDrop::into_inner(self.context));
    }
}

fn cleanup(context: &mut Context) {
    let report = context.cleanup();
    assert!(report.is_complete(), "{report:?}");
    assert!(report.failures().is_empty());
    let backend = context.backend();
    assert!(backend.submissions.is_empty());
    assert!(backend.allocations.is_empty());
    assert!(backend.streams.is_empty());
    assert_eq!(backend.cooperative_progress_quantum, None);
    for child in &backend.children {
        let driver = child.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted(), "{driver:?}");
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
    }
}

fn finish(f: &mut Four, copies: &mut [Copy], cancelled: Option<usize>) {
    for _ in 0..32 {
        for pair in 0..2 {
            if cancelled != Some(pair) {
                f.context
                    .progress_stream_v1(f.layout.streams[2 * pair + 1])
                    .unwrap();
            }
        }
        let states = copies
            .iter_mut()
            .map(|copy| f.context.poll(copy).unwrap())
            .collect::<Vec<_>>();
        if states
            .iter()
            .all(|status| *status != RuntimePollV1::Pending)
        {
            for (pair, copy) in copies.iter().enumerate() {
                assert_eq!(
                    f.context.query_submission(copy).unwrap(),
                    if cancelled == Some(pair) {
                        RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled)
                    } else {
                        RuntimeCompletionStatusV1::Succeeded
                    }
                );
            }
            return;
        }
    }
    panic!("bounded peer quanta did not settle the original submissions");
}

#[test]
fn one_quantum_samples_once_and_keeps_ready_retirement_separate() {
    let mut f = Four::new(37, 3);
    let (mut copies, ids) = f.submit();
    for _ in 0..8 {
        f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
        if root(&f.context, ids[0]).phase == Phase::Published {
            break;
        }
    }
    assert_eq!(root(&f.context, ids[0]).trace, [Stage::Create, Stage::Copy]);
    let original = root(&f.context, ids[0])
        .scripted_owners
        .each_ref()
        .map(|owner| owner.as_ref().unwrap().scripted_owner_id());
    for remaining in (0..3).rev() {
        let before = leaves(&root(&f.context, ids[0]).trace);
        f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
        let root = root(&f.context, ids[0]);
        assert_eq!(leaves(&root.trace), before + 1);
        assert_eq!(root.pending_samples, remaining);
        assert_eq!(root.phase, Phase::Published);
        assert!(!root.is_quiescent());
        assert_eq!(f.context.backend().cooperative_progress_quantum, None);
        assert_eq!(
            f.context.poll(&mut copies[0]).unwrap(),
            RuntimePollV1::Pending
        );
    }
    f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
    assert_eq!(root(&f.context, ids[0]).phase, Phase::Ready);
    assert_eq!(
        root(&f.context, ids[0])
            .scripted_owners
            .each_ref()
            .map(|owner| owner.as_ref().unwrap().scripted_owner_id()),
        original
    );
    assert_eq!(
        f.context.poll(&mut copies[0]).unwrap(),
        RuntimePollV1::Pending
    );
    f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
    assert!(root(&f.context, ids[0]).is_quiescent());
    finish(&mut f, &mut copies, None);
    let expected = f.layout.expected(None);
    f.clean(copies, expected);
}

#[test]
fn owned_engine_rotates_disjoint_pairs_before_always_ready_packet_tail_finishes() {
    let mut f = Four::new(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize + 13, 0);
    let (copies, ids) = f.submit();
    assert_eq!(root(&f.context, ids[0]).packet_plan.count(), 2);
    let (mut engine, handle, layout) = f.into_engine();
    let mut first = Box::pin(
        handle
            .enqueue_stream_registration(layout.streams[1])
            .unwrap(),
    );
    let mut second = Box::pin(
        handle
            .enqueue_stream_registration(layout.streams[3])
            .unwrap(),
    );
    engine.tick().unwrap();
    let guards = [
        ready(first.as_mut()).unwrap().unwrap(),
        ready(second.as_mut()).unwrap().unwrap(),
    ];
    drop(first);
    drop(second);
    let mut saw_second_before_first_retired = false;
    let mut previous = [0; 2];
    for _ in 0..14 {
        let mut observation = Box::pin(
            handle
                .observer()
                .enqueue_with_context(move |context| {
                    assert_eq!(context.backend().cooperative_progress_quantum, None);
                    ids.map(|id| (root(context, id).trace.clone(), root(context, id).phase))
                })
                .unwrap(),
        );
        engine.tick().unwrap();
        let observed = ready(observation.as_mut()).unwrap();
        drop(observation);
        let current = observed.each_ref().map(|(trace, _)| leaves(trace));
        assert!(current[0] >= previous[0] && current[1] >= previous[1]);
        assert!(
            current[0] - previous[0] + current[1] - previous[1] <= 1,
            "one owner tick crossed multiple cooperative leaves: {observed:?}"
        );
        saw_second_before_first_retired |= current[1] != 0 && observed[0].1 != Phase::Retired;
        previous = current;
    }
    assert!(
        saw_second_before_first_retired,
        "the long ready peer monopolized progress"
    );
    assert_eq!(previous, [5, 3]);
    drop(guards);
    let mut release = Box::pin(
        handle
            .observer()
            .enqueue_with_context(move |context| {
                layout.assert_bytes(context, &layout.expected(None));
                for mut copy in copies {
                    assert_eq!(context.poll(&mut copy).unwrap(), RuntimePollV1::Succeeded);
                    context.release_submission(copy).unwrap();
                }
                cleanup(context);
            })
            .unwrap(),
    );
    engine
        .drive_until_ready(release.as_mut(), Instant::now() + Duration::from_secs(2))
        .unwrap()
        .unwrap();
    drop(release);
    let report = engine.shutdown();
    assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
    assert!(!report.worker_panicked);
    assert!(report.native_failure.is_none());
}

#[test]
fn queued_cancellation_command_runs_between_native_publication_and_sampling() {
    let mut f = Four::new(37, 0);
    let (copies, ids) = f.submit();
    let (mut engine, handle, layout) = f.into_engine();
    let mut registration = Box::pin(
        handle
            .enqueue_stream_registration(layout.streams[1])
            .unwrap(),
    );
    engine.tick().unwrap();
    let guard = ready(registration.as_mut()).unwrap().unwrap();
    drop(registration);
    engine.tick().unwrap();
    let mut cancel = Box::pin(
        handle
            .observer()
            .enqueue_with_context(move |context| {
                assert_eq!(root(context, ids[0]).trace, [Stage::Create, Stage::Copy]);
                assert!(root(context, ids[1]).trace.is_empty());
                let mut copies = copies;
                assert_eq!(
                    context.cancel(&mut copies[1]).unwrap(),
                    RuntimeCancellationV1::Cancelled
                );
                assert_eq!(root(context, ids[0]).trace, [Stage::Create, Stage::Copy]);
                assert_eq!(
                    context.cancel(&mut copies[0]).unwrap(),
                    RuntimeCancellationV1::TooLate
                );
                copies
            })
            .unwrap(),
    );
    engine.tick().unwrap();
    let copies = ready(cancel.as_mut()).unwrap();
    drop(cancel);
    for _ in 0..4 {
        engine.tick().unwrap();
    }
    drop(guard);
    let mut release = Box::pin(
        handle
            .observer()
            .enqueue_with_context(move |context| {
                layout.assert_bytes(context, &layout.expected(Some(1)));
                for (index, mut copy) in copies.into_iter().enumerate() {
                    let observed = context.poll(&mut copy).unwrap();
                    if index == 0 {
                        assert_eq!(observed, RuntimePollV1::Succeeded);
                    } else {
                        assert!(matches!(observed, RuntimePollV1::Failed { .. }));
                        assert_eq!(
                            context.query_submission(&copy).unwrap(),
                            RuntimeCompletionStatusV1::Failed(
                                RuntimeCompletionFailureV1::Cancelled
                            )
                        );
                    }
                    context.release_submission(copy).unwrap();
                }
                cleanup(context);
            })
            .unwrap(),
    );
    engine
        .drive_until_ready(release.as_mut(), Instant::now() + Duration::from_secs(2))
        .unwrap()
        .unwrap();
    drop(release);
    let report = engine.shutdown();
    assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
    assert!(!report.worker_panicked);
    assert!(report.native_failure.is_none());
}

#[test]
fn rejected_quantum_clears_budget_and_explicit_flush_keeps_its_full_progress_contract() {
    let mut f = Four::new(37, 0);
    let (mut copies, ids) = f.submit();
    assert!(matches!(
        f.context
            .backend_mut_for_test_v1()
            .progress_stream_v1(u64::MAX),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(f.context.backend().cooperative_progress_quantum, None);
    assert!(ids.iter().all(|id| root(&f.context, *id).trace.is_empty()));
    for _ in 0..2 {
        f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
    }
    assert_eq!(root(&f.context, ids[0]).trace, [Stage::Create, Stage::Copy]);
    f.context.flush_stream(f.layout.streams[1]).unwrap();
    assert_eq!(root(&f.context, ids[0]).phase, Phase::Retired);
    assert_eq!(
        root(&f.context, ids[0]).trace,
        [
            Stage::Create,
            Stage::Copy,
            Stage::Poll,
            Stage::Finish,
            Stage::Retire,
            Stage::Restore
        ]
    );
    assert_eq!(f.context.backend().cooperative_progress_quantum, None);
    finish(&mut f, &mut copies, None);
    let expected = f.layout.expected(None);
    f.clean(copies, expected);
}

#[test]
fn late_directed_fanout_spends_one_quantum_on_its_started_resource_predecessor() {
    for late in [false, true] {
        let mut f = Four::new(37, 0);
        f.context
            .backend_mut_for_test_v1()
            .compute_xgmi_routes
            .insert(
                (0, 2),
                Route::Scripted {
                    failure: None,
                    unwind: false,
                    pending_samples: 0,
                },
            );
        let read = f.layout.region(0, 7, 37, RuntimeAccessV1::Read);
        let first_write = f.layout.region(1, 19, 37, RuntimeAccessV1::Write);
        let second_write = f.layout.region(2, 19, 37, RuntimeAccessV1::Write);
        let mut first = f
            .context
            .directed_peer_copy_v1(f.layout.streams[1], read, first_write, &[])
            .unwrap();
        let first_id = f.context.backend_submission_for_test_v1(&first).unwrap();
        let mut second = if late {
            None
        } else {
            Some(
                f.context
                    .directed_peer_copy_v1(f.layout.streams[2], read, second_write, &[])
                    .unwrap(),
            )
        };
        for _ in 0..8 {
            f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
            if root(&f.context, first_id).phase == Phase::Published {
                break;
            }
        }
        assert_eq!(
            root(&f.context, first_id).trace,
            [Stage::Create, Stage::Copy]
        );
        if late {
            second = Some(
                f.context
                    .directed_peer_copy_v1(f.layout.streams[2], read, second_write, &[])
                    .unwrap(),
            );
        }
        let mut second = second.unwrap();
        let second_id = f.context.backend_submission_for_test_v1(&second).unwrap();
        assert!(root(&f.context, second_id).trace.is_empty());
        let mut observed_restoration_boundary = false;
        for _ in 0..24 {
            let before = leaves(&root(&f.context, first_id).trace)
                + leaves(&root(&f.context, second_id).trace);
            f.context.progress_stream_v1(f.layout.streams[2]).unwrap();
            let after = leaves(&root(&f.context, first_id).trace)
                + leaves(&root(&f.context, second_id).trace);
            assert!(
                after - before <= 1,
                "resource-prefix progress bypassed the shared leaf budget"
            );
            assert_eq!(f.context.backend().cooperative_progress_quantum, None);
            if root(&f.context, first_id).phase == Phase::Retired
                && root(&f.context, second_id).trace.is_empty()
            {
                observed_restoration_boundary = true;
                assert!(
                    f.context
                        .backend()
                        .compute_xgmi_children
                        .iter()
                        .all(Option::is_none)
                );
            }
            if f.context.poll(&mut second).unwrap() == RuntimePollV1::Succeeded {
                break;
            }
        }
        assert!(observed_restoration_boundary);
        assert_eq!(
            f.context.poll(&mut first).unwrap(),
            RuntimePollV1::Succeeded
        );
        assert_eq!(
            f.context.poll(&mut second).unwrap(),
            RuntimePollV1::Succeeded
        );
        let mut expected = f.layout.initial.clone();
        for destination in [1, 2] {
            expected[destination][19..56].copy_from_slice(&f.layout.initial[0][7..44]);
        }
        f.clean(vec![first, second], expected);
    }
}

#[test]
fn native_quantum_error_and_unwind_reset_budget_without_releasing_paired_custody() {
    for stage in [Stage::Poll, Stage::Retire, Stage::Restore] {
        for unwind in [false, true] {
            let mut f = Four::new(37, 0);
            f.context
                .backend_mut_for_test_v1()
                .compute_xgmi_routes
                .insert(
                    (0, 1),
                    Route::Scripted {
                        failure: Some(stage),
                        unwind,
                        pending_samples: 0,
                    },
                );
            let (_copies, ids) = f.submit();
            let mut failed = false;
            for _ in 0..8 {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    f.context.progress_stream_v1(f.layout.streams[1])
                }));
                assert_eq!(f.context.backend().cooperative_progress_quantum, None);
                match result {
                    Err(_) => {
                        assert!(unwind);
                        failed = true;
                        break;
                    }
                    Ok(Err(RuntimeErrorV1::BackendTerminal(_))) => {
                        assert!(!unwind);
                        failed = true;
                        break;
                    }
                    Ok(Ok(())) => {}
                    other => panic!("unexpected failure policy: {other:?}"),
                }
            }
            assert!(failed);
            assert!(f.context.backend().terminal);
            let held = root(&f.context, ids[0]);
            assert!(!held.is_quiescent());
            assert_eq!(
                held.scripted_owners
                    .each_ref()
                    .map(|owner| owner.as_ref().unwrap().scripted_owner_id()),
                [f.layout.owners[0], f.layout.owners[1]]
            );
            for child in 0..2 {
                let backend = f.context.backend();
                assert!(backend.children[child].terminal);
                assert_eq!(backend.compute_xgmi_children[child], Some(ids[0]));
                assert!(
                    matches!(backend.children[child].allocations[&f.layout.routes[child].local].sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(id)) if id == ids[0])
                );
            }
            assert!(root(&f.context, ids[1]).trace.is_empty());
            assert!(f.context.progress_stream_v1(f.layout.streams[3]).is_err());
            let report = f.context.cleanup();
            assert!(!report.is_complete());
            assert!(report.is_terminal());
            assert_eq!(report.retained().submissions, 2);
            assert_eq!(report.retained().allocations, 4);
            assert!(
                f.context
                    .backend_mut_for_test_v1()
                    .shutdown_native_v1()
                    .is_err()
            );
            for child in &f.context.backend().children {
                let driver = child.scripted_sdma.as_ref().unwrap();
                assert_eq!(driver.live_owner_count(), 1);
                assert_eq!(driver.unexpected_drops(), 0);
            }
            // Terminal native custody stays retained, never flag-cleared for Drop.
        }
    }
}
