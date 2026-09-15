use super::*;

impl AdmittedInertSemanticMirV1 {
    pub fn decode_exact_v21_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V21),
        )
    }
}
