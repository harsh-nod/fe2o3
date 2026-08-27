use std::collections::BTreeSet;

use crate::{
    AccessMode, AddressSpace, Convergence, MemoryEffect, ScalarType, SynchronizationScope,
    TargetCapability, Type, ValueDef, ValueId, WaveWidth,
};

pub const MATRIX_CAPABILITY_NAMESPACE: &str = "fe2o3.matrix";
pub const BF16_F32_M16N16K16_CAPABILITY: &str = "mma-bf16-f32-m16n16k16-wave64.v1";
pub const SCALED_FP8_E4M3_F32_M16N16K128_CAPABILITY: &str =
    "scaled-mma-fp8-e4m3-f32-m16n16k128-wave64.v1";
pub const SCALED_FP4_E2M1_F32_M16N16K128_CAPABILITY: &str =
    "scaled-mma-fp4-e2m1-f32-m16n16k128-wave64.v1";
pub const SCALED_FP4_E2M1_FP8_E4M3_F32_M16N16K128_CAPABILITY: &str =
    "scaled-mma-fp4-e2m1-fp8-e4m3-f32-m16n16k128-wave64.v1";
pub const LDS_TILE_16X16_XOR4_CAPABILITY: &str = "lds-tile-16x16-xor4-wave64.v1";

pub const MATRIX_SOURCE_ABI_OBSERVATION_NAMESPACE_V2: &str =
    "fe2o3.rustc.source-abi-observation.v2";
pub const MATRIX_PROJECTED_KERNARG_POLICY_NAMESPACE_V1: &str =
    "fe2o3.amdhsa.projected-kernarg-policy.v1";
pub const MATRIX_SOURCE_ABI_RECORD_DOMAIN_V2: &[u8] =
    b"FE2O3/MATRIX-RUSTC-SOURCE-ABI-OBSERVATION/V2\0";
const MAX_MATRIX_SOURCE_ABI_RECORD_BYTES_V2: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatrixProviderIdentityV2 {
    pub crate_name: String,
    pub stable_crate_id: u64,
    pub crate_hash: [u8; 16],
    pub cargo_metadata_build_observation: [u8; 32],
    pub source_identity: [u8; 32],
    pub definition_identities: Vec<[u8; 16]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatrixSourceAbiObservationV2 {
    pub provider: MatrixProviderIdentityV2,
    pub canonical_record: Vec<u8>,
    pub digest: [u8; 32],
}

impl MatrixSourceAbiObservationV2 {
    /// Constructs an untrusted frontend observation claim.
    ///
    /// Kernel IR can preserve and integrity-check this record, but only the
    /// rustc importer can establish that the bytes came from the current
    /// compiler session. Generic IR construction does not authenticate it.
    pub fn new_untrusted_claim(
        provider: MatrixProviderIdentityV2,
        canonical_record: Vec<u8>,
    ) -> Result<Self, &'static str> {
        let digest = crate::launch_kernel_contract_v2::sha256_v2(&canonical_record);
        let observation = Self {
            provider,
            canonical_record,
            digest,
        };
        observation.validate()?;
        Ok(observation)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.provider.crate_name != "fe2o3_device"
            || self.provider.stable_crate_id == 0
            || self.provider.crate_hash == [0; 16]
            || self.provider.cargo_metadata_build_observation == [0; 32]
            || self.provider.source_identity == [0; 32]
            || self.provider.definition_identities.len() != 6
            || self
                .provider
                .definition_identities
                .iter()
                .any(|identity| identity == &[0; 16])
        {
            return Err("matrix source ABI provider identity is incomplete");
        }
        if self.canonical_record.len() > MAX_MATRIX_SOURCE_ABI_RECORD_BYTES_V2
            || !self
                .canonical_record
                .starts_with(MATRIX_SOURCE_ABI_RECORD_DOMAIN_V2)
        {
            return Err("matrix source ABI record has an invalid domain or size");
        }
        validate_provider_record_prefix(&self.provider, &self.canonical_record)?;
        if crate::launch_kernel_contract_v2::sha256_v2(&self.canonical_record) != self.digest {
            return Err("matrix source ABI record digest mismatch");
        }
        Ok(())
    }

    pub fn capability(&self) -> TargetCapability {
        TargetCapability::Extension {
            namespace: MATRIX_SOURCE_ABI_OBSERVATION_NAMESPACE_V2.to_owned(),
            name: lower_hex(&self.digest),
        }
    }
}

fn validate_provider_record_prefix(
    provider: &MatrixProviderIdentityV2,
    record: &[u8],
) -> Result<(), &'static str> {
    struct Reader<'a> {
        remaining: &'a [u8],
    }

    impl<'a> Reader<'a> {
        fn take(&mut self, length: usize) -> Result<&'a [u8], &'static str> {
            let (value, remaining) = self
                .remaining
                .split_at_checked(length)
                .ok_or("matrix source ABI provider prefix is truncated")?;
            self.remaining = remaining;
            Ok(value)
        }

        fn u32(&mut self) -> Result<u32, &'static str> {
            Ok(u32::from_le_bytes(
                self.take(4)?.try_into().expect("four-byte slice is a u32"),
            ))
        }

        fn u64(&mut self) -> Result<u64, &'static str> {
            Ok(u64::from_le_bytes(
                self.take(8)?.try_into().expect("eight-byte slice is a u64"),
            ))
        }

        fn bytes(&mut self) -> Result<&'a [u8], &'static str> {
            let length = usize::try_from(self.u32()?)
                .map_err(|_| "matrix source ABI provider prefix length overflowed")?;
            self.take(length)
        }
    }

    let mut reader = Reader {
        remaining: &record[MATRIX_SOURCE_ABI_RECORD_DOMAIN_V2.len()..],
    };
    if reader.bytes()? != provider.crate_name.as_bytes()
        || reader.u64()? != provider.stable_crate_id
        || reader.bytes()? != provider.crate_hash
        || reader.bytes()? != provider.cargo_metadata_build_observation
        || reader.bytes()? != provider.source_identity
        || usize::try_from(reader.u32()?).ok() != Some(provider.definition_identities.len())
    {
        return Err("matrix source ABI provider prefix disagrees with its structured identity");
    }
    for identity in &provider.definition_identities {
        if reader.bytes()? != identity {
            return Err("matrix source ABI provider prefix disagrees with its structured identity");
        }
    }
    if reader.remaining.is_empty() {
        return Err("matrix source ABI record omits rustc layout and FnAbi facts");
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatrixProjectedParameterV1 {
    pub source: u8,
    pub lane: u8,
    pub element: MatrixElement,
    pub offset: u16,
    pub size: u8,
    pub alignment: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatrixProjectedKernargPolicyV1 {
    pub parameters: [MatrixProjectedParameterV1; 12],
    pub explicit_argument_size: u16,
    pub implicit_argument_bytes: u16,
    pub kernarg_segment_size: u16,
    pub kernarg_segment_alignment: u8,
    pub digest: [u8; 32],
}

impl MatrixProjectedKernargPolicyV1 {
    pub fn canonical() -> Self {
        let parameters = [
            projected(0, 0, MatrixElement::Bf16, 0, 2, 2),
            projected(0, 1, MatrixElement::Bf16, 2, 2, 2),
            projected(0, 2, MatrixElement::Bf16, 4, 2, 2),
            projected(0, 3, MatrixElement::Bf16, 6, 2, 2),
            projected(1, 0, MatrixElement::Bf16, 8, 2, 2),
            projected(1, 1, MatrixElement::Bf16, 10, 2, 2),
            projected(1, 2, MatrixElement::Bf16, 12, 2, 2),
            projected(1, 3, MatrixElement::Bf16, 14, 2, 2),
            projected(2, 0, MatrixElement::F32, 16, 4, 4),
            projected(2, 1, MatrixElement::F32, 20, 4, 4),
            projected(2, 2, MatrixElement::F32, 24, 4, 4),
            projected(2, 3, MatrixElement::F32, 28, 4, 4),
        ];
        let mut policy = Self {
            parameters,
            explicit_argument_size: 32,
            implicit_argument_bytes: 256,
            kernarg_segment_size: 288,
            kernarg_segment_alignment: 8,
            digest: [0; 32],
        };
        policy.digest = crate::launch_kernel_contract_v2::sha256_v2(&policy.canonical_bytes());
        policy
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self != &Self::canonical() {
            return Err("matrix projected kernarg policy differs from canonical V1");
        }
        Ok(())
    }

    pub fn capability(&self) -> TargetCapability {
        TargetCapability::Extension {
            namespace: MATRIX_PROJECTED_KERNARG_POLICY_NAMESPACE_V1.to_owned(),
            name: lower_hex(&self.digest),
        }
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = b"FE2O3/MATRIX-PROJECTED-KERNARG-POLICY/V1\0".to_vec();
        for parameter in self.parameters {
            bytes.extend_from_slice(&[
                parameter.source,
                parameter.lane,
                match parameter.element {
                    MatrixElement::Bf16 => 1,
                    MatrixElement::F32 => 2,
                    MatrixElement::Fp8E4M3 => 3,
                    MatrixElement::Fp4E2M1 => 4,
                },
            ]);
            bytes.extend_from_slice(&parameter.offset.to_le_bytes());
            bytes.extend_from_slice(&[parameter.size, parameter.alignment]);
        }
        bytes.extend_from_slice(&self.explicit_argument_size.to_le_bytes());
        bytes.extend_from_slice(&self.implicit_argument_bytes.to_le_bytes());
        bytes.extend_from_slice(&self.kernarg_segment_size.to_le_bytes());
        bytes.push(self.kernarg_segment_alignment);
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatrixFrontendBindingV2 {
    pub observed_source: MatrixSourceAbiObservationV2,
    pub projected_kernarg: MatrixProjectedKernargPolicyV1,
}

impl MatrixFrontendBindingV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        self.observed_source.validate()?;
        self.projected_kernarg.validate()
    }

    pub fn capabilities(&self) -> [TargetCapability; 2] {
        [
            self.observed_source.capability(),
            self.projected_kernarg.capability(),
        ]
    }
}

const fn projected(
    source: u8,
    lane: u8,
    element: MatrixElement,
    offset: u16,
    size: u8,
    alignment: u8,
) -> MatrixProjectedParameterV1 {
    MatrixProjectedParameterV1 {
        source,
        lane,
        element,
        offset,
        size,
        alignment,
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MatrixElement {
    Bf16,
    F32,
    Fp4E2M1,
    Fp8E4M3,
}

impl MatrixElement {
    pub const fn ty(self) -> Type {
        match self {
            Self::Bf16 => Type::Scalar(ScalarType::Bf16),
            Self::F32 => Type::F32,
            Self::Fp4E2M1 => Type::Scalar(ScalarType::U8),
            Self::Fp8E4M3 => Type::Scalar(ScalarType::U8),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MatrixLayout {
    RowMajorXor4,
}

/// Operand position within a cooperative tensor instruction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TensorOperandRoleV1 {
    A,
    B,
    Accumulator,
}

/// Target packing consumed by one instruction operand.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TensorElementPackingV1 {
    Bf16PairInI32,
    F32Scalar,
    Fp4EightInI32,
    Fp8FourInI32,
    Unsupported(u8),
}

/// Whether the symbolic map owns each coordinate or intentionally broadcasts it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TensorMultiplicityV1 {
    Unique,
    Broadcast { factor: u8 },
}

/// Operand-local LDS storage-to-register transform applied before MFMA.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TensorLdsSwizzleV1 {
    None,
    Xor4,
    Unsupported(u8),
}

/// Tail participation contract at the tensor-instruction boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TensorTailMaskV1 {
    /// The physical M/N/K tile is fully inside all logical tensors.
    ExactPhysicalTile,
    /// Declares that out-of-range A/B components are zero before the full instruction.
    ///
    /// The source projector must authenticate the dominating zero-fill;
    /// retaining this enum alone is not evidence.
    ZeroFilledPredicateInputs,
    PredicateMask,
    Missing,
    Unsupported(u8),
}

/// A reviewed target instruction profile or an explicitly opaque future one.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TensorInstructionProfileV1 {
    Gfx942MfmaBf16F32M16N16K16Wave64,
    Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64,
    Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64,
    Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64,
    IncompatibleWave32,
    Opaque(u32),
}

/// Target-owned semantic facts consumed by workload-neutral tensor composition.
///
/// This describes one instruction ABI and its cooperative contribution tile;
/// it does not name or recognize a source workload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TensorInstructionSemanticDescriptorV1 {
    pub call_argument_count: u16,
    pub subgroup_width: u16,
    pub contribution_shape: [u16; 3],
    pub output_shape: [u16; 2],
}

impl TensorInstructionProfileV1 {
    pub const fn semantic_descriptor(self) -> Option<TensorInstructionSemanticDescriptorV1> {
        match self {
            Self::Gfx942MfmaBf16F32M16N16K16Wave64 => Some(TensorInstructionSemanticDescriptorV1 {
                call_argument_count: 4,
                subgroup_width: 64,
                contribution_shape: [16, 16, 16],
                output_shape: [16, 16],
            }),
            Self::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64 => {
                Some(TensorInstructionSemanticDescriptorV1 {
                    call_argument_count: 4,
                    subgroup_width: 64,
                    contribution_shape: [16, 16, 128],
                    output_shape: [16, 16],
                })
            }
            Self::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64 => {
                Some(TensorInstructionSemanticDescriptorV1 {
                    call_argument_count: 4,
                    subgroup_width: 64,
                    contribution_shape: [16, 16, 128],
                    output_shape: [16, 16],
                })
            }
            Self::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64 => {
                Some(TensorInstructionSemanticDescriptorV1 {
                    call_argument_count: 4,
                    subgroup_width: 64,
                    contribution_shape: [16, 16, 128],
                    output_shape: [16, 16],
                })
            }
            Self::IncompatibleWave32 | Self::Opaque(_) => None,
        }
    }
}

/// One logical coordinate expression:
/// `constant + lane % M * mod_scale + lane / D * div_scale + component * component_scale`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TensorCoordinateExprV1 {
    pub constant: u16,
    pub lane_mod_scale: u16,
    pub lane_div_scale: u16,
    pub component_scale: u16,
    pub tile_origin: bool,
}

impl TensorCoordinateExprV1 {
    pub const fn new(lane_mod_scale: u16, lane_div_scale: u16, component_scale: u16) -> Self {
        Self {
            constant: 0,
            lane_mod_scale,
            lane_div_scale,
            component_scale,
            tile_origin: true,
        }
    }
}

/// Closed symbolic forms understood by the bounded verifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TensorSymbolicMapV1 {
    LaneComponentAffine {
        lane_modulus: u16,
        lane_divisor: u16,
        axes: [TensorCoordinateExprV1; 2],
    },
    /// gfx950 FP8 operands split each lane's K contribution across K0..63/K64..127.
    Gfx950Fp8M16N16K128SplitK,
    /// Preserved for future extensions, but never accepted as a proof.
    Opaque(u32),
}

/// Explicit distribution of one instruction operand over subgroup lanes/register components.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TensorFragmentLayoutV1 {
    pub role: TensorOperandRoleV1,
    pub shape: [u16; 2],
    pub element: MatrixElement,
    pub fragment_elements: u8,
    pub mapping: TensorSymbolicMapV1,
    pub multiplicity: TensorMultiplicityV1,
    pub packing: TensorElementPackingV1,
    /// Storage transform for this operand only; it does not alter the register map.
    pub lds_swizzle: TensorLdsSwizzleV1,
}

/// Workload-neutral layout contract for one cooperative tensor instruction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TensorLayoutContractV1 {
    pub profile: TensorInstructionProfileV1,
    pub subgroup_width: u16,
    pub a: TensorFragmentLayoutV1,
    pub b: TensorFragmentLayoutV1,
    pub accumulator: TensorFragmentLayoutV1,
    pub tail_mask: TensorTailMaskV1,
}

impl TensorLayoutContractV1 {
    pub const fn gfx942_mfma_bf16_f32_m16n16k16_wave64() -> Self {
        Self {
            profile: TensorInstructionProfileV1::Gfx942MfmaBf16F32M16N16K16Wave64,
            subgroup_width: 64,
            a: canonical_fragment(TensorOperandRoleV1::A),
            b: canonical_fragment(TensorOperandRoleV1::B),
            accumulator: canonical_fragment(TensorOperandRoleV1::Accumulator),
            tail_mask: TensorTailMaskV1::ExactPhysicalTile,
        }
    }

    pub const fn gfx942_mfma_bf16_f32_m16n16k16_wave64_lds_xor4() -> Self {
        let mut contract = Self::gfx942_mfma_bf16_f32_m16n16k16_wave64();
        contract.a.lds_swizzle = TensorLdsSwizzleV1::Xor4;
        contract.b.lds_swizzle = TensorLdsSwizzleV1::Xor4;
        contract
    }

    pub const fn gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64() -> Self {
        Self {
            profile: TensorInstructionProfileV1::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64,
            subgroup_width: 64,
            a: canonical_gfx950_fp8_fragment(TensorOperandRoleV1::A),
            b: canonical_gfx950_fp8_fragment(TensorOperandRoleV1::B),
            accumulator: canonical_gfx950_fp8_fragment(TensorOperandRoleV1::Accumulator),
            tail_mask: TensorTailMaskV1::ZeroFilledPredicateInputs,
        }
    }

    pub const fn gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64() -> Self {
        Self {
            profile: TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64,
            subgroup_width: 64,
            a: canonical_gfx950_fp4_fragment(TensorOperandRoleV1::A),
            b: canonical_gfx950_fp4_fragment(TensorOperandRoleV1::B),
            accumulator: canonical_gfx950_fp4_fragment(TensorOperandRoleV1::Accumulator),
            tail_mask: TensorTailMaskV1::ZeroFilledPredicateInputs,
        }
    }

    pub const fn gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64() -> Self {
        Self {
            profile: TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64,
            subgroup_width: 64,
            a: canonical_gfx950_fp4_fragment(TensorOperandRoleV1::A),
            b: canonical_gfx950_fp8_fragment(TensorOperandRoleV1::B),
            accumulator: canonical_fragment(TensorOperandRoleV1::Accumulator),
            tail_mask: TensorTailMaskV1::ZeroFilledPredicateInputs,
        }
    }

    /// Declares an XOR4 LDS storage transform for operand A only.
    pub const fn with_a_lds_xor4(mut self) -> Self {
        self.a.lds_swizzle = TensorLdsSwizzleV1::Xor4;
        self
    }

    /// Declares an XOR4 LDS storage transform for operand B only.
    pub const fn with_b_lds_xor4(mut self) -> Self {
        self.b.lds_swizzle = TensorLdsSwizzleV1::Xor4;
        self
    }

    pub const fn with_zero_filled_predicate_inputs(mut self) -> Self {
        self.tail_mask = TensorTailMaskV1::ZeroFilledPredicateInputs;
        self
    }
}

const fn canonical_fragment(role: TensorOperandRoleV1) -> TensorFragmentLayoutV1 {
    let (element, packing, first, second) = match role {
        TensorOperandRoleV1::A => (
            MatrixElement::Bf16,
            TensorElementPackingV1::Bf16PairInI32,
            TensorCoordinateExprV1::new(1, 0, 0),
            TensorCoordinateExprV1::new(0, 4, 1),
        ),
        TensorOperandRoleV1::B => (
            MatrixElement::Bf16,
            TensorElementPackingV1::Bf16PairInI32,
            TensorCoordinateExprV1::new(0, 4, 1),
            TensorCoordinateExprV1::new(1, 0, 0),
        ),
        TensorOperandRoleV1::Accumulator => (
            MatrixElement::F32,
            TensorElementPackingV1::F32Scalar,
            TensorCoordinateExprV1::new(0, 4, 1),
            TensorCoordinateExprV1::new(1, 0, 0),
        ),
    };
    TensorFragmentLayoutV1 {
        role,
        shape: [16, 16],
        element,
        fragment_elements: 4,
        mapping: TensorSymbolicMapV1::LaneComponentAffine {
            lane_modulus: 16,
            lane_divisor: 16,
            axes: [first, second],
        },
        multiplicity: TensorMultiplicityV1::Unique,
        packing,
        lds_swizzle: TensorLdsSwizzleV1::None,
    }
}

const fn canonical_gfx950_fp8_fragment(role: TensorOperandRoleV1) -> TensorFragmentLayoutV1 {
    if matches!(role, TensorOperandRoleV1::Accumulator) {
        return canonical_fragment(role);
    }
    TensorFragmentLayoutV1 {
        role,
        shape: match role {
            TensorOperandRoleV1::A => [16, 128],
            TensorOperandRoleV1::B => [128, 16],
            TensorOperandRoleV1::Accumulator => [16, 16],
        },
        element: MatrixElement::Fp8E4M3,
        fragment_elements: 32,
        mapping: TensorSymbolicMapV1::Gfx950Fp8M16N16K128SplitK,
        multiplicity: TensorMultiplicityV1::Unique,
        packing: TensorElementPackingV1::Fp8FourInI32,
        lds_swizzle: TensorLdsSwizzleV1::None,
    }
}

const fn canonical_gfx950_fp4_fragment(role: TensorOperandRoleV1) -> TensorFragmentLayoutV1 {
    if matches!(role, TensorOperandRoleV1::Accumulator) {
        return canonical_fragment(role);
    }
    let (first, second) = match role {
        TensorOperandRoleV1::A => (
            TensorCoordinateExprV1::new(1, 0, 0),
            TensorCoordinateExprV1::new(0, 32, 1),
        ),
        TensorOperandRoleV1::B => (
            TensorCoordinateExprV1::new(0, 32, 1),
            TensorCoordinateExprV1::new(1, 0, 0),
        ),
        TensorOperandRoleV1::Accumulator => unreachable!(),
    };
    TensorFragmentLayoutV1 {
        role,
        shape: match role {
            TensorOperandRoleV1::A => [16, 128],
            TensorOperandRoleV1::B => [128, 16],
            TensorOperandRoleV1::Accumulator => [16, 16],
        },
        element: MatrixElement::Fp4E2M1,
        fragment_elements: 32,
        mapping: TensorSymbolicMapV1::LaneComponentAffine {
            lane_modulus: 16,
            lane_divisor: 16,
            axes: [first, second],
        },
        multiplicity: TensorMultiplicityV1::Unique,
        packing: TensorElementPackingV1::Fp4EightInI32,
        lds_swizzle: TensorLdsSwizzleV1::None,
    }
}

impl TensorFragmentLayoutV1 {
    /// Evaluates the reviewed affine form relative to the symbolic tile origin.
    pub fn logical_coordinate(self, lane: u16, component: u8) -> Option<[u64; 2]> {
        if lane >= 64 || component >= self.fragment_elements {
            return None;
        }
        match self.mapping {
            TensorSymbolicMapV1::LaneComponentAffine {
                lane_modulus,
                lane_divisor,
                axes,
            } => {
                if lane_modulus == 0 || lane_divisor == 0 {
                    return None;
                }
                evaluate_tensor_coordinate_v1(
                    u64::from(lane),
                    u64::from(component),
                    lane_modulus,
                    lane_divisor,
                    axes,
                )
            }
            TensorSymbolicMapV1::Gfx950Fp8M16N16K128SplitK => {
                let lane_axis = u64::from(lane % 16);
                let k = u64::from(lane / 16) * 16
                    + u64::from(component % 16)
                    + u64::from(component / 16) * 64;
                match self.role {
                    TensorOperandRoleV1::A => Some([lane_axis, k]),
                    TensorOperandRoleV1::B => Some([k, lane_axis]),
                    TensorOperandRoleV1::Accumulator => None,
                }
            }
            TensorSymbolicMapV1::Opaque(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TensorLayoutFindingV1 {
    UnsupportedProfile,
    ProfileMismatch {
        field: &'static str,
    },
    RoleMismatch {
        position: TensorOperandRoleV1,
        actual: TensorOperandRoleV1,
    },
    ShapeOrElementMismatch {
        role: TensorOperandRoleV1,
    },
    FragmentWidthMismatch {
        role: TensorOperandRoleV1,
        actual: u8,
    },
    UnsupportedSymbolicMap {
        role: TensorOperandRoleV1,
    },
    MalformedSymbolicMap {
        role: TensorOperandRoleV1,
    },
    SymbolicMapMismatch {
        role: TensorOperandRoleV1,
    },
    CoordinateOutOfBounds {
        role: TensorOperandRoleV1,
    },
    DuplicateCoordinate {
        role: TensorOperandRoleV1,
    },
    IncompleteCoverage {
        role: TensorOperandRoleV1,
    },
    BroadcastContractMismatch {
        role: TensorOperandRoleV1,
    },
    PackingMismatch {
        role: TensorOperandRoleV1,
    },
    SwizzleMismatch {
        role: TensorOperandRoleV1,
    },
    TailMaskMismatch,
}

impl TensorLayoutFindingV1 {
    pub const fn is_incomplete(&self) -> bool {
        matches!(
            self,
            Self::UnsupportedProfile | Self::UnsupportedSymbolicMap { .. }
        )
    }
}

impl std::fmt::Display for TensorLayoutFindingV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str("unsupported tensor target profile"),
            Self::ProfileMismatch { field } => {
                write!(
                    formatter,
                    "tensor instruction profile has incompatible {field}"
                )
            }
            Self::RoleMismatch { position, actual } => write!(
                formatter,
                "tensor operand position {position:?} carries role {actual:?}"
            ),
            Self::ShapeOrElementMismatch { role } => write!(
                formatter,
                "tensor {role:?} fragment shape or element type is incompatible with the instruction profile"
            ),
            Self::FragmentWidthMismatch { role, actual } => write!(
                formatter,
                "tensor {role:?} fragment has {actual} register components; expected 4"
            ),
            Self::UnsupportedSymbolicMap { role } => {
                write!(
                    formatter,
                    "tensor {role:?} fragment uses an unsupported symbolic map"
                )
            }
            Self::MalformedSymbolicMap { role } => write!(
                formatter,
                "tensor {role:?} fragment has a zero divisor or overflowing symbolic map"
            ),
            Self::SymbolicMapMismatch { role } => write!(
                formatter,
                "tensor {role:?} lane/component mapping does not match the target operand profile"
            ),
            Self::CoordinateOutOfBounds { role } => write!(
                formatter,
                "tensor {role:?} lane/component mapping produces an out-of-bounds logical coordinate"
            ),
            Self::DuplicateCoordinate { role } => write!(
                formatter,
                "tensor {role:?} lane/component mapping aliases a logical coordinate without declared broadcast"
            ),
            Self::IncompleteCoverage { role } => write!(
                formatter,
                "tensor {role:?} lane/component mapping does not cover its logical fragment"
            ),
            Self::BroadcastContractMismatch { role } => write!(
                formatter,
                "tensor {role:?} broadcast multiplicity does not match observed lane coverage"
            ),
            Self::PackingMismatch { role } => {
                write!(
                    formatter,
                    "tensor {role:?} target register packing is incompatible"
                )
            }
            Self::SwizzleMismatch { role } => {
                write!(
                    formatter,
                    "tensor {role:?} LDS swizzle is incompatible with its layout"
                )
            }
            Self::TailMaskMismatch => formatter.write_str(
                "tensor instruction tail-mask contract is incompatible with the exact-tile profile",
            ),
        }
    }
}

/// Bounded verification shared by canonical Kernel IR and ranked PLIRON.
pub fn verify_tensor_layout_contract_v1(
    contract: &TensorLayoutContractV1,
) -> Vec<TensorLayoutFindingV1> {
    let mut findings = Vec::new();
    match contract.profile {
        TensorInstructionProfileV1::Gfx942MfmaBf16F32M16N16K16Wave64
        | TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64
        | TensorInstructionProfileV1::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64
        | TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64 => {}
        TensorInstructionProfileV1::IncompatibleWave32 => {
            findings.push(TensorLayoutFindingV1::ProfileMismatch {
                field: "wave32 target profile",
            });
            return findings;
        }
        TensorInstructionProfileV1::Opaque(_) => {
            findings.push(TensorLayoutFindingV1::UnsupportedProfile);
            return findings;
        }
    }
    if contract.subgroup_width != 64 {
        findings.push(TensorLayoutFindingV1::ProfileMismatch {
            field: "subgroup width",
        });
    }
    for (position, fragment) in [
        (TensorOperandRoleV1::A, &contract.a),
        (TensorOperandRoleV1::B, &contract.b),
        (TensorOperandRoleV1::Accumulator, &contract.accumulator),
    ] {
        verify_tensor_fragment_v1(
            contract.profile,
            position,
            fragment,
            contract.subgroup_width,
            &mut findings,
        );
    }
    if !matches!(
        contract.tail_mask,
        TensorTailMaskV1::ExactPhysicalTile | TensorTailMaskV1::ZeroFilledPredicateInputs
    ) {
        findings.push(TensorLayoutFindingV1::TailMaskMismatch);
    }
    for (role, storage_transform) in [
        (TensorOperandRoleV1::A, contract.a.lds_swizzle),
        (TensorOperandRoleV1::B, contract.b.lds_swizzle),
    ] {
        if !matches!(
            storage_transform,
            TensorLdsSwizzleV1::None | TensorLdsSwizzleV1::Xor4
        ) {
            findings.push(TensorLayoutFindingV1::SwizzleMismatch { role });
        }
    }
    if contract.accumulator.lds_swizzle != TensorLdsSwizzleV1::None {
        findings.push(TensorLayoutFindingV1::SwizzleMismatch {
            role: TensorOperandRoleV1::Accumulator,
        });
    }
    findings
}

fn verify_tensor_fragment_v1(
    profile: TensorInstructionProfileV1,
    position: TensorOperandRoleV1,
    fragment: &TensorFragmentLayoutV1,
    subgroup_width: u16,
    findings: &mut Vec<TensorLayoutFindingV1>,
) {
    if fragment.role != position {
        findings.push(TensorLayoutFindingV1::RoleMismatch {
            position,
            actual: fragment.role,
        });
    }
    let expected = match profile {
        TensorInstructionProfileV1::Gfx942MfmaBf16F32M16N16K16Wave64 => {
            canonical_fragment(position)
        }
        TensorInstructionProfileV1::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64 => {
            canonical_gfx950_fp8_fragment(position)
        }
        TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64 => {
            canonical_gfx950_fp4_fragment(position)
        }
        TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64 => {
            match position {
                TensorOperandRoleV1::A => canonical_gfx950_fp4_fragment(position),
                TensorOperandRoleV1::B => canonical_gfx950_fp8_fragment(position),
                TensorOperandRoleV1::Accumulator => canonical_fragment(position),
            }
        }
        TensorInstructionProfileV1::IncompatibleWave32 | TensorInstructionProfileV1::Opaque(_) => {
            return;
        }
    };
    if fragment.shape != expected.shape || fragment.element != expected.element {
        findings.push(TensorLayoutFindingV1::ShapeOrElementMismatch { role: position });
    }
    if fragment.fragment_elements != expected.fragment_elements {
        findings.push(TensorLayoutFindingV1::FragmentWidthMismatch {
            role: position,
            actual: fragment.fragment_elements,
        });
        return;
    }
    if fragment.packing != expected.packing {
        findings.push(TensorLayoutFindingV1::PackingMismatch { role: position });
    }
    match fragment.mapping {
        TensorSymbolicMapV1::LaneComponentAffine {
            lane_modulus,
            lane_divisor,
            ..
        } if lane_modulus == 0 || lane_divisor == 0 => {
            findings.push(TensorLayoutFindingV1::MalformedSymbolicMap { role: position });
            return;
        }
        TensorSymbolicMapV1::Opaque(_) => {
            findings.push(TensorLayoutFindingV1::UnsupportedSymbolicMap { role: position });
            return;
        }
        TensorSymbolicMapV1::LaneComponentAffine { .. }
        | TensorSymbolicMapV1::Gfx950Fp8M16N16K128SplitK => {}
    }
    if fragment.mapping != expected.mapping {
        findings.push(TensorLayoutFindingV1::SymbolicMapMismatch { role: position });
    }

    let mut coordinates = std::collections::BTreeMap::<[u64; 2], u16>::new();
    for lane in 0..u64::from(subgroup_width.min(64)) {
        for component in 0..u64::from(fragment.fragment_elements) {
            let Some(coordinate) = fragment.logical_coordinate(lane as u16, component as u8) else {
                findings.push(TensorLayoutFindingV1::MalformedSymbolicMap { role: position });
                return;
            };
            if coordinate[0] >= u64::from(fragment.shape[0])
                || coordinate[1] >= u64::from(fragment.shape[1])
            {
                findings.push(TensorLayoutFindingV1::CoordinateOutOfBounds { role: position });
                return;
            }
            let count = coordinates.entry(coordinate).or_default();
            *count = count.saturating_add(1);
        }
    }
    let expected_coordinates = usize::from(fragment.shape[0]) * usize::from(fragment.shape[1]);
    match fragment.multiplicity {
        TensorMultiplicityV1::Unique => {
            if coordinates.values().any(|count| *count != 1) {
                findings.push(TensorLayoutFindingV1::DuplicateCoordinate { role: position });
            }
            if coordinates.len() != expected_coordinates {
                findings.push(TensorLayoutFindingV1::IncompleteCoverage { role: position });
            }
        }
        TensorMultiplicityV1::Broadcast { factor } => {
            if factor == 0
                || coordinates.len() != expected_coordinates
                || coordinates
                    .values()
                    .any(|count| *count != u16::from(factor))
            {
                findings.push(TensorLayoutFindingV1::BroadcastContractMismatch { role: position });
            }
        }
    }
}

fn evaluate_tensor_coordinate_v1(
    lane: u64,
    component: u64,
    lane_modulus: u16,
    lane_divisor: u16,
    axes: [TensorCoordinateExprV1; 2],
) -> Option<[u64; 2]> {
    let mut result = [0; 2];
    for (index, axis) in axes.into_iter().enumerate() {
        // Tile origins are symbolic translations. They do not change relative
        // coverage, but must be explicitly retained on both logical axes.
        if !axis.tile_origin {
            return None;
        }
        result[index] = u64::from(axis.constant)
            .checked_add(
                (lane % u64::from(lane_modulus)).checked_mul(u64::from(axis.lane_mod_scale))?,
            )?
            .checked_add(
                (lane / u64::from(lane_divisor)).checked_mul(u64::from(axis.lane_div_scale))?,
            )?
            .checked_add(component.checked_mul(u64::from(axis.component_scale))?)?;
    }
    Some(result)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MatrixMultiplyProfile {
    pub m: u16,
    pub n: u16,
    pub k: u16,
    pub input: MatrixElement,
    pub accumulator: MatrixElement,
    pub wave_width: WaveWidth,
}

impl MatrixMultiplyProfile {
    pub const fn bf16_f32_m16n16k16_wave64() -> Self {
        Self {
            m: 16,
            n: 16,
            k: 16,
            input: MatrixElement::Bf16,
            accumulator: MatrixElement::F32,
            wave_width: WaveWidth::Wave64,
        }
    }

    pub const fn fp8_e4m3_f32_m16n16k128_wave64() -> Self {
        Self {
            m: 16,
            n: 16,
            k: 128,
            input: MatrixElement::Fp8E4M3,
            accumulator: MatrixElement::F32,
            wave_width: WaveWidth::Wave64,
        }
    }

    pub const fn fp4_e2m1_f32_m16n16k128_wave64() -> Self {
        Self {
            m: 16,
            n: 16,
            k: 128,
            input: MatrixElement::Fp4E2M1,
            accumulator: MatrixElement::F32,
            wave_width: WaveWidth::Wave64,
        }
    }

    pub const fn is_supported_v1(self) -> bool {
        self.m == 16
            && self.n == 16
            && matches!(
                (self.input, self.k),
                (MatrixElement::Bf16, 16)
                    | (MatrixElement::Fp4E2M1, 128)
                    | (MatrixElement::Fp8E4M3, 128)
            )
            && matches!(self.accumulator, MatrixElement::F32)
            && matches!(self.wave_width, WaveWidth::Wave64)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MatrixLdsProfile {
    pub rows: u16,
    pub columns: u16,
    pub element: MatrixElement,
    pub layout: MatrixLayout,
    pub fragment_elements: u8,
    pub wave_width: WaveWidth,
}

impl MatrixLdsProfile {
    pub const fn tile_16x16_xor4_wave64(element: MatrixElement) -> Self {
        Self {
            rows: 16,
            columns: 16,
            element,
            layout: MatrixLayout::RowMajorXor4,
            fragment_elements: 4,
            wave_width: WaveWidth::Wave64,
        }
    }

    pub const fn is_supported_v1(self) -> bool {
        self.rows == 16
            && self.columns == 16
            && self.fragment_elements == 4
            && matches!(self.layout, MatrixLayout::RowMajorXor4)
            && matches!(self.wave_width, WaveWidth::Wave64)
    }

    pub const fn required_elements(self) -> u32 {
        self.rows as u32 * self.columns as u32
    }

    pub const fn required_alignment(self) -> u32 {
        match self.element {
            MatrixElement::Bf16 => 2,
            MatrixElement::F32 => 4,
            MatrixElement::Fp4E2M1 => 1,
            MatrixElement::Fp8E4M3 => 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatrixOperationKind {
    MultiplyAccumulate {
        lhs: [ValueId; 4],
        rhs: [ValueId; 4],
        accumulator: [ValueId; 4],
        profile: MatrixMultiplyProfile,
    },
    ScaledMultiplyAccumulate {
        lhs: [ValueId; 8],
        rhs: [ValueId; 8],
        accumulator: [ValueId; 4],
        profile: MatrixMultiplyProfile,
    },
    LdsLoad {
        base: ValueId,
        profile: MatrixLdsProfile,
    },
    LdsStore {
        base: ValueId,
        values: [ValueId; 4],
        profile: MatrixLdsProfile,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatrixOperation {
    pub kind: MatrixOperationKind,
    pub active_lanes: u32,
    pub convergence: Convergence,
    pub frontend_binding: Option<MatrixFrontendBindingV2>,
    pub tensor_layout: Option<TensorLayoutContractV1>,
}

impl MatrixOperation {
    pub fn multiply_accumulate(
        lhs: [ValueId; 4],
        rhs: [ValueId; 4],
        accumulator: [ValueId; 4],
    ) -> Self {
        Self::full(MatrixOperationKind::MultiplyAccumulate {
            lhs,
            rhs,
            accumulator,
            profile: MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64(),
        })
    }

    pub fn scaled_multiply_accumulate_fp8_e4m3(
        lhs: [ValueId; 8],
        rhs: [ValueId; 8],
        accumulator: [ValueId; 4],
    ) -> Self {
        Self::full(MatrixOperationKind::ScaledMultiplyAccumulate {
            lhs,
            rhs,
            accumulator,
            profile: MatrixMultiplyProfile::fp8_e4m3_f32_m16n16k128_wave64(),
        })
    }

    pub fn scaled_multiply_accumulate_fp4_e2m1(
        lhs: [ValueId; 8],
        rhs: [ValueId; 8],
        accumulator: [ValueId; 4],
    ) -> Self {
        Self::full(MatrixOperationKind::ScaledMultiplyAccumulate {
            lhs,
            rhs,
            accumulator,
            profile: MatrixMultiplyProfile::fp4_e2m1_f32_m16n16k128_wave64(),
        })
    }

    pub fn lds_load(base: ValueId, element: MatrixElement) -> Self {
        Self::full(MatrixOperationKind::LdsLoad {
            base,
            profile: MatrixLdsProfile::tile_16x16_xor4_wave64(element),
        })
    }

    pub fn lds_store(base: ValueId, values: [ValueId; 4], element: MatrixElement) -> Self {
        Self::full(MatrixOperationKind::LdsStore {
            base,
            values,
            profile: MatrixLdsProfile::tile_16x16_xor4_wave64(element),
        })
    }

    fn full(kind: MatrixOperationKind) -> Self {
        Self {
            kind,
            active_lanes: 64,
            convergence: Convergence::uniform(SynchronizationScope::Subgroup),
            frontend_binding: None,
            tensor_layout: None,
        }
    }

    /// Retains a declared layout/tail contract on this instruction occurrence.
    ///
    /// This builder grants no source-provenance, artifact, or launch authority.
    /// A production frontend must derive the declaration from authenticated
    /// semantic terminals and operand dominance before constructing Kernel IR.
    pub fn with_declared_tensor_layout(mut self, contract: TensorLayoutContractV1) -> Self {
        self.tensor_layout = Some(contract);
        self
    }

    pub fn with_frontend_binding(mut self, binding: MatrixFrontendBindingV2) -> Self {
        self.frontend_binding = Some(binding);
        self
    }

    pub fn operands(&self) -> Vec<ValueId> {
        match self.kind {
            MatrixOperationKind::MultiplyAccumulate {
                lhs,
                rhs,
                accumulator,
                ..
            } => lhs.into_iter().chain(rhs).chain(accumulator).collect(),
            MatrixOperationKind::ScaledMultiplyAccumulate {
                lhs,
                rhs,
                accumulator,
                ..
            } => lhs.into_iter().chain(rhs).chain(accumulator).collect(),
            MatrixOperationKind::LdsLoad { base, .. } => vec![base],
            MatrixOperationKind::LdsStore { base, values, .. } => {
                std::iter::once(base).chain(values).collect()
            }
        }
    }

    pub fn result_types(&self) -> Vec<Type> {
        match self.kind {
            MatrixOperationKind::MultiplyAccumulate { profile, .. }
            | MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } => {
                vec![profile.accumulator.ty(); 4]
            }
            MatrixOperationKind::LdsLoad { profile, .. } => vec![profile.element.ty(); 4],
            MatrixOperationKind::LdsStore { .. } => Vec::new(),
        }
    }

    pub fn memory_effects(&self) -> Vec<MemoryEffect> {
        match self.kind {
            MatrixOperationKind::MultiplyAccumulate { .. }
            | MatrixOperationKind::ScaledMultiplyAccumulate { .. } => Vec::new(),
            MatrixOperationKind::LdsLoad { .. } => {
                vec![MemoryEffect::Read(AddressSpace::Workgroup)]
            }
            MatrixOperationKind::LdsStore { .. } => {
                vec![MemoryEffect::Write(AddressSpace::Workgroup)]
            }
        }
    }

    pub fn required_capabilities(&self) -> BTreeSet<TargetCapability> {
        let (wave, extension) = match self.kind {
            MatrixOperationKind::MultiplyAccumulate { profile, .. } => {
                (profile.wave_width, BF16_F32_M16N16K16_CAPABILITY)
            }
            MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } => {
                let extension = if self.tensor_layout.as_ref().is_some_and(|contract| {
                    contract.profile
                        == TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64
                }) {
                    SCALED_FP4_E2M1_FP8_E4M3_F32_M16N16K128_CAPABILITY
                } else if profile == MatrixMultiplyProfile::fp4_e2m1_f32_m16n16k128_wave64() {
                        SCALED_FP4_E2M1_F32_M16N16K128_CAPABILITY
                    } else {
                        SCALED_FP8_E4M3_F32_M16N16K128_CAPABILITY
                    };
                (profile.wave_width, extension)
            }
            MatrixOperationKind::LdsLoad { profile, .. }
            | MatrixOperationKind::LdsStore { profile, .. } => {
                (profile.wave_width, LDS_TILE_16X16_XOR4_CAPABILITY)
            }
        };
        let mut capabilities = BTreeSet::from([
            TargetCapability::Subgroups,
            TargetCapability::SubgroupSize(wave.lanes()),
            TargetCapability::WaveWidth(wave),
            TargetCapability::Extension {
                namespace: MATRIX_CAPABILITY_NAMESPACE.to_string(),
                name: extension.to_string(),
            },
        ]);
        match self.kind {
            MatrixOperationKind::MultiplyAccumulate { .. } => {
                capabilities.insert(TargetCapability::BFloat16);
            }
            MatrixOperationKind::ScaledMultiplyAccumulate { .. } => {}
            MatrixOperationKind::LdsLoad { profile, .. }
            | MatrixOperationKind::LdsStore { profile, .. } => {
                capabilities.insert(TargetCapability::WorkgroupMemory);
                if profile.element == MatrixElement::Bf16 {
                    capabilities.insert(TargetCapability::BFloat16);
                }
            }
        }
        if let Some(binding) = &self.frontend_binding {
            capabilities.extend(binding.capabilities());
        }
        capabilities
    }

    pub fn verify(
        &self,
        operand_types: &[Option<Type>],
        results: &[ValueDef],
    ) -> Vec<MatrixVerificationIssue> {
        let mut issues = Vec::new();
        if self.active_lanes != 64 {
            issues.push(MatrixVerificationIssue::structure(format!(
                "matrix V1 requires all 64 lanes active, found {}",
                self.active_lanes
            )));
        }
        if self.convergence.scope() != SynchronizationScope::Subgroup {
            issues.push(MatrixVerificationIssue::structure(
                "matrix V1 requires uniform subgroup convergence",
            ));
        }
        if let Some(binding) = &self.frontend_binding
            && let Err(reason) = binding.validate()
        {
            issues.push(MatrixVerificationIssue::structure(reason));
        }
        match self.kind {
            MatrixOperationKind::MultiplyAccumulate { profile, .. } => {
                if profile != MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64() {
                    issues.push(MatrixVerificationIssue::structure(format!(
                        "unsupported matrix multiply profile {profile:?}"
                    )));
                }
                for actual in operand_types.iter().take(8) {
                    expect_type(actual, Type::Scalar(ScalarType::Bf16), &mut issues);
                }
                for actual in operand_types.iter().skip(8) {
                    expect_type(actual, Type::F32, &mut issues);
                }
                match &self.tensor_layout {
                    Some(contract) => {
                        for finding in verify_tensor_layout_contract_v1(contract) {
                            issues.push(MatrixVerificationIssue::structure(finding.to_string()));
                        }
                    }
                    None => issues.push(MatrixVerificationIssue::structure(
                        "matrix multiply requires an explicit tensor layout contract",
                    )),
                }
            }
            MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } => {
                let expected_layout_profiles = if profile
                    == MatrixMultiplyProfile::fp4_e2m1_f32_m16n16k128_wave64()
                {
                    &[
                            TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64,
                            TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64,
                        ][..]
                } else if profile == MatrixMultiplyProfile::fp8_e4m3_f32_m16n16k128_wave64() {
                    &[TensorInstructionProfileV1::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64][..]
                } else {
                    &[][..]
                };
                if expected_layout_profiles.is_empty() {
                    issues.push(MatrixVerificationIssue::structure(format!(
                        "unsupported scaled matrix multiply profile {profile:?}"
                    )));
                }
                for actual in operand_types.iter().take(16) {
                    expect_type(actual, Type::Scalar(ScalarType::U32), &mut issues);
                }
                for actual in operand_types.iter().skip(16) {
                    expect_type(actual, Type::F32, &mut issues);
                }
                match &self.tensor_layout {
                    Some(contract) if expected_layout_profiles.contains(&contract.profile) => {
                        for finding in verify_tensor_layout_contract_v1(contract) {
                            issues.push(MatrixVerificationIssue::structure(finding.to_string()));
                        }
                    }
                    Some(_) => issues.push(MatrixVerificationIssue::structure(
                        "scaled matrix multiply profile and gfx950 tensor layout disagree",
                    )),
                    None => issues.push(MatrixVerificationIssue::structure(
                        "scaled matrix multiply requires an explicit tensor layout contract",
                    )),
                }
            }
            MatrixOperationKind::LdsLoad { profile, .. } => {
                if self.tensor_layout.is_some() {
                    issues.push(MatrixVerificationIssue::structure(
                        "matrix LDS operations cannot carry an instruction layout contract",
                    ));
                }
                if self.frontend_binding.is_some() {
                    issues.push(MatrixVerificationIssue::structure(
                        "matrix frontend ABI binding is valid only on multiply-accumulate",
                    ));
                }
                verify_lds_profile(profile, false, operand_types.first(), &mut issues);
            }
            MatrixOperationKind::LdsStore { profile, .. } => {
                if self.tensor_layout.is_some() {
                    issues.push(MatrixVerificationIssue::structure(
                        "matrix LDS operations cannot carry an instruction layout contract",
                    ));
                }
                if self.frontend_binding.is_some() {
                    issues.push(MatrixVerificationIssue::structure(
                        "matrix frontend ABI binding is valid only on multiply-accumulate",
                    ));
                }
                verify_lds_profile(profile, true, operand_types.first(), &mut issues);
                for actual in operand_types.iter().skip(1) {
                    expect_type(actual, profile.element.ty(), &mut issues);
                }
            }
        }
        if operand_types.len() != self.operands().len() {
            issues.push(MatrixVerificationIssue::structure(
                "matrix operand/type arity mismatch",
            ));
        }
        let expected = self.result_types();
        if results.len() != expected.len() {
            issues.push(MatrixVerificationIssue::result(format!(
                "matrix operation defines {} results, expected {}",
                results.len(),
                expected.len()
            )));
        }
        for (result, expected) in results.iter().zip(expected) {
            if result.ty != expected {
                issues.push(MatrixVerificationIssue::result(format!(
                    "matrix result {} has type {:?}, expected {expected:?}",
                    result.id, result.ty
                )));
            }
        }
        issues
    }
}

fn verify_lds_profile(
    profile: MatrixLdsProfile,
    writable: bool,
    base: Option<&Option<Type>>,
    issues: &mut Vec<MatrixVerificationIssue>,
) {
    if !profile.is_supported_v1() {
        issues.push(MatrixVerificationIssue::structure(format!(
            "unsupported matrix LDS profile {profile:?}"
        )));
    }
    let Some(Some(Type::Pointer(pointer))) = base else {
        if !matches!(base, Some(None)) {
            issues.push(MatrixVerificationIssue::operand(
                "matrix LDS base must be a workgroup pointer",
            ));
        }
        return;
    };
    if pointer.address_space != AddressSpace::Workgroup
        || pointer.pointee.as_ref() != &profile.element.ty()
        || (writable && pointer.access != AccessMode::ReadWrite)
    {
        issues.push(MatrixVerificationIssue::operand(format!(
            "matrix LDS base {pointer:?} does not match element {:?}, workgroup address space, writable {writable}",
            profile.element
        )));
    }
}

fn expect_type(actual: &Option<Type>, expected: Type, issues: &mut Vec<MatrixVerificationIssue>) {
    if let Some(actual) = actual
        && actual != &expected
    {
        issues.push(MatrixVerificationIssue::operand(format!(
            "matrix operand has type {actual:?}, expected {expected:?}"
        )));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatrixVerificationIssueKind {
    InvalidStructure,
    InvalidOperandType,
    InvalidResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatrixVerificationIssue {
    pub kind: MatrixVerificationIssueKind,
    pub message: String,
}

impl MatrixVerificationIssue {
    fn structure(message: impl Into<String>) -> Self {
        Self {
            kind: MatrixVerificationIssueKind::InvalidStructure,
            message: message.into(),
        }
    }

    fn operand(message: impl Into<String>) -> Self {
        Self {
            kind: MatrixVerificationIssueKind::InvalidOperandType,
            message: message.into(),
        }
    }

    fn result(message: impl Into<String>) -> Self {
        Self {
            kind: MatrixVerificationIssueKind::InvalidResult,
            message: message.into(),
        }
    }
}
