//! Payload only; the shared V23 codec owns the closed tag-7 dispatch.

use super::*;
use crate::semantic_mir_v1::defined_matrix_issuer_v1::{
    SemanticKernelMatrixDeriveTypesV1, SemanticKernelMatrixDeriveV1, SemanticMatrixIssuerBodyV1,
};

impl CanonicalDecoderV1<'_> {
    fn matrix_issuer_body(
        &mut self,
    ) -> Result<SemanticMatrixIssuerBodyV1, SemanticMirDecodeErrorV1> {
        SemanticMatrixIssuerBodyV1::from_encoded_parts(
            SemanticFunctionIdV1(self.u32()?),
            SemanticFunctionIdentityV1(self.identity()?),
            SemanticAbiIdentityV1(self.identity()?),
            self.identity()?,
        )
        .map_err(Into::into)
    }

    pub(super) fn kernel_matrix_derive_payload(
        &mut self,
    ) -> Result<SemanticKernelMatrixDeriveV1, SemanticMirDecodeErrorV1> {
        let origin = self.matrix_issuer_body()?;
        let bridge = self.matrix_issuer_body()?;
        let current = SemanticCallableIdV1(self.u32()?);
        let source = SemanticFunctionIdentityV1(self.identity()?);
        let abi = SemanticAbiIdentityV1(self.identity()?);
        let mut ids = [SemanticTypeIdV1(0); 6];
        for id in &mut ids {
            *id = SemanticTypeIdV1(self.u32()?);
        }
        let provenance = self.kernel_capability_provenance()?;
        let brand = SemanticTypeIdentityV1(self.identity()?);
        SemanticKernelMatrixDeriveV1::from_encoded_parts(
            origin,
            bridge,
            current,
            source,
            abi,
            SemanticKernelMatrixDeriveTypesV1::new(ids),
            provenance,
            brand,
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "canonical_decode_tests.rs"]
mod tests;
