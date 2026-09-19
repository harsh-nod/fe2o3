//! Borrowed SDMA teardown, rooted before any destructive operation.

#![forbid(unsafe_code)]

use super::owner_release::OwnerProgressV1;
pub(crate) use super::owner_release::SdmaOwnerReleaseMemoryV1;
use super::*;
#[cfg(test)]
use crate::queue_linux::LinuxDoorbellReleaseProgressV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[cfg(test)]
pub(crate) mod fixture;

pub(crate) trait SdmaReleaseMemoryV1: SdmaOwnerReleaseMemoryV1 {
    fn sdma_release_topology(&mut self) -> Result<(), MemorySessionError>;
    fn sdma_release_poison(&mut self);
}

impl SdmaReleaseMemoryV1 for SharedGttMemorySessionV1 {
    fn sdma_release_topology(&mut self) -> Result<(), MemorySessionError> {
        self.check_gfx942_sdma_topology_capability_currentness()
    }
    fn sdma_release_poison(&mut self) {
        let _ = self.quarantine_queue_composition("terminal retained SDMA release");
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetainedSdmaReleaseProfileV1 {
    Generic { targeted: bool },
    Directional,
    Striped { owner_count: usize },
    LogicalMuxV2,
}

impl RetainedSdmaReleaseProfileV1 {
    fn owner_count(self) -> usize {
        match self {
            Self::Generic { .. } => 1,
            Self::Directional => 2,
            Self::Striped { owner_count } => owner_count,
            Self::LogicalMuxV2 => GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2,
        }
    }

    fn destroy_indices(self) -> impl Iterator<Item = usize> {
        (0..self.owner_count()).map(move |index| match self {
            Self::Directional => 1 - index,
            _ => index,
        })
    }

    fn resource_indices(self) -> impl Iterator<Item = usize> {
        (0..self.owner_count()).rev()
    }

    fn requires_topology(self) -> bool {
        !matches!(self, Self::Generic { targeted: false })
    }
}

pub(crate) struct RetainedSdmaReleaseCustodyV1 {
    set: Gfx942SdmaQueueSetV1,
    owner: QueueKeyV1,
    primary_id: u32,
    profile: Option<RetainedSdmaReleaseProfileV1>,
    progress: [OwnerProgressV1; GFX942_SDMA_MAX_STRIPED_QUEUES_V1],
    started: bool,
    resources_started: bool,
    failed: bool,
    destroyed: usize,
    released: usize,
}

pub(crate) fn supports_retained_sdma_composition_v1(
    sdma: Option<&Gfx942SdmaQueueSetV1>,
    striped: Option<&Gfx942SdmaQueueSetV1>,
) -> Result<bool, Gfx942SdmaErrorV1> {
    let Some(striped) = striped else {
        return sdma.map_or(
            Ok(true),
            Gfx942SdmaQueueSetV1::supports_retained_sdma_release_v1,
        );
    };
    let Some(sdma) = sdma else {
        return Err(Gfx942SdmaErrorV1::Contract("orphan secondary SDMA set"));
    };
    match (
        sdma.retained_release_profile_v1()?,
        striped.retained_release_profile_v1()?,
    ) {
        (
            Some(RetainedSdmaReleaseProfileV1::Directional),
            Some(RetainedSdmaReleaseProfileV1::Striped { owner_count }),
        ) if combined_striped_sdma_queue_count_is_admitted(owner_count as u32) => Ok(true),
        _ => Err(Gfx942SdmaErrorV1::Contract("combined SDMA owner roster")),
    }
}

pub(crate) fn preflight_retained_sdma_composition_v1(
    sdma: Option<&Gfx942SdmaQueueSetV1>,
    striped: Option<&Gfx942SdmaQueueSetV1>,
    key: QueueKeyV1,
    primary_id: u32,
) -> Result<(), Gfx942SdmaErrorV1> {
    if !supports_retained_sdma_composition_v1(sdma, striped)? {
        return Err(Gfx942SdmaErrorV1::Contract("retained SDMA release profile"));
    }
    for set in [sdma, striped].into_iter().flatten() {
        set.preflight_retained_sdma_release_v1(key, primary_id)?;
    }
    if let (
        Some(Gfx942SdmaQueueSetV1::Directional(directional)),
        Some(Gfx942SdmaQueueSetV1::Striped { owners, .. }),
    ) = (sdma, striped)
        && owners.iter().any(|owner| {
            directional
                .iter()
                .any(|other| other.queue_id == owner.queue_id)
        })
    {
        return Err(Gfx942SdmaErrorV1::Contract("combined SDMA queue identity"));
    }
    Ok(())
}

impl Gfx942SdmaQueueSetV1 {
    fn retained_release_profile_v1(
        &self,
    ) -> Result<Option<RetainedSdmaReleaseProfileV1>, Gfx942SdmaErrorV1> {
        match self {
            Self::Generic(owners) => match owners.as_slice() {
                [owner] if matches!(owner.engine_index, None | Some(0 | 1)) => {
                    Ok(Some(RetainedSdmaReleaseProfileV1::Generic {
                        targeted: owner.engine_index.is_some(),
                    }))
                }
                _ => Err(Gfx942SdmaErrorV1::Contract("generic SDMA owner roster")),
            },
            Self::Directional(owners) if owners.len() == 2 => {
                Ok(Some(RetainedSdmaReleaseProfileV1::Directional))
            }
            Self::Directional(_) => {
                Err(Gfx942SdmaErrorV1::Contract("directional SDMA owner roster"))
            }
            Self::Striped { owners, next_owner }
                if owners.len() <= GFX942_SDMA_MAX_STRIPED_QUEUES_V1
                    && striped_sdma_queue_count_is_admitted(owners.len() as u32)
                    && *next_owner < owners.len() =>
            {
                Ok(Some(RetainedSdmaReleaseProfileV1::Striped {
                    owner_count: owners.len(),
                }))
            }
            Self::Striped { .. } => Err(Gfx942SdmaErrorV1::Contract("striped SDMA owner roster")),
            Self::LogicalMuxV2 {
                owners,
                logical_lane_count,
                next_logical_lane,
            } if owners.len() == GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2
                && gfx942_sdma_logical_mux_lane_count_is_admitted_v2(u32::from(
                    *logical_lane_count,
                ))
                && *next_logical_lane < *logical_lane_count =>
            {
                Ok(Some(RetainedSdmaReleaseProfileV1::LogicalMuxV2))
            }
            Self::LogicalMuxV2 { .. } => {
                Err(Gfx942SdmaErrorV1::Contract("logical mux SDMA owner roster"))
            }
            Self::TerminalRetained { .. } => Err(Gfx942SdmaErrorV1::Contract(
                "terminal retained SDMA queues require process teardown",
            )),
        }
    }

    pub(crate) fn supports_retained_sdma_release_v1(&self) -> Result<bool, Gfx942SdmaErrorV1> {
        Ok(self.retained_release_profile_v1()?.is_some())
    }

    pub(crate) fn preflight_retained_sdma_release_v1(
        &self,
        key: QueueKeyV1,
        primary_id: u32,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Some(profile) = self.retained_release_profile_v1()? else {
            return Err(Gfx942SdmaErrorV1::Contract("retained SDMA release profile"));
        };
        let (Self::Generic(owners)
        | Self::Directional(owners)
        | Self::Striped { owners, .. }
        | Self::LogicalMuxV2 { owners, .. }) = self
        else {
            unreachable!()
        };
        if profile == RetainedSdmaReleaseProfileV1::Directional
            && owners[0].queue_id == owners[1].queue_id
        {
            return Err(Gfx942SdmaErrorV1::Contract("directional SDMA owner roster"));
        }
        if profile == RetainedSdmaReleaseProfileV1::LogicalMuxV2
            && owners[0].queue_id == owners[1].queue_id
        {
            return Err(Gfx942SdmaErrorV1::Contract("logical mux SDMA owner roster"));
        }
        for (index, owner) in owners.iter().enumerate() {
            if matches!(profile, RetainedSdmaReleaseProfileV1::Striped { .. })
                && owners[..index]
                    .iter()
                    .any(|prior| prior.queue_id == owner.queue_id)
            {
                return Err(Gfx942SdmaErrorV1::Contract("striped SDMA owner roster"));
            }
            owner.require_live()?;
            if owner.owner != key
                || owner.queue_id == primary_id
                || (profile == RetainedSdmaReleaseProfileV1::Directional
                    && owner.engine_index != Some(index as u32))
                || (matches!(
                    profile,
                    RetainedSdmaReleaseProfileV1::Striped { .. }
                        | RetainedSdmaReleaseProfileV1::LogicalMuxV2
                ) && owner.engine_index != Some(index as u32 % 2))
                || owner.records.len() != GFX942_SDMA_RING_SLOT_COUNT_V1
                || owner.xgmi_records.len() != GFX942_SDMA_RING_SLOT_COUNT_V1
                || owner.persistent_window_slots.len() != GFX942_SDMA_RING_SLOT_COUNT_V1
                || owner.persistent_window_records.len() != GFX942_SDMA_RING_SLOT_COUNT_V1
                || owner.ring.is_none()
                || owner.control.is_none()
                || owner.completions.is_none()
            {
                return Err(Gfx942SdmaErrorV1::Contract("retained SDMA release owners"));
            }
            if owner.records.iter().any(Option::is_some)
                || owner.xgmi_records.iter().any(Option::is_some)
                || owner.persistent_window_slots.iter().any(Option::is_some)
                || owner.persistent_window_records.iter().any(Option::is_some)
                || owner.uncertain_xgmi_ticket.is_some()
            {
                return Err(Gfx942SdmaErrorV1::Pending);
            }
            owner
                .doorbell
                .as_ref()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA doorbell"))?
                .validate_release_v1()
                .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA doorbell release owner"))?;
        }
        Ok(())
    }
}

impl RetainedSdmaReleaseCustodyV1 {
    pub(crate) fn new(set: Gfx942SdmaQueueSetV1, owner: QueueKeyV1, primary_id: u32) -> Self {
        Self {
            set,
            owner,
            primary_id,
            profile: None,
            progress: std::array::from_fn(|_| OwnerProgressV1::default()),
            started: false,
            resources_started: false,
            failed: false,
            destroyed: 0,
            released: 0,
        }
    }

    pub(crate) fn poison_retained_owners_v1(&mut self) {
        self.failed = true;
        if let Gfx942SdmaQueueSetV1::Generic(owners)
        | Gfx942SdmaQueueSetV1::Directional(owners)
        | Gfx942SdmaQueueSetV1::Striped { owners, .. }
        | Gfx942SdmaQueueSetV1::LogicalMuxV2 { owners, .. } = &mut self.set
        {
            for owner in owners {
                owner.poisoned = true;
            }
        }
    }

    fn settle(
        &mut self,
        memory: &mut impl SdmaReleaseMemoryV1,
        result: std::thread::Result<Result<(), Gfx942SdmaErrorV1>>,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        match result {
            Ok(Ok(())) => Ok(()),
            result => {
                self.poison_retained_owners_v1();
                memory.sdma_release_poison();
                match result {
                    Ok(Err(error)) => Err(error),
                    Err(payload) => resume_unwind(payload),
                    Ok(Ok(())) => unreachable!(),
                }
            }
        }
    }

    pub(crate) fn destroy_in_place(
        &mut self,
        memory: &mut impl SdmaReleaseMemoryV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        if self.started {
            return Err(Gfx942SdmaErrorV1::Contract("SDMA destroy is one-shot"));
        }
        self.started = true;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.set
                .preflight_retained_sdma_release_v1(self.owner, self.primary_id)?;
            let profile = self
                .set
                .retained_release_profile_v1()?
                .expect("preflight profile");
            self.profile = Some(profile);
            if profile.requires_topology() {
                memory.sdma_release_topology()?;
            }
            let (Gfx942SdmaQueueSetV1::Generic(owners)
            | Gfx942SdmaQueueSetV1::Directional(owners)
            | Gfx942SdmaQueueSetV1::Striped { owners, .. }
            | Gfx942SdmaQueueSetV1::LogicalMuxV2 { owners, .. }) = &mut self.set
            else {
                unreachable!()
            };
            for index in profile.destroy_indices() {
                let owner = &mut owners[index];
                let progress = &mut self.progress[index];
                progress.destroy_in_place(owner, memory, |_| {
                    Gfx942SdmaErrorV1::Contract("SDMA doorbell release")
                })?;
                self.destroyed += 1;
            }
            if profile.requires_topology() {
                memory.sdma_release_topology()?;
            }
            Ok(())
        }));
        self.settle(memory, result)
    }

    pub(crate) fn release_resources_in_place(
        &mut self,
        memory: &mut impl SdmaReleaseMemoryV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Some(profile) = self.profile else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "SDMA resources are not releasable",
            ));
        };
        if !self.started
            || self.failed
            || self.destroyed != profile.owner_count()
            || self.resources_started
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "SDMA resources are not releasable",
            ));
        }
        self.resources_started = true;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let (Gfx942SdmaQueueSetV1::Generic(owners)
            | Gfx942SdmaQueueSetV1::Directional(owners)
            | Gfx942SdmaQueueSetV1::Striped { owners, .. }
            | Gfx942SdmaQueueSetV1::LogicalMuxV2 { owners, .. }) = &mut self.set
            else {
                unreachable!()
            };
            for index in profile.resource_indices() {
                let owner = &mut owners[index];
                self.progress[index].release_resources_in_place(owner, memory)?;
                self.released += 1;
            }
            Ok(())
        }));
        self.settle(memory, result)
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.profile.is_some_and(|profile| {
            !self.failed
                && self.destroyed == profile.owner_count()
                && self.released == profile.owner_count()
        })
    }

    pub(crate) fn additional_resource_count(&self) -> u8 {
        self.profile
            .map_or(0, |profile| (3 * profile.owner_count()) as u8)
    }
}
