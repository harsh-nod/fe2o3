//! Exact profile namespace and source-identity admission.

/// Canonical identity string whose SHA-256 is the attributed namespace.
pub const FLASH_ATTENTION_PROFILE_IDENTITY_V1: &str = "fe2o3.flash_attention_v1.causal.qkv_f32.b1_h1_n8_d16.row_major.scale_0p25.gfx942_xnack_minus.wave64";

/// SHA-256 of [`FLASH_ATTENTION_PROFILE_IDENTITY_V1`].
pub const FLASH_ATTENTION_KERNEL_NAMESPACE_V1: &str =
    "4dfe870bb76dd32b49144ee70ec4925eab8677b7cbd1a1bfe99fa2294f85fec8";

/// Reviewed SHA-256 of `src/kernel.rs`.
///
/// The source-identity test recomputes this digest from the exact checked-in
/// bytes. It is intentionally stored outside `kernel.rs` to avoid a circular
/// self-hash.
pub const FLASH_ATTENTION_KERNEL_SOURCE_SHA256_V1: &str =
    "c7a94f86b4ce08043d127cb87c8b521c5bb554b8cdadedf710e327b55d60d8b0";

/// Exact source-identity mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIdentityMismatchV1;

/// Admits only the reviewed exact `src/kernel.rs` SHA-256.
pub fn validate_kernel_source_identity_v1(
    actual_sha256: &str,
) -> Result<(), SourceIdentityMismatchV1> {
    if actual_sha256 == FLASH_ATTENTION_KERNEL_SOURCE_SHA256_V1 {
        Ok(())
    } else {
        Err(SourceIdentityMismatchV1)
    }
}
