//! Closed gfx950 non-MES compute/AQL output observations, never native authority.

use crate::queue_output_numeric::{NumericQueueOutputError, decode_non_mes_soc15_outputs};

pub const KFD_GFX950_QUEUE_OUTPUT_PROFILE_MANIFEST_V1: &str =
    include_str!("gfx950_queue_outputs/profile.manifest");
pub const KFD_GFX950_QUEUE_OUTPUT_PROFILE_SHA256_V1: &str =
    "e84371e6caa91667ab4279731296dd30989a0d618af8aca9eb72320fecea07fe";
pub const KFD_GFX950_PROCESS_QUEUE_SLOTS_V1: u32 = 1024;
pub const KFD_GFX950_DOORBELL_BYTES_V1: u64 = 8;
pub const KFD_GFX950_PROCESS_DOORBELL_SLICE_BYTES_V1: u64 = 8192;

/// Numeric gfx950 process slot observation, not a CP doorbell index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdGfx950QueueIdObservationV1(u32);

impl KfdGfx950QueueIdObservationV1 {
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Numeric gfx950 doorbell observation; no mmap or store is authorized.
///
/// ```compile_fail
/// use fe2o3_kfd_uapi::{KfdGfx942DoorbellOffsetObservation, KfdGfx950DoorbellOffsetObservationV1};
/// fn requires_gfx942(_: KfdGfx942DoorbellOffsetObservation) {}
/// fn wrong_target(value: KfdGfx950DoorbellOffsetObservationV1) { requires_gfx942(value); }
/// ```
/// ```compile_fail
/// use fe2o3_kfd_uapi::{KfdGfx942DoorbellOffsetObservation, KfdGfx950DoorbellOffsetObservationV1};
/// fn requires_gfx950(_: KfdGfx950DoorbellOffsetObservationV1) {}
/// fn wrong_target(value: KfdGfx942DoorbellOffsetObservation) { requires_gfx950(value); }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdGfx950DoorbellOffsetObservationV1 {
    raw: u64,
    encoded_process_slice_offset: u64,
    in_process_byte_offset: u64,
}

impl KfdGfx950DoorbellOffsetObservationV1 {
    pub const fn raw(self) -> u64 {
        self.raw
    }
    /// Encoded mmap offset with the complete 8192-byte slice mask cleared.
    pub const fn encoded_process_slice_offset(self) -> u64 {
        self.encoded_process_slice_offset
    }
    pub const fn in_process_byte_offset(self) -> u64 {
        self.in_process_byte_offset
    }
}

/// Checked numeric observations under the independently reviewed gfx950 profile.
///
/// The retained full GPU ID is caller-supplied, not recoverable or authenticated
/// from the 16-bit wire hash. Neither construction nor this type proves that an
/// ioctl succeeded or any queue exists.
///
/// ```compile_fail
/// use fe2o3_kfd_uapi::{KfdGfx942CreateQueueOutputs, KfdGfx950CreateQueueOutputObservationV1};
/// fn requires_gfx942(_: KfdGfx942CreateQueueOutputs) {}
/// fn wrong_target(value: KfdGfx950CreateQueueOutputObservationV1) { requires_gfx942(value); }
/// ```
/// ```compile_fail
/// use fe2o3_kfd_uapi::{KfdGfx942CreateQueueOutputs, KfdGfx950CreateQueueOutputObservationV1};
/// fn requires_gfx950(_: KfdGfx950CreateQueueOutputObservationV1) {}
/// fn wrong_target(value: KfdGfx942CreateQueueOutputs) { requires_gfx950(value); }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdGfx950CreateQueueOutputObservationV1 {
    gpu_id: u32,
    queue_id: KfdGfx950QueueIdObservationV1,
    doorbell_offset: KfdGfx950DoorbellOffsetObservationV1,
}

impl KfdGfx950CreateQueueOutputObservationV1 {
    pub const fn gpu_id(self) -> u32 {
        self.gpu_id
    }
    pub const fn queue_id(self) -> KfdGfx950QueueIdObservationV1 {
        self.queue_id
    }
    pub const fn doorbell_offset(self) -> KfdGfx950DoorbellOffsetObservationV1 {
        self.doorbell_offset
    }
    pub const fn profile_sha256(self) -> &'static str {
        KFD_GFX950_QUEUE_OUTPUT_PROFILE_SHA256_V1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdGfx950CreateQueueOutputErrorV1 {
    QueueIdOutOfRange { queue_id: u32 },
    DoorbellMmapType { observed: u64 },
    DoorbellGpuIdHash { expected: u16, observed: u16 },
    DoorbellOffsetOutOfRange { offset: u64 },
    DoorbellOffsetMisaligned { offset: u64 },
}

/// Checks numeric output shape only, under the closed gfx950 source profile.
///
/// A future native adapter must separately establish the exact device/platform,
/// current VM/resource custody, successful syscall and unchanged inputs. This
/// pure decoder does none of those things and admits no SDMA or MES profile.
pub const fn observe_kfd_gfx950_create_queue_outputs_v1(
    queue_id: u32,
    raw_doorbell_offset: u64,
    gpu_id: u32,
) -> Result<KfdGfx950CreateQueueOutputObservationV1, KfdGfx950CreateQueueOutputErrorV1> {
    use KfdGfx950CreateQueueOutputErrorV1 as Error;
    let decoded = match decode_non_mes_soc15_outputs(
        queue_id,
        raw_doorbell_offset,
        gpu_id,
        KFD_GFX950_PROCESS_QUEUE_SLOTS_V1,
        KFD_GFX950_DOORBELL_BYTES_V1,
        KFD_GFX950_PROCESS_DOORBELL_SLICE_BYTES_V1,
    ) {
        Ok(decoded) => decoded,
        Err(NumericQueueOutputError::QueueIdOutOfRange { queue_id }) => {
            return Err(Error::QueueIdOutOfRange { queue_id });
        }
        Err(NumericQueueOutputError::DoorbellMmapType { observed }) => {
            return Err(Error::DoorbellMmapType { observed });
        }
        Err(NumericQueueOutputError::DoorbellGpuIdHash { expected, observed }) => {
            return Err(Error::DoorbellGpuIdHash { expected, observed });
        }
        Err(NumericQueueOutputError::DoorbellOffsetOutOfRange { offset }) => {
            return Err(Error::DoorbellOffsetOutOfRange { offset });
        }
        Err(NumericQueueOutputError::DoorbellOffsetMisaligned { offset }) => {
            return Err(Error::DoorbellOffsetMisaligned { offset });
        }
    };
    Ok(KfdGfx950CreateQueueOutputObservationV1 {
        gpu_id,
        queue_id: KfdGfx950QueueIdObservationV1(queue_id),
        doorbell_offset: KfdGfx950DoorbellOffsetObservationV1 {
            raw: raw_doorbell_offset,
            encoded_process_slice_offset: decoded.encoded_process_slice_offset,
            in_process_byte_offset: decoded.in_process_byte_offset,
        },
    })
}
