use fe2o3_amdgcn_model::AddressSpace;
use fe2o3_llvm_handoff::{
    AddressSpaceV1, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallingConventionV2, EvidenceV2,
    ExecutableModuleV2, FloatBinaryOperationV2, FunctionAttributeV1, FunctionAttributeV2,
    FunctionIdV2, FunctionKindV2, FunctionParameterV2, FunctionV2, Gfx942HandoffV1,
    Gfx942HandoffV2, InstructionKindV2, InstructionV2, KernelValueTypeV1, ModuleFlagV1,
    ObligationKindV1, ReturnTypeV2, ScalarTypeV1, TerminatorV2, TypedValueV2, ValueIdV2,
    ValueTypeV2,
};
use fe2o3_pliron::{ContextIdentity, require_context_identity};
use pliron::{
    builtin::{
        op_interfaces::{SingleBlockRegionInterface, SymbolOpInterface},
        type_interfaces::FunctionTypeInterface,
        types::FP32Type,
    },
    common_traits::Named,
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::{TypeHandle, Typed},
    value::Value,
};
use pliron_llvm::{
    attributes::FastmathFlagsAttr,
    op_interfaces::{AlignableOpInterface, BinArithOp, FastMathFlags, LlvmSymbolName},
    ops::{FAddOp, FuncOp, LoadOp, ReturnOp, StoreOp},
    types::{PointerType, VoidType},
};

use crate::{
    lower::encode_receipt,
    model::{CanonicalLoweringReceiptV1, HandoffExtractionDiagnosticV2, admitted_obligations_v1},
};

const FUNCTION_ID: FunctionIdV2 = FunctionIdV2::new(0);
const ENTRY_BLOCK_ID: BlockIdV2 = BlockIdV2::new(0);
const INPUT_VALUE_ID: ValueIdV2 = ValueIdV2::new(0);
const OUTPUT_VALUE_ID: ValueIdV2 = ValueIdV2::new(1);
const ADDEND_VALUE_ID: ValueIdV2 = ValueIdV2::new(2);
const LOADED_VALUE_ID: ValueIdV2 = ValueIdV2::new(3);
const COMPUTED_VALUE_ID: ValueIdV2 = ValueIdV2::new(4);
const F32_ALIGNMENT: u32 = 4;

struct LiveFunctionFacts {
    symbol: String,
    parameter_names: [String; 3],
}

pub(crate) fn extract_handoff_v2(
    context: &Context,
    retained_identity: ContextIdentity,
    module_owner: ContextIdentity,
    module: &pliron::builtin::ops::ModuleOp,
    expected_module_name: &str,
    base: &Gfx942HandoffV1,
    receipt: &CanonicalLoweringReceiptV1,
) -> Result<Gfx942HandoffV2, HandoffExtractionDiagnosticV2> {
    validate_owner(context, retained_identity, module_owner, module)?;
    validate_policy_and_receipt(expected_module_name, base, receipt)?;
    let evidence = validate_evidence(base)?;
    let facts = inspect_live_graph(context, module, expected_module_name, base)?;
    build_handoff_v2(base, evidence, facts)
}

fn validate_owner(
    context: &Context,
    retained_identity: ContextIdentity,
    module_owner: ContextIdentity,
    module: &pliron::builtin::ops::ModuleOp,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let current = require_context_identity(context)
        .map_err(|_| HandoffExtractionDiagnosticV2::ContextIdentityInvalid)?;
    if current != retained_identity {
        return Err(HandoffExtractionDiagnosticV2::ContextIdentityInvalid);
    }
    if module_owner != current {
        return Err(HandoffExtractionDiagnosticV2::ForeignOwner);
    }
    let module_pointer = module.get_operation();
    let module_ref = module_pointer
        .try_deref(context)
        .map_err(|_| HandoffExtractionDiagnosticV2::StaleModule)?;
    drop(module_ref);
    verify_operation(module_pointer, context)
        .map_err(|_| HandoffExtractionDiagnosticV2::DialectVerificationFailed)
}

fn validate_policy_and_receipt(
    module_name: &str,
    base: &Gfx942HandoffV1,
    receipt: &CanonicalLoweringReceiptV1,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let expected_receipt = encode_receipt(module_name, base)
        .map_err(|_| HandoffExtractionDiagnosticV2::EvidenceMismatch)?;
    if expected_receipt != *receipt {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    }

    if base.kernels().len() != 1
        || base.module().flags() != [ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2]
        || !base.module().named_metadata().is_empty()
        || !base.module().device_libraries().is_empty()
    {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    }
    let kernel = &base.kernels()[0];
    if kernel.parameters().len() != 3
        || kernel.parameters()[0].value_type()
            != (KernelValueTypeV1::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            })
        || kernel.parameters()[1].value_type()
            != (KernelValueTypeV1::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            })
        || kernel.parameters()[2].value_type() != KernelValueTypeV1::Scalar(ScalarTypeV1::F32)
        || kernel
            .parameters()
            .iter()
            .any(|parameter| !parameter.attributes().is_empty())
        || !has_exact_function_attributes(kernel.function_attributes())
    {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    }
    Ok(())
}

fn has_exact_function_attributes(attributes: &[FunctionAttributeV1]) -> bool {
    attributes.len() == 9
        && attributes
            .iter()
            .filter(|attribute| matches!(attribute, FunctionAttributeV1::NoUnwind))
            .count()
            == 1
        && attributes
            .iter()
            .filter(|attribute| {
                matches!(
                    attribute,
                    FunctionAttributeV1::FlatWorkgroupSize(range)
                        if range.minimum() == 64 && range.maximum() == 64
                )
            })
            .count()
            == 1
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::DenormalFpMathF32Ieee))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::UnsafeFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::NoInfsFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::NoNansFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::NoSignedZerosFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::ApproxFuncFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::FpContractOff))
}

fn validate_evidence(base: &Gfx942HandoffV1) -> Result<EvidenceV2, HandoffExtractionDiagnosticV2> {
    let [origin] = base.origins() else {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    };
    if origin.kind() != fe2o3_llvm_handoff::OriginKindV1::AmdgcnIr
        || origin.span().is_some()
        || base.kernels()[0].origin() != origin.identity()
        || base.obligations().len() != admitted_obligations_v1().len()
    {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    }

    for required in admitted_obligations_v1() {
        let matching = base
            .obligations()
            .iter()
            .copied()
            .filter(|obligation| obligation.kind() == *required)
            .collect::<Vec<_>>();
        let [obligation] = matching.as_slice() else {
            return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
        };
        let expected_subject = match required {
            ObligationKindV1::PreserveKernelAbi | ObligationKindV1::MaintainOriginCoverage => {
                base.stage_identities().semantic()
            }
            ObligationKindV1::PreserveAddressSpaces
            | ObligationKindV1::PreserveTargetFeatures
            | ObligationKindV1::PreserveCallingConvention
            | ObligationKindV1::PreserveFunctionAttributes
            | ObligationKindV1::PreserveModuleMetadata => base.stage_identities().target_plan(),
            ObligationKindV1::AuthenticateDeviceLibraries => {
                return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
            }
        };
        if obligation.origin() != origin.identity() || obligation.subject() != expected_subject {
            return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
        }
    }

    EvidenceV2::new(
        origin.identity(),
        base.obligations()
            .iter()
            .map(|obligation| obligation.identity())
            .collect(),
    )
    .map_err(|_| HandoffExtractionDiagnosticV2::EvidenceMismatch)
}

fn inspect_live_graph(
    context: &Context,
    module: &pliron::builtin::ops::ModuleOp,
    expected_module_name: &str,
    base: &Gfx942HandoffV1,
) -> Result<LiveFunctionFacts, HandoffExtractionDiagnosticV2> {
    validate_module_shape(context, module, expected_module_name)?;
    let module_body = module.get_body(context, 0);
    let module_operations = module_body.deref(context).iter(context).collect::<Vec<_>>();
    let [function_pointer] = module_operations.as_slice() else {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    };
    let function = Operation::get_op::<FuncOp>(*function_pointer, context)
        .ok_or(HandoffExtractionDiagnosticV2::OperationShapeMismatch)?;
    validate_function_shape(context, &function, *function_pointer, base)?;

    let symbol = function.get_symbol_name(context);
    if symbol.as_ref() != base.kernels()[0].symbol() || function.llvm_symbol_name(context).is_some()
    {
        return Err(HandoffExtractionDiagnosticV2::SymbolMismatch);
    }

    let entry = function
        .get_entry_block(context)
        .ok_or(HandoffExtractionDiagnosticV2::ControlFlowMismatch)?;
    let arguments = entry.deref(context).arguments().collect::<Vec<_>>();
    let [input, output, addend] = arguments.as_slice() else {
        return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
    };
    let parameter_names = validate_parameters(context, [*input, *output, *addend], base)?;

    let body = entry.deref(context).iter(context).collect::<Vec<_>>();
    let [load_pointer, add_pointer, store_pointer, return_pointer] = body.as_slice() else {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    };
    let load = Operation::get_op::<LoadOp>(*load_pointer, context)
        .ok_or(HandoffExtractionDiagnosticV2::OperationShapeMismatch)?;
    let add = Operation::get_op::<FAddOp>(*add_pointer, context)
        .ok_or(HandoffExtractionDiagnosticV2::OperationShapeMismatch)?;
    let store = Operation::get_op::<StoreOp>(*store_pointer, context)
        .ok_or(HandoffExtractionDiagnosticV2::OperationShapeMismatch)?;
    let return_op = Operation::get_op::<ReturnOp>(*return_pointer, context)
        .ok_or(HandoffExtractionDiagnosticV2::OperationShapeMismatch)?;

    validate_leaf_shape(context, *load_pointer, 1, 1)?;
    validate_leaf_shape(context, *add_pointer, 1, 2)?;
    validate_leaf_shape(context, *store_pointer, 0, 2)?;
    validate_leaf_shape(context, *return_pointer, 0, 0)?;
    if body
        .iter()
        .any(|operation| operation.deref(context).get_num_successors() != 0)
    {
        return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
    }

    if load.alignment(context) != Some(F32_ALIGNMENT)
        || store.alignment(context) != Some(F32_ALIGNMENT)
    {
        return Err(HandoffExtractionDiagnosticV2::AlignmentMismatch);
    }
    if load_pointer.deref(context).attributes.0.len() != 1
        || store_pointer.deref(context).attributes.0.len() != 1
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    if add.fast_math_flags(context) != FastmathFlagsAttr::default() {
        return Err(HandoffExtractionDiagnosticV2::StrictFpMismatch);
    }
    if add_pointer.deref(context).attributes.0.len() != 1
        || !return_pointer.deref(context).attributes.0.is_empty()
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }

    let loaded = load_pointer.deref(context).get_result(0);
    let computed = add_pointer.deref(context).get_result(0);
    require_f32_value(context, loaded)?;
    require_f32_value(context, computed)?;
    for operand in add_pointer.deref(context).operands() {
        require_f32_value(context, operand)?;
    }
    require_f32_value(context, store_pointer.deref(context).get_operand(0))?;

    if load_pointer.deref(context).get_operand(0) != *input
        || add.lhs(context) != loaded
        || add.rhs(context) != *addend
        || store_pointer.deref(context).get_operand(0) != computed
        || store_pointer.deref(context).get_operand(1) != *output
        || input.num_uses(context) != 1
        || output.num_uses(context) != 1
        || addend.num_uses(context) != 1
        || loaded.num_uses(context) != 1
        || computed.num_uses(context) != 1
    {
        return Err(HandoffExtractionDiagnosticV2::DefUseMismatch);
    }

    if return_op.retval(context).is_some()
        || entry.deref(context).get_terminator(context) != Some(*return_pointer)
        || entry.deref(context).num_succ(context) != 0
    {
        return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
    }

    Ok(LiveFunctionFacts {
        symbol: symbol.as_ref().to_owned(),
        parameter_names,
    })
}

fn validate_module_shape(
    context: &Context,
    module: &pliron::builtin::ops::ModuleOp,
    expected_module_name: &str,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let module_pointer = module.get_operation();
    let operation = module_pointer.deref(context);
    if operation.get_num_results() != 0
        || operation.get_num_operands() != 0
        || operation.get_num_successors() != 0
        || operation.num_regions() != 1
        || operation.attributes.0.len() != 1
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    drop(operation);
    if module.get_symbol_name(context).as_ref() != expected_module_name {
        return Err(HandoffExtractionDiagnosticV2::SymbolMismatch);
    }
    let body = module.get_body(context, 0);
    let body_ref = body.deref(context);
    if body_ref.get_num_arguments() != 0
        || !body_ref.attributes.0.is_empty()
        || body_ref.given_name(context).is_some()
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    Ok(())
}

fn validate_function_shape(
    context: &Context,
    function: &FuncOp,
    function_pointer: Ptr<Operation>,
    base: &Gfx942HandoffV1,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let operation = function_pointer.deref(context);
    if operation.get_num_results() != 0
        || operation.get_num_operands() != 0
        || operation.get_num_successors() != 0
        || operation.num_regions() != 1
        || operation.attributes.0.len() != 2
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    let region = operation.get_region(0);
    drop(operation);
    let blocks = region.deref(context).iter(context).collect::<Vec<_>>();
    let [entry] = blocks.as_slice() else {
        return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
    };
    if entry
        .deref(context)
        .given_name(context)
        .as_ref()
        .map(AsRef::as_ref)
        != Some("entry")
        || entry.deref(context).attributes.0.len() != 1
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }

    let function_type = function.get_type(context);
    let function_type_ref = function_type.deref(context);
    if function_type_ref.is_var_arg()
        || function_type_ref
            .result_type()
            .deref(context)
            .downcast_ref::<VoidType>()
            .is_none()
    {
        return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
    }
    let argument_types = function_type_ref.arg_types();
    drop(function_type_ref);
    let [input, output, addend] = argument_types.as_slice() else {
        return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
    };
    require_global_pointer_type(context, *input)?;
    require_global_pointer_type(context, *output)?;
    require_f32_type(context, *addend)?;

    if base.kernels()[0].function_attributes().len() != 9 {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    }
    Ok(())
}

fn validate_parameters(
    context: &Context,
    arguments: [Value; 3],
    base: &Gfx942HandoffV1,
) -> Result<[String; 3], HandoffExtractionDiagnosticV2> {
    require_global_pointer_value(context, arguments[0])?;
    require_global_pointer_value(context, arguments[1])?;
    require_f32_value(context, arguments[2])?;

    let kernel = &base.kernels()[0];
    let mut names = Vec::with_capacity(arguments.len());
    for (argument, parameter) in arguments.into_iter().zip(kernel.parameters()) {
        let name = argument
            .given_name(context)
            .ok_or(HandoffExtractionDiagnosticV2::SymbolMismatch)?;
        if name.as_ref() != parameter.name() {
            return Err(HandoffExtractionDiagnosticV2::SymbolMismatch);
        }
        names.push(name.as_ref().to_owned());
    }
    names
        .try_into()
        .map_err(|_| HandoffExtractionDiagnosticV2::SymbolMismatch)
}

fn validate_leaf_shape(
    context: &Context,
    operation: Ptr<Operation>,
    results: usize,
    operands: usize,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let operation = operation.deref(context);
    if operation.get_num_results() != results
        || operation.get_num_operands() != operands
        || operation.num_regions() != 0
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    Ok(())
}

fn require_global_pointer_value(
    context: &Context,
    value: Value,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    require_global_pointer_type(context, value.get_type(context))
}

fn require_global_pointer_type(
    context: &Context,
    value_type: TypeHandle,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let value_type = value_type.deref(context);
    let pointer = value_type
        .downcast_ref::<PointerType>()
        .ok_or(HandoffExtractionDiagnosticV2::TypeMismatch)?;
    if pointer.address_space() != AddressSpace::Global.llvm_id() {
        return Err(HandoffExtractionDiagnosticV2::AddressSpaceMismatch);
    }
    Ok(())
}

fn require_f32_value(context: &Context, value: Value) -> Result<(), HandoffExtractionDiagnosticV2> {
    require_f32_type(context, value.get_type(context))
}

fn require_f32_type(
    context: &Context,
    value_type: TypeHandle,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    if value_type
        .deref(context)
        .downcast_ref::<FP32Type>()
        .is_none()
    {
        return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
    }
    Ok(())
}

fn build_handoff_v2(
    base: &Gfx942HandoffV1,
    evidence: EvidenceV2,
    facts: LiveFunctionFacts,
) -> Result<Gfx942HandoffV2, HandoffExtractionDiagnosticV2> {
    let pointer_type = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let parameters = [
        (INPUT_VALUE_ID, pointer_type),
        (OUTPUT_VALUE_ID, pointer_type),
        (ADDEND_VALUE_ID, f32_type),
    ]
    .into_iter()
    .zip(facts.parameter_names)
    .map(|((id, value_type), name)| {
        FunctionParameterV2::new(TypedValueV2::new(id, value_type), &name, vec![])
            .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)
    })
    .collect::<Result<Vec<_>, _>>()?;

    let load = InstructionV2::new(
        Some(TypedValueV2::new(LOADED_VALUE_ID, f32_type)),
        InstructionKindV2::Load {
            pointer: INPUT_VALUE_ID,
            value_type: ScalarTypeV1::F32,
            alignment: F32_ALIGNMENT as u16,
        },
        evidence.clone(),
    )
    .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)?;
    let add = InstructionV2::new(
        Some(TypedValueV2::new(COMPUTED_VALUE_ID, f32_type)),
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Add),
            left: LOADED_VALUE_ID,
            right: ADDEND_VALUE_ID,
        },
        evidence.clone(),
    )
    .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)?;
    let store = InstructionV2::new(
        None,
        InstructionKindV2::Store {
            pointer: OUTPUT_VALUE_ID,
            value: COMPUTED_VALUE_ID,
            value_type: ScalarTypeV1::F32,
            alignment: F32_ALIGNMENT as u16,
        },
        evidence.clone(),
    )
    .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)?;
    let block = BasicBlockV2::new(
        ENTRY_BLOCK_ID,
        vec![load, add, store],
        TerminatorV2::Return(None),
    );
    let function = FunctionV2::new(
        FUNCTION_ID,
        &facts.symbol,
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        parameters,
        base.kernels()[0]
            .function_attributes()
            .iter()
            .copied()
            .map(FunctionAttributeV2::from)
            .collect(),
        ENTRY_BLOCK_ID,
        vec![block],
        evidence,
    )
    .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)?;
    let module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        vec![],
        vec![],
        vec![function],
    )
    .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)?;
    Gfx942HandoffV2::new(base.clone(), module)
        .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)
}
