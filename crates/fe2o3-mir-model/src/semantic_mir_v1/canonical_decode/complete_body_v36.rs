//! Private exact MIR36 decoder; fixed size before semantic admission.
use super::*;

impl CanonicalDecoderV1<'_> {
    pub(super) fn complete_body_packing_v36(
        &mut self,
    ) -> Result<SemanticCompilerIntrinsicOperationV1, SemanticMirDecodeErrorV1> {
        self.tagged("gfx942 complete body revision", 0)?;
        let block_count = self.u8()?;
        let instruction_count = self.u8()?;
        let mut block_words = [0u64; 4];
        let mut instruction_words = [0u64; 4];
        for word in &mut block_words {
            *word = self.u64()?;
        }
        for word in &mut instruction_words {
            *word = self.u64()?;
        }
        let packed = SemanticCompleteBodyPackingVNext {
            block_count,
            instruction_count,
            block_words,
            instruction_words,
        };
        super::super::complete_body_v36::validate_packing(packed)?;
        Ok(SemanticCompilerIntrinsicOperationV1::Gfx942CompleteBody(
            packed,
        ))
    }

    pub(super) fn complete_body_source_v36(
        &mut self,
    ) -> Result<Option<SemanticCompleteBodySourceVNext>, SemanticMirDecodeErrorV1> {
        self.option("complete body source", |decoder| {
            let mut axes = [[0u8; 32]; 5];
            for axis in &mut axes {
                *axis = decoder.identity()?;
            }
            let mir_body = decoder.identity()?;
            let block_identity = decoder.identity()?;
            let source_signature = decoder.identity()?;
            let rustc_fn_abi = decoder.identity()?;
            let frontend = decoder.identity()?;
            let raw_block = decoder.u32()?;
            SemanticCompleteBodySourceVNext::new(
                axes,
                mir_body,
                block_identity,
                source_signature,
                rustc_fn_abi,
                frontend,
                raw_block,
            )
            .map_err(|_| SemanticMirErrorV1::InvalidCompleteBodyV36.into())
        })
    }
}
