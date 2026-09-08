use core::fmt;

use fe2o3_target_spec::{
    TargetAbiConstraintV1, TargetAddressSpaceV1, TargetAsyncCopyRequirementV1,
    TargetAsyncWaitRequirementV1, TargetAtomicOperationV1, TargetAtomicRequirementV1,
    TargetBarrierParticipationV1, TargetBarrierRequirementV1, TargetCapabilityDecisionOutcomeV1,
    TargetCapabilityModelIdentityErrorV1, TargetCapabilityModelIdentityV1, TargetCapabilityQueryV1,
    TargetCapabilityRequirementV1, TargetCollectiveOperationV1, TargetCollectiveParticipationV1,
    TargetCollectiveRequirementV1, TargetEndiannessV1, TargetExecutionScopeV1,
    TargetFenceRequirementV1, TargetLaunchEvidenceKindV1, TargetMatrixLayoutV1,
    TargetMatrixOperationV1, TargetMatrixRequirementV1, TargetMemoryAccessV1,
    TargetMemoryOrderingV1, TargetMemoryScopeV1, TargetNumericalModeV1,
    TargetNumericalRequirementV1, TargetObjectConstraintV1, TargetObjectFormatV1,
    TargetResourceRequirementV1, TargetScalarEncodingV1, TargetScalarKindV1, TargetScalarTypeV1,
};

use crate::{
    ADVANCED_CAPABILITY_MODEL_REVISION, AdvancedCapabilityStatus, AmdTargetCapabilities,
    CapabilityDerivationError, ProductionAmdTargetProfileV1, WavefrontWidth,
};

/// Canonical revision of the AMD adapter for the neutral V1 query contract.
///
/// The revision includes the advanced AMD model revision because neutral
/// decisions consume those reviewed facts directly.
pub const PRODUCTION_AMD_NEUTRAL_CAPABILITY_MODEL_REVISION_V1: &str =
    "production-neutral-v1-backend-contract-v3";

const PRODUCTION_KERNEL_ARGUMENT_ALIGNMENT_V1: u16 = 8;
const PRODUCTION_KERNEL_ARGUMENT_BYTES_V1: u32 = 1 << 20;
// Conservatively shared with the reviewed gfx942 launch contract in
// fe2o3-kernel-ir/src/launch_kernel_v2.rs.
const PRODUCTION_PRIVATE_BYTES_PER_INVOCATION_V1: u64 = 1 << 20;

const _: () = assert!(ADVANCED_CAPABILITY_MODEL_REVISION.get() == 3);

/// Failure to construct an exact AMD adapter for the neutral V1 query model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionAmdTargetCapabilityModelErrorV1 {
    /// Existing AMD target capabilities could not be derived.
    CapabilityDerivation(CapabilityDerivationError),
    /// The neutral model identity failed canonical validation.
    InvalidModelIdentity(TargetCapabilityModelIdentityErrorV1),
}

impl fmt::Display for ProductionAmdTargetCapabilityModelErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityDerivation(error) => {
                write!(
                    formatter,
                    "could not derive AMD target capabilities: {error}"
                )
            }
            Self::InvalidModelIdentity(error) => {
                write!(
                    formatter,
                    "invalid neutral AMD capability model identity: {error}"
                )
            }
        }
    }
}

impl core::error::Error for ProductionAmdTargetCapabilityModelErrorV1 {}

/// Exact gfx942 or gfx950 adapter for the target-neutral capability contract.
///
/// Construction derives the existing AMD capability records once. Queries do
/// not infer support from unrelated coarse projections: exact advanced facts
/// take precedence, and absent tuple dimensions produce a nonfinal outcome.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProductionAmdTargetCapabilityModelV1 {
    profile: ProductionAmdTargetProfileV1,
    capabilities: AmdTargetCapabilities,
    identity: TargetCapabilityModelIdentityV1,
}

/// Existing production owner that implements or enforces an admitted tuple.
///
/// This is a handoff inventory, not authority. A future lowering may add an
/// owner only together with its reviewed lowering and executable tests.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionAmdCapabilityOwnerV1 {
    /// Scalar, pointer, and ordinary memory lowering.
    ScalarMemoryLowering,
    /// LLVM atomic instruction emission.
    AtomicLowering,
    /// Fence and workgroup-barrier lowering.
    SynchronizationLowering,
    /// Wave operation lowering.
    WaveLowering,
    /// Static LDS tree/scan recipes with exact workgroup participation.
    WorkgroupCollectiveLowering,
    /// Cooperative global-to-LDS transfer and completion lowering.
    AsyncCopyLowering,
    /// Cooperative matrix lowering.
    MatrixLowering,
    /// Static and launch-dependent resource admission.
    ResourceAdmission,
    /// Kernel argument ABI construction and validation.
    KernelAbi,
    /// Loadable code-object production.
    LoadableObjectEmitter,
}

impl ProductionAmdTargetCapabilityModelV1 {
    /// Builds the neutral adapter for one exact AMD production profile.
    pub fn for_profile(
        profile: ProductionAmdTargetProfileV1,
    ) -> Result<Self, ProductionAmdTargetCapabilityModelErrorV1> {
        let identity = TargetCapabilityModelIdentityV1::new_unchecked(
            profile.target_profile_spec(),
            PRODUCTION_AMD_NEUTRAL_CAPABILITY_MODEL_REVISION_V1,
        );
        identity
            .validate()
            .map_err(ProductionAmdTargetCapabilityModelErrorV1::InvalidModelIdentity)?;
        let capabilities = AmdTargetCapabilities::derive(profile.target_id())
            .map_err(ProductionAmdTargetCapabilityModelErrorV1::CapabilityDerivation)?;
        Ok(Self {
            profile,
            capabilities,
            identity,
        })
    }

    /// Returns the exact production profile represented by this model.
    pub const fn profile(self) -> ProductionAmdTargetProfileV1 {
        self.profile
    }

    /// Returns the underlying reviewed AMD capability record.
    pub const fn amd_capabilities(self) -> AmdTargetCapabilities {
        self.capabilities
    }

    fn address_space_outcome(
        space: TargetAddressSpaceV1,
        access: TargetMemoryAccessV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Supported, Unsupported};

        match (space, access) {
            (
                TargetAddressSpaceV1::Global
                | TargetAddressSpaceV1::Workgroup
                | TargetAddressSpaceV1::Private,
                TargetMemoryAccessV1::Read
                | TargetMemoryAccessV1::Write
                | TargetMemoryAccessV1::ReadWrite,
            ) => Supported,
            (
                TargetAddressSpaceV1::Constant | TargetAddressSpaceV1::Generic,
                TargetMemoryAccessV1::Read
                | TargetMemoryAccessV1::Write
                | TargetMemoryAccessV1::ReadWrite,
            ) => Unsupported,
        }
    }

    fn scalar_type_outcome(&self, scalar: TargetScalarTypeV1) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Incomplete, Supported, Unsupported};

        match (scalar.kind(), scalar.bit_width(), scalar.encoding()) {
            (TargetScalarKindV1::Boolean, 1, TargetScalarEncodingV1::Boolean)
            | (
                TargetScalarKindV1::SignedInteger | TargetScalarKindV1::UnsignedInteger,
                8 | 16 | 32 | 64,
                TargetScalarEncodingV1::TwosComplement,
            )
            | (TargetScalarKindV1::Float, 16 | 32, TargetScalarEncodingV1::IeeeBinary)
            | (TargetScalarKindV1::Float, 16, TargetScalarEncodingV1::BFloat) => Supported,
            (TargetScalarKindV1::Float, 64, TargetScalarEncodingV1::IeeeBinary)
            | (
                TargetScalarKindV1::SignedInteger | TargetScalarKindV1::UnsignedInteger,
                _,
                TargetScalarEncodingV1::TwosComplement,
            ) => Unsupported,
            (
                TargetScalarKindV1::Float,
                _,
                TargetScalarEncodingV1::Float4E2M1Ocp
                | TargetScalarEncodingV1::Float8E4M3Fnuz
                | TargetScalarEncodingV1::Float8E5M2Fnuz
                | TargetScalarEncodingV1::Float8E4M3Ocp
                | TargetScalarEncodingV1::Float8E5M2Ocp,
            ) => Incomplete,
            _ => Unsupported,
        }
    }

    fn atomic_outcome(
        &self,
        requirement: TargetAtomicRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{
            DynamicLaunchEvidenceRequired, Supported, Unsupported,
        };

        let value_type = requirement.value_type();
        if !matches!(
            (
                value_type.kind(),
                value_type.bit_width(),
                value_type.encoding()
            ),
            (
                TargetScalarKindV1::SignedInteger | TargetScalarKindV1::UnsignedInteger,
                32 | 64,
                TargetScalarEncodingV1::TwosComplement,
            )
        ) || requirement.operation() == TargetAtomicOperationV1::Nand
        {
            return Unsupported;
        }

        let legal_address_scope = match requirement.address_space() {
            TargetAddressSpaceV1::Workgroup => {
                requirement.scope() == TargetMemoryScopeV1::Workgroup
            }
            TargetAddressSpaceV1::Global => matches!(
                requirement.scope(),
                TargetMemoryScopeV1::Workgroup
                    | TargetMemoryScopeV1::Device
                    | TargetMemoryScopeV1::System
            ),
            TargetAddressSpaceV1::Private
            | TargetAddressSpaceV1::Constant
            | TargetAddressSpaceV1::Generic => false,
        };
        if !legal_address_scope {
            return Unsupported;
        }

        // Both exact production profiles use the same reviewed LLVM atomic
        // emission path. This table is intentionally narrower than the older
        // source-legalizability model: the backend currently emits only i32
        // and i64 atomics and has no NAND KIR operation.
        match self.profile {
            ProductionAmdTargetProfileV1::Gfx942 | ProductionAmdTargetProfileV1::Gfx950 => {}
        }
        if requirement.scope() == TargetMemoryScopeV1::System {
            DynamicLaunchEvidenceRequired(TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility)
        } else {
            Supported
        }
    }

    fn barrier_outcome(
        &self,
        requirement: TargetBarrierRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Supported, Unsupported};

        if requirement.participation() != TargetBarrierParticipationV1::Uniform
            || requirement.ordering() == TargetMemoryOrderingV1::Relaxed
        {
            return Unsupported;
        }
        let matching_scope = matches!(
            (requirement.execution_scope(), requirement.memory_scope()),
            (
                TargetExecutionScopeV1::Subgroup,
                TargetMemoryScopeV1::Subgroup
            ) | (
                TargetExecutionScopeV1::Workgroup,
                TargetMemoryScopeV1::Workgroup
            ) | (
                TargetExecutionScopeV1::Workgroup,
                TargetMemoryScopeV1::Device
            ) | (
                TargetExecutionScopeV1::Workgroup,
                TargetMemoryScopeV1::System
            )
        );
        if !matching_scope {
            return Unsupported;
        }
        let visible = match requirement.memory_scope() {
            TargetMemoryScopeV1::Subgroup | TargetMemoryScopeV1::Workgroup => matches!(
                requirement.address_space(),
                TargetAddressSpaceV1::Global | TargetAddressSpaceV1::Workgroup
            ),
            TargetMemoryScopeV1::Device | TargetMemoryScopeV1::System => {
                requirement.address_space() == TargetAddressSpaceV1::Global
            }
            TargetMemoryScopeV1::Invocation => false,
        };
        if visible { Supported } else { Unsupported }
    }

    fn fence_outcome(
        &self,
        requirement: TargetFenceRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Supported, Unsupported};

        let visible = match requirement.memory_scope() {
            TargetMemoryScopeV1::Subgroup | TargetMemoryScopeV1::Workgroup => matches!(
                requirement.address_space(),
                TargetAddressSpaceV1::Global | TargetAddressSpaceV1::Workgroup
            ),
            TargetMemoryScopeV1::Device | TargetMemoryScopeV1::System => {
                requirement.address_space() == TargetAddressSpaceV1::Global
            }
            TargetMemoryScopeV1::Invocation => false,
        };
        if visible { Supported } else { Unsupported }
    }

    fn collective_outcome(
        &self,
        requirement: TargetCollectiveRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Incomplete, Supported, Unsupported};

        if requirement.execution_scope() == TargetExecutionScopeV1::Grid
            || requirement.participation() != TargetCollectiveParticipationV1::Full
        {
            return Unsupported;
        }

        let value = requirement.value_type();
        let is_bool = matches!(
            (value.kind(), value.bit_width(), value.encoding()),
            (
                TargetScalarKindV1::Boolean,
                1,
                TargetScalarEncodingV1::Boolean
            )
        );
        let is_i32 = matches!(
            (value.kind(), value.bit_width(), value.encoding()),
            (
                TargetScalarKindV1::SignedInteger | TargetScalarKindV1::UnsignedInteger,
                32,
                TargetScalarEncodingV1::TwosComplement
            )
        );
        let is_f32 = matches!(
            (value.kind(), value.bit_width(), value.encoding()),
            (
                TargetScalarKindV1::Float,
                32,
                TargetScalarEncodingV1::IeeeBinary
            )
        );
        let numerical_mode_matches = if is_f32 {
            requirement.numerical_mode() == Some(TargetNumericalModeV1::IeeeStrict)
        } else {
            requirement.numerical_mode().is_none()
        };
        if !numerical_mode_matches {
            return Unsupported;
        }

        match (requirement.execution_scope(), requirement.operation()) {
            (
                TargetExecutionScopeV1::Subgroup,
                TargetCollectiveOperationV1::Ballot
                | TargetCollectiveOperationV1::Any
                | TargetCollectiveOperationV1::All,
            ) if is_bool && requirement.participants() == 64 => Supported,
            (TargetExecutionScopeV1::Subgroup, TargetCollectiveOperationV1::Broadcast)
                if (is_i32 || is_f32)
                    && requirement.participants().is_power_of_two()
                    && requirement.participants() <= 64 =>
            {
                Supported
            }
            (
                TargetExecutionScopeV1::Subgroup,
                TargetCollectiveOperationV1::ReduceAdd
                | TargetCollectiveOperationV1::InclusiveScanAdd
                | TargetCollectiveOperationV1::ExclusiveScanAdd,
            ) if (is_i32 || is_f32)
                && requirement.participants().is_power_of_two()
                && requirement.participants() <= 64 =>
            {
                Supported
            }
            (
                TargetExecutionScopeV1::Subgroup,
                TargetCollectiveOperationV1::ReduceMin | TargetCollectiveOperationV1::ReduceMax,
            ) if is_f32
                && requirement.participants().is_power_of_two()
                && requirement.participants() <= 64 =>
            {
                if self.profile == ProductionAmdTargetProfileV1::Gfx950 {
                    Supported
                } else {
                    Incomplete
                }
            }
            (TargetExecutionScopeV1::Subgroup, _)
                if requirement.participants() > 64
                    || !requirement.participants().is_power_of_two() =>
            {
                Unsupported
            }
            (TargetExecutionScopeV1::Workgroup, _) if requirement.participants() > 1024 => {
                Unsupported
            }
            (TargetExecutionScopeV1::Workgroup, TargetCollectiveOperationV1::ReduceAdd)
                if (is_i32 || is_f32)
                    && requirement.participants() != 0
                    && requirement.participants().is_power_of_two() =>
            {
                Supported
            }
            (
                TargetExecutionScopeV1::Workgroup,
                TargetCollectiveOperationV1::InclusiveScanAdd
                | TargetCollectiveOperationV1::ExclusiveScanAdd,
            ) if (is_i32 || is_f32) && requirement.participants() != 0 => Supported,
            _ => Incomplete,
        }
    }

    fn matrix_outcome(
        &self,
        requirement: TargetMatrixRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Incomplete, Supported, Unsupported};

        if requirement.m() == 0 || requirement.n() == 0 || requirement.k() == 0 {
            return Unsupported;
        }
        if requirement.subgroup_size() != 64
            || requirement.active_participants() != 64
            || !matches!(
                (
                    requirement.lhs_layout(),
                    requirement.rhs_layout(),
                    requirement.output_layout(),
                ),
                (
                    TargetMatrixLayoutV1::CooperativeFragment,
                    TargetMatrixLayoutV1::CooperativeFragment,
                    TargetMatrixLayoutV1::CooperativeFragment
                )
            )
        {
            return Unsupported;
        }

        let lhs = requirement.lhs_type();
        let rhs = requirement.rhs_type();
        let acc = requirement.accumulator_type();
        let bf16_f32 = lhs == TargetScalarTypeV1::bfloat16()
            && rhs == TargetScalarTypeV1::bfloat16()
            && acc == TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32);
        if requirement.operation() == TargetMatrixOperationV1::MatrixMultiplyAccumulate
            && (requirement.m(), requirement.n(), requirement.k()) == (16, 16, 16)
            && bf16_f32
            && requirement.numerical_mode() == TargetNumericalModeV1::AllowContraction
        {
            return Supported;
        }

        let scaled_input = |value: TargetScalarTypeV1| {
            matches!(
                (value.bit_width(), value.encoding()),
                (4, TargetScalarEncodingV1::Float4E2M1Ocp)
                    | (8, TargetScalarEncodingV1::Float8E4M3Ocp)
            )
        };
        if requirement.operation() == TargetMatrixOperationV1::ScaledMatrixMultiplyAccumulate
            && (requirement.m(), requirement.n(), requirement.k()) == (16, 16, 128)
            && scaled_input(lhs)
            && scaled_input(rhs)
            && acc == TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32)
            && requirement.numerical_mode() == TargetNumericalModeV1::AllowApproximation
        {
            return if self.profile == ProductionAmdTargetProfileV1::Gfx950 {
                Supported
            } else {
                Unsupported
            };
        }

        Incomplete
    }

    fn async_copy_outcome(
        &self,
        requirement: TargetAsyncCopyRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Supported, Unsupported};

        if requirement.bytes() == 0
            || requirement.alignment() == 0
            || !requirement.alignment().is_power_of_two()
            || requirement.source() != TargetAddressSpaceV1::Global
            || requirement.destination() != TargetAddressSpaceV1::Workgroup
            || u64::from(requirement.bytes())
                > u64::from(self.capabilities.max_lds_bytes_per_workgroup())
        {
            return Unsupported;
        }
        if !self
            .capabilities
            .async_copy_instruction_sets()
            .contains(crate::AsyncCopyInstructionSet::VmemToLds)
        {
            return Unsupported;
        }

        // The V13 lowerer uses a cooperative, strided VMEM-to-LDS transfer.
        // Native asynchronous selection is an optimization, not a semantic
        // prerequisite for the pending/wait typestate contract.
        Supported
    }

    fn async_wait_outcome(
        &self,
        requirement: TargetAsyncWaitRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Supported, Unsupported};
        if requirement.execution_scope() == TargetExecutionScopeV1::Workgroup
            && requirement.memory_scope() == TargetMemoryScopeV1::Workgroup
            && requirement.ordering() == TargetMemoryOrderingV1::AcquireRelease
            && requirement.max_pending_groups() == 1
        {
            Supported
        } else {
            Unsupported
        }
    }

    fn resource_outcome(
        &self,
        requirement: TargetResourceRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{
            DynamicLaunchEvidenceRequired, Incomplete, Supported, Unreviewed, Unsupported,
        };

        match requirement {
            TargetResourceRequirementV1::WorkgroupInvocationsAtMost(requested) => {
                match self.capabilities.workgroup_limits_support() {
                    AdvancedCapabilityStatus::Supported => {
                        match self.capabilities.workgroup_limits() {
                            Some(limits) if requested <= limits.max_workitems() => Supported,
                            Some(_) => Unsupported,
                            None => Incomplete,
                        }
                    }
                    AdvancedCapabilityStatus::Unsupported => Unsupported,
                    AdvancedCapabilityStatus::Unreviewed => Unreviewed,
                    AdvancedCapabilityStatus::RequiresRuntimeEvidence => Incomplete,
                }
            }
            TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(requested) => {
                if requested <= u64::from(self.capabilities.max_lds_bytes_per_workgroup()) {
                    Supported
                } else {
                    Unsupported
                }
            }
            TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(0) => Supported,
            TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(requested) => {
                if requested <= u64::from(self.capabilities.max_lds_bytes_per_workgroup()) {
                    DynamicLaunchEvidenceRequired(
                        TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes,
                    )
                } else {
                    Unsupported
                }
            }
            TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(requested) => {
                if requested <= PRODUCTION_PRIVATE_BYTES_PER_INVOCATION_V1 {
                    Supported
                } else {
                    Unsupported
                }
            }
            TargetResourceRequirementV1::WorkgroupDimensions { x, y, z } => {
                match self.capabilities.workgroup_limits_support() {
                    AdvancedCapabilityStatus::Supported => {
                        match self.capabilities.workgroup_limits() {
                            Some(limits) if limits.supports_dimensions(x, y, z) => Supported,
                            Some(_) => Unsupported,
                            None => Incomplete,
                        }
                    }
                    AdvancedCapabilityStatus::Unsupported => Unsupported,
                    AdvancedCapabilityStatus::Unreviewed => Unreviewed,
                    AdvancedCapabilityStatus::RequiresRuntimeEvidence => Incomplete,
                }
            }
            TargetResourceRequirementV1::SubgroupsPerWorkgroupAtMost(requested) => {
                if requested <= 16 {
                    Supported
                } else {
                    Unsupported
                }
            }
            TargetResourceRequirementV1::RegistersPerInvocationAtMost(_) => Incomplete,
        }
    }

    fn abi_outcome(&self, requirement: TargetAbiConstraintV1) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Supported, Unsupported};

        match requirement {
            TargetAbiConstraintV1::PointerWidth(64)
            | TargetAbiConstraintV1::Endianness(TargetEndiannessV1::Little) => Supported,
            TargetAbiConstraintV1::PointerWidth(_)
            | TargetAbiConstraintV1::Endianness(TargetEndiannessV1::Big) => Unsupported,
            TargetAbiConstraintV1::KernelArgumentAlignmentAtMost(required) => {
                if required <= PRODUCTION_KERNEL_ARGUMENT_ALIGNMENT_V1 {
                    Supported
                } else {
                    Unsupported
                }
            }
            TargetAbiConstraintV1::KernelArgumentSegmentBytesAtMost(required) => {
                if required <= PRODUCTION_KERNEL_ARGUMENT_BYTES_V1 {
                    Supported
                } else {
                    Unsupported
                }
            }
        }
    }

    fn object_outcome(
        &self,
        requirement: TargetObjectConstraintV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Supported, Unsupported};

        match requirement {
            TargetObjectConstraintV1::Format(TargetObjectFormatV1::LoadableExecutable) => Supported,
            TargetObjectConstraintV1::Format(
                TargetObjectFormatV1::RelocatableObject | TargetObjectFormatV1::PortableModule,
            ) => Unsupported,
            TargetObjectConstraintV1::Relocatable(false) => Supported,
            TargetObjectConstraintV1::Relocatable(true) => Unsupported,
        }
    }

    fn numerical_outcome(
        &self,
        requirement: TargetNumericalRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Incomplete, Supported, Unsupported};

        let value = requirement.value_type();
        match (value.bit_width(), value.encoding()) {
            (32, TargetScalarEncodingV1::IeeeBinary) => Supported,
            (16, TargetScalarEncodingV1::IeeeBinary | TargetScalarEncodingV1::BFloat) => Incomplete,
            (64, TargetScalarEncodingV1::IeeeBinary) => Unsupported,
            _ => Incomplete,
        }
    }

    /// Returns the existing production owner for a final target decision.
    ///
    /// `None` means the tuple must not be admitted even if a capability table
    /// is accidentally broadened without landing its lowering owner.
    pub fn capability_owner(
        &self,
        requirement: TargetCapabilityRequirementV1,
    ) -> Option<ProductionAmdCapabilityOwnerV1> {
        use TargetCapabilityDecisionOutcomeV1::{DynamicLaunchEvidenceRequired, Supported};

        if !matches!(
            self.query_outcome(requirement),
            Supported | DynamicLaunchEvidenceRequired(_)
        ) {
            return None;
        }
        match requirement {
            TargetCapabilityRequirementV1::ScalarType(_)
            | TargetCapabilityRequirementV1::AddressSpace(_, _)
            | TargetCapabilityRequirementV1::Numerical(_) => {
                Some(ProductionAmdCapabilityOwnerV1::ScalarMemoryLowering)
            }
            TargetCapabilityRequirementV1::SubgroupSize(_) => {
                Some(ProductionAmdCapabilityOwnerV1::WaveLowering)
            }
            TargetCapabilityRequirementV1::Collective(collective) => {
                match collective.execution_scope() {
                    TargetExecutionScopeV1::Subgroup => {
                        Some(ProductionAmdCapabilityOwnerV1::WaveLowering)
                    }
                    TargetExecutionScopeV1::Workgroup => {
                        Some(ProductionAmdCapabilityOwnerV1::WorkgroupCollectiveLowering)
                    }
                    TargetExecutionScopeV1::Grid => None,
                }
            }
            TargetCapabilityRequirementV1::Atomic(_) => {
                Some(ProductionAmdCapabilityOwnerV1::AtomicLowering)
            }
            TargetCapabilityRequirementV1::Barrier(_) | TargetCapabilityRequirementV1::Fence(_) => {
                Some(ProductionAmdCapabilityOwnerV1::SynchronizationLowering)
            }
            TargetCapabilityRequirementV1::Matrix(_) => {
                Some(ProductionAmdCapabilityOwnerV1::MatrixLowering)
            }
            TargetCapabilityRequirementV1::Resource(_) => {
                Some(ProductionAmdCapabilityOwnerV1::ResourceAdmission)
            }
            TargetCapabilityRequirementV1::Abi(_) => {
                Some(ProductionAmdCapabilityOwnerV1::KernelAbi)
            }
            TargetCapabilityRequirementV1::Object(_) => {
                Some(ProductionAmdCapabilityOwnerV1::LoadableObjectEmitter)
            }
            TargetCapabilityRequirementV1::AsyncCopy(_)
            | TargetCapabilityRequirementV1::AsyncWait(_) => {
                Some(ProductionAmdCapabilityOwnerV1::AsyncCopyLowering)
            }
        }
    }
}

impl TargetCapabilityQueryV1 for ProductionAmdTargetCapabilityModelV1 {
    fn model_identity(&self) -> TargetCapabilityModelIdentityV1 {
        self.identity
    }

    fn query_outcome(
        &self,
        requirement: TargetCapabilityRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        use TargetCapabilityDecisionOutcomeV1::{Supported, Unsupported};

        match requirement {
            TargetCapabilityRequirementV1::ScalarType(scalar) => self.scalar_type_outcome(scalar),
            TargetCapabilityRequirementV1::SubgroupSize(32) => {
                if self
                    .capabilities
                    .wavefront_widths()
                    .contains(WavefrontWidth::Wave32)
                {
                    Supported
                } else {
                    Unsupported
                }
            }
            TargetCapabilityRequirementV1::SubgroupSize(64) => {
                if self
                    .capabilities
                    .wavefront_widths()
                    .contains(WavefrontWidth::Wave64)
                {
                    Supported
                } else {
                    Unsupported
                }
            }
            TargetCapabilityRequirementV1::SubgroupSize(_) => Unsupported,
            TargetCapabilityRequirementV1::AddressSpace(space, access) => {
                Self::address_space_outcome(space, access)
            }
            TargetCapabilityRequirementV1::Atomic(atomic) => self.atomic_outcome(atomic),
            TargetCapabilityRequirementV1::Barrier(barrier) => self.barrier_outcome(barrier),
            TargetCapabilityRequirementV1::Fence(fence) => self.fence_outcome(fence),
            TargetCapabilityRequirementV1::Collective(collective) => {
                self.collective_outcome(collective)
            }
            TargetCapabilityRequirementV1::Matrix(matrix) => self.matrix_outcome(matrix),
            TargetCapabilityRequirementV1::AsyncCopy(copy) => self.async_copy_outcome(copy),
            TargetCapabilityRequirementV1::AsyncWait(wait) => self.async_wait_outcome(wait),
            TargetCapabilityRequirementV1::Numerical(numerical) => {
                self.numerical_outcome(numerical)
            }
            TargetCapabilityRequirementV1::Resource(resource) => self.resource_outcome(resource),
            TargetCapabilityRequirementV1::Abi(abi) => self.abi_outcome(abi),
            TargetCapabilityRequirementV1::Object(object) => self.object_outcome(object),
        }
    }
}

impl ProductionAmdTargetProfileV1 {
    /// Builds this profile's exact target-neutral capability adapter.
    pub fn capability_model(
        self,
    ) -> Result<ProductionAmdTargetCapabilityModelV1, ProductionAmdTargetCapabilityModelErrorV1>
    {
        ProductionAmdTargetCapabilityModelV1::for_profile(self)
    }
}
