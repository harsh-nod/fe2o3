//! Private numeric mechanics only; public target wrappers retain their brands.

use crate::{
    KFD_MMAP_GPU_ID_HASH_MASK, KFD_MMAP_GPU_ID_HASH_SHIFT, KFD_MMAP_OFFSET_MASK,
    KFD_MMAP_TYPE_DOORBELL, KFD_MMAP_TYPE_SHIFT,
};

pub(super) struct DecodedQueueOutputs {
    pub(super) encoded_process_slice_offset: u64,
    pub(super) in_process_byte_offset: u64,
}

pub(super) enum NumericQueueOutputError {
    QueueIdOutOfRange { queue_id: u32 },
    DoorbellMmapType { observed: u64 },
    DoorbellGpuIdHash { expected: u16, observed: u16 },
    DoorbellOffsetOutOfRange { offset: u64 },
    DoorbellOffsetMisaligned { offset: u64 },
}

// Both closed callers supply reviewed nonzero power-of-two width/slice values.
// The shared source encoding does not imply shared target admission authority.
pub(super) const fn decode_non_mes_soc15_outputs(
    queue_id: u32,
    raw: u64,
    gpu_id: u32,
    queue_slots: u32,
    doorbell_bytes: u64,
    slice_bytes: u64,
) -> Result<DecodedQueueOutputs, NumericQueueOutputError> {
    use NumericQueueOutputError as Error;
    if queue_id >= queue_slots {
        return Err(Error::QueueIdOutOfRange { queue_id });
    }
    let observed = raw >> KFD_MMAP_TYPE_SHIFT;
    if observed != KFD_MMAP_TYPE_DOORBELL {
        return Err(Error::DoorbellMmapType { observed });
    }
    let observed = ((raw >> KFD_MMAP_GPU_ID_HASH_SHIFT) & KFD_MMAP_GPU_ID_HASH_MASK) as u16;
    let expected = (gpu_id & KFD_MMAP_GPU_ID_HASH_MASK as u32) as u16;
    if observed != expected {
        return Err(Error::DoorbellGpuIdHash { expected, observed });
    }
    let offset = raw & KFD_MMAP_OFFSET_MASK;
    if offset >= slice_bytes {
        return Err(Error::DoorbellOffsetOutOfRange { offset });
    }
    if !offset.is_multiple_of(doorbell_bytes) {
        return Err(Error::DoorbellOffsetMisaligned { offset });
    }
    Ok(DecodedQueueOutputs {
        encoded_process_slice_offset: raw & !(slice_bytes - 1),
        in_process_byte_offset: offset,
    })
}
