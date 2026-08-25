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
pub const GENERAL_GEMM_VECTOR_LANES_V2: u8 = 4;
pub const GENERAL_GEMM_LDS_ELEMENTS_V2: u16 = 256;
pub const MAX_CONSTANT_GLOBAL_BYTES_V2: usize = 4 * 1024;
pub const KERNEL_DESCRIPTOR_SECTION_V2: &str = ".fe2o3.kd.v1";
pub const GENERAL_GEMM_BINDING_SECTION_V2: &str = ".fe2o3.general-gemm.binding.v1";

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
    Vector {
        element: ScalarTypeV1,
        lanes: u8,
    },
    Pointer {
        pointee: ScalarTypeV1,
        address_space: AddressSpaceV1,
    },
    ArrayPointer {
        element: ScalarTypeV1,
        elements: u16,
        address_space: AddressSpaceV1,
    },
}

impl ValueTypeV2 {
    pub const fn is_pointer(self) -> bool {
        matches!(self, Self::Pointer { .. } | Self::ArrayPointer { .. })
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

    pub const fn fixed_vector(element: ScalarTypeV1) -> Self {
        Self::Vector {
            element,
            lanes: GENERAL_GEMM_VECTOR_LANES_V2,
        }
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
    pub(crate) array_elements: Option<u16>,
    pub(crate) alignment: u16,
    pub(crate) byte_initializer: Option<Vec<u8>>,
    pub(crate) section: Option<String>,
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
            array_elements: None,
            alignment: 1,
            byte_initializer: None,
            section: None,
            evidence,
        })
    }

    pub fn new_lds_bf16_array_256(
        id: GlobalIdV2,
        symbol: &str,
        evidence: EvidenceV2,
    ) -> Result<Self, HandoffDiagnosticV2> {
        validate_symbol(symbol)?;
        Ok(Self {
            id,
            symbol: symbol.to_string(),
            linkage: GlobalLinkageV2::Internal,
            address_space: AddressSpaceV1::Local,
            mutable: true,
            value_type: ScalarTypeV1::I16,
            initializer: None,
            array_elements: Some(GENERAL_GEMM_LDS_ELEMENTS_V2),
            alignment: 16,
            byte_initializer: None,
            section: None,
            evidence,
        })
    }

    pub fn new_private_constant_bytes(
        id: GlobalIdV2,
        symbol: &str,
        section: &str,
        bytes: Vec<u8>,
        alignment: u16,
        evidence: EvidenceV2,
    ) -> Result<Self, HandoffDiagnosticV2> {
        validate_symbol(symbol)?;
        if !matches!(
            section,
            KERNEL_DESCRIPTOR_SECTION_V2 | GENERAL_GEMM_BINDING_SECTION_V2
        ) || bytes.is_empty()
            || bytes.len() > MAX_CONSTANT_GLOBAL_BYTES_V2
            || alignment == 0
            || !alignment.is_power_of_two()
            || alignment > 256
        {
            return Err(HandoffDiagnosticV2::UnsupportedInstruction);
        }
        let elements =
            u16::try_from(bytes.len()).map_err(|_| HandoffDiagnosticV2::UnsupportedInstruction)?;
        Ok(Self {
            id,
            symbol: symbol.to_string(),
            linkage: GlobalLinkageV2::Internal,
            address_space: AddressSpaceV1::Constant,
            mutable: false,
            value_type: ScalarTypeV1::I8,
            initializer: None,
            array_elements: Some(elements),
            alignment,
            byte_initializer: Some(bytes),
            section: Some(section.to_string()),
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

    pub const fn array_elements(&self) -> Option<u16> {
        self.array_elements
    }

    pub const fn alignment(&self) -> u16 {
        self.alignment
    }

    pub fn byte_initializer(&self) -> Option<&[u8]> {
        self.byte_initializer.as_deref()
    }

    pub fn section(&self) -> Option<&str> {
        self.section.as_deref()
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
    Trap,
    AmdGpuMfmaF32_16x16x16Bf16_1k,
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
            Self::Trap => (ReturnTypeV2::Void, vec![]),
            Self::AmdGpuMfmaF32_16x16x16Bf16_1k => {
                let i16x4 = ValueTypeV2::fixed_vector(ScalarTypeV1::I16);
                let f32x4 = ValueTypeV2::fixed_vector(ScalarTypeV1::F32);
                (
                    ReturnTypeV2::Value(f32x4),
                    vec![
                        i16x4,
                        i16x4,
                        f32x4,
                        ValueTypeV2::Scalar(ScalarTypeV1::I32),
                        ValueTypeV2::Scalar(ScalarTypeV1::I32),
                        ValueTypeV2::Scalar(ScalarTypeV1::I32),
                    ],
                )
            }
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
    RequiredWorkgroupSize([u16; 3]),
    NoCompletionAction,
    NoDefaultQueue,
    NoHeapPointer,
    NoHostcallPointer,
    NoMultigridSyncArgument,
    NoQueuePointer,
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
            Self::RequiredWorkgroupSize(_) => 15,
            Self::NoCompletionAction => 16,
            Self::NoDefaultQueue => 17,
            Self::NoHeapPointer => 18,
            Self::NoHostcallPointer => 19,
            Self::NoMultigridSyncArgument => 20,
            Self::NoQueuePointer => 21,
        }
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::NoUnwind => "nounwind",
            Self::AlwaysInline => "alwaysinline",
            Self::NoInline => "noinline",
            Self::ReadNone => "memory(none)",
            Self::WillReturn => "willreturn",
            Self::FlatWorkgroupSize(_) => "amdgpu-flat-work-group-size",
            Self::WavesPerEu(_) => "amdgpu-waves-per-eu",
            Self::DenormalFpMathF32Ieee => "denormal-fp-math-f32=ieee,ieee",
            Self::UnsafeFpMathDisabled => "unsafe-fp-math=false",
            Self::NoInfsFpMathDisabled => "no-infs-fp-math=false",
            Self::NoNansFpMathDisabled => "no-nans-fp-math=false",
            Self::NoSignedZerosFpMathDisabled => "no-signed-zeros-fp-math=false",
            Self::ApproxFuncFpMathDisabled => "approx-func-fp-math=false",
            Self::FpContractOff => "fp-contract=off",
            Self::RequiredWorkgroupSize(_) => "reqd_work_group_size",
            Self::NoCompletionAction => "amdgpu-no-completion-action",
            Self::NoDefaultQueue => "amdgpu-no-default-queue",
            Self::NoHeapPointer => "amdgpu-no-heap-ptr",
            Self::NoHostcallPointer => "amdgpu-no-hostcall-ptr",
            Self::NoMultigridSyncArgument => "amdgpu-no-multigrid-sync-arg",
            Self::NoQueuePointer => "amdgpu-no-queue-ptr",
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
            FunctionAttributeV1::NoCompletionAction => Self::NoCompletionAction,
            FunctionAttributeV1::NoDefaultQueue => Self::NoDefaultQueue,
            FunctionAttributeV1::NoHeapPointer => Self::NoHeapPointer,
            FunctionAttributeV1::NoHostcallPointer => Self::NoHostcallPointer,
            FunctionAttributeV1::NoMultigridSyncArgument => Self::NoMultigridSyncArgument,
            FunctionAttributeV1::NoQueuePointer => Self::NoQueuePointer,
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
    PointerToInt,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallTargetV2 {
    Function(FunctionIdV2),
    Intrinsic(IntrinsicV2),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionKindV2 {
    Constant(ScalarConstantV2),
    VectorZero {
        element_type: ScalarTypeV1,
    },
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
    VectorLoad4 {
        pointer: ValueIdV2,
        element_type: ScalarTypeV1,
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
    Phi {
        incoming: Vec<(ValueIdV2, BlockIdV2)>,
    },
    InsertElement {
        vector: ValueIdV2,
        element: ValueIdV2,
        index: ValueIdV2,
    },
    ExtractElement {
        vector: ValueIdV2,
        index: ValueIdV2,
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
            InstructionKindV2::Phi { incoming } => check_nonempty_count(
                "phi incoming values",
                HandoffLimitV2::GetElementPtrIndices,
                incoming.len(),
                MAX_FUNCTION_BLOCKS_V2,
            )?,
            InstructionKindV2::Load { alignment, .. }
            | InstructionKindV2::VectorLoad4 { alignment, .. }
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
        if matches!(return_type, ReturnTypeV2::Value(value) if !valid_value_type_v2(value))
            || parameters
                .iter()
                .any(|parameter| !valid_value_type_v2(parameter.value.value_type))
            || blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    instruction
                        .result
                        .is_some_and(|result| !valid_value_type_v2(result.value_type))
                })
            })
        {
            return Err(HandoffDiagnosticV2::UnsupportedInstruction);
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
        if let Some(shape) = attributes.iter().find_map(|attribute| match attribute {
            FunctionAttributeV2::RequiredWorkgroupSize(shape) => Some(*shape),
            _ => None,
        }) {
            let product = shape.into_iter().try_fold(1_u32, |product, extent| {
                (extent != 0)
                    .then(|| product.checked_mul(u32::from(extent)))
                    .flatten()
            });
            if kind != FunctionKindV2::Kernel || product.is_none_or(|product| product > 1_024) {
                return Err(HandoffDiagnosticV2::InvalidFunctionAttribute);
            }
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
            let mut available = function
                .parameters
                .iter()
                .map(|parameter| (parameter.value.id, parameter.value.value_type))
                .collect::<Vec<_>>();
            for result in function.blocks.iter().flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .filter_map(|instruction| instruction.result)
            }) {
                if available.iter().any(|(id, _)| *id == result.id) {
                    return Err(HandoffDiagnosticV2::DuplicateDefinition(
                        DefinitionKindV2::Value,
                    ));
                }
                available.push((result.id, result.value_type));
            }
            check_count(
                HandoffLimitV2::Values,
                available.len(),
                MAX_VALUES_PER_FUNCTION_V2,
            )?;
            for block in &function.blocks {
                let mut saw_non_phi = false;
                for instruction in &block.instructions {
                    if matches!(instruction.kind, InstructionKindV2::Phi { .. }) {
                        if saw_non_phi {
                            return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                        }
                    } else {
                        saw_non_phi = true;
                    }
                    self.validate_instruction(instruction, &available)?;
                }
                self.validate_terminator(function, &block.terminator, &available)?;
            }
            validate_function_ssa_v2(function)?;
        }
        Ok(())
    }

    fn instruction_may_access_memory(&self, instruction: &InstructionKindV2) -> bool {
        match instruction {
            InstructionKindV2::Load { .. }
            | InstructionKindV2::VectorLoad4 { .. }
            | InstructionKindV2::Store { .. } => true,
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

    fn validate_instruction(
        &self,
        instruction: &InstructionV2,
        available: &[(ValueIdV2, ValueTypeV2)],
    ) -> Result<(), HandoffDiagnosticV2> {
        let value_type = |id| lookup_value(available, id);
        match &instruction.kind {
            InstructionKindV2::Constant(value) => require_result(instruction, value.value_type()),
            InstructionKindV2::VectorZero { element_type } => {
                if !matches!(element_type, ScalarTypeV1::I16 | ScalarTypeV1::F32) {
                    return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                }
                require_result(instruction, ValueTypeV2::fixed_vector(*element_type))
            }
            InstructionKindV2::GlobalAddress(id) => {
                let global = self
                    .globals
                    .iter()
                    .find(|global| global.id == *id)
                    .ok_or(HandoffDiagnosticV2::MissingGlobalReference(*id))?;
                let result_type = match global.array_elements {
                    Some(elements) => ValueTypeV2::ArrayPointer {
                        element: global.value_type,
                        elements,
                        address_space: global.address_space,
                    },
                    None => ValueTypeV2::Pointer {
                        pointee: global.value_type,
                        address_space: global.address_space,
                    },
                };
                require_result(instruction, result_type)
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
                let result_type = match base_type {
                    ValueTypeV2::Pointer { .. } => base_type,
                    ValueTypeV2::ArrayPointer {
                        element,
                        address_space,
                        ..
                    } if indices.len() == 2 => ValueTypeV2::Pointer {
                        pointee: element,
                        address_space,
                    },
                    ValueTypeV2::ArrayPointer { .. } => {
                        return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                    }
                    _ => return Err(HandoffDiagnosticV2::UnsupportedInstruction),
                };
                require_result(instruction, result_type)
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
            InstructionKindV2::VectorLoad4 {
                pointer,
                element_type,
                ..
            } => {
                let pointer_type = value_type(*pointer)?;
                if !matches!(
                    pointer_type,
                    ValueTypeV2::Pointer { pointee, .. } if pointee == *element_type
                ) {
                    return Err(HandoffDiagnosticV2::ValueTypeMismatch(*pointer));
                }
                require_result(instruction, ValueTypeV2::fixed_vector(*element_type))
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
            InstructionKindV2::Phi { incoming } => {
                let result = instruction
                    .result
                    .ok_or(HandoffDiagnosticV2::InvalidInstructionResult)?;
                for (value, _) in incoming {
                    require_value_type(available, *value, result.value_type)?;
                }
                Ok(())
            }
            InstructionKindV2::InsertElement {
                vector,
                element,
                index,
            } => {
                let vector_type = value_type(*vector)?;
                let ValueTypeV2::Vector {
                    element: element_type,
                    lanes,
                } = vector_type
                else {
                    return Err(HandoffDiagnosticV2::ValueTypeMismatch(*vector));
                };
                if lanes != GENERAL_GEMM_VECTOR_LANES_V2 {
                    return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                }
                require_value_type(available, *element, ValueTypeV2::Scalar(element_type))?;
                require_value_type(available, *index, ValueTypeV2::Scalar(ScalarTypeV1::I32))?;
                require_result(instruction, vector_type)
            }
            InstructionKindV2::ExtractElement { vector, index } => {
                let vector_type = value_type(*vector)?;
                let ValueTypeV2::Vector {
                    element: element_type,
                    lanes,
                } = vector_type
                else {
                    return Err(HandoffDiagnosticV2::ValueTypeMismatch(*vector));
                };
                if lanes != GENERAL_GEMM_VECTOR_LANES_V2 {
                    return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                }
                require_value_type(available, *index, ValueTypeV2::Scalar(ScalarTypeV1::I32))?;
                require_result(instruction, ValueTypeV2::Scalar(element_type))
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
            let semantic_attributes = kernel
                .attributes
                .iter()
                .copied()
                .filter(|attribute| {
                    !matches!(attribute, FunctionAttributeV2::RequiredWorkgroupSize(_))
                })
                .collect::<Vec<_>>();
            if kernel.calling_convention != CallingConventionV2::AmdGpuKernel
                || kernel.return_type != ReturnTypeV2::Void
                || kernel.evidence.origin != v1.origin()
                || !parameters_match
                || semantic_attributes != expected_attributes
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

#[derive(Clone, Copy)]
enum SsaDefinitionSiteV2 {
    Parameter,
    Instruction { block: usize, instruction: usize },
}

#[derive(Clone, Copy)]
struct SsaDefinitionV2 {
    id: ValueIdV2,
    site: SsaDefinitionSiteV2,
}

fn validate_function_ssa_v2(function: &FunctionV2) -> Result<(), HandoffDiagnosticV2> {
    let block_index = |id: BlockIdV2| {
        function
            .blocks
            .binary_search_by_key(&id, |block| block.id)
            .map_err(|_| HandoffDiagnosticV2::MissingBlockReference(id))
    };
    let entry = block_index(function.entry)?;
    let mut successors = vec![Vec::new(); function.blocks.len()];
    for (index, block) in function.blocks.iter().enumerate() {
        match block.terminator {
            TerminatorV2::Branch(target) => successors[index].push(block_index(target)?),
            TerminatorV2::ConditionalBranch {
                then_block,
                else_block,
                ..
            } => {
                successors[index].push(block_index(then_block)?);
                let other = block_index(else_block)?;
                if !successors[index].contains(&other) {
                    successors[index].push(other);
                }
            }
            TerminatorV2::Return(_) | TerminatorV2::Unreachable => {}
        }
    }
    let mut predecessors = vec![Vec::new(); function.blocks.len()];
    for (source, targets) in successors.iter().enumerate() {
        for target in targets {
            predecessors[*target].push(source);
        }
    }
    if !predecessors[entry].is_empty() {
        return Err(HandoffDiagnosticV2::UnsupportedInstruction);
    }
    let mut reachable = vec![false; function.blocks.len()];
    let mut worklist = vec![entry];
    reachable[entry] = true;
    let mut cursor = 0;
    while cursor < worklist.len() {
        let block = worklist[cursor];
        cursor += 1;
        for successor in &successors[block] {
            if !reachable[*successor] {
                reachable[*successor] = true;
                worklist.push(*successor);
            }
        }
    }
    if reachable.iter().any(|reachable| !reachable) {
        return Err(HandoffDiagnosticV2::UnsupportedInstruction);
    }

    let words = function.blocks.len().div_ceil(u64::BITS as usize);
    let mut dominators = vec![vec![u64::MAX; words]; function.blocks.len()];
    dominators[entry].fill(0);
    set_ssa_bit_v2(&mut dominators[entry], entry);
    loop {
        let mut changed = false;
        for block in 0..function.blocks.len() {
            if block == entry {
                continue;
            }
            let Some((first, rest)) = predecessors[block].split_first() else {
                return Err(HandoffDiagnosticV2::UnsupportedInstruction);
            };
            let mut updated = dominators[*first].clone();
            for predecessor in rest {
                for (word, predecessor_word) in updated.iter_mut().zip(&dominators[*predecessor]) {
                    *word &= predecessor_word;
                }
            }
            set_ssa_bit_v2(&mut updated, block);
            if updated != dominators[block] {
                dominators[block] = updated;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let dominates = |definition: usize, use_block: usize| {
        dominators[use_block][definition / u64::BITS as usize]
            & (1_u64 << (definition % u64::BITS as usize))
            != 0
    };

    let mut definitions = function
        .parameters
        .iter()
        .map(|parameter| SsaDefinitionV2 {
            id: parameter.value.id,
            site: SsaDefinitionSiteV2::Parameter,
        })
        .collect::<Vec<_>>();
    for (block, body) in function.blocks.iter().enumerate() {
        for (instruction, operation) in body.instructions.iter().enumerate() {
            if let Some(result) = operation.result {
                definitions.push(SsaDefinitionV2 {
                    id: result.id,
                    site: SsaDefinitionSiteV2::Instruction { block, instruction },
                });
            }
        }
    }
    definitions.sort_unstable_by_key(|definition| definition.id);
    let definition = |id: ValueIdV2| {
        definitions
            .binary_search_by_key(&id, |definition| definition.id)
            .ok()
            .map(|index| definitions[index])
            .ok_or(HandoffDiagnosticV2::MissingValueReference(id))
    };
    let ordinary_use_is_valid =
        |id: ValueIdV2, block: usize, instruction: usize| match definition(id)?.site {
            SsaDefinitionSiteV2::Parameter => Ok(()),
            SsaDefinitionSiteV2::Instruction {
                block: definition_block,
                instruction: definition_instruction,
            } if (definition_block == block && definition_instruction < instruction)
                || (definition_block != block && dominates(definition_block, block)) =>
            {
                Ok(())
            }
            SsaDefinitionSiteV2::Instruction { .. } => {
                Err(HandoffDiagnosticV2::UnsupportedInstruction)
            }
        };

    for (block, body) in function.blocks.iter().enumerate() {
        for (instruction_index, operation) in body.instructions.iter().enumerate() {
            match &operation.kind {
                InstructionKindV2::Phi { incoming } => {
                    let mut actual = incoming
                        .iter()
                        .map(|(_, predecessor)| block_index(*predecessor))
                        .collect::<Result<Vec<_>, _>>()?;
                    actual.sort_unstable();
                    if actual != predecessors[block]
                        || actual.windows(2).any(|pair| pair[0] == pair[1])
                    {
                        return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                    }
                    for (value, predecessor) in incoming {
                        let predecessor = block_index(*predecessor)?;
                        match definition(*value)?.site {
                            SsaDefinitionSiteV2::Parameter => {}
                            SsaDefinitionSiteV2::Instruction {
                                block: definition_block,
                                ..
                            } if definition_block == predecessor
                                || dominates(definition_block, predecessor) => {}
                            SsaDefinitionSiteV2::Instruction { .. } => {
                                return Err(HandoffDiagnosticV2::UnsupportedInstruction);
                            }
                        }
                    }
                }
                kind => {
                    for operand in instruction_operands_v2(kind) {
                        ordinary_use_is_valid(operand, block, instruction_index)?;
                    }
                }
            }
        }
        let terminator_position = body.instructions.len();
        match body.terminator {
            TerminatorV2::Return(Some(value)) => {
                ordinary_use_is_valid(value, block, terminator_position)?;
            }
            TerminatorV2::ConditionalBranch { condition, .. } => {
                ordinary_use_is_valid(condition, block, terminator_position)?;
            }
            TerminatorV2::Return(None) | TerminatorV2::Branch(_) | TerminatorV2::Unreachable => {}
        }
    }
    Ok(())
}

fn instruction_operands_v2(kind: &InstructionKindV2) -> Vec<ValueIdV2> {
    match kind {
        InstructionKindV2::Constant(_)
        | InstructionKindV2::VectorZero { .. }
        | InstructionKindV2::GlobalAddress(_) => Vec::new(),
        InstructionKindV2::Binary { left, right, .. }
        | InstructionKindV2::Compare { left, right, .. } => vec![*left, *right],
        InstructionKindV2::Cast { value, .. } => vec![*value],
        InstructionKindV2::GetElementPtr { base, indices } => {
            let mut values = vec![*base];
            values.extend(indices.iter().copied());
            values
        }
        InstructionKindV2::Load { pointer, .. }
        | InstructionKindV2::VectorLoad4 { pointer, .. } => vec![*pointer],
        InstructionKindV2::Store { pointer, value, .. } => vec![*pointer, *value],
        InstructionKindV2::Call { arguments, .. } => arguments.clone(),
        InstructionKindV2::Phi { .. } => Vec::new(),
        InstructionKindV2::InsertElement {
            vector,
            element,
            index,
        } => vec![*vector, *element, *index],
        InstructionKindV2::ExtractElement { vector, index } => vec![*vector, *index],
    }
}

fn set_ssa_bit_v2(words: &mut [u64], bit: usize) {
    words[bit / u64::BITS as usize] |= 1_u64 << (bit % u64::BITS as usize);
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

const fn valid_value_type_v2(value: ValueTypeV2) -> bool {
    match value {
        ValueTypeV2::Scalar(_) | ValueTypeV2::Pointer { .. } => true,
        ValueTypeV2::Vector { element, lanes } => {
            lanes == GENERAL_GEMM_VECTOR_LANES_V2
                && matches!(element, ScalarTypeV1::I16 | ScalarTypeV1::F32)
        }
        ValueTypeV2::ArrayPointer {
            element,
            elements,
            address_space,
        } => {
            matches!(element, ScalarTypeV1::I16)
                && elements == GENERAL_GEMM_LDS_ELEMENTS_V2
                && matches!(address_space, AddressSpaceV1::Local)
        }
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
        ValueTypeV2::Vector { .. }
        | ValueTypeV2::Pointer { .. }
        | ValueTypeV2::ArrayPointer { .. } => None,
    };
    if matches!(operation, CastOperationV2::PointerToInt) {
        return from.is_pointer() && to == ValueTypeV2::Scalar(ScalarTypeV1::I64);
    }
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
        CastOperationV2::PointerToInt => false,
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
