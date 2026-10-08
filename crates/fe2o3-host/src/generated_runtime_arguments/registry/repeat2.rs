//! A fresh data-only result frame; no second source or native authority.

use super::*;
use crate::generated_runtime_results::{ChargedOutputCustodyV1, GeneratedRuntimeChargedResultV1};

pub(crate) struct GeneratedRegistryRepeatFrameV1 {
    readback: Option<GeneratedRuntimeReadbackOwnerV1>,
    decoder: Option<GeneratedRuntimeOutputDecoderV1>,
}

impl GeneratedRegistryRepeatFrameV1 {
    pub(crate) fn prepare(
        original: &GeneratedRuntimeStorageV1<Payload>,
        budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<(Self, GeneratedRuntimeChargedResultV1<u32>), Error> {
        if !matches!(
            original.payload.invocation_binding(),
            fe2o3_runtime::Gfx942RuntimeInvocationBindingV1::NativeConditionalFill64V1 { .. }
        ) {
            return Err(Error::BindingMismatch);
        }
        Self::prepare_frame(original, budget)
    }

    #[cfg(test)]
    pub(crate) fn prepare_data_only_for_test(
        original: &GeneratedRuntimeStorageV1<Payload>,
        budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<(Self, GeneratedRuntimeChargedResultV1<u32>), Error> {
        Self::prepare_frame(original, budget)
    }

    fn prepare_frame(
        original: &GeneratedRuntimeStorageV1<Payload>,
        budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<(Self, GeneratedRuntimeChargedResultV1<u32>), Error> {
        let [expected] = original.decoder.expectations.as_slice() else {
            return Err(Error::BindingMismatch);
        };
        if expected.byte_len == 0
            || !expected.byte_len.is_multiple_of(4)
            || expected.access != Gfx942RuntimeBufferAccessV1::WriteOnly
            || expected.read_credit.is_some()
            || !matches!(expected.custody, Some(OutputCustody::Charged(_)))
        {
            return Err(Error::BindingMismatch);
        }
        let elements = expected.byte_len / 4;
        let access = Gfx942RuntimeBufferAccessV1::WriteOnly;
        let (custody, observer) = ChargedOutputCustodyV1::new::<u32>(elements);
        let descriptor = ResultDescriptorV1::new::<u32>(elements, access, Some(&custody))?;
        let mut preflight = ResultPreflightV1::new()?;
        preflight.push(ResultDescriptorV1::new::<u32>(
            elements,
            access,
            Some(&custody),
        )?)?;
        let mut binding = preflight.reserve(budget)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(elements)
            .map_err(|_| Error::Allocation)?;
        if values.capacity() != elements {
            return Err(Error::Allocation);
        }
        values.resize(elements, 0_u32);
        custody.bind_seed(values.into_boxed_slice(), binding.take(&descriptor)?)?;
        if !binding.complete() {
            return Err(Error::BindingMismatch);
        }
        let mut expectations = Vec::new();
        expectations
            .try_reserve_exact(1)
            .map_err(|_| Error::Allocation)?;
        expectations.push(OwnedBufferExpectation {
            byte_len: expected.byte_len,
            access,
            custody: Some(OutputCustody::Charged(custody)),
            read_credit: None,
        });
        let decoder = GeneratedRuntimeOutputDecoderV1 {
            expectations,
            result_gate: Some(Arc::clone(&binding.gate)),
        };
        let readback =
            readback::prepare_for(&original.payload, &decoder, readback::allocate_readback)?;
        Ok((
            Self {
                readback: Some(readback),
                decoder: Some(decoder),
            },
            observer,
        ))
    }

    pub(crate) fn domain(&self) -> Result<fe2o3_runtime::RuntimeGeneratedResultDomainV1, Error> {
        let gate = self
            .decoder
            .as_ref()
            .and_then(|decoder| decoder.result_gate.as_ref())
            .ok_or(Error::StaleOrAliasedOutput)?;
        if gate.ready() {
            return Err(Error::StaleOrAliasedOutput);
        }
        Ok(fe2o3_runtime::RuntimeGeneratedResultDomainV1::from_owner(
            Arc::clone(gate),
        ))
    }

    pub(crate) fn borrow_readback<'a>(
        &'a mut self,
        payload: &Payload,
    ) -> Result<&'a mut [(Gfx942RuntimeBufferAccessV1, Vec<u8>)], Error> {
        self.readback
            .as_mut()
            .ok_or(Error::StaleOrAliasedOutput)?
            .borrow_for(
                payload,
                self.decoder.as_ref().ok_or(Error::StaleOrAliasedOutput)?,
            )
    }

    pub(crate) fn decode(&mut self, payload: &Payload) -> Result<(), Error> {
        self.decode_with(payload, |_| {})
    }

    pub(crate) fn decode_with(
        &mut self,
        payload: &Payload,
        after_output: impl Fn(usize),
    ) -> Result<(), Error> {
        readback::validate_shape(
            payload,
            self.decoder.as_ref().ok_or(Error::StaleOrAliasedOutput)?,
            self.readback.as_ref(),
        )?;
        let Some(readback) = self.readback.take() else {
            std::process::abort()
        };
        let Some(decoder) = self.decoder.take() else {
            std::process::abort()
        };
        // Only copied bytes/decoder move. The carrier still roots the original
        // source and authority, including if the second decoder refuses/unwinds.
        charged_decode::DecodeTransaction::reserved(readback, decoder).decode_with(after_output)
    }
}
