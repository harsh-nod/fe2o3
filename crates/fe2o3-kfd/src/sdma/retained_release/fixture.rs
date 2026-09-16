use super::*;
use crate::queue_linux::doorbell_release_tests::{cleanup_local_doorbell, local_doorbell};
use crate::shared_memory::PreparationMemoryFixtureV1;

pub(crate) fn directional(
    memory: &mut PreparationMemoryFixtureV1,
    key: QueueKeyV1,
) -> Gfx942SdmaQueueSetV1 {
    let owners = (0..2)
        .map(|index| {
            let mut ring = memory.allocate::<AqlQueueGttV1>(4096).unwrap();
            memory
                .primary_write(&mut ring, |bytes| bytes.fill(0))
                .unwrap();
            let ring = memory.map(ring).unwrap();
            let ring = memory
                .primary_retain::<AqlRingResourceRoleV1, _, _>(ring)
                .unwrap();
            let mut control = memory.allocate::<UserptrAqlControlGttV1>(4096).unwrap();
            memory
                .primary_write(&mut control, initialize_amd_aql_control)
                .unwrap()
                .unwrap();
            let control = memory.map(control).unwrap();
            let control = memory
                .primary_retain::<AqlControlResourceRoleV1, _, _>(control)
                .unwrap();
            let mut completions = memory.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
            memory
                .primary_write(&mut completions, |bytes| bytes.fill(0))
                .unwrap();
            let completions = memory.map(completions).unwrap();
            let host = prepare_sdma_queue_host_resources()
                .unwrap_or_else(|_| panic!("fixture roster allocation"));
            PreparedGfx942SdmaQueueV1 {
                owner: key,
                engine_index: Some(index),
                ring,
                control,
                completions,
                records: host.records,
                xgmi_records: host.xgmi_records,
                persistent_window_slots: host.persistent_window_slots,
                persistent_window_records: host.persistent_window_records,
            }
            .into_live(100 + index, local_doorbell())
        })
        .collect();
    Gfx942SdmaQueueSetV1::Directional(owners)
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OwnerObservation {
    key: QueueKeyV1,
    id: u32,
    engine: Option<u32>,
    pub(crate) destroyed: bool,
    pub(crate) poisoned: bool,
    identities: [Option<SharedGttAllocationIdentityV1>; 3],
    pub(crate) request: Option<KfdIoctlDestroyQueueArgs>,
    pub(crate) attempted: bool,
    pub(crate) result: Option<Result<(), rustix::io::Errno>>,
    pub(crate) doorbell: LinuxDoorbellReleaseProgressV1,
    doorbell_present: bool,
    pub(crate) resources: Option<crate::shared_memory::QueueResourceCleanupObservationV1<3>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Observation {
    pub(crate) owners: Vec<OwnerObservation>,
    pub(crate) state: (bool, bool, bool, usize, usize),
}

impl DirectionalSdmaReleaseCustodyV1 {
    pub(crate) fn assert_late_resource_failure(
        &self,
        memory: &PreparationMemoryFixtureV1,
        before: &crate::shared_memory::ControlReleaseMemorySnapshotV1,
        occurrence: usize,
        panic: bool,
    ) {
        let index = 2 - occurrence;
        before.assert_sdma_late_cleanup_v1(
            memory,
            self.progress[index].resources.as_ref().unwrap(),
            panic,
        );
        assert_eq!(self.released, occurrence - 1);
        assert_eq!(self.destroyed, 2);
        if occurrence == 1 {
            assert!(self.progress[0].resources.is_none());
        } else {
            assert!(self.progress[1].resources.as_ref().unwrap().is_complete());
        }
    }
    pub(crate) fn observation(&self) -> Observation {
        let Gfx942SdmaQueueSetV1::Directional(owners) = &self.set else {
            panic!("fixture profile")
        };
        Observation {
            owners: owners
                .iter()
                .enumerate()
                .map(|(index, owner)| {
                    let p = &self.progress[index];
                    OwnerObservation {
                        key: owner.owner,
                        id: owner.queue_id,
                        engine: owner.engine_index,
                        destroyed: owner.destroyed,
                        poisoned: owner.poisoned,
                        identities: [
                            owner.completions.as_ref().map(|t| t.storage_identity()),
                            owner
                                .control
                                .as_ref()
                                .map(PreparationMemoryFixtureV1::primary_token_identity),
                            owner
                                .ring
                                .as_ref()
                                .map(PreparationMemoryFixtureV1::primary_token_identity),
                        ],
                        request: p.request,
                        attempted: p.attempted,
                        result: p.result,
                        doorbell: p.doorbell,
                        doorbell_present: owner.doorbell.is_some(),
                        resources: p.resources.as_ref().map(|r| r.observation()),
                    }
                })
                .collect(),
            state: (
                self.started,
                self.resources_started,
                self.failed,
                self.destroyed,
                self.released,
            ),
        }
    }

    pub(crate) fn cleanup_local_mappings(&mut self) {
        cleanup_set(&mut self.set);
    }
}

pub(crate) fn cleanup_set(set: &mut Gfx942SdmaQueueSetV1) {
    let Gfx942SdmaQueueSetV1::Directional(owners) = set else {
        panic!("fixture profile")
    };
    for owner in owners {
        cleanup_local_doorbell(owner.doorbell.as_mut().unwrap());
    }
}

pub(crate) fn corrupt_preflight(set: &mut Gfx942SdmaQueueSetV1, case: u8, primary_id: u32) {
    let Gfx942SdmaQueueSetV1::Directional(owners) = set else {
        panic!("fixture profile")
    };
    let duplicate = owners[1].queue_id;
    let owner = &mut owners[0]; // H2D would otherwise be destroyed first.
    match case {
        0 => owner.engine_index = Some(1),
        1 => owner.queue_id = primary_id,
        2 => owner.queue_id = duplicate,
        3 => owner.poisoned = true,
        4 => owner.destroyed = true,
        5 => owner.records.truncate(GFX942_SDMA_RING_SLOT_COUNT_V1 - 1),
        6 => {
            owner.uncertain_xgmi_ticket = Some(Gfx942SdmaCopyTicketV1 {
                owner: owner.owner,
                queue_id: owner.queue_id,
                slot: 0,
                generation: 1,
            })
        }
        7 => {
            owner.persistent_window_slots[0] = Some(PersistentSdmaWindowSlotV1 {
                anchor_slot: 0,
                generation: 1,
                completion_value: 1,
            })
        }
        8 => owner.owner.generation.0 += 1,
        _ => unreachable!(),
    }
}
