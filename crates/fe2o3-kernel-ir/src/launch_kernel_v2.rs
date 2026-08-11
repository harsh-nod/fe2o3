//! Inert gfx942 launch and kernel-family contract model.
//!
//! This file is intentionally not exported by `fe2o3-kernel-ir`. It is a pure,
//! bounded schema for later compiler, bundle, runtime, and Verus integration.
//! Decoding and validation grant no load, launch, artifact, or proof authority.
//!
//! # Deliberate limitations
//!
//! - Only `gfx942:xnack-`, code object V6, Wave64, and little-endian 64-bit
//!   kernargs are admitted.
//! - Kernel identities, type identities, artifact identities, and proof
//!   obligations are opaque declarations until authenticated by later layers.
//! - Direct host dispatch is the only admitted launch mode. Cooperative-grid
//!   launch, device-side enqueue, and dynamic parallelism fail closed.
//! - Occupancy is admitted only through a tuple-bound verifier/metadata
//!   witness. Witness identities remain unauthenticated, and occupancy is not
//!   derived from executable register/resource usage by this model.
//! - No source extraction, LLVM lowering, bundle wiring, runtime enforcement,
//!   hardware evidence, or formal-verification claim is made here.

use std::collections::BTreeSet;

pub const LAUNCH_KERNEL_V2_MAGIC: [u8; 8] = *b"F2LKV2\0\0";
pub const LAUNCH_KERNEL_V2_VERSION: u16 = 5;
pub const LAUNCH_KERNEL_V2_LIMITATIONS: &str = "gfx942:xnack- COV6 Wave64 only; canonical tuple identities, proof records, and occupancy verifier/metadata identities remain unauthenticated caller-supplied claims; direct host dispatch only; no cooperative grid, device enqueue, or dynamic parallelism; occupancy is witnessed, not derived; no export, lowering, bundle, runtime, hardware, or formal-verification claim";
pub const VARIANT_TUPLE_DOMAIN_V2: &[u8] = b"fe2o3.launch-kernel.variant-tuple.v5\0";
pub const OCCUPANCY_SUBJECT_DOMAIN_V2: &[u8] = b"fe2o3.launch-kernel.occupancy-subject.v1\0";

pub const GFX942_MAX_FLAT_WORKGROUP_SIZE_V2: u32 = 1_024;
pub const GFX942_MAX_WAVES_PER_EXECUTION_UNIT_V2: u8 = 8;
pub const GFX942_MAX_LDS_BYTES_PER_WORKGROUP_V2: u32 = 65_536;
pub const GFX942_MAX_KERNARG_SEGMENT_BYTES_V2: u32 = 1 << 20;
pub const GFX942_MAX_PRIVATE_SEGMENT_BYTES_V2: u32 = 1 << 20;

const HEADER_BYTES: usize = 24;
const TARGET_BYTES: usize = 40;
const PARAMETER_BYTES: usize = 48;
const OCCUPANCY_WITNESS_BYTES: usize = 104;
const VARIANT_FIXED_BYTES: usize = 212 + OCCUPANCY_WITNESS_BYTES;
const PROOF_OBLIGATION_BYTES: usize = 33;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchKernelLimitsV2 {
    pub max_encoded_bytes: usize,
    pub max_name_bytes: usize,
    pub max_variants: usize,
    pub max_parameters: usize,
    pub max_capabilities_per_variant: usize,
    pub max_proof_obligations_per_variant: usize,
}

impl Default for LaunchKernelLimitsV2 {
    fn default() -> Self {
        Self {
            max_encoded_bytes: 1 << 20,
            max_name_bytes: 96,
            max_variants: 32,
            max_parameters: 128,
            max_capabilities_per_variant: 32,
            max_proof_obligations_per_variant: 16,
        }
    }
}

macro_rules! opaque_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub [u8; 32]);

        impl $name {
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            const fn is_zero(self) -> bool {
                let mut index = 0;
                while index < self.0.len() {
                    if self.0[index] != 0 {
                        return false;
                    }
                    index += 1;
                }
                true
            }
        }
    };
}

opaque_identity!(TargetIdentityV2);
opaque_identity!(KernelFamilyIdentityV2);
opaque_identity!(KernelIdentityV2);
opaque_identity!(KernelSignatureIdentityV2);
opaque_identity!(KernelPolicyIdentityV2);
opaque_identity!(ArtifactIdentityV2);
opaque_identity!(SemanticTypeIdentityV2);
opaque_identity!(KernelVariantTupleIdentityV2);
opaque_identity!(OccupancyVerifierIdentityV2);
opaque_identity!(OccupancyMetadataIdentityV2);
opaque_identity!(OccupancySubjectIdentityV2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AmdArchitectureV2 {
    Gfx942 = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum XnackModeV2 {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CodeObjectVersionV2 {
    V5 = 5,
    V6 = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EndiannessV2 {
    Little = 1,
    Big = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942TargetBindingV2 {
    pub identity: TargetIdentityV2,
    pub architecture: AmdArchitectureV2,
    pub xnack: XnackModeV2,
    pub code_object: CodeObjectVersionV2,
    pub pointer_width_bytes: u8,
    pub endianness: EndiannessV2,
}

impl Gfx942TargetBindingV2 {
    pub const fn gfx942_xnack_minus(identity: TargetIdentityV2) -> Self {
        Self {
            identity,
            architecture: AmdArchitectureV2::Gfx942,
            xnack: XnackModeV2::Disabled,
            code_object: CodeObjectVersionV2::V6,
            pointer_width_bytes: 8,
            endianness: EndiannessV2::Little,
        }
    }

    fn validate(self) -> Result<(), LaunchKernelValidationErrorV2> {
        if self.identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity("target"));
        }
        if self.architecture != AmdArchitectureV2::Gfx942
            || self.xnack != XnackModeV2::Disabled
            || self.code_object != CodeObjectVersionV2::V6
            || self.pointer_width_bytes != 8
            || self.endianness != EndiannessV2::Little
        {
            return Err(LaunchKernelValidationErrorV2::UnsupportedTarget);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AbiParameterKindV2 {
    ByValue = 0,
    SharedGlobalPointer = 1,
    UniqueGlobalPointer = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiParameterV2 {
    pub source_index: u16,
    pub kind: AbiParameterKindV2,
    pub semantic_type: SemanticTypeIdentityV2,
    pub offset: u32,
    pub size: u32,
    pub alignment: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelSignatureV2 {
    pub identity: KernelSignatureIdentityV2,
    pub explicit_argument_bytes: u32,
    pub kernarg_segment_bytes: u32,
    pub kernarg_segment_alignment: u32,
    pub parameters: Vec<AbiParameterV2>,
}

impl KernelSignatureV2 {
    fn validate(&self, limits: &LaunchKernelLimitsV2) -> Result<(), LaunchKernelValidationErrorV2> {
        if self.identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity("signature"));
        }
        check_count("parameters", self.parameters.len(), limits.max_parameters)?;
        if self.explicit_argument_bytes > self.kernarg_segment_bytes
            || self.kernarg_segment_bytes > GFX942_MAX_KERNARG_SEGMENT_BYTES_V2
        {
            return Err(LaunchKernelValidationErrorV2::InvalidAbiBounds);
        }
        if !valid_alignment(
            self.kernarg_segment_alignment,
            GFX942_MAX_KERNARG_SEGMENT_BYTES_V2,
        ) {
            return Err(LaunchKernelValidationErrorV2::InvalidAbiAlignment);
        }

        let mut previous_end = 0_u32;
        for (position, parameter) in self.parameters.iter().enumerate() {
            let expected = u16::try_from(position)
                .map_err(|_| LaunchKernelValidationErrorV2::ArithmeticOverflow)?;
            if parameter.source_index != expected {
                return Err(LaunchKernelValidationErrorV2::NonCanonicalParameterOrder);
            }
            if parameter.semantic_type.is_zero() {
                return Err(LaunchKernelValidationErrorV2::ZeroIdentity(
                    "parameter type",
                ));
            }
            if !valid_alignment(parameter.alignment, self.kernarg_segment_alignment) {
                return Err(LaunchKernelValidationErrorV2::InvalidAbiAlignment);
            }
            if parameter.offset % parameter.alignment != 0 {
                return Err(LaunchKernelValidationErrorV2::MisalignedParameter {
                    index: parameter.source_index,
                });
            }
            let end = parameter
                .offset
                .checked_add(parameter.size)
                .ok_or(LaunchKernelValidationErrorV2::ArithmeticOverflow)?;
            if end > self.explicit_argument_bytes {
                return Err(LaunchKernelValidationErrorV2::ParameterOutOfBounds {
                    index: parameter.source_index,
                });
            }
            if parameter.size != 0 && parameter.offset < previous_end {
                return Err(LaunchKernelValidationErrorV2::OverlappingParameters {
                    index: parameter.source_index,
                });
            }
            if matches!(
                parameter.kind,
                AbiParameterKindV2::SharedGlobalPointer | AbiParameterKindV2::UniqueGlobalPointer
            ) && (parameter.size != 8 || parameter.alignment < 8)
            {
                return Err(LaunchKernelValidationErrorV2::InvalidPointerParameter {
                    index: parameter.source_index,
                });
            }
            previous_end = previous_end.max(end);
        }
        Ok(())
    }
}

fn valid_alignment(alignment: u32, maximum: u32) -> bool {
    alignment != 0 && alignment.is_power_of_two() && alignment <= maximum
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DimensionsV2 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl DimensionsV2 {
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    fn validate(self, rank: u8) -> Result<(), LaunchKernelValidationErrorV2> {
        if self.x == 0 || self.y == 0 || self.z == 0 {
            return Err(LaunchKernelValidationErrorV2::ZeroDimension);
        }
        if (rank < 2 && self.y != 1) || (rank < 3 && self.z != 1) {
            return Err(LaunchKernelValidationErrorV2::UnusedDimensionNotOne);
        }
        self.checked_product()?;
        Ok(())
    }

    pub fn checked_product(self) -> Result<u64, LaunchKernelValidationErrorV2> {
        u64::from(self.x)
            .checked_mul(u64::from(self.y))
            .and_then(|xy| xy.checked_mul(u64::from(self.z)))
            .ok_or(LaunchKernelValidationErrorV2::ArithmeticOverflow)
    }

    fn componentwise_le(self, other: Self) -> bool {
        self.x <= other.x && self.y <= other.y && self.z <= other.z
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockShapePolicyV2 {
    Exact(DimensionsV2),
    Bounded {
        minimum: DimensionsV2,
        maximum: DimensionsV2,
    },
}

impl BlockShapePolicyV2 {
    fn bounds(self) -> (DimensionsV2, DimensionsV2) {
        match self {
            Self::Exact(shape) => (shape, shape),
            Self::Bounded { minimum, maximum } => (minimum, maximum),
        }
    }

    fn admits(self, shape: DimensionsV2) -> bool {
        let (minimum, maximum) = self.bounds();
        minimum.componentwise_le(shape) && shape.componentwise_le(maximum)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum WavefrontWidthV2 {
    Wave32 = 1,
    Wave64 = 2,
}

pub const GFX942_REQUIRED_WAVEFRONT_WIDTH_V2: WavefrontWidthV2 = WavefrontWidthV2::Wave64;

impl WavefrontWidthV2 {
    pub const fn lanes(self) -> u32 {
        match self {
            Self::Wave32 => 32,
            Self::Wave64 => 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedLaunchFeaturesV2 {
    pub cooperative_grid: bool,
    pub device_side_enqueue: bool,
    pub dynamic_parallelism: bool,
}

impl UnsupportedLaunchFeaturesV2 {
    pub const NONE: Self = Self {
        cooperative_grid: false,
        device_side_enqueue: false,
        dynamic_parallelism: false,
    };

    const fn any(self) -> bool {
        self.cooperative_grid || self.device_side_enqueue || self.dynamic_parallelism
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942LaunchContractV2 {
    pub rank: u8,
    pub block: BlockShapePolicyV2,
    pub max_grid_blocks: DimensionsV2,
    pub minimum_flat_workgroup_size: u32,
    pub maximum_flat_workgroup_size: u32,
    pub wavefront: WavefrontWidthV2,
    pub require_full_waves: bool,
    pub minimum_waves_per_execution_unit: u8,
    pub maximum_waves_per_execution_unit: u8,
    pub max_total_workitems: u64,
    pub unsupported: UnsupportedLaunchFeaturesV2,
}

impl Gfx942LaunchContractV2 {
    fn validate(self) -> Result<(), LaunchKernelValidationErrorV2> {
        if !(1..=3).contains(&self.rank) {
            return Err(LaunchKernelValidationErrorV2::InvalidRank);
        }
        if self.unsupported.any() {
            return Err(LaunchKernelValidationErrorV2::UnsupportedLaunchFeature);
        }
        if self.wavefront != GFX942_REQUIRED_WAVEFRONT_WIDTH_V2 {
            return Err(LaunchKernelValidationErrorV2::UnsupportedWavefrontWidth);
        }
        let (minimum, maximum) = self.block.bounds();
        minimum.validate(self.rank)?;
        maximum.validate(self.rank)?;
        if !minimum.componentwise_le(maximum) {
            return Err(LaunchKernelValidationErrorV2::InvalidBlockRange);
        }
        self.max_grid_blocks.validate(self.rank)?;

        let maximum_threads = maximum.checked_product()?;
        if self.minimum_flat_workgroup_size == 0
            || self.minimum_flat_workgroup_size > self.maximum_flat_workgroup_size
            || self.maximum_flat_workgroup_size > GFX942_MAX_FLAT_WORKGROUP_SIZE_V2
            || maximum_threads > u64::from(GFX942_MAX_FLAT_WORKGROUP_SIZE_V2)
        {
            return Err(LaunchKernelValidationErrorV2::InvalidFlatWorkgroupBounds);
        }
        if self.minimum_waves_per_execution_unit == 0
            || self.minimum_waves_per_execution_unit > self.maximum_waves_per_execution_unit
            || self.maximum_waves_per_execution_unit > GFX942_MAX_WAVES_PER_EXECUTION_UNIT_V2
        {
            return Err(LaunchKernelValidationErrorV2::InvalidOccupancyBounds);
        }
        let admitted = self.admitted_block_summary()?;
        if admitted.count == 0 || !admitted.has_full_wave {
            return Err(LaunchKernelValidationErrorV2::NoAdmittedBlockShape);
        }
        let canonical = match self.block {
            BlockShapePolicyV2::Exact(shape) => {
                admitted.count == 1
                    && admitted.minimum == shape
                    && admitted.maximum == shape
                    && self.minimum_flat_workgroup_size == admitted.minimum_flat
                    && self.maximum_flat_workgroup_size == admitted.maximum_flat
                    && !self.require_full_waves
            }
            BlockShapePolicyV2::Bounded { minimum, maximum } => {
                admitted.count > 1
                    && admitted.minimum == minimum
                    && admitted.maximum == maximum
                    && self.minimum_flat_workgroup_size == admitted.minimum_flat
                    && self.maximum_flat_workgroup_size == admitted.maximum_flat
                    && (!self.require_full_waves || admitted.excluded_non_full_wave)
            }
        };
        if !canonical {
            return Err(LaunchKernelValidationErrorV2::NonCanonicalBlockPolicy);
        }

        let maximum_geometry = checked_geometry(self.max_grid_blocks, maximum)?;
        if maximum_geometry.total_workitems > self.max_total_workitems
            || self.max_total_workitems == 0
        {
            return Err(LaunchKernelValidationErrorV2::TotalWorkitemLimitExceeded);
        }
        Ok(())
    }

    fn admits_block_shape(
        self,
        shape: DimensionsV2,
    ) -> Result<bool, LaunchKernelValidationErrorV2> {
        if !self.block.admits(shape) {
            return Ok(false);
        }
        let flat = shape.checked_product()?;
        Ok(flat >= u64::from(self.minimum_flat_workgroup_size)
            && flat <= u64::from(self.maximum_flat_workgroup_size)
            && (!self.require_full_waves
                || flat % u64::from(GFX942_REQUIRED_WAVEFRONT_WIDTH_V2.lanes()) == 0))
    }

    fn admitted_block_summary(
        self,
    ) -> Result<AdmittedBlockSummaryV2, LaunchKernelValidationErrorV2> {
        let (minimum, maximum) = self.block.bounds();
        let mut summary = AdmittedBlockSummaryV2::empty();
        for z in minimum.z..=maximum.z {
            for y in minimum.y..=maximum.y {
                for x in minimum.x..=maximum.x {
                    let shape = DimensionsV2::new(x, y, z);
                    let flat = shape.checked_product()?;
                    if flat < u64::from(self.minimum_flat_workgroup_size)
                        || flat > u64::from(self.maximum_flat_workgroup_size)
                    {
                        continue;
                    }
                    let full_wave =
                        flat % u64::from(GFX942_REQUIRED_WAVEFRONT_WIDTH_V2.lanes()) == 0;
                    if self.require_full_waves && !full_wave {
                        summary.excluded_non_full_wave = true;
                        continue;
                    }
                    summary.include(
                        shape,
                        u32::try_from(flat)
                            .map_err(|_| LaunchKernelValidationErrorV2::ArithmeticOverflow)?,
                        full_wave,
                    );
                }
            }
        }
        Ok(summary)
    }
}

#[derive(Clone, Copy)]
struct AdmittedBlockSummaryV2 {
    count: u32,
    minimum: DimensionsV2,
    maximum: DimensionsV2,
    minimum_flat: u32,
    maximum_flat: u32,
    has_full_wave: bool,
    excluded_non_full_wave: bool,
}

impl AdmittedBlockSummaryV2 {
    const fn empty() -> Self {
        Self {
            count: 0,
            minimum: DimensionsV2::new(u32::MAX, u32::MAX, u32::MAX),
            maximum: DimensionsV2::new(0, 0, 0),
            minimum_flat: u32::MAX,
            maximum_flat: 0,
            has_full_wave: false,
            excluded_non_full_wave: false,
        }
    }

    fn include(&mut self, shape: DimensionsV2, flat: u32, full_wave: bool) {
        self.count += 1;
        self.minimum.x = self.minimum.x.min(shape.x);
        self.minimum.y = self.minimum.y.min(shape.y);
        self.minimum.z = self.minimum.z.min(shape.z);
        self.maximum.x = self.maximum.x.max(shape.x);
        self.maximum.y = self.maximum.y.max(shape.y);
        self.maximum.z = self.maximum.z.max(shape.z);
        self.minimum_flat = self.minimum_flat.min(flat);
        self.maximum_flat = self.maximum_flat.max(flat);
        self.has_full_wave |= full_wave;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ResourceLimitsV2 {
    pub static_lds_bytes: u32,
    pub maximum_dynamic_lds_bytes: u32,
    pub dynamic_lds_alignment: u32,
    pub private_segment_bytes: u32,
}

impl Gfx942ResourceLimitsV2 {
    fn validate(self) -> Result<(), LaunchKernelValidationErrorV2> {
        let total = self
            .static_lds_bytes
            .checked_add(self.maximum_dynamic_lds_bytes)
            .ok_or(LaunchKernelValidationErrorV2::ArithmeticOverflow)?;
        if total > GFX942_MAX_LDS_BYTES_PER_WORKGROUP_V2 {
            return Err(LaunchKernelValidationErrorV2::LdsLimitExceeded);
        }
        if !valid_alignment(self.dynamic_lds_alignment, 256) {
            return Err(LaunchKernelValidationErrorV2::InvalidLdsAlignment);
        }
        if self.maximum_dynamic_lds_bytes == 0 && self.dynamic_lds_alignment != 1 {
            return Err(LaunchKernelValidationErrorV2::InvalidLdsAlignment);
        }
        if self.private_segment_bytes > GFX942_MAX_PRIVATE_SEGMENT_BYTES_V2 {
            return Err(LaunchKernelValidationErrorV2::PrivateSegmentLimitExceeded);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942OccupancyWitnessV2 {
    pub verifier_identity: OccupancyVerifierIdentityV2,
    pub metadata_identity: OccupancyMetadataIdentityV2,
    pub subject_identity: OccupancySubjectIdentityV2,
    pub minimum_waves_per_execution_unit: u8,
    pub maximum_waves_per_execution_unit: u8,
}

impl Gfx942OccupancyWitnessV2 {
    fn validate(
        self,
        launch: Gfx942LaunchContractV2,
        expected_subject: OccupancySubjectIdentityV2,
    ) -> Result<(), LaunchKernelValidationErrorV2> {
        if self.verifier_identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity(
                "occupancy verifier",
            ));
        }
        if self.metadata_identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity(
                "occupancy metadata",
            ));
        }
        if self.verifier_identity.0 == self.metadata_identity.0 {
            return Err(LaunchKernelValidationErrorV2::OccupancyAuthoritiesNotIndependent);
        }
        if self.subject_identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity(
                "occupancy subject",
            ));
        }
        if self.subject_identity != expected_subject {
            return Err(LaunchKernelValidationErrorV2::OccupancySubjectMismatch);
        }
        if self.minimum_waves_per_execution_unit != launch.minimum_waves_per_execution_unit
            || self.maximum_waves_per_execution_unit != launch.maximum_waves_per_execution_unit
        {
            return Err(LaunchKernelValidationErrorV2::OccupancyBoundsMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LaunchCapabilityV2 {
    ExactWaveMode = 1,
    StaticLds = 2,
    DynamicLds = 3,
    WorkgroupBarrier = 4,
    DeviceAtomics = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LaunchProofKindV2 {
    TargetAuthenticated = 1,
    ArtifactAuthenticated = 2,
    KernelIdentityAuthenticated = 3,
    SignatureLayoutAuthenticated = 4,
    PolicySelectionAuthenticated = 5,
    GeometryAndResourcesProved = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct LaunchProofObligationV2 {
    pub kind: LaunchProofKindV2,
    pub variant_tuple_identity: KernelVariantTupleIdentityV2,
}

impl LaunchProofObligationV2 {
    pub const fn new(
        kind: LaunchProofKindV2,
        variant_tuple_identity: KernelVariantTupleIdentityV2,
    ) -> Self {
        Self {
            kind,
            variant_tuple_identity,
        }
    }
}

const REQUIRED_PROOFS: [LaunchProofKindV2; 6] = [
    LaunchProofKindV2::TargetAuthenticated,
    LaunchProofKindV2::ArtifactAuthenticated,
    LaunchProofKindV2::KernelIdentityAuthenticated,
    LaunchProofKindV2::SignatureLayoutAuthenticated,
    LaunchProofKindV2::PolicySelectionAuthenticated,
    LaunchProofKindV2::GeometryAndResourcesProved,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelVariantV2 {
    pub kernel_identity: KernelIdentityV2,
    pub policy_identity: KernelPolicyIdentityV2,
    pub artifact_identity: ArtifactIdentityV2,
    pub tuple_identity: KernelVariantTupleIdentityV2,
    pub variant_name: String,
    pub entry_name: String,
    pub launch: Gfx942LaunchContractV2,
    pub resources: Gfx942ResourceLimitsV2,
    pub occupancy_witness: Option<Gfx942OccupancyWitnessV2>,
    pub capabilities: Vec<LaunchCapabilityV2>,
    pub proof_obligations: Vec<LaunchProofObligationV2>,
}

impl KernelVariantV2 {
    fn validate(
        &self,
        limits: &LaunchKernelLimitsV2,
        expected_tuple_identity: KernelVariantTupleIdentityV2,
        expected_occupancy_subject: OccupancySubjectIdentityV2,
    ) -> Result<(), LaunchKernelValidationErrorV2> {
        if self.kernel_identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity("kernel"));
        }
        if self.policy_identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity("policy"));
        }
        if self.artifact_identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity("artifact"));
        }
        if self.tuple_identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity("variant tuple"));
        }
        validate_name(&self.variant_name, limits)?;
        validate_name(&self.entry_name, limits)?;
        self.launch.validate()?;
        self.resources.validate()?;
        validate_sorted_unique(
            "capabilities",
            &self.capabilities,
            limits.max_capabilities_per_variant,
        )?;
        validate_sorted_unique(
            "proof obligations",
            &self.proof_obligations,
            limits.max_proof_obligations_per_variant,
        )?;
        if !self
            .capabilities
            .contains(&LaunchCapabilityV2::ExactWaveMode)
        {
            return Err(LaunchKernelValidationErrorV2::MissingCapability(
                LaunchCapabilityV2::ExactWaveMode,
            ));
        }
        if self.resources.static_lds_bytes != 0
            && !self.capabilities.contains(&LaunchCapabilityV2::StaticLds)
        {
            return Err(LaunchKernelValidationErrorV2::MissingCapability(
                LaunchCapabilityV2::StaticLds,
            ));
        }
        if self.resources.static_lds_bytes == 0
            && self.capabilities.contains(&LaunchCapabilityV2::StaticLds)
        {
            return Err(LaunchKernelValidationErrorV2::RedundantCapability(
                LaunchCapabilityV2::StaticLds,
            ));
        }
        if self.resources.maximum_dynamic_lds_bytes != 0
            && !self.capabilities.contains(&LaunchCapabilityV2::DynamicLds)
        {
            return Err(LaunchKernelValidationErrorV2::MissingCapability(
                LaunchCapabilityV2::DynamicLds,
            ));
        }
        if self.resources.maximum_dynamic_lds_bytes == 0
            && self.capabilities.contains(&LaunchCapabilityV2::DynamicLds)
        {
            return Err(LaunchKernelValidationErrorV2::RedundantCapability(
                LaunchCapabilityV2::DynamicLds,
            ));
        }
        let occupancy_witness = self
            .occupancy_witness
            .ok_or(LaunchKernelValidationErrorV2::MissingOccupancyWitness)?;
        for required in REQUIRED_PROOFS {
            if !self
                .proof_obligations
                .iter()
                .any(|obligation| obligation.kind == required)
            {
                return Err(LaunchKernelValidationErrorV2::MissingProofObligation(
                    required,
                ));
            }
        }
        for obligation in &self.proof_obligations {
            if obligation.variant_tuple_identity != self.tuple_identity {
                return Err(LaunchKernelValidationErrorV2::ProofTupleIdentityMismatch(
                    obligation.kind,
                ));
            }
        }
        if self.tuple_identity != expected_tuple_identity {
            return Err(LaunchKernelValidationErrorV2::VariantTupleIdentityMismatch);
        }
        occupancy_witness.validate(self.launch, expected_occupancy_subject)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchKernelFamilyV2 {
    pub target: Gfx942TargetBindingV2,
    pub family_identity: KernelFamilyIdentityV2,
    pub logical_name: String,
    pub signature: KernelSignatureV2,
    pub variants: Vec<KernelVariantV2>,
}

impl LaunchKernelFamilyV2 {
    pub fn validate(
        &self,
        limits: &LaunchKernelLimitsV2,
    ) -> Result<(), LaunchKernelValidationErrorV2> {
        self.target.validate()?;
        if self.family_identity.is_zero() {
            return Err(LaunchKernelValidationErrorV2::ZeroIdentity("family"));
        }
        validate_name(&self.logical_name, limits)?;
        self.signature.validate(limits)?;
        if self.variants.is_empty() {
            return Err(LaunchKernelValidationErrorV2::EmptyKernelFamily);
        }
        check_count("variants", self.variants.len(), limits.max_variants)?;

        let mut variant_names = BTreeSet::new();
        let mut entry_names = BTreeSet::new();
        let mut kernels = BTreeSet::new();
        let mut policies = BTreeSet::new();
        let mut previous_name: Option<&str> = None;
        for variant in &self.variants {
            let expected_occupancy_subject = canonical_occupancy_subject_identity_v2(
                &self.target,
                &self.signature,
                variant.artifact_identity,
                &variant.entry_name,
                variant.resources,
            );
            let expected_tuple_identity = canonical_variant_tuple_identity_v2(
                &self.target,
                self.family_identity,
                &self.logical_name,
                &self.signature,
                variant,
            );
            variant.validate(limits, expected_tuple_identity, expected_occupancy_subject)?;
            if !variant_names.insert(&variant.variant_name) {
                return Err(LaunchKernelValidationErrorV2::DuplicateVariantName);
            }
            if previous_name.is_some_and(|previous| previous > variant.variant_name.as_str()) {
                return Err(LaunchKernelValidationErrorV2::NonCanonicalVariantOrder);
            }
            previous_name = Some(&variant.variant_name);
            if !entry_names.insert(&variant.entry_name) {
                return Err(LaunchKernelValidationErrorV2::DuplicateEntryName);
            }
            if !kernels.insert(variant.kernel_identity) {
                return Err(LaunchKernelValidationErrorV2::DuplicateKernelIdentity);
            }
            if !policies.insert(variant.policy_identity) {
                return Err(LaunchKernelValidationErrorV2::DuplicatePolicyIdentity);
            }
        }
        Ok(())
    }

    pub fn validate_launch(
        &self,
        variant_name: &str,
        request: LaunchRequestV2,
        limits: &LaunchKernelLimitsV2,
    ) -> Result<ValidatedLaunchFactsV2, LaunchKernelValidationErrorV2> {
        self.validate(limits)?;
        let variant = self
            .variants
            .iter()
            .find(|variant| variant.variant_name == variant_name)
            .ok_or(LaunchKernelValidationErrorV2::UnknownVariant)?;
        validate_request(variant, request)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchRequestV2 {
    pub grid_blocks: DimensionsV2,
    pub block_threads: DimensionsV2,
    pub dynamic_lds_bytes: u32,
    pub dynamic_lds_alignment: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedLaunchFactsV2 {
    pub flat_workgroup_size: u32,
    pub grid_block_count: u64,
    pub global_workitems: DimensionsV2,
    pub total_workitems: u64,
    pub waves_per_workgroup: u32,
    pub total_lds_bytes: u32,
}

fn validate_request(
    variant: &KernelVariantV2,
    request: LaunchRequestV2,
) -> Result<ValidatedLaunchFactsV2, LaunchKernelValidationErrorV2> {
    let launch = variant.launch;
    request.grid_blocks.validate(launch.rank)?;
    request.block_threads.validate(launch.rank)?;
    if !request.grid_blocks.componentwise_le(launch.max_grid_blocks) {
        return Err(LaunchKernelValidationErrorV2::GridLimitExceeded);
    }
    if !launch.admits_block_shape(request.block_threads)? {
        return Err(LaunchKernelValidationErrorV2::BlockShapeRejected);
    }
    let flat = request.block_threads.checked_product()?;
    let lanes = u64::from(launch.wavefront.lanes());
    if request.dynamic_lds_bytes > variant.resources.maximum_dynamic_lds_bytes {
        return Err(LaunchKernelValidationErrorV2::DynamicLdsLimitExceeded);
    }
    if request.dynamic_lds_bytes == 0 {
        if request.dynamic_lds_alignment != 1 {
            return Err(LaunchKernelValidationErrorV2::InvalidLdsAlignment);
        }
    } else if !valid_alignment(request.dynamic_lds_alignment, 256)
        || request.dynamic_lds_alignment < variant.resources.dynamic_lds_alignment
    {
        return Err(LaunchKernelValidationErrorV2::InvalidLdsAlignment);
    }
    let total_lds_bytes = variant
        .resources
        .static_lds_bytes
        .checked_add(request.dynamic_lds_bytes)
        .ok_or(LaunchKernelValidationErrorV2::ArithmeticOverflow)?;
    if total_lds_bytes > GFX942_MAX_LDS_BYTES_PER_WORKGROUP_V2 {
        return Err(LaunchKernelValidationErrorV2::LdsLimitExceeded);
    }

    let geometry = checked_geometry(request.grid_blocks, request.block_threads)?;
    if geometry.total_workitems > launch.max_total_workitems {
        return Err(LaunchKernelValidationErrorV2::TotalWorkitemLimitExceeded);
    }
    let flat_u32 =
        u32::try_from(flat).map_err(|_| LaunchKernelValidationErrorV2::ArithmeticOverflow)?;
    let waves = flat
        .checked_add(lanes - 1)
        .ok_or(LaunchKernelValidationErrorV2::ArithmeticOverflow)?
        / lanes;
    Ok(ValidatedLaunchFactsV2 {
        flat_workgroup_size: flat_u32,
        grid_block_count: request.grid_blocks.checked_product()?,
        global_workitems: geometry.global_workitems,
        total_workitems: geometry.total_workitems,
        waves_per_workgroup: u32::try_from(waves)
            .map_err(|_| LaunchKernelValidationErrorV2::ArithmeticOverflow)?,
        total_lds_bytes,
    })
}

#[derive(Clone, Copy)]
struct CheckedGeometryV2 {
    global_workitems: DimensionsV2,
    total_workitems: u64,
}

fn checked_geometry(
    grid_blocks: DimensionsV2,
    block_threads: DimensionsV2,
) -> Result<CheckedGeometryV2, LaunchKernelValidationErrorV2> {
    let x = u64::from(grid_blocks.x)
        .checked_mul(u64::from(block_threads.x))
        .ok_or(LaunchKernelValidationErrorV2::ArithmeticOverflow)?;
    let y = u64::from(grid_blocks.y)
        .checked_mul(u64::from(block_threads.y))
        .ok_or(LaunchKernelValidationErrorV2::ArithmeticOverflow)?;
    let z = u64::from(grid_blocks.z)
        .checked_mul(u64::from(block_threads.z))
        .ok_or(LaunchKernelValidationErrorV2::ArithmeticOverflow)?;
    let global_workitems = DimensionsV2 {
        x: u32::try_from(x).map_err(|_| LaunchKernelValidationErrorV2::ArithmeticOverflow)?,
        y: u32::try_from(y).map_err(|_| LaunchKernelValidationErrorV2::ArithmeticOverflow)?,
        z: u32::try_from(z).map_err(|_| LaunchKernelValidationErrorV2::ArithmeticOverflow)?,
    };
    let total_workitems = x
        .checked_mul(y)
        .and_then(|xy| xy.checked_mul(z))
        .ok_or(LaunchKernelValidationErrorV2::ArithmeticOverflow)?;
    Ok(CheckedGeometryV2 {
        global_workitems,
        total_workitems,
    })
}

/// Bind an occupancy claim to the exact executable metadata subject.
///
/// This digest establishes internal substitution resistance only. It does not
/// authenticate either the metadata producer or the independent verifier.
pub fn canonical_occupancy_subject_identity_v2(
    target: &Gfx942TargetBindingV2,
    signature: &KernelSignatureV2,
    artifact_identity: ArtifactIdentityV2,
    entry_name: &str,
    resources: Gfx942ResourceLimitsV2,
) -> OccupancySubjectIdentityV2 {
    let mut digest = Sha256V2::new();
    tuple_bytes(&mut digest, OCCUPANCY_SUBJECT_DOMAIN_V2);
    tuple_bytes(&mut digest, &target.identity.0);
    tuple_u8(&mut digest, target.architecture as u8);
    tuple_u8(&mut digest, target.xnack as u8);
    tuple_u8(&mut digest, target.code_object as u8);
    tuple_u8(&mut digest, target.pointer_width_bytes);
    tuple_u8(&mut digest, target.endianness as u8);
    tuple_bytes(&mut digest, &artifact_identity.0);
    tuple_name(&mut digest, entry_name);
    tuple_bytes(&mut digest, &signature.identity.0);
    tuple_u32(&mut digest, signature.explicit_argument_bytes);
    tuple_u32(&mut digest, signature.kernarg_segment_bytes);
    tuple_u32(&mut digest, signature.kernarg_segment_alignment);
    tuple_u64(&mut digest, signature.parameters.len() as u64);
    for parameter in &signature.parameters {
        tuple_u16(&mut digest, parameter.source_index);
        tuple_u8(&mut digest, parameter.kind as u8);
        tuple_bytes(&mut digest, &parameter.semantic_type.0);
        tuple_u32(&mut digest, parameter.offset);
        tuple_u32(&mut digest, parameter.size);
        tuple_u32(&mut digest, parameter.alignment);
    }
    tuple_u32(&mut digest, resources.static_lds_bytes);
    tuple_u32(&mut digest, resources.maximum_dynamic_lds_bytes);
    tuple_u32(&mut digest, resources.dynamic_lds_alignment);
    tuple_u32(&mut digest, resources.private_segment_bytes);
    OccupancySubjectIdentityV2(digest.finalize())
}

/// Derive the canonical identity of one complete launch variant tuple.
///
/// The domain-separated digest commits the exact target and COV, family,
/// signature layout, artifact entry symbol and metadata, kernel, policy,
/// launch contract, resources, and capabilities. It deliberately excludes the
/// identity field itself and proof records. The digest establishes internal
/// consistency only; neither it nor a matching proof record authenticates who
/// produced or verified any component.
pub fn canonical_variant_tuple_identity_v2(
    target: &Gfx942TargetBindingV2,
    family_identity: KernelFamilyIdentityV2,
    logical_name: &str,
    signature: &KernelSignatureV2,
    variant: &KernelVariantV2,
) -> KernelVariantTupleIdentityV2 {
    let mut digest = Sha256V2::new();
    tuple_bytes(&mut digest, VARIANT_TUPLE_DOMAIN_V2);
    tuple_bytes(&mut digest, &target.identity.0);
    tuple_u8(&mut digest, target.architecture as u8);
    tuple_u8(&mut digest, target.xnack as u8);
    tuple_u8(&mut digest, target.code_object as u8);
    tuple_u8(&mut digest, target.pointer_width_bytes);
    tuple_u8(&mut digest, target.endianness as u8);
    tuple_u8(&mut digest, GFX942_REQUIRED_WAVEFRONT_WIDTH_V2 as u8);

    tuple_bytes(&mut digest, &family_identity.0);
    tuple_name(&mut digest, logical_name);
    tuple_bytes(&mut digest, &signature.identity.0);
    tuple_u32(&mut digest, signature.explicit_argument_bytes);
    tuple_u32(&mut digest, signature.kernarg_segment_bytes);
    tuple_u32(&mut digest, signature.kernarg_segment_alignment);
    tuple_u64(&mut digest, signature.parameters.len() as u64);
    for parameter in &signature.parameters {
        tuple_u16(&mut digest, parameter.source_index);
        tuple_u8(&mut digest, parameter.kind as u8);
        tuple_bytes(&mut digest, &parameter.semantic_type.0);
        tuple_u32(&mut digest, parameter.offset);
        tuple_u32(&mut digest, parameter.size);
        tuple_u32(&mut digest, parameter.alignment);
    }

    tuple_bytes(&mut digest, &variant.artifact_identity.0);
    tuple_name(&mut digest, &variant.entry_name);
    tuple_bytes(&mut digest, &variant.kernel_identity.0);
    tuple_bytes(&mut digest, &signature.identity.0);
    tuple_bytes(&mut digest, &variant.policy_identity.0);
    tuple_name(&mut digest, &variant.variant_name);
    tuple_u8(&mut digest, variant.launch.rank);
    tuple_u8(
        &mut digest,
        match variant.launch.block {
            BlockShapePolicyV2::Exact(_) => 0,
            BlockShapePolicyV2::Bounded { .. } => 1,
        },
    );
    let (minimum, maximum) = variant.launch.block.bounds();
    tuple_dimensions(&mut digest, minimum);
    tuple_dimensions(&mut digest, maximum);
    tuple_dimensions(&mut digest, variant.launch.max_grid_blocks);
    tuple_u32(&mut digest, variant.launch.minimum_flat_workgroup_size);
    tuple_u32(&mut digest, variant.launch.maximum_flat_workgroup_size);
    tuple_u8(&mut digest, variant.launch.wavefront as u8);
    tuple_u8(&mut digest, u8::from(variant.launch.require_full_waves));
    tuple_u8(&mut digest, variant.launch.minimum_waves_per_execution_unit);
    tuple_u8(&mut digest, variant.launch.maximum_waves_per_execution_unit);
    tuple_u64(&mut digest, variant.launch.max_total_workitems);
    tuple_u8(&mut digest, unsupported_bits(variant.launch.unsupported));
    tuple_u32(&mut digest, variant.resources.static_lds_bytes);
    tuple_u32(&mut digest, variant.resources.maximum_dynamic_lds_bytes);
    tuple_u32(&mut digest, variant.resources.dynamic_lds_alignment);
    tuple_u32(&mut digest, variant.resources.private_segment_bytes);
    match variant.occupancy_witness {
        Some(witness) => {
            tuple_u8(&mut digest, 1);
            tuple_bytes(&mut digest, &witness.verifier_identity.0);
            tuple_bytes(&mut digest, &witness.metadata_identity.0);
            tuple_bytes(&mut digest, &witness.subject_identity.0);
            tuple_u8(&mut digest, witness.minimum_waves_per_execution_unit);
            tuple_u8(&mut digest, witness.maximum_waves_per_execution_unit);
        }
        None => tuple_u8(&mut digest, 0),
    }
    tuple_u64(&mut digest, variant.capabilities.len() as u64);
    for capability in &variant.capabilities {
        tuple_u8(&mut digest, *capability as u8);
    }
    KernelVariantTupleIdentityV2(digest.finalize())
}

fn tuple_bytes(digest: &mut Sha256V2, bytes: &[u8]) {
    digest.update(bytes);
}

fn tuple_name(digest: &mut Sha256V2, value: &str) {
    tuple_u64(digest, value.len() as u64);
    tuple_bytes(digest, value.as_bytes());
}

fn tuple_dimensions(digest: &mut Sha256V2, value: DimensionsV2) {
    tuple_u32(digest, value.x);
    tuple_u32(digest, value.y);
    tuple_u32(digest, value.z);
}

fn tuple_u8(digest: &mut Sha256V2, value: u8) {
    digest.update(&[value]);
}

fn tuple_u16(digest: &mut Sha256V2, value: u16) {
    digest.update(&value.to_le_bytes());
}

fn tuple_u32(digest: &mut Sha256V2, value: u32) {
    digest.update(&value.to_le_bytes());
}

fn tuple_u64(digest: &mut Sha256V2, value: u64) {
    digest.update(&value.to_le_bytes());
}

struct Sha256V2 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    message_len: u64,
}

impl Sha256V2 {
    const ROUND_CONSTANTS: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    const fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffer_len: 0,
            message_len: 0,
        }
    }

    fn update(&mut self, mut bytes: &[u8]) {
        self.message_len = self
            .message_len
            .checked_add(bytes.len() as u64)
            .expect("bounded variant tuple length");
        if self.buffer_len != 0 {
            let count = (64 - self.buffer_len).min(bytes.len());
            self.buffer[self.buffer_len..self.buffer_len + count].copy_from_slice(&bytes[..count]);
            self.buffer_len += count;
            bytes = &bytes[count..];
            if self.buffer_len != 64 {
                return;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffer_len = 0;
        }
        while bytes.len() >= 64 {
            let block: &[u8; 64] = bytes[..64].try_into().expect("exact block");
            self.compress(block);
            bytes = &bytes[64..];
        }
        self.buffer[..bytes.len()].copy_from_slice(bytes);
        self.buffer_len = bytes.len();
    }

    fn finalize(mut self) -> [u8; 32] {
        let bit_len = self
            .message_len
            .checked_mul(8)
            .expect("bounded variant tuple bit length");
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;
        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..].fill(0);
            let block = self.buffer;
            self.compress(&block);
            self.buffer = [0; 64];
        } else {
            self.buffer[self.buffer_len..56].fill(0);
        }
        self.buffer[56..].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut output = [0_u8; 32];
        for (chunk, word) in output.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        output
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut words = [0_u32; 64];
        for (index, chunk) in block.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes(chunk.try_into().expect("four-byte word"));
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for (word, constant) in words.into_iter().zip(Self::ROUND_CONSTANTS) {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(sum1)
                .wrapping_add(choose)
                .wrapping_add(constant)
                .wrapping_add(word);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

#[cfg(test)]
pub fn sha256_test_vector_v2(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256V2::new();
    digest.update(bytes);
    digest.finalize()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchKernelValidationErrorV2 {
    ResourceLimit {
        resource: &'static str,
        observed: usize,
        limit: usize,
    },
    ArithmeticOverflow,
    ZeroIdentity(&'static str),
    UnsupportedTarget,
    InvalidName,
    InvalidAbiBounds,
    InvalidAbiAlignment,
    NonCanonicalParameterOrder,
    MisalignedParameter {
        index: u16,
    },
    ParameterOutOfBounds {
        index: u16,
    },
    OverlappingParameters {
        index: u16,
    },
    InvalidPointerParameter {
        index: u16,
    },
    InvalidRank,
    ZeroDimension,
    UnusedDimensionNotOne,
    InvalidBlockRange,
    NonCanonicalBlockPolicy,
    NoAdmittedBlockShape,
    InvalidFlatWorkgroupBounds,
    InvalidOccupancyBounds,
    UnsupportedWavefrontWidth,
    GridLimitExceeded,
    BlockShapeRejected,
    TotalWorkitemLimitExceeded,
    LdsLimitExceeded,
    DynamicLdsLimitExceeded,
    InvalidLdsAlignment,
    PrivateSegmentLimitExceeded,
    UnsupportedLaunchFeature,
    NonCanonicalSet(&'static str),
    MissingCapability(LaunchCapabilityV2),
    RedundantCapability(LaunchCapabilityV2),
    MissingOccupancyWitness,
    OccupancySubjectMismatch,
    OccupancyBoundsMismatch,
    OccupancyAuthoritiesNotIndependent,
    MissingProofObligation(LaunchProofKindV2),
    VariantTupleIdentityMismatch,
    ProofTupleIdentityMismatch(LaunchProofKindV2),
    EmptyKernelFamily,
    NonCanonicalVariantOrder,
    DuplicateVariantName,
    DuplicateEntryName,
    DuplicateKernelIdentity,
    DuplicatePolicyIdentity,
    UnknownVariant,
}

fn check_count(
    resource: &'static str,
    observed: usize,
    limit: usize,
) -> Result<(), LaunchKernelValidationErrorV2> {
    if observed > limit {
        return Err(LaunchKernelValidationErrorV2::ResourceLimit {
            resource,
            observed,
            limit,
        });
    }
    Ok(())
}

fn validate_name(
    name: &str,
    limits: &LaunchKernelLimitsV2,
) -> Result<(), LaunchKernelValidationErrorV2> {
    check_count("name bytes", name.len(), limits.max_name_bytes)?;
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || !matches!(bytes[0], b'A'..=b'Z' | b'a'..=b'z' | b'_')
        || !bytes
            .iter()
            .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'$' | b'-'))
    {
        return Err(LaunchKernelValidationErrorV2::InvalidName);
    }
    Ok(())
}

fn validate_sorted_unique<T: Ord>(
    resource: &'static str,
    values: &[T],
    limit: usize,
) -> Result<(), LaunchKernelValidationErrorV2> {
    check_count(resource, values.len(), limit)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LaunchKernelValidationErrorV2::NonCanonicalSet(resource));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchKernelEncodeErrorV2 {
    Model(LaunchKernelValidationErrorV2),
    EncodedLengthOverflow,
}

impl From<LaunchKernelValidationErrorV2> for LaunchKernelEncodeErrorV2 {
    fn from(value: LaunchKernelValidationErrorV2) -> Self {
        Self::Model(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchKernelDecodeErrorV2 {
    ResourceLimit {
        resource: &'static str,
        observed: usize,
        limit: usize,
    },
    Truncated,
    BadMagic,
    UnsupportedVersion(u16),
    NonZeroReserved,
    LengthMismatch,
    UnknownTag,
    InvalidUtf8,
    InvalidEncoding,
    Model(LaunchKernelValidationErrorV2),
    NonCanonicalEncoding,
}

impl From<LaunchKernelValidationErrorV2> for LaunchKernelDecodeErrorV2 {
    fn from(value: LaunchKernelValidationErrorV2) -> Self {
        match value {
            LaunchKernelValidationErrorV2::ResourceLimit {
                resource,
                observed,
                limit,
            } => Self::ResourceLimit {
                resource,
                observed,
                limit,
            },
            other => Self::Model(other),
        }
    }
}

/// Encode one validated family using the frozen bounded V2 representation.
pub fn encode_launch_kernel_family_v2(
    family: &LaunchKernelFamilyV2,
    limits: &LaunchKernelLimitsV2,
) -> Result<Vec<u8>, LaunchKernelEncodeErrorV2> {
    family.validate(limits)?;
    let encoded_len = encoded_len(family)?;
    if encoded_len > limits.max_encoded_bytes {
        return Err(LaunchKernelValidationErrorV2::ResourceLimit {
            resource: "encoded bytes",
            observed: encoded_len,
            limit: limits.max_encoded_bytes,
        }
        .into());
    }
    let total_len =
        u32::try_from(encoded_len).map_err(|_| LaunchKernelEncodeErrorV2::EncodedLengthOverflow)?;
    let variant_count = u16::try_from(family.variants.len())
        .map_err(|_| LaunchKernelEncodeErrorV2::EncodedLengthOverflow)?;
    let parameter_count = u16::try_from(family.signature.parameters.len())
        .map_err(|_| LaunchKernelEncodeErrorV2::EncodedLengthOverflow)?;

    let mut bytes = Vec::with_capacity(encoded_len);
    bytes.extend_from_slice(&LAUNCH_KERNEL_V2_MAGIC);
    put_u16(&mut bytes, LAUNCH_KERNEL_V2_VERSION);
    put_u16(&mut bytes, 0);
    put_u32(&mut bytes, total_len);
    put_u16(&mut bytes, variant_count);
    put_u16(&mut bytes, parameter_count);
    put_u32(&mut bytes, 0);

    bytes.extend_from_slice(&family.target.identity.0);
    bytes.push(family.target.architecture as u8);
    bytes.push(family.target.xnack as u8);
    bytes.push(family.target.code_object as u8);
    bytes.push(family.target.pointer_width_bytes);
    bytes.push(family.target.endianness as u8);
    bytes.extend_from_slice(&[0; 3]);

    bytes.extend_from_slice(&family.family_identity.0);
    bytes.extend_from_slice(&family.signature.identity.0);
    put_name(&mut bytes, &family.logical_name)?;
    put_u32(&mut bytes, family.signature.explicit_argument_bytes);
    put_u32(&mut bytes, family.signature.kernarg_segment_bytes);
    put_u32(&mut bytes, family.signature.kernarg_segment_alignment);
    for parameter in &family.signature.parameters {
        put_u16(&mut bytes, parameter.source_index);
        bytes.push(parameter.kind as u8);
        bytes.push(0);
        put_u32(&mut bytes, parameter.offset);
        put_u32(&mut bytes, parameter.size);
        put_u32(&mut bytes, parameter.alignment);
        bytes.extend_from_slice(&parameter.semantic_type.0);
    }

    for variant in &family.variants {
        bytes.extend_from_slice(&variant.kernel_identity.0);
        bytes.extend_from_slice(&variant.policy_identity.0);
        bytes.extend_from_slice(&variant.artifact_identity.0);
        bytes.extend_from_slice(&variant.tuple_identity.0);
        put_name(&mut bytes, &variant.variant_name)?;
        put_name(&mut bytes, &variant.entry_name)?;

        bytes.push(variant.launch.rank);
        bytes.push(match variant.launch.block {
            BlockShapePolicyV2::Exact(_) => 0,
            BlockShapePolicyV2::Bounded { .. } => 1,
        });
        bytes.push(variant.launch.wavefront as u8);
        bytes.push(u8::from(variant.launch.require_full_waves));
        bytes.push(variant.launch.minimum_waves_per_execution_unit);
        bytes.push(variant.launch.maximum_waves_per_execution_unit);
        bytes.push(unsupported_bits(variant.launch.unsupported));
        bytes.push(0);
        let (minimum, maximum) = variant.launch.block.bounds();
        put_dimensions(&mut bytes, minimum);
        put_dimensions(&mut bytes, maximum);
        put_dimensions(&mut bytes, variant.launch.max_grid_blocks);
        put_u32(&mut bytes, variant.launch.minimum_flat_workgroup_size);
        put_u32(&mut bytes, variant.launch.maximum_flat_workgroup_size);
        put_u64(&mut bytes, variant.launch.max_total_workitems);
        put_u32(&mut bytes, variant.resources.static_lds_bytes);
        put_u32(&mut bytes, variant.resources.maximum_dynamic_lds_bytes);
        put_u32(&mut bytes, variant.resources.dynamic_lds_alignment);
        put_u32(&mut bytes, variant.resources.private_segment_bytes);
        match variant.occupancy_witness {
            Some(witness) => {
                bytes.extend_from_slice(&[1, 0, 0, 0]);
                bytes.extend_from_slice(&witness.verifier_identity.0);
                bytes.extend_from_slice(&witness.metadata_identity.0);
                bytes.extend_from_slice(&witness.subject_identity.0);
                bytes.push(witness.minimum_waves_per_execution_unit);
                bytes.push(witness.maximum_waves_per_execution_unit);
                bytes.extend_from_slice(&[0; 2]);
            }
            None => bytes.extend_from_slice(&[0; OCCUPANCY_WITNESS_BYTES]),
        }
        put_u16(
            &mut bytes,
            u16::try_from(variant.capabilities.len())
                .map_err(|_| LaunchKernelEncodeErrorV2::EncodedLengthOverflow)?,
        );
        put_u16(
            &mut bytes,
            u16::try_from(variant.proof_obligations.len())
                .map_err(|_| LaunchKernelEncodeErrorV2::EncodedLengthOverflow)?,
        );
        put_u32(&mut bytes, 0);
        bytes.extend(
            variant
                .capabilities
                .iter()
                .map(|capability| *capability as u8),
        );
        bytes.extend(variant.proof_obligations.iter().flat_map(|obligation| {
            std::iter::once(obligation.kind as u8).chain(obligation.variant_tuple_identity.0)
        }));
    }
    debug_assert_eq!(bytes.len(), encoded_len);
    Ok(bytes)
}

/// Decode hostile bytes, establish canonical wire form, and validate the model.
///
/// The result remains descriptive data and carries no execution authority.
pub fn decode_launch_kernel_family_v2(
    bytes: &[u8],
    limits: &LaunchKernelLimitsV2,
) -> Result<LaunchKernelFamilyV2, LaunchKernelDecodeErrorV2> {
    if bytes.len() > limits.max_encoded_bytes {
        return Err(LaunchKernelDecodeErrorV2::ResourceLimit {
            resource: "encoded bytes",
            observed: bytes.len(),
            limit: limits.max_encoded_bytes,
        });
    }
    let mut reader = ReaderV2::new(bytes);
    if reader.take(8)? != LAUNCH_KERNEL_V2_MAGIC {
        return Err(LaunchKernelDecodeErrorV2::BadMagic);
    }
    let version = reader.u16()?;
    if version != LAUNCH_KERNEL_V2_VERSION {
        return Err(LaunchKernelDecodeErrorV2::UnsupportedVersion(version));
    }
    if reader.u16()? != 0 {
        return Err(LaunchKernelDecodeErrorV2::NonZeroReserved);
    }
    let declared_len =
        usize::try_from(reader.u32()?).map_err(|_| LaunchKernelDecodeErrorV2::InvalidEncoding)?;
    if declared_len != bytes.len() {
        return Err(LaunchKernelDecodeErrorV2::LengthMismatch);
    }
    let variant_count = usize::from(reader.u16()?);
    let parameter_count = usize::from(reader.u16()?);
    if reader.u32()? != 0 {
        return Err(LaunchKernelDecodeErrorV2::NonZeroReserved);
    }
    decode_count("variants", variant_count, limits.max_variants)?;
    decode_count("parameters", parameter_count, limits.max_parameters)?;
    let minimum_len = HEADER_BYTES
        .checked_add(TARGET_BYTES)
        .and_then(|value| value.checked_add(64 + 2 + 12))
        .and_then(|value| value.checked_add(parameter_count.checked_mul(PARAMETER_BYTES)?))
        .and_then(|value| value.checked_add(variant_count.checked_mul(VARIANT_FIXED_BYTES + 4)?))
        .ok_or(LaunchKernelDecodeErrorV2::InvalidEncoding)?;
    if minimum_len > bytes.len() {
        return Err(LaunchKernelDecodeErrorV2::Truncated);
    }

    let target = Gfx942TargetBindingV2 {
        identity: TargetIdentityV2(reader.array32()?),
        architecture: match reader.u8()? {
            1 => AmdArchitectureV2::Gfx942,
            _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
        },
        xnack: match reader.u8()? {
            0 => XnackModeV2::Disabled,
            1 => XnackModeV2::Enabled,
            _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
        },
        code_object: match reader.u8()? {
            5 => CodeObjectVersionV2::V5,
            6 => CodeObjectVersionV2::V6,
            _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
        },
        pointer_width_bytes: reader.u8()?,
        endianness: match reader.u8()? {
            1 => EndiannessV2::Little,
            2 => EndiannessV2::Big,
            _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
        },
    };
    if reader.take(3)? != [0, 0, 0] {
        return Err(LaunchKernelDecodeErrorV2::NonZeroReserved);
    }

    let family_identity = KernelFamilyIdentityV2(reader.array32()?);
    let signature_identity = KernelSignatureIdentityV2(reader.array32()?);
    let logical_name = reader.name(limits)?;
    let explicit_argument_bytes = reader.u32()?;
    let kernarg_segment_bytes = reader.u32()?;
    let kernarg_segment_alignment = reader.u32()?;
    let mut parameters = Vec::with_capacity(parameter_count);
    for _ in 0..parameter_count {
        let source_index = reader.u16()?;
        let kind = match reader.u8()? {
            0 => AbiParameterKindV2::ByValue,
            1 => AbiParameterKindV2::SharedGlobalPointer,
            2 => AbiParameterKindV2::UniqueGlobalPointer,
            _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
        };
        if reader.u8()? != 0 {
            return Err(LaunchKernelDecodeErrorV2::NonZeroReserved);
        }
        parameters.push(AbiParameterV2 {
            source_index,
            kind,
            offset: reader.u32()?,
            size: reader.u32()?,
            alignment: reader.u32()?,
            semantic_type: SemanticTypeIdentityV2(reader.array32()?),
        });
    }

    let mut variants = Vec::with_capacity(variant_count);
    for _ in 0..variant_count {
        let kernel_identity = KernelIdentityV2(reader.array32()?);
        let policy_identity = KernelPolicyIdentityV2(reader.array32()?);
        let artifact_identity = ArtifactIdentityV2(reader.array32()?);
        let tuple_identity = KernelVariantTupleIdentityV2(reader.array32()?);
        let variant_name = reader.name(limits)?;
        let entry_name = reader.name(limits)?;
        let rank = reader.u8()?;
        let block_tag = reader.u8()?;
        let wavefront = match reader.u8()? {
            1 => WavefrontWidthV2::Wave32,
            2 => WavefrontWidthV2::Wave64,
            _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
        };
        let require_full_waves = decode_bool(reader.u8()?)?;
        let minimum_waves_per_execution_unit = reader.u8()?;
        let maximum_waves_per_execution_unit = reader.u8()?;
        let unsupported = decode_unsupported(reader.u8()?)?;
        if reader.u8()? != 0 {
            return Err(LaunchKernelDecodeErrorV2::NonZeroReserved);
        }
        let minimum = reader.dimensions()?;
        let maximum = reader.dimensions()?;
        let block = match block_tag {
            0 if minimum == maximum => BlockShapePolicyV2::Exact(minimum),
            0 => return Err(LaunchKernelDecodeErrorV2::NonCanonicalEncoding),
            1 => BlockShapePolicyV2::Bounded { minimum, maximum },
            _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
        };
        let max_grid_blocks = reader.dimensions()?;
        let minimum_flat_workgroup_size = reader.u32()?;
        let maximum_flat_workgroup_size = reader.u32()?;
        let max_total_workitems = reader.u64()?;
        let resources = Gfx942ResourceLimitsV2 {
            static_lds_bytes: reader.u32()?,
            maximum_dynamic_lds_bytes: reader.u32()?,
            dynamic_lds_alignment: reader.u32()?,
            private_segment_bytes: reader.u32()?,
        };
        let occupancy_tag = reader.u8()?;
        if reader.take(3)? != [0, 0, 0] {
            return Err(LaunchKernelDecodeErrorV2::NonZeroReserved);
        }
        let occupancy_verifier = OccupancyVerifierIdentityV2(reader.array32()?);
        let occupancy_metadata = OccupancyMetadataIdentityV2(reader.array32()?);
        let occupancy_subject = OccupancySubjectIdentityV2(reader.array32()?);
        let occupancy_minimum = reader.u8()?;
        let occupancy_maximum = reader.u8()?;
        if reader.take(2)? != [0, 0] {
            return Err(LaunchKernelDecodeErrorV2::NonZeroReserved);
        }
        let occupancy_witness = match occupancy_tag {
            0 if occupancy_verifier.is_zero()
                && occupancy_metadata.is_zero()
                && occupancy_subject.is_zero()
                && occupancy_minimum == 0
                && occupancy_maximum == 0 =>
            {
                None
            }
            0 => return Err(LaunchKernelDecodeErrorV2::NonCanonicalEncoding),
            1 => Some(Gfx942OccupancyWitnessV2 {
                verifier_identity: occupancy_verifier,
                metadata_identity: occupancy_metadata,
                subject_identity: occupancy_subject,
                minimum_waves_per_execution_unit: occupancy_minimum,
                maximum_waves_per_execution_unit: occupancy_maximum,
            }),
            _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
        };
        let capability_count = usize::from(reader.u16()?);
        let proof_count = usize::from(reader.u16()?);
        if reader.u32()? != 0 {
            return Err(LaunchKernelDecodeErrorV2::NonZeroReserved);
        }
        decode_count(
            "capabilities",
            capability_count,
            limits.max_capabilities_per_variant,
        )?;
        decode_count(
            "proof obligations",
            proof_count,
            limits.max_proof_obligations_per_variant,
        )?;
        let mut capabilities = Vec::with_capacity(capability_count);
        for _ in 0..capability_count {
            capabilities.push(match reader.u8()? {
                1 => LaunchCapabilityV2::ExactWaveMode,
                2 => LaunchCapabilityV2::StaticLds,
                3 => LaunchCapabilityV2::DynamicLds,
                4 => LaunchCapabilityV2::WorkgroupBarrier,
                5 => LaunchCapabilityV2::DeviceAtomics,
                _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
            });
        }
        let mut proof_obligations = Vec::with_capacity(proof_count);
        for _ in 0..proof_count {
            let kind = match reader.u8()? {
                1 => LaunchProofKindV2::TargetAuthenticated,
                2 => LaunchProofKindV2::ArtifactAuthenticated,
                3 => LaunchProofKindV2::KernelIdentityAuthenticated,
                4 => LaunchProofKindV2::SignatureLayoutAuthenticated,
                5 => LaunchProofKindV2::PolicySelectionAuthenticated,
                6 => LaunchProofKindV2::GeometryAndResourcesProved,
                _ => return Err(LaunchKernelDecodeErrorV2::UnknownTag),
            };
            proof_obligations.push(LaunchProofObligationV2::new(
                kind,
                KernelVariantTupleIdentityV2(reader.array32()?),
            ));
        }
        variants.push(KernelVariantV2 {
            kernel_identity,
            policy_identity,
            artifact_identity,
            tuple_identity,
            variant_name,
            entry_name,
            launch: Gfx942LaunchContractV2 {
                rank,
                block,
                max_grid_blocks,
                minimum_flat_workgroup_size,
                maximum_flat_workgroup_size,
                wavefront,
                require_full_waves,
                minimum_waves_per_execution_unit,
                maximum_waves_per_execution_unit,
                max_total_workitems,
                unsupported,
            },
            resources,
            occupancy_witness,
            capabilities,
            proof_obligations,
        });
    }
    if !reader.is_empty() {
        return Err(LaunchKernelDecodeErrorV2::LengthMismatch);
    }

    let family = LaunchKernelFamilyV2 {
        target,
        family_identity,
        logical_name,
        signature: KernelSignatureV2 {
            identity: signature_identity,
            explicit_argument_bytes,
            kernarg_segment_bytes,
            kernarg_segment_alignment,
            parameters,
        },
        variants,
    };
    family.validate(limits)?;
    let canonical =
        encode_launch_kernel_family_v2(&family, limits).map_err(|error| match error {
            LaunchKernelEncodeErrorV2::Model(model) => LaunchKernelDecodeErrorV2::Model(model),
            LaunchKernelEncodeErrorV2::EncodedLengthOverflow => {
                LaunchKernelDecodeErrorV2::InvalidEncoding
            }
        })?;
    if canonical != bytes {
        return Err(LaunchKernelDecodeErrorV2::NonCanonicalEncoding);
    }
    Ok(family)
}

fn encoded_len(family: &LaunchKernelFamilyV2) -> Result<usize, LaunchKernelEncodeErrorV2> {
    let mut length = HEADER_BYTES
        .checked_add(TARGET_BYTES)
        .and_then(|value| value.checked_add(64))
        .and_then(|value| value.checked_add(2 + family.logical_name.len()))
        .and_then(|value| value.checked_add(12))
        .and_then(|value| {
            value.checked_add(
                family
                    .signature
                    .parameters
                    .len()
                    .checked_mul(PARAMETER_BYTES)?,
            )
        })
        .ok_or(LaunchKernelEncodeErrorV2::EncodedLengthOverflow)?;
    for variant in &family.variants {
        length = length
            .checked_add(VARIANT_FIXED_BYTES)
            .and_then(|value| value.checked_add(2 + variant.variant_name.len()))
            .and_then(|value| value.checked_add(2 + variant.entry_name.len()))
            .and_then(|value| value.checked_add(variant.capabilities.len()))
            .and_then(|value| {
                value.checked_add(
                    variant
                        .proof_obligations
                        .len()
                        .checked_mul(PROOF_OBLIGATION_BYTES)?,
                )
            })
            .ok_or(LaunchKernelEncodeErrorV2::EncodedLengthOverflow)?;
    }
    Ok(length)
}

fn put_name(bytes: &mut Vec<u8>, name: &str) -> Result<(), LaunchKernelEncodeErrorV2> {
    let length =
        u16::try_from(name.len()).map_err(|_| LaunchKernelEncodeErrorV2::EncodedLengthOverflow)?;
    put_u16(bytes, length);
    bytes.extend_from_slice(name.as_bytes());
    Ok(())
}

fn put_dimensions(bytes: &mut Vec<u8>, dimensions: DimensionsV2) {
    put_u32(bytes, dimensions.x);
    put_u32(bytes, dimensions.y);
    put_u32(bytes, dimensions.z);
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

const fn unsupported_bits(features: UnsupportedLaunchFeaturesV2) -> u8 {
    (features.cooperative_grid as u8)
        | ((features.device_side_enqueue as u8) << 1)
        | ((features.dynamic_parallelism as u8) << 2)
}

fn decode_unsupported(bits: u8) -> Result<UnsupportedLaunchFeaturesV2, LaunchKernelDecodeErrorV2> {
    if bits & !0b111 != 0 {
        return Err(LaunchKernelDecodeErrorV2::UnknownTag);
    }
    Ok(UnsupportedLaunchFeaturesV2 {
        cooperative_grid: bits & 1 != 0,
        device_side_enqueue: bits & 2 != 0,
        dynamic_parallelism: bits & 4 != 0,
    })
}

fn decode_bool(value: u8) -> Result<bool, LaunchKernelDecodeErrorV2> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(LaunchKernelDecodeErrorV2::InvalidEncoding),
    }
}

fn decode_count(
    resource: &'static str,
    observed: usize,
    limit: usize,
) -> Result<(), LaunchKernelDecodeErrorV2> {
    if observed > limit {
        return Err(LaunchKernelDecodeErrorV2::ResourceLimit {
            resource,
            observed,
            limit,
        });
    }
    Ok(())
}

struct ReaderV2<'a> {
    remaining: &'a [u8],
}

impl<'a> ReaderV2<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], LaunchKernelDecodeErrorV2> {
        if count > self.remaining.len() {
            return Err(LaunchKernelDecodeErrorV2::Truncated);
        }
        let (head, tail) = self.remaining.split_at(count);
        self.remaining = tail;
        Ok(head)
    }

    fn u8(&mut self) -> Result<u8, LaunchKernelDecodeErrorV2> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, LaunchKernelDecodeErrorV2> {
        let bytes: [u8; 2] = self
            .take(2)?
            .try_into()
            .map_err(|_| LaunchKernelDecodeErrorV2::Truncated)?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, LaunchKernelDecodeErrorV2> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| LaunchKernelDecodeErrorV2::Truncated)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, LaunchKernelDecodeErrorV2> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| LaunchKernelDecodeErrorV2::Truncated)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn array32(&mut self) -> Result<[u8; 32], LaunchKernelDecodeErrorV2> {
        self.take(32)?
            .try_into()
            .map_err(|_| LaunchKernelDecodeErrorV2::Truncated)
    }

    fn name(&mut self, limits: &LaunchKernelLimitsV2) -> Result<String, LaunchKernelDecodeErrorV2> {
        let length = usize::from(self.u16()?);
        decode_count("name bytes", length, limits.max_name_bytes)?;
        let value = std::str::from_utf8(self.take(length)?)
            .map_err(|_| LaunchKernelDecodeErrorV2::InvalidUtf8)?;
        Ok(value.to_owned())
    }

    fn dimensions(&mut self) -> Result<DimensionsV2, LaunchKernelDecodeErrorV2> {
        Ok(DimensionsV2 {
            x: self.u32()?,
            y: self.u32()?,
            z: self.u32()?,
        })
    }
}
