//! Read-only logical storage of the inert production locator, not its Context.
//!
//! The walk itself performs no admission, graph verification, serialization,
//! mutation, or allocation. This is one owner component, not complete semantic MIR/SSA/session or
//! recipe storage, allocator telemetry, peak memory, or RSS.

use std::mem::size_of;

use super::{
    MirProductionBlockLocatorV1, MirProductionFunctionLocatorV1, MirProductionModuleLocatorV1,
    MirProductionStatementLocatorV1, MirProductionSuccessorArcV1, MirProductionTerminatorLocatorV1,
};

impl MirProductionModuleLocatorV1 {
    /// Visits one enclosing locator header and every owned Vec's actual capacity.
    ///
    /// The callback receives `(count, element_width)`. It must check multiplication
    /// and cumulative byte/item additions and refuse before returning success if
    /// an observation limit is exceeded. Each callback is one owner/collection
    /// visit, including empty collections. The first refusal is returned unchanged;
    /// the callback may then contain a partial observation, which must be discarded.
    ///
    /// Fixed IDs, digest, Vec headers, block/terminator headers and fixed leaf rows
    /// are included exactly once by the enclosing header or parent Vec slots.
    /// Spare capacity counts; allocator metadata and temporary/stack work do not.
    /// A successful observation grants no compiler, proof, load or launch authority.
    pub fn visit_logical_retained_storage_v1<E>(
        &self,
        visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(1, size_of::<Self>())?;
        self.visit_logical_retained_buffers_v1(visit)
    }

    /// Visits the same owned buffers without charging the enclosing locator header.
    ///
    /// Use when that header is already included in an enclosing owner or collection.
    /// The first `(0, 1)` callback still charges an owner visit. Checked arithmetic,
    /// explicit limits and incomplete-observation rules are identical to the full
    /// method. This does not follow graph handles or measure a Pliron Context.
    pub fn visit_logical_retained_heap_v1<E>(
        &self,
        visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(0, 1)?;
        self.visit_logical_retained_buffers_v1(visit)
    }

    fn visit_logical_retained_buffers_v1<E>(
        &self,
        visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            semantic_sha256,
            functions,
        } = self;
        fixed(semantic_sha256);
        fixed_type::<MirProductionStatementLocatorV1>();
        fixed_type::<MirProductionSuccessorArcV1>();
        visit(
            functions.capacity(),
            size_of::<MirProductionFunctionLocatorV1>(),
        )?;
        for function in functions {
            let MirProductionFunctionLocatorV1 {
                function_id,
                entry_block_id,
                blocks,
            } = function;
            fixed(function_id);
            fixed(entry_block_id);
            visit(blocks.capacity(), size_of::<MirProductionBlockLocatorV1>())?;
            for block in blocks {
                let MirProductionBlockLocatorV1 {
                    block_id,
                    statements,
                    terminator,
                } = block;
                fixed(block_id);
                let MirProductionTerminatorLocatorV1 { successors } = terminator;
                visit(
                    statements.capacity(),
                    size_of::<MirProductionStatementLocatorV1>(),
                )?;
                visit(
                    successors.capacity(),
                    size_of::<MirProductionSuccessorArcV1>(),
                )?;
            }
        }
        Ok(())
    }
}

fn fixed<T: Copy>(_: &T) {}
fn fixed_type<T: Copy>() {}

#[cfg(test)]
#[path = "locator_storage_v1_tests.rs"]
mod tests;
