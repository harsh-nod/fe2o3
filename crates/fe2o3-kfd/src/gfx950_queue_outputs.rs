//! Read-only gfx950 doorbell planning. No native queue or mapping authority.

use core::fmt;

use fe2o3_kfd_uapi::{
    KFD_GFX950_DOORBELL_BYTES_V1, KFD_GFX950_PROCESS_DOORBELL_SLICE_BYTES_V1,
    KfdGfx950CreateQueueOutputObservationV1,
};

use crate::gfx950_queue_resources::Gfx950AqlQueueResourcePlanV1;
use crate::topology::GfxTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DoorbellPlanningErrorV1 {
    GpuIdMismatch { planned: u32, observed: u32 },
}

impl fmt::Display for Gfx950DoorbellPlanningErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Gfx950DoorbellPlanningErrorV1 {}

/// Descriptive geometry joined to exact full-GPU-ID output observations.
///
/// This type retains its read-only geometry, topology generation and independent
/// output profile. It does not establish syscall success, unchanged inputs,
/// native device/VM identity, currentness or resource custody. A future native
/// adapter must establish those separately before any mmap or doorbell store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950AqlDoorbellPlanV1 {
    resources: Gfx950AqlQueueResourcePlanV1,
    outputs: KfdGfx950CreateQueueOutputObservationV1,
}

impl Gfx950AqlDoorbellPlanV1 {
    pub const fn resource_plan(self) -> Gfx950AqlQueueResourcePlanV1 {
        self.resources
    }
    pub const fn outputs(self) -> KfdGfx950CreateQueueOutputObservationV1 {
        self.outputs
    }
    pub const fn target(self) -> GfxTarget {
        self.resources.target()
    }
    pub const fn unique_id(self) -> u64 {
        self.resources.unique_id()
    }
    pub const fn gpu_id(self) -> u32 {
        self.resources.gpu_id()
    }
    pub const fn topology_generation(self) -> u64 {
        self.resources.topology_generation()
    }
    pub const fn resource_profile_sha256(self) -> &'static str {
        self.resources.profile_sha256()
    }
    pub const fn output_profile_sha256(self) -> &'static str {
        self.outputs.profile_sha256()
    }
    pub const fn mapping_bytes(self) -> u64 {
        KFD_GFX950_PROCESS_DOORBELL_SLICE_BYTES_V1
    }
    pub const fn doorbell_bytes(self) -> u64 {
        KFD_GFX950_DOORBELL_BYTES_V1
    }
    pub const fn encoded_process_slice_offset(self) -> u64 {
        self.outputs
            .doorbell_offset()
            .encoded_process_slice_offset()
    }
    pub const fn in_process_byte_offset(self) -> u64 {
        self.outputs.doorbell_offset().in_process_byte_offset()
    }
}

/// Joins gfx950 read-only geometry and numeric observations without any effects.
///
/// The full caller-retained GPU ID must match, not just the 16-bit mmap hash.
/// Neither input can be substituted by its gfx942 counterpart.
///
/// ```compile_fail
/// use fe2o3_kfd::gfx950_queue_outputs::plan_gfx950_aql_doorbell_v1;
/// fn wrong_target(resources: fe2o3_kfd::Gfx942AqlQueueResourcePlanV1,
///                 outputs: fe2o3_kfd_uapi::KfdGfx950CreateQueueOutputObservationV1) {
///     let _ = plan_gfx950_aql_doorbell_v1(resources, outputs);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kfd::gfx950_queue_outputs::plan_gfx950_aql_doorbell_v1;
/// fn wrong_target(resources: fe2o3_kfd::gfx950_queue_resources::Gfx950AqlQueueResourcePlanV1,
///                 outputs: fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs) {
///     let _ = plan_gfx950_aql_doorbell_v1(resources, outputs);
/// }
/// ```
pub const fn plan_gfx950_aql_doorbell_v1(
    resources: Gfx950AqlQueueResourcePlanV1,
    outputs: KfdGfx950CreateQueueOutputObservationV1,
) -> Result<Gfx950AqlDoorbellPlanV1, Gfx950DoorbellPlanningErrorV1> {
    if resources.gpu_id() != outputs.gpu_id() {
        return Err(Gfx950DoorbellPlanningErrorV1::GpuIdMismatch {
            planned: resources.gpu_id(),
            observed: outputs.gpu_id(),
        });
    }
    Ok(Gfx950AqlDoorbellPlanV1 { resources, outputs })
}
