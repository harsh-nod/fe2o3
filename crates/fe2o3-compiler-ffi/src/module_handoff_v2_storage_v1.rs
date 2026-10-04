use super::*;
use crate::logical_retained_storage_v1::{
    CompilerFfiLogicalStorageErrorV1, CompilerFfiLogicalStorageLimitsV1,
    CompilerFfiLogicalStorageV1, Counter, StorageResult, inline, observe,
};

impl CompilerModuleHandoffV2 {
    /// Complete actual-capacity root report, including decoded metadata.
    ///
    /// The only shared slot in this closed owner is canonical_bytes. Charge its
    /// entire reachable backing once, never the selected range or strong count.
    /// An outer owner retaining another alias must deduplicate that backing when
    /// composing an aggregate. Arc counters/allocator bookkeeping are excluded.
    pub fn logical_retained_storage_v1(
        &self,
        limits: CompilerFfiLogicalStorageLimitsV1,
    ) -> Result<CompilerFfiLogicalStorageV1, CompilerFfiLogicalStorageErrorV1> {
        observe(self, limits, Self::charge_logical_heap_v1)
    }
    pub(crate) fn charge_logical_heap_v1(&self, counter: &mut Counter) -> StorageResult {
        counter.owner()?;
        let Self {
            kind,
            target,
            code_object_version,
            module_identity,
            envelope,
            symbol_manifest,
            identity,
            canonical_bytes,
            module_offset,
        } = self;
        inline(kind);
        inline(target);
        inline(code_object_version);
        inline(module_identity);
        inline(identity);
        inline(module_offset);
        envelope.charge_logical_heap_v1(counter)?;
        symbol_manifest.charge_logical_heap_v1(counter)?;
        match canonical_bytes {
            CanonicalHandoffBytesV2::Vec(bytes) => counter.vector(bytes)?,
            CanonicalHandoffBytesV2::Box(bytes) => counter.array::<u8>(bytes.len())?,
            CanonicalHandoffBytesV2::Shared { backing, range } => {
                let Range { start, end } = range;
                inline(start);
                inline(end);
                counter.array::<u8>(backing.len())?;
            }
            CanonicalHandoffBytesV2::SharedVector { backing, range } => {
                let Range { start, end } = range;
                inline(start);
                inline(end);
                // The Arc owns a heap-resident Vec header in addition to its
                // byte allocation. The Arc pointer in Self covers neither.
                counter.array::<Vec<u8>>(1)?;
                counter.vector(backing.as_ref())?;
            }
        }
        Ok(())
    }
}
impl CompilerModuleHandoffPartsV2 {
    /// Complete moved-parts root report; embedded child headers are not repeated.
    pub fn logical_retained_storage_v1(
        &self,
        limits: CompilerFfiLogicalStorageLimitsV1,
    ) -> Result<CompilerFfiLogicalStorageV1, CompilerFfiLogicalStorageErrorV1> {
        observe(self, limits, Self::charge_logical_heap_v1)
    }
    pub(crate) fn charge_logical_heap_v1(&self, counter: &mut Counter) -> StorageResult {
        counter.owner()?;
        let Self {
            envelope,
            symbol_manifest,
            module,
        } = self;
        envelope.charge_logical_heap_v1(counter)?;
        symbol_manifest.charge_logical_heap_v1(counter)?;
        module.charge_logical_heap_v1(counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical_retained_storage_v1::tests::{envelope, limits};
    use std::mem::size_of;

    fn value() -> CompilerModuleHandoffV2 {
        CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            CodeObjectVersion::V5,
            envelope(),
            CompilerModuleSymbolManifestV1::new([
                (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
                (CompilerModuleSymbolRoleV1::DeviceFfiExport, "helper"),
            ])
            .unwrap(),
            b"; module\ndefine amdgpu_kernel void @kernel() { ret void }\n",
        )
        .unwrap()
    }
    fn metadata_heap(value: &CompilerModuleHandoffV2) -> usize {
        value
            .envelope()
            .logical_retained_storage_v1(limits())
            .unwrap()
            .heap_bytes()
            + value
                .symbol_manifest()
                .logical_retained_storage_v1(limits())
                .unwrap()
                .heap_bytes()
    }
    fn unchanged(value: &CompilerModuleHandoffV2, backing_bytes: usize) {
        let before = value.canonical_bytes().to_vec();
        let identity = value.identity();
        let r = value.logical_retained_storage_v1(limits()).unwrap();
        assert_eq!(r.header_bytes(), size_of::<CompilerModuleHandoffV2>());
        assert_eq!(r.heap_bytes(), metadata_heap(value) + backing_bytes);
        assert_eq!(value.canonical_bytes(), before);
        assert_eq!(value.identity(), identity);
        assert!(!value.grants_compiler_authority());
        assert!(!value.grants_worker_authority());
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
        assert_eq!(
            value.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
                max_bytes: None,
                max_items: r.visited_items() - 1,
            }),
            Err(CompilerFfiLogicalStorageErrorV1::ItemLimit)
        );
    }

    #[test]
    fn handoff_vec_and_box_count_actual_backing_not_module_view() {
        let mut original = value();
        let bytes = original.canonical_bytes().to_vec();
        if let CanonicalHandoffBytesV2::Vec(v) = &mut original.canonical_bytes {
            v.reserve(97);
        } else {
            panic!("new handoff did not retain Vec");
        }
        unchanged(&original, original.backing_capacity());
        let boxed = CompilerModuleHandoffV2::decode_owned(bytes.into_boxed_slice()).unwrap();
        unchanged(&boxed, boxed.backing_capacity());
        assert!(original.backing_capacity() > original.module_bytes().len());
    }

    #[test]
    fn shared_slice_counts_unselected_backing_once_and_does_not_clone_arc() {
        let original = value();
        let mut bytes = vec![0x11; 17];
        bytes.extend_from_slice(original.canonical_bytes());
        bytes.extend_from_slice(&[0x22; 31]);
        let backing: Arc<[u8]> = bytes.into();
        let owner = CompilerModuleHandoffV2::decode_shared_range(
            Arc::clone(&backing),
            17,
            original.canonical_bytes().len(),
        )
        .unwrap();
        let alias = Arc::clone(&backing);
        let strong = Arc::strong_count(&backing);
        unchanged(&owner, backing.len());
        assert_eq!(Arc::strong_count(&backing), strong);
        drop(alias);
        unchanged(&owner, backing.len());
    }

    #[test]
    fn shared_vector_counts_heap_vec_header_and_full_spare_backing_once() {
        let original = value();
        let mut bytes = Vec::with_capacity(original.canonical_bytes().len() + 211);
        bytes.extend_from_slice(&[0x11; 17]);
        bytes.extend_from_slice(original.canonical_bytes());
        bytes.extend_from_slice(&[0x22; 31]);
        let backing = Arc::new(bytes);
        let owner = CompilerModuleHandoffV2::decode_shared_vec_range(
            Arc::clone(&backing),
            17,
            original.canonical_bytes().len(),
        )
        .unwrap();
        let alias = Arc::clone(&backing);
        let strong = Arc::strong_count(&backing);
        let expected = size_of::<Vec<u8>>() + backing.capacity();
        unchanged(&owner, expected);
        assert_eq!(Arc::strong_count(&backing), strong);
        drop(alias);
        unchanged(&owner, expected);
    }

    #[test]
    fn moved_parts_count_each_actual_child_heap_but_only_one_root_header() {
        let parts = value().into_parts();
        let expected = parts
            .envelope()
            .logical_retained_storage_v1(limits())
            .unwrap()
            .heap_bytes()
            + parts
                .symbol_manifest()
                .logical_retained_storage_v1(limits())
                .unwrap()
                .heap_bytes()
            + parts
                .module()
                .logical_retained_storage_v1(limits())
                .unwrap()
                .heap_bytes();
        let r = parts.logical_retained_storage_v1(limits()).unwrap();
        assert_eq!(r.header_bytes(), size_of::<CompilerModuleHandoffPartsV2>());
        assert_eq!(r.heap_bytes(), expected);
        assert!(!parts.authenticates_compiler_origin());
        assert!(!parts.grants_launch_authority());
    }
}
