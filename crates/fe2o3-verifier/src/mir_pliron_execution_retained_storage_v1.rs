//! Read-only heap composition for an existing admitted execution owner.
//! No alternate constructor or test-only admitted execution is introduced.

use super::ProductionMirPlironPerCompilationVerusExecutionV1;

fn fixed<T: Copy>(_: &T) {}

impl ProductionMirPlironPerCompilationVerusExecutionV1 {
    /// Visit separately owned retained heap using the actual child owner.
    ///
    /// The report is fixed/Copy inline storage. The retained receipt delegates
    /// to the functional-proof owner. Currently the complete chain has no heap
    /// and emits no callbacks or root visits. Count this enclosing header/root
    /// exactly once; its inline report, receipt, proof, key and wire are already
    /// included. A separately retained staging policy is not inside this owner.
    ///
    /// The callback must use the same checked/bounded enclosing ledger as other
    /// children. Any first child Err is propagated unchanged; discard partial
    /// observations on Err. Do not infer authority, successful proof execution,
    /// peak/RSS, or whole ranked-roster coverage from an observation.
    ///
    /// This does not allocate, hash, validate, clone, run a runtime, or create an
    /// execution receipt. Exhaustive fields and Copy checks guard the current
    /// inline shape while the non-Copy receipt uses its own heap visitor.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self { report, retained } = self;
        fixed(report);
        retained.visit_retained_heap_storage_v1(visit)
    }
}
