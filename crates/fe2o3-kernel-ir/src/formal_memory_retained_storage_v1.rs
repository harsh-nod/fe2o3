//! Actual retained heap of one formal-memory report, not proof/admission.
use super::{
    FormalAllocationParameter, FormalBoundsRequirement, FormalMemoryAccess,
    FormalMemoryObligations, InterInvocationConflictRequirement, RuntimeAliasRequirement,
};
use crate::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}
fn fixed_rows<T: Copy>(_: &Vec<T>) {}

// These two row types are not Copy, despite retaining only inline Copy fields.
// Exhaustive static guards avoid scanning every row merely to count capacity.
fn allocation_row_shape(row: &FormalAllocationParameter) {
    let FormalAllocationParameter {
        identity,
        value,
        kind,
        address_space,
        access,
    } = row;
    fixed(identity);
    fixed(value);
    fixed(kind);
    fixed(address_space);
    fixed(access);
}
fn access_row_shape(row: &FormalMemoryAccess) {
    let FormalMemoryAccess {
        location,
        allocation,
        kind,
        address_space,
        byte_offset,
        byte_width,
        alignment,
        invocations,
        domain,
    } = row;
    fixed(location);
    fixed(allocation);
    fixed(kind);
    fixed(address_space);
    fixed(byte_offset);
    fixed(byte_width);
    fixed(alignment);
    fixed(invocations);
    fixed(domain);
}
fn vector_bytes<T>(capacity: usize) -> Result<usize, Error> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(Error::Arithmetic)
}
fn heap_bytes(owner: &FormalMemoryObligations) -> Result<usize, Error> {
    let FormalMemoryObligations {
        kernel,
        entry,
        index_width,
        invocations,
        allocations,
        accesses,
        bounds_requirements,
        runtime_alias_requirements,
        inter_invocation_conflicts,
    } = owner;
    fixed(index_width);
    fixed(invocations);
    let _: &Vec<FormalAllocationParameter> = allocations;
    let _: &Vec<FormalMemoryAccess> = accesses;
    let _: fn(&FormalAllocationParameter) = allocation_row_shape;
    let _: fn(&FormalMemoryAccess) = access_row_shape;
    fixed_rows(bounds_requirements);
    fixed_rows(runtime_alias_requirements);
    fixed_rows(inter_invocation_conflicts);
    // Both name types are private single-String wrappers. Their existing
    // observers return real String capacity, not visible/serialized length.
    let mut bytes = kernel
        .retained_capacity_bytes()
        .checked_add(entry.retained_capacity_bytes())
        .ok_or(Error::Arithmetic)?;
    for extent in [
        vector_bytes::<FormalAllocationParameter>(allocations.capacity())?,
        vector_bytes::<FormalMemoryAccess>(accesses.capacity())?,
        vector_bytes::<FormalBoundsRequirement>(bounds_requirements.capacity())?,
        vector_bytes::<RuntimeAliasRequirement>(runtime_alias_requirements.capacity())?,
        vector_bytes::<InterInvocationConflictRequirement>(inter_invocation_conflicts.capacity())?,
    ] {
        bytes = bytes.checked_add(extent).ok_or(Error::Arithmetic)?;
    }
    Ok(bytes)
}

impl FormalMemoryObligations {
    /// Charges this report's two actual name capacities and five actual Vec
    /// capacities, atomically, with seven collection visits.
    ///
    /// Excludes this report's inline header and root visit. When composing a
    /// Box<[FormalMemoryObligations]>, its enclosing walker must charge the
    /// boxed element headers once and bound each initialized report visit;
    /// equal-content reports in separate boxes remain separate allocations.
    ///
    /// No row scan, clone, allocation, derivation, validation, serialization or
    /// proof execution occurs. Byte/item/arithmetic refusal leaves the supplied
    /// counter unchanged. A count does not authenticate launch inputs, prove
    /// obligations, cover an enclosing analysis enum's reasons, or measure
    /// allocator/peak/RSS/active-pipeline storage.
    pub fn charge_retained_heap_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        counter.charge(heap_bytes(self)?, 7)
    }
}

#[cfg(test)]
#[path = "formal_memory_retained_storage_v1_tests.rs"]
mod tests;
