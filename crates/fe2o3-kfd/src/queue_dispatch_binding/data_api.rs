//! Inert fixed-dispatch packet, DATA, and completed-readback accessors.

use super::*;

impl Gfx942FixedDispatchPacketV1 {
    pub fn new(
        program_index: usize,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        kernarg_bytes: Box<[u8]>,
        buffers: Box<[Gfx942DispatchBufferBindingV1]>,
    ) -> Self {
        Self::new_with_ordering(
            program_index,
            geometry,
            AqlDispatchOrderingV1::WaitForPrior,
            dynamic_group_segment_bytes,
            kernarg_bytes,
            buffers,
        )
    }

    /// Creates an explicitly independent packet that need not wait for prior work.
    ///
    /// Phase-1 native fixed-recipe preparation rejects independent packets before
    /// acquiring native resources. This constructor remains for API compatibility
    /// with non-fixed paths and future ordering policies.
    pub fn new_independent(
        program_index: usize,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        kernarg_bytes: Box<[u8]>,
        buffers: Box<[Gfx942DispatchBufferBindingV1]>,
    ) -> Self {
        Self::new_with_ordering(
            program_index,
            geometry,
            AqlDispatchOrderingV1::Independent,
            dynamic_group_segment_bytes,
            kernarg_bytes,
            buffers,
        )
    }

    /// Creates a packet with an explicit reviewed AQL execution-order policy.
    pub fn new_with_ordering(
        program_index: usize,
        geometry: AqlDispatchGeometryV1,
        ordering: AqlDispatchOrderingV1,
        dynamic_group_segment_bytes: u32,
        kernarg_bytes: Box<[u8]>,
        buffers: Box<[Gfx942DispatchBufferBindingV1]>,
    ) -> Self {
        Self {
            program_index,
            geometry,
            ordering,
            dynamic_group_segment_bytes,
            kernarg_bytes,
            buffers,
            conditional_fill: false,
        }
    }

    /// Requires the closed full64 fill profile at native preparation and binding.
    ///
    /// This only narrows admission. It supplies no compiler, proof or execution
    /// authority. The first profile requires one ordinary packet, one program,
    /// one nonempty whole coherent-host output and 272-byte kernargs. Only its
    /// first dispatch generation may bind; replay requires fresh preparation.
    #[must_use]
    pub fn require_conditional_fill_v1(mut self) -> Self {
        self.conditional_fill = true;
        self
    }

    pub const fn program_index(&self) -> usize {
        self.program_index
    }

    pub const fn geometry(&self) -> AqlDispatchGeometryV1 {
        self.geometry
    }

    pub const fn ordering(&self) -> AqlDispatchOrderingV1 {
        self.ordering
    }

    pub const fn dynamic_group_segment_bytes(&self) -> u32 {
        self.dynamic_group_segment_bytes
    }

    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }
}

impl Gfx942FixedDispatchDataV1 {
    pub fn uninitialized(lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>) -> Self {
        Self {
            storage: DispatchDataStorageV1::Uninitialized(lease),
        }
    }

    pub fn initialized(memory: Gfx942InitializedDeviceMemoryV1) -> Self {
        Self {
            storage: DispatchDataStorageV1::InitializedContent(memory),
        }
    }

    pub(in super::super) fn initialized_storage(
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Self {
        Self {
            storage: DispatchDataStorageV1::InitializedStorage(lease),
        }
    }

    #[cfg(test)]
    pub(crate) fn initialized_storage_for_test(
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Self {
        Self::initialized_storage(lease)
    }

    pub fn host_visible_uninitialized(
        token: SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>,
    ) -> Self {
        Self {
            storage: DispatchDataStorageV1::HostVisibleUninitialized(token),
        }
    }

    pub fn host_visible_initialized(memory: Gfx942InitializedHostVisibleMemoryV1) -> Self {
        Self {
            storage: DispatchDataStorageV1::HostVisibleInitialized(memory),
        }
    }

    pub const fn layout(&self) -> Gfx942FixedDispatchDataLayoutV1 {
        match &self.storage {
            DispatchDataStorageV1::Uninitialized(lease)
            | DispatchDataStorageV1::InitializedStorage(lease) => {
                let layout = lease.layout();
                Gfx942FixedDispatchDataLayoutV1 {
                    kind: Gfx942FixedDispatchDataKindV1::DeviceLocal,
                    requested_bytes: layout.requested_bytes(),
                    alignment: layout.alignment(),
                }
            }
            DispatchDataStorageV1::InitializedContent(memory) => {
                let layout = memory.layout();
                Gfx942FixedDispatchDataLayoutV1 {
                    kind: Gfx942FixedDispatchDataKindV1::DeviceLocal,
                    requested_bytes: layout.requested_bytes(),
                    alignment: layout.alignment(),
                }
            }
            DispatchDataStorageV1::HostVisibleUninitialized(token) => {
                Gfx942FixedDispatchDataLayoutV1 {
                    kind: Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
                    requested_bytes: token.layout().requested_bytes() as u64,
                    alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
                }
            }
            DispatchDataStorageV1::HostVisibleInitialized(memory) => {
                Gfx942FixedDispatchDataLayoutV1 {
                    kind: Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
                    requested_bytes: memory.layout().requested_bytes() as u64,
                    alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
                }
            }
        }
    }

    pub(crate) const fn storage_identity(&self) -> Gfx942FixedDispatchStorageIdentityV1 {
        match &self.storage {
            DispatchDataStorageV1::Uninitialized(lease) => {
                Gfx942FixedDispatchStorageIdentityV1::DeviceUninitialized(lease.storage_identity())
            }
            DispatchDataStorageV1::InitializedContent(memory) => {
                Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedContent(
                    memory.storage_identity(),
                )
            }
            DispatchDataStorageV1::InitializedStorage(lease) => {
                Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedStorage(
                    lease.storage_identity(),
                )
            }
            DispatchDataStorageV1::HostVisibleUninitialized(token) => {
                Gfx942FixedDispatchStorageIdentityV1::HostVisibleUninitialized(
                    token.storage_identity(),
                )
            }
            DispatchDataStorageV1::HostVisibleInitialized(memory) => {
                Gfx942FixedDispatchStorageIdentityV1::HostVisibleInitialized(
                    memory.storage_identity(),
                )
            }
        }
    }

    pub(crate) const fn sdma_storage_identity(&self) -> Gfx942SdmaBufferStorageIdentityV1 {
        match &self.storage {
            DispatchDataStorageV1::Uninitialized(lease)
            | DispatchDataStorageV1::InitializedStorage(lease) => {
                Gfx942SdmaBufferStorageIdentityV1::Device(lease.storage_identity())
            }
            DispatchDataStorageV1::InitializedContent(memory) => {
                Gfx942SdmaBufferStorageIdentityV1::Device(memory.storage_identity())
            }
            DispatchDataStorageV1::HostVisibleUninitialized(token) => {
                Gfx942SdmaBufferStorageIdentityV1::Host(token.storage_identity())
            }
            DispatchDataStorageV1::HostVisibleInitialized(memory) => {
                Gfx942SdmaBufferStorageIdentityV1::Host(memory.storage_identity())
            }
        }
    }

    pub(crate) fn into_sdma_storage(self) -> Gfx942SdmaBufferStorageV1 {
        match self.storage {
            DispatchDataStorageV1::Uninitialized(lease)
            | DispatchDataStorageV1::InitializedStorage(lease) => {
                Gfx942SdmaBufferStorageV1::Device(lease)
            }
            DispatchDataStorageV1::InitializedContent(memory) => {
                Gfx942SdmaBufferStorageV1::Device(memory.into_parts().0)
            }
            DispatchDataStorageV1::HostVisibleUninitialized(token) => {
                Gfx942SdmaBufferStorageV1::Host(token)
            }
            DispatchDataStorageV1::HostVisibleInitialized(memory) => {
                Gfx942SdmaBufferStorageV1::Host(memory.into_token())
            }
        }
    }

    pub(crate) fn initialized_host_visible_token_mut(
        &mut self,
    ) -> Option<&mut SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>>
    {
        match &mut self.storage {
            DispatchDataStorageV1::HostVisibleInitialized(memory) => Some(memory.token_mut()),
            _ => None,
        }
    }

    /// Returns whether the complete requested extent has initialized bytes.
    ///
    /// This observation does not identify their current content after any
    /// device publication.
    pub const fn is_fully_initialized(&self) -> bool {
        !matches!(
            self.storage,
            DispatchDataStorageV1::Uninitialized(_)
                | DispatchDataStorageV1::HostVisibleUninitialized(_)
        )
    }

    pub(crate) fn into_parts(self) -> DispatchDataInputV1 {
        let layout = self.layout();
        match self.storage {
            DispatchDataStorageV1::Uninitialized(lease) => DispatchDataInputV1 {
                layout,
                storage: DispatchDataInputStorageV1::Device(lease),
                initialized_content: None,
                fully_initialized: false,
            },
            DispatchDataStorageV1::InitializedContent(memory) => {
                let (lease, content) = memory.into_parts();
                DispatchDataInputV1 {
                    layout,
                    storage: DispatchDataInputStorageV1::Device(lease),
                    initialized_content: Some(content),
                    fully_initialized: true,
                }
            }
            DispatchDataStorageV1::InitializedStorage(lease) => DispatchDataInputV1 {
                layout,
                storage: DispatchDataInputStorageV1::Device(lease),
                initialized_content: None,
                fully_initialized: true,
            },
            DispatchDataStorageV1::HostVisibleUninitialized(token) => DispatchDataInputV1 {
                layout,
                storage: DispatchDataInputStorageV1::HostVisible(token),
                initialized_content: None,
                fully_initialized: false,
            },
            DispatchDataStorageV1::HostVisibleInitialized(memory) => DispatchDataInputV1 {
                layout,
                storage: DispatchDataInputStorageV1::HostVisible(memory.into_token()),
                initialized_content: None,
                fully_initialized: true,
            },
        }
    }

    pub(crate) fn storage_ref(&self) -> DispatchDataStorageRefV1<'_> {
        match &self.storage {
            DispatchDataStorageV1::Uninitialized(lease)
            | DispatchDataStorageV1::InitializedStorage(lease) => {
                DispatchDataStorageRefV1::Device(lease)
            }
            DispatchDataStorageV1::InitializedContent(memory) => {
                DispatchDataStorageRefV1::Device(memory.lease())
            }
            DispatchDataStorageV1::HostVisibleUninitialized(token) => {
                DispatchDataStorageRefV1::HostVisible(token)
            }
            DispatchDataStorageV1::HostVisibleInitialized(memory) => {
                DispatchDataStorageRefV1::HostVisible(memory.token())
            }
        }
    }

    pub(crate) fn initialized_content(&self) -> Option<Gfx942DeviceContentDescriptorV1> {
        match &self.storage {
            DispatchDataStorageV1::InitializedContent(memory) => Some(memory.content()),
            _ => None,
        }
    }
}

impl Gfx942CompletedDispatchReadbackV1 {
    pub const fn dispatch_generation(&self) -> u64 {
        self.dispatch_generation
    }

    pub const fn data_index(&self) -> usize {
        self.data_index
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the readback and returns its owned bytes.
    pub fn into_bytes(self) -> Box<[u8]> {
        self.bytes
    }
}
