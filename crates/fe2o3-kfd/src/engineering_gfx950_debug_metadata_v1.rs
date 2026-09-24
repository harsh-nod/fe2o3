//! Actual-Kernel-derived, owned version-zero metadata preparation.
//! Only the consuming cold owner retains this alongside the real Kernel.
//! No runtime-enable, trap operation, active metadata or pointer accessor.

use super::{Backend, Kernel};
use crate::memory::MemoryBackend;
use fe2o3_amdhsa_loader::AdmittedProfile;
use sha2::{Digest, Sha256};

#[path = "runtime_debug_metadata_storage_v1.rs"]
mod storage;
pub(super) use storage::MetadataErrorV1;
use storage::{MetadataStorageV1, checked_load_bias};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreparedMetadataFactsV1 {
    pub(super) artifact_sha256: [u8; 32],
    pub(super) artifact_bytes: usize,
    /// Logical retained bytes, not allocator/RSS usage.
    pub(super) logical_retained_bytes: usize,
}

/// Owned immutable metadata with binding derived solely from an actual Kernel.
/// This is crate-private and has no activation or native address accessor.
pub(super) struct OwnedPreparedDebugMetadataV1 {
    storage: MetadataStorageV1,
    facts: PreparedMetadataFactsV1,
    mapping_va: u64,
    descriptor_offset: u64,
}

impl OwnedPreparedDebugMetadataV1 {
    pub(super) fn from_kernel(kernel: &Kernel) -> Result<Self, MetadataErrorV1> {
        if kernel.object.is_empty() || kernel.object.len() > fe2o3_hsaco::MAX_HSACO_BYTES {
            return Err(MetadataErrorV1::ArtifactBound);
        }
        let digest: [u8; 32] = Sha256::digest(&kernel.object).into();
        if digest != kernel.metadata.object_sha256 {
            return Err(MetadataErrorV1::Identity);
        }
        let closure =
            fe2o3_amdhsa_loader::validate(&kernel.object, AdmittedProfile::Gfx950XnackOffCov6)
                .map_err(|_| MetadataErrorV1::Identity)?
                .bind_kernel(kernel.inspected.name())
                .map_err(|_| MetadataErrorV1::Identity)?;
        let image_start = closure.envelope().plan().image_start();
        let image_len = usize::try_from(closure.envelope().materialization().image_len())
            .map_err(|_| MetadataErrorV1::Mapping)?;
        let descriptor_offset = closure
            .selected_binding()
            .descriptor_address()
            .checked_sub(image_start)
            .ok_or(MetadataErrorV1::Mapping)?;
        if closure.selected_kernel() != &kernel.inspected
            || closure.resources() != kernel.resources
            || kernel.resources.wavefront_size() != 64
            || kernel.resources.private_segment_fixed_size() != 0
            || descriptor_offset != kernel.descriptor_offset
            || kernel.code.requested != image_len
            || kernel.code.backing < image_len
            || Backend::mapping_address(&kernel.code.mapping) != kernel.code.va
        {
            return Err(MetadataErrorV1::Mapping);
        }
        let bias = checked_load_bias(kernel.code.va, image_start)?;
        kernel
            .code
            .va
            .checked_add(image_len as u64)
            .ok_or(MetadataErrorV1::Mapping)?;
        let storage = MetadataStorageV1::prepare(&kernel.object, bias)?;
        if storage.retained_elf() != kernel.object.as_slice() || !storage.is_prepared() {
            return Err(MetadataErrorV1::Identity);
        }
        let facts = PreparedMetadataFactsV1 {
            artifact_sha256: digest,
            artifact_bytes: kernel.object.len(),
            logical_retained_bytes: storage.prepared_bytes(),
        };
        Ok(Self {
            storage,
            facts,
            mapping_va: kernel.code.va,
            descriptor_offset: kernel.descriptor_offset,
        })
    }

    pub(super) fn facts(&self) -> PreparedMetadataFactsV1 {
        debug_assert!(self.storage.is_prepared());
        self.facts
    }
    pub(super) fn matches_kernel(&self, kernel: &Kernel) -> bool {
        self.storage.is_prepared()
            && self.storage.retained_elf() == kernel.object.as_slice()
            && self.facts.artifact_sha256 == kernel.metadata.object_sha256
            && self.facts.artifact_bytes == kernel.object.len()
            && self.mapping_va == kernel.code.va
            && self.descriptor_offset == kernel.descriptor_offset
            && Backend::mapping_address(&kernel.code.mapping) == self.mapping_va
    }
}
