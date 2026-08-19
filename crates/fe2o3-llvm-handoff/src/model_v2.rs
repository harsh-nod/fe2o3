use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::fmt;

use sha2::{Digest as _, Sha256};

use crate::{
    AddressSpaceV1, DefinitionKindV2, FunctionAttributeV1, Gfx942HandoffV1, HandoffDiagnosticV2,
    HandoffLimitV2, ModuleFlagV1, NamedMetadataV1, ObligationIdentityV1, OriginIdentityV1,
    ParameterAttributeV1, ScalarTypeV1,
};

pub const MAX_CANONICAL_HANDOFF_BYTES_V2: usize = 4 * 1024 * 1024;
pub const MAX_SYMBOL_BYTES_V2: usize = 128;
pub const MAX_GLOBALS_V2: usize = 256;
pub const MAX_INTRINSICS_V2: usize = 64;
pub const MAX_FUNCTIONS_V2: usize = 256;
pub const MAX_FUNCTION_PARAMETERS_V2: usize = 128;
pub const MAX_PARAMETER_ATTRIBUTES_V2: usize = 16;
pub const MAX_FUNCTION_ATTRIBUTES_V2: usize = 24;
pub const MAX_FUNCTION_BLOCKS_V2: usize = 1_024;
pub const MAX_INSTRUCTIONS_PER_FUNCTION_V2: usize = 65_536;
pub const MAX_VALUES_PER_FUNCTION_V2: usize = 65_536;
pub const MAX_GEP_INDICES_V2: usize = 8;
pub const MAX_EVIDENCE_OBLIGATIONS_V2: usize = 64;
pub const MAX_MODULE_FLAGS_V2: usize = 8;
pub const MAX_NAMED_METADATA_V2: usize = 8;

const MODULE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.llvm-handoff.module.identity.v2";
const HANDOFF_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.llvm-handoff.identity.v2";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GlobalIdV2(u32);

impl GlobalIdV2 {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionIdV2(u32);

impl FunctionIdV2 {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockIdV2(u32);

impl BlockIdV2 {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueIdV2(u32);

impl ValueIdV2 {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueTypeV2 {
    Scalar(ScalarTypeV1),
    Pointer {
        pointee: ScalarTypeV1,
        address_space: AddressSpaceV1,
    },
}

impl ValueTypeV2 {
    pub const fn is_pointer(self) -> bool {
        matches!(self, Self::Pointer { .. })
    }

    pub const fn is_integer(self) -> bool {
        matches!(
            self,
            Self::Scalar(
                ScalarTypeV1::I1
                    | ScalarTypeV1::I8
                    | ScalarTypeV1::I16
                    | ScalarTypeV1::I32
                    | ScalarTypeV1::I64
            )
        )
    }

    pub const fn is_float(self) -> bool {
        matches!(
            self,
            Self::Scalar(
                ScalarTypeV1::F16 | ScalarTypeV1::Bf16 | ScalarTypeV1::F32 | ScalarTypeV1::F64
            )
        )
    }
}

impl From<crate::KernelValueTypeV1> for ValueTypeV2 {
    fn from(value: crate::KernelValueTypeV1) -> Self {
        match value {
            crate::KernelValueTypeV1::Scalar(scalar) => Self::Scalar(scalar),
            crate::KernelValueTypeV1::Pointer {
                pointee,
                address_space,
            } => Self::Pointer {
                pointee,
                address_space,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReturnTypeV2 {
    Void,
    Value(ValueTypeV2),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarConstantV2 {
    pub(crate) scalar_type: ScalarTypeV1,
    pub(crate) bits: u64,
}

impl ScalarConstantV2 {
    pub fn new(scalar_type: ScalarTypeV1, bits: u64) -> Result<Self, HandoffDiagnosticV2> {
        let valid = match scalar_type {
            ScalarTypeV1::I1 => bits <= 1,
            ScalarTypeV1::I8 => bits <= u8::MAX.into(),
            ScalarTypeV1::I16 | ScalarTypeV1::F16 | ScalarTypeV1::Bf16 => bits <= u16::MAX.into(),
            ScalarTypeV1::I32 | ScalarTypeV1::F32 => bits <= u32::MAX.into(),
            ScalarTypeV1::I64 | ScalarTypeV1::F64 => true,
        };
        if !valid {
            return Err(HandoffDiagnosticV2::InvalidScalarConstant);
        }
        Ok(Self { scalar_type, bits })
    }

    pub const fn scalar_type(self) -> ScalarTypeV1 {
        self.scalar_type
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn value_type(self) -> ValueTypeV2 {
        ValueTypeV2::Scalar(self.scalar_type)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceV2 {
    pub(crate) origin: OriginIdentityV1,
    pub(crate) obligations: Vec<ObligationIdentityV1>,
}

impl EvidenceV2 {
    pub fn new(
        origin: OriginIdentityV1,
        mut obligations: Vec<ObligationIdentityV1>,
    ) -> Result<Self, HandoffDiagnosticV2> {
        check_count(
            HandoffLimitV2::EvidenceObligations,
            obligations.len(),
            MAX_EVIDENCE_OBLIGATIONS_V2,
        )?;
        obligations.sort_unstable();
        if obligations.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::Obligation,
            ));
        }
        Ok(Self {
            origin,
            obligations,
        })
    }

    pub const fn origin(&self) -> OriginIdentityV1 {
        self.origin
    }

    pub fn obligations(&self) -> &[ObligationIdentityV1] {
        &self.obligations
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GlobalLinkageV2 {
    Internal,
    External,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalV2 {
    pub(crate) id: GlobalIdV2,
    pub(crate) symbol: String,
    pub(crate) linkage: GlobalLinkageV2,
    pub(crate) address_space: AddressSpaceV1,
    pub(crate) mutable: bool,
    pub(crate) value_type: ScalarTypeV1,
    pub(crate) initializer: Option<ScalarConstantV2>,
    pub(crate) evidence: EvidenceV2,
}

impl GlobalV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: GlobalIdV2,
        symbol: &str,
        linkage: GlobalLinkageV2,
        address_space: AddressSpaceV1,
        mutable: bool,
        value_type: ScalarTypeV1,
        initializer: Option<ScalarConstantV2>,
        evidence: EvidenceV2,
    ) -> Result<Self, HandoffDiagnosticV2> {
        validate_symbol(symbol)?;
        if initializer.is_some_and(|value| value.scalar_type != value_type)
            || matches!(linkage, GlobalLinkageV2::Internal) != initializer.is_some()
        {
            return Err(HandoffDiagnosticV2::InvalidScalarConstant);
        }
        Ok(Self {
            id,
            symbol: symbol.to_string(),
            linkage,
            address_space,
            mutable,
            value_type,
            initializer,
            evidence,
        })
    }

    pub const fn id(&self) -> GlobalIdV2 {
        self.id
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn linkage(&self) -> GlobalLinkageV2 {
        self.linkage
    }

    pub const fn address_space(&self) -> AddressSpaceV1 {
        self.address_space
    }

    pub const fn is_mutable(&self) -> bool {
        self.mutable
    }

    pub const fn value_type(&self) -> ScalarTypeV1 {
        self.value_type
    }

    pub const fn initializer(&self) -> Option<ScalarConstantV2> {
        self.initializer
    }

    pub const fn evidence(&self) -> &EvidenceV2 {
        &self.evidence
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AxisV2 {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntrinsicV2 {
    AmdGpuWorkitemId(AxisV2),
    AmdGpuWorkgroupId(AxisV2),
    AmdGpuBarrier,
    FmaF32,
    SqrtF32,
}

impl IntrinsicV2 {
    pub fn signature(self) -> (ReturnTypeV2, Vec<ValueTypeV2>) {
        let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
        match self {
            Self::AmdGpuWorkitemId(_) | Self::AmdGpuWorkgroupId(_) => (
                ReturnTypeV2::Value(ValueTypeV2::Scalar(ScalarTypeV1::I32)),
                vec![],
            ),
            Self::AmdGpuBarrier => (ReturnTypeV2::Void, vec![]),
            Self::FmaF32 => (
                ReturnTypeV2::Value(f32_type),
                vec![f32_type, f32_type, f32_type],
            ),
            Self::SqrtF32 => (ReturnTypeV2::Value(f32_type), vec![f32_type]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntrinsicReferenceV2 {
    pub(crate) intrinsic: IntrinsicV2,
    pub(crate) evidence: EvidenceV2,
}

impl IntrinsicReferenceV2 {
    pub const fn new(intrinsic: IntrinsicV2, evidence: EvidenceV2) -> Self {
        Self {
            intrinsic,
            evidence,
        }
    }

    pub const fn intrinsic(&self) -> IntrinsicV2 {
        self.intrinsic
    }

    pub const fn evidence(&self) -> &EvidenceV2 {
        &self.evidence
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypedValueV2 {
    pub(crate) id: ValueIdV2,
    pub(crate) value_type: ValueTypeV2,
}

impl TypedValueV2 {
    pub const fn new(id: ValueIdV2, value_type: ValueTypeV2) -> Self {
        Self { id, value_type }
    }

    pub const fn id(self) -> ValueIdV2 {
        self.id
    }

    pub const fn value_type(self) -> ValueTypeV2 {
        self.value_type
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionParameterV2 {
    pub(crate) value: TypedValueV2,
    pub(crate) name: String,
    pub(crate) attributes: Vec<ParameterAttributeV1>,
}

impl FunctionParameterV2 {
    pub fn new(
        value: TypedValueV2,
        name: &str,
        mut attributes: Vec<ParameterAttributeV1>,
    ) -> Result<Self, HandoffDiagnosticV2> {
        validate_symbol(name)?;
        check_count(
            HandoffLimitV2::ParameterAttributes,
            attributes.len(),
            MAX_PARAMETER_ATTRIBUTES_V2,
        )?;
        attributes.sort_unstable();
        if attributes
            .windows(2)
            .any(|pair| pair[0].kind() == pair[1].kind())
        {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::Parameter,
            ));
        }
        if attributes.contains(&ParameterAttributeV1::ReadOnly)
            && attributes.contains(&ParameterAttributeV1::WriteOnly)
        {
            return Err(HandoffDiagnosticV2::ConflictingParameterAttributes);
        }
        for attribute in &attributes {
            if !value.value_type.is_pointer() {
                return Err(HandoffDiagnosticV2::AttributeRequiresPointer);
            }
            match attribute {
                ParameterAttributeV1::Align(value)
                    if *value == 0 || !value.is_power_of_two() || *value > 256 =>
                {
                    return Err(HandoffDiagnosticV2::InvalidParameterAttribute);
                }
                ParameterAttributeV1::Dereferenceable(0) => {
                    return Err(HandoffDiagnosticV2::InvalidParameterAttribute);
                }
                _ => {}
            }
        }
        Ok(Self {
            value,
            name: name.to_string(),
            attributes,
        })
    }

    pub const fn value(&self) -> TypedValueV2 {
        self.value
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn attributes(&self) -> &[ParameterAttributeV1] {
        &self.attributes
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionKindV2 {
    Kernel,
    Helper,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallingConventionV2 {
    C,
    AmdGpuKernel,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionAttributeV2 {
    NoUnwind,
    AlwaysInline,
    NoInline,
    ReadNone,
    WillReturn,
    FlatWorkgroupSize(crate::WorkgroupSizeRangeV1),
    WavesPerEu(crate::WavesPerEuV1),
    DenormalFpMathF32Ieee,
    UnsafeFpMathDisabled,
    NoInfsFpMathDisabled,
    NoNansFpMathDisabled,
    NoSignedZerosFpMathDisabled,
    ApproxFuncFpMathDisabled,
    FpContractOff,
}

impl FunctionAttributeV2 {
    pub(crate) const fn kind(self) -> u8 {
        match self {
            Self::NoUnwind => 1,
            Self::AlwaysInline => 2,
            Self::NoInline => 3,
            Self::ReadNone => 4,
            Self::WillReturn => 5,
            Self::FlatWorkgroupSize(_) => 6,
            Self::WavesPerEu(_) => 7,
            Self::DenormalFpMathF32Ieee => 8,
            Self::UnsafeFpMathDisabled => 9,
            Self::NoInfsFpMathDisabled => 10,
            Self::NoNansFpMathDisabled => 11,
            Self::NoSignedZerosFpMathDisabled => 12,
            Self::ApproxFuncFpMathDisabled => 13,
            Self::FpContractOff => 14,
        }
    }
}

impl From<FunctionAttributeV1> for FunctionAttributeV2 {
    fn from(value: FunctionAttributeV1) -> Self {
        match value {
            FunctionAttributeV1::NoUnwind => Self::NoUnwind,
            FunctionAttributeV1::FlatWorkgroupSize(range) => Self::FlatWorkgroupSize(range),
            FunctionAttributeV1::WavesPerEu(range) => Self::WavesPerEu(range),
            FunctionAttributeV1::DenormalFpMathF32Ieee => Self::DenormalFpMathF32Ieee,
            FunctionAttributeV1::UnsafeFpMathDisabled => Self::UnsafeFpMathDisabled,
            FunctionAttributeV1::NoInfsFpMathDisabled => Self::NoInfsFpMathDisabled,
            FunctionAttributeV1::NoNansFpMathDisabled => Self::NoNansFpMathDisabled,
            FunctionAttributeV1::NoSignedZerosFpMathDisabled => Self::NoSignedZerosFpMathDisabled,
            FunctionAttributeV1::ApproxFuncFpMathDisabled => Self::ApproxFuncFpMathDisabled,
            FunctionAttributeV1::FpContractOff => Self::FpContractOff,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntegerBinaryOperationV2 {
    Add,
    Subtract,
    Multiply,
    And,
    Or,
    Xor,
    ShiftLeft,
    LogicalShiftRight,
    ArithmeticShiftRight,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatBinaryOperationV2 {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BinaryOperationV2 {
    Integer(IntegerBinaryOperationV2),
    Float(FloatBinaryOperationV2),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ComparePredicateV2 {
    IntegerEqual,
    IntegerNotEqual,
    UnsignedLessThan,
    UnsignedLessOrEqual,
    SignedLessThan,
    SignedLessOrEqual,
    OrderedEqual,
    OrderedNotEqual,
    OrderedLessThan,
    OrderedLessOrEqual,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CastOperationV2 {
    ZeroExtend,
    SignExtend,
    Truncate,
    FloatExtend,
    FloatTruncate,
    UnsignedIntToFloat,
    SignedIntToFloat,
    FloatToUnsignedInt,
    FloatToSignedInt,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallTargetV2 {
    Function(FunctionIdV2),
    Intrinsic(IntrinsicV2),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionKindV2 {
    Constant(ScalarConstantV2),
    GlobalAddress(GlobalIdV2),
    Binary {
        operation: BinaryOperationV2,
        left: ValueIdV2,
        right: ValueIdV2,
    },
    Compare {
        predicate: ComparePredicateV2,
        left: ValueIdV2,
        right: ValueIdV2,
    },
    Cast {
        operation: CastOperationV2,
        value: ValueIdV2,
        to: ValueTypeV2,
    },
    GetElementPtr {
        base: ValueIdV2,
        indices: Vec<ValueIdV2>,
    },
    Load {
        pointer: ValueIdV2,
        value_type: ScalarTypeV1,
        alignment: u16,
    },
    Store {
        pointer: ValueIdV2,
        value: ValueIdV2,
        value_type: ScalarTypeV1,
        alignment: u16,
    },
    Call {
        target: CallTargetV2,
        arguments: Vec<ValueIdV2>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionV2 {
    pub(crate) result: Option<TypedValueV2>,
    pub(crate) kind: InstructionKindV2,
    pub(crate) evidence: EvidenceV2,
}

impl InstructionV2 {
    pub fn new(
        result: Option<TypedValueV2>,
        kind: InstructionKindV2,
        evidence: EvidenceV2,
    ) -> Result<Self, HandoffDiagnosticV2> {
        match &kind {
            InstructionKindV2::GetElementPtr { indices, .. } => check_nonempty_count(
                "getelementptr indices",
                HandoffLimitV2::GetElementPtrIndices,
                indices.len(),
                MAX_GEP_INDICES_V2,
            )?,
            InstructionKindV2::Load { alignment, .. }
            | InstructionKindV2::Store { alignment, .. }
                if *alignment == 0 || !alignment.is_power_of_two() || *alignment > 256 =>
            {
                return Err(HandoffDiagnosticV2::InvalidAlignment);
            }
            _ => {}
        }
        let result_is_valid = match kind {
            InstructionKindV2::Store { .. } => result.is_none(),
            InstructionKindV2::Call { .. } => true,
            _ => result.is_some(),
        };
        if !result_is_valid {
            return Err(HandoffDiagnosticV2::InvalidInstructionResult);
        }
        Ok(Self {
            result,
            kind,
            evidence,
        })
    }

    pub const fn result(&self) -> Option<TypedValueV2> {
        self.result
    }

    pub const fn kind(&self) -> &InstructionKindV2 {
        &self.kind
    }

    pub const fn evidence(&self) -> &EvidenceV2 {
        &self.evidence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminatorV2 {
    Return(Option<ValueIdV2>),
    Branch(BlockIdV2),
    ConditionalBranch {
        condition: ValueIdV2,
        then_block: BlockIdV2,
        else_block: BlockIdV2,
    },
    Unreachable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicBlockV2 {
    pub(crate) id: BlockIdV2,
    pub(crate) instructions: Vec<InstructionV2>,
    pub(crate) terminator: TerminatorV2,
}

impl BasicBlockV2 {
    pub fn new(id: BlockIdV2, instructions: Vec<InstructionV2>, terminator: TerminatorV2) -> Self {
        Self {
            id,
            instructions,
            terminator,
        }
    }

    pub const fn id(&self) -> BlockIdV2 {
        self.id
    }

    pub fn instructions(&self) -> &[InstructionV2] {
        &self.instructions
    }

    pub const fn terminator(&self) -> &TerminatorV2 {
        &self.terminator
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionV2 {
    pub(crate) id: FunctionIdV2,
    pub(crate) symbol: String,
    pub(crate) kind: FunctionKindV2,
    pub(crate) calling_convention: CallingConventionV2,
    pub(crate) return_type: ReturnTypeV2,
    pub(crate) parameters: Vec<FunctionParameterV2>,
    pub(crate) attributes: Vec<FunctionAttributeV2>,
    pub(crate) entry: BlockIdV2,
    pub(crate) blocks: Vec<BasicBlockV2>,
    pub(crate) evidence: EvidenceV2,
}

impl FunctionV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: FunctionIdV2,
        symbol: &str,
        kind: FunctionKindV2,
        calling_convention: CallingConventionV2,
        return_type: ReturnTypeV2,
        parameters: Vec<FunctionParameterV2>,
        mut attributes: Vec<FunctionAttributeV2>,
        entry: BlockIdV2,
        mut blocks: Vec<BasicBlockV2>,
        evidence: EvidenceV2,
    ) -> Result<Self, HandoffDiagnosticV2> {
        validate_symbol(symbol)?;
        check_count(
            HandoffLimitV2::FunctionParameters,
            parameters.len(),
            MAX_FUNCTION_PARAMETERS_V2,
        )?;
        check_count(
            HandoffLimitV2::FunctionAttributes,
            attributes.len(),
            MAX_FUNCTION_ATTRIBUTES_V2,
        )?;
        check_nonempty_count(
            "function blocks",
            HandoffLimitV2::FunctionBlocks,
            blocks.len(),
            MAX_FUNCTION_BLOCKS_V2,
        )?;
        match (kind, calling_convention, return_type) {
            (FunctionKindV2::Kernel, CallingConventionV2::AmdGpuKernel, ReturnTypeV2::Void)
            | (FunctionKindV2::Helper, CallingConventionV2::C, _) => {}
            _ => return Err(HandoffDiagnosticV2::UnsupportedCallingConvention),
        }
        for (index, parameter) in parameters.iter().enumerate() {
            if parameters[..index]
                .iter()
                .any(|prior| prior.value.id == parameter.value.id || prior.name == parameter.name)
            {
                return Err(HandoffDiagnosticV2::DuplicateDefinition(
                    DefinitionKindV2::Parameter,
                ));
            }
        }
        attributes.sort_unstable();
        if attributes
            .windows(2)
            .any(|pair| pair[0].kind() == pair[1].kind())
        {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::Function,
            ));
        }
        if attributes.contains(&FunctionAttributeV2::AlwaysInline)
            && attributes.contains(&FunctionAttributeV2::NoInline)
        {
            return Err(HandoffDiagnosticV2::ConflictingFunctionAttributes);
        }
        if !attributes.contains(&FunctionAttributeV2::NoUnwind) {
            return Err(HandoffDiagnosticV2::UnsupportedCallingConvention);
        }
        blocks.sort_unstable_by_key(|block| block.id);
        if blocks.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::Block,
            ));
        }
        if !blocks.iter().any(|block| block.id == entry) {
            return Err(HandoffDiagnosticV2::MissingEntryBlock(entry));
        }
        Ok(Self {
            id,
            symbol: symbol.to_string(),
            kind,
            calling_convention,
            return_type,
            parameters,
            attributes,
            entry,
            blocks,
            evidence,
        })
    }

    pub const fn id(&self) -> FunctionIdV2 {
        self.id
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn kind(&self) -> FunctionKindV2 {
        self.kind
    }

    pub const fn calling_convention(&self) -> CallingConventionV2 {
        self.calling_convention
    }

    pub const fn return_type(&self) -> ReturnTypeV2 {
        self.return_type
    }

    pub fn parameters(&self) -> &[FunctionParameterV2] {
        &self.parameters
    }

    pub fn attributes(&self) -> &[FunctionAttributeV2] {
        &self.attributes
    }

    pub const fn entry(&self) -> BlockIdV2 {
        self.entry
    }

    pub fn blocks(&self) -> &[BasicBlockV2] {
        &self.blocks
    }

    pub const fn evidence(&self) -> &EvidenceV2 {
        &self.evidence
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleIdentityV2([u8; 32]);

impl ModuleIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ModuleIdentityV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableModuleV2 {
    pub(crate) flags: Vec<ModuleFlagV1>,
    pub(crate) named_metadata: Vec<NamedMetadataV1>,
    pub(crate) globals: Vec<GlobalV2>,
    pub(crate) intrinsics: Vec<IntrinsicReferenceV2>,
    pub(crate) functions: Vec<FunctionV2>,
}

impl ExecutableModuleV2 {
    pub fn new(
        mut flags: Vec<ModuleFlagV1>,
        mut named_metadata: Vec<NamedMetadataV1>,
        mut globals: Vec<GlobalV2>,
        mut intrinsics: Vec<IntrinsicReferenceV2>,
        mut functions: Vec<FunctionV2>,
    ) -> Result<Self, HandoffDiagnosticV2> {
        check_count(
            HandoffLimitV2::ModuleFlags,
            flags.len(),
            MAX_MODULE_FLAGS_V2,
        )?;
        check_count(
            HandoffLimitV2::NamedMetadata,
            named_metadata.len(),
            MAX_NAMED_METADATA_V2,
        )?;
        check_count(HandoffLimitV2::Globals, globals.len(), MAX_GLOBALS_V2)?;
        check_count(
            HandoffLimitV2::Intrinsics,
            intrinsics.len(),
            MAX_INTRINSICS_V2,
        )?;
        check_nonempty_count(
            "functions",
            HandoffLimitV2::Functions,
            functions.len(),
            MAX_FUNCTIONS_V2,
        )?;
        flags.sort_unstable();
        if flags.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::ModuleFlag,
            ));
        }
        for required in [ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2] {
            if !flags.contains(&required) {
                return Err(HandoffDiagnosticV2::MetadataMismatch);
            }
        }
        named_metadata.sort_unstable();
        if named_metadata
            .windows(2)
            .any(|pair| pair[0].kind() == pair[1].kind())
        {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::NamedMetadata,
            ));
        }
        globals.sort_unstable_by_key(|global| global.id);
        if globals.windows(2).any(|pair| pair[0].id == pair[1].id)
            || duplicate_symbol(globals.iter().map(|global| global.symbol.as_str()))
        {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::Global,
            ));
        }
        intrinsics.sort_unstable_by_key(|reference| reference.intrinsic);
        if intrinsics
            .windows(2)
            .any(|pair| pair[0].intrinsic == pair[1].intrinsic)
        {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::Intrinsic,
            ));
        }
        functions.sort_unstable_by_key(|function| function.id);
        if functions.windows(2).any(|pair| pair[0].id == pair[1].id)
            || duplicate_symbol(functions.iter().map(|function| function.symbol.as_str()))
        {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::Function,
            ));
        }
        if globals.iter().any(|global| {
            functions
                .iter()
                .any(|function| function.symbol == global.symbol)
        }) {
            return Err(HandoffDiagnosticV2::DuplicateDefinition(
                DefinitionKindV2::Symbol,
            ));
        }

        let module = Self {
            flags,
            named_metadata,
            globals,
            intrinsics,
            functions,
        };
        module.validate_graph()?;
        Ok(module)
    }

    pub fn flags(&self) -> &[ModuleFlagV1] {
        &self.flags
    }

    pub fn named_metadata(&self) -> &[NamedMetadataV1] {
        &self.named_metadata
    }

    pub fn globals(&self) -> &[GlobalV2] {
        &self.globals
    }

    pub fn intrinsics(&self) -> &[IntrinsicReferenceV2] {
        &self.intrinsics
    }

    pub fn functions(&self) -> &[FunctionV2] {
        &self.functions
    }

    pub fn identity(&self) -> ModuleIdentityV2 {
        let payload = crate::codec_v2::encode_module_v2(self);
        ModuleIdentityV2(hash_identity(MODULE_IDENTITY_DOMAIN_V2, &payload))
    }

    fn validate_graph(&self) -> Result<(), HandoffDiagnosticV2> {
        for function in &self.functions {
            let instruction_count = function
                .blocks
                .iter()
                .try_fold(0_usize, |count, block| {
                    count.checked_add(block.instructions.len())
                })
                .ok_or(HandoffDiagnosticV2::LimitExceeded {
                    limit: HandoffLimitV2::FunctionInstructions,
                    observed: u64::MAX,
                    maximum: MAX_INSTRUCTIONS_PER_FUNCTION_V2 as u64,
                })?;
            check_count(
                HandoffLimitV2::FunctionInstructions,
                instruction_count,
                MAX_INSTRUCTIONS_PER_FUNCTION_V2,
            )?;
            if function.attributes.contains(&FunctionAttributeV2::ReadNone)
                && function.blocks.iter().any(|block| {
                    block
                        .instructions
                        .iter()
                        .any(|instruction| self.instruction_may_access_memory(&instruction.kind))
                })
            {
                return Err(HandoffDiagnosticV2::InvalidFunctionAttribute);
            }
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| (parameter.value.id, parameter.value.value_type))
                .collect::<Vec<_>>();
            let mut definitions = parameters.iter().map(|(id, _)| *id).collect::<Vec<_>>();
            let entry = function
                .blocks
                .iter()
                .find(|block| block.id == function.entry)
                .expect("checked function entry exists");
            let mut entry_available = parameters.clone();
            self.validate_block(function, entry, &mut entry_available, &mut definitions)?;
            for block in function
                .blocks
                .iter()
                .filter(|block| block.id != function.entry)
            {
                // V2 has no phi nodes. Only parameters, entry definitions, and
                // prior definitions in the same block may be consumed.
                let mut available = entry_available.clone();
                self.validate_block(function, block, &mut available, &mut definitions)?;
            }
        }
        Ok(())
    }

    fn instruction_may_access_memory(&self, instruction: &InstructionKindV2) -> bool {
        match instruction {
            InstructionKindV2::Load { .. } | InstructionKindV2::Store { .. } => true,
            InstructionKindV2::Call {
                target: CallTargetV2::Function(id),
                ..
            } => self
                .functions
                .iter()
                .find(|function| function.id == *id)
                .is_none_or(|function| {
                    !function.attributes.contains(&FunctionAttributeV2::ReadNone)
                }),
            InstructionKindV2::Call {
                target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuBarrier),
                ..
            } => true,
            _ => false,
        }
    }

    fn validate_block(
        &self,
        function: &FunctionV2,
        block: &BasicBlockV2,
        available: &mut Vec<(ValueIdV2, ValueTypeV2)>,
        definitions: &mut Vec<ValueIdV2>,
    ) -> Result<(), HandoffDiagnosticV2> {
        for instruction in &block.instructions {
            self.validate_instruction(instruction, available)?;
            if let Some(result) = instruction.result {
                if definitions.contains(&result.id) {
                    return Err(HandoffDiagnosticV2::DuplicateDefinition(
                        DefinitionKindV2::Value,
                    ));
                }
                definitions.push(result.id);
                available.push((result.id, result.value_type));
                check_count(
                    HandoffLimitV2::Values,
                    definitions.len(),
                    MAX_VALUES_PER_FUNCTION_V2,
                )?;
            }
        }
        self.validate_terminator(function, &block.terminator, available)
    }

    fn validate_instruction(
        &self,
        instruction: &InstructionV2,
        available: &[(ValueIdV2, ValueTypeV2)],
    ) -> Result<(), HandoffDiagnosticV2> {
        let value_type = |id| lookup_value(available, id);
        match &instruction.kind {
            InstructionKindV2::Constant(value) => require_result(instruction, value.value_type()),
            InstructionKindV2::GlobalAddress(id) => {
                let global = self
                    .globals
                    .iter()
                    .find(|global| global.id == *id)
                    .ok_or(HandoffDiagnosticV2::MissingGlobalReference(*id))?;
                require_result(
                    instruction,
                    ValueTypeV2::Pointer {
                        pointee: global.value_type,
                        address_space: global.address_space,
                    },
                )
            }
            InstructionKindV2::Binary {
                operation,
                left,
                right,
            } => {
                let left_type = value_type(*left)?;
                require_value_type(available, *right, left_type)?;
                let valid = match operation {
                    BinaryOperationV2::Integer(_) => left_type.is_integer(),
                    BinaryOperationV2::Float(_) => left_type.is_float(),
                };
                if !valid {
                    return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                }
                require_result(instruction, left_type)
            }
            InstructionKindV2::Compare {
                predicate,
                left,
                right,
            } => {
                let left_type = value_type(*left)?;
                require_value_type(available, *right, left_type)?;
                let integer = matches!(
                    predicate,
                    ComparePredicateV2::IntegerEqual
                        | ComparePredicateV2::IntegerNotEqual
                        | ComparePredicateV2::UnsignedLessThan
                        | ComparePredicateV2::UnsignedLessOrEqual
                        | ComparePredicateV2::SignedLessThan
                        | ComparePredicateV2::SignedLessOrEqual
                );
                if (integer && !left_type.is_integer()) || (!integer && !left_type.is_float()) {
                    return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                }
                require_result(instruction, ValueTypeV2::Scalar(ScalarTypeV1::I1))
            }
            InstructionKindV2::Cast {
                operation,
                value,
                to,
            } => {
                let from = value_type(*value)?;
                if !valid_cast(*operation, from, *to) {
                    return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                }
                require_result(instruction, *to)
            }
            InstructionKindV2::GetElementPtr { base, indices } => {
                let base_type = value_type(*base)?;
                if !base_type.is_pointer() {
                    return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                }
                for index in indices {
                    if !value_type(*index)?.is_integer() {
                        return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                    }
                }
                require_result(instruction, base_type)
            }
            InstructionKindV2::Load {
                pointer,
                value_type: loaded,
                ..
            } => {
                let pointer_type = value_type(*pointer)?;
                if !matches!(
                    pointer_type,
                    ValueTypeV2::Pointer { pointee, .. } if pointee == *loaded
                ) {
                    return Err(HandoffDiagnosticV2::ValueTypeMismatch(*pointer));
                }
                require_result(instruction, ValueTypeV2::Scalar(*loaded))
            }
            InstructionKindV2::Store {
                pointer,
                value,
                value_type: stored,
                ..
            } => {
                if instruction.result.is_some() {
                    return Err(HandoffDiagnosticV2::InvalidInstructionResult);
                }
                let pointer_type = value_type(*pointer)?;
                if !matches!(
                    pointer_type,
                    ValueTypeV2::Pointer { pointee, .. } if pointee == *stored
                ) {
                    return Err(HandoffDiagnosticV2::ValueTypeMismatch(*pointer));
                }
                require_value_type(available, *value, ValueTypeV2::Scalar(*stored))
            }
            InstructionKindV2::Call { target, arguments } => {
                let (return_type, parameter_types) = match target {
                    CallTargetV2::Function(id) => {
                        let function = self
                            .functions
                            .iter()
                            .find(|function| function.id == *id)
                            .ok_or(HandoffDiagnosticV2::MissingFunctionReference(*id))?;
                        (
                            function.return_type,
                            function
                                .parameters
                                .iter()
                                .map(|parameter| parameter.value.value_type)
                                .collect::<Vec<_>>(),
                        )
                    }
                    CallTargetV2::Intrinsic(intrinsic) => {
                        if !self
                            .intrinsics
                            .iter()
                            .any(|reference| reference.intrinsic == *intrinsic)
                        {
                            return Err(HandoffDiagnosticV2::MissingIntrinsicReference);
                        }
                        intrinsic.signature()
                    }
                };
                if arguments.len() != parameter_types.len() {
                    return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                }
                for (argument, expected) in arguments.iter().zip(parameter_types) {
                    require_value_type(available, *argument, expected)?;
                }
                match return_type {
                    ReturnTypeV2::Void if instruction.result.is_none() => Ok(()),
                    ReturnTypeV2::Value(value_type) => require_result(instruction, value_type),
                    _ => Err(HandoffDiagnosticV2::InvalidInstructionResult),
                }
            }
        }
    }

    fn validate_terminator(
        &self,
        function: &FunctionV2,
        terminator: &TerminatorV2,
        available: &[(ValueIdV2, ValueTypeV2)],
    ) -> Result<(), HandoffDiagnosticV2> {
        let block_exists = |id| function.blocks.iter().any(|block| block.id == id);
        match terminator {
            TerminatorV2::Return(None) if function.return_type == ReturnTypeV2::Void => Ok(()),
            TerminatorV2::Return(Some(value)) => match function.return_type {
                ReturnTypeV2::Value(expected) => require_value_type(available, *value, expected),
                ReturnTypeV2::Void => Err(HandoffDiagnosticV2::InvalidTerminator),
            },
            TerminatorV2::Branch(block) => {
                if block_exists(*block) {
                    Ok(())
                } else {
                    Err(HandoffDiagnosticV2::MissingBlockReference(*block))
                }
            }
            TerminatorV2::ConditionalBranch {
                condition,
                then_block,
                else_block,
            } => {
                require_value_type(available, *condition, ValueTypeV2::Scalar(ScalarTypeV1::I1))?;
                if !block_exists(*then_block) {
                    return Err(HandoffDiagnosticV2::MissingBlockReference(*then_block));
                }
                if !block_exists(*else_block) {
                    return Err(HandoffDiagnosticV2::MissingBlockReference(*else_block));
                }
                Ok(())
            }
            TerminatorV2::Unreachable => Ok(()),
            _ => Err(HandoffDiagnosticV2::InvalidTerminator),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HandoffIdentityV2([u8; 32]);

impl HandoffIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for HandoffIdentityV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942HandoffV2 {
    pub(crate) base: Gfx942HandoffV1,
    pub(crate) module: ExecutableModuleV2,
}

impl Gfx942HandoffV2 {
    pub fn new(
        base: Gfx942HandoffV1,
        module: ExecutableModuleV2,
    ) -> Result<Self, HandoffDiagnosticV2> {
        if module.flags != base.module().flags()
            || module.named_metadata != base.module().named_metadata()
        {
            return Err(HandoffDiagnosticV2::MetadataMismatch);
        }
        let kernels = module
            .functions
            .iter()
            .filter(|function| function.kind == FunctionKindV2::Kernel)
            .collect::<Vec<_>>();
        if kernels.len() != base.kernels().len() {
            return Err(HandoffDiagnosticV2::MissingKernelSignature);
        }
        for kernel in kernels {
            let Some(v1) = base
                .kernels()
                .iter()
                .find(|candidate| candidate.symbol() == kernel.symbol)
            else {
                return Err(HandoffDiagnosticV2::MissingKernelSignature);
            };
            let parameters_match = kernel.parameters.len() == v1.parameters().len()
                && kernel
                    .parameters
                    .iter()
                    .zip(v1.parameters())
                    .all(|(v2, v1)| {
                        v2.name == v1.name()
                            && v2.value.value_type == v1.value_type().into()
                            && v2.attributes == v1.attributes()
                    });
            let expected_attributes = v1
                .function_attributes()
                .iter()
                .copied()
                .map(FunctionAttributeV2::from)
                .collect::<Vec<_>>();
            if kernel.calling_convention != CallingConventionV2::AmdGpuKernel
                || kernel.return_type != ReturnTypeV2::Void
                || kernel.evidence.origin != v1.origin()
                || !parameters_match
                || kernel.attributes != expected_attributes
            {
                return Err(HandoffDiagnosticV2::KernelSignatureMismatch);
            }
        }

        for evidence in module.evidence_iter() {
            if !base
                .origins()
                .iter()
                .any(|origin| origin.identity() == evidence.origin)
            {
                return Err(HandoffDiagnosticV2::MissingOriginReference);
            }
            for obligation in &evidence.obligations {
                if !base
                    .obligations()
                    .iter()
                    .any(|candidate| candidate.identity() == *obligation)
                {
                    return Err(HandoffDiagnosticV2::MissingObligationReference);
                }
            }
        }
        let handoff = Self { base, module };
        let encoded_len = crate::codec_v2::encode_handoff_v2(&handoff).len();
        if encoded_len > MAX_CANONICAL_HANDOFF_BYTES_V2 {
            return Err(HandoffDiagnosticV2::LimitExceeded {
                limit: HandoffLimitV2::CanonicalBytes,
                observed: encoded_len as u64,
                maximum: MAX_CANONICAL_HANDOFF_BYTES_V2 as u64,
            });
        }
        Ok(handoff)
    }

    pub const fn base(&self) -> &Gfx942HandoffV1 {
        &self.base
    }

    pub const fn module(&self) -> &ExecutableModuleV2 {
        &self.module
    }

    pub fn encode_canonical(&self) -> crate::CanonicalHandoffBytesV2 {
        crate::CanonicalHandoffBytesV2::from_validated(crate::codec_v2::encode_handoff_v2(self))
    }

    pub fn identity(&self) -> HandoffIdentityV2 {
        HandoffIdentityV2(hash_identity(
            HANDOFF_IDENTITY_DOMAIN_V2,
            self.encode_canonical().as_bytes(),
        ))
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, crate::DecodeHandoffErrorV2> {
        crate::codec_v2::decode_handoff_v2(bytes)
    }
}

impl ExecutableModuleV2 {
    fn evidence_iter(&self) -> impl Iterator<Item = &EvidenceV2> {
        self.globals
            .iter()
            .map(|global| &global.evidence)
            .chain(self.intrinsics.iter().map(|intrinsic| &intrinsic.evidence))
            .chain(self.functions.iter().flat_map(|function| {
                core::iter::once(&function.evidence).chain(
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| block.instructions.iter())
                        .map(|instruction| &instruction.evidence),
                )
            }))
    }
}

fn require_result(
    instruction: &InstructionV2,
    expected: ValueTypeV2,
) -> Result<(), HandoffDiagnosticV2> {
    match instruction.result {
        Some(result) if result.value_type == expected => Ok(()),
        Some(result) => Err(HandoffDiagnosticV2::ValueTypeMismatch(result.id)),
        None => Err(HandoffDiagnosticV2::InvalidInstructionResult),
    }
}

fn lookup_value(
    available: &[(ValueIdV2, ValueTypeV2)],
    id: ValueIdV2,
) -> Result<ValueTypeV2, HandoffDiagnosticV2> {
    available
        .iter()
        .find_map(|(candidate, value_type)| (*candidate == id).then_some(*value_type))
        .ok_or(HandoffDiagnosticV2::MissingValueReference(id))
}

fn require_value_type(
    available: &[(ValueIdV2, ValueTypeV2)],
    id: ValueIdV2,
    expected: ValueTypeV2,
) -> Result<(), HandoffDiagnosticV2> {
    if lookup_value(available, id)? == expected {
        Ok(())
    } else {
        Err(HandoffDiagnosticV2::ValueTypeMismatch(id))
    }
}

fn valid_cast(operation: CastOperationV2, from: ValueTypeV2, to: ValueTypeV2) -> bool {
    let scalar_width = |value| match value {
        ValueTypeV2::Scalar(ScalarTypeV1::I1) => Some((true, 1_u8)),
        ValueTypeV2::Scalar(ScalarTypeV1::I8) => Some((true, 8)),
        ValueTypeV2::Scalar(ScalarTypeV1::I16) => Some((true, 16)),
        ValueTypeV2::Scalar(ScalarTypeV1::I32) => Some((true, 32)),
        ValueTypeV2::Scalar(ScalarTypeV1::I64) => Some((true, 64)),
        ValueTypeV2::Scalar(ScalarTypeV1::F16 | ScalarTypeV1::Bf16) => Some((false, 16)),
        ValueTypeV2::Scalar(ScalarTypeV1::F32) => Some((false, 32)),
        ValueTypeV2::Scalar(ScalarTypeV1::F64) => Some((false, 64)),
        ValueTypeV2::Pointer { .. } => None,
    };
    let (Some((from_integer, from_width)), Some((to_integer, to_width))) =
        (scalar_width(from), scalar_width(to))
    else {
        return false;
    };
    match operation {
        CastOperationV2::ZeroExtend | CastOperationV2::SignExtend => {
            from_integer && to_integer && from_width < to_width
        }
        CastOperationV2::Truncate => from_integer && to_integer && from_width > to_width,
        CastOperationV2::FloatExtend => !from_integer && !to_integer && from_width < to_width,
        CastOperationV2::FloatTruncate => !from_integer && !to_integer && from_width > to_width,
        CastOperationV2::UnsignedIntToFloat | CastOperationV2::SignedIntToFloat => {
            from_integer && !to_integer
        }
        CastOperationV2::FloatToUnsignedInt | CastOperationV2::FloatToSignedInt => {
            !from_integer && to_integer
        }
    }
}

fn duplicate_symbol<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let values = values.collect::<Vec<_>>();
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
}

fn validate_symbol(value: &str) -> Result<(), HandoffDiagnosticV2> {
    if value.len() > MAX_SYMBOL_BYTES_V2 {
        return Err(HandoffDiagnosticV2::LimitExceeded {
            limit: HandoffLimitV2::SymbolBytes,
            observed: value.len() as u64,
            maximum: MAX_SYMBOL_BYTES_V2 as u64,
        });
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(HandoffDiagnosticV2::InvalidSymbol);
    };
    if !(first.is_ascii_alphabetic() || first == b'_' || first == b'.')
        || !bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$' | b'-'))
    {
        return Err(HandoffDiagnosticV2::InvalidSymbol);
    }
    Ok(())
}

fn check_nonempty_count(
    name: &'static str,
    limit: HandoffLimitV2,
    observed: usize,
    maximum: usize,
) -> Result<(), HandoffDiagnosticV2> {
    if observed == 0 {
        return Err(HandoffDiagnosticV2::EmptyCollection(name));
    }
    check_count(limit, observed, maximum)
}

fn check_count(
    limit: HandoffLimitV2,
    observed: usize,
    maximum: usize,
) -> Result<(), HandoffDiagnosticV2> {
    if observed > maximum {
        return Err(HandoffDiagnosticV2::LimitExceeded {
            limit,
            observed: observed as u64,
            maximum: maximum as u64,
        });
    }
    Ok(())
}

fn hash_identity(domain: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u32).to_le_bytes());
    hasher.update(domain);
    hasher.update((payload.len() as u32).to_le_bytes());
    hasher.update(payload);
    hasher.finalize().into()
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}
