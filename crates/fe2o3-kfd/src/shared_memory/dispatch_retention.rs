//! Pure, whole-roster validation precedes every linear data-token move.

use super::*;
use crate::queue::dispatch_binding::{
    DispatchDataAuthorityV1, DispatchDataInputStorageV1, DispatchDataStorageRefV1,
    GFX942_MAX_FIXED_DISPATCH_DATA_V1, Gfx942FixedDispatchDataLayoutV1, Gfx942FixedDispatchDataV1,
};
use arrayvec::ArrayVec;

pub(crate) struct RetainedDispatchDataV1 {
    pub(crate) authority: DispatchDataAuthorityV1,
    pub(crate) layout: Gfx942FixedDispatchDataLayoutV1,
    pub(crate) initialized_content: Option<Gfx942DeviceContentDescriptorV1>,
    pub(crate) fully_initialized: bool,
}

pub(crate) type RetainedDispatchDataRosterV1 =
    ArrayVec<RetainedDispatchDataV1, GFX942_MAX_FIXED_DISPATCH_DATA_V1>;

enum DataFacts {
    Device(Gfx942DeviceMemoryDispatchFactsV1),
    Host(SharedGttMappedResourceFactsV1),
}

pub(super) fn retain_v1<B: MemoryBackend>(
    engine: &SharedMemoryEngine<B>,
    device: DeviceKeyV1,
    vm: VmKeyV1,
    data: &mut Vec<Gfx942FixedDispatchDataV1>,
) -> Result<RetainedDispatchDataRosterV1, MemorySessionError> {
    retain_with_v1(engine, device, vm, data, |_| {})
}

pub(super) fn retain_with_v1<B: MemoryBackend>(
    engine: &SharedMemoryEngine<B>,
    device: DeviceKeyV1,
    vm: VmKeyV1,
    data: &mut Vec<Gfx942FixedDispatchDataV1>,
    mut before_validation: impl FnMut(usize),
) -> Result<RetainedDispatchDataRosterV1, MemorySessionError> {
    engine.require_active()?;
    if data.len() > GFX942_MAX_FIXED_DISPATCH_DATA_V1 {
        return Err(MemorySessionError::SharedAllocationCapacity {
            maximum: GFX942_MAX_FIXED_DISPATCH_DATA_V1,
        });
    }
    let mut retained = RetainedDispatchDataRosterV1::new();
    let mut facts = ArrayVec::<DataFacts, GFX942_MAX_FIXED_DISPATCH_DATA_V1>::new();
    for (ordinal, input) in data.iter().enumerate() {
        before_validation(ordinal);
        if input
            .initialized_content()
            .is_some_and(|content| content.byte_len() != input.layout().requested_bytes())
        {
            return Err(MemorySessionError::DeviceContentMismatch);
        }
        if data[..ordinal]
            .iter()
            .any(|prior| prior.sdma_storage_identity() == input.sdma_storage_identity())
        {
            return Err(MemorySessionError::InvalidAllocationAuthority);
        }
        let projected = match input.storage_ref() {
            DispatchDataStorageRefV1::Device(lease) => {
                engine.device_pool_backing_bytes_v1(lease, device, vm)?;
                let index = engine.device_memory_index(lease, DeviceMemoryPhaseV1::Mapped)?;
                let record = &engine.device_memory[index];
                DataFacts::Device(Gfx942DeviceMemoryDispatchFactsV1 {
                    id: record.id,
                    generation: record.generation,
                    device: record.device,
                    vm: record.vm,
                    gpu_va: record.gpu_va,
                    layout: record.layout,
                })
            }
            DispatchDataStorageRefV1::HostVisible(token) => {
                engine.host_pool_backing_bytes_v1(token, device, vm)?;
                let index = engine.index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)?;
                let record = &engine.allocations[index];
                let (_, _, mapping) = model_keys(vm, record.id, record.generation);
                DataFacts::Host(SharedGttMappedResourceFactsV1 {
                    gpu_va: record.gpu_va,
                    logical_bytes: record.layout.requested_bytes,
                    cpu_mapping_bytes: record.layout.cpu_mapping_bytes,
                    gpu_va_bytes: record.layout.gpu_va_bytes,
                    mapping,
                    publication: MemoryPublicationKeyV1 {
                        mapping,
                        id: MemoryPublicationIdV1(record.id),
                    },
                })
            }
        };
        facts.push(projected);
    }

    // The original roster stays exclusively borrowed through this commit. No
    // allocation, callback, native operation or fallible validation occurs here.
    for (input, facts) in data.drain(..).zip(facts) {
        let input = input.into_parts();
        let authority = match (input.storage, facts) {
            (DispatchDataInputStorageV1::Device(lease), DataFacts::Device(facts)) => {
                DispatchDataAuthorityV1::Device(Gfx942DeviceMemoryDispatchAuthorityV1 {
                    lease,
                    facts,
                })
            }
            (DispatchDataInputStorageV1::HostVisible(token), DataFacts::Host(facts)) => {
                DispatchDataAuthorityV1::HostVisible(SharedGttQueueResourceAuthorityV1 {
                    token,
                    facts,
                    role: PhantomData,
                })
            }
            _ => unreachable!("exclusive original roster preserves validated storage kinds"),
        };
        retained.push(RetainedDispatchDataV1 {
            authority,
            layout: input.layout,
            initialized_content: input.initialized_content,
            fully_initialized: input.fully_initialized,
        });
    }
    Ok(retained)
}
