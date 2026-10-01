//! Heap-only composition of the original reconciled semantic-contract owner.

use super::ProductionReconciledMirPlironSemanticContractV1;

fn fixed<T: Copy>(_: &T) {}

impl ProductionReconciledMirPlironSemanticContractV1 {
    /// Delegate the actual contract's retained heap on the caller's ledger.
    ///
    /// The total-output and semantic reports are fixed inline Copy fields.
    /// The enclosing owner header already includes both and the contract's
    /// inline header; charge it once, not again for each field. No root visit
    /// is emitted here. The child callback contract requires checked arithmetic,
    /// explicit byte/item limits and one item per callback. First refusal is
    /// propagated unchanged; discard any partial enclosing observation.
    ///
    /// No reconciliation, graph inspection, clone, allocation, constructor,
    /// semantic validation or new authority is performed.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            contract,
            total_output,
            semantics,
        } = self;
        fixed(total_output);
        fixed(semantics);
        contract.visit_retained_heap_storage_v1(visit)
    }
}
