//! Borrowed observations and local-only disposal, never native recovery.

use super::*;
use crate::queue_linux::doorbell_release_tests::{
    LocalDoorbellObservation, observe_local_doorbell,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct StorageObservation {
    pointer: usize,
    len: usize,
    capacity: usize,
    occupied: Vec<usize>,
}

fn storage<T>(values: &Vec<Option<T>>) -> StorageObservation {
    StorageObservation {
        pointer: values.as_ptr() as usize,
        len: values.len(),
        capacity: values.capacity(),
        occupied: values
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.is_some().then_some(i))
            .collect(),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct CreationOwnerObservation {
    key: QueueKeyV1,
    id: u32,
    engine: Option<u32>,
    state: (bool, bool),
    identities: [Option<SharedGttAllocationIdentityV1>; 3],
    storage: [StorageObservation; 4],
    generations: Vec<u32>,
    uncertain: Option<Gfx942SdmaCopyTicketV1>,
    doorbell: Option<LocalDoorbellObservation>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CreationRosterObservation {
    storage: (usize, usize, usize),
    owners: Vec<CreationOwnerObservation>,
}

fn roster(owners: &Vec<Gfx942SdmaQueueOwnerV1>) -> CreationRosterObservation {
    CreationRosterObservation {
        storage: (owners.as_ptr() as usize, owners.len(), owners.capacity()),
        owners: owners
            .iter()
            .map(|owner| CreationOwnerObservation {
                key: owner.owner,
                id: owner.queue_id,
                engine: owner.engine_index,
                state: (owner.destroyed, owner.poisoned),
                identities: [
                    owner
                        .ring
                        .as_ref()
                        .map(PreparationMemoryFixtureV1::primary_token_identity),
                    owner
                        .control
                        .as_ref()
                        .map(PreparationMemoryFixtureV1::primary_token_identity),
                    owner.completions.as_ref().map(|v| v.storage_identity()),
                ],
                storage: [
                    storage(&owner.records),
                    storage(&owner.xgmi_records),
                    storage(&owner.persistent_window_slots),
                    storage(&owner.persistent_window_records),
                ],
                generations: owner.generations.to_vec(),
                uncertain: owner.uncertain_xgmi_ticket,
                doorbell: owner.doorbell.as_ref().map(observe_local_doorbell),
            })
            .collect(),
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CreationAttemptObservation {
    Opaque,
    Prepared {
        key: QueueKeyV1,
        engine: Option<u32>,
        identities: [SharedGttAllocationIdentityV1; 3],
        storage: Box<[StorageObservation; 4]>,
        native: Gfx942SdmaQueueCreationNativeObservationV1,
        doorbell: Option<LocalDoorbellObservation>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CreationObservation {
    pub(crate) primary: CreationRosterObservation,
    pub(crate) secondary: Option<CreationRosterObservation>,
    pub(crate) attempted: Option<CreationAttemptObservation>,
    logical_mux: Option<(u8, u8)>,
}

pub(super) fn logical_mux_metadata(set: &Gfx942SdmaQueueSetV1) -> Option<(u8, u8)> {
    match set {
        Gfx942SdmaQueueSetV1::LogicalMuxV2 {
            logical_lane_count,
            next_logical_lane,
            ..
        } => Some((*logical_lane_count, *next_logical_lane)),
        _ => None,
    }
}

pub(crate) fn creation_observation(set: &Gfx942SdmaQueueSetV1) -> CreationObservation {
    let (primary, secondary, attempted) = match set {
        Gfx942SdmaQueueSetV1::Generic(owners)
        | Gfx942SdmaQueueSetV1::Directional(owners)
        | Gfx942SdmaQueueSetV1::Striped { owners, .. }
        | Gfx942SdmaQueueSetV1::LogicalMuxV2 { owners, .. } => (owners, None, None),
        Gfx942SdmaQueueSetV1::TerminalRetained {
            primary,
            secondary,
            attempted,
        } => (primary, Some(secondary), attempted.as_ref()),
    };
    CreationObservation {
        logical_mux: logical_mux_metadata(set),
        primary: roster(primary),
        secondary: secondary.map(roster),
        attempted: attempted.map(|attempt| match attempt {
            TerminalGfx942SdmaQueueCreationV1::OpaqueAfterFirstMemoryOperation => {
                CreationAttemptObservation::Opaque
            }
            TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
                prepared,
                native,
                doorbell,
            } => CreationAttemptObservation::Prepared {
                key: prepared.owner,
                engine: prepared.engine_index,
                identities: [
                    PreparationMemoryFixtureV1::primary_token_identity(&prepared.ring),
                    PreparationMemoryFixtureV1::primary_token_identity(&prepared.control),
                    prepared.completions.storage_identity(),
                ],
                storage: Box::new([
                    storage(&prepared.records),
                    storage(&prepared.xgmi_records),
                    storage(&prepared.persistent_window_slots),
                    storage(&prepared.persistent_window_records),
                ]),
                native: *native,
                doorbell: doorbell.as_ref().map(observe_local_doorbell),
            },
        }),
    }
}

pub(crate) fn creation_failure(
    retained: Option<Gfx942SdmaQueueSetV1>,
    terminal: bool,
) -> Gfx942SdmaQueueSetCreationFailureV1 {
    Gfx942SdmaQueueSetCreationFailureV1 {
        error: Gfx942SdmaErrorV1::Contract("injected creation failure"),
        disposition: if terminal {
            Gfx942SdmaQueueSetCreationDispositionV1::Terminal
        } else {
            Gfx942SdmaQueueSetCreationDispositionV1::Retryable
        },
        retained,
    }
}

pub(crate) fn add_creation_attempt(set: &mut Gfx942SdmaQueueSetV1, kind: u8) {
    let Gfx942SdmaQueueSetV1::TerminalRetained {
        primary, attempted, ..
    } = set
    else {
        panic!("terminal fixture")
    };
    *attempted = Some(if kind == 0 {
        TerminalGfx942SdmaQueueCreationV1::OpaqueAfterFirstMemoryOperation
    } else {
        let mut owner = primary.pop().unwrap();
        TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
            prepared: PreparedGfx942SdmaQueueV1 {
                owner: owner.owner,
                engine_index: owner.engine_index,
                ring: owner.ring.take().unwrap(),
                control: owner.control.take().unwrap(),
                completions: owner.completions.take().unwrap(),
                records: owner.records,
                xgmi_records: owner.xgmi_records,
                persistent_window_slots: owner.persistent_window_slots,
                persistent_window_records: owner.persistent_window_records,
            },
            native: if kind == 1 {
                Gfx942SdmaQueueCreationNativeObservationV1::ValidatedQueueId(owner.queue_id)
            } else {
                Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs {
                    queue_id: owner.queue_id,
                    doorbell_offset: 4096,
                }
            },
            doorbell: owner.doorbell.take(),
        }
    });
}

pub(crate) fn creation_profile(
    memory: &mut PreparationMemoryFixtureV1,
    key: QueueKeyV1,
    profile: u8,
) -> Gfx942SdmaQueueSetV1 {
    match profile {
        0 => Gfx942SdmaQueueSetV1::Generic(creation_owners(memory, key, 100, 1)),
        1 => directional_with_ids(memory, key, 100),
        2 => Gfx942SdmaQueueSetV1::Striped {
            owners: creation_owners(memory, key, 100, 4),
            next_owner: 0,
        },
        3 => Gfx942SdmaQueueSetV1::LogicalMuxV2 {
            owners: creation_owners(memory, key, 100, 2),
            logical_lane_count: 4,
            next_logical_lane: 0,
        },
        4 => {
            let mut owners = creation_owners(memory, key, 100, 1);
            owners[0].engine_index = Some(1);
            Gfx942SdmaQueueSetV1::Generic(owners)
        }
        _ => unreachable!(),
    }
}

pub(crate) fn cleanup_creation_set(set: &mut Gfx942SdmaQueueSetV1) {
    fn cleanup(owners: &mut [Gfx942SdmaQueueOwnerV1]) {
        for owner in owners {
            if let Some(doorbell) = &mut owner.doorbell {
                cleanup_local_doorbell(doorbell);
            }
        }
    }
    match set {
        Gfx942SdmaQueueSetV1::Generic(owners)
        | Gfx942SdmaQueueSetV1::Directional(owners)
        | Gfx942SdmaQueueSetV1::Striped { owners, .. }
        | Gfx942SdmaQueueSetV1::LogicalMuxV2 { owners, .. } => cleanup(owners),
        Gfx942SdmaQueueSetV1::TerminalRetained {
            primary,
            secondary,
            attempted,
        } => {
            cleanup(primary);
            cleanup(secondary);
            if let Some(TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
                doorbell: Some(doorbell),
                ..
            }) = attempted
            {
                cleanup_local_doorbell(doorbell);
            }
        }
    }
}
