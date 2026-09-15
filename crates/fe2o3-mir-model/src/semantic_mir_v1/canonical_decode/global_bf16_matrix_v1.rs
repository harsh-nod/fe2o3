use super::*;

impl AdmittedInertSemanticMirV1 {
    pub fn decode_exact_v22_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V22),
        )
    }
}

impl CanonicalDecoderV1<'_> {
    pub(super) fn global_bf16_matrix_load(
        &mut self,
    ) -> Result<SemanticGlobalBf16MatrixLoadV1, SemanticMirDecodeErrorV1> {
        let types = SemanticGlobalBf16MatrixTypesV1 {
            matrix: SemanticTypeIdV1(self.u32()?),
            global: SemanticTypeIdV1(self.u32()?),
            lane: SemanticTypeIdV1(self.u32()?),
            fragment: SemanticTypeIdV1(self.u32()?),
            element: SemanticTypeIdV1(self.u32()?),
            index: SemanticTypeIdV1(self.u32()?),
        };
        let operand = self.mfma_operand_contract()?;
        let storage = self.mfma_storage_layout()?;
        let matrix_brand = SemanticTypeIdentityV1(self.identity()?);
        let global_brand = SemanticTypeIdentityV1(self.identity()?);
        let memory = self.capability_memory_contract()?;
        let provenance = self.kernel_capability_provenance()?;
        let source = SemanticFunctionIdentityV1(self.identity()?);
        let contract = SemanticGlobalBf16MatrixLoadV1::new(
            types,
            operand,
            matrix_brand,
            global_brand,
            provenance,
            source,
        )?;
        if storage != contract.storage_layout() || memory != contract.memory() {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi.into());
        }
        Ok(contract)
    }
}
