use std::collections::BTreeSet;

use crate::{
    AccessMode, AddressSpace, Convergence, MemoryEffect, ScalarType, SynchronizationScope,
    TargetCapability, Type, ValueDef, ValueId, WaveWidth,
};

pub const MATRIX_CAPABILITY_NAMESPACE: &str = "fe2o3.matrix";
pub const BF16_F32_M16N16K16_CAPABILITY: &str = "mma-bf16-f32-m16n16k16-wave64.v1";
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
}

impl MatrixElement {
    pub const fn ty(self) -> Type {
        match self {
            Self::Bf16 => Type::Scalar(ScalarType::Bf16),
            Self::F32 => Type::F32,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MatrixLayout {
    RowMajorXor4,
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

    pub const fn is_supported_v1(self) -> bool {
        self.m == 16
            && self.n == 16
            && self.k == 16
            && matches!(self.input, MatrixElement::Bf16)
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
        }
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
            MatrixOperationKind::LdsLoad { base, .. } => vec![base],
            MatrixOperationKind::LdsStore { base, values, .. } => {
                std::iter::once(base).chain(values).collect()
            }
        }
    }

    pub fn result_types(&self) -> Vec<Type> {
        match self.kind {
            MatrixOperationKind::MultiplyAccumulate { profile, .. } => {
                vec![profile.accumulator.ty(); 4]
            }
            MatrixOperationKind::LdsLoad { profile, .. } => vec![profile.element.ty(); 4],
            MatrixOperationKind::LdsStore { .. } => Vec::new(),
        }
    }

    pub fn memory_effects(&self) -> Vec<MemoryEffect> {
        match self.kind {
            MatrixOperationKind::MultiplyAccumulate { .. } => Vec::new(),
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
                if !profile.is_supported_v1() {
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
            }
            MatrixOperationKind::LdsLoad { profile, .. } => {
                if self.frontend_binding.is_some() {
                    issues.push(MatrixVerificationIssue::structure(
                        "matrix frontend ABI binding is valid only on multiply-accumulate",
                    ));
                }
                verify_lds_profile(profile, false, operand_types.first(), &mut issues);
            }
            MatrixOperationKind::LdsStore { profile, .. } => {
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
