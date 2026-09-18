use super::*;
use crate::queue_linux::doorbell_release_tests::{cleanup_local_doorbell, local_doorbell};
use crate::shared_memory::PreparationMemoryFixtureV1;

#[path = "creation_fixture.rs"]
mod creation;
pub(crate) use creation::*;

#[path = "generic_fixture.rs"]
mod generic;
pub(crate) use generic::*;

pub(crate) fn directional(
    memory: &mut PreparationMemoryFixtureV1,
    key: QueueKeyV1,
) -> Gfx942SdmaQueueSetV1 {
    directional_with_ids(memory, key, 100)
}

pub(crate) fn generic(
    memory: &mut PreparationMemoryFixtureV1,
    key: QueueKeyV1,
    engine: Option<u32>,
) -> Gfx942SdmaQueueSetV1 {
    let mut owners = creation_owners(memory, key, 100, 1);
    owners[0].engine_index = engine;
    Gfx942SdmaQueueSetV1::Generic(owners)
}

pub(crate) fn directional_with_ids(
    memory: &mut PreparationMemoryFixtureV1,
    key: QueueKeyV1,
    first_id: u32,
) -> Gfx942SdmaQueueSetV1 {
    Gfx942SdmaQueueSetV1::Directional(creation_owners(memory, key, first_id, 2))
}

fn creation_owners(
    memory: &mut PreparationMemoryFixtureV1,
    key: QueueKeyV1,
    first_id: u32,
    count: u32,
) -> Vec<Gfx942SdmaQueueOwnerV1> {
    (0..count)
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
                engine_index: Some(index % 2),
                ring,
                control,
                completions,
                records: host.records,
                xgmi_records: host.xgmi_records,
                persistent_window_slots: host.persistent_window_slots,
                persistent_window_records: host.persistent_window_records,
            }
            .into_live(first_id + index, local_doorbell())
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OwnerObservation {
    pub(crate) key: QueueKeyV1,
    pub(crate) id: u32,
    pub(crate) engine: Option<u32>,
    pub(crate) destroyed: bool,
    pub(crate) poisoned: bool,
    pub(crate) identities: [Option<SharedGttAllocationIdentityV1>; 3],
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
    roster_address: usize,
    profile: Option<RetainedSdmaReleaseProfileV1>,
    pub(crate) state: (bool, bool, bool, usize, usize),
}

impl Observation {
    pub(crate) fn assert_original_owners(&self, before: &Self) {
        assert_eq!(self.roster_address, before.roster_address);
        assert_eq!(self.owners.len(), before.owners.len());
        for (owner, original) in self.owners.iter().zip(&before.owners) {
            assert_eq!(
                (owner.key, owner.id, owner.engine),
                (original.key, original.id, original.engine)
            );
            let identities = owner
                .resources
                .as_ref()
                .map_or(owner.identities, |resources| {
                    resources
                        .controls
                        .each_ref()
                        .map(|control| Some(control.identity))
                });
            assert_eq!(identities, original.identities);
        }
    }
}

pub(crate) fn unreleased_observation(set: &Gfx942SdmaQueueSetV1) -> Observation {
    observe(
        set,
        &std::array::from_fn(|_| OwnerProgressV1::default()),
        None,
        (false, false, false, 0, 0),
    )
}

impl RetainedSdmaReleaseCustodyV1 {
    pub(crate) fn assert_late_resource_failure(
        &self,
        memory: &PreparationMemoryFixtureV1,
        before: &crate::shared_memory::ControlReleaseMemorySnapshotV1,
        occurrence: usize,
        panic: bool,
    ) {
        let indices = self.profile.unwrap().owner_indices();
        let index = indices[occurrence - 1];
        before.assert_sdma_late_cleanup_v1(
            memory,
            self.progress[index].resources.as_ref().unwrap(),
            panic,
        );
        assert_eq!(self.released, occurrence - 1);
        assert_eq!(self.destroyed, indices.len());
        for &index in &indices[..occurrence - 1] {
            assert!(
                self.progress[index]
                    .resources
                    .as_ref()
                    .unwrap()
                    .is_complete()
            );
        }
        for &index in &indices[occurrence..] {
            assert!(self.progress[index].resources.is_none());
        }
    }
    pub(crate) fn observation(&self) -> Observation {
        observe(
            &self.set,
            &self.progress,
            self.profile,
            (
                self.started,
                self.resources_started,
                self.failed,
                self.destroyed,
                self.released,
            ),
        )
    }

    pub(crate) fn cleanup_local_mappings(&mut self) {
        cleanup_set(&mut self.set);
    }
}

fn observe(
    set: &Gfx942SdmaQueueSetV1,
    progress: &[OwnerProgressV1; 2],
    profile: Option<RetainedSdmaReleaseProfileV1>,
    state: (bool, bool, bool, usize, usize),
) -> Observation {
    let (Gfx942SdmaQueueSetV1::Generic(owners) | Gfx942SdmaQueueSetV1::Directional(owners)) = set
    else {
        panic!("fixture profile")
    };
    Observation {
        roster_address: owners.as_ptr() as usize,
        profile,
        owners: owners
            .iter()
            .enumerate()
            .map(|(index, owner)| {
                let p = &progress[index];
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
        state,
    }
}

pub(crate) fn cleanup_set(set: &mut Gfx942SdmaQueueSetV1) {
    let (Gfx942SdmaQueueSetV1::Generic(owners) | Gfx942SdmaQueueSetV1::Directional(owners)) = set
    else {
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
