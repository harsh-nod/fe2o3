use super::*;
use crate::logical_retained_storage_v1::{
    CompilerFfiLogicalStorageErrorV1, CompilerFfiLogicalStorageLimitsV1,
    CompilerFfiLogicalStorageV1, Counter, StorageResult, inline, observe,
};

impl CompilerModulePayloadV1 {
    /// Complete root report; the retained Vec may include drained prefix capacity.
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
            identity,
            bytes,
        } = self;
        inline(kind);
        inline(identity);
        counter.vector(bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical_retained_storage_v1::tests::limits;

    #[test]
    fn module_payload_counts_owned_spare_capacity_without_serialization() {
        let mut bytes = Vec::with_capacity(97);
        bytes.extend_from_slice(b"; module\n");
        let capacity = bytes.capacity();
        let identity = CompilerModuleIdentityV1::calculate(&bytes);
        let value = CompilerModulePayloadV1::from_validated(
            CompilerModuleKindV1::LlvmTextIr,
            identity,
            bytes,
        );
        let r = value.logical_retained_storage_v1(limits()).unwrap();
        assert_eq!(r.heap_bytes(), capacity);
        assert_eq!(value.bytes(), b"; module\n");
        assert_eq!(value.identity(), identity);
        assert!(!value.grants_compiler_authority());
    }
}
