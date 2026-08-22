//! Exact profile namespace and source-identity admission.

/// Canonical identity string whose SHA-256 is the attributed namespace.
pub const MOE_PROFILE_IDENTITY_V1: &str = "fe2o3.moe_top2_v1.logits_f32.t8_e4_k2.capacity4.token_major.lower_expert_ties.stable_drop.gfx942_xnack_minus.wave64";

/// SHA-256 of [`MOE_PROFILE_IDENTITY_V1`].
pub const MOE_KERNEL_NAMESPACE_V1: &str =
    "4180ef61545684e646bd5227333e7514d22a2d379d7d657397df4d41f7a192d1";

/// Reviewed SHA-256 of `src/kernel.rs`.
///
/// This lives outside `kernel.rs` to avoid a circular self-hash.
pub const MOE_KERNEL_SOURCE_SHA256_V1: &str =
    "0260f144150e6fee7d9bd6a3d919e99ded0e43666509770f6e6186f5100fee25";

/// Exact source-identity mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIdentityMismatchV1;

/// Admits only the reviewed exact `src/kernel.rs` SHA-256.
pub fn validate_kernel_source_identity_v1(
    actual_sha256: &str,
) -> Result<(), SourceIdentityMismatchV1> {
    if actual_sha256 == MOE_KERNEL_SOURCE_SHA256_V1 {
        Ok(())
    } else {
        Err(SourceIdentityMismatchV1)
    }
}
