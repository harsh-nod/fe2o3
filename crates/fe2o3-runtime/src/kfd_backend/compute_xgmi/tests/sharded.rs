//! Real router/Context/owner scheduling with scripted storage, not GPU-byte evidence.

use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::ScriptedExecutionOutcomeV1;
use crate::{
    RuntimeAllocationIdV1, RuntimeAsyncCurrentThreadOwnedEngineV1, RuntimeAsyncDriveErrorV1,
    RuntimeAsyncEngineConfigV1, RuntimeAsyncOwnedDispositionV1, RuntimeAsyncProgressConfigV1,
    RuntimeAsyncProgressHandleV1, RuntimeCancellationV1, RuntimeCompletionFailureV1,
    RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeErrorV1, RuntimeMemoryRegionV1,
    RuntimePeerCopyV1, RuntimePollV1, RuntimeStreamIdV1, RuntimeSubmissionV1,
    RuntimeValidationErrorV1,
};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll, Waker};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Copy = RuntimeSubmissionV1<RuntimePeerCopyV1>;
type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<KfdMultiDeviceRuntimeBackendV1>;
type Handle = RuntimeAsyncProgressHandleV1<KfdMultiDeviceRuntimeBackendV1>;
type Completions = Arc<Mutex<Vec<(usize, RuntimeCompletionStatusV1)>>>;

const TOTAL_BYTES: usize = 257;
const COUNTS: [usize; 4] = [2, 3, 5, 8];

#[derive(Clone)]
struct Ring {
    sizes: Vec<usize>,
    offsets: Vec<usize>,
    sources: Vec<RuntimeAllocationIdV1>,
    incoming: Vec<RuntimeAllocationIdV1>,
    streams: Vec<RuntimeStreamIdV1>,
    routes: Vec<[RoutedHandleV1; 2]>,
    owners: Vec<[Option<u64>; 2]>,
}

impl Ring {
    fn next(&self, source: usize) -> usize {
        (source + 1) % self.sizes.len()
    }

    fn previous(&self, destination: usize) -> usize {
        (destination + self.sizes.len() - 1) % self.sizes.len()
    }

    fn regions(&self, source: usize) -> [RuntimeMemoryRegionV1; 2] {
        [
            RuntimeMemoryRegionV1 {
                allocation: self.sources[source],
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: self.sizes[source] as u64,
            },
            RuntimeMemoryRegionV1 {
                allocation: self.incoming[self.next(source)],
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: self.sizes[source] as u64,
            },
        ]
    }

    fn bytes(&self, source: usize) -> Vec<u8> {
        (self.offsets[source]..self.offsets[source] + self.sizes[source])
            .map(|offset| {
                let mut value = (offset as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
                value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                (value ^ (value >> 31)) as u8
            })
            .collect()
    }

    fn sentinel(&self, destination: usize) -> Vec<u8> {
        self.bytes(self.previous(destination))
            .into_iter()
            .map(|byte| !byte)
            .collect()
    }

    fn assert_storage(&self, context: &Context, cancelled: Option<usize>) {
        let backend = context.backend();
        let mut received = 0;
        for (child, routes) in self.routes.iter().enumerate() {
            let source = device_owner(context, routes[0]);
            let incoming = device_owner(context, routes[1]);
            assert_eq!(
                [source.scripted_owner_id(), incoming.scripted_owner_id()],
                self.owners[child]
            );
            assert_eq!(source.scripted_bytes().unwrap(), self.bytes(child));
            let predecessor = self.previous(child);
            let expected = if cancelled == Some(predecessor) {
                self.sentinel(child)
            } else {
                received += self.sizes[predecessor];
                self.bytes(predecessor)
            };
            assert_eq!(incoming.scripted_bytes().unwrap(), expected);
        }
        assert_eq!(
            received,
            TOTAL_BYTES - cancelled.map_or(0, |source| self.sizes[source])
        );
        assert!(backend.compute_xgmi_children.iter().all(Option::is_none));
        assert_eq!(backend.cooperative_staging_bytes, 0);
        // Scripted bytes and logical results are not native completion evidence.
        assert_eq!(backend.completed_compute_xgmi_copies, 0);
        backend.assert_cooperative_indexes_consistent();
    }
}

struct RingFixture {
    context: ManuallyDrop<Context>,
    ring: Ring,
    completions: Completions,
}

fn release_steps(bytes: usize) -> [ScriptedSdmaStepV1; 8] {
    [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len: bytes,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: bytes,
        },
        ScriptedSdmaStepV1::Submit {
            direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: bytes as u32,
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]
}

fn copy(context: &Context, id: u64) -> &CooperativeCopySubmissionV1 {
    let RoutedSubmissionV1::CooperativeCopy(copy) = &context.backend().submissions[&id] else {
        panic!("ring edge must remain a cooperative copy")
    };
    copy
}

fn root(context: &Context, id: u64) -> &Root {
    copy(context, id).compute_xgmi.as_deref().unwrap()
}

fn device_owner(context: &Context, route: RoutedHandleV1) -> &DirectionalSdmaDeviceOwnerV1 {
    let KfdRuntimeSdmaStorageV1::Device(owner) =
        &context.backend().children[route.child].allocations[&route.local].sdma_storage
    else {
        panic!("original device owner must be restored")
    };
    owner
}

fn assert_recycled(context: &Context) {
    let backend = context.backend();
    assert!(backend.submissions.is_empty());
    assert!(backend.allocations.is_empty());
    assert!(backend.streams.is_empty());
    for child in &backend.children {
        let driver = child.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted(), "{driver:?}");
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
    }
}

impl RingFixture {
    fn new(count: usize, pending_samples: usize) -> Self {
        let children = (0..count)
            .map(|index| {
                let mut child = KfdRuntimeBackendV1::mock();
                child.description.backend_device = 7 + index as u64;
                child
            })
            .collect();
        let mut context = ManuallyDrop::new(
            RuntimeContextV1::open(
                KfdMultiDeviceRuntimeBackendV1::from_backends(children).unwrap(),
            )
            .unwrap(),
        );
        let quotient = TOTAL_BYTES / count;
        let remainder = TOTAL_BYTES % count;
        let mut ring = Ring {
            sizes: (0..count)
                .map(|index| quotient + usize::from(index < remainder))
                .collect(),
            offsets: (0..count)
                .map(|index| index * quotient + index.min(remainder))
                .collect(),
            sources: Vec::new(),
            incoming: Vec::new(),
            streams: Vec::new(),
            routes: Vec::new(),
            owners: Vec::new(),
        };
        assert_eq!(ring.sizes.iter().sum::<usize>(), TOTAL_BYTES);
        assert_ne!(ring.sizes.iter().min(), ring.sizes.iter().max());
        for child in 0..count {
            let device = context.devices()[child].id();
            ring.streams.push(context.create_stream(device).unwrap());
            let source = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    ring.sizes[child] as u64,
                    8,
                )
                .unwrap();
            context
                .write_allocation(source, 0, &ring.bytes(child))
                .unwrap();
            ring.sources.push(source);
            let incoming = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    ring.sizes[ring.previous(child)] as u64,
                    8,
                )
                .unwrap();
            context
                .write_allocation(incoming, 0, &ring.sentinel(child))
                .unwrap();
            ring.incoming.push(incoming);
        }
        let backend = context.backend_mut_for_test_v1();
        for (index, child) in backend.children.iter_mut().enumerate() {
            let mut ids = child.allocations.keys().copied().collect::<Vec<_>>();
            ids.sort_unstable();
            assert_eq!(ids.len(), 2);
            let routes = ids
                .iter()
                .map(|local| RoutedHandleV1 {
                    child: index,
                    local: *local,
                })
                .collect::<Vec<_>>();
            ring.routes.push([routes[0], routes[1]]);
            // Context cleanup releases allocations in original handle order.
            let driver = ScriptedSdmaDriverV1::new(
                ids.iter()
                    .flat_map(|id| release_steps(child.allocations[id].bytes.len())),
            );
            let mut identities = Vec::new();
            for id in ids {
                let record = child.allocations.get_mut(&id).unwrap();
                let mut owner = driver.test_device_owner(record.bytes.len());
                owner
                    .scripted_bytes_mut()
                    .unwrap()
                    .copy_from_slice(&record.bytes);
                identities.push(owner.scripted_owner_id());
                record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(owner));
                record.sdma_backed = true;
                record.sdma_initialized = true;
                record.sdma_shadow_dirty = true;
            }
            ring.owners.push([identities[0], identities[1]]);
            child.native_available = true;
            child.sdma_enabled = true;
            child.peer_visible_device_allocations = true;
            child.scripted_sdma = Some(driver);
        }
        for source in 0..count {
            backend.compute_xgmi_routes.insert(
                (source, ring.next(source)),
                Route::Scripted {
                    failure: None,
                    unwind: false,
                    pending_samples,
                },
            );
        }
        Self {
            context,
            ring,
            completions: Completions::default(),
        }
    }

    fn submit_all(&mut self) -> (Vec<Copy>, Vec<u64>) {
        let mut copies = Vec::new();
        let mut ids = Vec::new();
        for source in 0..self.ring.sizes.len() {
            let [read, write] = self.ring.regions(source);
            let copy = self
                .context
                .peer_copy(self.ring.streams[self.ring.next(source)], read, write, &[])
                .unwrap();
            let id = self.context.backend_submission_for_test_v1(&copy).unwrap();
            let completions = Arc::clone(&self.completions);
            self.context
                .on_completion(&copy, move |status| {
                    completions.lock().unwrap().push((source, status));
                })
                .unwrap();
            assert!(root(&self.context, id).trace.is_empty());
            assert!(copy_submission_has_no_staging(&self.context, id));
            copies.push(copy);
            ids.push(id);
        }
        assert_eq!(self.context.backend().submissions.len(), copies.len());
        assert!(
            self.context
                .backend()
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        (copies, ids)
    }

    fn flush(&mut self, source: usize) {
        self.context
            .flush_stream(self.ring.streams[self.ring.next(source)])
            .unwrap();
    }

    fn finish_all(&mut self, copies: &mut [Copy], cancelled: Option<usize>) {
        for _ in 0..128 {
            for source in 0..copies.len() {
                self.flush(source);
            }
            let mut pending = false;
            for copy in copies.iter_mut() {
                pending |= self.context.poll(copy).unwrap() == RuntimePollV1::Pending;
            }
            if !pending {
                for (source, copy) in copies.iter().enumerate() {
                    let expected = if cancelled == Some(source) {
                        RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled)
                    } else {
                        RuntimeCompletionStatusV1::Succeeded
                    };
                    assert_eq!(self.context.query_submission(copy).unwrap(), expected);
                }
                let mut completions = self.completions.lock().unwrap().clone();
                completions.sort_unstable_by_key(|(source, _)| *source);
                assert_eq!(completions.len(), copies.len());
                for (source, (tag, status)) in completions.into_iter().enumerate() {
                    assert_eq!(tag, source);
                    assert_eq!(
                        status,
                        self.context.query_submission(&copies[source]).unwrap()
                    );
                }
                return;
            }
        }
        panic!("round-robin ring progress exhausted its deterministic step bound");
    }

    fn clean(mut self, copies: Vec<Copy>) {
        for copy in copies {
            self.context.release_submission(copy).unwrap();
        }
        let cleanup = self.context.cleanup();
        assert!(cleanup.is_complete(), "{cleanup:?}");
        assert!(cleanup.failures().is_empty());
        assert_recycled(&self.context);
        self.context
            .backend_mut_for_test_v1()
            .shutdown_native_v1()
            .unwrap();
        drop(ManuallyDrop::into_inner(self.context));
    }

    fn into_engine(self) -> (Engine, Handle, Ring) {
        let count = self.ring.sizes.len();
        let (engine, handle) = Engine::new_with_progress(
            || Ok::<_, ()>(ManuallyDrop::into_inner(self.context)),
            RuntimeAsyncEngineConfigV1::new(
                2 * count,
                2 * count,
                2 * count,
                2 * count,
                Duration::from_micros(1),
            )
            .unwrap(),
            RuntimeAsyncProgressConfigV1::new(count, 1).unwrap(),
        )
        .unwrap();
        (engine, handle, self.ring)
    }
}

fn copy_submission_has_no_staging(context: &Context, id: u64) -> bool {
    let copy = copy(context, id);
    copy.compute_xgmi.is_some() && copy.staging.is_empty() && copy.scratch_byte_len == 0
}

#[test]
fn unequal_sharded_rings_admit_all_edges_and_resume_shared_child_progress() {
    for count in COUNTS {
        let mut f = RingFixture::new(count, 3);
        let (mut copies, ids) = f.submit_all();
        assert!(matches!(
            f.context.drain(&mut copies[0], Instant::now()),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidDeadline
            ))
        ));
        assert!(ids.iter().all(|id| root(&f.context, *id).trace.is_empty()));
        f.flush(0);
        assert_eq!(root(&f.context, ids[0]).phase, Phase::Published);
        assert_eq!(f.context.backend().compute_xgmi_children[0], Some(ids[0]));
        assert_eq!(f.context.backend().compute_xgmi_children[1], Some(ids[0]));
        for adjacent in [1, count - 1] {
            f.flush(adjacent);
            assert!(root(&f.context, ids[adjacent]).trace.is_empty());
            assert!(copy_submission_has_no_staging(&f.context, ids[adjacent]));
        }
        let trace = root(&f.context, ids[0]).trace.clone();
        for copy in &mut copies {
            assert_eq!(f.context.poll(copy).unwrap(), RuntimePollV1::Pending);
        }
        assert_eq!(root(&f.context, ids[0]).trace, trace);
        assert!(f.completions.lock().unwrap().is_empty());
        f.finish_all(&mut copies, None);
        for id in ids {
            let root = root(&f.context, id);
            assert!(root.is_quiescent());
            for stage in [
                Stage::Create,
                Stage::Copy,
                Stage::Finish,
                Stage::Retire,
                Stage::Restore,
            ] {
                assert_eq!(
                    root.trace.iter().filter(|actual| **actual == stage).count(),
                    1
                );
            }
            assert_eq!(
                root.trace
                    .iter()
                    .filter(|stage| **stage == Stage::Poll)
                    .count(),
                4
            );
        }
        f.ring.assert_storage(&f.context, None);
        f.clean(copies);
    }
}

#[test]
fn sharded_ring_cancellation_refunds_only_the_unstarted_edge() {
    for count in COUNTS {
        let mut f = RingFixture::new(count, 3);
        let (mut copies, ids) = f.submit_all();
        f.flush(0);
        let trace = root(&f.context, ids[0]).trace.clone();
        assert_eq!(
            f.context.cancel(&mut copies[0]).unwrap(),
            RuntimeCancellationV1::TooLate
        );
        assert_eq!(
            f.context.cancel(&mut copies[1]).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        assert!(root(&f.context, ids[1]).trace.is_empty());
        assert_eq!(root(&f.context, ids[0]).trace, trace);
        assert_eq!(f.context.backend().compute_xgmi_children[0], Some(ids[0]));
        assert_eq!(f.context.backend().compute_xgmi_children[1], Some(ids[0]));
        f.finish_all(&mut copies, Some(1));
        f.ring.assert_storage(&f.context, Some(1));
        f.clean(copies);
    }
}

#[test]
fn sharded_ring_retirement_failure_retains_exact_pair_and_unstarted_neighbors() {
    for unwind in [false, true] {
        let mut f = RingFixture::new(5, 1);
        f.context
            .backend_mut_for_test_v1()
            .compute_xgmi_routes
            .insert(
                (0, 1),
                Route::Scripted {
                    failure: Some(Stage::Retire),
                    unwind,
                    pending_samples: 1,
                },
            );
        let (mut copies, ids) = f.submit_all();
        assert_eq!(
            f.context
                .drain(&mut copies[2], Instant::now() + Duration::from_secs(2))
                .unwrap(),
            RuntimePollV1::Succeeded
        );
        f.flush(0);
        for source in [1, 4] {
            f.flush(source);
            assert!(root(&f.context, ids[source]).trace.is_empty());
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            f.context.flush_stream(f.ring.streams[1])
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            assert!(matches!(
                outcome.unwrap(),
                Err(RuntimeErrorV1::BackendTerminal(_))
            ));
        }
        assert!(f.context.backend().terminal);
        let failed = root(&f.context, ids[0]);
        assert!(!failed.is_quiescent());
        assert!(failed.shells.iter().all(Option::is_some));
        assert_eq!(
            failed
                .scripted_owners
                .each_ref()
                .map(|owner| owner.as_ref().unwrap().scripted_owner_id()),
            [f.ring.owners[0][0], f.ring.owners[1][1]]
        );
        for route in [f.ring.routes[0][0], f.ring.routes[1][1]] {
            let child = &f.context.backend().children[route.child];
            assert!(child.terminal);
            assert_eq!(
                f.context.backend().compute_xgmi_children[route.child],
                Some(ids[0])
            );
            assert!(matches!(
                child.allocations[&route.local].sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(id)) if id == ids[0]
            ));
        }
        for source in [1, 3, 4] {
            assert!(root(&f.context, ids[source]).trace.is_empty());
        }
        assert_eq!(
            *f.completions.lock().unwrap(),
            [(2, RuntimeCompletionStatusV1::Succeeded)]
        );
        let report = f.context.cleanup();
        assert!(!report.is_complete());
        assert!(report.is_terminal());
        assert_eq!(report.retained().submissions, 5);
        assert_eq!(report.retained().allocations, 10);
        assert!(matches!(
            f.context.backend_mut_for_test_v1().shutdown_native_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        for child in &f.context.backend().children {
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.live_owner_count(), 2);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        // No false cleanup after ambiguous retirement: retain the entire Context.
    }
}

#[test]
fn current_thread_sharded_ring_queues_all_futures_before_expired_and_resumed_drive() {
    for count in COUNTS {
        let (mut engine, handle, ring) = RingFixture::new(count, 3).into_engine();
        let mut futures = (0..count)
            .map(|source| {
                let [read, write] = ring.regions(source);
                Box::pin(
                    handle
                        .peer_copy(ring.streams[ring.next(source)], read, write, vec![])
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        for future in &mut futures {
            assert!(matches!(
                future
                    .as_mut()
                    .poll(&mut TaskContext::from_waker(Waker::noop())),
                Poll::Pending
            ));
        }
        assert!(matches!(
            engine.drive_until_ready(futures[0].as_mut(), Instant::now()),
            Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
        ));
        engine.tick().unwrap();
        let mut admitted = Box::pin(
            handle
                .observer()
                .enqueue_with_context(move |context| {
                    let backend = context.backend();
                    assert_eq!(backend.submissions.len(), count);
                    let mut published = 0;
                    for id in backend.submissions.keys() {
                        assert!(copy_submission_has_no_staging(context, *id));
                        let root = root(context, *id);
                        if root.phase == Phase::Published {
                            published += 1;
                        } else {
                            assert!(root.trace.is_empty());
                        }
                    }
                    assert_eq!(published, 1);
                    assert_eq!(
                        backend
                            .compute_xgmi_children
                            .iter()
                            .filter(|id| id.is_some())
                            .count(),
                        2
                    );
                    assert_eq!(backend.completed_compute_xgmi_copies, 0);
                })
                .unwrap(),
        );
        engine
            .drive_until_ready(admitted.as_mut(), Instant::now() + Duration::from_secs(2))
            .unwrap()
            .unwrap();
        let mut submissions = Vec::new();
        for future in &mut futures {
            let result = engine
                .drive_until_ready(future.as_mut(), Instant::now() + Duration::from_secs(2))
                .unwrap()
                .unwrap();
            assert!(matches!(
                result.observation,
                Ok(RuntimeCompletionStatusV1::Succeeded)
            ));
            assert_eq!(result.rejected_observations, 0);
            assert!(result.last_rejected_observation.is_none());
            submissions.push(result.submission.unwrap());
        }
        let mut cleanup = Box::pin(
            handle
                .observer()
                .enqueue_with_context(move |context| {
                    ring.assert_storage(context, None);
                    assert_eq!(context.backend().submissions.len(), count);
                    for submission in submissions {
                        assert_eq!(
                            context.query_submission(&submission).unwrap(),
                            RuntimeCompletionStatusV1::Succeeded
                        );
                        context.release_submission(submission).unwrap();
                    }
                    let report = context.cleanup();
                    assert!(report.is_complete(), "{report:?}");
                    assert!(report.failures().is_empty());
                    assert_recycled(context);
                })
                .unwrap(),
        );
        engine
            .drive_until_ready(cleanup.as_mut(), Instant::now() + Duration::from_secs(2))
            .unwrap()
            .unwrap();
        let report = engine.shutdown();
        assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
        assert!(!report.worker_panicked);
        assert!(report.native_failure.is_none());
        let cleanup = report.cleanup.unwrap();
        assert!(cleanup.is_complete());
        assert!(cleanup.failures().is_empty());
    }
}
