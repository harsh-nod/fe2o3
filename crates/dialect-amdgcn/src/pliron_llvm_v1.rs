//! Typed admission contract for the bounded AMDGPU-to-Pliron-LLVM lane.

use core::{error::Error, fmt};

use fe2o3_llvm_handoff::{
    AddressSpaceV1, BinaryOperationV2, CallTargetV2, CallingConventionV2, CastOperationV2,
    FloatBinaryOperationV2, FunctionAttributeV2, FunctionKindV2, Gfx942HandoffV2,
    Gfx942TargetPolicyV1, GlobalLinkageV2, GlobalV2, InstructionKindV2, IntegerBinaryOperationV2,
    IntrinsicV2, ModuleFlagV1, NamedMetadataV1, ObligationKindV1, OriginKindV1, ReturnTypeV2,
    ScalarTypeV1, TerminatorV2, ValueTypeV2,
};

/// The closed source profiles recognized by the first typed lowering lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdgcnPlironLlvmProfileV1 {
    /// Straight-line scalar memory and arithmetic.
    ScalarMemoryArithmetic,
    /// Scalar GEMM-style control flow with block-argument phi values.
    ScalarControlFlowGemm,
    /// Global arrays and fixed vectors used by tiled GEMM data movement.
    TiledDataRepresentationGemm,
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
    /// A direct function call.
    Call,
    /// A fixed-vector insertion.
    InsertElement,
    /// A fixed-vector extraction.
    ExtractElement,
    /// A GEP shape outside the one-index scalar-pointer V1 rule.
    GetElementPtrShape,
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
    let mut tiled_data_representation = !handoff.module().globals().is_empty();
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
                tiled_data_representation |= matches!(
                    instruction.kind(),
                    InstructionKindV2::GlobalAddress(_)
                        | InstructionKindV2::VectorZero { .. }
                        | InstructionKindV2::VectorLoad4 { .. }
                        | InstructionKindV2::InsertElement { .. }
                        | InstructionKindV2::ExtractElement { .. }
                        | InstructionKindV2::Call {
                            target: CallTargetV2::Intrinsic(
                                IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k
                            ),
                            ..
                        }
                );
                control_flow_gemm |= matches!(instruction.kind(), InstructionKindV2::Phi { .. });
                control_flow_gemm |= matches!(
                    instruction.kind(),
                    InstructionKindV2::Binary {
                        operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Multiply),
                        ..
                    } | InstructionKindV2::Call {
                        target: CallTargetV2::Intrinsic(IntrinsicV2::FmaF32),
                        ..
                    }
                );
            }
            validate_terminator(block.terminator())?;
        }
    }

    Ok(AdmittedAmdgcnPlironLlvmV1 {
        handoff,
        profile: if tiled_data_representation {
            AmdgcnPlironLlvmProfileV1::TiledDataRepresentationGemm
        } else if control_flow_gemm {
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
    for global in handoff.module().globals() {
        validate_global(global)?;
    }
    for reference in handoff.module().intrinsics() {
        validate_intrinsic(reference.intrinsic())?;
    }
    Ok(())
}

fn validate_intrinsic(intrinsic: IntrinsicV2) -> Result<(), AmdgcnPlironLlvmRejectionV1> {
    let (return_type, parameters) = intrinsic.signature();
    if let ReturnTypeV2::Value(value_type) = return_type {
        validate_value_type(value_type)?;
    }
    for parameter in parameters {
        validate_value_type(parameter)?;
    }

    match intrinsic {
        IntrinsicV2::AmdGpuWorkitemId(_)
        | IntrinsicV2::AmdGpuWorkgroupId(_)
        | IntrinsicV2::AmdGpuBarrier
        | IntrinsicV2::FmaF32
        | IntrinsicV2::SqrtF32
        | IntrinsicV2::Trap
        | IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k => Ok(()),
    }
}

fn validate_global(global: &GlobalV2) -> Result<(), AmdgcnPlironLlvmRejectionV1> {
    if !is_supported_scalar(global.value_type()) {
        return Err(AmdgcnPlironLlvmRejectionV1::UnsupportedValueType(
            ValueTypeV2::Scalar(global.value_type()),
        ));
    }
    match (global.array_elements(), global.byte_initializer()) {
        (Some(elements), Some(bytes))
            if global.linkage() == GlobalLinkageV2::Internal
                && global.address_space() == AddressSpaceV1::Constant
                && !global.is_mutable()
                && global.value_type() == ScalarTypeV1::I8
                && usize::from(elements) == bytes.len() =>
        {
            Ok(())
        }
        (Some(256), None)
            if global.linkage() == GlobalLinkageV2::Internal
                && global.address_space() == AddressSpaceV1::Local
                && global.is_mutable()
                && global.value_type() == ScalarTypeV1::I16 =>
        {
            Ok(())
        }
        (Some(_), Some(_)) => Err(AmdgcnPlironLlvmRejectionV1::UnsupportedGlobal(
            UnsupportedGlobalV1::PrivateConstantBytes,
        )),
        (Some(_), None) => Err(AmdgcnPlironLlvmRejectionV1::UnsupportedGlobal(
            UnsupportedGlobalV1::LdsArray,
        )),
        (None, _) => Err(AmdgcnPlironLlvmRejectionV1::UnsupportedGlobal(
            UnsupportedGlobalV1::Scalar,
        )),
    }
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
    let supported = match value_type {
        ValueTypeV2::Scalar(scalar)
        | ValueTypeV2::Pointer {
            pointee: scalar, ..
        } => is_supported_scalar(scalar),
        ValueTypeV2::Vector { element, lanes } => lanes == 4 && is_supported_scalar(element),
        ValueTypeV2::ArrayPointer {
            element, elements, ..
        } => elements > 0 && is_supported_scalar(element),
    };
    if supported {
        Ok(())
    } else {
        Err(AmdgcnPlironLlvmRejectionV1::UnsupportedValueType(
            value_type,
        ))
    }
}

const fn is_supported_scalar(scalar: ScalarTypeV1) -> bool {
    matches!(
        scalar,
        ScalarTypeV1::I1
            | ScalarTypeV1::I8
            | ScalarTypeV1::I16
            | ScalarTypeV1::I32
            | ScalarTypeV1::I64
            | ScalarTypeV1::F32
    )
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
        | InstructionKindV2::GlobalAddress(_)
        | InstructionKindV2::VectorZero { .. }
        | InstructionKindV2::Load { .. }
        | InstructionKindV2::VectorLoad4 { .. }
        | InstructionKindV2::Store { .. }
        | InstructionKindV2::Phi { .. }
        | InstructionKindV2::InsertElement { .. }
        | InstructionKindV2::ExtractElement { .. } => Ok(()),
        InstructionKindV2::GetElementPtr { indices, .. } if matches!(indices.len(), 1 | 2) => {
            Ok(())
        }
        InstructionKindV2::GetElementPtr { .. } => {
            Err(AmdgcnPlironLlvmRejectionV1::UnsupportedInstruction(
                UnsupportedInstructionV1::GetElementPtrShape,
            ))
        }
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(intrinsic),
            ..
        } => validate_intrinsic(*intrinsic),
        InstructionKindV2::Call {
            target: CallTargetV2::Function(_),
            ..
        } => Err(AmdgcnPlironLlvmRejectionV1::UnsupportedInstruction(
            UnsupportedInstructionV1::Call,
        )),
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
