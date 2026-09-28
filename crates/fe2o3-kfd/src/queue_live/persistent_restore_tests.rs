//! Registered storage and real dispatch transitions with CPU-injected completion.

use super::tests::{persistent_compute_cancellation_test_session, test_queue_key};
use super::*;
use crate::persistent_allocation::{
    PersistentOwnerSnapshotForTestV1, PersistentUseIdentityForTestV1,
};
use crate::persistent_compute::{
    Gfx942PersistentComputeTerminalStageV1, PersistentComputeAttachmentEntryV1,
};
use crate::persistent_directional_sdma::Gfx942PersistentDirectionalSdmaAttachmentV1;
use crate::queue::completion::CompletionCustodySnapshotV1;
use crate::queue::dispatch_binding::control_release::{
    RetainedControlSnapshotV1, ReturningControlCleanupCustodyV1, ReturningControlModeV1,
};
use crate::queue::dispatch_binding::preparation::persistent_cancel_control_in_memory_v1;
use crate::shared_memory::{
    DataCleanupCustodyV1, DispatchDataReleaseV1, PreparationMemoryFixtureV1 as Memory,
    PreparationMemoryObservationV1,
};
use arrayvec::ArrayVec;

struct Case {
    memory: Memory,
    session: ComputeAqlQueueSessionV1,
    binding: PersistentComputeBindingKeyV1,
    published: Option<Gfx942DispatchBatchV1<1>>,
    recycle: Option<Gfx942CompletionRecycleObservationV1>,
    identities: Vec<Gfx942DeviceMemoryIdentityV1>,
    digests: Vec<Option<[u8; 32]>>,
}

type FrontierIdentity = (usize, Gfx942DeviceMemoryIdentityV1, u64, u64);

#[derive(Debug, Eq, PartialEq)]
enum UseSnapshot {
    Published(PersistentUseIdentityForTestV1),
    Recycled(PersistentUseIdentityForTestV1),
}

#[derive(Debug, Eq, PartialEq)]
struct EntrySnapshot {
    owner: PersistentOwnerSnapshotForTestV1,
    attachment: Gfx942PersistentDirectionalSdmaAttachmentV1,
    initialization: PersistentComputeInitializationV1,
    storage_identity: Option<Gfx942DeviceMemoryIdentityV1>,
    effect: Gfx942PersistentComputeEffectV1,
    state: UseSnapshot,
}

#[derive(Debug, Eq, PartialEq)]
struct CaseSnapshot {
    memory: PreparationMemoryObservationV1,
    completion: CompletionCustodySnapshotV1,
    completion_poisoned: bool,
    control: RetainedControlSnapshotV1,
    queue: QueueKeyV1,
    binding: PersistentComputeBindingKeyV1,
    predecessor: Option<u64>,
    entries: Vec<EntrySnapshot>,
    detached_count: usize,
    detached_generation: Option<u64>,
    detached_identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    detached_next: Option<usize>,
    next_generation: u64,
    terminal_poisoned: bool,
}

#[derive(Debug, Eq, PartialEq)]
struct CompletedSnapshot {
    owner: PersistentOwnerSnapshotForTestV1,
    attachment: Gfx942PersistentDirectionalSdmaAttachmentV1,
    frontier: FrontierIdentity,
    effect: Gfx942PersistentComputeEffectV1,
    digest: Option<[u8; 32]>,
    initialized: bool,
}

fn completed_snapshot(
    completed: &Gfx942ThreeBindingPersistentComputeCompletedV1,
) -> [CompletedSnapshot; 3] {
    completed
        .completed
        .each_ref()
        .map(|entry| CompletedSnapshot {
            owner: entry.allocation.owner.ownership_snapshot_for_test_v1(),
            attachment: entry.allocation.attachment,
            frontier: entry.frontier.identity_for_test_v1(),
            effect: entry.effect,
            digest: entry.authenticated_sha256,
            initialized: entry.fully_initialized,
        })
}

impl Case {
    fn new(count: usize, initialized: bool) -> Self {
        Self::new_for_queue(count, initialized, 710)
    }

    fn new_for_queue(count: usize, initialized: bool, queue_id: u64) -> Self {
        let mut case = Self::published_for_queue(count, initialized, queue_id);
        case.complete_and_recycle();
        case
    }

    fn published_for_queue(count: usize, initialized: bool, queue_id: u64) -> Self {
        assert!(count == 1 || (count == 3 && initialized));
        let mut memory = Memory::new(true);
        let mut queue = test_queue_key(queue_id, 1);
        queue.vm = memory.primary_vm();
        let binding = PersistentComputeBindingKeyV1 {
            queue,
            attachment_generation: 1,
        };
        let pair = crate::persistent_directional_sdma::Gfx942PersistentDirectionalSdmaPairV1 {
            host_to_device_queue_id: 17,
            device_to_host_queue_id: 23,
        };
        let mut entries = ArrayVec::new();
        let mut roster = Vec::new();
        let mut identities = Vec::new();
        let mut digests = Vec::new();
        for index in 0..count {
            let read = count == 3 && index < 2;
            let data = memory.device(initialized);
            let content = data.initialized_content();
            let buffer =
                Gfx942SdmaBufferV1::from_bridge_parts(data.into_sdma_storage(), queue, 1, 4096);
            let (mut allocation, _) =
                promote_directional_persistent_sdma_custody_v1(buffer, pair, 1).unwrap();
            let request = Gfx942PersistentUseRequestV1::new(
                if read {
                    Gfx942PersistentOperationV1::ComputeRead
                } else {
                    Gfx942PersistentOperationV1::ComputeWrite
                },
                0,
                4096,
            )
            .unwrap();
            let reserved = allocation.owner.reserve(request, None).unwrap();
            let prepared = allocation.owner.prepare(reserved).unwrap();
            let lease = allocation
                .owner
                .detach_local_native_for_compute(&prepared)
                .unwrap();
            let identity = lease.storage_identity();
            let data = match content {
                Some(content) => Gfx942FixedDispatchDataV1::initialized(
                    Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                        lease, content,
                    )
                    .unwrap(),
                ),
                None => Gfx942FixedDispatchDataV1::uninitialized(lease),
            };
            let digest = content.map(|c| c.sha256());
            entries.push(PersistentComputeAttachmentEntryV1 {
                allocation,
                initialization: PersistentComputeInitializationV1::from_test_parts(
                    digest,
                    initialized,
                ),
                state: PersistentComputeUseStateV1::Prepared(prepared),
                storage_identity: Some(identity),
                effect: if read {
                    Gfx942PersistentComputeEffectV1::Read
                } else {
                    Gfx942PersistentComputeEffectV1::Write
                },
            });
            roster.push(data);
            identities.push(identity);
            digests.push(digest);
        }
        let dispatch = persistent_cancel_control_in_memory_v1(&mut memory, queue, roster, Some(7));
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        session.dispatch = Some(dispatch);
        session.persistent_compute = Some(BoundedPersistentComputeAttachmentV1 {
            entries,
            binding,
            predecessor_dispatch_generation: Some(7),
            terminal_custody: None,
        });
        let (templates, epoch) = session
            .dispatch
            .as_mut()
            .unwrap()
            .bind_templates::<1>(queue)
            .unwrap();
        let (_, retention) = session
            .completion_owner
            .bind_batch(templates)
            .unwrap()
            .into_parts();
        let published = session
            .completion_owner
            .mark_published(retention, 1)
            .unwrap();
        session
            .dispatch
            .as_mut()
            .unwrap()
            .mark_published(epoch, &published)
            .unwrap();
        for entry in &mut session.persistent_compute.as_mut().unwrap().entries {
            let state =
                core::mem::replace(&mut entry.state, PersistentComputeUseStateV1::Quarantined);
            let PersistentComputeUseStateV1::Prepared(prepared) = state else {
                unreachable!()
            };
            entry.state = PersistentComputeUseStateV1::Published(
                entry.allocation.owner.publish(prepared).unwrap(),
            );
        }
        Self {
            memory,
            session,
            binding,
            published: Some(wrap_published(published, epoch)),
            recycle: None,
            identities,
            digests,
        }
    }

    fn complete_and_recycle(&mut self) {
        let (published, epoch) = unwrap_published(self.published.take().unwrap());
        let completed = self
            .session
            .completion_owner
            .complete_one_without_native_for_test(published);
        self.session
            .dispatch
            .as_mut()
            .unwrap()
            .mark_completed(epoch, &completed)
            .unwrap();
        let occurrence = completed.occurrence_v1().unwrap();
        let recycle = self
            .session
            .completion_owner
            .recycle_one_without_native_for_test(completed);
        self.session
            .dispatch
            .as_mut()
            .unwrap()
            .mark_recycled_occurrence(epoch, occurrence)
            .unwrap();
        for entry in &mut self.session.persistent_compute.as_mut().unwrap().entries {
            let state =
                core::mem::replace(&mut entry.state, PersistentComputeUseStateV1::Quarantined);
            let PersistentComputeUseStateV1::Published(published) = state else {
                unreachable!()
            };
            entry.state = PersistentComputeUseStateV1::Recycled(
                entry.allocation.owner.complete(published).unwrap(),
            );
        }
        self.recycle = Some(recycle);
    }

    fn snapshot(&self) -> CaseSnapshot {
        let session = &self.session;
        let attachment = session.persistent_compute.as_ref().unwrap();
        assert!(attachment.terminal_custody.is_none());
        CaseSnapshot {
            memory: self.memory.observation(),
            completion: session.completion_owner.custody_snapshot_for_test(),
            completion_poisoned: session.completion_owner.is_poisoned_for_test(),
            control: RetainedControlSnapshotV1::recycled_owner_v1(
                session.dispatch.as_ref().unwrap(),
            ),
            queue: session.key,
            binding: attachment.binding,
            predecessor: attachment.predecessor_dispatch_generation,
            entries: attachment
                .entries
                .iter()
                .map(|entry| EntrySnapshot {
                    owner: entry.allocation.owner.ownership_snapshot_for_test_v1(),
                    attachment: entry.allocation.attachment,
                    initialization: entry.initialization,
                    storage_identity: entry.storage_identity,
                    effect: entry.effect,
                    state: match &entry.state {
                        PersistentComputeUseStateV1::Published(lease) => {
                            UseSnapshot::Published(lease.cancellation_identity_for_test())
                        }
                        PersistentComputeUseStateV1::Recycled(lease) => {
                            UseSnapshot::Recycled(lease.cancellation_identity_for_test())
                        }
                        _ => panic!("expected published or recycled test custody"),
                    },
                })
                .collect(),
            detached_count: session.detached_data_count,
            detached_generation: session.detached_dispatch_generation,
            detached_identities: session.detached_data_identities.clone(),
            detached_next: session.detached_next_insertion_index,
            next_generation: session.next_persistent_compute_generation,
            terminal_poisoned: session.terminal_poisoned,
        }
    }

    fn recycled_three_receipt(&self) -> Gfx942RecycledThreeBindingPersistentComputeDispatchV1 {
        Gfx942RecycledThreeBindingPersistentComputeDispatchV1 {
            binding: self.binding,
            recycle: self.recycle.unwrap(),
            thread_affinity: PhantomData,
        }
    }

    fn finish_three(&mut self, receipt: Gfx942RecycledThreeBindingPersistentComputeDispatchV1) {
        let completed = self
            .session
            .detach_recycled_three_binding_directional_persistent_fixed_dispatch_v1(receipt)
            .unwrap();
        for (index, completed) in completed.into_completed().into_iter().enumerate() {
            self.check_completed(completed, index, true);
        }
        assert!(self.session.persistent_compute.is_none());
        assert!(!self.session.terminal_poisoned);
        self.release_control();
    }

    fn release_data(&mut self, data: Gfx942FixedDispatchDataV1) {
        let mut cleanup = DataCleanupCustodyV1::new(data);
        self.memory.release_data(&mut cleanup).unwrap();
        assert!(cleanup.is_complete());
    }

    fn release_control(&mut self) {
        let mut cleanup = ReturningControlCleanupCustodyV1::new(
            self.session.dispatch.take().unwrap(),
            ReturningControlModeV1::DetachedPersistent {
                expected_generation: self.session.detached_dispatch_generation.unwrap(),
            },
        );
        cleanup.release_in_place(&mut self.memory).unwrap();
        assert!(cleanup.is_complete());
        self.memory.primary_assert_all_released_v1();
    }

    fn check_completed(
        &mut self,
        completed: Gfx942PersistentComputeCompletedV1,
        index: usize,
        initialized: bool,
    ) {
        let expected_digest = if self.identities.len() == 3 && index < 2 {
            self.digests[index]
        } else {
            None
        };
        assert_eq!(completed.authenticated_sha256, expected_digest);
        assert_eq!(completed.fully_initialized, initialized);
        let input = completed.retire_settled_frontier_for_replay_v1().unwrap().0;
        self.check_input(input, index, initialized);
    }

    fn check_input(
        &mut self,
        input: Gfx942PersistentComputeInputV1,
        index: usize,
        initialized: bool,
    ) {
        let mut allocation = input.into_allocation();
        assert_eq!(allocation.owner.live_use_count(), 0);
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
        let attachment = allocation.attachment;
        let buffer = allocation
            .owner
            .detach_sdma_buffer(
                attachment.queue,
                attachment.pool_generation,
                attachment.logical_bytes,
            )
            .unwrap();
        assert_eq!(
            buffer.storage_identity(),
            Gfx942SdmaBufferStorageIdentityV1::Device(self.identities[index])
        );
        assert_eq!(buffer.initialized_range_is_known(0, 4096), initialized);
        let mut cleanup = DataCleanupCustodyV1::from_sdma(buffer);
        self.memory.release_data(&mut cleanup).unwrap();
        assert!(cleanup.is_complete());
    }
}

#[test]
fn persistent_completed_restore_foreign_poll_returns_exact_published_receipt() {
    let mut original = Case::published_for_queue(3, true, 710);
    let mut foreign = Case::published_for_queue(3, true, 711);
    let (completion, epoch) = unwrap_published(original.published.take().unwrap());
    let occurrence = completion.occurrence_v1().unwrap();
    let receipt = Gfx942ThreeBindingPersistentComputeDispatchV1 {
        binding: original.binding,
        batch: wrap_published(completion, epoch),
        thread_affinity: PhantomData,
    };
    let original_before = original.snapshot();
    let foreign_before = foreign.snapshot();
    let (error, recovered) = foreign
        .session
        .poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1(receipt)
        .err()
        .expect("foreign queue must reject published receipt")
        .into_parts();
    assert!(matches!(
        error,
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(original.snapshot(), original_before);
    assert_eq!(foreign.snapshot(), foreign_before);
    let recovered = recovered.expect("foreign queue must return the caller's receipt");
    assert_eq!(recovered.binding, original.binding);
    let (completion, recovered_epoch) = unwrap_published(recovered.batch);
    assert_eq!(recovered_epoch, epoch);
    assert_eq!(completion.occurrence_v1().unwrap(), occurrence);
    let recovered = Gfx942ThreeBindingPersistentComputeDispatchV1 {
        binding: recovered.binding,
        batch: wrap_published(completion, recovered_epoch),
        thread_affinity: PhantomData,
    };
    let pending = original
        .session
        .poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1_using(
            recovered,
            |session, identity, completion| {
                session
                    .dispatch
                    .as_ref()
                    .unwrap()
                    .validate_published(identity, completion)
                    .is_ok()
            },
            |session, completion| {
                session
                    .completion_owner
                    .observe_one_pending_with_current_closing_for_test(completion)
                    .map_err(|(error, completion)| (error.into(), completion))
            },
        )
        .unwrap();
    let Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Pending(receipt) = pending else {
        panic!("injected Pending must preserve original-session custody")
    };
    assert_eq!(receipt.binding, original.binding);
    let (completion, continued_epoch) = unwrap_published(receipt.batch);
    assert_eq!(continued_epoch, epoch);
    assert_eq!(completion.occurrence_v1().unwrap(), occurrence);
    original.published = Some(wrap_published(completion, continued_epoch));
    assert_eq!(original.snapshot(), original_before);
    assert_eq!(foreign.snapshot(), foreign_before);
    for case in [&mut original, &mut foreign] {
        case.complete_and_recycle();
        case.finish_three(case.recycled_three_receipt());
    }
}

#[test]
fn persistent_completed_restore_foreign_detach_returns_exact_recycled_receipt() {
    let mut original = Case::new_for_queue(3, true, 710);
    let mut foreign = Case::new_for_queue(3, true, 711);
    let original_before = original.snapshot();
    let foreign_before = foreign.snapshot();
    let (error, recovered) = foreign
        .session
        .detach_recycled_three_binding_directional_persistent_fixed_dispatch_v1(
            original.recycled_three_receipt(),
        )
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    let recovered = recovered.expect("foreign queue must return the caller's receipt");
    assert_eq!(recovered.binding, original.binding);
    assert_eq!(Some(recovered.recycle), original.recycle);
    assert_eq!(original.snapshot(), original_before);
    assert_eq!(foreign.snapshot(), foreign_before);
    original.finish_three(recovered);
    foreign.finish_three(foreign.recycled_three_receipt());
}

#[test]
fn persistent_completed_restore_three_retirement_rejection_preserves_all_frontiers() {
    for rejected in 0..3 {
        let mut case = Case::new(3, true);
        let mut completed = case
            .session
            .detach_recycled_three_binding_directional_persistent_fixed_dispatch_v1(
                case.recycled_three_receipt(),
            )
            .unwrap();
        for entry in &completed.completed {
            assert_eq!(entry.allocation.owner.live_use_count(), 0);
            assert_eq!(entry.allocation.owner.retained_settled_use_count(), 1);
        }
        let entry = &mut completed.completed[rejected];
        let request =
            Gfx942PersistentUseRequestV1::new(Gfx942PersistentOperationV1::ComputeWrite, 0, 4096)
                .unwrap();
        assert_eq!(
            entry
                .allocation
                .owner
                .reserve(request, None)
                .unwrap_err()
                .error(),
            Gfx942PersistentUseErrorV1::DependencyRequired
        );
        let reserved = entry
            .allocation
            .owner
            .reserve(request, Some(&entry.frontier))
            .unwrap();
        assert_eq!(entry.allocation.owner.live_use_count(), 1);
        let before = completed_snapshot(&completed);
        let memory_before = case.memory.observation();
        let mut recovered = completed
            .retire_settled_frontiers_for_replay_v1()
            .unwrap_err();
        assert_eq!(
            completed_snapshot(&recovered),
            before,
            "rejected ordinal {rejected}"
        );
        assert_eq!(case.memory.observation(), memory_before);
        for entry in &recovered.completed {
            assert_eq!(entry.allocation.owner.retained_settled_use_count(), 1);
        }
        recovered.completed[rejected]
            .allocation
            .owner
            .cancel_reserved(reserved)
            .unwrap();
        let inputs = recovered.retire_settled_frontiers_for_replay_v1().unwrap();
        for (index, (input, effect)) in inputs.into_iter().enumerate() {
            assert_eq!(
                effect,
                if index < 2 {
                    Gfx942PersistentComputeEffectV1::Read
                } else {
                    Gfx942PersistentComputeEffectV1::Write
                }
            );
            if index < 2 {
                let Gfx942PersistentComputeInputV1::Initialized(ready) = &input else {
                    panic!("read input must preserve its authenticated digest")
                };
                assert_eq!(Some(ready.authenticated_sha256), case.digests[index]);
            } else {
                assert!(matches!(
                    input,
                    Gfx942PersistentComputeInputV1::InitializedAfterDispatch(_)
                ));
            }
            case.check_input(input, index, true);
        }
        case.release_control();
    }
}

#[test]
fn persistent_completed_restore_public_single_preserves_cold_and_initialized_storage() {
    for initialized in [false, true] {
        let mut case = Case::new(1, initialized);
        let receipt = Gfx942RecycledPersistentComputeDispatchV1 {
            binding: case.binding,
            recycle: case.recycle.unwrap(),
            thread_affinity: PhantomData,
        };
        let completed = case
            .session
            .detach_recycled_directional_persistent_fixed_dispatch_v1(receipt)
            .unwrap();
        case.check_completed(completed, 0, initialized);
        assert!(case.session.persistent_compute.is_none());
        assert!(!case.session.terminal_poisoned);
        case.release_control();
    }
}

#[test]
fn persistent_completed_restore_public_three_preserves_storage_and_read_digests() {
    let mut case = Case::new(3, true);
    let receipt = Gfx942RecycledThreeBindingPersistentComputeDispatchV1 {
        binding: case.binding,
        recycle: case.recycle.unwrap(),
        thread_affinity: PhantomData,
    };
    let completed = case
        .session
        .detach_recycled_three_binding_directional_persistent_fixed_dispatch_v1(receipt)
        .unwrap();
    for (index, completed) in completed.completed.into_iter().enumerate() {
        case.check_completed(completed, index, true);
    }
    assert!(case.session.persistent_compute.is_none());
    assert!(!case.session.terminal_poisoned);
    case.release_control();
}

#[test]
fn persistent_completed_restore_public_scope_failure_keeps_entire_original_roster() {
    for (count, initialized) in [(1, false), (1, true), (3, true)] {
        for rejected in 0..count {
            let mut case = Case::new(count, initialized);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            case.session.persistent_compute.as_mut().unwrap().entries[rejected]
                .allocation
                .attachment
                .pool_generation += 1;
            if count == 1 {
                let failure = case
                    .session
                    .detach_recycled_directional_persistent_fixed_dispatch_v1(
                        Gfx942RecycledPersistentComputeDispatchV1 {
                            binding: case.binding,
                            recycle: case.recycle.unwrap(),
                            thread_affinity: PhantomData,
                        },
                    )
                    .err()
                    .expect("scope substitution must reject");
                assert!(failure.recovered.is_none());
                assert!(failure.retained.is_none());
            } else {
                let failure = case
                    .session
                    .detach_recycled_three_binding_directional_persistent_fixed_dispatch_v1(
                        Gfx942RecycledThreeBindingPersistentComputeDispatchV1 {
                            binding: case.binding,
                            recycle: case.recycle.unwrap(),
                            thread_affinity: PhantomData,
                        },
                    )
                    .unwrap_err();
                assert!(failure.recovered.is_none());
            }
            assert!(case.session.terminal_poisoned);
            assert_eq!(take_dispatch_terminal_process_gate_record_v1(), count == 3);
            let mut attachment = case.session.persistent_compute.take().unwrap();
            for entry in &attachment.entries {
                assert!(!entry.allocation.owner.local_native_is_attached_for_sdma());
                assert_eq!(entry.allocation.owner.live_use_count(), 1);
                assert!(entry.allocation.owner.quarantine_reason().is_some());
                assert!(matches!(
                    entry.state,
                    PersistentComputeUseStateV1::Quarantined
                ));
            }
            let terminal = attachment.terminal_custody.take().unwrap();
            assert_eq!(
                terminal.stage(),
                Some(Gfx942PersistentComputeTerminalStageV1::DataDetached)
            );
            let PersistentComputeTerminalNativeCustodyV1::Data(data) = terminal else {
                unreachable!()
            };
            assert_eq!(data.len(), count);
            for (data, identity) in data.iter().zip(&case.identities) {
                assert_eq!(
                    data.storage_identity(),
                    if initialized {
                        Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedStorage(*identity)
                    } else {
                        Gfx942FixedDispatchStorageIdentityV1::DeviceUninitialized(*identity)
                    }
                );
                assert_eq!(data.layout().requested_bytes(), 4096);
                assert_eq!(data.is_fully_initialized(), initialized);
                assert!(data.initialized_content().is_none());
            }
            for data in data.into_data_for_test() {
                case.release_data(data);
            }
            crate::queue::dispatch_binding::preparation::teardown_completed_restore_fixture_v1(
                case.session.dispatch.take().unwrap(),
                &mut case.memory,
            );
            case.memory.primary_assert_all_released_v1();
        }
    }
}
