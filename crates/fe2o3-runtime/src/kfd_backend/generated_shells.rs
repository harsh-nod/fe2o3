//! Nonexecuting generated registration in the existing handle namespace.

use super::*;
use crate::generated_source::{GeneratedHostRosterV1, RuntimeGfx942GeneratedSourceMutV1};
use crate::{RuntimeAllocationIdV1, RuntimeDeviceIdV1, RuntimeStreamIdV1};
use allocation_table::GeneratedAllocationV1;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedShellBindingV1 {
    pub context_generation: u64,
    pub device: RuntimeDeviceIdV1,
    pub stream: RuntimeStreamIdV1,
    pub hold: u64,
    pub backend_device: u64,
    pub backend_stream: u64,
    pub native_device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedShellMemberV1 {
    pub logical: RuntimeAllocationIdV1,
    pub backend: u64,
    pub description: GeneratedAllocationV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedShellPlanV1 {
    pub binding: GeneratedShellBindingV1,
    pub key: u64,
    pub count: usize,
    pub members: [Option<GeneratedShellMemberV1>; GFX942_MAX_FIXED_DISPATCH_DATA_V1],
    next_handle: u64,
}

pub(super) struct GeneratedShellRecordV1 {
    plan: GeneratedShellPlanV1,
    source_identity: Arc<()>,
    // This state owns only inert control. Native construction requires a distinct
    // rooted phase and must disable metadata-only disposal before its first effect.
    control: Option<Gfx942FixedDispatchPacketV1>,
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn prepare_generated_shells_v1(
        &mut self,
        binding: GeneratedShellBindingV1,
        roster: &GeneratedHostRosterV1,
        logical: &[RuntimeAllocationIdV1],
    ) -> Result<GeneratedShellPlanV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        self.require_device(binding.backend_device)?;
        if self.streams.get(&binding.backend_stream) != Some(&binding.backend_device)
            || self
                .generated_shells
                .values()
                .any(|record| record.plan.binding.backend_stream == binding.backend_stream)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "generated stream is unavailable",
            ));
        }
        if self.next_handle == 0
            || roster.count == 0
            || roster.count > GFX942_MAX_FIXED_DISPATCH_DATA_V1
            || logical.len() != roster.count
            || self.generated_shells.len() >= MAX_RUNTIME_STREAMS_V1
            || self
                .allocations
                .len()
                .checked_add(roster.count)
                .is_none_or(|n| n > crate::MAX_RUNTIME_ALLOCATIONS_V1)
        {
            return Err(Self::capacity("generated shell roster capacity"));
        }
        let next_handle = self
            .next_handle
            .checked_add(1 + roster.count as u64)
            .ok_or_else(|| Self::capacity("generated shell handle exhaustion"))?;
        let mut plan = GeneratedShellPlanV1 {
            binding,
            key: self.next_handle,
            count: roster.count,
            members: [None; GFX942_MAX_FIXED_DISPATCH_DATA_V1],
            next_handle,
        };
        if self.generated_shells.contains_key(&plan.key) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated owner handle collision",
            ));
        }
        for (ordinal, &logical) in logical.iter().enumerate() {
            let Some(slot) = roster.buffers[ordinal] else {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "missing generated ordinal",
                ));
            };
            if slot.ordinal != ordinal
                || slot.bytes == 0
                || usize::try_from(slot.bytes).is_err()
                || plan.members[..ordinal]
                    .iter()
                    .flatten()
                    .any(|member| member.logical == logical)
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "invalid generated shell descriptor",
                ));
            }
            let backend = plan.key + 1 + ordinal as u64;
            if self.allocations.contains_key(&backend) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "generated allocation handle collision",
                ));
            }
            plan.members[ordinal] = Some(GeneratedShellMemberV1 {
                logical,
                backend,
                description: GeneratedAllocationV1 {
                    device: binding.backend_device,
                    kind: RuntimeMemoryKindV1::HostVisible,
                    alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
                    byte_len: slot.bytes,
                    adoption: plan.key,
                    ordinal,
                },
            });
        }
        if roster.buffers[roster.count..].iter().any(Option::is_some) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "extra generated ordinal",
            ));
        }
        self.allocations
            .try_reserve(roster.count)
            .map_err(|_| Self::capacity("generated allocation-table growth"))?;
        self.generated_shells
            .try_reserve(1)
            .map_err(|_| Self::capacity("generated owner-table growth"))?;
        Ok(plan)
    }

    pub(crate) fn commit_generated_shells_v1<E>(
        &mut self,
        plan: GeneratedShellPlanV1,
        source: &mut RuntimeGfx942GeneratedSourceMutV1<'_, E>,
        roster: &GeneratedHostRosterV1,
    ) {
        assert_eq!(self.next_handle, plan.key, "preflighted handle range");
        assert!(
            source.matches_roster(roster),
            "same reserved source with available control"
        );
        assert!(!self.generated_shells.contains_key(&plan.key));
        for member in plan.members[..plan.count].iter().flatten() {
            assert!(!self.allocations.contains_key(&member.backend));
        }
        self.generated_shells.insert(
            plan.key,
            GeneratedShellRecordV1 {
                plan,
                source_identity: Arc::clone(&roster.source_identity),
                control: None,
            },
        );
        self.next_handle = plan.next_handle;
        for member in plan.members[..plan.count].iter().flatten() {
            self.allocations
                .insert_generated(member.backend, member.description);
        }
        let record = self
            .generated_shells
            .get_mut(&plan.key)
            .expect("rooted shell owner");
        assert!(Arc::ptr_eq(
            &record.source_identity,
            &roster.source_identity
        ));
        assert!(
            source.transfer_control_into(&mut record.control),
            "one closed control transfer"
        );
    }

    pub(crate) fn generated_shell_plan_v1(
        &self,
        key: u64,
        generation: u64,
        stream: RuntimeStreamIdV1,
        hold: u64,
    ) -> Option<GeneratedShellPlanV1> {
        self.generated_shells
            .get(&key)
            .filter(|record| {
                record.plan.binding.context_generation == generation
                    && record.plan.binding.stream == stream
                    && record.plan.binding.hold == hold
                    && record.control.is_some()
            })
            .map(|record| record.plan)
    }

    pub(crate) fn validate_generated_shell_disposal_v1(&self, plan: &GeneratedShellPlanV1) -> bool {
        plan.key != 0
            && plan.count > 0
            && plan.count <= GFX942_MAX_FIXED_DISPATCH_DATA_V1
            && plan.key.checked_add(1 + plan.count as u64) == Some(plan.next_handle)
            && plan.members[plan.count..].iter().all(Option::is_none)
            && self
                .generated_shells
                .get(&plan.key)
                .is_some_and(|record| record.plan == *plan && record.control.is_some())
            && self.allocations.generated_count_for_adoption(plan.key) == plan.count
            && plan.members[..plan.count]
                .iter()
                .enumerate()
                .all(|(ordinal, member)| {
                    member.is_some_and(|member| {
                        member.backend == plan.key + 1 + ordinal as u64
                            && member.description.ordinal == ordinal
                            && member.description.adoption == plan.key
                            && member.description.device == plan.binding.backend_device
                            && member.description.kind == RuntimeMemoryKindV1::HostVisible
                            && member.description.alignment == HOST_VISIBLE_MEMORY_PAGE_BYTES_V1
                            && member.description.byte_len != 0
                            && usize::try_from(member.description.byte_len).is_ok()
                            && plan.members[..ordinal]
                                .iter()
                                .flatten()
                                .all(|prior| prior.logical != member.logical)
                            && self.allocations.generated(member.backend)
                                == Some(member.description)
                    })
                })
    }

    pub(crate) fn dispose_generated_shells_v1(&mut self, plan: &GeneratedShellPlanV1) {
        assert!(
            self.validate_generated_shell_disposal_v1(plan),
            "complete exact shell-only disposal"
        );
        // Control is inert and closed: no native owner or callback can hide in it.
        drop(self.generated_shells.remove(&plan.key));
        for member in plan.members[..plan.count].iter().flatten() {
            assert!(
                self.allocations
                    .remove_generated(member.backend, member.description)
            );
        }
    }
}
