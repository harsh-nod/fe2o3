use core::fmt;

use fe2o3_amd_target::{
    AdvancedCapabilityStatus, AmdTargetId, LdsTransposeInstruction, MatrixInstructionSet,
    MfmaFamily, MxFormat,
};

use crate::AmdgcnIntrinsic;

/// The exact low-precision input formats accepted by the reviewed gfx950
/// `v_mfma_f32_16x16x128_f8f6f4` profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx950MfmaFormat {
    Fp8E4M3Ocp,
    Fp8E5M2Ocp,
    Fp4E2M1Ocp,
}

impl Gfx950MfmaFormat {
    /// Hardware format immediate used for CBSZ or BLGP by LLVM's intrinsic.
    pub const fn format_immediate(self) -> u32 {
        match self {
            Self::Fp8E4M3Ocp => 0,
            Self::Fp8E5M2Ocp => 1,
            Self::Fp4E2M1Ocp => 4,
        }
    }

    /// Number of i32 VGPR values required by the LLVM overload for one input.
    pub const fn register_dwords(self) -> u32 {
        match self {
            Self::Fp8E4M3Ocp | Self::Fp8E5M2Ocp => 8,
            Self::Fp4E2M1Ocp => 4,
        }
    }

    fn llvm_vector_type(self) -> &'static str {
        match self.register_dwords() {
            4 => "<4 x i32>",
            8 => "<8 x i32>",
            _ => unreachable!("closed gfx950 MFMA format set"),
        }
    }

    const fn mfma_family(self) -> MfmaFamily {
        match self {
            Self::Fp8E4M3Ocp => MfmaFamily::F32FromFp8Ocp,
            Self::Fp8E5M2Ocp => MfmaFamily::F32FromBf8Ocp,
            Self::Fp4E2M1Ocp => MfmaFamily::F32FromFp4Ocp,
        }
    }

    const fn mx_format(self) -> MxFormat {
        match self {
            Self::Fp8E4M3Ocp => MxFormat::Fp8,
            Self::Fp8E5M2Ocp => MxFormat::Bf8,
            Self::Fp4E2M1Ocp => MxFormat::Fp4,
        }
    }
}

/// One exact scaled 16x16x128 MFMA operation with FP32 accumulation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx950ScaledMfma {
    lhs_format: Gfx950MfmaFormat,
    rhs_format: Gfx950MfmaFormat,
}

impl Gfx950ScaledMfma {
    pub const fn new(lhs_format: Gfx950MfmaFormat, rhs_format: Gfx950MfmaFormat) -> Self {
        Self {
            lhs_format,
            rhs_format,
        }
    }

    pub const fn lhs_format(self) -> Gfx950MfmaFormat {
        self.lhs_format
    }

    pub const fn rhs_format(self) -> Gfx950MfmaFormat {
        self.rhs_format
    }

    /// Exact overloaded LLVM intrinsic name for this input-format pair.
    pub fn intrinsic_name(self) -> String {
        format!(
            "llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v{}i32.v{}i32",
            self.lhs_format.register_dwords(),
            self.rhs_format.register_dwords(),
        )
    }

    /// Exact LLVM declaration required by this input-format pair.
    pub fn declaration(self) -> String {
        format!(
            "declare <4 x float> @{}({}, {}, <4 x float>, i32 immarg, i32 immarg, i32 immarg, i32, i32 immarg, i32)",
            self.intrinsic_name(),
            self.lhs_format.llvm_vector_type(),
            self.rhs_format.llvm_vector_type(),
        )
    }
}

/// One exact gfx950 LDS transpose-load form.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx950LdsTranspose {
    B4,
    B8,
    B16,
}

impl Gfx950LdsTranspose {
    pub const fn intrinsic(self) -> AmdgcnIntrinsic {
        match self {
            Self::B4 => AmdgcnIntrinsic::DsReadTr4B64,
            Self::B8 => AmdgcnIntrinsic::DsReadTr8B64,
            Self::B16 => AmdgcnIntrinsic::DsReadTr16B64,
        }
    }

    pub const fn declaration(self) -> &'static str {
        match self {
            Self::B4 => {
                "declare <2 x i32> @llvm.amdgcn.ds.read.tr4.b64.v2i32(ptr addrspace(3) nocapture)"
            }
            Self::B8 => {
                "declare <2 x i32> @llvm.amdgcn.ds.read.tr8.b64.v2i32(ptr addrspace(3) nocapture)"
            }
            Self::B16 => {
                "declare <4 x i16> @llvm.amdgcn.ds.read.tr16.b64.v4i16(ptr addrspace(3) nocapture)"
            }
        }
    }

    const fn capability(self) -> LdsTransposeInstruction {
        match self {
            Self::B4 => LdsTransposeInstruction::DsReadTr4B64,
            Self::B8 => LdsTransposeInstruction::DsReadTr8B64,
            Self::B16 => LdsTransposeInstruction::DsReadTr16B64,
        }
    }

    const fn result_type(self) -> &'static str {
        match self {
            Self::B4 | Self::B8 => "<2 x i32>",
            Self::B16 => "<4 x i16>",
        }
    }
}

/// Why a gfx950-specific LLVM fragment was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950LoweringError {
    UnsupportedTarget(AmdTargetId),
    UnsupportedCapability(&'static str),
}

impl fmt::Display for Gfx950LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTarget(target) => {
                write!(formatter, "gfx950 lowering rejects target {target}")
            }
            Self::UnsupportedCapability(capability) => {
                write!(
                    formatter,
                    "gfx950 target profile does not admit {capability}"
                )
            }
        }
    }
}

impl std::error::Error for Gfx950LoweringError {}

/// Emits one exact LLVM call fragment for gfx950 scaled MFMA.
///
/// Numeric SSA IDs keep the textual boundary injection-free. The caller owns
/// the surrounding function and must bind each input to the format-dependent
/// vector type (`<4 x i32>` for FP4 or `<8 x i32>` for FP8/BF8), followed by
/// one `<4 x float>` accumulator. Scale operands are fixed to the reviewed
/// identity value; the format immediates select FP8/BF8/FP4.
pub fn lower_gfx950_scaled_mfma_to_llvm_ir(
    target: AmdTargetId,
    operation: Gfx950ScaledMfma,
    result: u32,
    lhs: u32,
    rhs: u32,
    accumulator: u32,
) -> Result<String, Gfx950LoweringError> {
    let capabilities = validate_gfx950_target(target)?;
    if !capabilities
        .matrix_instruction_sets()
        .contains(MatrixInstructionSet::ScaledMfmaF8F6F4)
    {
        return Err(Gfx950LoweringError::UnsupportedCapability(
            "scaled f8/f6/f4 MFMA",
        ));
    }
    for format in [operation.lhs_format, operation.rhs_format] {
        if capabilities.mfma_family_support(format.mfma_family())
            != AdvancedCapabilityStatus::Supported
            || capabilities.mx_format_support(format.mx_format())
                != AdvancedCapabilityStatus::Supported
        {
            return Err(Gfx950LoweringError::UnsupportedCapability(
                "requested MFMA input format",
            ));
        }
    }

    Ok(format!(
        "%{result} = call <4 x float> @{}({} %{lhs}, {} %{rhs}, <4 x float> %{accumulator}, i32 {}, i32 {}, i32 0, i32 0, i32 0, i32 0)",
        operation.intrinsic_name(),
        operation.lhs_format.llvm_vector_type(),
        operation.rhs_format.llvm_vector_type(),
        operation.lhs_format.format_immediate(),
        operation.rhs_format.format_immediate(),
    ))
}

/// Emits one exact LLVM call fragment for a gfx950 LDS transpose load.
///
/// The address SSA value must be a pointer in address space 3 in the
/// surrounding function.
pub fn lower_gfx950_lds_transpose_to_llvm_ir(
    target: AmdTargetId,
    operation: Gfx950LdsTranspose,
    result: u32,
    lds_address: u32,
) -> Result<String, Gfx950LoweringError> {
    let capabilities = validate_gfx950_target(target)?;
    if capabilities.lds_transpose_instruction_support(operation.capability())
        != AdvancedCapabilityStatus::Supported
    {
        return Err(Gfx950LoweringError::UnsupportedCapability(
            "requested LDS transpose load",
        ));
    }

    Ok(format!(
        "%{result} = call {} @{}(ptr addrspace(3) %{lds_address})",
        operation.result_type(),
        operation.intrinsic().llvm_name(),
    ))
}

fn validate_gfx950_target(
    target: AmdTargetId,
) -> Result<fe2o3_amd_target::AmdTargetCapabilities, Gfx950LoweringError> {
    if target.processor() != "gfx950" {
        return Err(Gfx950LoweringError::UnsupportedTarget(target));
    }
    target
        .capabilities()
        .map_err(|_| Gfx950LoweringError::UnsupportedCapability("target capabilities"))
}
