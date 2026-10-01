//! Complete supported-subset tail observation, never an active-pipeline peak.
use super::OwnedU32LocalOrderContinuationV1 as Tail;
use fe2o3_kernel_ir::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error,
    LogicalStorageLimitsV1 as Limits,
};
use std::mem::size_of;

/// One actual tail header plus its output and inert receipt heap payloads.
///
/// Vector/String capacities are observed; ordered-set payloads retain the
/// existing Module walker's logical-entry convention. Allocator metadata,
/// observer scratch, the original input/prefix, compiler arenas, other recipe
/// owners and earlier/later temporaries are excluded. This is not a peak, RSS,
/// full continued-owner result, 128 MiB comparison, or compiler authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedU32LocalOrderRetainedStorageV1 {
    pub inline_bytes: usize,
    pub output_owned_bytes: usize,
    pub receipt_owned_bytes: usize,
    pub total_bytes: usize,
    pub visited_items: usize,
}

fn fixed<T: Copy>(_: &T) {}

impl Tail {
    fn charge_retained_parts_v11(&self, counter: &mut Counter) -> Result<(usize, usize), Error> {
        let Self {
            output,
            receipt,
            region,
            preference,
            retained,
        } = self;
        fixed(region);
        fixed(preference);
        fixed(retained);
        let before_output = counter.bytes();
        output.charge_retained_heap_v11(counter)?;
        let output_bytes = counter
            .bytes()
            .checked_sub(before_output)
            .ok_or(Error::Arithmetic)?;
        let before_receipt = counter.bytes();
        receipt.charge_retained_heap_storage_v1(counter)?;
        let receipt_bytes = counter
            .bytes()
            .checked_sub(before_receipt)
            .ok_or(Error::Arithmetic)?;
        Ok((output_bytes, receipt_bytes))
    }

    /// Observes this retained tail with caller-supplied byte/item bounds.
    ///
    /// The enclosing tail header (including both nested owner headers) is
    /// charged once, with one tail-root visit. Heap traversal then charges the
    /// canonical vector, Module visits, and ten receipt vectors. No report is
    /// returned on any refusal, including unsupported post-V11 ownership.
    /// Does not replay, allocate, encode, hash or inspect a compiler source.
    pub fn retained_logical_storage_v11(
        &self,
        limits: Limits,
    ) -> Result<OwnedU32LocalOrderRetainedStorageV1, Error> {
        let mut counter = Counter::new(limits);
        let inline_bytes = size_of::<Self>();
        counter.charge(inline_bytes, 1)?;
        let (output_owned_bytes, receipt_owned_bytes) =
            self.charge_retained_parts_v11(&mut counter)?;
        Ok(OwnedU32LocalOrderRetainedStorageV1 {
            inline_bytes,
            output_owned_bytes,
            receipt_owned_bytes,
            total_bytes: counter.bytes(),
            visited_items: counter.items(),
        })
    }

    /// Adds only this tail's owned heap to a bounded enclosing observation.
    ///
    /// Excludes the tail header and tail-root visit; the nested Module walker
    /// still includes its own root visit. The caller must count the enclosing
    /// header once, including all nested inline fields. Do not add transfer
    /// receipts or the standalone total for this same tail.
    ///
    /// Each traversal uses the supplied counter's remaining limits. No unbounded
    /// preliminary walk or local unlimited ledger is used. On an error the
    /// counter may be partial and MUST be discarded with the enclosing result.
    pub fn charge_retained_heap_v11(&self, counter: &mut Counter) -> Result<(), Error> {
        self.charge_retained_parts_v11(counter).map(|_| ())
    }
}

#[cfg(test)]
#[path = "checked_u32_local_order_retained_storage_v1_tests.rs"]
mod tests;
