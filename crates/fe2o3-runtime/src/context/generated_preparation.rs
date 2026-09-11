//! Context binding for inert generated preparation, not launch authority.

use std::rc::Rc;

use fe2o3_kfd::CheckedGfx942XnackMinusDevice;
use fe2o3_runtime_model::ModelDeviceAdmissionV1;

use super::*;
use crate::{KfdRuntimeBackendErrorV1, KfdRuntimeBackendV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparationBindingV1 {
    context_generation: u64,
    device: RuntimeDeviceIdV1,
    backend_device: u64,
    native_device: ModelDeviceAdmissionV1,
}

impl PreparationBindingV1 {
    fn matches_context(self, generation: u64, device: RuntimeDeviceIdV1, backend: u64) -> bool {
        self.context_generation == generation
            && self.device == device
            && self.backend_device == backend
    }

    fn matches_native(self, native: ModelDeviceAdmissionV1) -> bool {
        self.native_device == native
    }
}

/// Inert owner-local payload bound to one Context and retained device generation.
///
/// It has no public constructor, consuming extraction, Clone, Send or Sync.
/// This binding grants no execution authority, native allocation lease or future
/// currentness. A Context may be used again immediately after preparation.
///
/// ```compile_fail,E0277
/// use fe2o3_runtime::RuntimeGfx942PreparedV1;
/// fn send<T: Send>() {}
/// send::<RuntimeGfx942PreparedV1<()>>();
/// ```
/// ```compile_fail,E0277
/// use fe2o3_runtime::RuntimeGfx942PreparedV1;
/// fn sync<T: Sync>() {}
/// sync::<RuntimeGfx942PreparedV1<()>>();
/// ```
/// ```compile_fail,E0616
/// use fe2o3_runtime::RuntimeGfx942PreparedV1;
/// fn extract(value: RuntimeGfx942PreparedV1<Vec<u8>>) -> Vec<u8> { value.value }
/// ```
#[must_use]
pub struct RuntimeGfx942PreparedV1<T> {
    value: T,
    binding: PreparationBindingV1,
    owner_local: PhantomData<Rc<()>>,
}

impl<T> RuntimeGfx942PreparedV1<T> {
    pub const fn value(&self) -> &T {
        &self.value
    }
}

impl<T> fmt::Debug for RuntimeGfx942PreparedV1<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeGfx942PreparedV1")
            .field("device", &self.binding.device)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum RuntimeGfx942PreparationErrorV1<E> {
    Context(RuntimeErrorV1<KfdRuntimeBackendErrorV1>),
    Preparation(E),
}

impl<E: fmt::Display> fmt::Display for RuntimeGfx942PreparationErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Context(error) => error.fmt(formatter),
            Self::Preparation(error) => error.fmt(formatter),
        }
    }
}

impl<E: Error + 'static> Error for RuntimeGfx942PreparationErrorV1<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Context(error) => Some(error),
            Self::Preparation(error) => Some(error),
        }
    }
}

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    #[allow(
        dead_code,
        reason = "private DATA adoption handoff; native effects are not installed"
    )]
    pub(crate) fn register_gfx942_generated_shells_v1<T: crate::RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<T>,
        hold: &ContextUnpublishedHoldV1,
        expected: &crate::generated_source::GeneratedHostRosterV1,
    ) -> Result<(), crate::RuntimeGfx942GeneratedReservationErrorV1> {
        use crate::RuntimeGfx942GeneratedReservationErrorV1 as Error;
        self.validate_unpublished_hold_v1(hold)
            .map_err(|error| Error::Context(error.into()))?;
        let binding = prepared.binding;
        let stream = self.streams.get(&hold.stream()).expect("exact held stream");
        if stream.device != binding.device || stream.generated.is_some() {
            return Err(Error::Context(
                RuntimeValidationErrorV1::ContextReserved.into(),
            ));
        }
        let backend_device = self
            .preparation_backend_device_v1(binding.device)
            .map_err(Error::Context)?;
        if !binding.matches_context(self.context_generation, binding.device, backend_device) {
            return Err(Error::Context(
                RuntimeValidationErrorV1::InvalidBackendDescription.into(),
            ));
        }
        let mut source = prepared
            .value
            .source_mut()
            .ok_or(Error::UnsupportedPreparation)?;
        let roster = self
            .with_preparation_owner_v1(backend_device, |owner| {
                if !binding.matches_native(owner.model_admission()) {
                    return Err(Error::Context(
                        RuntimeValidationErrorV1::InvalidBackendDescription.into(),
                    ));
                }
                let roster = source.validate(owner.observation().unique_id())?;
                if !roster.matches(expected) {
                    return Err(Error::InvalidRoster);
                }
                Ok(roster)
            })
            .map_err(Error::Context)??;
        self.install_generated_shells_v1(
            binding.device,
            binding.native_device,
            hold,
            &mut source,
            &roster,
        )
        .map_err(Error::Context)
    }

    pub(crate) fn reserve_gfx942_prepared_v1<T: crate::RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<T>,
    ) -> Result<
        crate::generated_source::GeneratedHostRosterV1,
        crate::RuntimeGfx942GeneratedReservationErrorV1,
    > {
        use crate::RuntimeGfx942GeneratedReservationErrorV1 as Error;
        let binding = prepared.binding;
        let backend_device = self
            .preparation_backend_device_v1(binding.device)
            .map_err(Error::Context)?;
        if !binding.matches_context(self.context_generation, binding.device, backend_device) {
            return Err(Error::Context(
                RuntimeValidationErrorV1::InvalidBackendDescription.into(),
            ));
        }
        install_checked_readback(&mut prepared.value, |value| {
            self.with_preparation_owner_v1(backend_device, |owner| {
                if !binding.matches_native(owner.model_admission()) {
                    return Err(Error::Context(
                        RuntimeValidationErrorV1::InvalidBackendDescription.into(),
                    ));
                }
                let source = value.source();
                let roster = source.validate(owner.observation().unique_id())?;
                let readback = value.prepare_readback().map_err(Error::Readback)?;
                source.revalidate()?;
                Ok((roster, readback))
            })
            .map_err(Error::Context)?
        })
    }

    /// Runs nonexecuting preparation against this Context's retained checked device.
    ///
    /// The immutable borrow cannot escape or replace device custody. Full native
    /// currentness brackets the callback, including an error-valued result.
    /// Runtime failures take precedence over callback results. Unwind seals the
    /// Context. No queue or VM is created here, and neither callback success nor
    /// the returned wrapper authorizes publication.
    ///
    /// ```compile_fail
    /// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeContextV1, RuntimeDeviceIdV1};
    /// fn escape(context: &mut RuntimeContextV1<KfdRuntimeBackendV1>, device: RuntimeDeviceIdV1) {
    ///     let _ = context.with_gfx942_preparation_device_v1(device, |owner| Ok::<_, ()>(owner));
    /// }
    /// ```
    /// ```compile_fail,E0308
    /// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeContextV1, RuntimeDeviceIdV1};
    /// use fe2o3_kfd::CheckedGfx942XnackMinusDevice;
    /// fn replace(context: &mut RuntimeContextV1<KfdRuntimeBackendV1>, id: RuntimeDeviceIdV1,
    ///            replacement: CheckedGfx942XnackMinusDevice) {
    ///     let _ = context.with_gfx942_preparation_device_v1(id, |owner| {
    ///         Ok::<_, ()>(std::mem::replace(owner, replacement))
    ///     });
    /// }
    /// ```
    pub fn with_gfx942_preparation_device_v1<T, E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        prepare: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> Result<T, E>,
    ) -> Result<RuntimeGfx942PreparedV1<T>, RuntimeGfx942PreparationErrorV1<E>> {
        let backend_device = self
            .preparation_backend_device_v1(device)
            .map_err(RuntimeGfx942PreparationErrorV1::Context)?;
        let (native_device, value) = self
            .with_preparation_owner_v1(backend_device, |owner| {
                (owner.model_admission(), prepare(owner))
            })
            .map_err(RuntimeGfx942PreparationErrorV1::Context)?;
        let value = value.map_err(RuntimeGfx942PreparationErrorV1::Preparation)?;
        Ok(RuntimeGfx942PreparedV1 {
            value,
            binding: PreparationBindingV1 {
                context_generation: self.context_generation,
                device,
                backend_device,
                native_device,
            },
            owner_local: PhantomData,
        })
    }

    /// Rechecks exact binding and native currentness without consuming the payload.
    /// This is not operation admission, native completion or permission to replay.
    pub fn validate_gfx942_prepared_v1<T>(
        &mut self,
        prepared: &RuntimeGfx942PreparedV1<T>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let binding = prepared.binding;
        let backend_device = self.preparation_backend_device_v1(binding.device)?;
        if !binding.matches_context(self.context_generation, binding.device, backend_device) {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        let current =
            self.with_preparation_owner_v1(backend_device, |owner| owner.model_admission())?;
        if !binding.matches_native(current) {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        Ok(())
    }

    fn preparation_backend_device_v1(
        &self,
        device: RuntimeDeviceIdV1,
    ) -> Result<u64, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        if self.terminal {
            return Err(RuntimeValidationErrorV1::ContextTerminal.into());
        }
        Ok(self.device(device)?.backend_device)
    }

    fn with_preparation_owner_v1<T>(
        &mut self,
        backend_device: u64,
        prepare: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> T,
    ) -> Result<T, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        match catch_unwind(AssertUnwindSafe(|| {
            self.backend
                .with_retained_preparation_device_v1(backend_device, prepare)
        })) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(failure)) => {
                if matches!(failure, RuntimeBackendFailureV1::Terminal(_)) {
                    self.terminal = true;
                }
                Err(map_backend_error(failure))
            }
            Err(payload) => {
                self.terminal = true;
                std::panic::resume_unwind(payload)
            }
        }
    }
}

fn install_checked_readback<T: crate::RuntimeGfx942GeneratedCarrierV1>(
    value: &mut T,
    checked_stage: impl FnOnce(
        &T,
    ) -> Result<
        (crate::generated_source::GeneratedHostRosterV1, T::Readback),
        crate::RuntimeGfx942GeneratedReservationErrorV1,
    >,
) -> Result<
    crate::generated_source::GeneratedHostRosterV1,
    crate::RuntimeGfx942GeneratedReservationErrorV1,
> {
    // The scope, including its closing device checks, must return success
    // before installation can change the original carrier.
    let (roster, readback) = checked_stage(value)?;
    value.install_readback(readback);
    Ok(roster)
}

#[cfg(test)]
mod tests;
