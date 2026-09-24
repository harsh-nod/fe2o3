//! Private exact MIR39 decoder. Fixed-size records are not source authority.
use super::*;

impl CanonicalDecoderV1<'_> {
    pub(super) fn physical_lds_exchange_operation_v39(
        &mut self,
        tag: u8,
    ) -> Result<SemanticCompilerIntrinsicOperationV1, SemanticMirDecodeErrorV1> {
        self.tagged("gfx942 physical-lds-exchange revision", 0)?;
        Ok(match tag {
            99 => SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalLdsExchangeBegin(
                SemanticPhysicalLdsExchangeFrameV39::new(
                    self.u32()?,
                    self.u32()?,
                    self.u32()?,
                    self.u8()?,
                )?,
            ),
            100 => SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalLdsExchangeLabel(
                self.tagged("physical global-copy label", 0)?,
            ),
            101 => SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalLdsExchangeStep(
                SemanticPhysicalLdsExchangeInstructionV39::from_descriptor(self.array()?)?,
            ),
            _ => return Err(SemanticMirErrorV1::InvalidPhysicalLdsExchangeV39.into()),
        })
    }

    pub(super) fn physical_lds_exchange_source_v39(
        &mut self,
    ) -> Result<Option<SemanticPhysicalLdsExchangeSourceV39>, SemanticMirDecodeErrorV1> {
        self.option("physical-lds-exchange source", |decoder| {
            let mut axes = [[0; 32]; 5];
            for axis in &mut axes {
                *axis = decoder.identity()?;
            }
            let mir_body = decoder.identity()?;
            let block_identity = decoder.identity()?;
            let source_signature = decoder.identity()?;
            let rustc_fn_abi = decoder.identity()?;
            let frontend = decoder.identity()?;
            let raw_block = decoder.u32()?;
            let occurrence = decoder.u8()?;
            SemanticPhysicalLdsExchangeSourceV39::new(
                axes,
                mir_body,
                block_identity,
                source_signature,
                rustc_fn_abi,
                frontend,
                (raw_block, occurrence),
            )
            .map_err(|_| SemanticMirErrorV1::InvalidPhysicalLdsExchangeV39.into())
        })
    }
}
