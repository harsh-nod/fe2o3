//! Read-only complete retained Policy6 tree within the supported Module grammar.
use super::{CheckedCanonicalKernelIrOwnerPolicy6V1, Policy6ExecutionWitnessV1};
use fe2o3_kernel_ir::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error,
    LogicalStorageLimitsV1 as Limits,
};
use std::mem::size_of;

/// One immutable Policy6 owner observation, not its reservation receipt.
/// The root header includes every nested inline header and fixed witness once.
/// Counts actual Vec/String capacity and the existing Module logical tree
/// payload convention. Excludes allocator overhead, compiler arenas, borrowed
/// B, construction temporaries, observer stack/report, active-pipeline peaks
/// and RSS. Equal-content allocations are not deduplicated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Policy6RetainedLogicalStorageV1 {
    pub inline_bytes: usize,
    pub prefix_owned_bytes: usize,
    pub continuation_owned_bytes: usize,
    pub total_bytes: usize,
    pub visited_items: usize,
}

impl CheckedCanonicalKernelIrOwnerPolicy6V1 {
    fn charge_parts_v11(&self, counter: &mut Counter) -> Result<(usize, usize), Error> {
        let Self {
            prefix,
            continuation,
            execution,
            retained,
        } = self;
        let Policy6ExecutionWitnessV1 { bytes } = execution;
        let _: &[u8; super::POLICY6_EXECUTION_RECORD_BYTES_V1] = bytes;
        let _: &usize = retained;
        let before = counter.bytes();
        prefix.charge_retained_heap_v11(counter)?;
        let prefix_owned_bytes = counter
            .bytes()
            .checked_sub(before)
            .ok_or(Error::Arithmetic)?;
        let before = counter.bytes();
        continuation.charge_retained_heap_v11(counter)?;
        let continuation_owned_bytes = counter
            .bytes()
            .checked_sub(before)
            .ok_or(Error::Arithmetic)?;
        Ok((prefix_owned_bytes, continuation_owned_bytes))
    }

    /// Bounded observation of this complete retained C/S/O/I owner tree.
    /// Charges its one enclosing header and one root visit, then every owned
    /// heap through the same bounded counter. Native B and O audit Vecs are
    /// separate retained copies; original borrowed B is not counted.
    ///
    /// This uses the existing V11 Module ownership grammar. Any unsupported
    /// newer payload, arithmetic overflow, byte refusal or item refusal returns
    /// Err and no report. It never changes admission or stored budget semantics,
    /// and cannot establish the recipe's active-pipeline or peak-storage target.
    pub fn retained_logical_storage_v11(
        &self,
        limits: Limits,
    ) -> Result<Policy6RetainedLogicalStorageV1, Error> {
        let mut counter = Counter::new(limits);
        let inline_bytes = size_of::<Self>();
        counter.charge(inline_bytes, 1)?;
        let (prefix_owned_bytes, continuation_owned_bytes) = self.charge_parts_v11(&mut counter)?;
        Ok(Policy6RetainedLogicalStorageV1 {
            inline_bytes,
            prefix_owned_bytes,
            continuation_owned_bytes,
            total_bytes: counter.bytes(),
            visited_items: counter.items(),
        })
    }

    /// Heap-only enclosing-owner composition. Excludes this complete inline
    /// header and root visit; all nested collection/Module visits are included.
    /// On Err the counter may be partial: discard the complete enclosing
    /// observation. No unbounded preliminary traversal or scratch owner exists.
    pub fn charge_retained_heap_v11(&self, counter: &mut Counter) -> Result<(), Error> {
        self.charge_parts_v11(counter).map(|_| ())
    }
}

#[cfg(test)]
#[path = "checked_optimization_policy6_retained_storage_v1_tests.rs"]
mod tests;
