//! Synthetic target used to test that capability consumers remain portable.
//!
//! This model deliberately differs from the production targets currently
//! shipped by fe2o3. It has no compiler triple, data layout, lowering, object
//! writer, runtime, or launch implementation. Its decisions are useful only as
//! inert conformance inputs for target-neutral compiler components.

use crate::{
    TargetAbiConstraintV1, TargetAddressSpaceV1, TargetArchitectureFamilyV1,
    TargetArtifactFormatV1, TargetAtomicOperationV1, TargetBarrierParticipationV1,
    TargetCapabilityDecisionOutcomeV1, TargetCapabilityModelIdentityV1, TargetCapabilityQueryV1,
    TargetCapabilityRequirementV1, TargetCollectiveOperationV1, TargetCollectiveParticipationV1,
    TargetEndiannessV1, TargetExecutionModelV1, TargetExecutionScopeV1, TargetMatrixLayoutV1,
    TargetMatrixOperationV1, TargetMemoryAccessV1, TargetMemoryOrderingV1, TargetMemoryScopeV1,
    TargetNumericalModeV1, TargetObjectConstraintV1, TargetObjectFormatV1, TargetProfileSpecV1,
    TargetResourceRequirementV1, TargetScalarEncodingV1, TargetScalarKindV1, TargetScalarTypeV1,
    TargetVendorV1,
};

/// Exact profile for the synthetic, vendor-independent conformance target.
pub const SYNTHETIC_CONFORMANCE_TARGET_PROFILE_V1: TargetProfileSpecV1 =
    TargetProfileSpecV1::from_static_parts(
        TargetVendorV1::Other,
        TargetArchitectureFamilyV1::Other,
        "synthetic-grid-v1",
        None,
        None,
        TargetArtifactFormatV1::Unknown,
        TargetExecutionModelV1::GpuGrid,
        None,
        &[],
    );

/// Exact capability-model identity for the synthetic conformance target.
pub const SYNTHETIC_CONFORMANCE_TARGET_MODEL_V1: TargetCapabilityModelIdentityV1 =
    TargetCapabilityModelIdentityV1::new_unchecked(
        SYNTHETIC_CONFORMANCE_TARGET_PROFILE_V1,
        "synthetic-conformance-v1",
    );

const U16: TargetScalarTypeV1 = TargetScalarTypeV1::new(TargetScalarKindV1::UnsignedInteger, 16);
const F32: TargetScalarTypeV1 = TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32);

/// Bounded capability provider for portability and fail-closed conformance tests.
///
/// The unusual 32-bit big-endian ABI, subgroup sizes, matrix shape, and transfer
/// width make accidental dependence on a currently shipped backend observable.
/// A positive answer grants no lowering, artifact, publication, load, or launch
/// authority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SyntheticConformanceTargetV1;

impl SyntheticConformanceTargetV1 {
    fn supported(condition: bool) -> TargetCapabilityDecisionOutcomeV1 {
        if condition {
            TargetCapabilityDecisionOutcomeV1::Supported
        } else {
            TargetCapabilityDecisionOutcomeV1::Unsupported
        }
    }

    fn scalar_supported(scalar: TargetScalarTypeV1) -> bool {
        matches!(
            (scalar.kind(), scalar.bit_width(), scalar.encoding()),
            (
                TargetScalarKindV1::Boolean,
                1,
                TargetScalarEncodingV1::Boolean
            ) | (
                TargetScalarKindV1::SignedInteger | TargetScalarKindV1::UnsignedInteger,
                8 | 16 | 32,
                TargetScalarEncodingV1::TwosComplement
            ) | (
                TargetScalarKindV1::Float,
                32,
                TargetScalarEncodingV1::IeeeBinary
            )
        )
    }

    fn address_space_supported(space: TargetAddressSpaceV1, access: TargetMemoryAccessV1) -> bool {
        match space {
            TargetAddressSpaceV1::Global
            | TargetAddressSpaceV1::Workgroup
            | TargetAddressSpaceV1::Private => true,
            TargetAddressSpaceV1::Constant => access == TargetMemoryAccessV1::Read,
            TargetAddressSpaceV1::Generic => false,
        }
    }

    fn outcome(requirement: TargetCapabilityRequirementV1) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{
            DynamicLaunchEvidenceRequired, Incomplete, Unreviewed,
        };

        match requirement {
            TargetCapabilityRequirementV1::ScalarType(scalar) => {
                Self::supported(Self::scalar_supported(scalar))
            }
            TargetCapabilityRequirementV1::SubgroupSize(size) => {
                Self::supported(matches!(size, 8 | 16))
            }
            TargetCapabilityRequirementV1::AddressSpace(space, access) => {
                Self::supported(Self::address_space_supported(space, access))
            }
            TargetCapabilityRequirementV1::Atomic(atomic) => Self::supported(
                atomic.value_type() == U16
                    && atomic.operation() == TargetAtomicOperationV1::Add
                    && atomic.ordering() == TargetMemoryOrderingV1::Relaxed
                    && atomic.failure_ordering().is_none()
                    && atomic.scope() == TargetMemoryScopeV1::Device
                    && atomic.address_space() == TargetAddressSpaceV1::Global,
            ),
            TargetCapabilityRequirementV1::Barrier(barrier) => Self::supported(
                barrier.execution_scope() == TargetExecutionScopeV1::Workgroup
                    && barrier.memory_scope() == TargetMemoryScopeV1::Workgroup
                    && barrier.address_space() == TargetAddressSpaceV1::Workgroup
                    && barrier.ordering() == TargetMemoryOrderingV1::AcquireRelease
                    && barrier.participation() == TargetBarrierParticipationV1::Uniform,
            ),
            TargetCapabilityRequirementV1::Fence(fence) => Self::supported(
                fence.memory_scope() == TargetMemoryScopeV1::Workgroup
                    && fence.address_space() == TargetAddressSpaceV1::Workgroup
                    && fence.ordering() == TargetMemoryOrderingV1::AcquireRelease,
            ),
            TargetCapabilityRequirementV1::Collective(collective) => Self::supported(
                collective.execution_scope() == TargetExecutionScopeV1::Subgroup
                    && collective.operation() == TargetCollectiveOperationV1::ReduceAdd
                    && collective.value_type() == U16
                    && collective.participants() == 16
                    && collective.participation() == TargetCollectiveParticipationV1::Full
                    && collective.numerical_mode().is_none(),
            ),
            TargetCapabilityRequirementV1::Matrix(matrix) => Self::supported(
                matrix.operation() == TargetMatrixOperationV1::TensorContraction
                    && (matrix.m(), matrix.n(), matrix.k()) == (8, 4, 2)
                    && matrix.lhs_type() == F32
                    && matrix.rhs_type() == F32
                    && matrix.accumulator_type() == F32
                    && matrix.lhs_layout() == TargetMatrixLayoutV1::RowMajor
                    && matrix.rhs_layout() == TargetMatrixLayoutV1::RowMajor
                    && matrix.output_layout() == TargetMatrixLayoutV1::ColumnMajor
                    && matrix.numerical_mode() == TargetNumericalModeV1::AllowContraction
                    && matrix.subgroup_size() == 16
                    && matrix.active_participants() == 8,
            ),
            TargetCapabilityRequirementV1::AsyncCopy(copy) => Self::supported(
                copy.source() == TargetAddressSpaceV1::Global
                    && copy.destination() == TargetAddressSpaceV1::Workgroup
                    && copy.bytes() == 12
                    && copy.alignment() == 4,
            ),
            TargetCapabilityRequirementV1::AsyncWait(wait) => Self::supported(
                wait.execution_scope() == TargetExecutionScopeV1::Workgroup
                    && wait.memory_scope() == TargetMemoryScopeV1::Workgroup
                    && wait.ordering() == TargetMemoryOrderingV1::Acquire
                    && wait.max_pending_groups() == 1,
            ),
            TargetCapabilityRequirementV1::Numerical(numerical) => Self::supported(
                numerical.value_type() == F32
                    && numerical.mode() == TargetNumericalModeV1::IeeeStrict,
            ),
            TargetCapabilityRequirementV1::Resource(resource) => match resource {
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(value) => {
                    Self::supported(value <= 192)
                }
                TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(value) => {
                    Self::supported(value <= 24 * 1024)
                }
                TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(value)
                    if value <= 12 * 1024 =>
                {
                    DynamicLaunchEvidenceRequired(
                        crate::TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes,
                    )
                }
                TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(_) => {
                    TargetCapabilityDecisionOutcomeV1::Unsupported
                }
                TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(_) => Incomplete,
                TargetResourceRequirementV1::WorkgroupDimensions { x, y, z } => Self::supported(
                    x <= 96
                        && y <= 32
                        && z <= 3
                        && x.checked_mul(y).and_then(|xy| xy.checked_mul(z)) <= Some(192),
                ),
                TargetResourceRequirementV1::SubgroupsPerWorkgroupAtMost(value) => {
                    Self::supported(value <= 24)
                }
                TargetResourceRequirementV1::RegistersPerInvocationAtMost(_) => Unreviewed,
            },
            TargetCapabilityRequirementV1::Abi(
                TargetAbiConstraintV1::KernelArgumentSegmentBytesAtMost(_),
            ) => Unreviewed,
            TargetCapabilityRequirementV1::Abi(abi) => Self::supported(matches!(
                abi,
                TargetAbiConstraintV1::PointerWidth(32)
                    | TargetAbiConstraintV1::Endianness(TargetEndiannessV1::Big)
                    | TargetAbiConstraintV1::KernelArgumentAlignmentAtMost(1 | 2 | 4 | 8)
            )),
            TargetCapabilityRequirementV1::Object(object) => Self::supported(matches!(
                object,
                TargetObjectConstraintV1::Format(TargetObjectFormatV1::PortableModule)
                    | TargetObjectConstraintV1::Relocatable(false)
            )),
        }
    }
}

impl TargetCapabilityQueryV1 for SyntheticConformanceTargetV1 {
    fn model_identity(&self) -> TargetCapabilityModelIdentityV1 {
        SYNTHETIC_CONFORMANCE_TARGET_MODEL_V1
    }

    fn query_outcome(
        &self,
        requirement: TargetCapabilityRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        Self::outcome(requirement)
    }
}
