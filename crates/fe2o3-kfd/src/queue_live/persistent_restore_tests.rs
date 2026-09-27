//! Registered storage and real dispatch transitions with CPU-injected completion.

use super::tests::{persistent_compute_cancellation_test_session, test_queue_key};
use super::*;
use crate::persistent_compute::{
    Gfx942PersistentComputeTerminalStageV1, PersistentComputeAttachmentEntryV1,
};
use crate::queue::dispatch_binding::control_release::{
    ReturningControlCleanupCustodyV1, ReturningControlModeV1,
};
use crate::queue::dispatch_binding::preparation::persistent_cancel_control_in_memory_v1;
use crate::shared_memory::{
    DataCleanupCustodyV1, DispatchDataReleaseV1, PreparationMemoryFixtureV1 as Memory,
};
use arrayvec::ArrayVec;

struct Case {
    memory: Memory,
    session: ComputeAqlQueueSessionV1,
    binding: PersistentComputeBindingKeyV1,
    recycle: Gfx942CompletionRecycleObservationV1,
    identities: Vec<Gfx942DeviceMemoryIdentityV1>,
    digests: Vec<Option<[u8; 32]>>,
}

impl Case {
    fn new(count: usize, initialized: bool) -> Self {
        assert!(count == 1 || (count == 3 && initialized));
        let mut memory = Memory::new(true);
        let mut queue = test_queue_key(710, 1);
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
                authenticated_sha256: digest,
                fully_initialized: initialized,
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
        let completed = session
            .completion_owner
            .complete_one_without_native_for_test(published);
        session
            .dispatch
            .as_mut()
            .unwrap()
            .mark_completed(epoch, &completed)
            .unwrap();
        let occurrence = completed.occurrence_v1().unwrap();
        let recycle = session
            .completion_owner
            .recycle_one_without_native_for_test(completed);
        session
            .dispatch
            .as_mut()
            .unwrap()
            .mark_recycled_occurrence(epoch, occurrence)
            .unwrap();
        for entry in &mut session.persistent_compute.as_mut().unwrap().entries {
            let state =
                core::mem::replace(&mut entry.state, PersistentComputeUseStateV1::Quarantined);
            let PersistentComputeUseStateV1::Prepared(prepared) = state else {
                unreachable!()
            };
            let published = entry.allocation.owner.publish(prepared).unwrap();
            entry.state = PersistentComputeUseStateV1::Recycled(
                entry.allocation.owner.complete(published).unwrap(),
            );
        }
        Self {
            memory,
            session,
            binding,
            recycle,
            identities,
            digests,
        }
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
        let mut allocation = completed
            .retire_settled_frontier_for_replay_v1()
            .unwrap()
            .0
            .into_allocation();
        assert_eq!(allocation.owner.live_use_count(), 0);
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
fn persistent_completed_restore_public_single_preserves_cold_and_initialized_storage() {
    for initialized in [false, true] {
        let mut case = Case::new(1, initialized);
        let receipt = Gfx942RecycledPersistentComputeDispatchV1 {
            binding: case.binding,
            recycle: case.recycle,
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
        recycle: case.recycle,
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
                            recycle: case.recycle,
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
                            recycle: case.recycle,
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
                        Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedAfterDispatch(
                            *identity,
                        )
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
