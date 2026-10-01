//! Complete retained logical-storage observation for the immutable V6/V11 snapshot.

use super::AuthoringSnapshotV1;
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1, LogicalStorageLimitsV1};
use std::mem::size_of;

/// One root header plus actual owned Vec/String capacities, Box payloads and
/// BTree key/value payloads. Not serialized length, physical tree allocation,
/// allocator overhead, temporary/peak memory or RSS. This report changes no
/// source/canonical/runtime authority and is not part of existing wire schemas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthoringSnapshotRetainedStorageV1 {
    pub inline_bytes: usize,
    pub bundle_owned_bytes: usize,
    pub module_owned_bytes: usize,
    pub source_map_owned_bytes: usize,
    pub operation_index_owned_bytes: usize,
    pub definition_index_owned_bytes: usize,
    pub source_site_index_owned_bytes: usize,
    pub source_file_index_owned_bytes: usize,
    pub total_bytes: usize,
    pub visited_items: usize,
}

fn part(
    counter: &mut LogicalStorageCounterV1,
    walk: impl FnOnce(&mut LogicalStorageCounterV1) -> Result<(), LogicalStorageErrorV1>,
) -> Result<usize, LogicalStorageErrorV1> {
    let before = counter.bytes();
    walk(counter)?;
    counter
        .bytes()
        .checked_sub(before)
        .ok_or(LogicalStorageErrorV1::Arithmetic)
}

impl AuthoringSnapshotV1 {
    /// Reads every retained owner under a shared, explicit observation budget.
    /// No constructor limit is changed; pass max_bytes=None to measure instead
    /// of imposing a target. Item limits bound the allocation-free traversal.
    /// Failure returns no successful partial report. The snapshot is unchanged.
    pub fn retained_logical_storage_v1(
        &self,
        limits: LogicalStorageLimitsV1,
    ) -> Result<AuthoringSnapshotRetainedStorageV1, LogicalStorageErrorV1> {
        let Self {
            bundle,
            module,
            source_map,
            operations,
            definitions,
            source_sites,
            source_files,
        } = self;
        let mut counter = LogicalStorageCounterV1::new(limits);
        let inline_bytes = size_of::<Self>();
        counter.charge(inline_bytes, 1)?;
        let bundle_owned_bytes = part(&mut counter, |c| bundle.charge_retained_heap_v6(c))?;
        let module_owned_bytes = part(&mut counter, |c| module.charge_retained_heap_v11(c))?;
        let source_map_owned_bytes = part(&mut counter, |c| source_map.charge_retained_heap_v2(c))?;
        let operation_index_owned_bytes = part(&mut counter, |c| c.vector(operations))?;
        let definition_index_owned_bytes = part(&mut counter, |c| {
            c.vector(definitions)?;
            for definition_map in definitions {
                c.map(definition_map)?;
            }
            Ok(())
        })?;
        let source_site_index_owned_bytes = part(&mut counter, |c| c.map(source_sites))?;
        let source_file_index_owned_bytes = part(&mut counter, |c| c.map(source_files))?;
        Ok(AuthoringSnapshotRetainedStorageV1 {
            inline_bytes,
            bundle_owned_bytes,
            module_owned_bytes,
            source_map_owned_bytes,
            operation_index_owned_bytes,
            definition_index_owned_bytes,
            source_site_index_owned_bytes,
            source_file_index_owned_bytes,
            total_bytes: counter.bytes(),
            visited_items: counter.items(),
        })
    }
}
