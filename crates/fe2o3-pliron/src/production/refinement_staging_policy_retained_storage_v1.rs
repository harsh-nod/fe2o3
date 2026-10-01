//! Logical key-payload observation of the original non-authoritative policy.
//! BTree node backing/capacity is deliberately not modeled.

use super::ProductionRefinementStagingPolicyV2;
use fe2o3_proof_contracts::DigestV1;
use std::{collections::BTreeSet, mem::size_of};

fn fixed<T: Copy>(_: &T) {}
fn fixed_keys<T: Copy>(_: &BTreeSet<T>) {}

impl ProductionRefinementStagingPolicyV2 {
    /// Visit logical retained BTree key payload, not physical tree allocation.
    ///
    /// Exactly one callback reports the actual set length times DigestV1 width.
    /// Copy keys own no nested heap, so no keys are scanned. Unused node slots,
    /// node headers/links/padding and allocator metadata are NOT included.
    /// This method is not a capacity/allocator upper bound.
    ///
    /// The enclosing observer charges this policy's inline header/root once
    /// (or already includes it in its parent). The Copy toolchain and BTreeSet
    /// handle belong to that header. There is no extra root callback.
    /// The callback must checked-multiply count/width, checked-add byte/item
    /// totals, charge one item and enforce explicit remaining bounds using the
    /// same enclosing ledger. Its error is returned unchanged.
    ///
    /// No allocation, iteration, cloning, signer admission or authority is
    /// performed. This observer has the same internal-proof-staging feature
    /// gate as the existing caller-selected policy; it adds no constructor.
    pub fn visit_retained_logical_key_payload_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            signer_identities,
            toolchain,
        } = self;
        fixed(toolchain);
        fixed_keys(signer_identities);
        let _: &BTreeSet<DigestV1> = signer_identities;
        visit(signer_identities.len(), size_of::<DigestV1>())
    }
}

#[cfg(test)]
#[path = "refinement_staging_policy_retained_storage_v1_tests.rs"]
mod tests;
