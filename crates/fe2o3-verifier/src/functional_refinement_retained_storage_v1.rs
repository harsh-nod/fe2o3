//! Heap-only composition for the actual locally imported receipt owner.

use super::RetainedImportedFunctionalRefinementReceiptV2;

fn fixed<T: Copy>(_: &T) {}

impl RetainedImportedFunctionalRefinementReceiptV2 {
    /// Visit this retained receipt's separately owned heap payloads.
    ///
    /// The fixed key and wire are inline; the actual imported proof is delegated
    /// to its owner-defined visitor. Today that child also owns no heap, so no
    /// callback or root visit occurs. The enclosing observer must account this
    /// owner's entire header/root once (or include it in an enclosing header),
    /// not add the inline proof, key and wire headers a second time.
    ///
    /// Use the same bounded checked-arithmetic callback for every child; any
    /// first Err is propagated unchanged and partial observations must be
    /// discarded. This method neither reimports nor validates the receipt,
    /// creates authority, clones owners, allocates nor runs a proof/runtime.
    /// Importer/policy/runtime temporary owners are outside this retained value.
    /// This is not allocator, stack, peak, RSS or whole-roster accounting.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            proof,
            verifying_key,
            wire,
        } = self;
        fixed(verifying_key);
        fixed(wire);
        proof.visit_retained_heap_storage_v1(visit)
    }
}

#[cfg(test)]
#[path = "functional_refinement_retained_storage_v1_tests.rs"]
mod tests;
