//! Frontend-neutral production middle-end ownership.
//!
//! Frontends stop at admitted semantic MIR plus authenticated, type-erased
//! reference effects. This crate owns ranked projection, generic verification,
//! and the transition into target-neutral lowering custody.

#![forbid(unsafe_code)]

mod mir_pliron_verus_join;
mod ranked_projection;
mod reference_effect;

pub use mir_pliron_verus_join::ProductionMirPlironVerusJoinErrorV1;
pub use ranked_projection::{
    AuthenticatedRankedVerificationRootV1, AuthenticatedRankedVerificationRosterV1,
    AuthenticatedRankedVerificationV5, ProductionRankedKernelRosterIdentityV1,
    ProductionRankedProjectionErrorV1, ProductionRankedRootInputV1, ProductionRankedRootProgramV1,
    ProductionRankedSemanticProgramV1, ProductionRankedSemanticProjectionRosterReceiptV1,
    ProductionRankedVerificationErrorV1, ProjectedAccessSourceV1,
    project_and_verify_ranked_semantic_mir_v1,
};
pub use reference_effect::{
    AuthenticatedReferenceEffectsV1, ProductionReferenceEffectErrorV1, RankedGpuWriteV2,
};

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}
