//! Join the original Context-bound carrier to backend-owned native DATA.

use super::*;
use crate::RuntimeGfx942GeneratedCarrierV1;
use crate::generated_source::GeneratedHostRosterV1;

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(crate) fn preflight_gfx942_adoption_v1<T: RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &RuntimeGfx942PreparedV1<T>,
        expected: &GeneratedHostRosterV1,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_gfx942_prepared_v1(prepared)?;
        let stream = self
            .streams
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        if stream.device != prepared.binding.device || stream.generated.is_some() {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        self.with_preparation_owner_v1(prepared.binding.backend_device, |owner| {
            let actual = prepared
                .value
                .source()
                .validate(owner.observation().unique_id())
                .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)?;
            if !actual.matches(expected) {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
            }
            Ok(())
        })??;
        Ok(())
    }

    pub(crate) fn gfx942_adoption_ready_v1(
        &self,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        let stream = self.streams.get(&hold.stream()).expect("exact held stream");
        if stream.generated.is_some()
            || !self
                .backend
                .generated_empty_prefix_v1(stream.backend_stream)
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        self.backend
            .generated_lane_ready_v1()
            .map_err(map_backend_error)
    }

    pub(crate) fn adopt_gfx942_prepared_v1<T: RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<T>,
        expected: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.register_gfx942_generated_shells_v1(prepared, hold, expected)
                .map_err(|error| match error {
                    crate::RuntimeGfx942GeneratedReservationErrorV1::Context(error) => error,
                    _ => RuntimeValidationErrorV1::InvalidBackendDescription.into(),
                })?;
            let plan = self.generated_plan_for_hold_v1(hold)?;
            let device_uid = self
                .with_preparation_owner_v1(prepared.binding.backend_device, |owner| {
                    owner.observation().unique_id()
                })?;
            prepared
                .value
                .source()
                .with_native_inputs_v1(device_uid, expected, |program, buffers| {
                    self.backend
                        .adopt_generated_data_v1(&plan, expected, program, buffers)
                })
                .map_err(|_| {
                    RuntimeErrorV1::Validation(RuntimeValidationErrorV1::InvalidBackendDescription)
                })?
                .map_err(map_backend_error)
        }));
        self.finish_gfx942_adoption_v1(result)
    }

    pub(crate) fn retire_gfx942_adoption_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        if self.generated_issues.contains_key(&hold.stream()) {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            let stream = *self.streams.get(&hold.stream()).expect("exact held stream");
            if stream.generated.is_none() {
                return if self
                    .backend
                    .generated_empty_prefix_v1(stream.backend_stream)
                {
                    Ok(())
                } else {
                    Err(RuntimeValidationErrorV1::InvalidBackendDescription.into())
                };
            }
            let plan = self.generated_plan_for_hold_v1(hold)?;
            self.backend
                .retire_generated_data_v1(&plan)
                .map_err(map_backend_error)?;
            self.retire_generated_shells_v1(hold)
        }));
        self.finish_gfx942_adoption_v1(result)
    }

    pub(in crate::context) fn finish_gfx942_adoption_v1(
        &mut self,
        result: std::thread::Result<Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.terminal = true;
                self.backend.quarantine_generated_adoption_v1();
                Err(error)
            }
            Err(payload) => {
                self.terminal = true;
                self.backend.resume_generated_adoption_panic_v1(payload)
            }
        }
    }
}
