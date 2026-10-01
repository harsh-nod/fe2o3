//! Constant-time capacity observation; no transition checking or receipt replay.
use super::{InertCanonicalKirTransitionReceiptV1 as Receipt, Rows};
use crate::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}

fn payload<T: Copy>(values: &Vec<T>) -> Result<usize, Error> {
    extent(values.capacity(), size_of::<T>())
}
fn extent(capacity: usize, slot: usize) -> Result<usize, Error> {
    capacity.checked_mul(slot).ok_or(Error::Arithmetic)
}
fn sum(payloads: [usize; 10]) -> Result<usize, Error> {
    payloads.into_iter().try_fold(0_usize, |total, bytes| {
        total.checked_add(bytes).ok_or(Error::Arithmetic)
    })
}

impl Receipt {
    /// Charges all nine actual typed-row capacities and the canonical byte
    /// capacity, excluding the receipt's inline header and root visit.
    ///
    /// Ten constant-time collection observations are charged in one atomic
    /// counter operation; arithmetic, byte and item refusal leaves it unchanged.
    /// Current row types are fixed-width Copy values with no nested owned heap.
    /// Distinct equal-byte receipts remain separate owners. The enclosing owner
    /// must count its header once and must not add an old transfer receipt too.
    /// This operation allocates nothing and does not verify graph authority.
    pub fn charge_retained_heap_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self {
            input,
            output,
            rows,
            bytes,
            digest,
        } = self;
        fixed(input);
        fixed(output);
        fixed(digest);
        let Rows {
            functions,
            blocks,
            segments,
            operations,
            definitions,
            definition_outputs,
            uses,
            edges,
            edge_arguments,
        } = rows;
        let canonical: &Vec<u8> = bytes;
        let heap = sum([
            payload(functions)?,
            payload(blocks)?,
            payload(segments)?,
            payload(operations)?,
            payload(definitions)?,
            payload(definition_outputs)?,
            payload(uses)?,
            payload(edges)?,
            payload(edge_arguments)?,
            payload(canonical)?,
        ])?;
        counter.charge(heap, 10)
    }
}

#[cfg(test)]
#[path = "canonical_kir_transition_retained_storage_v1_tests.rs"]
mod tests;
