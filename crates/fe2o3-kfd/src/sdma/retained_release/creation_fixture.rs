//! Borrowed observations and local-only disposal, never native recovery.

use super::*;
use crate::sdma::creation::{SdmaCreationEscrowV1, SdmaCreationProfileV1};

fn creation_args() -> KfdIoctlCreateQueueArgs {
    KfdIoctlCreateQueueArgs::new_sdma(
        KfdSdmaQueueBuffers {
            ring_base_address: 0x1000,
            write_pointer_address: 0x2000,
            read_pointer_address: 0x2080,
        },
        admit_kfd_aql_queue_ring_size(4096).unwrap(),
        0,
        admit_kfd_queue_percentage(100).unwrap(),
        admit_kfd_queue_priority(0).unwrap(),
    )
}
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
pub(crate) struct CreationOwnerObservation {
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
        owners: owners.iter().map(creation_owner_observation).collect(),
    }
}

pub(crate) fn creation_owner_observation(
    owner: &Gfx942SdmaQueueOwnerV1,
) -> CreationOwnerObservation {
    CreationOwnerObservation {
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
        native: Box<Gfx942SdmaQueueCreationNativeObservationV1>,
        doorbell: Option<LocalDoorbellObservation>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CreationObservation {
    pub(crate) primary: CreationRosterObservation,
    pub(crate) secondary: Option<CreationRosterObservation>,
    pub(crate) attempted: Option<CreationAttemptObservation>,
    logical_mux: Option<(u8, u8)>,
    pub(crate) profiles: (SdmaCreationProfileV1, Option<SdmaCreationProfileV1>),
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

fn creation_profiles(
    set: &Gfx942SdmaQueueSetV1,
) -> (SdmaCreationProfileV1, Option<SdmaCreationProfileV1>) {
    let profile = match set {
        Gfx942SdmaQueueSetV1::Generic(owners) => SdmaCreationProfileV1::Generic {
            engine_index: owners.first().and_then(|owner| owner.engine_index),
        },
        Gfx942SdmaQueueSetV1::Directional(_) => SdmaCreationProfileV1::Directional,
        Gfx942SdmaQueueSetV1::Striped { owners, next_owner } => SdmaCreationProfileV1::Striped {
            queue_count: owners.len() as u32,
            next_owner: *next_owner,
        },
        Gfx942SdmaQueueSetV1::LogicalMuxV2 {
            logical_lane_count,
            next_logical_lane,
            ..
        } => SdmaCreationProfileV1::LogicalMux {
            logical_lane_count: *logical_lane_count,
            next_logical_lane: *next_logical_lane,
        },
        Gfx942SdmaQueueSetV1::TerminalRetained {
            primary_profile,
            secondary_profile,
            ..
        } => return (*primary_profile, *secondary_profile),
    };
    (profile, None)
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
            ..
        } => (primary, Some(secondary), attempted.as_ref()),
    };
    CreationObservation {
        logical_mux: logical_mux_metadata(set),
        profiles: creation_profiles(set),
        primary: roster(primary),
        secondary: secondary.map(roster),
        attempted: attempted.map(creation_attempt_observation),
    }
}

pub(crate) fn creation_attempt_observation(
    attempt: &TerminalGfx942SdmaQueueCreationV1,
) -> CreationAttemptObservation {
    match attempt {
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
            native: Box::new(*native),
            doorbell: doorbell.as_ref().map(observe_local_doorbell),
        },
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
                    actual: KfdIoctlCreateQueueArgs {
                        queue_id: owner.queue_id,
                        doorbell_offset: 4096,
                        ..creation_args()
                    },
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

pub(crate) fn escrow_observation(escrow: &SdmaCreationEscrowV1) -> CreationObservation {
    creation_observation(escrow.retained.as_ref().unwrap())
}

pub(crate) fn escrow_prefix(
    memory: &mut PreparationMemoryFixtureV1,
    key: QueueKeyV1,
    escrow: &mut SdmaCreationEscrowV1,
    primary: SdmaCreationProfileV1,
    secondary: Option<SdmaCreationProfileV1>,
    prefix: usize,
) {
    escrow
        .begin(primary, secondary)
        .unwrap_or_else(|_| panic!("fixture reserve"));
    let primary_count = match primary {
        SdmaCreationProfileV1::Generic { .. } => 1,
        SdmaCreationProfileV1::Directional | SdmaCreationProfileV1::LogicalMux { .. } => 2,
        SdmaCreationProfileV1::Striped { queue_count, .. } => queue_count as usize,
    };
    for index in 0..prefix {
        let group = index >= primary_count;
        let profile = if group { secondary.unwrap() } else { primary };
        let slot = if group { index - primary_count } else { index };
        let mut owners = creation_owners(memory, key, 100 + index as u32, 1);
        let mut owner = owners.pop().unwrap();
        owner.engine_index = match profile {
            SdmaCreationProfileV1::Generic { engine_index } => engine_index,
            _ => Some(slot as u32 % 2),
        };
        escrow.push(owner, group);
    }
}

pub(crate) fn escrow_opaque(escrow: &mut SdmaCreationEscrowV1) {
    *escrow.attempted() = Some(TerminalGfx942SdmaQueueCreationV1::OpaqueAfterFirstMemoryOperation);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreationNativeFault {
    Success,
    BeforeIoctlPanic,
    AfterIoctlPanic,
    IoctlError,
    ImmutableMutation,
    InvalidOutput,
    DoorbellPanic,
    DoorbellError,
    CurrentnessPanic,
    CurrentnessError,
}

impl CreationNativeFault {
    pub(crate) fn panics(self) -> bool {
        matches!(
            self,
            Self::BeforeIoctlPanic
                | Self::AfterIoctlPanic
                | Self::DoorbellPanic
                | Self::CurrentnessPanic
        )
    }
}

pub(crate) fn escrow_native_attempt(
    memory: &mut PreparationMemoryFixtureV1,
    key: QueueKeyV1,
    escrow: &mut SdmaCreationEscrowV1,
    fault: CreationNativeFault,
    expected_observation: &mut Option<CreationObservation>,
) -> Result<(), Gfx942SdmaErrorV1> {
    use crate::sdma::creation::{SdmaNativeCreationV1, finish_native_attempt};

    struct Native<'a> {
        fault: CreationNativeFault,
        expected: &'a mut CreationObservation,
        actual_address: usize,
        queue_id: u32,
    }
    impl Native<'_> {
        fn expected_attempt(
            &mut self,
        ) -> (
            &mut Gfx942SdmaQueueCreationNativeObservationV1,
            &mut Option<LocalDoorbellObservation>,
        ) {
            let Some(CreationAttemptObservation::Prepared {
                native, doorbell, ..
            }) = &mut self.expected.attempted
            else {
                panic!("prepared fixture")
            };
            (native.as_mut(), doorbell)
        }
    }
    impl SdmaNativeCreationV1 for Native<'_> {
        fn create(
            &mut self,
            actual: &mut KfdIoctlCreateQueueArgs,
        ) -> Result<(), rustix::io::Errno> {
            assert_eq!(actual as *mut _ as usize, self.actual_address);
            if self.fault == CreationNativeFault::BeforeIoctlPanic {
                std::panic::panic_any(self.fault);
            }
            actual.queue_id = self.queue_id;
            actual.doorbell_offset =
                fe2o3_kfd_uapi::KFD_MMAP_TYPE_DOORBELL << fe2o3_kfd_uapi::KFD_MMAP_TYPE_SHIFT;
            if self.fault == CreationNativeFault::ImmutableMutation {
                actual.ring_size = 8192;
            }
            if self.fault == CreationNativeFault::InvalidOutput {
                actual.queue_id = u32::MAX;
            }
            *self.expected_attempt().0 =
                Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs {
                    actual: *actual,
                };
            if self.fault == CreationNativeFault::AfterIoctlPanic {
                std::panic::panic_any(self.fault);
            }
            if self.fault == CreationNativeFault::IoctlError {
                return Err(rustix::io::Errno::IO);
            }
            Ok(())
        }

        fn doorbell(
            &mut self,
            outputs: fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs,
        ) -> Result<LinuxDoorbellSliceV1, ()> {
            assert_eq!(outputs.queue_id().value(), self.queue_id);
            *self.expected_attempt().0 =
                Gfx942SdmaQueueCreationNativeObservationV1::ValidatedQueueId(self.queue_id);
            if self.fault == CreationNativeFault::DoorbellPanic {
                std::panic::panic_any(self.fault);
            }
            if self.fault == CreationNativeFault::DoorbellError {
                return Err(());
            }
            let doorbell = local_doorbell();
            *self.expected_attempt().1 = Some(observe_local_doorbell(&doorbell));
            Ok(doorbell)
        }

        fn currentness(&mut self) -> Result<(), Gfx942SdmaErrorV1> {
            if self.fault == CreationNativeFault::Success {
                return Ok(());
            }
            if self.fault == CreationNativeFault::CurrentnessPanic {
                std::panic::panic_any(self.fault);
            }
            assert_eq!(self.fault, CreationNativeFault::CurrentnessError);
            Err(Gfx942SdmaErrorV1::Contract("injected creation currentness"))
        }
    }

    let Some(Gfx942SdmaQueueSetV1::TerminalRetained {
        primary_profile,
        secondary_profile,
        primary,
        secondary,
        ..
    }) = &escrow.retained
    else {
        panic!("fixture escrow")
    };
    let primary_count = match primary_profile {
        SdmaCreationProfileV1::Generic { .. } => 1,
        SdmaCreationProfileV1::Directional | SdmaCreationProfileV1::LogicalMux { .. } => 2,
        SdmaCreationProfileV1::Striped { queue_count, .. } => *queue_count as usize,
    };
    let secondary_group = primary.len() == primary_count;
    let (profile, slot) = if secondary_group {
        (secondary_profile.unwrap(), secondary.len())
    } else {
        (*primary_profile, primary.len())
    };
    let engine = match profile {
        SdmaCreationProfileV1::Generic { engine_index } => engine_index,
        _ => Some(slot as u32 % 2),
    };
    let queue_id = 100 + primary.len() as u32 + secondary.len() as u32;
    let mut owner = creation_owners(memory, key, queue_id, 1).pop().unwrap();
    owner.engine_index = engine;
    cleanup_local_doorbell(owner.doorbell.as_mut().unwrap());
    let args = match engine {
        None => creation_args(),
        Some(engine) => KfdIoctlCreateQueueArgs::new_sdma_on_engine(
            KfdSdmaQueueBuffers {
                ring_base_address: 0x1000,
                write_pointer_address: 0x2000,
                read_pointer_address: 0x2080,
            },
            admit_kfd_aql_queue_ring_size(4096).unwrap(),
            0,
            admit_kfd_queue_percentage(100).unwrap(),
            admit_kfd_queue_priority(0).unwrap(),
            admit_kfd_gfx942_sdma_engine_id(engine).unwrap(),
        ),
    };
    *escrow.attempted() = Some(TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
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
        native: Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs { actual: args },
        doorbell: None,
    });
    *expected_observation = Some(escrow_observation(escrow));
    let Some(TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
        native: Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs { actual },
        ..
    }) = escrow.attempted()
    else {
        panic!("attempt fixture")
    };
    let mut native = Native {
        fault,
        expected: expected_observation.as_mut().unwrap(),
        actual_address: actual as *mut _ as usize,
        queue_id,
    };
    let owner = finish_native_attempt(
        &mut native,
        escrow.attempted(),
        args,
        "fixture doorbell failure".into(),
    )?;
    let Some(CreationAttemptObservation::Prepared {
        key,
        engine,
        identities,
        storage: expected_storage,
        doorbell,
        native,
        ..
    }) = &expected_observation.as_ref().unwrap().attempted
    else {
        panic!("prepared observation")
    };
    assert_eq!(
        **native,
        Gfx942SdmaQueueCreationNativeObservationV1::ValidatedQueueId(queue_id)
    );
    assert_eq!(owner.owner, *key);
    assert_eq!(owner.engine_index, *engine);
    assert_eq!(owner.queue_id, queue_id);
    assert_eq!(
        *identities,
        [
            PreparationMemoryFixtureV1::primary_token_identity(owner.ring.as_ref().unwrap()),
            PreparationMemoryFixtureV1::primary_token_identity(owner.control.as_ref().unwrap()),
            owner.completions.as_ref().unwrap().storage_identity(),
        ]
    );
    assert_eq!(
        **expected_storage,
        [
            storage(&owner.records),
            storage(&owner.xgmi_records),
            storage(&owner.persistent_window_slots),
            storage(&owner.persistent_window_records)
        ]
    );
    assert_eq!(
        *doorbell,
        owner.doorbell.as_ref().map(observe_local_doorbell)
    );
    assert!(!owner.poisoned && !owner.destroyed);
    assert!(escrow.attempted().is_none());
    escrow.push(owner, secondary_group);
    *expected_observation = Some(escrow_observation(escrow));
    Ok(())
}

pub(crate) fn escrow_owner_failure(
    escrow: &mut SdmaCreationEscrowV1,
    error: Gfx942SdmaErrorV1,
) -> Gfx942SdmaQueueSetCreationFailureV1 {
    let retained = escrow.attempted().take().unwrap();
    escrow.owner_failure(
        Gfx942SdmaQueueOwnerCreationFailureV1::TerminalAfterMemoryOperation { error, retained },
    )
}

pub(crate) fn finish_creation_escrow(
    escrow: &mut SdmaCreationEscrowV1,
) -> (Gfx942SdmaQueueSetV1, Option<Gfx942SdmaQueueSetV1>) {
    escrow.finish()
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
            ..
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

pub(crate) fn assert_creation_owner_matches_attempt(
    owner: &Gfx942SdmaQueueOwnerV1,
    expected: &CreationAttemptObservation,
    queue_id: u32,
) {
    let CreationAttemptObservation::Prepared {
        key,
        engine,
        identities,
        storage: expected_storage,
        native,
        doorbell,
    } = expected
    else {
        panic!("prepared attempt required")
    };
    assert_eq!(
        **native,
        Gfx942SdmaQueueCreationNativeObservationV1::ValidatedQueueId(queue_id)
    );
    assert_eq!(owner.owner, *key);
    assert_eq!(owner.engine_index, *engine);
    assert_eq!(owner.queue_id, queue_id);
    assert!(!owner.destroyed && !owner.poisoned);
    assert_eq!(owner.generations, [0; GFX942_SDMA_RING_SLOT_COUNT_V1]);
    assert!(owner.uncertain_xgmi_ticket.is_none());
    assert_eq!(
        *identities,
        [
            PreparationMemoryFixtureV1::primary_token_identity(owner.ring.as_ref().unwrap()),
            PreparationMemoryFixtureV1::primary_token_identity(owner.control.as_ref().unwrap()),
            owner.completions.as_ref().unwrap().storage_identity(),
        ]
    );
    assert_eq!(
        **expected_storage,
        [
            storage(&owner.records),
            storage(&owner.xgmi_records),
            storage(&owner.persistent_window_slots),
            storage(&owner.persistent_window_records)
        ]
    );
    assert_eq!(
        *doorbell,
        owner.doorbell.as_ref().map(observe_local_doorbell)
    );
}
