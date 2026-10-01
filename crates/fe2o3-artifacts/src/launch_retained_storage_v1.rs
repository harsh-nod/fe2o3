//! Complete zero-heap observation of the current launch-contract owner.
use super::LaunchContract;

fn fixed<T: Copy>(_: &T) {}

impl LaunchContract {
    /// Visits this fixed-size owner once as the zero-byte extent (0, 1).
    ///
    /// The enclosing caller already owns its inline header. The callback must
    /// checked-multiply/add using the same explicitly bounded byte/item ledger,
    /// including one visit even though the byte extent is zero. Its first error
    /// is returned unchanged; discard incomplete observations. No construction,
    /// validation, canonical bytes, launch authority or allocation is changed.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            rank,
            block_size,
            max_grid,
            static_shared_memory_bytes,
            max_dynamic_shared_memory_bytes,
        } = self;
        fixed(rank);
        fixed(block_size);
        fixed(max_grid);
        fixed(static_shared_memory_bytes);
        fixed(max_dynamic_shared_memory_bytes);
        visit(0, 1)
    }
}
