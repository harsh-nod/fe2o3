//! Exact-three preparation retains each original readback and authority owner.

use super::*;
use crate::RuntimeGfx942GeneratedReservationErrorV1 as ReservationError;
use crate::generated_source::GeneratedHostRosterV1;
use crate::{RuntimeGfx942GeneratedCarrierV1, RuntimeGfx942GeneratedCohort3V1};

impl<P: crate::RuntimeGfx942GeneratedCompletionCarrierV1>
    RuntimeGfx942PreparedV1<RuntimeGfx942GeneratedCohort3V1<P>>
{
    pub(in crate::context) fn complete_cohort3_readbacks_v1(
        self,
    ) -> Result<(), crate::RuntimeGfx942ReadbackErrorV1> {
        let mut result = Ok(());
        for member in self.value.members {
            let next = member.complete_readback_v1();
            if result.is_ok() {
                result = next;
            }
        }
        result
    }
}

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(in crate::context) fn reserve_gfx942_cohort3_v1<P: RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<RuntimeGfx942GeneratedCohort3V1<P>>,
    ) -> Result<GeneratedHostRosterV1, ReservationError> {
        let binding = prepared.binding;
        let backend_device = self
            .preparation_backend_device_v1(binding.device)
            .map_err(ReservationError::Context)?;
        if !binding.matches_context(self.context_generation, binding.device, backend_device) {
            return Err(ReservationError::Context(
                RuntimeValidationErrorV1::InvalidBackendDescription.into(),
            ));
        }
        let (roster, readbacks) = self
            .with_preparation_owner_v1(backend_device, |owner| {
                if !binding.matches_native(owner.model_admission()) {
                    return Err(ReservationError::InvalidRoster);
                }
                let value = &prepared.value;
                let roster = value.validate_sources(owner.observation().unique_id())?;
                let [a, b, c] = value.members.each_ref();
                let readbacks = [
                    a.prepare_readback().map_err(ReservationError::Readback)?,
                    b.prepare_readback().map_err(ReservationError::Readback)?,
                    c.prepare_readback().map_err(ReservationError::Readback)?,
                ];
                for member in &value.members {
                    member.source().revalidate()?;
                }
                Ok((roster, readbacks))
            })
            .map_err(ReservationError::Context)??;
        // Installation follows all three source checks and the closing device check.
        for (member, readback) in prepared.value.members.iter_mut().zip(readbacks) {
            member.install_readback(readback);
        }
        Ok(roster)
    }

    pub(in crate::context) fn preflight_gfx942_cohort3_v1<P: RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &RuntimeGfx942PreparedV1<RuntimeGfx942GeneratedCohort3V1<P>>,
        expected: &GeneratedHostRosterV1,
        stream: RuntimeStreamIdV1,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.require_unpublished_open_access_v1(access)?;
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
        self.with_preparation_owner_v1(prepared.binding.backend_device, |owner| {
            let actual = prepared
                .value
                .validate_sources(owner.observation().unique_id())
                .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)?;
            if !actual.matches(expected) {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
            }
            Ok(())
        })??;
        Ok(())
    }

    pub(in crate::context) fn adopt_gfx942_cohort3_v1<P: RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<RuntimeGfx942GeneratedCohort3V1<P>>,
        expected: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_unpublished_hold_v1(hold)?;
            let binding = prepared.binding;
            let stream = self.streams.get(&hold.stream()).expect("exact held stream");
            if stream.device != binding.device || stream.generated.is_some() {
                return Err(RuntimeValidationErrorV1::ContextReserved.into());
            }
            let backend_device = self.preparation_backend_device_v1(binding.device)?;
            if !binding.matches_context(self.context_generation, binding.device, backend_device) {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            {
                let mut source = prepared
                    .value
                    .source_mut_v1()
                    .ok_or(RuntimeValidationErrorV1::Unsupported)?;
                self.with_preparation_owner_v1(backend_device, |owner| {
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
                self.install_generated_cohort3_shells_v1(
                    binding.device,
                    binding.native_device,
                    hold,
                    &mut source,
                    expected,
                )?;
            }
            let plan = self.generated_plan_for_hold_v1(hold)?;
            let uid = self.with_preparation_owner_v1(backend_device, |owner| {
                owner.observation().unique_id()
            })?;
            prepared
                .value
                .with_native_inputs_v1(uid, expected, |programs, buffers| {
                    self.backend
                        .adopt_generated_cohort3_data_v1(&plan, expected, programs, buffers)
                })
                .map_err(|_| {
                    RuntimeErrorV1::Validation(RuntimeValidationErrorV1::InvalidBackendDescription)
                })?
                .map_err(map_backend_error)
        }));
        self.finish_gfx942_adoption_scoped_v1(scope, result)
    }
}
