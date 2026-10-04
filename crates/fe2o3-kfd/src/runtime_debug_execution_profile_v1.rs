//! Closed static geometry and descriptive requirements. No runtime permission.
use crate::engineering_gfx950_profile::{
    CONTEXT_BYTES_PER_XCC, CONTROL_STACK_BYTES, CWSR_BYTES, DEBUG_BYTES_TOTAL, PAGE_BYTES,
    RING_BYTES, XCC_COUNT,
};
use fe2o3_amdhsa_loader::SelectedKernelResourceBindingV1;

pub(super) const MAX_OBJECT_BYTES: usize = 1024 * 1024;
pub(super) const MAX_IMAGE_BYTES: usize = 1024 * 1024;
pub(super) const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_LOGICAL_BYTES: u64 = 256 * 1024 * 1024;
const KERNARG_BACKING_BYTES: u64 = 65536;
const OUTPUT_LOGICAL_BYTES: u64 = 272;

/// A precise static refusal. It carries no live device or queue authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugExecutionPreparationErrorV1 {
    ActiveContext,
    DeviceProfile,
    ResourceCardinality,
    ResourceIdentity,
    ResourceAccounting,
    ObjectBound,
    ObjectIdentity,
    LoaderClosure,
    ImageBound,
    ImageIdentity,
    TrapIdentity,
    MetadataIdentity,
    Wave,
    Scratch,
    GroupMemory,
    RegisterResources,
    Workgroup,
    Cluster,
    Kernarg,
    Geometry,
    LogicalStorageBound,
}

/// The sole planned mode, not evidence that this mode is active.
/// Its non-sampling condition requires an actual lifetime-owned native guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugExecutionContractV1 {
    NoSamplingRuntime3TtmpCwsr8XccMetadata11,
}

/// Read-only checked arithmetic, not allocated queue resources or an execution
/// token. All values are logical backing byte counts, never process RSS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950DebugQueueGeometryV1 {
    queue_backing_bytes: u64,
    projected_native_bytes: u64,
    projected_logical_bytes: u64,
}
impl Gfx950DebugQueueGeometryV1 {
    pub const fn queue_backing_bytes(self) -> u64 {
        self.queue_backing_bytes
    }
    pub const fn projected_native_bytes(self) -> u64 {
        self.projected_native_bytes
    }
    pub const fn projected_logical_bytes(self) -> u64 {
        self.projected_logical_bytes
    }
    pub const fn ring_bytes(self) -> u64 {
        RING_BYTES as u64
    }
    pub const fn cwsr_bytes(self) -> u64 {
        CWSR_BYTES as u64
    }
    pub const fn output_logical_bytes(self) -> u64 {
        OUTPUT_LOGICAL_BYTES
    }
    pub const fn grid(self) -> [u32; 3] {
        [64, 1, 1]
    }
    pub const fn workgroup(self) -> [u32; 3] {
        [64, 1, 1]
    }
    pub const fn packet_limit(self) -> u32 {
        1
    }
    pub const fn contract(self) -> Gfx950DebugExecutionContractV1 {
        Gfx950DebugExecutionContractV1::NoSamplingRuntime3TtmpCwsr8XccMetadata11
    }
}

#[derive(Clone, Copy)]
pub(super) struct KernelFactsV1 {
    pub(super) wave: u32,
    pub(super) private: u64,
    pub(super) group: u64,
    pub(super) sgprs: u16,
    pub(super) vgprs: u16,
    pub(super) agprs: Option<u32>,
    pub(super) sgpr_spills: Option<u32>,
    pub(super) vgpr_spills: Option<u32>,
    pub(super) max_workgroup: u32,
    pub(super) required_workgroup: Option<[u32; 3]>,
    pub(super) max_workgroups: [Option<u32>; 3],
    pub(super) cluster: Option<[u32; 3]>,
    pub(super) kernarg_bytes: u64,
    pub(super) kernarg_alignment: u64,
}
impl From<SelectedKernelResourceBindingV1> for KernelFactsV1 {
    fn from(r: SelectedKernelResourceBindingV1) -> Self {
        Self {
            wave: r.wavefront_size(),
            private: r.private_segment_fixed_size(),
            group: r.group_segment_fixed_size(),
            sgprs: r.sgpr_count(),
            vgprs: r.vgpr_count(),
            agprs: r.agpr_count(),
            sgpr_spills: r.sgpr_spill_count(),
            vgpr_spills: r.vgpr_spill_count(),
            max_workgroup: r.max_flat_workgroup_size(),
            required_workgroup: r.required_workgroup_size(),
            max_workgroups: r.max_workgroups(),
            cluster: r.cluster_dims(),
            kernarg_bytes: r.kernarg_segment_size(),
            kernarg_alignment: r.kernarg_segment_alignment(),
        }
    }
}
pub(super) fn check_kernel(f: KernelFactsV1) -> Result<(), Gfx950DebugExecutionPreparationErrorV1> {
    use Gfx950DebugExecutionPreparationErrorV1 as E;
    if f.wave != 64 {
        return Err(E::Wave);
    }
    if f.private != 0 {
        return Err(E::Scratch);
    }
    if f.group != 0 {
        return Err(E::GroupMemory);
    }
    // Closed first engineering profile. The normal loader remains responsible
    // for the exact hardware descriptor encoding and resource cross-checks.
    if !(1..=102).contains(&f.sgprs)
        || !(1..=256).contains(&f.vgprs)
        || f.agprs.is_some_and(|v| v != 0)
        || f.sgpr_spills.is_some_and(|v| v != 0)
        || f.vgpr_spills.is_some_and(|v| v != 0)
    {
        return Err(E::RegisterResources);
    }
    if !(64..=1024).contains(&f.max_workgroup)
        || f.required_workgroup.is_some_and(|v| v != [64, 1, 1])
        || f.max_workgroups.into_iter().any(|v| v == Some(0))
    {
        return Err(E::Workgroup);
    }
    if f.cluster.is_some() {
        return Err(E::Cluster);
    }
    if f.kernarg_bytes > KERNARG_BACKING_BYTES
        || f.kernarg_alignment == 0
        || !f.kernarg_alignment.is_power_of_two()
        || f.kernarg_alignment > PAGE_BYTES as u64
    {
        return Err(E::Kernarg);
    }
    Ok(())
}

pub(super) fn geometry(
    cold_backing: u64,
    object_bytes: usize,
    metadata_bytes: usize,
) -> Result<Gfx950DebugQueueGeometryV1, Gfx950DebugExecutionPreparationErrorV1> {
    use Gfx950DebugExecutionPreparationErrorV1 as E;
    if object_bytes == 0 || object_bytes > MAX_OBJECT_BYTES {
        return Err(E::ObjectBound);
    }
    if metadata_bytes == 0 || metadata_bytes > MAX_METADATA_BYTES {
        return Err(E::MetadataIdentity);
    }
    if PAGE_BYTES != 4096
        || RING_BYTES != 131072 * 64
        || XCC_COUNT != 8
        || CONTROL_STACK_BYTES != 0x3000
        || CONTEXT_BYTES_PER_XCC != 0x15a3000
        || DEBUG_BYTES_TOTAL != 0x50000
        || CWSR_BYTES != 0xad68000
    {
        return Err(E::Geometry);
    }
    let context_end = (CONTEXT_BYTES_PER_XCC as u64)
        .checked_mul(XCC_COUNT as u64)
        .ok_or(E::Geometry)?;
    if context_end.checked_add(u64::from(DEBUG_BYTES_TOTAL)) != Some(CWSR_BYTES as u64) {
        return Err(E::Geometry);
    }
    // Every actual existing header's relative debug offset reaches the same
    // complete shared debug region; no smaller invented CWSR allocation.
    for xcc in 0..XCC_COUNT {
        let base = (xcc as u64)
            .checked_mul(CONTEXT_BYTES_PER_XCC as u64)
            .ok_or(E::Geometry)?;
        let offset = ((XCC_COUNT - xcc) as u64)
            .checked_mul(CONTEXT_BYTES_PER_XCC as u64)
            .ok_or(E::Geometry)?;
        if u32::try_from(offset).is_err() || base.checked_add(offset) != Some(context_end) {
            return Err(E::Geometry);
        }
    }
    let queue = (RING_BYTES as u64)
        .checked_add(CWSR_BYTES as u64)
        .and_then(|v| v.checked_add(3 * PAGE_BYTES as u64))
        .and_then(|v| v.checked_add(KERNARG_BACKING_BYTES))
        .ok_or(E::Geometry)?;
    // One future output page includes 256 payload and 16 canary bytes. It is
    // planned only: no output allocation or dispatch is performed here.
    let native = cold_backing
        .checked_add(queue)
        .and_then(|v| v.checked_add(PAGE_BYTES as u64))
        .ok_or(E::LogicalStorageBound)?;
    let logical = native
        .checked_add(object_bytes as u64)
        .and_then(|v| v.checked_add(metadata_bytes as u64))
        .ok_or(E::LogicalStorageBound)?;
    if logical > MAX_LOGICAL_BYTES {
        return Err(E::LogicalStorageBound);
    }
    Ok(Gfx950DebugQueueGeometryV1 {
        queue_backing_bytes: queue,
        projected_native_bytes: native,
        projected_logical_bytes: logical,
    })
}
