//! Unified Context and owner scheduling over scripted native segment effects.
//! The byte oracle does not establish physical DMA or GPU compute semantics.

mod retained_frames;

use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::ScriptedExecutionOutcomeV1;
use crate::{
    BackendCancellationV1, BackendLaunchProducerV1, BackendProducerAwareLaunchV1,
    BackendSemanticLaunchV1, RuntimeAllocationIdV1, RuntimeAsyncCurrentThreadOwnedEngineV1,
    RuntimeAsyncDriveErrorV1, RuntimeAsyncEngineConfigV1, RuntimeAsyncOwnedDispositionV1,
    RuntimeAsyncProgressConfigV1, RuntimeCancellationV1, RuntimeCompletionStatusV1,
    RuntimeContextV1, RuntimeLaunchGeometryV1, RuntimeMemoryRegionV1, RuntimePeerCopySegmentV1,
    RuntimePeerCopySegmentsBackendV1, RuntimePeerCopySegmentsV1, RuntimePollV1,
    RuntimeProducerAwareLaunchBackendV1, RuntimeStreamIdV1, RuntimeSubmissionV1,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context as TaskContext, Poll, Waker};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Submission = RuntimeSubmissionV1<RuntimePeerCopySegmentsV1>;
type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<KfdMultiDeviceRuntimeBackendV1>;

fn segment(source_offset: u64, destination_offset: u64, byte_len: u64) -> RuntimePeerCopySegmentV1 {
    RuntimePeerCopySegmentV1 {
        source_offset,
        destination_offset,
        byte_len,
    }
}

fn descriptors() -> Vec<RuntimePeerCopySegmentV1> {
    vec![
        segment(0, 0, 9),
        segment(12, 4, 13),
        segment(3, 27, 1),
        segment(12, 4, 13),
    ]
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

#[derive(Clone)]
struct Layout {
    allocations: Vec<RuntimeAllocationIdV1>,
    streams: Vec<RuntimeStreamIdV1>,
    global_allocations: Vec<u64>,
    global_streams: Vec<u64>,
    routes: Vec<RoutedHandleV1>,
    identities: Vec<Option<u64>>,
    initial: Vec<Vec<u8>>,
}

impl Layout {
    fn regions(&self, pair: usize) -> [RuntimeMemoryRegionV1; 2] {
        [
            RuntimeMemoryRegionV1 {
                allocation: self.allocations[2 * pair],
                access: RuntimeAccessV1::Read,
                byte_offset: 3,
                byte_len: self.initial[2 * pair].len() as u64 - 11,
            },
            RuntimeMemoryRegionV1 {
                allocation: self.allocations[2 * pair + 1],
                access: RuntimeAccessV1::Write,
                byte_offset: 5,
                byte_len: self.initial[2 * pair + 1].len() as u64 - 13,
            },
        ]
    }

    fn backend_regions(&self, pair: usize) -> [BackendMemoryRegionV1; 2] {
        let regions = self.regions(pair);
        std::array::from_fn(|index| BackendMemoryRegionV1 {
            allocation: self.global_allocations[2 * pair + index],
            access: regions[index].access,
            byte_offset: regions[index].byte_offset,
            byte_len: regions[index].byte_len,
        })
    }

    fn expected(&self, lists: &[&[RuntimePeerCopySegmentV1]]) -> Vec<Vec<u8>> {
        let mut expected = self.initial.clone();
        for (pair, list) in lists.iter().enumerate() {
            let regions = self.regions(pair);
            for entry in *list {
                let from = (regions[0].byte_offset + entry.source_offset) as usize;
                let to = (regions[1].byte_offset + entry.destination_offset) as usize;
                let len = entry.byte_len as usize;
                expected[2 * pair + 1][to..to + len]
                    .copy_from_slice(&self.initial[2 * pair][from..from + len]);
            }
        }
        expected
    }

    fn assert_storage(&self, context: &Context, expected: &[Vec<u8>]) {
        let backend = context.backend();
        for (child, route) in self.routes.iter().enumerate() {
            let record = &backend.children[child].allocations[&route.local];
            let KfdRuntimeSdmaStorageV1::Device(owner) = &record.sdma_storage else {
                panic!("original whole owner not restored for child {child}")
            };
            assert_eq!(owner.scripted_owner_id(), self.identities[child]);
            assert_eq!(owner.scripted_bytes().unwrap(), expected[child]);
            assert!(record.sdma_initialized && record.sdma_backed);
        }
        assert!(backend.compute_xgmi_children.iter().all(Option::is_none));
        assert_eq!(backend.cooperative_staging_bytes, 0);
        // Scripted completion is never counted as completed physical native DMA.
        assert_eq!(backend.completed_compute_xgmi_copies, 0);
        backend.assert_cooperative_indexes_consistent();
    }
}

struct Native {
    context: ManuallyDrop<Context>,
    layout: Layout,
}

impl Native {
    fn new(sizes: &[usize], pending_samples: usize) -> Self {
        assert!(matches!(sizes.len(), 2 | 4));
        let children = (0..sizes.len())
            .map(|index| {
                let mut child = KfdRuntimeBackendV1::mock();
                child.description.backend_device = 7 + index as u64;
                child
            })
            .collect();
        let mut context = ManuallyDrop::new(
            Context::open_with_version_journal_v1(
                KfdMultiDeviceRuntimeBackendV1::from_backends(children).unwrap(),
                64,
                32,
            )
            .unwrap(),
        );
        let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
        let initial: Vec<Vec<u8>> = sizes
            .iter()
            .enumerate()
            .map(|(child, bytes)| {
                (0..*bytes)
                    .map(|byte| ((byte as u64 * 37 + 11) ^ (child as u64 * 71)) as u8)
                    .collect()
            })
            .collect();
        let streams: Vec<_> = devices
            .iter()
            .map(|device| context.create_stream(*device).unwrap())
            .collect();
        let allocations: Vec<_> = devices
            .iter()
            .enumerate()
            .map(|(child, device)| {
                let id = context
                    .allocate(
                        *device,
                        RuntimeMemoryKindV1::DeviceLocal,
                        sizes[child] as u64,
                        8,
                    )
                    .unwrap();
                context.write_allocation(id, 0, &initial[child]).unwrap();
                id
            })
            .collect();
        let backend = context.backend_mut_for_test_v1();
        let pairs: Vec<_> = (0..sizes.len())
            .map(|child| {
                backend
                    .allocations
                    .iter()
                    .find(|(_, route)| route.child == child)
                    .map(|(id, route)| (*id, *route))
                    .unwrap()
            })
            .collect();
        let global_streams = (0..sizes.len())
            .map(|child| {
                *backend
                    .streams
                    .iter()
                    .find(|(_, route)| route.child == child)
                    .unwrap()
                    .0
            })
            .collect();
        let identities = pairs
            .iter()
            .enumerate()
            .map(|(index, (_, route))| {
                let child = &mut backend.children[index];
                let driver = ScriptedSdmaDriverV1::new(release_steps(sizes[index]));
                let mut owner = driver.test_device_owner(sizes[index]);
                owner
                    .scripted_bytes_mut()
                    .unwrap()
                    .copy_from_slice(&initial[index]);
                let identity = owner.scripted_owner_id();
                let record = child.allocations.get_mut(&route.local).unwrap();
                record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(owner));
                record.sdma_backed = true;
                record.sdma_initialized = true;
                record.sdma_shadow_dirty = true;
                child.native_available = true;
                child.sdma_enabled = true;
                child.peer_visible_device_allocations = true;
                child.scripted_sdma = Some(driver);
                identity
            })
            .collect();
        for source in (0..sizes.len()).step_by(2) {
            backend.compute_xgmi_routes.insert(
                (source, source + 1),
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
                global_allocations: pairs.iter().map(|(id, _)| *id).collect(),
                global_streams,
                routes: pairs.iter().map(|(_, route)| *route).collect(),
                identities,
                initial,
            },
        }
    }

    fn submit(&mut self, pair: usize, list: &[RuntimePeerCopySegmentV1]) -> Submission {
        let [source, destination] = self.layout.regions(pair);
        self.context
            .peer_copy_segments(
                self.layout.streams[2 * pair + 1],
                source,
                destination,
                list,
                &[],
            )
            .unwrap()
    }

    fn id(&self, submission: &Submission) -> u64 {
        self.context
            .backend_submission_for_test_v1(submission)
            .unwrap()
    }

    fn clean(mut self, submissions: Vec<Submission>, expected: Vec<Vec<u8>>) {
        self.layout.assert_storage(&self.context, &expected);
        for submission in submissions.into_iter().rev() {
            self.context.release_submission(submission).unwrap();
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
    for child in &context.backend().children {
        let driver = child.scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.remaining_steps(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
    }
    assert!(context.backend().submissions.is_empty());
}

fn copy(context: &Context, id: u64) -> &CooperativeCopySubmissionV1 {
    let RoutedSubmissionV1::CooperativeCopy(copy) = &context.backend().submissions[&id] else {
        panic!("ordered list must retain the original cooperative root")
    };
    assert!(copy.staging.is_empty());
    assert_eq!(copy.scratch_byte_len, 0);
    copy
}

fn root(context: &Context, id: u64) -> &Root {
    copy(context, id).compute_xgmi.as_deref().unwrap()
}

fn leaves(trace: &[Stage]) -> usize {
    trace
        .iter()
        .filter(|stage| {
            matches!(
                stage,
                Stage::Copy | Stage::Poll | Stage::NextPacket | Stage::NextSegment | Stage::Finish
            )
        })
        .count()
}

fn drive(f: &mut Native, submission: &mut Submission, pair: usize) {
    for _ in 0..128 {
        f.context
            .progress_stream_v1(f.layout.streams[2 * pair + 1])
            .unwrap();
        match f.context.poll(submission).unwrap() {
            RuntimePollV1::Pending => {}
            RuntimePollV1::Succeeded => return,
            status => panic!("ordered native list failed: {status:?}"),
        }
    }
    panic!("ordered native list did not settle within its bounded progress budget")
}

fn assert_pair_held(f: &Native, id: u64, pair: usize) {
    let backend = f.context.backend();
    let transport = root(&f.context, id);
    for index in 0..2 {
        let child = 2 * pair + index;
        assert_eq!(backend.compute_xgmi_children[child], Some(id));
        assert!(
            matches!(backend.children[child].allocations[&f.layout.routes[child].local].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner)) if owner == id)
        );
        assert_eq!(
            transport.scripted_owners[index]
                .as_ref()
                .unwrap()
                .scripted_owner_id(),
            f.layout.identities[child]
        );
    }
    assert!(!transport.is_quiescent());
    backend.assert_cooperative_indexes_consistent();
}

#[test]
fn unified_segments_snapshot_overlap_and_final_only_context_completion() {
    let mut f = Native::new(&[81, 117], 2);
    let original = descriptors();
    let expected = f.layout.expected(&[&original]);
    let mut caller = original.clone();
    let mut submission = f.submit(0, &caller);
    let id = f.id(&submission);
    caller.reverse();
    caller[0].byte_len = 0;
    let callbacks = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&callbacks);
    f.context
        .on_completion(&submission, move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    let event = f.context.record_event(&submission).unwrap();
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    let plan = root(&f.context, id).segments.as_ref().unwrap().clone();
    assert_eq!(plan.windows().len(), original.len());
    assert_eq!(plan.total_bytes(), 36);
    let mut seen_next = 0;
    let mut succeeded = false;
    for _ in 0..64 {
        let before = root(&f.context, id).trace.clone();
        assert_eq!(
            f.context.poll(&mut submission).unwrap(),
            RuntimePollV1::Pending
        );
        assert_eq!(
            f.context.query_event(event).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(
            f.context.wait_event(event, Duration::ZERO).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(root(&f.context, id).trace, before);
        f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
        let transport = root(&f.context, id);
        assert!(Arc::ptr_eq(transport.segments.as_ref().unwrap(), &plan));
        let phase = transport.phase;
        let trace = transport.trace.clone();
        assert!(leaves(&trace) - leaves(&before) <= 1);
        if phase == Phase::Retired {
            assert_eq!(
                f.context.poll(&mut submission).unwrap(),
                RuntimePollV1::Succeeded
            );
            succeeded = true;
            break;
        }
        if phase != Phase::Prepared {
            assert_pair_held(&f, id, 0);
            assert_eq!(
                f.context.cancel(&mut submission).unwrap(),
                RuntimeCancellationV1::TooLate
            );
        }
        seen_next = trace
            .iter()
            .filter(|stage| **stage == Stage::NextSegment)
            .count();
        assert_eq!(callbacks.load(Ordering::SeqCst), 0);
        assert!(
            !trace
                .iter()
                .any(|stage| matches!(stage, Stage::Finish | Stage::Retire | Stage::Restore))
        );
        for allocation in &f.layout.allocations {
            assert!(f.context.write_allocation(*allocation, 0, &[1]).is_err());
            assert!(f.context.release_allocation(*allocation).is_err());
        }
        assert!(
            f.context
                .backend_mut_for_test_v1()
                .shutdown_native_v1()
                .is_err()
        );
    }
    assert!(succeeded);
    assert_eq!(seen_next, original.len() - 1);
    assert_eq!(callbacks.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.context.query_event(event).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    for stage in [Stage::Create, Stage::Finish, Stage::Retire, Stage::Restore] {
        assert_eq!(
            root(&f.context, id)
                .trace
                .iter()
                .filter(|entry| **entry == stage)
                .count(),
            1
        );
    }
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    f.context.release_event(event).unwrap();
    f.clean(vec![submission], expected);
}

#[test]
fn unified_segments_complete_preflight_rejects_late_descriptor_and_envelope_errors() {
    for invalid in 0..10 {
        let mut f = Native::new(&[81, 117], 0);
        let mut list = descriptors();
        let [mut source, mut destination] = f.layout.backend_regions(0);
        match invalid {
            0 => list.clear(),
            1 => list = vec![segment(0, 0, 1); 4097],
            2 => list.push(segment(0, 0, 0)),
            3 => list.push(segment(u64::MAX, 0, 1)),
            4 => list.push(segment(0, u64::MAX, 1)),
            5 => list.push(segment(source.byte_len, 0, 1)),
            6 => list.push(segment(0, destination.byte_len, 1)),
            7 => source.byte_offset = u64::MAX,
            8 => destination.byte_offset = u64::MAX,
            9 => source.byte_len = f.layout.initial[0].len() as u64,
            _ => unreachable!(),
        }
        let stream = f.layout.global_streams[1];
        let before = f.context.backend().next_handle;
        assert!(matches!(
            f.context.backend_mut_for_test_v1().peer_copy_segments_v1(
                stream,
                source,
                destination,
                &list,
                &[]
            ),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(f.context.backend().next_handle, before);
        assert!(f.context.backend().submissions.is_empty());
        let expected = f.layout.initial.clone();
        f.clean(vec![], expected);
    }
}

#[test]
fn unified_segments_native_only_admission_does_not_silently_stage() {
    for invalid in 0..5 {
        let mut f = Native::new(&[81, 117], 0);
        let [source, destination] = f.layout.backend_regions(0);
        let stream = f.layout.global_streams[1];
        let routes = f.layout.routes.clone();
        let backend = f.context.backend_mut_for_test_v1();
        match invalid {
            0 => backend.compute_xgmi_routes.clear(),
            1 => backend.children[0].peer_visible_device_allocations = false,
            2 => {
                backend.children[0]
                    .allocations
                    .get_mut(&routes[0].local)
                    .unwrap()
                    .kind = RuntimeMemoryKindV1::HostVisible
            }
            3 => {
                backend.children[0]
                    .allocations
                    .get_mut(&routes[0].local)
                    .unwrap()
                    .sdma_initialized = false
            }
            4 => {
                backend.children[1]
                    .allocations
                    .get_mut(&routes[1].local)
                    .unwrap()
                    .sdma_initialized = false
            }
            _ => unreachable!(),
        }
        let before = backend.next_handle;
        assert!(matches!(
            backend.peer_copy_segments_v1(stream, source, destination, &descriptors(), &[]),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(backend.next_handle, before);
        assert!(backend.submissions.is_empty());
        assert_eq!(backend.cooperative_staging_bytes, 0);
        assert!(backend.compute_xgmi_children.iter().all(Option::is_none));
        backend.children[0].peer_visible_device_allocations = true;
        for (child, route) in routes.iter().enumerate() {
            let record = backend.children[child]
                .allocations
                .get_mut(&route.local)
                .unwrap();
            record.kind = RuntimeMemoryKindV1::DeviceLocal;
            record.sdma_initialized = true;
        }
        let expected = f.layout.initial.clone();
        f.clean(vec![], expected);
    }
}

#[test]
fn unified_segments_accepts_bound_and_cancels_only_before_any_publication() {
    for count in [1, 4096] {
        let mut f = Native::new(&[81, 117], 0);
        let list = vec![segment(0, 0, 1); count];
        let mut submission = f.submit(0, &list);
        let id = f.id(&submission);
        assert_eq!(
            root(&f.context, id)
                .segments
                .as_ref()
                .unwrap()
                .windows()
                .len(),
            count
        );
        assert_eq!(
            f.context.cancel(&mut submission).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        assert!(root(&f.context, id).trace.is_empty());
        let expected = f.layout.initial.clone();
        f.clean(vec![submission], expected);
    }
}

#[test]
fn unified_segments_packet_tails_preserve_each_window_and_one_final_restoration() {
    let cap = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
    let mut f = Native::new(&[cap + 101, cap + 147], 0);
    let list = [
        segment(1, 9, cap as u64 + 13),
        segment(3, 2, 1),
        segment(7, 17, 31),
    ];
    let expected = f.layout.expected(&[&list]);
    let mut submission = f.submit(0, &list);
    let id = f.id(&submission);
    assert_eq!(
        root(&f.context, id)
            .segments
            .as_ref()
            .unwrap()
            .packet_count(),
        4
    );
    drive(&mut f, &mut submission, 0);
    let trace = &root(&f.context, id).trace;
    assert_eq!(
        trace
            .iter()
            .filter(|stage| **stage == Stage::NextPacket)
            .count(),
        1
    );
    assert_eq!(
        trace
            .iter()
            .filter(|stage| **stage == Stage::NextSegment)
            .count(),
        2
    );
    for stage in [Stage::Finish, Stage::Retire, Stage::Restore] {
        assert_eq!(trace.iter().filter(|entry| **entry == stage).count(), 1);
    }
    f.clean(vec![submission], expected);
}

#[test]
fn unified_segments_quiescent_packet_boundary_keeps_whole_pair_reserved() {
    let mut f = Native::new(&[81, 117], 0);
    let list = [segment(0, 0, 9), segment(12, 4, 13)];
    let mut submission = f.submit(0, &list);
    let id = f.id(&submission);
    for _ in 0..8 {
        f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
        if root(&f.context, id).between_segments {
            break;
        }
    }
    let transport = root(&f.context, id);
    assert!(transport.between_segments);
    assert_eq!(transport.segment_index, 0);
    assert_eq!(transport.phase, Phase::Published);
    assert_pair_held(&f, id, 0);
    let first_only = f.layout.expected(&[&list[..1]]);
    assert_eq!(
        transport.scripted_owners[1]
            .as_ref()
            .unwrap()
            .scripted_bytes()
            .unwrap(),
        first_only[1]
    );
    let before = transport.trace.clone();
    for _ in 0..3 {
        assert_eq!(
            f.context.poll(&mut submission).unwrap(),
            RuntimePollV1::Pending
        );
        assert_eq!(
            f.context.cancel(&mut submission).unwrap(),
            RuntimeCancellationV1::TooLate
        );
        assert_eq!(root(&f.context, id).trace, before);
    }
    let [source, destination] = f.layout.backend_regions(0);
    let stream = f.layout.global_streams[1];
    let next = f.context.backend().next_handle;
    assert!(matches!(
        f.context.backend_mut_for_test_v1().peer_copy_segments_v1(
            stream,
            source,
            destination,
            &list,
            &[]
        ),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(f.context.backend().next_handle, next);
    for child in 0..2 {
        let allocation = f.layout.global_allocations[child];
        assert!(
            f.context
                .backend_mut_for_test_v1()
                .write_allocation_v1(allocation, 0, &[0])
                .is_err()
        );
        assert!(
            f.context
                .backend_mut_for_test_v1()
                .release_allocation_v1(allocation)
                .is_err()
        );
    }
    assert_pair_held(&f, id, 0);
    drive(&mut f, &mut submission, 0);
    let expected = f.layout.expected(&[&list]);
    f.clean(vec![submission], expected);
}

#[test]
fn unified_segments_logical_extent_not_physical_padding_controls_admission() {
    for source_side in [true, false] {
        let mut f = Native::new(&[81, 117], 0);
        let child = usize::from(!source_side);
        let route = f.layout.routes[child];
        // The allocation metadata admits 40 logical bytes while the scripted
        // pooled owner still has its larger physical backing. Nothing publishes.
        f.context.backend_mut_for_test_v1().children[child]
            .allocations
            .get_mut(&route.local)
            .unwrap()
            .bytes = vec![0; 40].into();
        let [mut source, mut destination] = f.layout.backend_regions(0);
        let region = if source_side {
            &mut source
        } else {
            &mut destination
        };
        region.byte_offset = 39;
        region.byte_len = 2;
        let stream = f.layout.global_streams[1];
        let before = f.context.backend().next_handle;
        assert!(matches!(
            f.context.backend_mut_for_test_v1().peer_copy_segments_v1(
                stream,
                source,
                destination,
                &[segment(0, 0, 2)],
                &[]
            ),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(f.context.backend().next_handle, before);
        assert!(f.context.backend().submissions.is_empty());
        f.context.backend_mut_for_test_v1().children[child]
            .allocations
            .get_mut(&route.local)
            .unwrap()
            .bytes = f.layout.initial[child].clone().into();
        let expected = f.layout.initial.clone();
        f.clean(vec![], expected);
    }
}

#[test]
fn unified_segments_second_descriptor_failure_keeps_applied_prefix_and_original_custody() {
    for stage in [
        Stage::NextSegment,
        Stage::Poll,
        Stage::Finish,
        Stage::Retire,
        Stage::Restore,
    ] {
        for unwind in [false, true] {
            let mut f = Native::new(&[81, 117], 0);
            let list = [segment(0, 0, 9), segment(12, 4, 13)];
            let submission = f.submit(0, &list);
            let id = f.id(&submission);
            for _ in 0..8 {
                f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
                if root(&f.context, id).between_segments {
                    break;
                }
            }
            assert!(root(&f.context, id).between_segments);
            assert_pair_held(&f, id, 0);
            let RoutedSubmissionV1::CooperativeCopy(copy) = f
                .context
                .backend_mut_for_test_v1()
                .submissions
                .get_mut(&id)
                .unwrap()
            else {
                unreachable!()
            };
            copy.compute_xgmi.as_mut().unwrap().route = Route::Scripted {
                failure: Some(stage),
                unwind,
                pending_samples: 0,
            };
            let stream = f.layout.global_streams[1];
            let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), Failure> {
                for _ in 0..16 {
                    f.context
                        .backend_mut_for_test_v1()
                        .progress_stream_v1(stream)?;
                }
                Ok(())
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(RuntimeBackendFailureV1::Terminal(_))
                ));
            }
            let backend = f.context.backend();
            assert!(backend.terminal && backend.children.iter().all(|child| child.terminal));
            assert_eq!(backend.cooperative_progress_quantum, None);
            assert_eq!(root(&f.context, id).trace.last(), Some(&stage));
            assert_pair_held(&f, id, 0);
            let expected =
                f.layout
                    .expected(&[if matches!(stage, Stage::NextSegment | Stage::Poll) {
                        &list[..1]
                    } else {
                        &list
                    }]);
            for (index, bytes) in expected.iter().enumerate() {
                assert_eq!(
                    root(&f.context, id).scripted_owners[index]
                        .as_ref()
                        .unwrap()
                        .scripted_bytes()
                        .unwrap(),
                    bytes
                );
                let driver = backend.children[index].scripted_sdma.as_ref().unwrap();
                assert_eq!(driver.live_owner_count(), 1);
                assert_eq!(driver.unexpected_drops(), 0);
            }
            assert_eq!(backend.completed_compute_xgmi_copies, 0);
            // Preserve the terminal Context and both native roots. Prefix effects
            // do not authorize rollback, result release or invented cleanup.
        }
    }
}

#[test]
fn unified_segments_envelope_drift_fails_closed_before_the_next_descriptor() {
    for source_side in [true, false] {
        let mut f = Native::new(&[81, 117], 0);
        let list = [segment(0, 0, 9), segment(12, 4, 13)];
        let submission = f.submit(0, &list);
        let id = f.id(&submission);
        for _ in 0..8 {
            f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
            if root(&f.context, id).between_segments {
                break;
            }
        }
        assert!(root(&f.context, id).between_segments);
        let trace = root(&f.context, id).trace.clone();
        let plan = root(&f.context, id).segments.as_ref().unwrap().clone();
        let RoutedSubmissionV1::CooperativeCopy(copy) = f
            .context
            .backend_mut_for_test_v1()
            .submissions
            .get_mut(&id)
            .unwrap()
        else {
            unreachable!()
        };
        if source_side {
            copy.source_region.byte_offset += 1;
        } else {
            copy.destination_region.byte_offset += 1;
        }
        let stream = f.layout.global_streams[1];
        assert!(matches!(
            f.context
                .backend_mut_for_test_v1()
                .progress_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(root(&f.context, id).trace, trace);
        assert!(Arc::ptr_eq(
            root(&f.context, id).segments.as_ref().unwrap(),
            &plan
        ));
        assert_pair_held(&f, id, 0);
        assert!(f.context.backend().terminal);
        assert_eq!(f.context.backend().cooperative_progress_quantum, None);
        // Deliberate immutable-root corruption retains custody until process exit.
    }
}

fn ready<F: Future>(future: Pin<&mut F>) -> F::Output {
    match future.poll(&mut TaskContext::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("owner tick did not service its observation command"),
    }
}
