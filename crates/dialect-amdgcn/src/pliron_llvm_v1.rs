//! Typed admission contract for the bounded AMDGPU-to-Pliron-LLVM lane.

use core::{error::Error, fmt};

use fe2o3_llvm_handoff::{
    BinaryOperationV2, CallingConventionV2, CastOperationV2, FloatBinaryOperationV2,
    FunctionAttributeV2, FunctionKindV2, Gfx942HandoffV2, Gfx942TargetPolicyV1, InstructionKindV2,
    IntegerBinaryOperationV2, IntrinsicV2, ModuleFlagV1, NamedMetadataV1, ObligationKindV1,
    OriginKindV1, ReturnTypeV2, ScalarTypeV1, TerminatorV2, ValueTypeV2,
};

/// The two closed source profiles recognized by the first typed lowering lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdgcnPlironLlvmProfileV1 {
    /// Straight-line scalar memory and arithmetic.
    ScalarMemoryArithmetic,
    /// Scalar GEMM-style control flow with block-argument phi values.
    ScalarControlFlowGemm,
}

/// A typed class of global rejected before Pliron construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedGlobalV1 {
    /// A scalar global definition or declaration.
    Scalar,
    /// An LDS array used by tiled GEMM.
    LdsArray,
    /// A retained private constant byte array.
    PrivateConstantBytes,
}

/// A typed instruction class rejected by the bounded V1 lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedInstructionV1 {
    /// The address of a global value.
    GlobalAddress,
    /// A zero-initialized fixed vector.
    VectorZero,
    /// A four-lane vector load.
    VectorLoad4,
    /// A direct function or intrinsic call.
    Call,
    /// A fixed-vector insertion.
    InsertElement,
    /// A fixed-vector extraction.
    ExtractElement,
}

/// Stable, typed rejection categories for the bounded V1 lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdgcnPlironLlvmRejectionV1 {
    /// The target-machine policy is not the exact canonical gfx942 policy.
    UnsupportedTarget,
    /// A module flag has no reviewed V1 preservation rule.
    UnsupportedModuleFlag(ModuleFlagV1),
    /// Named metadata has no reviewed V1 preservation rule.
    UnsupportedNamedMetadata(NamedMetadataV1),
    /// Device-library inputs are outside this no-linking milestone.
    UnsupportedDeviceLibrary,
    /// An origin kind has no reviewed V1 preservation rule.
    UnsupportedOrigin(OriginKindV1),
    /// Source spans are not admitted by this backend-only milestone.
    UnsupportedOriginSpan,
    /// A required preservation obligation is absent.
    MissingObligation(ObligationKindV1),
    /// A global requires a future typed extension.
    UnsupportedGlobal(UnsupportedGlobalV1),
    /// An intrinsic requires a future typed extension.
    UnsupportedIntrinsic(IntrinsicV2),
    /// Helpers are not admitted by the one-kernel V1 lane.
    UnsupportedFunctionKind(FunctionKindV2),
    /// The calling convention has no reviewed V1 lowering.
    UnsupportedCallingConvention(CallingConventionV2),
    /// A return type has no reviewed V1 lowering.
    UnsupportedReturnType(ReturnTypeV2),
    /// A function attribute has no reviewed V1 preservation rule.
    UnsupportedFunctionAttribute(FunctionAttributeV2),
    /// A value type has no reviewed V1 Pliron LLVM representation.
    UnsupportedValueType(ValueTypeV2),
    /// An instruction has no reviewed V1 lowering.
    UnsupportedInstruction(UnsupportedInstructionV1),
    /// A terminator has no reviewed V1 lowering.
    UnsupportedTerminator,
}

impl fmt::Display for AmdgcnPlironLlvmRejectionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "typed AMDGPU-to-Pliron-LLVM V1 rejection: {self:?}"
        )
    }
}

impl Error for AmdgcnPlironLlvmRejectionV1 {}

/// A validated, borrowed AMDGPU module admitted for typed Pliron LLVM lowering.
///
/// Construction is intentionally available only through
/// [`admit_amdgcn_pliron_llvm_v1`]. The wrapped handoff remains the authority
/// for target policy, ABI, metadata, origin, and preservation obligations.
#[derive(Clone, Copy, Debug)]
pub struct AdmittedAmdgcnPlironLlvmV1<'a> {
    handoff: &'a Gfx942HandoffV2,
    profile: AmdgcnPlironLlvmProfileV1,
}

impl<'a> AdmittedAmdgcnPlironLlvmV1<'a> {
    /// Returns the exact validated typed source handoff.
    pub const fn handoff(self) -> &'a Gfx942HandoffV2 {
        self.handoff
    }

    /// Returns the closed profile derived from the admitted typed graph.
    pub const fn profile(self) -> AmdgcnPlironLlvmProfileV1 {
        self.profile
    }
}

/// Validates the complete bounded AMDGPU-to-Pliron-LLVM V1 support matrix.
///
/// Validation walks the canonical ordering already enforced by
/// `Gfx942HandoffV2`, so the selected rejection is independent of caller
/// allocation and pre-construction collection order.
pub fn admit_amdgcn_pliron_llvm_v1(
    handoff: &Gfx942HandoffV2,
) -> Result<AdmittedAmdgcnPlironLlvmV1<'_>, AmdgcnPlironLlvmRejectionV1> {
    validate_policy(handoff)?;

    let mut control_flow_gemm = false;
    for function in handoff.module().functions() {
        if function.calling_convention() != CallingConventionV2::AmdGpuKernel {
            return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedCallingConvention(
                function.calling_convention(),
            ));
        }
        if function.kind() != FunctionKindV2::Kernel {
            return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedFunctionKind(
                function.kind(),
            ));
        }
        if function.return_type() != ReturnTypeV2::Void {
            return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedReturnType(
                function.return_type(),
            ));
        }
        for parameter in function.parameters() {
            validate_value_type(parameter.value().value_type())?;
        }
        for attribute in function.attributes() {
            validate_function_attribute(*attribute)?;
        }
        control_flow_gemm |= function.blocks().len() > 1;
        for block in function.blocks() {
            for instruction in block.instructions() {
                if let Some(result) = instruction.result() {
                    validate_value_type(result.value_type())?;
                }
                validate_instruction(instruction.kind())?;
                control_flow_gemm |= matches!(instruction.kind(), InstructionKindV2::Phi { .. });
                control_flow_gemm |= matches!(
                    instruction.kind(),
                    InstructionKindV2::Binary {
                        operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Multiply),
                        ..
                    }
                );
            }
            validate_terminator(block.terminator())?;
        }
    }

    Ok(AdmittedAmdgcnPlironLlvmV1 {
        handoff,
        profile: if control_flow_gemm {
            AmdgcnPlironLlvmProfileV1::ScalarControlFlowGemm
        } else {
            AmdgcnPlironLlvmProfileV1::ScalarMemoryArithmetic
        },
    })
}

fn validate_policy(handoff: &Gfx942HandoffV2) -> Result<(), AmdgcnPlironLlvmRejectionV1> {
    let base = handoff.base();
    if base.target() != &Gfx942TargetPolicyV1::canonical() {
        return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedTarget);
    }
    for flag in base.module().flags() {
        if !matches!(
            flag,
            ModuleFlagV1::CodeObjectVersion6 | ModuleFlagV1::PicLevel2 | ModuleFlagV1::WcharSize4
        ) {
            return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedModuleFlag(*flag));
        }
    }
    for metadata in base.module().named_metadata() {
        if !matches!(
            metadata,
            NamedMetadataV1::OpenClVersion2_0 | NamedMetadataV1::ProducerIdentity(_)
        ) {
            return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedNamedMetadata(
                *metadata,
            ));
        }
    }
    if !base.module().device_libraries().is_empty() {
        return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedDeviceLibrary);
    }
    for origin in base.origins() {
        if !matches!(
            origin.kind(),
            OriginKindV1::KernelIr | OriginKindV1::AmdgcnIr
        ) {
            return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedOrigin(
                origin.kind(),
            ));
        }
        if origin.span().is_some() {
            return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedOriginSpan);
        }
    }
    for required in [
        ObligationKindV1::PreserveKernelAbi,
        ObligationKindV1::PreserveAddressSpaces,
        ObligationKindV1::PreserveTargetFeatures,
        ObligationKindV1::PreserveCallingConvention,
        ObligationKindV1::PreserveFunctionAttributes,
        ObligationKindV1::PreserveModuleMetadata,
        ObligationKindV1::MaintainOriginCoverage,
    ] {
        if !base
            .obligations()
            .iter()
            .any(|obligation| obligation.kind() == required)
        {
            return Err(AmdgcnPlironLlvmRejectionV1::MissingObligation(required));
        }
    }
    if let Some(global) = handoff.module().globals().first() {
        let kind = if global.byte_initializer().is_some() {
            UnsupportedGlobalV1::PrivateConstantBytes
        } else if global.array_elements().is_some() {
            UnsupportedGlobalV1::LdsArray
        } else {
            UnsupportedGlobalV1::Scalar
        };
        return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedGlobal(kind));
    }
    if let Some(intrinsic) = handoff.module().intrinsics().first() {
        return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedIntrinsic(
            intrinsic.intrinsic(),
        ));
    }
    Ok(())
}

fn validate_function_attribute(
    attribute: FunctionAttributeV2,
) -> Result<(), AmdgcnPlironLlvmRejectionV1> {
    match attribute {
        FunctionAttributeV2::NoUnwind
        | FunctionAttributeV2::FlatWorkgroupSize(_)
        | FunctionAttributeV2::WavesPerEu(_)
        | FunctionAttributeV2::DenormalFpMathF32Ieee
        | FunctionAttributeV2::UnsafeFpMathDisabled
        | FunctionAttributeV2::NoInfsFpMathDisabled
        | FunctionAttributeV2::NoNansFpMathDisabled
        | FunctionAttributeV2::NoSignedZerosFpMathDisabled
        | FunctionAttributeV2::ApproxFuncFpMathDisabled
        | FunctionAttributeV2::FpContractOff
        | FunctionAttributeV2::RequiredWorkgroupSize(_) => Ok(()),
        FunctionAttributeV2::AlwaysInline
        | FunctionAttributeV2::NoInline
        | FunctionAttributeV2::ReadNone
        | FunctionAttributeV2::WillReturn => Err(
            AmdgcnPlironLlvmRejectionV1::UnsupportedFunctionAttribute(attribute),
        ),
    }
}

fn validate_value_type(value_type: ValueTypeV2) -> Result<(), AmdgcnPlironLlvmRejectionV1> {
    let scalar_supported = |scalar| {
        matches!(
            scalar,
            ScalarTypeV1::I1
                | ScalarTypeV1::I8
                | ScalarTypeV1::I16
                | ScalarTypeV1::I32
                | ScalarTypeV1::I64
                | ScalarTypeV1::F32
        )
    };
    let supported = match value_type {
        ValueTypeV2::Scalar(scalar) => scalar_supported(scalar),
        ValueTypeV2::Pointer { pointee, .. } => scalar_supported(pointee),
        ValueTypeV2::Vector { .. } | ValueTypeV2::ArrayPointer { .. } => false,
    };
    if supported {
        Ok(())
    } else {
        Err(AmdgcnPlironLlvmRejectionV1::UnsupportedValueType(
            value_type,
        ))
    }
}

fn validate_instruction(
    instruction: &InstructionKindV2,
) -> Result<(), AmdgcnPlironLlvmRejectionV1> {
    match instruction {
        InstructionKindV2::Constant(value) => validate_value_type(value.value_type()),
        InstructionKindV2::Binary { operation, .. } => {
            match operation {
                BinaryOperationV2::Integer(
                    IntegerBinaryOperationV2::Add
                    | IntegerBinaryOperationV2::Subtract
                    | IntegerBinaryOperationV2::Multiply
                    | IntegerBinaryOperationV2::And
                    | IntegerBinaryOperationV2::Or
                    | IntegerBinaryOperationV2::Xor
                    | IntegerBinaryOperationV2::ShiftLeft
                    | IntegerBinaryOperationV2::LogicalShiftRight
                    | IntegerBinaryOperationV2::ArithmeticShiftRight,
                )
                | BinaryOperationV2::Float(
                    FloatBinaryOperationV2::Add
                    | FloatBinaryOperationV2::Subtract
                    | FloatBinaryOperationV2::Multiply
                    | FloatBinaryOperationV2::Divide,
                ) => {}
            }
            Ok(())
        }
        InstructionKindV2::Compare { .. }
        | InstructionKindV2::Cast {
            operation:
                CastOperationV2::ZeroExtend
                | CastOperationV2::SignExtend
                | CastOperationV2::Truncate
                | CastOperationV2::FloatExtend
                | CastOperationV2::FloatTruncate
                | CastOperationV2::UnsignedIntToFloat
                | CastOperationV2::SignedIntToFloat
                | CastOperationV2::FloatToUnsignedInt
                | CastOperationV2::FloatToSignedInt
                | CastOperationV2::PointerToInt,
            ..
        }
        | InstructionKindV2::GetElementPtr { .. }
        | InstructionKindV2::Load { .. }
        | InstructionKindV2::Store { .. }
        | InstructionKindV2::Phi { .. } => Ok(()),
        InstructionKindV2::GlobalAddress(_) => {
            Err(AmdgcnPlironLlvmRejectionV1::UnsupportedInstruction(
                UnsupportedInstructionV1::GlobalAddress,
            ))
        }
        InstructionKindV2::VectorZero { .. } => {
            Err(AmdgcnPlironLlvmRejectionV1::UnsupportedInstruction(
                UnsupportedInstructionV1::VectorZero,
            ))
        }
        InstructionKindV2::VectorLoad4 { .. } => {
            Err(AmdgcnPlironLlvmRejectionV1::UnsupportedInstruction(
                UnsupportedInstructionV1::VectorLoad4,
            ))
        }
        InstructionKindV2::Call { .. } => Err(AmdgcnPlironLlvmRejectionV1::UnsupportedInstruction(
            UnsupportedInstructionV1::Call,
        )),
        InstructionKindV2::InsertElement { .. } => {
            Err(AmdgcnPlironLlvmRejectionV1::UnsupportedInstruction(
                UnsupportedInstructionV1::InsertElement,
            ))
        }
        InstructionKindV2::ExtractElement { .. } => {
            Err(AmdgcnPlironLlvmRejectionV1::UnsupportedInstruction(
                UnsupportedInstructionV1::ExtractElement,
            ))
        }
    }
}

fn validate_terminator(terminator: &TerminatorV2) -> Result<(), AmdgcnPlironLlvmRejectionV1> {
    match terminator {
        TerminatorV2::Return(None)
        | TerminatorV2::Branch(_)
        | TerminatorV2::ConditionalBranch { .. }
        | TerminatorV2::Unreachable => Ok(()),
        TerminatorV2::Return(Some(_)) => Err(AmdgcnPlironLlvmRejectionV1::UnsupportedTerminator),
    }
}
