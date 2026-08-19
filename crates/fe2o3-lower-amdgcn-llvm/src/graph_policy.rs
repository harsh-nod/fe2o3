use fe2o3_llvm_handoff::{
    BlockIdV2, CallingConventionV2, FunctionAttributeV2, FunctionIdV2, FunctionKindV2,
    FunctionParameterV2, FunctionV2, Gfx942HandoffV2, Gfx942TargetPolicyV1, GlobalIdV2, GlobalV2,
    IdentityV1, IntrinsicReferenceV2, IntrinsicV2, MAX_FUNCTION_ATTRIBUTES_V2,
    MAX_FUNCTION_BLOCKS_V2, MAX_FUNCTION_PARAMETERS_V2, MAX_MODULE_FLAGS_V2, MAX_NAMED_METADATA_V2,
    MAX_PARAMETER_ATTRIBUTES_V2, MAX_SYMBOL_BYTES_V2, ModuleFlagV1, NamedMetadataV1,
    ParameterAttributeV1, TargetFeatureV1, TypedValueV2, ValueIdV2, ValueTypeV2, WavesPerEuV1,
    WorkgroupSizeRangeV1,
};
use pliron::{
    builtin::attributes::BytesAttr, context::Context, identifier::Identifier, operation::Operation,
};

use crate::model::{ConstructionStageV1, InspectionErrorV1, LoweringErrorV1};

pub(crate) const MODULE_TARGET_POLICY_ATTR_V1: &str = "fe2o3_module_target_policy_v1";
pub(crate) const MODULE_FLAGS_ATTR_V1: &str = "fe2o3_module_flags_v1";
pub(crate) const MODULE_METADATA_ATTR_V1: &str = "fe2o3_module_metadata_v1";
pub(crate) const GLOBAL_POLICY_ATTR_V1: &str = "fe2o3_global_policy_v1";
pub(crate) const INTRINSIC_POLICY_ATTR_V1: &str = "fe2o3_intrinsic_policy_v1";
pub(crate) const FUNCTION_ABI_ATTR_V1: &str = "fe2o3_function_abi_v1";
pub(crate) const FUNCTION_ATTRIBUTES_ATTR_V1: &str = "fe2o3_function_attributes_v1";
pub(crate) const FUNCTION_PARAMETERS_ATTR_V1: &str = "fe2o3_function_parameters_v1";
pub(crate) const FUNCTION_BLOCKS_ATTR_V1: &str = "fe2o3_function_blocks_v1";
pub(crate) const INSTRUCTION_BINDING_ATTR_V1: &str = "fe2o3_instruction_binding_v1";

const POLICY_VERSION_V1: u8 = 1;
const MAX_GRAPH_POLICY_BYTES_V1: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModuleGraphPolicyV1 {
    pub(crate) flags: Vec<ModuleFlagV1>,
    pub(crate) named_metadata: Vec<NamedMetadataV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GlobalGraphPolicyV1 {
    pub(crate) id: GlobalIdV2,
    pub(crate) mutable: bool,
    pub(crate) section: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionGraphPolicyV1 {
    pub(crate) id: FunctionIdV2,
    pub(crate) kind: FunctionKindV2,
    pub(crate) calling_convention: CallingConventionV2,
    pub(crate) entry: BlockIdV2,
    pub(crate) attributes: Vec<FunctionAttributeV2>,
    pub(crate) parameters: Vec<FunctionParameterV2>,
    pub(crate) blocks: Vec<BlockGraphPolicyV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BlockGraphPolicyV1 {
    pub(crate) id: BlockIdV2,
    pub(crate) phis: Vec<TypedValueV2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InstructionGraphBindingV1 {
    pub(crate) block: BlockIdV2,
    pub(crate) ordinal: u32,
    pub(crate) result: Option<TypedValueV2>,
}

pub(crate) fn install_module_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    source: &Gfx942HandoffV2,
) -> Result<(), LoweringErrorV1> {
    set_bytes(
        context,
        operation,
        MODULE_TARGET_POLICY_ATTR_V1,
        target_policy(source.base().target()),
    )?;
    set_bytes(
        context,
        operation,
        MODULE_FLAGS_ATTR_V1,
        module_flags(source.module().flags()),
    )?;
    set_bytes(
        context,
        operation,
        MODULE_METADATA_ATTR_V1,
        module_metadata(source.module().named_metadata()),
    )
}

pub(crate) fn install_global_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    source: &GlobalV2,
) -> Result<(), LoweringErrorV1> {
    set_bytes(
        context,
        operation,
        GLOBAL_POLICY_ATTR_V1,
        global_policy(source),
    )
}

pub(crate) fn install_intrinsic_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    source: &IntrinsicReferenceV2,
) -> Result<(), LoweringErrorV1> {
    let mut bytes = vec![POLICY_VERSION_V1];
    bytes.push(intrinsic_tag(source.intrinsic()));
    set_bytes(context, operation, INTRINSIC_POLICY_ATTR_V1, bytes)
}

pub(crate) fn install_function_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    source: &FunctionV2,
) -> Result<(), LoweringErrorV1> {
    set_bytes(
        context,
        operation,
        FUNCTION_ABI_ATTR_V1,
        function_abi(source),
    )?;
    set_bytes(
        context,
        operation,
        FUNCTION_ATTRIBUTES_ATTR_V1,
        function_attributes(source),
    )?;
    set_bytes(
        context,
        operation,
        FUNCTION_PARAMETERS_ATTR_V1,
        function_parameters(source),
    )?;
    set_bytes(
        context,
        operation,
        FUNCTION_BLOCKS_ATTR_V1,
        function_blocks(source),
    )
}

pub(crate) fn install_instruction_binding(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    block: u32,
    ordinal: u32,
    result: Option<TypedValueV2>,
) -> Result<(), LoweringErrorV1> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_u32(&mut bytes, block);
    put_u32(&mut bytes, ordinal);
    match result {
        Some(result) => {
            bytes.push(1);
            put_u32(&mut bytes, result.id().get());
            encode_value_type(&mut bytes, result.value_type());
        }
        None => bytes.push(0),
    }
    set_bytes(context, operation, INSTRUCTION_BINDING_ATTR_V1, bytes)
}

pub(crate) fn inspect_module_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    source: &Gfx942HandoffV2,
) -> Result<(), InspectionErrorV1> {
    require_bytes(
        context,
        operation,
        MODULE_TARGET_POLICY_ATTR_V1,
        &target_policy(source.base().target()),
    )?;
    require_bytes(
        context,
        operation,
        MODULE_FLAGS_ATTR_V1,
        &module_flags(source.module().flags()),
    )?;
    require_bytes(
        context,
        operation,
        MODULE_METADATA_ATTR_V1,
        &module_metadata(source.module().named_metadata()),
    )
}

pub(crate) fn inspect_global_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    source: &GlobalV2,
) -> Result<(), InspectionErrorV1> {
    require_bytes(
        context,
        operation,
        GLOBAL_POLICY_ATTR_V1,
        &global_policy(source),
    )
}

pub(crate) fn inspect_intrinsic_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    source: &IntrinsicReferenceV2,
) -> Result<(), InspectionErrorV1> {
    require_bytes(
        context,
        operation,
        INTRINSIC_POLICY_ATTR_V1,
        &[POLICY_VERSION_V1, intrinsic_tag(source.intrinsic())],
    )
}

pub(crate) fn inspect_function_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    source: &FunctionV2,
) -> Result<(), InspectionErrorV1> {
    require_bytes(
        context,
        operation,
        FUNCTION_ABI_ATTR_V1,
        &function_abi(source),
    )?;
    require_bytes(
        context,
        operation,
        FUNCTION_ATTRIBUTES_ATTR_V1,
        &function_attributes(source),
    )?;
    require_bytes(
        context,
        operation,
        FUNCTION_PARAMETERS_ATTR_V1,
        &function_parameters(source),
    )?;
    require_bytes(
        context,
        operation,
        FUNCTION_BLOCKS_ATTR_V1,
        &function_blocks(source),
    )
}

pub(crate) fn inspect_instruction_binding(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    block: u32,
    ordinal: u32,
    result: Option<TypedValueV2>,
) -> Result<(), InspectionErrorV1> {
    let mut expected = vec![POLICY_VERSION_V1];
    put_u32(&mut expected, block);
    put_u32(&mut expected, ordinal);
    match result {
        Some(result) => {
            expected.push(1);
            put_u32(&mut expected, result.id().get());
            encode_value_type(&mut expected, result.value_type());
        }
        None => expected.push(0),
    }
    require_bytes(context, operation, INSTRUCTION_BINDING_ATTR_V1, &expected)
}

pub(crate) fn decode_module_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
) -> Result<ModuleGraphPolicyV1, InspectionErrorV1> {
    require_bytes(
        context,
        operation,
        MODULE_TARGET_POLICY_ATTR_V1,
        &target_policy(&Gfx942TargetPolicyV1::canonical()),
    )?;
    let flag_bytes = attribute_bytes(context, operation, MODULE_FLAGS_ATTR_V1)?;
    let mut flags = ReaderV1::new(&flag_bytes);
    flags.version()?;
    let flag_count = flags.length(MAX_MODULE_FLAGS_V2)?;
    let mut decoded_flags = Vec::new();
    for _ in 0..flag_count {
        decoded_flags.push(match flags.byte()? {
            1 => ModuleFlagV1::CodeObjectVersion6,
            2 => ModuleFlagV1::PicLevel2,
            3 => ModuleFlagV1::WcharSize4,
            _ => return Err(InspectionErrorV1::UnexpectedGraph),
        });
    }
    flags.finish()?;

    let metadata_bytes = attribute_bytes(context, operation, MODULE_METADATA_ATTR_V1)?;
    let mut metadata = ReaderV1::new(&metadata_bytes);
    metadata.version()?;
    let metadata_count = metadata.length(MAX_NAMED_METADATA_V2)?;
    let mut decoded_metadata = Vec::new();
    for _ in 0..metadata_count {
        decoded_metadata.push(match metadata.byte()? {
            1 => NamedMetadataV1::OpenClVersion2_0,
            2 => NamedMetadataV1::OpenClSpirVersion2_0,
            3 => NamedMetadataV1::ProducerIdentity(
                IdentityV1::new(metadata.array()?)
                    .map_err(|_| InspectionErrorV1::UnexpectedGraph)?,
            ),
            _ => return Err(InspectionErrorV1::UnexpectedGraph),
        });
    }
    metadata.finish()?;
    Ok(ModuleGraphPolicyV1 {
        flags: decoded_flags,
        named_metadata: decoded_metadata,
    })
}

pub(crate) fn decode_global_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
) -> Result<GlobalGraphPolicyV1, InspectionErrorV1> {
    let bytes = attribute_bytes(context, operation, GLOBAL_POLICY_ATTR_V1)?;
    let mut reader = ReaderV1::new(&bytes);
    reader.version()?;
    let id = GlobalIdV2::new(reader.u32()?);
    let mutable = reader.boolean()?;
    let section = match reader.byte()? {
        0 => None,
        1 => Some(reader.string(MAX_SYMBOL_BYTES_V2)?),
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    };
    reader.finish()?;
    Ok(GlobalGraphPolicyV1 {
        id,
        mutable,
        section,
    })
}

pub(crate) fn decode_intrinsic_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
) -> Result<IntrinsicV2, InspectionErrorV1> {
    let bytes = attribute_bytes(context, operation, INTRINSIC_POLICY_ATTR_V1)?;
    let mut reader = ReaderV1::new(&bytes);
    reader.version()?;
    let intrinsic = decode_intrinsic(reader.byte()?)?;
    reader.finish()?;
    Ok(intrinsic)
}

pub(crate) fn decode_function_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
) -> Result<FunctionGraphPolicyV1, InspectionErrorV1> {
    let abi_bytes = attribute_bytes(context, operation, FUNCTION_ABI_ATTR_V1)?;
    let mut abi = ReaderV1::new(&abi_bytes);
    abi.version()?;
    let id = FunctionIdV2::new(abi.u32()?);
    let kind = match abi.byte()? {
        1 => FunctionKindV2::Kernel,
        2 => FunctionKindV2::Helper,
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    };
    let calling_convention = match abi.byte()? {
        1 => CallingConventionV2::C,
        2 => CallingConventionV2::AmdGpuKernel,
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    };
    let entry = BlockIdV2::new(abi.u32()?);
    abi.finish()?;

    let function_attribute_bytes =
        attribute_bytes(context, operation, FUNCTION_ATTRIBUTES_ATTR_V1)?;
    let mut attributes = ReaderV1::new(&function_attribute_bytes);
    attributes.version()?;
    let attribute_count = attributes.length(MAX_FUNCTION_ATTRIBUTES_V2)?;
    let mut decoded_attributes = Vec::new();
    for _ in 0..attribute_count {
        decoded_attributes.push(decode_function_attribute(&mut attributes)?);
    }
    attributes.finish()?;

    let parameter_bytes = attribute_bytes(context, operation, FUNCTION_PARAMETERS_ATTR_V1)?;
    let mut parameters = ReaderV1::new(&parameter_bytes);
    parameters.version()?;
    let parameter_count = parameters.length(MAX_FUNCTION_PARAMETERS_V2)?;
    let mut decoded_parameters = Vec::new();
    for _ in 0..parameter_count {
        let id = ValueIdV2::new(parameters.u32()?);
        let name = parameters.string(MAX_SYMBOL_BYTES_V2)?;
        let value = TypedValueV2::new(id, decode_value_type(&mut parameters)?);
        let count = parameters.length(MAX_PARAMETER_ATTRIBUTES_V2)?;
        let mut parameter_attributes = Vec::new();
        for _ in 0..count {
            parameter_attributes.push(decode_parameter_attribute(&mut parameters)?);
        }
        decoded_parameters.push(
            FunctionParameterV2::new(value, &name, parameter_attributes)
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?,
        );
    }
    parameters.finish()?;

    let block_bytes = attribute_bytes(context, operation, FUNCTION_BLOCKS_ATTR_V1)?;
    let mut blocks = ReaderV1::new(&block_bytes);
    blocks.version()?;
    let block_count = blocks.length(MAX_FUNCTION_BLOCKS_V2)?;
    let mut decoded_blocks = Vec::new();
    for _ in 0..block_count {
        let id = BlockIdV2::new(blocks.u32()?);
        let phi_count = blocks.length(MAX_FUNCTION_PARAMETERS_V2)?;
        let mut phis = Vec::new();
        for _ in 0..phi_count {
            phis.push(TypedValueV2::new(
                ValueIdV2::new(blocks.u32()?),
                decode_value_type(&mut blocks)?,
            ));
        }
        decoded_blocks.push(BlockGraphPolicyV1 { id, phis });
    }
    blocks.finish()?;
    Ok(FunctionGraphPolicyV1 {
        id,
        kind,
        calling_convention,
        entry,
        attributes: decoded_attributes,
        parameters: decoded_parameters,
        blocks: decoded_blocks,
    })
}

pub(crate) fn decode_instruction_binding(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
) -> Result<InstructionGraphBindingV1, InspectionErrorV1> {
    let bytes = attribute_bytes(context, operation, INSTRUCTION_BINDING_ATTR_V1)?;
    let mut reader = ReaderV1::new(&bytes);
    reader.version()?;
    let block = BlockIdV2::new(reader.u32()?);
    let ordinal = reader.u32()?;
    let result = match reader.byte()? {
        0 => None,
        1 => Some(TypedValueV2::new(
            ValueIdV2::new(reader.u32()?),
            decode_value_type(&mut reader)?,
        )),
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    };
    reader.finish()?;
    Ok(InstructionGraphBindingV1 {
        block,
        ordinal,
        result,
    })
}

fn attribute_bytes(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    name: &str,
) -> Result<Vec<u8>, InspectionErrorV1> {
    let key = Identifier::try_from(name).map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
    let operation = operation.deref(context);
    let bytes = operation
        .attributes
        .get::<BytesAttr>(&key)
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    if bytes.as_ref().len() > MAX_GRAPH_POLICY_BYTES_V1 {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    Ok(bytes.as_ref().clone())
}

fn decode_function_attribute(
    reader: &mut ReaderV1<'_>,
) -> Result<FunctionAttributeV2, InspectionErrorV1> {
    Ok(match reader.byte()? {
        1 => FunctionAttributeV2::NoUnwind,
        2 => FunctionAttributeV2::AlwaysInline,
        3 => FunctionAttributeV2::NoInline,
        4 => FunctionAttributeV2::ReadNone,
        5 => FunctionAttributeV2::WillReturn,
        6 => FunctionAttributeV2::FlatWorkgroupSize(
            WorkgroupSizeRangeV1::new(reader.u16()?, reader.u16()?)
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?,
        ),
        7 => FunctionAttributeV2::WavesPerEu(
            WavesPerEuV1::new(reader.byte()?, reader.byte()?)
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?,
        ),
        8 => FunctionAttributeV2::DenormalFpMathF32Ieee,
        9 => FunctionAttributeV2::UnsafeFpMathDisabled,
        10 => FunctionAttributeV2::NoInfsFpMathDisabled,
        11 => FunctionAttributeV2::NoNansFpMathDisabled,
        12 => FunctionAttributeV2::NoSignedZerosFpMathDisabled,
        13 => FunctionAttributeV2::ApproxFuncFpMathDisabled,
        14 => FunctionAttributeV2::FpContractOff,
        15 => FunctionAttributeV2::RequiredWorkgroupSize([
            reader.u16()?,
            reader.u16()?,
            reader.u16()?,
        ]),
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    })
}

fn decode_parameter_attribute(
    reader: &mut ReaderV1<'_>,
) -> Result<ParameterAttributeV1, InspectionErrorV1> {
    Ok(match reader.byte()? {
        1 => ParameterAttributeV1::NoAlias,
        2 => ParameterAttributeV1::NoCapture,
        3 => ParameterAttributeV1::NonNull,
        4 => ParameterAttributeV1::ReadOnly,
        5 => ParameterAttributeV1::WriteOnly,
        6 => ParameterAttributeV1::Align(reader.u16()?),
        7 => ParameterAttributeV1::Dereferenceable(reader.u32()?),
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    })
}

fn decode_value_type(reader: &mut ReaderV1<'_>) -> Result<ValueTypeV2, InspectionErrorV1> {
    Ok(match reader.byte()? {
        1 => ValueTypeV2::Scalar(decode_scalar(reader.byte()?)?),
        2 => ValueTypeV2::Vector {
            element: decode_scalar(reader.byte()?)?,
            lanes: reader.byte()?,
        },
        3 => ValueTypeV2::Pointer {
            pointee: decode_scalar(reader.byte()?)?,
            address_space: decode_address_space(reader.byte()?)?,
        },
        4 => ValueTypeV2::ArrayPointer {
            element: decode_scalar(reader.byte()?)?,
            elements: reader.u16()?,
            address_space: decode_address_space(reader.byte()?)?,
        },
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    })
}

fn decode_intrinsic(tag: u8) -> Result<IntrinsicV2, InspectionErrorV1> {
    use fe2o3_llvm_handoff::AxisV2;
    Ok(match tag {
        1 => IntrinsicV2::AmdGpuWorkitemId(AxisV2::X),
        2 => IntrinsicV2::AmdGpuWorkitemId(AxisV2::Y),
        3 => IntrinsicV2::AmdGpuWorkitemId(AxisV2::Z),
        4 => IntrinsicV2::AmdGpuWorkgroupId(AxisV2::X),
        5 => IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Y),
        6 => IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Z),
        7 => IntrinsicV2::AmdGpuBarrier,
        8 => IntrinsicV2::FmaF32,
        9 => IntrinsicV2::SqrtF32,
        10 => IntrinsicV2::Trap,
        11 => IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k,
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    })
}

fn decode_scalar(tag: u8) -> Result<fe2o3_llvm_handoff::ScalarTypeV1, InspectionErrorV1> {
    use fe2o3_llvm_handoff::ScalarTypeV1;
    Ok(match tag {
        1 => ScalarTypeV1::I1,
        2 => ScalarTypeV1::I8,
        3 => ScalarTypeV1::I16,
        4 => ScalarTypeV1::I32,
        5 => ScalarTypeV1::I64,
        6 => ScalarTypeV1::F16,
        7 => ScalarTypeV1::Bf16,
        8 => ScalarTypeV1::F32,
        9 => ScalarTypeV1::F64,
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    })
}

fn decode_address_space(tag: u8) -> Result<fe2o3_llvm_handoff::AddressSpaceV1, InspectionErrorV1> {
    use fe2o3_llvm_handoff::AddressSpaceV1;
    Ok(match tag {
        1 => AddressSpaceV1::Flat,
        2 => AddressSpaceV1::Global,
        3 => AddressSpaceV1::Region,
        4 => AddressSpaceV1::Local,
        5 => AddressSpaceV1::Constant,
        6 => AddressSpaceV1::Private,
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    })
}

struct ReaderV1<'a> {
    remaining: &'a [u8],
}

impl<'a> ReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn version(&mut self) -> Result<(), InspectionErrorV1> {
        if self.byte()? != POLICY_VERSION_V1 {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, InspectionErrorV1> {
        let (&value, remaining) = self
            .remaining
            .split_first()
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        self.remaining = remaining;
        Ok(value)
    }

    fn boolean(&mut self) -> Result<bool, InspectionErrorV1> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(InspectionErrorV1::UnexpectedGraph),
        }
    }

    fn u16(&mut self) -> Result<u16, InspectionErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, InspectionErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], InspectionErrorV1> {
        let (value, remaining) = self
            .remaining
            .split_at_checked(N)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        self.remaining = remaining;
        value
            .try_into()
            .map_err(|_| InspectionErrorV1::UnexpectedGraph)
    }

    fn length(&mut self, maximum: usize) -> Result<usize, InspectionErrorV1> {
        let value = usize::try_from(self.u32()?).map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
        if value > maximum {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        Ok(value)
    }

    fn string(&mut self, maximum: usize) -> Result<String, InspectionErrorV1> {
        let length = self.length(maximum)?;
        let (value, remaining) = self
            .remaining
            .split_at_checked(length)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        self.remaining = remaining;
        core::str::from_utf8(value)
            .map(str::to_owned)
            .map_err(|_| InspectionErrorV1::UnexpectedGraph)
    }

    fn finish(self) -> Result<(), InspectionErrorV1> {
        if !self.remaining.is_empty() {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        Ok(())
    }
}

fn set_bytes(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    name: &str,
    bytes: Vec<u8>,
) -> Result<(), LoweringErrorV1> {
    let key = Identifier::try_from(name)
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph))?;
    operation
        .deref_mut(context)
        .attributes
        .set(key, BytesAttr::new(bytes));
    Ok(())
}

fn require_bytes(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    name: &str,
    expected: &[u8],
) -> Result<(), InspectionErrorV1> {
    let key = Identifier::try_from(name).map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
    let operation = operation.deref(context);
    let actual = operation
        .attributes
        .get::<BytesAttr>(&key)
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    if actual.as_ref().as_slice() != expected {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    Ok(())
}

fn target_policy(target: &Gfx942TargetPolicyV1) -> Vec<u8> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_str(&mut bytes, target.target_triple());
    put_str(&mut bytes, target.data_layout());
    put_str(&mut bytes, target.cpu());
    put_len(&mut bytes, target.features().len());
    for feature in target.features() {
        bytes.push(match feature.feature() {
            TargetFeatureV1::WavefrontSize32 => 1,
            TargetFeatureV1::WavefrontSize64 => 2,
            TargetFeatureV1::Xnack => 3,
        });
        bytes.push(u8::from(feature.enabled()));
    }
    bytes.extend_from_slice(&[1, 1, 1, 1]);
    bytes
}

fn module_flags(flags: &[ModuleFlagV1]) -> Vec<u8> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_len(&mut bytes, flags.len());
    for flag in flags {
        bytes.push(match flag {
            ModuleFlagV1::CodeObjectVersion6 => 1,
            ModuleFlagV1::PicLevel2 => 2,
            ModuleFlagV1::WcharSize4 => 3,
        });
    }
    bytes
}

fn module_metadata(metadata: &[NamedMetadataV1]) -> Vec<u8> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_len(&mut bytes, metadata.len());
    for value in metadata {
        match value {
            NamedMetadataV1::OpenClVersion2_0 => bytes.push(1),
            NamedMetadataV1::OpenClSpirVersion2_0 => bytes.push(2),
            NamedMetadataV1::ProducerIdentity(identity) => {
                bytes.push(3);
                bytes.extend_from_slice(identity.as_bytes());
            }
        }
    }
    bytes
}

fn global_policy(global: &GlobalV2) -> Vec<u8> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_u32(&mut bytes, global.id().get());
    bytes.push(u8::from(global.is_mutable()));
    match global.section() {
        Some(section) => {
            bytes.push(1);
            put_str(&mut bytes, section);
        }
        None => bytes.push(0),
    }
    bytes
}

fn function_abi(function: &FunctionV2) -> Vec<u8> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_u32(&mut bytes, function.id().get());
    bytes.push(match function.kind() {
        fe2o3_llvm_handoff::FunctionKindV2::Kernel => 1,
        fe2o3_llvm_handoff::FunctionKindV2::Helper => 2,
    });
    bytes.push(match function.calling_convention() {
        fe2o3_llvm_handoff::CallingConventionV2::C => 1,
        fe2o3_llvm_handoff::CallingConventionV2::AmdGpuKernel => 2,
    });
    put_u32(&mut bytes, function.entry().get());
    bytes
}

fn function_attributes(function: &FunctionV2) -> Vec<u8> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_len(&mut bytes, function.attributes().len());
    for attribute in function.attributes() {
        match attribute {
            FunctionAttributeV2::NoUnwind => bytes.push(1),
            FunctionAttributeV2::AlwaysInline => bytes.push(2),
            FunctionAttributeV2::NoInline => bytes.push(3),
            FunctionAttributeV2::ReadNone => bytes.push(4),
            FunctionAttributeV2::WillReturn => bytes.push(5),
            FunctionAttributeV2::FlatWorkgroupSize(range) => {
                bytes.push(6);
                put_u16(&mut bytes, range.minimum());
                put_u16(&mut bytes, range.maximum());
            }
            FunctionAttributeV2::WavesPerEu(range) => {
                bytes.push(7);
                bytes.extend_from_slice(&[range.minimum(), range.maximum()]);
            }
            FunctionAttributeV2::DenormalFpMathF32Ieee => bytes.push(8),
            FunctionAttributeV2::UnsafeFpMathDisabled => bytes.push(9),
            FunctionAttributeV2::NoInfsFpMathDisabled => bytes.push(10),
            FunctionAttributeV2::NoNansFpMathDisabled => bytes.push(11),
            FunctionAttributeV2::NoSignedZerosFpMathDisabled => bytes.push(12),
            FunctionAttributeV2::ApproxFuncFpMathDisabled => bytes.push(13),
            FunctionAttributeV2::FpContractOff => bytes.push(14),
            FunctionAttributeV2::RequiredWorkgroupSize(shape) => {
                bytes.push(15);
                for extent in shape {
                    put_u16(&mut bytes, *extent);
                }
            }
        }
    }
    bytes
}

fn function_parameters(function: &FunctionV2) -> Vec<u8> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_len(&mut bytes, function.parameters().len());
    for parameter in function.parameters() {
        put_u32(&mut bytes, parameter.value().id().get());
        put_str(&mut bytes, parameter.name());
        encode_value_type(&mut bytes, parameter.value().value_type());
        put_len(&mut bytes, parameter.attributes().len());
        for attribute in parameter.attributes() {
            encode_parameter_attribute(&mut bytes, *attribute);
        }
    }
    bytes
}

fn function_blocks(function: &FunctionV2) -> Vec<u8> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_len(&mut bytes, function.blocks().len());
    for block in function.blocks() {
        put_u32(&mut bytes, block.id().get());
        let phis = block
            .instructions()
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.kind(),
                    fe2o3_llvm_handoff::InstructionKindV2::Phi { .. }
                )
            })
            .collect::<Vec<_>>();
        put_len(&mut bytes, phis.len());
        for phi in phis {
            let result = phi.result().expect("validated phi result");
            put_u32(&mut bytes, result.id().get());
            encode_value_type(&mut bytes, result.value_type());
        }
    }
    bytes
}

fn encode_parameter_attribute(bytes: &mut Vec<u8>, attribute: ParameterAttributeV1) {
    match attribute {
        ParameterAttributeV1::NoAlias => bytes.push(1),
        ParameterAttributeV1::NoCapture => bytes.push(2),
        ParameterAttributeV1::NonNull => bytes.push(3),
        ParameterAttributeV1::ReadOnly => bytes.push(4),
        ParameterAttributeV1::WriteOnly => bytes.push(5),
        ParameterAttributeV1::Align(value) => {
            bytes.push(6);
            put_u16(bytes, value);
        }
        ParameterAttributeV1::Dereferenceable(value) => {
            bytes.push(7);
            put_u32(bytes, value);
        }
    }
}

fn encode_value_type(bytes: &mut Vec<u8>, value: fe2o3_llvm_handoff::ValueTypeV2) {
    use fe2o3_llvm_handoff::ValueTypeV2;
    match value {
        ValueTypeV2::Scalar(scalar) => {
            bytes.push(1);
            bytes.push(scalar_tag(scalar));
        }
        ValueTypeV2::Vector { element, lanes } => {
            bytes.push(2);
            bytes.extend_from_slice(&[scalar_tag(element), lanes]);
        }
        ValueTypeV2::Pointer {
            pointee,
            address_space,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&[scalar_tag(pointee), address_space_tag(address_space)]);
        }
        ValueTypeV2::ArrayPointer {
            element,
            elements,
            address_space,
        } => {
            bytes.push(4);
            bytes.push(scalar_tag(element));
            put_u16(bytes, elements);
            bytes.push(address_space_tag(address_space));
        }
    }
}

fn intrinsic_tag(intrinsic: fe2o3_llvm_handoff::IntrinsicV2) -> u8 {
    use fe2o3_llvm_handoff::{AxisV2, IntrinsicV2};
    match intrinsic {
        IntrinsicV2::AmdGpuWorkitemId(AxisV2::X) => 1,
        IntrinsicV2::AmdGpuWorkitemId(AxisV2::Y) => 2,
        IntrinsicV2::AmdGpuWorkitemId(AxisV2::Z) => 3,
        IntrinsicV2::AmdGpuWorkgroupId(AxisV2::X) => 4,
        IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Y) => 5,
        IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Z) => 6,
        IntrinsicV2::AmdGpuBarrier => 7,
        IntrinsicV2::FmaF32 => 8,
        IntrinsicV2::SqrtF32 => 9,
        IntrinsicV2::Trap => 10,
        IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k => 11,
    }
}

fn scalar_tag(scalar: fe2o3_llvm_handoff::ScalarTypeV1) -> u8 {
    use fe2o3_llvm_handoff::ScalarTypeV1;
    match scalar {
        ScalarTypeV1::I1 => 1,
        ScalarTypeV1::I8 => 2,
        ScalarTypeV1::I16 => 3,
        ScalarTypeV1::I32 => 4,
        ScalarTypeV1::I64 => 5,
        ScalarTypeV1::F16 => 6,
        ScalarTypeV1::Bf16 => 7,
        ScalarTypeV1::F32 => 8,
        ScalarTypeV1::F64 => 9,
    }
}

fn address_space_tag(address_space: fe2o3_llvm_handoff::AddressSpaceV1) -> u8 {
    use fe2o3_llvm_handoff::AddressSpaceV1;
    match address_space {
        AddressSpaceV1::Flat => 1,
        AddressSpaceV1::Global => 2,
        AddressSpaceV1::Region => 3,
        AddressSpaceV1::Local => 4,
        AddressSpaceV1::Constant => 5,
        AddressSpaceV1::Private => 6,
    }
}

fn put_len(bytes: &mut Vec<u8>, value: usize) {
    put_u32(
        bytes,
        u32::try_from(value).expect("validated handoff count"),
    );
}

fn put_str(bytes: &mut Vec<u8>, value: &str) {
    put_len(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
