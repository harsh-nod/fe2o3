//! Read-only V12 owner composition over the unchanged V11 heap grammar.
use super::{VerifiedCanonicalKernelIrModuleV12, VerifiedCanonicalKernelIrV12};
use crate::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};

fn fixed<T: Copy>(_: &T) {}

impl VerifiedCanonicalKernelIrV12 {
    /// Charges this actual canonical byte-vector capacity, excluding this owner's
    /// inline header and root visit. Exactly one vector observation is charged.
    /// The immutable identity is inline. Failure leaves the counter unchanged.
    /// No re-encoding, identity check, allocation or admission is performed.
    pub fn charge_retained_heap_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self {
            canonical_bytes,
            identity,
        } = self;
        fixed(identity);
        let bytes: &Vec<u8> = canonical_bytes;
        counter.vector(bytes)
    }
}

impl VerifiedCanonicalKernelIrModuleV12 {
    /// Charges the real canonical byte capacity and decoded Module heap, without
    /// either owner's inline header or this wrapper's root visit.
    ///
    /// The existing Module walker includes its own root and bounds each visit.
    /// It supports the V11 ownership grammar, not every V12 payload; unsupported
    /// families return UnsupportedV11Owner, never a successful partial total.
    /// A refusal may leave a partial counter: the entire enclosing observation
    /// must then be discarded. No receipt, replay, constructor or wire changes.
    pub fn charge_retained_heap_v11(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self { canonical, module } = self;
        canonical.charge_retained_heap_storage_v1(counter)?;
        module.charge_retained_heap_v11(counter)
    }
}

#[cfg(test)]
#[path = "canonical_kir_v12_retained_storage_v1_tests.rs"]
mod tests;
