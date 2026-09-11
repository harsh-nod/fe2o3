//! Whole-roster metadata registration; no native allocation or publication.

use super::*;
use crate::generated_source::{GeneratedHostRosterV1, RuntimeGfx942GeneratedSourceMutV1};
use crate::kfd_backend::GeneratedShellBindingV1;
use crate::{KfdRuntimeBackendErrorV1, KfdRuntimeBackendV1};

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(super) fn install_generated_shells_v1<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        native_device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
        hold: &ContextUnpublishedHoldV1,
        source: &mut RuntimeGfx942GeneratedSourceMutV1<'_, E>,
        roster: &GeneratedHostRosterV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        let stream = *self.streams.get(&hold.stream()).expect("exact held stream");
        if stream.device != device || stream.generated.is_some() || !source.matches_roster(roster) {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        if self.next_identity == 0
            || roster.count == 0
            || roster.count > fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_DATA_V1
            || self
                .allocations
                .len()
                .checked_add(roster.count)
                .is_none_or(|count| count > MAX_RUNTIME_ALLOCATIONS_V1)
        {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        let record = self.device(device)?;
        if !record.capabilities.host_visible_memory {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let backend_device = record.backend_device;
        let next_identity = self
            .next_identity
            .checked_add(roster.count as u64)
            .ok_or(RuntimeValidationErrorV1::Capacity)?;
        let mut logical = Vec::new();
        logical
            .try_reserve_exact(roster.count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let mut lengths = Vec::new();
        lengths
            .try_reserve_exact(roster.count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        for ordinal in 0..roster.count {
            let slot = roster.buffers[ordinal]
                .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
            if slot.ordinal != ordinal || slot.bytes == 0 {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            let id = RuntimeAllocationIdV1::new(
                self.context_generation,
                self.next_identity + ordinal as u64,
            );
            if self.allocations.contains_key(&id) {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            logical.push(id);
            lengths.push(slot.bytes);
        }
        self.allocations
            .try_reserve(roster.count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        self.backend_allocations
            .try_reserve(roster.count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let binding = GeneratedShellBindingV1 {
            context_generation: self.context_generation,
            device,
            stream: hold.stream(),
            hold: hold.identity(),
            backend_device,
            backend_stream: stream.backend_stream,
            native_device,
        };
        let plan = self
            .backend
            .prepare_generated_shells_v1(binding, roster, &logical)
            .map_err(map_backend_error)?;
        if plan.members[..plan.count]
            .iter()
            .flatten()
            .any(|member| self.backend_allocations.contains(&member.backend))
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        let credits = self
            .allocation_admission
            .prepare_roster(device, &lengths)
            .map_err(|error| {
                if error == crate::RuntimeResourceCreditErrorV1::Invariant {
                    self.terminal = true;
                }
                RuntimeValidationErrorV1::Capacity
            })?;
        let mut credits = credits.map(|members| members.into_vec().into_iter());
        // Every capacity and identity check precedes this non-reentrant commit.
        // Root Context records/credits before transferring control into the backend.
        self.next_identity = next_identity;
        for member in plan.members[..plan.count].iter().flatten() {
            self.allocations.insert(
                member.logical,
                AllocationRecordV1 {
                    backend_allocation: member.backend,
                    device,
                    kind: member.description.kind,
                    byte_len: member.description.byte_len,
                },
            );
            assert!(self.backend_allocations.insert(member.backend));
            self.allocation_admission.attach(
                member.logical,
                credits
                    .as_mut()
                    .map(|members| members.next().expect("complete credit roster").retain()),
            );
        }
        self.streams
            .get_mut(&hold.stream())
            .expect("held stream")
            .generated = Some(plan.key);
        self.backend
            .commit_generated_shells_v1(plan, source, roster);
        Ok(())
    }

    #[allow(
        dead_code,
        reason = "private DATA adoption handoff; native effects are not installed"
    )]
    pub(crate) fn retire_generated_shells_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        let stream = *self.streams.get(&hold.stream()).expect("exact held stream");
        let key = stream
            .generated
            .ok_or(RuntimeValidationErrorV1::ContextReserved)?;
        let plan = self
            .backend
            .generated_shell_plan_v1(key, self.context_generation, hold.stream(), hold.identity())
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        if plan.binding.device != stream.device
            || plan.binding.backend_stream != stream.backend_stream
            || !self.backend.validate_generated_shell_disposal_v1(&plan)
            || plan.members[..plan.count].iter().any(|member| {
                member.is_none_or(|member| {
                    !self.backend_allocations.contains(&member.backend)
                        || self.allocations.get(&member.logical).is_none_or(|record| {
                            record.backend_allocation != member.backend
                                || record.device != stream.device
                                || record.kind != member.description.kind
                                || record.byte_len != member.description.byte_len
                        })
                })
            })
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        self.backend.dispose_generated_shells_v1(&plan);
        for member in plan.members[..plan.count].iter().flatten() {
            self.allocations.remove(&member.logical);
            self.backend_allocations.remove(&member.backend);
            self.dispose_allocation_credits_v1(member.logical);
        }
        self.streams
            .get_mut(&hold.stream())
            .expect("held stream")
            .generated = None;
        Ok(())
    }
}
