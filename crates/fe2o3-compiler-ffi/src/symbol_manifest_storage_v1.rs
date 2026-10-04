use super::*;
use crate::logical_retained_storage_v1::{
    CompilerFfiLogicalStorageErrorV1, CompilerFfiLogicalStorageLimitsV1,
    CompilerFfiLogicalStorageV1, Counter, StorageResult, inline, observe,
};

impl CompilerModuleSymbolManifestV1 {
    /// Complete root report, including owned canonical bytes and symbol capacity.
    pub fn logical_retained_storage_v1(
        &self,
        limits: CompilerFfiLogicalStorageLimitsV1,
    ) -> Result<CompilerFfiLogicalStorageV1, CompilerFfiLogicalStorageErrorV1> {
        observe(self, limits, Self::charge_logical_heap_v1)
    }
    pub(crate) fn charge_logical_heap_v1(&self, counter: &mut Counter) -> StorageResult {
        counter.owner()?;
        let Self {
            entries,
            canonical_bytes,
            identity,
        } = self;
        inline(identity);
        counter.vector(entries)?;
        counter.vector(canonical_bytes)?;
        for entry in entries {
            counter.owner()?;
            let SymbolRoleEntryV1 { role, symbol } = entry;
            inline(role);
            counter.string(symbol)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical_retained_storage_v1::tests::limits;
    use std::mem::size_of;

    #[test]
    fn manifest_empty_and_populated_actual_capacities_are_complete() {
        let mut symbol = String::with_capacity(73);
        symbol.push_str("entry");
        for mut value in [
            CompilerModuleSymbolManifestV1::new([] as [(CompilerModuleSymbolRoleV1, &str); 0])
                .unwrap(),
            CompilerModuleSymbolManifestV1::new([(
                CompilerModuleSymbolRoleV1::KernelEntry,
                symbol,
            )])
            .unwrap(),
        ] {
            value.entries.reserve(5);
            value.canonical_bytes.reserve(19);
            let identity = value.identity();
            let canonical = value.canonical_bytes().to_vec();
            let expected = value.entries.capacity() * size_of::<SymbolRoleEntryV1>()
                + value.canonical_bytes.capacity()
                + value
                    .entries
                    .iter()
                    .map(|e| e.symbol.capacity())
                    .sum::<usize>();
            let r = value.logical_retained_storage_v1(limits()).unwrap();
            assert_eq!(
                r.header_bytes(),
                size_of::<CompilerModuleSymbolManifestV1>()
            );
            assert_eq!(r.heap_bytes(), expected);
            assert_eq!(value.identity(), identity);
            assert_eq!(value.canonical_bytes(), canonical);
            assert!(!value.authenticates_compiler_origin());
            assert!(!value.grants_link_authority());
            assert_eq!(
                value.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
                    max_bytes: Some(r.total_bytes() - 1),
                    max_items: r.visited_items(),
                }),
                Err(CompilerFfiLogicalStorageErrorV1::ByteLimit)
            );
            assert_eq!(
                value.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
                    max_bytes: Some(r.total_bytes()),
                    max_items: r.visited_items(),
                }),
                Ok(r)
            );
        }
    }
}
