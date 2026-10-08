//! No-native child failure is joined to the original source and unpublished hold.

use super::*;
use crate::generated_source::GeneratedHostRosterV1;
use crate::kfd_backend::GeneratedColdDeviceFailureV1;

pub(in crate::context) struct ContextColdDeviceFailureV1 {
    original: GeneratedColdDeviceFailureV1,
    binding: PreparationBindingV1,
    stream: RuntimeStreamIdV1,
    hold: u64,
}

impl ContextColdDeviceFailureV1 {
    pub(in crate::context) fn device_uid(&self) -> u64 {
        self.original.device_uid()
    }
}

macro_rules! cold_context {
    ($backend:ty) => {
        impl RuntimeContextV1<$backend> {
            #[cfg(test)]
            pub(in crate::context) fn synthetic_cold_failure_for_test_v1<T>(
                &mut self,
                prepared: &RuntimeGfx942PreparedV1<T>,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<ContextColdDeviceFailureV1, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                let (device, stream) = self.cold_prepared_binding_v1(prepared, hold)?;
                let scope = self.backend.generated_cold_scope_v1(device, stream).map_err(map_backend_error)?;
                Ok(ContextColdDeviceFailureV1 {
                    original: GeneratedColdDeviceFailureV1::synthetic_for_test(scope, prepared.binding.native_device),
                    binding: prepared.binding,
                    stream: hold.stream(),
                    hold: hold.identity(),
                })
            }

            #[cfg(test)]
            pub(in crate::context) fn validate_synthetic_cold_failure_for_test_v1<T>(
                &self,
                prepared: &RuntimeGfx942PreparedV1<T>,
                hold: &ContextUnpublishedHoldV1,
                failure: &ContextColdDeviceFailureV1,
            ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                let (device, _) = self.cold_prepared_binding_v1(prepared, hold)?;
                if failure.binding != prepared.binding || failure.stream != hold.stream()
                    || failure.hold != hold.identity() || failure.device_uid() != device
                {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                }
                Ok(())
            }

            fn cold_prepared_binding_v1<T>(
                &self,
                prepared: &RuntimeGfx942PreparedV1<T>,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<(u64, u64), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.validate_unpublished_hold_v1(hold)?;
                let stream = self.streams.get(&hold.stream())
                    .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
                let device = self.device(stream.device)?.backend_device;
                if stream.generated.is_some()
                    || self.generated_issues.contains_key(&hold.stream())
                    || !prepared.binding.matches_context(self.context_generation, stream.device, device)
                {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                }
                Ok((device, stream.backend_stream))
            }

            pub(in crate::context) fn check_gfx942_cold_device_v1<T: crate::RuntimeGfx942GeneratedCarrierV1>(
                &mut self,
                prepared: &mut RuntimeGfx942PreparedV1<T>,
                roster: &GeneratedHostRosterV1,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<Option<ContextColdDeviceFailureV1>, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                let (device, stream) = self.cold_prepared_binding_v1(prepared, hold)?;
                let scope = self.backend.generated_cold_scope_v1(device, stream).map_err(map_backend_error)?;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    let mut outcome = None;
                    prepared.value.source().with_current_source_v1(device, roster, || {
                        outcome = self.backend.check_generated_cold_device_v1(scope, prepared.binding.native_device)
                            .map_err(map_backend_error)?;
                        Ok::<_, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>(())
                    }).map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)??;
                    Ok(outcome.map(|original| ContextColdDeviceFailureV1 {
                        original,
                        binding: prepared.binding,
                        stream: hold.stream(),
                        hold: hold.identity(),
                    }))
                }));
                self.finish_gfx942_adoption_scoped_v1(scope, result)
            }

            pub(in crate::context) fn settle_gfx942_cold_device_v1<T: crate::RuntimeGfx942GeneratedCarrierV1>(
                &mut self,
                prepared: &mut RuntimeGfx942PreparedV1<T>,
                roster: &GeneratedHostRosterV1,
                hold: &ContextUnpublishedHoldV1,
                failure: &ContextColdDeviceFailureV1,
            ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                let (device, stream) = self.cold_prepared_binding_v1(prepared, hold)?;
                let scope = self.backend.generated_cold_scope_v1(device, stream).map_err(map_backend_error)?;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    if failure.binding != prepared.binding || failure.stream != hold.stream()
                        || failure.hold != hold.identity() || failure.device_uid() != device
                    {
                        return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                    }
                    prepared.value.source().with_current_source_v1(device, roster, || {
                        self.backend.validate_generated_cold_failure_v1(&failure.original)
                            .map_err(map_backend_error)
                    }).map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)??;
                    // No generated shell, writer, submission, native DATA or
                    // debit was installed. The caller still owns the original
                    // carrier and hold, and must dispose both before reporting.
                    self.cold_prepared_binding_v1(prepared, hold)?;
                    Ok(())
                }));
                self.finish_gfx942_adoption_scoped_v1(scope, result)
            }
        }
    };
}
cold_context!(KfdRuntimeBackendV1);
cold_context!(KfdMultiDeviceRuntimeBackendV1);

#[cfg(test)]
mod tests;
