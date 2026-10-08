//! One original Context DATA debit for the closed WO arena.

use super::*;
use crate::generated_source::GeneratedHostRosterV1;
use crate::{
    RuntimeGfx942GeneratedArena1024V1 as Arena, RuntimeGfx942GeneratedCarrierV1,
    RuntimeGfx942GeneratedReservationErrorV1 as ReservationError,
};
use fe2o3_resource_accounting::HostMetadataTableV1;

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(in crate::context) fn reserve_gfx942_arena_v1<P: RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<Arena<P>>,
    ) -> Result<GeneratedHostRosterV1, ReservationError> {
        let binding = prepared.binding;
        let uid = self
            .preparation_backend_device_v1(binding.device)
            .map_err(ReservationError::Context)?;
        if !binding.matches_context(self.context_generation, binding.device, uid) {
            return Err(ReservationError::InvalidRoster);
        }
        let (roster, mut readbacks) = self
            .with_preparation_owner_v1(uid, |owner| {
                if !binding.matches_native(owner.model_admission()) {
                    return Err(ReservationError::InvalidRoster);
                }
                let value = &prepared.value;
                let roster = value.validate_sources(owner.observation().unique_id())?;
                let mut readbacks = HostMetadataTableV1::try_new(
                    value.members.len(),
                    Some(value.metadata()),
                    || None,
                )
                .map_err(|_| {
                    ReservationError::Readback(crate::RuntimeGfx942ReadbackErrorV1::Allocation)
                })?;
                for (slot, member) in readbacks.iter_mut().zip(value.members.iter()) {
                    *slot = Some(
                        member
                            .as_ref()
                            .unwrap_or_else(|| std::process::abort())
                            .prepare_readback()
                            .map_err(ReservationError::Readback)?,
                    );
                }
                value.revalidate_sources()?;
                Ok((roster, readbacks))
            })
            .map_err(ReservationError::Context)??;
        for (member, readback) in prepared.value.members.iter_mut().zip(readbacks.iter_mut()) {
            member
                .as_mut()
                .unwrap_or_else(|| std::process::abort())
                .install_readback(readback.take().unwrap_or_else(|| std::process::abort()));
        }
        Ok(roster)
    }

    pub(in crate::context) fn preflight_gfx942_arena_v1<P: RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &RuntimeGfx942PreparedV1<Arena<P>>,
        expected: &GeneratedHostRosterV1,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.require_unpublished_open_access_v1(None)?;
        self.validate_gfx942_prepared_v1(prepared)?;
        let stream = self
            .streams
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        if stream.device != prepared.binding.device || stream.generated.is_some() {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        self.backend
            .preflight_generated_cohort3_lane_v1()
            .map_err(map_backend_error)?;
        let actual = prepared
            .value
            .validate_sources(prepared.binding.backend_device)
            .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)?;
        if !actual.matches(expected) || prepared.value.packets.is_none() {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        Ok(())
    }

    pub(in crate::context) fn adopt_gfx942_arena_v1<P: RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<Arena<P>>,
        expected: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
        storage: crate::kfd_backend::ArenaPreallocationV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_unpublished_hold_v1(hold)?;
            let binding = prepared.binding;
            let stream = self
                .streams
                .get(&hold.stream())
                .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
            if stream.device != binding.device || stream.generated.is_some() {
                return Err(RuntimeValidationErrorV1::ContextReserved.into());
            }
            let uid = self.preparation_backend_device_v1(binding.device)?;
            if !binding.matches_context(self.context_generation, binding.device, uid) {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            self.with_preparation_owner_v1(uid, |owner| {
                if !binding.matches_native(owner.model_admission()) {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
                }
                let actual = prepared
                    .value
                    .validate_sources(owner.observation().unique_id())
                    .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)?;
                if !actual.matches(expected) || prepared.value.packets.is_none() {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
                }
                Ok(())
            })??;
            self.install_generated_arena_shells_v1(
                binding.device,
                binding.native_device,
                hold,
                &mut prepared.value,
                expected,
            )?;
            let plan = self.generated_plan_for_hold_v1(hold)?;
            prepared
                .value
                .with_native_inputs_v1(uid, expected, |program, bytes| {
                    self.backend
                        .adopt_generated_arena_v1(&plan, expected, program, bytes, storage)
                })
                .map_err(|_| {
                    RuntimeErrorV1::Validation(RuntimeValidationErrorV1::InvalidBackendDescription)
                })?
                .map_err(map_backend_error)
        }));
        self.finish_gfx942_adoption_scoped_v1(scope, result)
    }
}
