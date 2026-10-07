//! Creation state borrowed across unwind boundaries, never cleanup authority.

use super::*;

pub(super) trait SdmaNativeCreationV1 {
    fn create(&mut self, actual: &mut KfdIoctlCreateQueueArgs) -> Result<(), rustix::io::Errno>;
    fn doorbell(
        &mut self,
        outputs: fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs,
    ) -> Result<LinuxDoorbellSliceV1, ()>;
    fn currentness(&mut self) -> Result<(), Gfx942SdmaErrorV1>;
}

impl SdmaNativeCreationV1 for SharedGttMemorySessionV1 {
    fn create(&mut self, actual: &mut KfdIoctlCreateQueueArgs) -> Result<(), rustix::io::Errno> {
        create_queue(self.kfd_fd(), actual)
    }
    fn doorbell(
        &mut self,
        outputs: fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs,
    ) -> Result<LinuxDoorbellSliceV1, ()> {
        LinuxDoorbellSliceV1::map(self.kfd_fd(), outputs, self.opener_pid()).map_err(|_| ())
    }
    fn currentness(&mut self) -> Result<(), Gfx942SdmaErrorV1> {
        self.check_queue_currentness().map_err(Into::into)
    }
}

// The ioctl mutates the escrow's actual arguments, not a stack-local copy.
// Neither errors nor unwinds consume the borrowed attempt.
pub(super) fn finish_native_attempt(
    memory: &mut impl SdmaNativeCreationV1,
    attempted: &mut Option<TerminalGfx942SdmaQueueCreationV1>,
    expected: KfdIoctlCreateQueueArgs,
    doorbell_failure: String,
) -> Result<Gfx942SdmaQueueOwnerV1, Gfx942SdmaErrorV1> {
    let Some(TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
        native, doorbell, ..
    }) = attempted
    else {
        std::process::abort()
    };
    let Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs { actual } = native
    else {
        std::process::abort()
    };
    memory
        .create(actual)
        .map_err(|_| Gfx942SdmaErrorV1::QueueCreationIndeterminate)?;
    let mut immutable = *actual;
    immutable.queue_id = u32::MAX;
    immutable.doorbell_offset = u64::MAX;
    if immutable != expected {
        return Err(Gfx942SdmaErrorV1::Contract(
            "kernel changed immutable SDMA CREATE_QUEUE inputs",
        ));
    }
    let outputs = admit_kfd_gfx942_create_queue_outputs(
        actual.queue_id,
        actual.doorbell_offset,
        expected.gpu_id,
    )
    .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA CREATE_QUEUE outputs"))?;
    let queue_id = outputs.queue_id().value();
    *native = Gfx942SdmaQueueCreationNativeObservationV1::ValidatedQueueId(queue_id);
    *doorbell = Some(
        memory
            .doorbell(outputs)
            .map_err(|_| Gfx942SdmaErrorV1::Doorbell(doorbell_failure))?,
    );
    memory.currentness()?;
    let Some(TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
        prepared,
        doorbell: Some(doorbell),
        ..
    }) = attempted.take()
    else {
        std::process::abort()
    };
    Ok(prepared.into_live(queue_id, doorbell))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SdmaCreationProfileV1 {
    Generic {
        engine_index: Option<u32>,
    },
    Directional,
    Striped {
        queue_count: u32,
        next_owner: usize,
    },
    LogicalMux {
        logical_lane_count: u8,
        next_logical_lane: u8,
    },
}

impl SdmaCreationProfileV1 {
    fn owner_count(self) -> usize {
        match self {
            Self::Generic { .. } => 1,
            Self::Directional | Self::LogicalMux { .. } => 2,
            Self::Striped { queue_count, .. } => queue_count as usize,
        }
    }

    fn targeted(self) -> bool {
        !matches!(self, Self::Generic { engine_index: None })
    }

    fn engine(self, index: usize) -> Option<u32> {
        match self {
            Self::Generic { engine_index } => engine_index,
            _ => Some(index as u32 % KFD_GFX942_SDMA_ENGINE_COUNT_V1),
        }
    }

    fn valid_initial(self) -> bool {
        match self {
            Self::Generic { engine_index } => engine_index.is_none_or(|index| index < 2),
            Self::Directional => true,
            Self::Striped {
                queue_count,
                next_owner,
            } => striped_sdma_queue_count_is_admitted(queue_count) && next_owner == 0,
            Self::LogicalMux {
                logical_lane_count,
                next_logical_lane,
            } => {
                gfx942_sdma_logical_mux_lane_count_is_admitted_v2(u32::from(logical_lane_count))
                    && next_logical_lane == 0
            }
        }
    }

    fn into_set(self, owners: Vec<Gfx942SdmaQueueOwnerV1>) -> Gfx942SdmaQueueSetV1 {
        if owners.len() != self.owner_count() {
            core::mem::forget(owners);
            std::process::abort();
        }
        match self {
            Self::Generic { .. } => Gfx942SdmaQueueSetV1::Generic(owners),
            Self::Directional => Gfx942SdmaQueueSetV1::Directional(owners),
            Self::Striped { next_owner, .. } => {
                Gfx942SdmaQueueSetV1::Striped { owners, next_owner }
            }
            Self::LogicalMux {
                logical_lane_count,
                next_logical_lane,
            } => Gfx942SdmaQueueSetV1::LogicalMuxV2 {
                owners,
                logical_lane_count,
                next_logical_lane,
            },
        }
    }
}

#[derive(Default)]
pub(crate) struct SdmaCreationEscrowV1 {
    pub(super) retained: Option<Gfx942SdmaQueueSetV1>,
}

impl SdmaCreationEscrowV1 {
    #[allow(clippy::result_large_err)]
    pub(super) fn begin(
        &mut self,
        primary_profile: SdmaCreationProfileV1,
        secondary_profile: Option<SdmaCreationProfileV1>,
    ) -> Result<(), Gfx942SdmaQueueSetCreationFailureV1> {
        if self.retained.is_some() {
            std::process::abort();
        }
        let mut primary = Vec::new();
        let mut secondary = Vec::new();
        primary
            .try_reserve_exact(primary_profile.owner_count())
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "SDMA primary creation roster allocation",
                ))
            })?;
        secondary
            .try_reserve_exact(secondary_profile.map_or(0, |p| p.owner_count()))
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "SDMA secondary creation roster allocation",
                ))
            })?;
        self.retained = Some(Gfx942SdmaQueueSetV1::TerminalRetained {
            primary_profile,
            secondary_profile,
            primary,
            secondary,
            attempted: None,
        });
        Ok(())
    }

    pub(super) fn attempted(&mut self) -> &mut Option<TerminalGfx942SdmaQueueCreationV1> {
        let Some(Gfx942SdmaQueueSetV1::TerminalRetained { attempted, .. }) = &mut self.retained
        else {
            std::process::abort()
        };
        attempted
    }

    pub(super) fn push(&mut self, owner: Gfx942SdmaQueueOwnerV1, secondary_group: bool) {
        let Some(Gfx942SdmaQueueSetV1::TerminalRetained {
            primary_profile,
            secondary_profile,
            primary,
            secondary,
            attempted,
        }) = &mut self.retained
        else {
            core::mem::forget(owner);
            std::process::abort();
        };
        let (roster, count) = if secondary_group {
            (secondary, secondary_profile.map_or(0, |p| p.owner_count()))
        } else {
            (primary, primary_profile.owner_count())
        };
        if attempted.is_some() || roster.len() >= count || roster.len() >= roster.capacity() {
            core::mem::forget(owner);
            std::process::abort();
        }
        roster.push(owner);
    }

    pub(crate) fn take_terminal(&mut self) -> Option<Gfx942SdmaQueueSetV1> {
        let retained = self.retained.take()?;
        if let Gfx942SdmaQueueSetV1::TerminalRetained {
            primary,
            secondary,
            attempted,
            ..
        } = &retained
            && primary.is_empty()
            && secondary.is_empty()
            && attempted.is_none()
        {
            return None;
        }
        Some(retained)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.retained.is_none()
    }

    fn failure(&mut self, error: Gfx942SdmaErrorV1) -> Gfx942SdmaQueueSetCreationFailureV1 {
        let retained = self.take_terminal();
        // The caller's still-armed creation scope poisons global admission;
        // the facade installs this custody before poisoning the parent.
        Gfx942SdmaQueueSetCreationFailureV1 {
            error,
            disposition: Gfx942SdmaQueueSetCreationDispositionV1::Terminal,
            retained,
        }
    }

    pub(super) fn owner_failure(
        &mut self,
        failure: Gfx942SdmaQueueOwnerCreationFailureV1,
    ) -> Gfx942SdmaQueueSetCreationFailureV1 {
        let error = match failure {
            Gfx942SdmaQueueOwnerCreationFailureV1::RetryableBeforeCreate(error) => error,
            Gfx942SdmaQueueOwnerCreationFailureV1::TerminalAfterMemoryOperation {
                error,
                retained,
            } => {
                if self.attempted().is_some() {
                    core::mem::forget(retained);
                    std::process::abort();
                }
                *self.attempted() = Some(retained);
                error
            }
        };
        self.failure(error)
    }

    pub(super) fn finish(&mut self) -> (Gfx942SdmaQueueSetV1, Option<Gfx942SdmaQueueSetV1>) {
        let Some(Gfx942SdmaQueueSetV1::TerminalRetained {
            primary_profile,
            secondary_profile,
            primary,
            secondary,
            attempted: None,
        }) = self.retained.take()
        else {
            std::process::abort()
        };
        let secondary = match secondary_profile {
            Some(profile) => Some(profile.into_set(secondary)),
            None if secondary.is_empty() => None,
            None => {
                core::mem::forget(primary);
                core::mem::forget(secondary);
                std::process::abort()
            }
        };
        (primary_profile.into_set(primary), secondary)
    }
}

#[allow(clippy::result_large_err)]
pub(super) fn create_set(
    memory: &mut SharedGttMemorySessionV1,
    key: QueueKeyV1,
    reserved: &[u32],
    primary_profile: SdmaCreationProfileV1,
    secondary_profile: Option<SdmaCreationProfileV1>,
    escrow: &mut SdmaCreationEscrowV1,
) -> Result<
    (
        Gfx942SdmaQueueSetV1,
        Option<Gfx942SdmaQueueSetV1>,
        Vec<Gfx942SdmaQueueObservationV1>,
    ),
    Gfx942SdmaQueueSetCreationFailureV1,
> {
    let valid_secondary = secondary_profile.is_none_or(|profile| {
        primary_profile == SdmaCreationProfileV1::Directional
            && matches!(profile, SdmaCreationProfileV1::Striped { queue_count, next_owner: 0 }
                if combined_striped_sdma_queue_count_is_admitted(queue_count))
    });
    if !primary_profile.valid_initial() || !valid_secondary || queue_ids_have_duplicates(reserved) {
        return Err(retryable_sdma_queue_set_creation_failure(
            Gfx942SdmaErrorV1::Contract("SDMA creation profile or reserved IDs"),
        ));
    }
    let targeted = primary_profile.targeted();
    if targeted
        && memory.gfx942_sdma_engine_inventory()
            != (
                Some(KFD_GFX942_SDMA_ENGINE_COUNT_V1),
                Some(KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1),
            )
    {
        return Err(retryable_sdma_queue_set_creation_failure(
            Gfx942SdmaErrorV1::Contract("SDMA engine inventory is not the exact gfx942 profile"),
        ));
    }
    let total = primary_profile.owner_count() + secondary_profile.map_or(0, |p| p.owner_count());
    let hosts = prepare_sdma_queue_host_resource_roster(total)
        .map_err(recover_sdma_owner_preflight_error)
        .map_err(retryable_sdma_queue_set_creation_failure)?;
    let mut observations = Vec::new();
    observations.try_reserve_exact(total).map_err(|_| {
        retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
            "SDMA creation observation allocation",
        ))
    })?;
    escrow.begin(primary_profile, secondary_profile)?;
    let creation_arm = match arm_process_global_kfd_runtime_gate_for_creation_v1() {
        Ok(arm) => arm,
        Err(_) => {
            let _ = escrow.take_terminal();
            return Err(retryable_sdma_queue_set_creation_failure(
                Gfx942SdmaErrorV1::Contract("process-global KFD creation gate unavailable"),
            ));
        }
    };
    *escrow.attempted() = Some(TerminalGfx942SdmaQueueCreationV1::OpaqueAfterFirstMemoryOperation);
    if targeted && let Err(error) = memory.check_gfx942_sdma_topology_capability_currentness() {
        return Err(escrow.failure(error.into()));
    }
    for (index, host) in hosts.into_iter().enumerate() {
        let secondary = index >= primary_profile.owner_count();
        let (profile, slot) = if secondary {
            (
                secondary_profile.unwrap_or_else(|| std::process::abort()),
                index - primary_profile.owner_count(),
            )
        } else {
            (primary_profile, index)
        };
        let engine = profile.engine(slot).map(|index| {
            Gfx942SdmaEngineProfileV1::Ordinary(
                admit_kfd_gfx942_sdma_engine_id(index).unwrap_or_else(|_| std::process::abort()),
            )
        });
        let created = match Gfx942SdmaQueueOwnerV1::create_with_engine_in_armed_scope(
            memory,
            key,
            engine,
            host,
            &creation_arm,
            escrow.attempted(),
        ) {
            Ok(created) => created,
            Err(failure) => return Err(escrow.owner_failure(failure)),
        };
        let observation = created.observation();
        escrow.push(created, secondary);
        let duplicate = reserved.contains(&observation.queue_id)
            || observations
                .iter()
                .any(|other: &Gfx942SdmaQueueObservationV1| other.queue_id == observation.queue_id);
        observations.push(observation);
        if duplicate {
            return Err(escrow.failure(Gfx942SdmaErrorV1::Contract(
                "SDMA queue ID is not session-wide unique",
            )));
        }
    }
    if targeted && let Err(error) = memory.check_gfx942_sdma_topology_capability_currentness() {
        return Err(escrow.failure(error.into()));
    }
    creation_arm.disarm();
    let (primary, secondary) = escrow.finish();
    Ok((primary, secondary, observations))
}
