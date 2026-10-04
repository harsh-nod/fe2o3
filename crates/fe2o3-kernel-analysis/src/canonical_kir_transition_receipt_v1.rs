//! Actual admission of inert wire rows against two exact borrowed inventories.
use super::*;
use fe2o3_kernel_ir::{
    CANONICAL_KIR_TRANSITION_CHECKER_POLICY_V1, InertCanonicalKirTransitionReceiptV1,
};

impl CheckedCanonicalKirTransitionV1<'_, '_, '_, '_> {
    /// Fixed named-rule policy actually applied by this checker, not wire authority.
    pub const fn checker_policy(&self) -> u16 {
        CANONICAL_KIR_TRANSITION_CHECKER_POLICY_V1
    }
}

/// Compares both inert endpoint coordinates, then invokes the existing fixed
/// transition checker on the actual supplied inventories and immutable wire rows.
/// No graph is cloned, decoded, selected from a digest, or admitted by this helper.
/// The returned view borrows those exact inventories, not serialized owner claims.
///
/// Input/output owners, inventories and the complete row receipt must already be
/// reserved and remain alive. Work/failure history accumulates; all exits restore
/// the incoming storage floor. Success transfers the existing checked-view receipt.
/// This establishes the same local rules as the direct checker, not a formal
/// semantic-refinement theorem, compiler-execution proof or runtime authority.
pub fn check_canonical_kir_transition_receipt_v1<'a, 'input, 'output, 'rows>(
    input: &'a Inventory<'input>,
    output: &'a Inventory<'output>,
    receipt: &'rows InertCanonicalKirTransitionReceiptV1,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirTransitionV1<'a, 'input, 'output, 'rows>,
    CanonicalKirTransitionStorageV1,
)> {
    // Two fixed digest32+length8 comparisons precede any checker allocation.
    budget.charge_work(80)?;
    if receipt.checker_policy() != CANONICAL_KIR_TRANSITION_CHECKER_POLICY_V1
        || !receipt.input_identity().matches_verified(&input.identity())
        || !receipt
            .output_identity()
            .matches_verified(&output.identity())
    {
        return Err(Error::Rule("transition receipt endpoint or policy"));
    }
    check_canonical_kir_transition_v1(input, output, receipt.candidate(), budget)
}
