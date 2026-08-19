use fe2o3_llvm_handoff::{
    FunctionAttributeV2, FunctionV2, Gfx942HandoffV2, GlobalV2, IntrinsicReferenceV2, ModuleFlagV1,
    NamedMetadataV1, ParameterAttributeV1, TargetFeatureV1,
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

pub(crate) fn install_module_policy(
    context: &Context,
    operation: pliron::context::Ptr<Operation>,
    source: &Gfx942HandoffV2,
) -> Result<(), LoweringErrorV1> {
    set_bytes(
        context,
        operation,
        MODULE_TARGET_POLICY_ATTR_V1,
        target_policy(source),
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
    result: Option<u32>,
) -> Result<(), LoweringErrorV1> {
    let mut bytes = vec![POLICY_VERSION_V1];
    put_u32(&mut bytes, block);
    put_u32(&mut bytes, ordinal);
    match result {
        Some(result) => {
            bytes.push(1);
            put_u32(&mut bytes, result);
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
        &target_policy(source),
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
    result: Option<u32>,
) -> Result<(), InspectionErrorV1> {
    let mut expected = vec![POLICY_VERSION_V1];
    put_u32(&mut expected, block);
    put_u32(&mut expected, ordinal);
    match result {
        Some(result) => {
            expected.push(1);
            put_u32(&mut expected, result);
        }
        None => expected.push(0),
    }
    require_bytes(context, operation, INSTRUCTION_BINDING_ATTR_V1, &expected)
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

fn target_policy(source: &Gfx942HandoffV2) -> Vec<u8> {
    let target = source.base().target();
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
