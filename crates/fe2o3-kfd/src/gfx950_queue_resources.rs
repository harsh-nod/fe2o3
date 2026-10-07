//! Independent read-only gfx950 queue geometry. Plans grant no native authority.

use core::fmt;

use crate::topology::{ComputePartition, GfxTarget, HostTopologySnapshot, MemoryPartition};

pub const GFX950_QUEUE_RESOURCE_PROFILE_MANIFEST_V1: &str =
    include_str!("gfx950_queue_resources/profile.manifest");
pub const GFX950_QUEUE_RESOURCE_PROFILE_SHA256_V1: &str =
    "e838edc8d388cf4339ef9427175f4074ec2944af19394dd559bcbcc729fc0cbc";
pub const GFX950_QUEUE_PAGE_BYTES_V1: u64 = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950QueueResourcePlanningErrorV1 {
    HostProfileMismatch { field: &'static str },
    SelectedGpuNotFound,
    CorrelatedRenderNotFound,
    TargetMismatch,
    PartitionMismatch,
    CapacityMismatch { field: &'static str },
    RingSizeUnsupported,
    InvalidGeometry { field: &'static str },
    ArithmeticOverflow,
}

impl fmt::Display for Gfx950QueueResourcePlanningErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Gfx950QueueResourcePlanningErrorV1 {}
type Error = Gfx950QueueResourcePlanningErrorV1;

/// Descriptive geometry only, deliberately not a gfx942 resource plan.
///
/// ```compile_fail
/// use fe2o3_kfd::gfx950_queue_resources::Gfx950AqlQueueResourcePlanV1;
/// fn requires_gfx942(_: fe2o3_kfd::Gfx942AqlQueueResourcePlanV1) {}
/// fn wrong_target(plan: Gfx950AqlQueueResourcePlanV1) { requires_gfx942(plan); }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950AqlQueueResourcePlanV1 {
    unique_id: u64,
    gpu_id: u32,
    topology_generation: u64,
    ring_bytes: u32,
    cwsr: Gfx950CwsrGeometryV1,
}

impl Gfx950AqlQueueResourcePlanV1 {
    pub const fn unique_id(self) -> u64 {
        self.unique_id
    }
    pub const fn gpu_id(self) -> u32 {
        self.gpu_id
    }
    pub const fn topology_generation(self) -> u64 {
        self.topology_generation
    }
    pub const fn target(self) -> GfxTarget {
        GfxTarget::Gfx950
    }
    pub const fn profile_sha256(self) -> &'static str {
        GFX950_QUEUE_RESOURCE_PROFILE_SHA256_V1
    }
    pub const fn ring_mapping_bytes(self) -> u32 {
        self.ring_bytes
    }
    pub const fn packet_bytes(self) -> u32 {
        64
    }
    pub const fn resource_alignment_bytes(self) -> u64 {
        GFX950_QUEUE_PAGE_BYTES_V1
    }
    pub const fn control_mapping_bytes_per_pointer(self) -> u64 {
        GFX950_QUEUE_PAGE_BYTES_V1
    }
    pub const fn counter_bytes(self) -> u32 {
        8
    }
    pub const fn write_dispatch_id_offset(self) -> u32 {
        0x38
    }
    pub const fn read_dispatch_id_offset(self) -> u32 {
        0x80
    }
    pub const fn read_base_offset_field(self) -> u32 {
        0x88
    }
    pub const fn end_of_pipe_mapping_bytes(self) -> u32 {
        4096
    }
    pub const fn context_save(self) -> Gfx950CwsrGeometryV1 {
        self.cwsr
    }
}

/// Checked source-derived dimensions; no native address or allocation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950CwsrGeometryV1 {
    xcc_count: u32,
    cu_per_xcc: u32,
    waves_per_xcc: u32,
    control_bytes: u32,
    workgroup_bytes: u32,
    context_bytes: u32,
    debug_bytes: u32,
    mapping_bytes: u32,
}

impl Gfx950CwsrGeometryV1 {
    pub const fn xcc_count(self) -> u32 {
        self.xcc_count
    }
    pub const fn cu_per_xcc(self) -> u32 {
        self.cu_per_xcc
    }
    pub const fn waves_per_xcc(self) -> u32 {
        self.waves_per_xcc
    }
    pub const fn control_bytes_per_xcc(self) -> u32 {
        self.control_bytes
    }
    pub const fn workgroup_bytes_per_xcc(self) -> u32 {
        self.workgroup_bytes
    }
    pub const fn context_bytes_per_xcc(self) -> u32 {
        self.context_bytes
    }
    pub const fn debug_bytes_per_xcc(self) -> u32 {
        self.debug_bytes
    }
    pub const fn mapping_bytes(self) -> u32 {
        self.mapping_bytes
    }
    pub const fn debug_region_offset(self) -> u64 {
        self.context_bytes as u64 * self.xcc_count as u64
    }
    pub const fn debug_region_bytes(self) -> u64 {
        self.debug_bytes as u64 * self.xcc_count as u64
    }
    pub const fn shadow_page_count(self) -> u64 {
        self.control_bytes as u64 / 4096 * self.xcc_count as u64
    }

    pub fn header(self, xcc: u32) -> Option<Gfx950CwsrHeaderGeometryV1> {
        (xcc < self.xcc_count).then(|| Gfx950CwsrHeaderGeometryV1 {
            offset: u64::from(xcc) * u64::from(self.context_bytes),
            debug_offset: (self.xcc_count - xcc) * self.context_bytes,
            debug_size: self.xcc_count * self.debug_bytes,
        })
    }

    pub fn shadow_page_offset(self, page: u64) -> Option<u64> {
        if page >= self.shadow_page_count() {
            return None;
        }
        let per_xcc = u64::from(self.control_bytes) / GFX950_QUEUE_PAGE_BYTES_V1;
        Some(
            page / per_xcc * u64::from(self.context_bytes)
                + page % per_xcc * GFX950_QUEUE_PAGE_BYTES_V1,
        )
    }
}

/// Offsets consumed by a future independently checked native header writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950CwsrHeaderGeometryV1 {
    offset: u64,
    debug_offset: u32,
    debug_size: u32,
}
impl Gfx950CwsrHeaderGeometryV1 {
    pub const fn offset(self) -> u64 {
        self.offset
    }
    pub const fn bytes(self) -> u32 {
        40
    }
    pub const fn debug_offset(self) -> u32 {
        self.debug_offset
    }
    pub const fn debug_size(self) -> u32 {
        self.debug_size
    }
}

#[derive(Clone, Copy)]
struct GeometryInputs {
    simd: u64,
    simd_per_cu: u64,
    xcc: u64,
    arrays: u64,
    arrays_per_engine: u64,
    lds_kib: u64,
}

fn add(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_add(b).ok_or(Error::ArithmeticOverflow)
}
fn mul(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_mul(b).ok_or(Error::ArithmeticOverflow)
}
fn narrow(value: u64) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}
fn align(value: u64, alignment: u64) -> Result<u64, Error> {
    if !alignment.is_power_of_two() {
        return Err(Error::InvalidGeometry { field: "alignment" });
    }
    Ok(add(value, alignment - 1)? & !(alignment - 1))
}
fn exact_div(value: u64, divisor: u64) -> Result<u64, Error> {
    if value == 0 || divisor == 0 || !value.is_multiple_of(divisor) {
        return Err(Error::InvalidGeometry {
            field: "divisibility",
        });
    }
    Ok(value / divisor)
}

// ROCr selects the two positive kernel sizes independently, after computing the
// fallback context with the original derived control size. The reviewed sysfs
// profile exports neither size; this private policy also tests newer observations.
fn derive_cwsr(
    input: GeometryInputs,
    kernel_context: Option<u64>,
    kernel_control: Option<u64>,
) -> Result<Gfx950CwsrGeometryV1, Error> {
    let cu = exact_div(input.simd, mul(input.simd_per_cu, input.xcc)?)?;
    let waves = mul(cu, 40)?.min(mul(exact_div(input.arrays, input.arrays_per_engine)?, 512)?);
    let control = align(add(add(40, mul(waves, 8)?)?, 8)?, 4096)?;
    let vgpr_sgpr_hwreg = 0x80000 + 0x4000 + 0x1000;
    let workgroup = align(
        mul(cu, add(vgpr_sgpr_hwreg, mul(input.lds_kib, 1024)?)?)?,
        4096,
    )?;
    let context = kernel_context
        .filter(|value| *value != 0)
        .unwrap_or(add(control, workgroup)?);
    let selected_control = kernel_control
        .filter(|value| *value != 0)
        .unwrap_or(control);
    if input.lds_kib == 0
        || selected_control < 4096
        || !selected_control.is_multiple_of(4096)
        || !context.is_multiple_of(4096)
        || context < add(selected_control, workgroup)?
    {
        return Err(Error::InvalidGeometry {
            field: "selected_cwsr_sizes",
        });
    }
    let debug = align(mul(waves, 32)?, 64)?;
    let mapping = align(mul(add(context, debug)?, input.xcc)?, 4096)?;
    // These bounds also make every exported header/debug/shadow offset exact.
    narrow(mul(context, input.xcc)?)?;
    narrow(mul(debug, input.xcc)?)?;
    Ok(Gfx950CwsrGeometryV1 {
        xcc_count: narrow(input.xcc)?,
        cu_per_xcc: narrow(cu)?,
        waves_per_xcc: narrow(waves)?,
        control_bytes: narrow(selected_control)?,
        workgroup_bytes: narrow(workgroup)?,
        context_bytes: narrow(context)?,
        debug_bytes: narrow(debug)?,
        mapping_bytes: narrow(mapping)?,
    })
}

#[derive(Clone, Copy)]
struct TargetFacts {
    unique_id: u64,
    gpu_id: u64,
    generation: u64,
    target: GfxTarget,
    compute: ComputePartition,
    memory: MemoryPartition,
    geometry: GeometryInputs,
    pci: u32,
    firmware: u32,
    sdma_firmware: u32,
    wavefront: u32,
    max_waves: u32,
    queues: u32,
}

fn host_profile(
    kernel: &str,
    version: Option<&str>,
    source: Option<&str>,
    parameters: [Option<i32>; 3],
    page: usize,
) -> Result<(), Error> {
    for (field, valid) in [
        ("kernel_release", kernel == "6.8.0-124-generic"),
        ("amdgpu_module_version", version == Some("6.16.13")),
        (
            "amdgpu_module_srcversion",
            source == Some("703B1127E578BC5D4BD6615"),
        ),
        (
            "module_parameters",
            parameters == [Some(0), Some(0), Some(1)],
        ),
        ("page_size", page == 4096),
    ] {
        if !valid {
            return Err(Error::HostProfileMismatch { field });
        }
    }
    Ok(())
}

fn plan_from_facts(
    facts: TargetFacts,
    ring_bytes: u32,
) -> Result<Gfx950AqlQueueResourcePlanV1, Error> {
    if facts.target != GfxTarget::Gfx950 {
        return Err(Error::TargetMismatch);
    }
    if facts.compute != ComputePartition::Spx || facts.memory != MemoryPartition::Nps1 {
        return Err(Error::PartitionMismatch);
    }
    let g = facts.geometry;
    for (field, observed, expected) in [
        ("pci_device_id", u64::from(facts.pci), 0x75a0),
        ("firmware", u64::from(facts.firmware), 41),
        ("sdma_firmware", u64::from(facts.sdma_firmware), 12),
        ("simd_count", g.simd, 1024),
        ("simd_per_cu", g.simd_per_cu, 4),
        ("xcc_count", g.xcc, 8),
        ("array_count", g.arrays, 32),
        ("arrays_per_engine", g.arrays_per_engine, 1),
        ("lds_kib", g.lds_kib, 160),
        ("wavefront", u64::from(facts.wavefront), 64),
        ("max_waves_per_simd", u64::from(facts.max_waves), 8),
        ("compute_queue_count", u64::from(facts.queues), 24),
    ] {
        if observed != expected {
            return Err(Error::CapacityMismatch { field });
        }
    }
    if !(4096..=1 << 31).contains(&ring_bytes) || !ring_bytes.is_power_of_two() {
        return Err(Error::RingSizeUnsupported);
    }
    Ok(Gfx950AqlQueueResourcePlanV1 {
        unique_id: facts.unique_id,
        gpu_id: narrow(facts.gpu_id)?,
        topology_generation: facts.generation,
        ring_bytes,
        cwsr: derive_cwsr(g, None, None)?,
    })
}

/// Plans only the observed gfx950 SPX/NPS1 profile. Closed topology parsing
/// rejects unreviewed kernel size properties; no caller-supplied size override
/// can enter this public adapter. This is not device or queue admission.
pub fn plan_gfx950_aql_queue_resources_v1(
    snapshot: &HostTopologySnapshot,
    unique_id: u64,
    ring_bytes: u32,
) -> Result<Gfx950AqlQueueResourcePlanV1, Error> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(Error::HostProfileMismatch { field: "platform" });
    }
    let module = snapshot.amdgpu_module();
    host_profile(
        snapshot.kernel_release().as_str(),
        module.version(),
        module.srcversion(),
        [module.mes(), module.sched_policy(), module.cwsr_enable()],
        rustix::param::page_size(),
    )?;
    let gpu = snapshot
        .topology()
        .gpu_nodes()
        .iter()
        .find(|node| node.unique_id() == unique_id)
        .ok_or(Error::SelectedGpuNotFound)?;
    let render = snapshot
        .render_nodes()
        .iter()
        .find(|node| node.unique_id() == unique_id && node.node_id() == gpu.node_id())
        .ok_or(Error::CorrelatedRenderNotFound)?;
    let c = gpu.capacity();
    plan_from_facts(
        TargetFacts {
            unique_id,
            gpu_id: gpu.gpu_id(),
            generation: snapshot.topology().provenance().generation(),
            target: gpu.target(),
            compute: render.partition().compute(),
            memory: render.partition().memory(),
            geometry: GeometryInputs {
                simd: c.simd_count().into(),
                simd_per_cu: c.simd_per_cu().into(),
                xcc: c.xcc_count().into(),
                arrays: c.array_count().into(),
                arrays_per_engine: c.simd_arrays_per_engine().into(),
                lds_kib: c.lds_size_in_kb().into(),
            },
            pci: gpu.pci_device_id().into(),
            firmware: gpu.fw_version(),
            sdma_firmware: gpu.sdma_fw_version(),
            wavefront: c.wavefront_size(),
            max_waves: c.max_waves_per_simd(),
            queues: c.compute_queue_count(),
        },
        ring_bytes,
    )
}

#[cfg(test)]
#[path = "gfx950_queue_resources/tests.rs"]
mod tests;
