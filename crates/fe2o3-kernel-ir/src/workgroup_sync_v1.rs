//! Closed semantic profiles for the ordinary workgroup synchronization sources.
//!
//! These profiles are reviewed semantic sidecars selected only after a compiler
//! authenticates the exact attributed source, its complete reachable MIR modulo
//! an identity-bound reviewed semantic-terminal manifest, and that manifest's
//! providers. They are not generic lowering results, terminal-body refinement,
//! or compiler-refinement proofs and grant no Worker V2, finalization, artifact,
//! host, runtime, or execution authority.

use std::error::Error;
use std::fmt;

use crate::{ScalarType, TargetCapability, WaveWidth, WorkgroupSize};

pub const LDS_REDUCTION_V1_MODULE_ID: &str = "fe2o3::workgroup_lds_reduction_v1";
pub const LDS_REDUCTION_V1_FUNCTION_ID: &str = "__fe2o3_lds_publish_read_reduce_i32_v1_impl";
pub const LDS_REDUCTION_V1_KERNEL_ID: &str = "lds_publish_read_reduce_i32_v1";
pub const LDS_REDUCTION_V1_DESCRIPTOR_SYMBOL: &str = "lds_publish_read_reduce_i32_v1.kd";
pub const LDS_REDUCTION_V1_EXPLICIT_KERNARG_BYTES: u32 = 40;
pub const LDS_REDUCTION_V1_COMPLETE_COV6_KERNARG_BYTES: u32 = 296;
pub const LDS_REDUCTION_V1_SOURCE_SHA256: [u8; 32] = [
    0x3e, 0x7e, 0xc0, 0x81, 0xc7, 0x95, 0x82, 0x88, 0xf9, 0xd9, 0x97, 0xd4, 0x0e, 0x6f, 0x41, 0xa7,
    0xfa, 0xab, 0xc5, 0x6a, 0x3a, 0xdd, 0x73, 0x40, 0x99, 0xcd, 0x17, 0x77, 0x44, 0x3b, 0x29, 0x83,
];
pub const LDS_REDUCTION_V1_NAMESPACE: [u8; 32] = [
    0x6b, 0xc8, 0xf4, 0x49, 0xf4, 0x58, 0xcf, 0x8f, 0x31, 0xb4, 0x62, 0x5b, 0x38, 0xb7, 0x20, 0x4d,
    0xd3, 0x4f, 0x20, 0xbe, 0xea, 0xbb, 0x80, 0xb5, 0x54, 0x54, 0xa5, 0x66, 0x6b, 0xe7, 0x49, 0xb5,
];

pub const SCOPED_ATOMIC_V1_MODULE_ID: &str = "fe2o3::scoped_atomic_add_v1";
pub const SCOPED_ATOMIC_V1_FUNCTION_ID: &str = "__fe2o3_scoped_atomic_add_u32_v1_impl";
pub const SCOPED_ATOMIC_V1_KERNEL_ID: &str = "scoped_atomic_add_u32_v1";
pub const SCOPED_ATOMIC_V1_DESCRIPTOR_SYMBOL: &str = "scoped_atomic_add_u32_v1.kd";
pub const SCOPED_ATOMIC_V1_EXPLICIT_KERNARG_BYTES: u32 = 40;
pub const SCOPED_ATOMIC_V1_COMPLETE_COV6_KERNARG_BYTES: u32 = 296;
pub const SCOPED_ATOMIC_V1_SOURCE_SHA256: [u8; 32] = [
    0xc0, 0xf0, 0x0a, 0x14, 0xc5, 0x94, 0x1f, 0x34, 0x74, 0x1f, 0xc1, 0x0c, 0xa7, 0x79, 0x8c, 0xe9,
    0xcf, 0x47, 0x28, 0x82, 0x94, 0xb0, 0xbc, 0xc4, 0x3c, 0xdd, 0xb7, 0xd2, 0x2b, 0xbf, 0xe9, 0x7e,
];
pub const SCOPED_ATOMIC_V1_NAMESPACE: [u8; 32] = [
    0x40, 0x93, 0x57, 0xef, 0x99, 0xd9, 0xec, 0x78, 0xc9, 0x60, 0xcc, 0xa0, 0xe2, 0x1a, 0x4e, 0x15,
    0x3c, 0x60, 0xaf, 0x52, 0x2c, 0x1c, 0x4d, 0x72, 0x6a, 0x9f, 0x23, 0xb5, 0xc7, 0x27, 0x1b, 0x91,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkgroupSyncArgumentRoleV1 {
    Values,
    Epoch,
    ReductionOutput,
    Eligibility,
    AtomicTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkgroupSyncArgumentShapeV1 {
    SharedReadOnlySlice64,
    Scalar,
    LaneZeroOwnedWriteSlice1,
    UniqueGlobalAtomicObject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupSyncArgumentV1 {
    pub role: WorkgroupSyncArgumentRoleV1,
    pub shape: WorkgroupSyncArgumentShapeV1,
    pub scalar: ScalarType,
    pub offset: u32,
    pub size: u32,
    pub alignment: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkgroupBarrierKindV1 {
    PublishToRead,
    ReadToReuse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupBarrierV1 {
    pub ordinal: u8,
    pub kind: WorkgroupBarrierKindV1,
    pub convergent_threads: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LdsEpochV1 {
    Uninitialized,
    LaneInitialized,
    Published,
    Read,
    Reusable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinearLdsCapabilityV1 {
    pub element: ScalarType,
    pub elements: u32,
    pub bytes: u32,
    pub alignment: u32,
    pub allocation_count: u32,
    pub initial_epoch: LdsEpochV1,
    pub final_epoch: LdsEpochV1,
    pub pointer_escape: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReductionOperationV1 {
    WrappingI32SumEquivalentToAdmittedExactSum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputOwnershipV1 {
    LaneZeroOwnsOnlyElement,
    AnyLaneMayWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LdsReductionKernelIrV1 {
    pub module_id: &'static str,
    pub function_id: &'static str,
    pub kernel_id: &'static str,
    pub arguments: [WorkgroupSyncArgumentV1; 3],
    pub lds: LinearLdsCapabilityV1,
    pub barriers: [WorkgroupBarrierV1; 2],
    pub operation: ReductionOperationV1,
    pub output_ownership: OutputOwnershipV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicOperationV1 {
    FetchAdd,
    Exchange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicScopeV1 {
    System,
    Workgroup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicOrderingV1 {
    Relaxed,
    SequentiallyConsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicAddressSpaceV1 {
    Global,
    Workgroup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicParticipationV1 {
    EligibleLaneExactlyOnce,
    EveryLaneExactlyOnce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopedAtomicKernelIrV1 {
    pub module_id: &'static str,
    pub function_id: &'static str,
    pub kernel_id: &'static str,
    pub arguments: [WorkgroupSyncArgumentV1; 3],
    pub operation: AtomicOperationV1,
    pub scalar: ScalarType,
    pub scope: AtomicScopeV1,
    pub ordering: AtomicOrderingV1,
    pub address_space: AtomicAddressSpaceV1,
    pub participation: AtomicParticipationV1,
    pub unique_host_borrow: bool,
    pub device_lanes_alias_one_atomic: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkgroupSyncCorrespondenceV1 {
    ReviewedExactSourceAndMirToCanonicalProfileNotRefinementProof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkgroupSyncDescriptorV1 {
    pub logical_name: String,
    pub export_name: String,
    pub descriptor_symbol: String,
    pub code_object_version: u8,
    pub explicit_kernarg_bytes: u32,
    pub complete_kernarg_bytes: u32,
    pub workgroup_size: WorkgroupSize,
    pub wave_width: WaveWidth,
    pub static_lds_bytes: u32,
    pub required_dynamic_lds_bytes: u32,
    pub maximum_dynamic_lds_bytes: u32,
    pub hidden_dynamic_lds_size: Option<Cov6HiddenDynamicLdsSizeV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cov6HiddenDynamicLdsSizeV1 {
    pub relative_offset: u32,
    pub field_size: u32,
    pub required_launch_value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LdsReductionProfileV1 {
    pub source_sha256: [u8; 32],
    pub namespace: [u8; 32],
    pub target: TargetCapability,
    pub code_object_version: u8,
    pub wave_width: WaveWidth,
    pub workgroup_size: WorkgroupSize,
    pub grid: [u32; 3],
    pub correspondence: WorkgroupSyncCorrespondenceV1,
    pub descriptor: WorkgroupSyncDescriptorV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedAtomicProfileV1 {
    pub source_sha256: [u8; 32],
    pub namespace: [u8; 32],
    pub target: TargetCapability,
    pub code_object_version: u8,
    pub wave_width: WaveWidth,
    pub workgroup_size: WorkgroupSize,
    pub grid: [u32; 3],
    pub correspondence: WorkgroupSyncCorrespondenceV1,
    pub descriptor: WorkgroupSyncDescriptorV1,
}

impl LdsReductionProfileV1 {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        profile(
            LDS_REDUCTION_V1_SOURCE_SHA256,
            LDS_REDUCTION_V1_NAMESPACE,
            LDS_REDUCTION_V1_KERNEL_ID,
            LDS_REDUCTION_V1_DESCRIPTOR_SYMBOL,
            LDS_REDUCTION_V1_EXPLICIT_KERNARG_BYTES,
            LDS_REDUCTION_V1_COMPLETE_COV6_KERNARG_BYTES,
            0,
            256,
            Some(Cov6HiddenDynamicLdsSizeV1 {
                relative_offset: 120,
                field_size: 4,
                required_launch_value: 256,
            }),
        )
    }

    pub fn is_exact(&self) -> bool {
        self == &Self::exact_gfx942_xnack_minus_cov6()
    }
}

impl ScopedAtomicProfileV1 {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        let exact = profile(
            SCOPED_ATOMIC_V1_SOURCE_SHA256,
            SCOPED_ATOMIC_V1_NAMESPACE,
            SCOPED_ATOMIC_V1_KERNEL_ID,
            SCOPED_ATOMIC_V1_DESCRIPTOR_SYMBOL,
            SCOPED_ATOMIC_V1_EXPLICIT_KERNARG_BYTES,
            SCOPED_ATOMIC_V1_COMPLETE_COV6_KERNARG_BYTES,
            0,
            0,
            None,
        );
        Self {
            source_sha256: exact.source_sha256,
            namespace: exact.namespace,
            target: exact.target,
            code_object_version: exact.code_object_version,
            wave_width: exact.wave_width,
            workgroup_size: exact.workgroup_size,
            grid: exact.grid,
            correspondence: exact.correspondence,
            descriptor: exact.descriptor,
        }
    }

    pub fn is_exact(&self) -> bool {
        self == &Self::exact_gfx942_xnack_minus_cov6()
    }
}

pub const fn lds_reduction_v1_kernel_ir() -> LdsReductionKernelIrV1 {
    LdsReductionKernelIrV1 {
        module_id: LDS_REDUCTION_V1_MODULE_ID,
        function_id: LDS_REDUCTION_V1_FUNCTION_ID,
        kernel_id: LDS_REDUCTION_V1_KERNEL_ID,
        arguments: [
            argument(
                WorkgroupSyncArgumentRoleV1::Values,
                WorkgroupSyncArgumentShapeV1::SharedReadOnlySlice64,
                ScalarType::I32,
                0,
                16,
                8,
            ),
            argument(
                WorkgroupSyncArgumentRoleV1::Epoch,
                WorkgroupSyncArgumentShapeV1::Scalar,
                ScalarType::U32,
                16,
                4,
                4,
            ),
            argument(
                WorkgroupSyncArgumentRoleV1::ReductionOutput,
                WorkgroupSyncArgumentShapeV1::LaneZeroOwnedWriteSlice1,
                ScalarType::I32,
                24,
                16,
                8,
            ),
        ],
        lds: LinearLdsCapabilityV1 {
            element: ScalarType::I32,
            elements: 64,
            bytes: 256,
            alignment: 4,
            allocation_count: 1,
            initial_epoch: LdsEpochV1::Uninitialized,
            final_epoch: LdsEpochV1::Reusable,
            pointer_escape: false,
        },
        barriers: [
            WorkgroupBarrierV1 {
                ordinal: 0,
                kind: WorkgroupBarrierKindV1::PublishToRead,
                convergent_threads: 64,
            },
            WorkgroupBarrierV1 {
                ordinal: 1,
                kind: WorkgroupBarrierKindV1::ReadToReuse,
                convergent_threads: 64,
            },
        ],
        operation: ReductionOperationV1::WrappingI32SumEquivalentToAdmittedExactSum,
        output_ownership: OutputOwnershipV1::LaneZeroOwnsOnlyElement,
    }
}

pub const fn scoped_atomic_v1_kernel_ir() -> ScopedAtomicKernelIrV1 {
    ScopedAtomicKernelIrV1 {
        module_id: SCOPED_ATOMIC_V1_MODULE_ID,
        function_id: SCOPED_ATOMIC_V1_FUNCTION_ID,
        kernel_id: SCOPED_ATOMIC_V1_KERNEL_ID,
        arguments: [
            argument(
                WorkgroupSyncArgumentRoleV1::Values,
                WorkgroupSyncArgumentShapeV1::SharedReadOnlySlice64,
                ScalarType::U32,
                0,
                16,
                8,
            ),
            argument(
                WorkgroupSyncArgumentRoleV1::Eligibility,
                WorkgroupSyncArgumentShapeV1::SharedReadOnlySlice64,
                ScalarType::U32,
                16,
                16,
                8,
            ),
            argument(
                WorkgroupSyncArgumentRoleV1::AtomicTarget,
                WorkgroupSyncArgumentShapeV1::UniqueGlobalAtomicObject,
                ScalarType::U32,
                32,
                8,
                8,
            ),
        ],
        operation: AtomicOperationV1::FetchAdd,
        scalar: ScalarType::U32,
        scope: AtomicScopeV1::System,
        ordering: AtomicOrderingV1::Relaxed,
        address_space: AtomicAddressSpaceV1::Global,
        participation: AtomicParticipationV1::EligibleLaneExactlyOnce,
        unique_host_borrow: true,
        device_lanes_alias_one_atomic: true,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkgroupSyncV1Error {
    UnsupportedProfile,
    NonCanonicalKernelIr,
}

impl fmt::Display for WorkgroupSyncV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str(
                "workgroup synchronization V1 requires exact source/namespace, gfx942:xnack-, COV6, Wave64, WG64, one-workgroup geometry, resources, ABI, and descriptor",
            ),
            Self::NonCanonicalKernelIr => formatter.write_str(
                "workgroup synchronization semantic Kernel IR is not the reviewed canonical sidecar",
            ),
        }
    }
}

impl Error for WorkgroupSyncV1Error {}

pub fn verify_lds_reduction_v1(
    ir: &LdsReductionKernelIrV1,
    profile: &LdsReductionProfileV1,
) -> Result<(), WorkgroupSyncV1Error> {
    if !profile.is_exact() {
        return Err(WorkgroupSyncV1Error::UnsupportedProfile);
    }
    if ir != &lds_reduction_v1_kernel_ir() {
        return Err(WorkgroupSyncV1Error::NonCanonicalKernelIr);
    }
    Ok(())
}

pub fn verify_scoped_atomic_v1(
    ir: &ScopedAtomicKernelIrV1,
    profile: &ScopedAtomicProfileV1,
) -> Result<(), WorkgroupSyncV1Error> {
    if !profile.is_exact() {
        return Err(WorkgroupSyncV1Error::UnsupportedProfile);
    }
    if ir != &scoped_atomic_v1_kernel_ir() {
        return Err(WorkgroupSyncV1Error::NonCanonicalKernelIr);
    }
    Ok(())
}

const fn argument(
    role: WorkgroupSyncArgumentRoleV1,
    shape: WorkgroupSyncArgumentShapeV1,
    scalar: ScalarType,
    offset: u32,
    size: u32,
    alignment: u32,
) -> WorkgroupSyncArgumentV1 {
    WorkgroupSyncArgumentV1 {
        role,
        shape,
        scalar,
        offset,
        size,
        alignment,
    }
}

fn profile(
    source_sha256: [u8; 32],
    namespace: [u8; 32],
    kernel_id: &str,
    descriptor_symbol: &str,
    explicit_kernarg_bytes: u32,
    complete_kernarg_bytes: u32,
    static_lds_bytes: u32,
    required_dynamic_lds_bytes: u32,
    hidden_dynamic_lds_size: Option<Cov6HiddenDynamicLdsSizeV1>,
) -> LdsReductionProfileV1 {
    let workgroup_size = WorkgroupSize::new(64, 1, 1);
    LdsReductionProfileV1 {
        source_sha256,
        namespace,
        target: crate::gfx942_xnack_minus_target_capability(),
        code_object_version: 6,
        wave_width: WaveWidth::Wave64,
        workgroup_size,
        grid: [1, 1, 1],
        correspondence:
            WorkgroupSyncCorrespondenceV1::ReviewedExactSourceAndMirToCanonicalProfileNotRefinementProof,
        descriptor: WorkgroupSyncDescriptorV1 {
            logical_name: kernel_id.to_owned(),
            export_name: kernel_id.to_owned(),
            descriptor_symbol: descriptor_symbol.to_owned(),
            code_object_version: 6,
            explicit_kernarg_bytes,
            complete_kernarg_bytes,
            workgroup_size,
            wave_width: WaveWidth::Wave64,
            static_lds_bytes,
            required_dynamic_lds_bytes,
            maximum_dynamic_lds_bytes: required_dynamic_lds_bytes,
            hidden_dynamic_lds_size,
        },
    }
}
