//! A decoder transition that cannot dispose the original registry source.

use super::*;
use GeneratedRuntimeArgumentErrorV1 as Error;

type Payload = GeneratedGfx942PersistentStorageV1;
type BorrowedRegistryReadback<'a> = (
    &'a Payload,
    &'a mut [(Gfx942RuntimeBufferAccessV1, Vec<u8>)],
);

mod repeat2;
pub(crate) use repeat2::GeneratedRegistryRepeatFrameV1;

// The original source remains in one of these fields throughout decoding,
// including decoder refusal/unwind. No credit for that source is released here.
pub(crate) struct GeneratedRegistryStorageV1 {
    original: Option<GeneratedRuntimeStorageV1<Payload>>,
    retained: Option<Payload>,
    source_credit: Option<fe2o3_resource_accounting::RetainedResourceCreditsV1>,
}

impl GeneratedRegistryStorageV1 {
    pub(crate) fn new(original: GeneratedRuntimeStorageV1<Payload>) -> Result<Self, Error> {
        let gate = original
            .decoder
            .result_gate
            .as_ref()
            .ok_or(Error::BindingMismatch)?;
        let bytes = original
            .payload
            .buffers()
            .iter()
            .try_fold(0_u64, |bytes, buffer| {
                bytes
                    .checked_add(buffer.bytes().len() as u64)
                    .ok_or(Error::ByteLength)
            })?;
        // Existing output credit covers seed plus source-copy overlap and is
        // indivisible. A separately prepaid source debit permits that result
        // credit to leave with a decoded result while original source bytes stay.
        // The temporary overlap is conservative; no old credit is split/refunded.
        let credit = gate.reserve_readback(bytes)?.retain();
        Ok(Self {
            original: Some(original),
            retained: None,
            source_credit: Some(credit),
        })
    }

    pub(crate) fn prepared(&self) -> &Payload {
        match (&self.original, &self.retained) {
            (Some(original), None) => &original.payload,
            (None, Some(retained)) => retained,
            _ => std::process::abort(),
        }
    }

    pub(crate) fn prepared_mut(&mut self) -> Option<&mut Payload> {
        self.original.as_mut().map(|original| &mut original.payload)
    }

    pub(crate) fn domain(&self) -> Result<fe2o3_runtime::RuntimeGeneratedResultDomainV1, Error> {
        self.original
            .as_ref()
            .ok_or(Error::StaleOrAliasedOutput)?
            .completion_domain_v1()
    }

    pub(crate) fn prepare_readback(&self) -> Result<GeneratedRuntimeReadbackOwnerV1, Error> {
        self.original
            .as_ref()
            .ok_or(Error::StaleOrAliasedOutput)?
            .prepare_readback()
    }

    pub(crate) fn install_readback(&mut self, readback: GeneratedRuntimeReadbackOwnerV1) {
        match self.original.as_mut() {
            Some(original) => original.install_readback(readback),
            None => std::process::abort(),
        }
    }

    pub(crate) fn borrow_readback(&mut self) -> Result<BorrowedRegistryReadback<'_>, Error> {
        self.original
            .as_mut()
            .ok_or(Error::StaleOrAliasedOutput)?
            .borrow_reserved_readback_v1()
    }

    pub(crate) fn decode_retaining_source(&mut self) -> Result<(), Error> {
        self.decode_retaining_source_with(|_| {})
    }

    pub(super) fn decode_retaining_source_with(
        &mut self,
        after_output: impl Fn(usize),
    ) -> Result<(), Error> {
        let original = self.original.as_ref().ok_or(Error::StaleOrAliasedOutput)?;
        original.validate_reserved_readback()?;
        // The admitted registry profile is exclusive whole-output fill. Do not
        // let the generic decoder refund read credits for a retained source.
        if original.decoder.expectations.iter().any(|expected| {
            expected.access != Gfx942RuntimeBufferAccessV1::WriteOnly
                || expected.read_credit.is_some()
        }) {
            return Err(Error::BindingMismatch);
        }
        let Some(mut original) = self.original.take() else {
            std::process::abort();
        };
        let Some(readback) = original.readback.take() else {
            std::process::abort();
        };
        self.retained = Some(original.payload);
        // The parent field now owns the original payload before any decoder
        // callback can fail or unwind; only copied readback credits are settled.
        charged_decode::DecodeTransaction::reserved(readback, original.decoder)
            .decode_with(after_output)
    }
}

impl Drop for GeneratedRegistryStorageV1 {
    fn drop(&mut self) {
        drop(self.original.take());
        drop(self.retained.take());
        if let Some(credit) = self.source_credit.take() {
            let _ = credit.release_after_disposal();
        }
    }
}
