//! Original four-source preparation and common Context allocation debit.

use super::*;
use crate::RuntimeGfx942GeneratedReservationErrorV1 as ReservationError;
use crate::generated_source::GeneratedHostRosterV1;
use crate::{
    RuntimeGfx942GeneratedCarrierV1, RuntimeGfx942GeneratedResidentRegistryV1 as Registry,
};

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(in crate::context) fn reserve_gfx942_registry4_v1<
        P: RuntimeGfx942GeneratedCarrierV1,
        const N: usize,
    >(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<Registry<P, N>>,
    ) -> Result<GeneratedHostRosterV1, ReservationError> {
        let binding = prepared.binding;
        let uid = self
            .preparation_backend_device_v1(binding.device)
            .map_err(ReservationError::Context)?;
        if !binding.matches_context(self.context_generation, binding.device, uid) {
            return Err(ReservationError::InvalidRoster);
        }
        let (roster, readbacks) = self
            .with_preparation_owner_v1(uid, |owner| {
                if !binding.matches_native(owner.model_admission()) {
                    return Err(ReservationError::InvalidRoster);
                }
                let value = &prepared.value;
                let roster = value.validate_sources(owner.observation().unique_id())?;
                let mut readbacks = core::array::from_fn::<_, N, _>(|_| None);
                for (slot, member) in readbacks.iter_mut().zip(&value.members) {
                    *slot = Some(
                        member
                            .prepare_readback()
                            .map_err(ReservationError::Readback)?,
                    );
                }
                let readbacks =
                    readbacks.map(|value| value.unwrap_or_else(|| std::process::abort()));
                for member in &value.members {
                    member.source().revalidate()?;
                }
                Ok((roster, readbacks))
            })
            .map_err(ReservationError::Context)??;
        for (member, readback) in prepared.value.members.iter_mut().zip(readbacks) {
            member.install_readback(readback);
        }
        Ok(roster)
    }

    pub(in crate::context) fn preflight_gfx942_registry4_v1<
        P: RuntimeGfx942GeneratedCarrierV1,
        const N: usize,
    >(
        &mut self,
        prepared: &RuntimeGfx942PreparedV1<Registry<P, N>>,
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
        if !actual.matches(expected) {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        Ok(())
    }

    pub(in crate::context) fn adopt_gfx942_registry4_v1<
        P: RuntimeGfx942GeneratedCarrierV1,
        const N: usize,
    >(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<Registry<P, N>>,
        expected: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
        storage: crate::kfd_backend::RegistryStorageV1,
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
            {
                let mut source = prepared
                    .value
                    .source_mut_v1()
                    .ok_or(RuntimeValidationErrorV1::Unsupported)?;
                self.with_preparation_owner_v1(uid, |owner| {
                    if !binding.matches_native(owner.model_admission()) {
                        return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
                    }
                    let actual = source
                        .validate(owner.observation().unique_id())
                        .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)?;
                    if !actual.matches(expected) {
                        return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
                    }
                    Ok(())
                })??;
                self.install_generated_registry4_shells_v1(
                    binding.device,
                    binding.native_device,
                    hold,
                    &mut source,
                    expected,
                )?;
            }
            let plan = self.generated_plan_for_hold_v1(hold)?;
            prepared
                .value
                .with_native_inputs_v1(uid, expected, |programs, buffers| {
                    self.backend
                        .adopt_generated_registry4_v1(&plan, expected, programs, buffers, storage)
                })
                .map_err(|_| {
                    RuntimeErrorV1::Validation(RuntimeValidationErrorV1::InvalidBackendDescription)
                })?
                .map_err(map_backend_error)
        }));
        self.finish_gfx942_adoption_scoped_v1(scope, result)
    }
}
