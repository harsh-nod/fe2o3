//! Private exact MIR38 decoder. Fixed-size records are not source authority.
use super::*;

impl CanonicalDecoderV1<'_> {
    pub(super) fn physical_global_copy_operation_v38(
        &mut self,
        tag: u8,
    ) -> Result<SemanticCompilerIntrinsicOperationV1, SemanticMirDecodeErrorV1> {
        self.tagged("gfx942 physical-global-copy revision", 0)?;
        Ok(match tag {
            96 => SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin,
            97 => SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyLabel(
                self.tagged("physical global-copy label", 0)?,
            ),
            98 => SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyStep(
                SemanticPhysicalGlobalCopyInstructionV38::from_descriptor(self.array()?)?,
            ),
            _ => return Err(SemanticMirErrorV1::InvalidPhysicalGlobalCopyV38.into()),
        })
    }

    pub(super) fn physical_global_copy_source_v38(
        &mut self,
    ) -> Result<Option<SemanticPhysicalGlobalCopySourceV38>, SemanticMirDecodeErrorV1> {
        self.option("physical-global-copy source", |decoder| {
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
            SemanticPhysicalGlobalCopySourceV38::new(
                axes,
                mir_body,
                block_identity,
                source_signature,
                rustc_fn_abi,
                frontend,
                (raw_block, occurrence),
            )
            .map_err(|_| SemanticMirErrorV1::InvalidPhysicalGlobalCopyV38.into())
        })
    }
}
