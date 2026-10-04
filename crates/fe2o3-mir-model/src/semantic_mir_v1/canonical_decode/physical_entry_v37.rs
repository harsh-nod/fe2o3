//! Private exact MIR37 decoder. Fixed-size records are not source authority.
use super::*;

impl CanonicalDecoderV1<'_> {
    pub(super) fn physical_entry_operation_v37(
        &mut self,
        tag: u8,
    ) -> Result<SemanticCompilerIntrinsicOperationV1, SemanticMirDecodeErrorV1> {
        self.tagged("gfx942 physical-entry revision", 0)?;
        Ok(match tag {
            93 => SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin,
            94 => SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryLabel(self.u8()?),
            95 => SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryStep(
                SemanticPhysicalEntryInstructionV37::from_descriptor(self.array()?)?,
            ),
            _ => return Err(SemanticMirErrorV1::InvalidPhysicalEntryV37.into()),
        })
    }

    pub(super) fn physical_entry_source_v37(
        &mut self,
    ) -> Result<Option<SemanticPhysicalEntrySourceV37>, SemanticMirDecodeErrorV1> {
        self.option("physical-entry source", |decoder| {
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
            SemanticPhysicalEntrySourceV37::new(
                axes,
                mir_body,
                block_identity,
                source_signature,
                rustc_fn_abi,
                frontend,
                (raw_block, occurrence),
            )
            .map_err(|_| SemanticMirErrorV1::InvalidPhysicalEntryV37.into())
        })
    }
}
