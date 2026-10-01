//! Bounded heap-only composition of the existing exact occurrence-row counter.
use super::KirNeutralOccurrenceRowsV1 as Rows;
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};
use std::mem::size_of;

fn fixed_elements<T: Copy>(_: &Vec<T>) {}

impl Rows {
    /// Charges actual capacities of all nine flat typed-row vectors, atomically.
    ///
    /// The enclosing owner counts its inline header once, including these Vec
    /// headers. This method excludes that header and this row owner's root visit
    /// and charges nine constant-time collection observations, even for empty
    /// vectors. It does not scan rows, allocate, derive candidates or check a
    /// transition. Equal-content row owners remain distinct allocations.
    /// Any arithmetic, byte or item refusal leaves the counter unchanged.
    pub fn charge_retained_heap_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self {
            functions,
            blocks,
            segments,
            operations,
            definitions,
            definition_outputs,
            uses,
            edges,
            edge_arguments,
        } = self;
        fixed_elements(functions);
        fixed_elements(blocks);
        fixed_elements(segments);
        fixed_elements(operations);
        fixed_elements(definitions);
        fixed_elements(definition_outputs);
        fixed_elements(uses);
        fixed_elements(edges);
        fixed_elements(edge_arguments);
        // The unchanged helper counts the same nine actual capacities and
        // exactly one header, with checked arithmetic and no nested payloads.
        let heap = self
            .retained_storage()
            .map_err(|_| Error::Arithmetic)?
            .checked_sub(size_of::<Self>())
            .ok_or(Error::Arithmetic)?;
        counter.charge(heap, 9)
    }
}

#[cfg(test)]
#[path = "kir_occurrence_retained_storage_v1_tests.rs"]
mod tests;
