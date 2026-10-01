//! Read-only bounded composition of the actual checked native owner.
use super::CheckedNeutralKernelIrOwnerPolicy3V1;
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};
impl CheckedNeutralKernelIrOwnerPolicy3V1 {
    /// Charges every retained heap allocation, excluding this owner's complete
    /// inline header and root visit. Includes the decoded Module root, canonical
    /// bytes, pass report, bridge, map, occurrences and actual input-audit Vec
    /// capacity. The original borrowed input is excluded, but its owned audit
    /// copy is not; equal-content output/history allocations remain separate.
    ///
    /// Uses the existing bounded V11 Module ownership grammar. Unsupported
    /// newer payloads refuse, never return a successful partial count. On any
    /// error the counter can be partial: discard the entire enclosing
    /// observation. No admission, wire, reservation or execution is changed.
    pub fn charge_retained_heap_v11(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self { parts } = self;
        parts.charge_retained_heap_v11(counter)
    }
}

#[cfg(test)]
#[path = "neutral_policy3_retained_storage_v1_tests.rs"]
mod tests;
