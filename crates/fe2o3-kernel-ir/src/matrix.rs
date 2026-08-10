use std::collections::BTreeSet;

use crate::{
    AccessMode, AddressSpace, Convergence, MemoryEffect, ScalarType, SynchronizationScope,
    TargetCapability, Type, ValueDef, ValueId, WaveWidth,
};

pub const MATRIX_CAPABILITY_NAMESPACE: &str = "fe2o3.matrix";
pub const BF16_F32_M16N16K16_CAPABILITY: &str = "mma-bf16-f32-m16n16k16-wave64.v1";
pub const LDS_TILE_16X16_XOR4_CAPABILITY: &str = "lds-tile-16x16-xor4-wave64.v1";

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
        }
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
                verify_lds_profile(profile, false, operand_types.first(), &mut issues);
            }
            MatrixOperationKind::LdsStore { profile, .. } => {
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
