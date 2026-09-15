//! Payload-only decoder. Parent dispatch owns version and closed tag checks.
use super::*;
use crate::semantic_mir_v1::defined_matrix_v1::{
    SemanticDefinedMatrixBodyV1, SemanticDefinedMatrixIdentityV1,
    SemanticPolicyGfx950NarrowTypesV1, SemanticPolicyGfx950NarrowV1,
    SemanticPolicyMatrixBindTypesV1, SemanticPolicyMatrixBindV1,
};

impl AdmittedInertSemanticMirV1 {
    pub fn decode_exact_v23_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V23),
        )
    }
}

impl CanonicalDecoderV1<'_> {
    fn defined_matrix_body(
        &mut self,
    ) -> Result<SemanticDefinedMatrixBodyV1, SemanticMirDecodeErrorV1> {
        SemanticDefinedMatrixBodyV1::from_encoded_parts(
            SemanticFunctionIdV1(self.u32()?),
            SemanticFunctionIdentityV1(self.identity()?),
            SemanticAbiIdentityV1(self.identity()?),
            self.identity()?,
        )
        .map_err(Into::into)
    }

    fn defined_matrix_identity(
        &mut self,
    ) -> Result<SemanticDefinedMatrixIdentityV1, SemanticMirDecodeErrorV1> {
        SemanticDefinedMatrixIdentityV1::new(
            self.kernel_capability_provenance()?,
            SemanticTypeIdentityV1(self.identity()?),
            SemanticTypeIdentityV1(self.identity()?),
            SemanticTypeIdentityV1(self.identity()?),
            SemanticTypeIdentityV1(self.identity()?),
        )
        .map_err(Into::into)
    }

    pub(super) fn policy_matrix_bind_payload(
        &mut self,
    ) -> Result<SemanticPolicyMatrixBindV1, SemanticMirDecodeErrorV1> {
        self.policy_matrix_bind_identity_payload(false)
    }

    pub(super) fn policy_matrix_bind_phase_payload(
        &mut self,
    ) -> Result<SemanticPolicyMatrixBindV1, SemanticMirDecodeErrorV1> {
        self.policy_matrix_bind_identity_payload(true)
    }

    fn policy_matrix_bind_identity_payload(
        &mut self,
        phase: bool,
    ) -> Result<SemanticPolicyMatrixBindV1, SemanticMirDecodeErrorV1> {
        let origin = self.defined_matrix_body()?;
        let mut ids = [SemanticTypeIdV1(0); 5];
        for ty in &mut ids {
            *ty = SemanticTypeIdV1(self.u32()?);
        }
        let mut identity = self.defined_matrix_identity()?;
        if phase {
            identity = identity.with_execution_brand(SemanticTypeIdentityV1(self.identity()?))?;
        }
        SemanticPolicyMatrixBindV1::from_encoded_parts(
            origin,
            SemanticPolicyMatrixBindTypesV1::new(ids),
            identity,
        )
        .map_err(Into::into)
    }

    pub(super) fn policy_gfx950_narrow_payload(
        &mut self,
    ) -> Result<SemanticPolicyGfx950NarrowV1, SemanticMirDecodeErrorV1> {
        self.policy_gfx950_narrow_identity_payload(false)
    }

    pub(super) fn policy_gfx950_narrow_phase_payload(
        &mut self,
    ) -> Result<SemanticPolicyGfx950NarrowV1, SemanticMirDecodeErrorV1> {
        self.policy_gfx950_narrow_identity_payload(true)
    }

    fn policy_gfx950_narrow_identity_payload(
        &mut self,
        phase: bool,
    ) -> Result<SemanticPolicyGfx950NarrowV1, SemanticMirDecodeErrorV1> {
        let origin = self.defined_matrix_body()?;
        let projection = self.defined_matrix_body()?;
        let mut ids = [SemanticTypeIdV1(0); 7];
        for ty in &mut ids {
            *ty = SemanticTypeIdV1(self.u32()?);
        }
        let mut identity = self.defined_matrix_identity()?;
        if phase {
            identity = identity.with_execution_brand(SemanticTypeIdentityV1(self.identity()?))?;
        }
        SemanticPolicyGfx950NarrowV1::from_encoded_parts(
            origin,
            projection,
            SemanticPolicyGfx950NarrowTypesV1::new(ids),
            identity,
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "canonical_decode_tests.rs"]
mod tests;
