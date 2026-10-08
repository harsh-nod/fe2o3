//! Native registry carrier: decoded output never releases its source authority.

use super::*;
use crate::generated_runtime_arguments::{
    GeneratedRegistryRepeatFrameV1, GeneratedRegistryStorageV1,
};
use fe2o3_runtime::RuntimeGfx942RegistryCompletionCarrierV1;

pub(crate) struct GeneratedRegistryCarrierV1<A, const REPEAT: bool = false> {
    second: Option<GeneratedRegistryRepeatFrameV1>,
    storage: GeneratedRegistryStorageV1,
    authority: A,
    _footprint: GeneratedRuntimeArgumentFootprintV1,
    _result_budget: GeneratedRuntimeResultBudgetV1,
}

impl<A> GeneratedRegistryCarrierV1<A> {
    pub(crate) fn new(
        original: GeneratedRuntimeCarrierV1<A>,
    ) -> Result<Self, GeneratedRuntimeArgumentErrorV1> {
        Ok(Self {
            second: None,
            storage: GeneratedRegistryStorageV1::new(original.storage)?,
            authority: original.authority,
            _footprint: original.footprint,
            _result_budget: original.result_budget,
        })
    }
}

impl<A> GeneratedRegistryCarrierV1<A, true> {
    pub(crate) fn new_repeat2(
        original: GeneratedRuntimeCarrierV1<A>,
    ) -> Result<(Self, crate::GeneratedRuntimeChargedResultV1<u32>), GeneratedRuntimeArgumentErrorV1>
    {
        let (second, observer) =
            GeneratedRegistryRepeatFrameV1::prepare(&original.storage, &original.result_budget)?;
        Ok((
            Self {
                second: Some(second),
                storage: GeneratedRegistryStorageV1::new(original.storage)?,
                authority: original.authority,
                _footprint: original.footprint,
                _result_budget: original.result_budget,
            },
            observer,
        ))
    }
}

impl<A: GeneratedRuntimeAuthorityV1, const REPEAT: bool> RuntimeGfx942GeneratedCarrierV1
    for GeneratedRegistryCarrierV1<A, REPEAT>
{
    type CurrentnessError = A::CurrentnessError;
    type Readback = GeneratedRuntimeReadbackOwnerV1;

    fn source(&self) -> RuntimeGfx942GeneratedSourceV1<'_, Self::CurrentnessError> {
        RuntimeGfx942GeneratedSourceV1::from_generated_storage(
            self.storage.prepared(),
            self.authority.artifact_bytes(),
            &self.authority,
        )
    }

    fn source_mut(
        &mut self,
    ) -> Option<RuntimeGfx942GeneratedSourceMutV1<'_, Self::CurrentnessError>> {
        Some(RuntimeGfx942GeneratedSourceMutV1::new(
            self.storage.prepared_mut()?,
            self.authority.artifact_bytes(),
            &self.authority,
        ))
    }

    fn prepare_readback(&self) -> Result<Self::Readback, RuntimeGfx942ReadbackErrorV1> {
        self.storage.prepare_readback().map_err(readback_error)
    }

    fn install_readback(&mut self, readback: Self::Readback) {
        self.storage.install_readback(readback);
    }
}

// SAFETY: the original reserved destinations, decoder and gate remain private.
// The registry transition retains the original payload before decoding, keeps
// authority/epoch ownership even on refusal/unwind, and releases only disposed
// copied-output credits. It cannot produce a scalar native-settlement receipt.
unsafe impl<A: GeneratedRuntimeAuthorityV1, const REPEAT: bool>
    RuntimeGfx942RegistryCompletionCarrierV1 for GeneratedRegistryCarrierV1<A, REPEAT>
{
    fn registry_completion_domain_v1(
        &self,
    ) -> Result<fe2o3_runtime::RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
        self.storage.domain().map_err(readback_error)
    }

    fn with_registry_completion_view_v1(
        &mut self,
        callback: impl for<'a> FnOnce(
            RuntimeGfx942GeneratedCompletionViewV1<'a, Self::CurrentnessError>,
        ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let (payload, destinations) = self.storage.borrow_readback().map_err(|_| {
            RuntimeErrorV1::Validation(
                fe2o3_runtime::RuntimeValidationErrorV1::InvalidBackendDescription,
            )
        })?;
        callback(RuntimeGfx942GeneratedCompletionViewV1::new(
            RuntimeGfx942GeneratedSourceV1::from_generated_storage(
                payload,
                self.authority.artifact_bytes(),
                &self.authority,
            ),
            destinations,
        ))
    }

    fn decode_registry_readback_retaining_source_v1(
        &mut self,
    ) -> Result<(), RuntimeGfx942ReadbackErrorV1> {
        self.storage
            .decode_retaining_source()
            .map_err(readback_error)
    }

    fn registry_second_completion_domain_v1(
        &self,
    ) -> Result<fe2o3_runtime::RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
        self.second
            .as_ref()
            .ok_or(RuntimeGfx942ReadbackErrorV1::InvalidStorage)?
            .domain()
            .map_err(readback_error)
    }

    fn with_registry_cycle_completion_view_v1(
        &mut self,
        cycle: u8,
        callback: impl for<'a> FnOnce(
            RuntimeGfx942GeneratedCompletionViewV1<'a, Self::CurrentnessError>,
        ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        if cycle == 0 {
            return self.with_registry_completion_view_v1(callback);
        }
        if cycle != 1 || !REPEAT {
            return Err(RuntimeErrorV1::Validation(
                fe2o3_runtime::RuntimeValidationErrorV1::InvalidBackendDescription,
            ));
        }
        let payload = self.storage.prepared();
        let destinations = self
            .second
            .as_mut()
            .ok_or(RuntimeErrorV1::Validation(
                fe2o3_runtime::RuntimeValidationErrorV1::InvalidBackendDescription,
            ))?
            .borrow_readback(payload)
            .map_err(|_| {
                RuntimeErrorV1::Validation(
                    fe2o3_runtime::RuntimeValidationErrorV1::InvalidBackendDescription,
                )
            })?;
        callback(RuntimeGfx942GeneratedCompletionViewV1::new(
            RuntimeGfx942GeneratedSourceV1::from_generated_storage(
                payload,
                self.authority.artifact_bytes(),
                &self.authority,
            ),
            destinations,
        ))
    }

    fn decode_registry_cycle_retaining_source_v1(
        &mut self,
        cycle: u8,
    ) -> Result<(), RuntimeGfx942ReadbackErrorV1> {
        if cycle == 0 {
            return self.decode_registry_readback_retaining_source_v1();
        }
        if cycle != 1 || !REPEAT {
            return Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage);
        }
        self.second
            .as_mut()
            .ok_or(RuntimeGfx942ReadbackErrorV1::InvalidStorage)?
            .decode(self.storage.prepared())
            .map_err(readback_error)
    }
}

// SAFETY: the private constructor prepays both independent frames from the same
// result account before adoption. Neither can replace the original source or
// release its debit; source and authority outlive both decoder dispositions.
unsafe impl<A: GeneratedRuntimeAuthorityV1> fe2o3_runtime::RuntimeGfx942RegistryRepeat2CarrierV1
    for GeneratedRegistryCarrierV1<A, true>
{
}
