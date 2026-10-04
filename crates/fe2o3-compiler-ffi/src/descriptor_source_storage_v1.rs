use super::*;
use crate::logical_retained_storage_v1::{
    CompilerFfiLogicalStorageErrorV1, CompilerFfiLogicalStorageLimitsV1,
    CompilerFfiLogicalStorageV1, Counter, StorageResult, inline, observe,
};

impl CompilerDescriptorSourceV1 {
    /// Complete root report: decoded V1 table owners plus canonical Vec capacity.
    pub fn logical_retained_storage_v1(
        &self,
        limits: CompilerFfiLogicalStorageLimitsV1,
    ) -> Result<CompilerFfiLogicalStorageV1, CompilerFfiLogicalStorageErrorV1> {
        observe(self, limits, Self::charge_logical_heap_v1)
    }
    pub(crate) fn charge_logical_heap_v1(&self, counter: &mut Counter) -> StorageResult {
        counter.owner()?;
        let Self {
            table,
            identity,
            canonical_bytes,
        } = self;
        inline(identity);
        table.visit_logical_retained_heap_v1(&mut |count, width| counter.extent(count, width))?;
        counter.vector(canonical_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical_retained_storage_v1::tests::{descriptor_table, limits};
    use std::mem::size_of;

    #[test]
    fn descriptor_source_counts_table_heap_and_canonical_spare_capacity() {
        let mut value = CompilerDescriptorSourceV1::new(descriptor_table()).unwrap();
        value.canonical_bytes.reserve(97);
        let before = value.canonical_bytes().to_vec();
        let identity = value.identity();
        let mut table_bytes = 0usize;
        value
            .table()
            .visit_logical_retained_heap_v1(&mut |count, width| {
                table_bytes = count
                    .checked_mul(width)
                    .and_then(|b| table_bytes.checked_add(b))
                    .ok_or(())?;
                Ok::<_, ()>(())
            })
            .unwrap();
        let r = value.logical_retained_storage_v1(limits()).unwrap();
        assert_eq!(r.header_bytes(), size_of::<CompilerDescriptorSourceV1>());
        assert_eq!(
            r.heap_bytes(),
            table_bytes + value.canonical_bytes.capacity()
        );
        assert_eq!(value.identity(), identity);
        assert_eq!(value.canonical_bytes(), before);
        assert!(!value.authenticates_compiler_origin());
        assert!(!value.grants_link_authority());
        assert!(!value.grants_load_authority());
        assert!(!value.grants_launch_authority());
        assert_eq!(
            value.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
                max_bytes: Some(r.total_bytes()),
                max_items: r.visited_items(),
            }),
            Ok(r)
        );
        assert_eq!(
            value.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
                max_bytes: Some(r.total_bytes() - 1),
                max_items: r.visited_items(),
            }),
            Err(CompilerFfiLogicalStorageErrorV1::ByteLimit)
        );
        for max_items in 0..r.visited_items() {
            assert_eq!(
                value.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
                    max_bytes: None,
                    max_items,
                }),
                Err(CompilerFfiLogicalStorageErrorV1::ItemLimit)
            );
        }
    }
}
