//! Heap-only observation of one strictly imported functional receipt.
//! This does not inspect the separately owned importer or grant compiler authority.

use super::ImportedFunctionalRefinementProofV2;

fn fixed<T: Copy>(_: &T) {}

impl ImportedFunctionalRefinementProofV2 {
    /// Visit the separately owned heap payloads as (element count, width).
    ///
    /// This owner currently has no heap payloads: every retained field is fixed
    /// inline storage. Consequently this method invokes no callbacks, including
    /// no root visit. The enclosing observer must charge this owner's header and
    /// root item once, before calling, or include them in its enclosing header.
    /// Do not add the fields' sizes again on top of that header.
    ///
    /// The exhaustive field pattern and Copy guards below force a source review
    /// if a field is added or an existing field acquires ordinary owned heap.
    /// They do not make this move-only owner Copy or Clone.
    ///
    /// The callback has the same contract as other heap visitors: an enclosing
    /// observer uses checked arithmetic and explicit byte/item bounds, stops at
    /// the first Err, and discards partial observations. Currently no callback
    /// refusal can occur because there are zero heap visits. The importer's
    /// policy, deduplication set, signing/runtime state and earlier temporary
    /// buffers are separate owners, not retained by this proof.
    ///
    /// No allocation, cloning, hashing, validation, admission or authority is
    /// performed. This is logical retained payload only, not peak or RSS.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        _visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            receipt_identity,
            signer_identity,
            binding,
            toolchain,
            execution_identity,
            boundary,
        } = self;
        fixed(receipt_identity);
        fixed(signer_identity);
        fixed(binding);
        fixed(toolchain);
        fixed(execution_identity);
        fixed(boundary);
        Ok(())
    }
}

#[cfg(test)]
#[path = "imported_functional_retained_storage_v1_tests.rs"]
mod tests;
