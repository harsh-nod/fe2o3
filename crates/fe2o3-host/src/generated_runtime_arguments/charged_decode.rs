//! Returned storage remains owned until disposal and explicit credit settlement.

use super::*;
use GeneratedRuntimeArgumentErrorV1 as Error;

enum ChargedBufferOwnerV1 {
    Detached(Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)>),
    Reserved(GeneratedRuntimeReadbackOwnerV1),
}

impl ChargedBufferOwnerV1 {
    fn buffers(&self) -> &[(Gfx942RuntimeBufferAccessV1, Vec<u8>)] {
        match self {
            Self::Detached(buffers) => buffers,
            Self::Reserved(owner) => owner.buffers(),
        }
    }

    fn dispose(&mut self) -> Result<(), Error> {
        match std::mem::replace(self, Self::Detached(Vec::new())) {
            Self::Detached(buffers) => {
                drop(buffers);
                Ok(())
            }
            Self::Reserved(owner) => owner.dispose(),
        }
    }
}

// Declaration order also applies on validation failure and decoder unwind.
struct DecodeTransaction {
    buffers: ChargedBufferOwnerV1,
    decoder: GeneratedRuntimeOutputDecoderV1,
}

impl DecodeTransaction {
    fn decode_with(mut self, after_output: impl Fn(usize)) -> Result<(), Error> {
        let gate = self
            .decoder
            .result_gate
            .as_ref()
            .ok_or(Error::BindingMismatch)?;
        let expected_count = self
            .decoder
            .expectations
            .iter()
            .filter(|expected| expected.byte_len != 0)
            .count();
        if expected_count != self.buffers.buffers().len() {
            return Err(Error::BindingMismatch);
        }
        let mut buffers = self.buffers.buffers().iter();
        for expected in &self.decoder.expectations {
            if expected.byte_len != 0 {
                let (access, bytes) = buffers.next().ok_or(Error::BindingMismatch)?;
                if !fe2o3_runtime_model::r73_generated_result_shape_v1(
                    u64::try_from(expected.byte_len).map_err(|_| Error::ByteLength)?,
                    u64::try_from(bytes.len()).map_err(|_| Error::ByteLength)?,
                    u64::try_from(bytes.capacity()).map_err(|_| Error::ByteLength)?,
                    *access == expected.access,
                ) {
                    return Err(Error::BindingMismatch);
                }
            }
            if let Some(custody) = &expected.custody {
                custody.bound_to(Some(gate))?;
            } else if !expected
                .read_credit
                .as_ref()
                .is_some_and(|credit| credit.bound_to(gate))
            {
                return Err(Error::BindingMismatch);
            }
        }
        let mut buffers = self.buffers.buffers().iter();
        for (index, expected) in self.decoder.expectations.iter().enumerate() {
            let bytes = if expected.byte_len == 0 {
                &[][..]
            } else {
                buffers.next().ok_or(Error::BindingMismatch)?.1.as_slice()
            };
            if let Some(OutputCustody::Charged(custody)) = &expected.custody {
                custody.decode(bytes, gate)?;
                after_output(index);
            }
        }
        self.buffers.dispose()?;
        for expected in &mut self.decoder.expectations {
            if let Some(credit) = expected.read_credit.take() {
                credit.release_after_disposal()?;
            }
        }
        gate.commit();
        Ok(())
    }
}

impl GeneratedRuntimeOutputDecoderV1 {
    // Data decoding remains private until native completion admission is integrated.
    #[allow(dead_code)]
    pub(crate) fn decode_charged_buffers(
        self,
        buffers: Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)>,
    ) -> Result<(), Error> {
        self.decode_charged_with(buffers, |_| {})
    }

    pub(super) fn decode_charged_with(
        self,
        buffers: Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)>,
        after_output: impl Fn(usize),
    ) -> Result<(), Error> {
        DecodeTransaction {
            buffers: ChargedBufferOwnerV1::Detached(buffers),
            decoder: self,
        }
        .decode_with(after_output)
    }
}

impl<P> GeneratedRuntimeStorageV1<P> {
    pub(super) fn decode_validated_readback_with(
        mut self,
        after_output: impl Fn(usize),
    ) -> Result<(), Error> {
        let readback = self.readback.take().ok_or(Error::BindingMismatch)?;
        let transaction = DecodeTransaction {
            buffers: ChargedBufferOwnerV1::Reserved(readback),
            decoder: self.decoder,
        };
        drop(self.payload);
        transaction.decode_with(after_output)
    }
}
