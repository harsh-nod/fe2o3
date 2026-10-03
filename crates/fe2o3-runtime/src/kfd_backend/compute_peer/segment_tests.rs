//! Actual routed compute custody and scripted segment bytes, not computed arithmetic.

#[path = "settled_segment_tests.rs"]
mod settled;

#[path = "ordered_segment_tests.rs"]
mod ordered;

use super::window::initialize_bytes;
use super::*;
use crate::{RuntimePeerCopySegmentV1, RuntimePeerCopySegmentsBackendV1};
use fe2o3_kfd::Gfx942ComputeXgmiSegmentsPlanV1;

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

fn descriptors() -> Vec<RuntimePeerCopySegmentV1> {
    [(0, 0, 9), (12, 4, 13), (0, 0, 9), (3, 27, 1)]
        .map(
            |(source_offset, destination_offset, byte_len)| RuntimePeerCopySegmentV1 {
                source_offset,
                destination_offset,
                byte_len,
            },
        )
        .to_vec()
}

fn regions(f: &Fixture) -> [BackendMemoryRegionV1; 2] {
    [
        BackendMemoryRegionV1 {
            byte_offset: 3,
            byte_len: 53,
            ..region(f.allocations[0][2], RuntimeAccessV1::Read)
        },
        BackendMemoryRegionV1 {
            byte_offset: 5,
            byte_len: 51,
            ..region(f.allocations[1][3], RuntimeAccessV1::Write)
        },
    ]
}

fn owner(f: &Fixture, allocation: u64) -> (u64, &[u8]) {
    let route = f.backend.allocations[&allocation];
    let owner = match &f.backend.children[route.child].allocations[&route.local].sdma_storage {
        KfdRuntimeSdmaStorageV1::Device(owner) => owner.as_ref(),
        KfdRuntimeSdmaStorageV1::InitializedStorage(owner) => match owner.as_ref() {
            initialized_storage::InitializedStorageOwnerV1::Scripted(owner) => owner,
            _ => panic!("scripted initialized owner expected"),
        },
        _ => panic!("original native owner must be restored"),
    };
    (
        owner.scripted_owner_id().unwrap(),
        owner.scripted_bytes().unwrap(),
    )
}

struct PendingList {
    f: Fixture,
    producer: Option<u64>,
    producer_event: Option<u64>,
    list: u64,
    event: u64,
    identities: [u64; 4],
    source: Vec<u8>,
    input: Vec<u8>,
    output: Vec<u8>,
    returned: Vec<u8>,
}

impl PendingList {
    fn new(queued: bool, direct: bool, readback: bool) -> Self {
        Self::new_timed(queued, direct, readback, false)
    }

    fn new_timed(queued: bool, direct: bool, readback: bool, settled: bool) -> Self {
        Self::with_readback_progress(queued, direct, readback, settled, true)
    }

    fn with_readback_progress(
        queued: bool,
        direct: bool,
        readback: bool,
        settled: bool,
        readback_will_run: bool,
    ) -> Self {
        Self::with_source_profile(Some((queued, settled)), direct, readback, readback_will_run)
    }

    fn with_source_profile(
        compute: Option<(bool, bool)>,
        direct: bool,
        readback: bool,
        readback_will_run: bool,
    ) -> Self {
        let mut f = Fixture::with_layout_driver_prefixes_and_readback_progress(
            [BYTES; 2],
            readback.then_some(if direct {
                (1, 0, 0, BYTES)
            } else {
                (0, 7, 9, 47)
            }),
            false,
            [
                Vec::new(),
                if direct {
                    Vec::new()
                } else {
                    vec![ScriptedSdmaStepV1::PromoteInitializedStorage(
                        ScriptedFailureModeV1::Success,
                    )]
                },
            ],
            readback_will_run,
        );
        let source_allocation = f.allocations[0][2];
        let input_allocation = f.allocations[1][3];
        let output_allocation = f.allocations[1][2];
        let returned_allocation = f.allocations[0][3];
        let (source_id, source) = initialize_bytes(&mut f, source_allocation, 0x31);
        let (input_id, input) = initialize_bytes(&mut f, input_allocation, 0xc3);
        let (output_id, output) = initialize_bytes(&mut f, output_allocation, 0x49);
        let (return_id, returned) = initialize_bytes(&mut f, returned_allocation, 0xaf);
        let producer = compute.map(|(queued, _)| f.launch(0, 0, queued, true));
        let producer_event = producer.map(|id| f.event(0, id));
        if let Some((_, true)) = compute {
            let producer = producer.unwrap();
            f.drive(f.compute_streams[0], producer);
            assert_eq!(
                f.backend.poll_v1(producer).unwrap(),
                BackendPollV1::Succeeded
            );
            assert_eq!(owner(&f, source_allocation), (source_id, source.as_slice()));
        }
        let [source_region, destination] = regions(&f);
        let mut list = descriptors();
        let submission = f
            .backend
            .peer_copy_segments_v1(
                f.peer_stream,
                source_region,
                destination,
                &list,
                producer_event.as_slice(),
            )
            .unwrap();
        let retained = Arc::clone(
            f.copy(submission)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .segments_for_test_v1()
                .unwrap(),
        );
        assert_eq!(
            f.copy(submission).compute_producer.is_some(),
            compute.is_some()
        );
        assert!(f.copy(submission).staging.is_empty());
        assert_eq!(retained.windows().len(), list.len());
        list.reverse();
        list.fill(RuntimePeerCopySegmentV1 {
            source_offset: u64::MAX,
            destination_offset: u64::MAX,
            byte_len: 0,
        });
        assert!(Arc::ptr_eq(
            &retained,
            f.copy(submission)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .segments_for_test_v1()
                .unwrap()
        ));
        let event = f
            .backend
            .record_event_v1(f.peer_stream, submission)
            .unwrap();
        Self {
            f,
            producer,
            producer_event,
            list: submission,
            event,
            identities: [source_id, input_id, output_id, return_id],
            source,
            input,
            output,
            returned,
        }
    }

    fn expected_input(&self, count: usize) -> Vec<u8> {
        let mut expected = self.input.clone();
        for segment in descriptors().iter().take(count) {
            let source = 3 + segment.source_offset as usize;
            let destination = 5 + segment.destination_offset as usize;
            let bytes = segment.byte_len as usize;
            expected[destination..destination + bytes]
                .copy_from_slice(&self.source[source..source + bytes]);
        }
        expected
    }

    fn bindings(&self) -> [BackendBindingV1; 3] {
        std::array::from_fn(|index| BackendBindingV1 {
            region: region(
                self.f.allocations[1][[3, 1, 2][index]],
                if index == 2 {
                    RuntimeAccessV1::Write
                } else {
                    RuntimeAccessV1::Read
                },
            ),
            kernarg_byte_offset: index as u32 * 8,
        })
    }

    fn submit_consumer(
        &mut self,
        bindings: &[BackendBindingV1],
        dependency: BackendLaunchProducerV1,
    ) -> Result<u64, Failure> {
        let mut kernarg = [0; 32];
        kernarg[24..].copy_from_slice(&16_u64.to_le_bytes());
        self.f
            .backend
            .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                stream: self.f.compute_streams[1],
                kernel: self.f.kernels[1],
                explicit_kernarg: &kernarg,
                bindings,
                dependencies: &[dependency],
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
            })
    }

    fn consumer(&mut self) -> u64 {
        self.submit_consumer(
            &self.bindings(),
            BackendLaunchProducerV1 {
                event: self.event,
                producer_submission: self.list,
            },
        )
        .unwrap()
    }

    fn release_events(&mut self) {
        self.f.backend.release_event_v1(self.event).unwrap();
        if let Some(event) = self.producer_event {
            self.f.backend.release_event_v1(event).unwrap();
        }
    }

    fn assert_deferred(&self, consumer: u64) {
        let root = self.f.backend.deferred_compute_v1(consumer).unwrap();
        assert!(root.route.is_none());
        assert_eq!(root.status, BackendPollV1::Pending);
        assert!(!self.f.backend.children[1].any_compute_active_v1());
        assert!(self.f.backend.children[1].pending_compute.is_empty());
    }

    fn publish(&mut self) {
        for _ in 0..64 {
            self.f
                .backend
                .progress_stream_v1(self.f.peer_stream)
                .unwrap();
            if self
                .f
                .copy(self.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .contains(&Stage::Copy)
            {
                self.assert_pair();
                return;
            }
        }
        panic!("native first packet must publish under bounded progress");
    }

    fn assert_pair(&self) {
        assert!(
            self.f
                .backend
                .compute_xgmi_children
                .iter()
                .all(|id| *id == Some(self.list))
        );
        for allocation in [self.f.allocations[0][2], self.f.allocations[1][3]] {
            let route = self.f.backend.allocations[&allocation];
            assert!(
                matches!(self.f.backend.children[route.child].allocations[&route.local].sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(id)) if id == self.list)
            );
        }
        let root = self.f.copy(self.list).compute_xgmi.as_ref().unwrap();
        for (index, id) in self.identities[..2].iter().enumerate() {
            assert_eq!(
                root.scripted_owners_for_test_v1()[index]
                    .as_ref()
                    .unwrap()
                    .scripted_owner_id(),
                Some(*id)
            );
        }
    }

    fn direct_readback(&mut self) -> Result<u64, Failure> {
        self.f.backend.copy_async_v1(
            self.f.readback_stream,
            region(self.f.allocations[1][3], RuntimeAccessV1::Read),
            region(self.f.host.unwrap(), RuntimeAccessV1::Write),
            &[self.event],
        )
    }

    fn drive(&mut self, stream: u64, target: u64, consumer: Option<u64>) {
        for _ in 0..256 {
            if self.f.copy(self.list).status() == BackendPollV1::Pending {
                if let Some(id) = consumer {
                    self.assert_deferred(id);
                }
                let root = self.f.copy(self.list).compute_xgmi.as_ref().unwrap();
                if !root.is_quiescent() {
                    self.assert_pair();
                }
                assert!(!root.trace_for_test_v1().contains(&Stage::Restore));
            }
            self.f.backend.progress_stream_v1(stream).unwrap();
            if self.f.backend.poll_v1(target).unwrap() != BackendPollV1::Pending {
                return;
            }
        }
        panic!("pending list pipeline must settle under bounded final-stream progress");
    }
}

#[test]
fn pending_segments_compute_full_pipeline_releases_events_and_restores_exact_frame() {
    for queued in [false, true] {
        for late in [false, true] {
            let mut p = PendingList::new(queued, false, true);
            if late {
                p.publish();
            }
            let consumer = p.consumer();
            p.assert_deferred(consumer);
            let event = p.f.event(1, consumer);
            p.f.backend.compute_xgmi_routes.insert(
                (1, 0),
                Route::Scripted {
                    failure: None,
                    unwind: false,
                    pending_samples: 2,
                },
            );
            let stream = p.f.backend.create_stream_v1(7).unwrap();
            let returned =
                p.f.backend
                    .peer_copy_v1(
                        stream,
                        BackendMemoryRegionV1 {
                            byte_offset: 17,
                            byte_len: 47,
                            ..region(p.f.allocations[1][2], RuntimeAccessV1::Read)
                        },
                        BackendMemoryRegionV1 {
                            byte_offset: 7,
                            byte_len: 47,
                            ..region(p.f.allocations[0][3], RuntimeAccessV1::Write)
                        },
                        &[event],
                    )
                    .unwrap();
            p.f.backend.release_event_v1(event).unwrap();
            let event = p.f.backend.record_event_v1(stream, returned).unwrap();
            let readback =
                p.f.backend
                    .copy_async_v1(
                        p.f.readback_stream,
                        BackendMemoryRegionV1 {
                            byte_offset: 7,
                            byte_len: 47,
                            ..region(p.f.allocations[0][3], RuntimeAccessV1::Read)
                        },
                        BackendMemoryRegionV1 {
                            byte_offset: 9,
                            byte_len: 47,
                            ..region(p.f.host.unwrap(), RuntimeAccessV1::Write)
                        },
                        &[event],
                    )
                    .unwrap();
            p.f.backend.release_event_v1(event).unwrap();
            p.release_events();
            let generation = p.f.backend.cooperative_progress_generation;
            for id in [p.list, consumer, returned, readback] {
                assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
                assert_eq!(
                    p.f.backend.wait_v1(id, Instant::now()).unwrap(),
                    BackendPollV1::Pending
                );
                assert!(p.f.backend.release_submission_v1(id).is_err());
            }
            assert_eq!(p.f.backend.cooperative_progress_generation, generation);
            p.drive(p.f.readback_stream, readback, Some(consumer));
            for id in [p.producer.unwrap(), p.list, consumer, returned, readback] {
                assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
            }
            assert_eq!(
                owner(&p.f, p.f.allocations[0][2]),
                (p.identities[0], p.source.as_slice())
            );
            assert_eq!(
                owner(&p.f, p.f.allocations[1][3]),
                (p.identities[1], p.expected_input(4).as_slice())
            );
            assert_eq!(
                owner(&p.f, p.f.allocations[1][2]),
                (p.identities[2], p.output.as_slice())
            );
            p.returned[7..54].copy_from_slice(&p.output[17..64]);
            assert_eq!(
                owner(&p.f, p.f.allocations[0][3]),
                (p.identities[3], p.returned.as_slice())
            );
            let host = p.f.backend.allocations[&p.f.host.unwrap()];
            let KfdRuntimeSdmaStorageV1::Host(host) =
                &p.f.backend.children[0].allocations[&host.local].sdma_storage
            else {
                unreachable!()
            };
            let mut expected = vec![0; BYTES];
            expected[9..56].copy_from_slice(&p.output[17..64]);
            assert_eq!(host.scripted_bytes().unwrap(), expected);
            let root = p.f.copy(p.list).compute_xgmi.as_ref().unwrap();
            for stage in [Stage::Create, Stage::Finish, Stage::Retire, Stage::Restore] {
                assert_eq!(
                    root.trace_for_test_v1()
                        .iter()
                        .filter(|seen| **seen == stage)
                        .count(),
                    1
                );
            }
            assert_eq!(
                root.trace_for_test_v1()
                    .iter()
                    .filter(|seen| **seen == Stage::NextSegment)
                    .count(),
                3
            );
            assert!(
                p.f.backend
                    .compute_xgmi_children
                    .iter()
                    .all(Option::is_none)
            );
            assert!(p.f.backend.deferred_compute_retains.is_empty());
            assert_eq!(p.f.backend.completed_compute_xgmi_copies, 0);
            p.f.clean();
        }
    }
}

#[test]
fn pending_segments_direct_readback_observes_full_preserved_frame_not_envelope() {
    for queued in [false, true] {
        for late in [false, true] {
            let mut p = PendingList::new(queued, true, true);
            if late {
                p.publish();
            }
            let readback = p.direct_readback().unwrap();
            p.release_events();
            assert_eq!(
                p.f.backend.poll_v1(readback).unwrap(),
                BackendPollV1::Pending
            );
            p.drive(p.f.readback_stream, readback, None);
            assert_eq!(
                p.f.backend.poll_v1(readback).unwrap(),
                BackendPollV1::Succeeded
            );
            assert_eq!(
                owner(&p.f, p.f.allocations[1][3]),
                (p.identities[1], p.expected_input(4).as_slice())
            );
            let route = p.f.backend.allocations[&p.f.host.unwrap()];
            let KfdRuntimeSdmaStorageV1::Host(host) =
                &p.f.backend.children[1].allocations[&route.local].sdma_storage
            else {
                unreachable!()
            };
            assert_eq!(host.scripted_bytes().unwrap(), p.expected_input(4));
            assert_eq!(
                owner(&p.f, p.f.allocations[0][2]),
                (p.identities[0], p.source.as_slice())
            );
            assert_eq!(p.f.backend.completed_compute_xgmi_copies, 0);
            p.f.clean();
        }
    }
}

#[test]
fn pending_segments_cancelled_list_does_not_authorize_consumer_or_readback() {
    for direct in [false, true] {
        let mut p = PendingList::with_readback_progress(true, true, direct, false, false);
        let native_steps = p.f.backend.children[1]
            .scripted_sdma
            .as_ref()
            .unwrap()
            .remaining_steps();
        let target = if direct {
            p.direct_readback().unwrap()
        } else {
            p.consumer()
        };
        p.release_events();
        assert_eq!(
            p.f.backend.cancel_v1(p.list).unwrap(),
            BackendCancellationV1::Cancelled
        );
        let stream = if direct {
            p.f.readback_stream
        } else {
            p.f.compute_streams[1]
        };
        for _ in 0..64 {
            match p.f.backend.progress_stream_v1(stream) {
                Ok(()) => {}
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    assert!(matches!(
                        p.f.backend.poll_v1(target).unwrap(),
                        BackendPollV1::Failed { .. }
                    ));
                    if direct {
                        assert!(p.f.copy(target).is_quiescent());
                    } else {
                        assert!(
                            p.f.backend
                                .deferred_compute_v1(target)
                                .unwrap()
                                .route
                                .is_none()
                        );
                        assert!(p.f.backend.deferred_compute_retains.is_empty());
                    }
                }
                result => panic!("unexpected cancellation progress: {result:?}"),
            }
            if matches!(
                p.f.backend.poll_v1(target).unwrap(),
                BackendPollV1::Failed { .. }
            ) {
                break;
            }
        }
        assert!(matches!(
            p.f.backend.poll_v1(target).unwrap(),
            BackendPollV1::Failed { .. }
        ));
        assert!(
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        assert!(!p.f.backend.children[1].any_compute_active_v1());
        assert_eq!(
            p.f.backend.children[1]
                .scripted_sdma
                .as_ref()
                .unwrap()
                .remaining_steps(),
            native_steps
        );
        assert!(
            p.f.backend
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(
            p.f.backend.cancel_v1(p.producer.unwrap()).unwrap(),
            BackendCancellationV1::Cancelled
        );
        p.f.clean();
    }
}

#[test]
fn pending_segments_postpublication_cancel_and_prefix_failure_preserve_custody() {
    for unwind in [false, true] {
        for stage in [
            Stage::NextSegment,
            Stage::Poll,
            Stage::Finish,
            Stage::Restore,
        ] {
            let mut p = PendingList::new(false, false, false);
            let consumer = p.consumer();
            p.release_events();
            for _ in 0..64 {
                p.f.backend
                    .progress_stream_v1(p.f.compute_streams[1])
                    .unwrap();
                if p.f
                    .copy(p.list)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .between_segments_for_test_v1()
                {
                    break;
                }
            }
            assert!(
                p.f.copy(p.list)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .between_segments_for_test_v1()
            );
            p.assert_pair();
            p.assert_deferred(consumer);
            assert_eq!(
                p.f.backend.cancel_v1(p.list).unwrap(),
                BackendCancellationV1::TooLate
            );
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                p.f.backend.submissions.get_mut(&p.list).unwrap()
            else {
                unreachable!()
            };
            copy.compute_xgmi
                .as_mut()
                .unwrap()
                .inject_failure_for_test_v1(stage, unwind);
            let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<(), Failure> {
                for _ in 0..64 {
                    p.f.backend.progress_stream_v1(p.f.compute_streams[1])?;
                }
                Ok(())
            }));
            if unwind {
                assert!(outcome.is_err());
            } else {
                assert!(matches!(
                    outcome.unwrap(),
                    Err(RuntimeBackendFailureV1::Terminal(_))
                ));
            }
            assert!(
                p.f.backend.terminal && p.f.backend.children.iter().all(|child| child.terminal)
            );
            assert_eq!(p.f.backend.cooperative_progress_quantum, None);
            p.assert_pair();
            p.assert_deferred(consumer);
            let root = p.f.copy(p.list).compute_xgmi.as_ref().unwrap();
            let count = if matches!(stage, Stage::NextSegment | Stage::Poll) {
                1
            } else {
                4
            };
            assert_eq!(
                root.scripted_owners_for_test_v1()[0]
                    .as_ref()
                    .unwrap()
                    .scripted_bytes()
                    .unwrap(),
                p.source
            );
            assert_eq!(
                root.scripted_owners_for_test_v1()[1]
                    .as_ref()
                    .unwrap()
                    .scripted_bytes()
                    .unwrap(),
                p.expected_input(count)
            );
            assert!(
                p.f.backend
                    .cooperative_dependency_retain_counts
                    .contains_key(&p.producer.unwrap())
            );
            assert_eq!(p.f.backend.completed_compute_xgmi_copies, 0);
            // Fail-stop native roots remain retained; no fake cleanup or byte rollback.
        }
    }
}

#[test]
fn pending_segments_plan_substitution_cannot_hide_behind_equal_envelopes() {
    for late in [false, true] {
        for changed in [false, true] {
            let mut p = PendingList::new(false, false, false);
            let consumer = p.consumer();
            if late {
                p.publish();
            }
            p.release_events();
            let trace =
                p.f.copy(p.list)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .trace_for_test_v1()
                    .to_vec();
            let mut list = descriptors();
            if changed {
                list[3].destination_offset += 1;
            }
            let replacement = Arc::new(
                Gfx942ComputeXgmiSegmentsPlanV1::new(
                    BYTES as u64,
                    BYTES as u64,
                    3,
                    53,
                    5,
                    51,
                    &list,
                )
                .unwrap(),
            );
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                p.f.backend.submissions.get_mut(&p.list).unwrap()
            else {
                unreachable!()
            };
            copy.compute_xgmi
                .as_mut()
                .unwrap()
                .replace_segments_for_test_v1(replacement);
            assert!(
                p.f.backend
                    .progress_stream_v1(p.f.compute_streams[1])
                    .is_err()
            );
            assert_eq!(
                p.f.copy(p.list)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .trace_for_test_v1(),
                trace
            );
            p.assert_deferred(consumer);
            assert_eq!(p.f.backend.completed_compute_xgmi_copies, 0);
            if late {
                p.assert_pair();
            }
        }
    }
}

#[test]
fn pending_segments_consumer_rejects_wrong_exact_event_writable_alias_and_bounds() {
    for case in 0..6 {
        let mut p = PendingList::new(true, true, false);
        let mut bindings = p.bindings();
        let mut dependency = BackendLaunchProducerV1 {
            event: p.event,
            producer_submission: p.list,
        };
        match case {
            0 => dependency.event = p.producer_event.unwrap(),
            1 => dependency.producer_submission = p.producer.unwrap(),
            2 => bindings[0].region.access = RuntimeAccessV1::ReadWrite,
            3 => {
                bindings[1].region = BackendMemoryRegionV1 {
                    access: RuntimeAccessV1::Write,
                    ..bindings[0].region
                }
            }
            4 => bindings[0].region.byte_len += 1,
            5 => bindings[0].region.byte_offset = u64::MAX,
            _ => unreachable!(),
        }
        let before = (
            p.f.backend.next_handle,
            p.f.backend.submissions.len(),
            p.f.backend.deferred_compute_retains.is_empty(),
        );
        assert!(matches!(
            p.submit_consumer(&bindings, dependency),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(
            (
                p.f.backend.next_handle,
                p.f.backend.submissions.len(),
                p.f.backend.deferred_compute_retains.is_empty()
            ),
            before
        );
        assert!(
            p.f.backend
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        assert!(
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        p.release_events();
        assert_eq!(
            p.f.backend.cancel_v1(p.list).unwrap(),
            BackendCancellationV1::Cancelled
        );
        assert_eq!(
            p.f.backend.cancel_v1(p.producer.unwrap()).unwrap(),
            BackendCancellationV1::Cancelled
        );
        p.f.clean();
    }
}

#[test]
fn pending_segments_physically_completed_native_source_keeps_exact_frame_provenance() {
    for queued in [false, true] {
        let mut p = PendingList::new_timed(queued, true, true, true);
        assert_eq!(
            p.f.backend.poll_v1(p.producer.unwrap()).unwrap(),
            BackendPollV1::Succeeded
        );
        let before = (p.f.backend.next_handle, p.f.backend.submissions.len());
        let wrong = p.f.backend.copy_async_v1(
            p.f.readback_stream,
            region(p.f.allocations[1][3], RuntimeAccessV1::Read),
            region(p.f.host.unwrap(), RuntimeAccessV1::Write),
            &[p.producer_event.unwrap()],
        );
        assert!(matches!(wrong, Err(RuntimeBackendFailureV1::Rejected(_))));
        assert_eq!(
            (p.f.backend.next_handle, p.f.backend.submissions.len()),
            before
        );
        assert!(
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        let readback = p.direct_readback().unwrap();
        p.release_events();
        assert!(
            p.f.backend
                .release_submission_v1(p.producer.unwrap())
                .is_err()
        );
        p.drive(p.f.readback_stream, readback, None);
        assert_eq!(
            p.f.backend.poll_v1(readback).unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[1][3]),
            (p.identities[1], p.expected_input(4).as_slice())
        );
        let route = p.f.backend.allocations[&p.f.host.unwrap()];
        let KfdRuntimeSdmaStorageV1::Host(host) =
            &p.f.backend.children[1].allocations[&route.local].sdma_storage
        else {
            unreachable!()
        };
        assert_eq!(host.scripted_bytes().unwrap(), p.expected_input(4));
        p.f.clean();
    }
}

#[test]
fn pending_segments_completed_deferred_source_retains_one_accounted_original_launch() {
    let mut p = PendingList::new(false, false, true);
    let account = fe2o3_resource_accounting::ResourceCreditAccountV1::new(
        fe2o3_resource_accounting::ResourceVectorV1::ZERO.with(
            fe2o3_resource_accounting::ResourceKindV1::ControlResidentBytes,
            4096,
        ),
        4,
    )
    .unwrap();
    p.f.backend.children[1].launch_payload_account = Some(account.clone());
    let before = account.usage();
    let consumer = p.consumer();
    let event = p.f.event(1, consumer);
    assert_eq!(
        account.usage().retained_records,
        before.retained_records + 1
    );
    p.release_events();
    p.drive(p.f.compute_streams[1], consumer, Some(consumer));
    assert_eq!(
        p.f.backend.poll_v1(consumer).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        owner(&p.f, p.f.allocations[1][2]),
        (p.identities[2], p.output.as_slice())
    );
    let settled_usage = account.usage();
    assert_eq!(settled_usage.retained_records, before.retained_records + 1);
    p.f.backend.compute_xgmi_routes.insert(
        (1, 0),
        Route::Scripted {
            failure: None,
            unwind: false,
            pending_samples: 2,
        },
    );
    let stream = p.f.backend.create_stream_v1(7).unwrap();
    let source = BackendMemoryRegionV1 {
        byte_offset: 3,
        byte_len: 53,
        ..region(p.f.allocations[1][2], RuntimeAccessV1::Read)
    };
    let destination = BackendMemoryRegionV1 {
        byte_offset: 5,
        byte_len: 51,
        ..region(p.f.allocations[0][3], RuntimeAccessV1::Write)
    };
    let list =
        p.f.backend
            .peer_copy_segments_v1(stream, source, destination, &descriptors(), &[event])
            .unwrap();
    assert!(p.f.copy(list).compute_producer.is_some());
    assert_eq!(account.usage(), settled_usage);
    p.f.backend.release_event_v1(event).unwrap();
    assert!(p.f.backend.release_submission_v1(consumer).is_err());
    let event = p.f.backend.record_event_v1(stream, list).unwrap();
    let readback =
        p.f.backend
            .copy_async_v1(
                p.f.readback_stream,
                BackendMemoryRegionV1 {
                    byte_offset: 7,
                    byte_len: 47,
                    ..region(p.f.allocations[0][3], RuntimeAccessV1::Read)
                },
                BackendMemoryRegionV1 {
                    byte_offset: 9,
                    byte_len: 47,
                    ..region(p.f.host.unwrap(), RuntimeAccessV1::Write)
                },
                &[event],
            )
            .unwrap();
    p.f.backend.release_event_v1(event).unwrap();
    p.drive(p.f.readback_stream, readback, None);
    assert_eq!(
        p.f.backend.poll_v1(readback).unwrap(),
        BackendPollV1::Succeeded
    );
    for segment in descriptors() {
        let from = 3 + segment.source_offset as usize;
        let to = 5 + segment.destination_offset as usize;
        let bytes = segment.byte_len as usize;
        p.returned[to..to + bytes].copy_from_slice(&p.output[from..from + bytes]);
    }
    assert_eq!(
        owner(&p.f, p.f.allocations[0][3]),
        (p.identities[3], p.returned.as_slice())
    );
    let host = p.f.backend.allocations[&p.f.host.unwrap()];
    let KfdRuntimeSdmaStorageV1::Host(host) =
        &p.f.backend.children[0].allocations[&host.local].sdma_storage
    else {
        unreachable!()
    };
    let mut expected = vec![0; BYTES];
    expected[9..56].copy_from_slice(&p.returned[7..54]);
    assert_eq!(host.scripted_bytes().unwrap(), expected);
    assert_eq!(account.usage(), settled_usage);
    p.f.clean();
    assert_eq!(account.usage(), before);
}
