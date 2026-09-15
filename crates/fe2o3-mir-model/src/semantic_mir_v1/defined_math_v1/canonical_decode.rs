//! Mount as a child of canonical_decode; these methods consume payloads only.

use super::*;
use crate::semantic_mir_v1::defined_math_v1::{
    SemanticDefinedMathBodyV1, SemanticKernelMathDeriveTypesV1, SemanticKernelMathDeriveV1,
    SemanticPolicyMathBindTypesV1, SemanticPolicyMathBindV1,
};

impl CanonicalDecoderV1<'_> {
    fn defined_math_body(&mut self) -> Result<SemanticDefinedMathBodyV1, SemanticMirDecodeErrorV1> {
        SemanticDefinedMathBodyV1::from_encoded_parts(
            SemanticFunctionIdV1(self.u32()?),
            SemanticFunctionIdentityV1(self.identity()?),
            SemanticAbiIdentityV1(self.identity()?),
            self.identity()?,
        )
        .map_err(Into::into)
    }

    pub(super) fn kernel_math_derive_payload(
        &mut self,
    ) -> Result<SemanticKernelMathDeriveV1, SemanticMirDecodeErrorV1> {
        let origin = self.defined_math_body()?;
        let bridge = self.defined_math_body()?;
        let current = SemanticCallableIdV1(self.u32()?);
        let source = SemanticFunctionIdentityV1(self.identity()?);
        let abi = SemanticAbiIdentityV1(self.identity()?);
        let mut ids = [SemanticTypeIdV1(0); 4];
        for id in &mut ids {
            *id = SemanticTypeIdV1(self.u32()?);
        }
        let provenance = self.kernel_capability_provenance()?;
        let brand = SemanticTypeIdentityV1(self.identity()?);
        SemanticKernelMathDeriveV1::from_encoded_parts(
            origin,
            bridge,
            current,
            source,
            abi,
            SemanticKernelMathDeriveTypesV1::new(ids),
            provenance,
            brand,
        )
        .map_err(Into::into)
    }

    pub(super) fn policy_math_bind_payload(
        &mut self,
    ) -> Result<SemanticPolicyMathBindV1, SemanticMirDecodeErrorV1> {
        let origin = self.defined_math_body()?;
        let mut ids = [SemanticTypeIdV1(0); 5];
        for id in &mut ids {
            *id = SemanticTypeIdV1(self.u32()?);
        }
        let provenance = self.kernel_capability_provenance()?;
        let policy = SemanticTypeIdentityV1(self.identity()?);
        let brand = SemanticTypeIdentityV1(self.identity()?);
        SemanticPolicyMathBindV1::from_encoded_parts(
            origin,
            SemanticPolicyMathBindTypesV1::new(ids),
            provenance,
            policy,
            brand,
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "canonical_decode_tests.rs"]
mod tests;
