use std::collections::{BTreeMap, BTreeSet, HashMap};

use fe2o3_llvm_handoff::{
    AddressSpaceV1, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallTargetV2, CastOperationV2,
    ComparePredicateV2, ExecutableModuleV2, FloatBinaryOperationV2, FunctionAttributeV1,
    FunctionAttributeV2, FunctionIdV2, FunctionKindV2, FunctionV2, GENERAL_GEMM_LDS_ELEMENTS_V2,
    Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1, GlobalLinkageV2,
    GlobalV2, InstructionKindV2, InstructionV2, IntegerBinaryOperationV2, IntrinsicReferenceV2,
    IntrinsicV2, KernelEntryV1, KernelParameterV1, KernelValueTypeV1, MAX_FUNCTION_BLOCKS_V2,
    MAX_FUNCTIONS_V2, MAX_GLOBALS_V2, MAX_INSTRUCTIONS_PER_FUNCTION_V2, MAX_INTRINSICS_V2,
    MAX_SYMBOL_BYTES_V2, MAX_VALUES_PER_FUNCTION_V2, ModuleMetadataV1, ReturnTypeV2,
    ScalarConstantV2, ScalarTypeV1, TerminatorV2, TypedValueV2, ValueTypeV2,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::{BytesAttr, FPSingleAttr, IntegerAttr},
        op_interfaces::{
            AtMostOneRegionInterface, BranchOpInterface, CallOpCallable, CallOpInterface,
            SingleBlockRegionInterface, SymbolOpInterface,
        },
        type_interfaces::FunctionTypeInterface,
        types::{FP32Type, IntegerType},
    },
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::{TypeHandle, Typed},
    value::Value,
};
use pliron_llvm::{
    attributes::{
        FCmpPredicateAttr, FastmathFlagsAttr, ICmpPredicateAttr, IntegerOverflowFlagsAttr,
        LinkageAttr,
    },
    op_interfaces::{
        AlignableOpInterface, FastMathFlags, IntBinArithOpWithOverflowFlag, IsDeclaration, NNegFlag,
    },
    ops::{
        AShrOp, AddOp, AddressOfOp, AndOp, BrOp, CallOp, CondBrOp, ConstantOp, ExtractElementOp,
        FAddOp, FCmpOp, FDivOp, FMulOp, FPExtOp, FPToSIOp, FPToUIOp, FPTruncOp, FSubOp, FuncOp,
        GepIndex, GetElementPtrOp, GlobalOp, ICmpOp, InsertElementOp, LShrOp, LoadOp, MulOp, OrOp,
        PtrToIntOp, ReturnOp, SExtOp, SIToFPOp, ShlOp, StoreOp, SubOp, TruncOp, UIToFPOp,
        UnreachableOp, XorOp, ZExtOp, ZeroOp,
    },
    types::{ArrayType, PointerType, VectorType, VectorTypeKind, VoidType},
};

use crate::{
    graph_policy::{
        BlockGraphPolicyV1, FunctionGraphPolicyV1, InstructionGraphBindingV1,
        decode_function_policy, decode_global_policy, decode_instruction_binding,
        decode_intrinsic_policy, decode_module_policy,
    },
    lower::{intrinsic_function_type, intrinsic_symbol, value_type_matches},
    model::{InspectionErrorV1, LoweredAmdgcnPlironLlvmV1},
};

const MAX_MODULE_OPERATIONS_V1: usize = MAX_GLOBALS_V2 + MAX_INTRINSICS_V2 + MAX_FUNCTIONS_V2;

pub(crate) fn derive_graph_handoff(
    lowered: &LoweredAmdgcnPlironLlvmV1,
) -> Result<Gfx942HandoffV2, InspectionErrorV1> {
    let context = &lowered.context;
    let module = &lowered.module.module;
    let module_pointer = module.get_operation();
    module_pointer
        .try_deref(context)
        .map_err(|_| InspectionErrorV1::StaleModule)?;
    verify_operation(module_pointer, context)
        .map_err(|_| InspectionErrorV1::DialectVerification)?;
    let policy = decode_module_policy(context, module.get_operation(), &lowered.source)?;
    let operations = bounded_collect(
        module.get_body(context, 0).deref(context).iter(context),
        MAX_MODULE_OPERATIONS_V1,
    )?;

    let mut globals = Vec::new();
    let mut intrinsic_operations = Vec::new();
    let mut function_operations = Vec::new();
    for operation in operations {
        if Operation::get_op::<GlobalOp>(operation, context).is_some() {
            if globals.len() == MAX_GLOBALS_V2 {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            globals.push(derive_global(context, operation, &lowered.source)?);
        } else if let Some(function) = Operation::get_op::<FuncOp>(operation, context) {
            if function.is_declaration(context) {
                if intrinsic_operations.len() == MAX_INTRINSICS_V2 {
                    return Err(InspectionErrorV1::UnexpectedGraph);
                }
                intrinsic_operations.push(operation);
            } else {
                if function_operations.len() == MAX_FUNCTIONS_V2 {
                    return Err(InspectionErrorV1::UnexpectedGraph);
                }
                function_operations.push(operation);
            }
        } else {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
    }

    let mut intrinsics = Vec::new();
    for operation in intrinsic_operations {
        intrinsics.push(derive_intrinsic(context, operation, &lowered.source)?);
    }

    let mut function_symbols = BTreeMap::new();
    for operation in &function_operations {
        let function = Operation::get_op::<FuncOp>(*operation, context)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        let policy = decode_function_policy(context, *operation)?;
        let symbol_attribute = function.get_symbol_name(context);
        let symbol = symbol_attribute.as_ref();
        if symbol.len() > MAX_SYMBOL_BYTES_V2 {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        let symbol = symbol.to_owned();
        if function_symbols.insert(symbol, policy.id).is_some()
            || function_symbols
                .values()
                .filter(|id| **id == policy.id)
                .count()
                != 1
        {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
    }
    let intrinsic_symbols = intrinsics
        .iter()
        .map(|intrinsic| {
            (
                intrinsic_symbol(intrinsic.intrinsic()).to_owned(),
                intrinsic.intrinsic(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut functions = Vec::new();
    for operation in function_operations {
        let function = Operation::get_op::<FuncOp>(operation, context)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        let policy = decode_function_policy(context, operation)?;
        functions.push(derive_function(
            context,
            &function,
            policy,
            &globals,
            &intrinsic_symbols,
            &function_symbols,
            &lowered.source,
        )?);
    }

    let module = ExecutableModuleV2::new(
        policy.flags,
        policy.named_metadata,
        globals,
        intrinsics,
        functions,
    )
    .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
    let base = derive_graph_base(&lowered.source, &module)?;
    Gfx942HandoffV2::new(base, module).map_err(|_| InspectionErrorV1::UnexpectedGraph)
}

fn derive_graph_base(
    source: &Gfx942HandoffV2,
    module: &ExecutableModuleV2,
) -> Result<Gfx942HandoffV1, InspectionErrorV1> {
    let kernels = module
        .functions()
        .iter()
        .filter(|function| function.kind() == FunctionKindV2::Kernel)
        .map(derive_graph_kernel)
        .collect::<Result<Vec<_>, _>>()?;
    let metadata = ModuleMetadataV1::new(
        module.flags().to_vec(),
        module.named_metadata().to_vec(),
        source.base().module().device_libraries().to_vec(),
    )
    .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
    Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: *source.base().stage_identities(),
        target: Gfx942TargetPolicyV1::canonical(),
        kernels,
        module: metadata,
        origins: source.base().origins().to_vec(),
        obligations: source.base().obligations().to_vec(),
    })
    .map_err(|_| InspectionErrorV1::UnexpectedGraph)
}

fn derive_graph_kernel(function: &FunctionV2) -> Result<KernelEntryV1, InspectionErrorV1> {
    let parameters = function
        .parameters()
        .iter()
        .map(|parameter| {
            let value_type = match parameter.value().value_type() {
                ValueTypeV2::Scalar(scalar) => KernelValueTypeV1::Scalar(scalar),
                ValueTypeV2::Pointer {
                    pointee,
                    address_space,
                } => KernelValueTypeV1::Pointer {
                    pointee,
                    address_space,
                },
                _ => return Err(InspectionErrorV1::UnexpectedGraph),
            };
            KernelParameterV1::new(
                parameter.name(),
                value_type,
                parameter.attributes().to_vec(),
            )
            .map_err(|_| InspectionErrorV1::UnexpectedGraph)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let attributes = function
        .attributes()
        .iter()
        .filter_map(|attribute| match attribute {
            FunctionAttributeV2::NoUnwind => Some(Ok(FunctionAttributeV1::NoUnwind)),
            FunctionAttributeV2::FlatWorkgroupSize(range) => {
                Some(Ok(FunctionAttributeV1::FlatWorkgroupSize(*range)))
            }
            FunctionAttributeV2::WavesPerEu(range) => {
                Some(Ok(FunctionAttributeV1::WavesPerEu(*range)))
            }
            FunctionAttributeV2::DenormalFpMathF32Ieee => {
                Some(Ok(FunctionAttributeV1::DenormalFpMathF32Ieee))
            }
            FunctionAttributeV2::UnsafeFpMathDisabled => {
                Some(Ok(FunctionAttributeV1::UnsafeFpMathDisabled))
            }
            FunctionAttributeV2::NoInfsFpMathDisabled => {
                Some(Ok(FunctionAttributeV1::NoInfsFpMathDisabled))
            }
            FunctionAttributeV2::NoNansFpMathDisabled => {
                Some(Ok(FunctionAttributeV1::NoNansFpMathDisabled))
            }
            FunctionAttributeV2::NoSignedZerosFpMathDisabled => {
                Some(Ok(FunctionAttributeV1::NoSignedZerosFpMathDisabled))
            }
            FunctionAttributeV2::ApproxFuncFpMathDisabled => {
                Some(Ok(FunctionAttributeV1::ApproxFuncFpMathDisabled))
            }
            FunctionAttributeV2::FpContractOff => Some(Ok(FunctionAttributeV1::FpContractOff)),
            FunctionAttributeV2::RequiredWorkgroupSize(_) => None,
            FunctionAttributeV2::AlwaysInline
            | FunctionAttributeV2::NoInline
            | FunctionAttributeV2::ReadNone
            | FunctionAttributeV2::WillReturn => Some(Err(InspectionErrorV1::UnexpectedGraph)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    KernelEntryV1::new(
        function.symbol(),
        parameters,
        attributes,
        function.evidence().origin(),
    )
    .map_err(|_| InspectionErrorV1::UnexpectedGraph)
}

fn derive_global(
    context: &Context,
    operation: Ptr<Operation>,
    source: &Gfx942HandoffV2,
) -> Result<GlobalV2, InspectionErrorV1> {
    let global = Operation::get_op::<GlobalOp>(operation, context)
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    let policy = decode_global_policy(context, operation)?;
    let evidence = source
        .module()
        .globals()
        .iter()
        .find(|candidate| candidate.id() == policy.id)
        .map(|candidate| candidate.evidence().clone())
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    let linkage = match global.get_attr_llvm_global_linkage(context).as_deref() {
        Some(LinkageAttr::InternalLinkage) => GlobalLinkageV2::Internal,
        Some(LinkageAttr::ExternalLinkage) => GlobalLinkageV2::External,
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    };
    let address_space = decode_address_space(global.address_space(context))?;
    let alignment = global
        .alignment(context)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    if global.get_initializer_region(context).is_some() {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let symbol_attribute = global.get_symbol_name(context);
    let symbol = symbol_attribute.as_ref();
    if symbol.len() > MAX_SYMBOL_BYTES_V2 {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let symbol = symbol.to_owned();
    let (value_type, elements) = decode_global_type(context, global.get_type(context))?;
    match global.get_initializer_value(context) {
        Some(initializer) if initializer.downcast_ref::<BytesAttr>().is_some() => {
            let bytes = initializer.downcast_ref::<BytesAttr>().unwrap().as_ref();
            if value_type != ScalarTypeV1::I8
                || usize::from(elements.ok_or(InspectionErrorV1::UnexpectedGraph)?) != bytes.len()
                || policy.mutable
                || linkage != GlobalLinkageV2::Internal
                || address_space != AddressSpaceV1::Constant
            {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            GlobalV2::new_private_constant_bytes(
                policy.id,
                &symbol,
                policy
                    .section
                    .as_deref()
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?,
                bytes.clone(),
                alignment,
                evidence,
            )
            .map_err(|_| InspectionErrorV1::UnexpectedGraph)
        }
        Some(initializer) if elements.is_none() => GlobalV2::new(
            policy.id,
            &symbol,
            linkage,
            address_space,
            policy.mutable,
            value_type,
            Some(decode_constant(context, initializer)?),
            evidence,
        )
        .map_err(|_| InspectionErrorV1::UnexpectedGraph),
        None if elements == Some(GENERAL_GEMM_LDS_ELEMENTS_V2)
            && value_type == ScalarTypeV1::I16
            && linkage == GlobalLinkageV2::Internal
            && address_space == AddressSpaceV1::Local
            && policy.mutable
            && policy.section.is_none()
            && alignment == 16 =>
        {
            GlobalV2::new_lds_bf16_array_256(policy.id, &symbol, evidence)
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)
        }
        None if elements.is_none() && linkage == GlobalLinkageV2::External => GlobalV2::new(
            policy.id,
            &symbol,
            linkage,
            address_space,
            policy.mutable,
            value_type,
            None,
            evidence,
        )
        .map_err(|_| InspectionErrorV1::UnexpectedGraph),
        _ => Err(InspectionErrorV1::UnexpectedGraph),
    }
}

fn derive_intrinsic(
    context: &Context,
    operation: Ptr<Operation>,
    source: &Gfx942HandoffV2,
) -> Result<IntrinsicReferenceV2, InspectionErrorV1> {
    let declaration = Operation::get_op::<FuncOp>(operation, context)
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    let intrinsic = decode_intrinsic_policy(context, operation)?;
    if !declaration.is_declaration(context)
        || declaration.get_symbol_name(context).as_ref() != intrinsic_symbol(intrinsic)
        || declaration.get_type(context)
            != intrinsic_function_type(context, intrinsic)
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?
        || declaration
            .get_attr_llvm_function_linkage(context)
            .is_some()
    {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let evidence = source
        .module()
        .intrinsics()
        .iter()
        .find(|candidate| candidate.intrinsic() == intrinsic)
        .map(|candidate| candidate.evidence().clone())
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    Ok(IntrinsicReferenceV2::new(intrinsic, evidence))
}

#[allow(clippy::too_many_arguments)]
fn derive_function(
    context: &Context,
    function: &FuncOp,
    policy: FunctionGraphPolicyV1,
    globals: &[GlobalV2],
    intrinsic_symbols: &BTreeMap<String, IntrinsicV2>,
    function_symbols: &BTreeMap<String, FunctionIdV2>,
    source: &Gfx942HandoffV2,
) -> Result<FunctionV2, InspectionErrorV1> {
    let source_function = source
        .module()
        .functions()
        .iter()
        .find(|candidate| candidate.id() == policy.id)
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    if function.get_attr_llvm_function_linkage(context).is_some() {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let evidence = source_function.evidence().clone();
    let function_type = function.get_type(context);
    let function_type = function_type.deref(context);
    if function_type.is_var_arg() || function_type.arg_types().len() != policy.parameters.len() {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let return_type = decode_return_type(context, function_type.result_type())?;
    for (actual, expected) in function_type.arg_types().iter().zip(&policy.parameters) {
        if !value_type_matches(context, *actual, expected.value().value_type()) {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
    }
    drop(function_type);

    let region = function
        .get_region(context)
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    let actual_blocks =
        bounded_collect(region.deref(context).iter(context), MAX_FUNCTION_BLOCKS_V2)?;
    let ordered_policy = ordered_block_policy(&policy)?;
    if actual_blocks.len() != ordered_policy.len() {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let block_ids = actual_blocks
        .iter()
        .copied()
        .zip(&ordered_policy)
        .map(|(block, policy)| (block, policy.id))
        .collect::<HashMap<_, _>>();
    if block_ids.len() != actual_blocks.len() {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }

    let mut values = HashMap::new();
    let entry = *actual_blocks
        .first()
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    if function.get_entry_block(context) != Some(entry) {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    if entry.deref(context).get_num_arguments() != policy.parameters.len() {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    for (index, parameter) in policy.parameters.iter().enumerate() {
        define_value(
            context,
            &mut values,
            entry.deref(context).get_argument(index),
            parameter.value(),
        )?;
    }
    for (block, block_policy) in actual_blocks.iter().copied().zip(&ordered_policy) {
        if block == entry {
            if !block_policy.phis.is_empty() {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            continue;
        }
        if block.deref(context).get_num_arguments() != block_policy.phis.len() {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        for (index, phi) in block_policy.phis.iter().copied().enumerate() {
            define_value(
                context,
                &mut values,
                block.deref(context).get_argument(index),
                phi,
            )?;
        }
    }

    let mut operation_bindings = HashMap::new();
    for (block, block_policy) in actual_blocks.iter().copied().zip(&ordered_policy) {
        let operations = bounded_collect(
            block.deref(context).iter(context),
            MAX_INSTRUCTIONS_PER_FUNCTION_V2 + 1,
        )?;
        let terminator = block
            .deref(context)
            .get_terminator(context)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        for actual in operations {
            if actual == terminator {
                continue;
            }
            let binding = decode_instruction_binding(context, actual)?;
            if binding.block != block_policy.id
                || operation_bindings.insert(actual, binding).is_some()
            {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            match (actual.deref(context).get_num_results(), binding.result) {
                (1, Some(result)) => {
                    if values.len() == MAX_VALUES_PER_FUNCTION_V2 {
                        return Err(InspectionErrorV1::UnexpectedGraph);
                    }
                    define_value(
                        context,
                        &mut values,
                        actual.deref(context).get_result(0),
                        result,
                    )?;
                }
                (1, None)
                    if actual
                        .deref(context)
                        .get_type(0)
                        .deref(context)
                        .is::<VoidType>() => {}
                (0, None) => {}
                _ => return Err(InspectionErrorV1::UnexpectedGraph),
            }
        }
    }

    let mut blocks = Vec::new();
    for (block, block_policy) in actual_blocks.iter().copied().zip(&ordered_policy) {
        let mut instructions = derive_phis(
            context,
            block,
            block_policy,
            &actual_blocks,
            &block_ids,
            &values,
            source_function,
        )?;
        let mut ordinals = BTreeSet::new();
        let operations = bounded_collect(
            block.deref(context).iter(context),
            MAX_INSTRUCTIONS_PER_FUNCTION_V2 + 1,
        )?;
        let terminator = block
            .deref(context)
            .get_terminator(context)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        for actual in operations {
            if actual == terminator {
                continue;
            }
            let binding = *operation_bindings
                .get(&actual)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if !ordinals.insert(binding.ordinal) {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            let source_instruction = source_function
                .blocks()
                .iter()
                .find(|candidate| candidate.id() == block_policy.id)
                .and_then(|source_block| source_block.instructions().get(binding.ordinal as usize))
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let kind = derive_instruction_kind(
                context,
                actual,
                binding,
                &values,
                globals,
                intrinsic_symbols,
                function_symbols,
            )?;
            instructions.push(
                InstructionV2::new(binding.result, kind, source_instruction.evidence().clone())
                    .map_err(|_| InspectionErrorV1::UnexpectedGraph)?,
            );
        }
        if instructions.len() > MAX_INSTRUCTIONS_PER_FUNCTION_V2 {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        let terminator = derive_terminator(context, terminator, &block_ids, &values)?;
        blocks.push(BasicBlockV2::new(block_policy.id, instructions, terminator));
    }

    FunctionV2::new(
        policy.id,
        function.get_symbol_name(context).as_ref(),
        policy.kind,
        policy.calling_convention,
        return_type,
        policy.parameters,
        policy.attributes,
        policy.entry,
        blocks,
        evidence,
    )
    .map_err(|_| InspectionErrorV1::UnexpectedGraph)
}

fn ordered_block_policy(
    policy: &FunctionGraphPolicyV1,
) -> Result<Vec<BlockGraphPolicyV1>, InspectionErrorV1> {
    if policy.blocks.len() > MAX_FUNCTION_BLOCKS_V2 {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let mut result = Vec::new();
    let entry = policy
        .blocks
        .iter()
        .find(|block| block.id == policy.entry)
        .cloned()
        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    result.push(entry);
    result.extend(
        policy
            .blocks
            .iter()
            .filter(|block| block.id != policy.entry)
            .cloned(),
    );
    let unique = result.iter().map(|block| block.id).collect::<BTreeSet<_>>();
    if unique.len() != result.len() {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    Ok(result)
}

fn define_value(
    context: &Context,
    values: &mut HashMap<Value, TypedValueV2>,
    actual: Value,
    expected: TypedValueV2,
) -> Result<(), InspectionErrorV1> {
    if !value_type_matches(context, actual.get_type(context), expected.value_type())
        || values.insert(actual, expected).is_some()
        || values
            .iter()
            .any(|(value, prior)| *value != actual && prior.id() == expected.id())
    {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    Ok(())
}

fn derive_phis(
    context: &Context,
    target: Ptr<BasicBlock>,
    policy: &BlockGraphPolicyV1,
    blocks: &[Ptr<BasicBlock>],
    block_ids: &HashMap<Ptr<BasicBlock>, BlockIdV2>,
    values: &HashMap<Value, TypedValueV2>,
    source_function: &FunctionV2,
) -> Result<Vec<InstructionV2>, InspectionErrorV1> {
    let mut result = Vec::new();
    for (phi_index, phi) in policy.phis.iter().copied().enumerate() {
        let mut incoming = Vec::new();
        for &predecessor in blocks {
            let terminator = predecessor
                .deref(context)
                .get_terminator(context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let operation = terminator.deref(context);
            if operation.get_num_successors() > 2 {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            for successor_index in 0..operation.get_num_successors() {
                if operation.get_successor(successor_index) != target {
                    continue;
                }
                let operands = successor_operands(context, terminator, successor_index)?;
                let value = operands
                    .get(phi_index)
                    .and_then(|value| values.get(value))
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?;
                if incoming.len() == 2 * MAX_FUNCTION_BLOCKS_V2 {
                    return Err(InspectionErrorV1::UnexpectedGraph);
                }
                incoming.push((
                    value.id(),
                    *block_ids
                        .get(&predecessor)
                        .ok_or(InspectionErrorV1::UnexpectedGraph)?,
                ));
            }
        }
        incoming.sort_unstable_by_key(|(_, block)| *block);
        let evidence = source_function
            .blocks()
            .iter()
            .flat_map(|block| block.instructions())
            .find(|instruction| {
                instruction
                    .result()
                    .is_some_and(|value| value.id() == phi.id())
            })
            .map(|instruction| instruction.evidence().clone())
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        result.push(
            InstructionV2::new(Some(phi), InstructionKindV2::Phi { incoming }, evidence)
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?,
        );
    }
    Ok(result)
}

fn successor_operands(
    context: &Context,
    terminator: Ptr<Operation>,
    successor_index: usize,
) -> Result<Vec<Value>, InspectionErrorV1> {
    if terminator.deref(context).get_num_operands()
        > 1 + 2 * fe2o3_llvm_handoff::MAX_FUNCTION_PARAMETERS_V2
    {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    if let Some(branch) = Operation::get_op::<BrOp>(terminator, context) {
        return Ok(branch.successor_operands(context, successor_index));
    }
    if let Some(branch) = Operation::get_op::<CondBrOp>(terminator, context) {
        return Ok(branch.successor_operands(context, successor_index));
    }
    Err(InspectionErrorV1::UnexpectedGraph)
}

#[allow(clippy::too_many_arguments)]
fn derive_instruction_kind(
    context: &Context,
    actual: Ptr<Operation>,
    binding: InstructionGraphBindingV1,
    values: &HashMap<Value, TypedValueV2>,
    globals: &[GlobalV2],
    intrinsic_symbols: &BTreeMap<String, IntrinsicV2>,
    function_symbols: &BTreeMap<String, FunctionIdV2>,
) -> Result<InstructionKindV2, InspectionErrorV1> {
    let operation = actual.deref(context);
    if operation.num_regions() != 0 || operation.get_num_successors() != 0 {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let operand = |index: usize| {
        operation
            .operands()
            .nth(index)
            .and_then(|value| values.get(&value))
            .copied()
            .ok_or(InspectionErrorV1::UnexpectedGraph)
    };
    let binary = |operation| -> Result<InstructionKindV2, InspectionErrorV1> {
        if actual.deref(context).get_num_operands() != 2 {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        Ok(InstructionKindV2::Binary {
            operation,
            left: operand(0)?.id(),
            right: operand(1)?.id(),
        })
    };
    macro_rules! integer_binary {
        ($op:ty, $kind:expr) => {
            if let Some(op) = Operation::get_op::<$op>(actual, context) {
                if op.integer_overflow_flag(context) != IntegerOverflowFlagsAttr::default() {
                    return Err(InspectionErrorV1::UnexpectedGraph);
                }
                return binary(BinaryOperationV2::Integer($kind));
            }
        };
    }
    macro_rules! plain_binary {
        ($op:ty, $kind:expr) => {
            if Operation::get_op::<$op>(actual, context).is_some() {
                return binary(BinaryOperationV2::Integer($kind));
            }
        };
    }
    macro_rules! float_binary {
        ($op:ty, $kind:expr) => {
            if let Some(op) = Operation::get_op::<$op>(actual, context) {
                if op.fast_math_flags(context) != FastmathFlagsAttr::default() {
                    return Err(InspectionErrorV1::UnexpectedGraph);
                }
                return binary(BinaryOperationV2::Float($kind));
            }
        };
    }

    if let Some(constant) = Operation::get_op::<ConstantOp>(actual, context) {
        if operation.get_num_operands() != 0 {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        return Ok(InstructionKindV2::Constant(decode_constant(
            context,
            constant.get_value(context),
        )?));
    }
    if Operation::get_op::<ZeroOp>(actual, context).is_some() {
        if binding.result.is_none() {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        let ValueTypeV2::Vector { element, lanes: 4 } = binding.result.unwrap().value_type() else {
            return Err(InspectionErrorV1::UnexpectedGraph);
        };
        return Ok(InstructionKindV2::VectorZero {
            element_type: element,
        });
    }
    if let Some(address) = Operation::get_op::<AddressOfOp>(actual, context) {
        let symbol = address.get_global_name(context);
        let global = globals
            .iter()
            .find(|global| global.symbol() == symbol.as_ref())
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        return Ok(InstructionKindV2::GlobalAddress(global.id()));
    }
    integer_binary!(AddOp, IntegerBinaryOperationV2::Add);
    integer_binary!(SubOp, IntegerBinaryOperationV2::Subtract);
    integer_binary!(MulOp, IntegerBinaryOperationV2::Multiply);
    integer_binary!(ShlOp, IntegerBinaryOperationV2::ShiftLeft);
    plain_binary!(AndOp, IntegerBinaryOperationV2::And);
    plain_binary!(OrOp, IntegerBinaryOperationV2::Or);
    plain_binary!(XorOp, IntegerBinaryOperationV2::Xor);
    plain_binary!(LShrOp, IntegerBinaryOperationV2::LogicalShiftRight);
    plain_binary!(AShrOp, IntegerBinaryOperationV2::ArithmeticShiftRight);
    float_binary!(FAddOp, FloatBinaryOperationV2::Add);
    float_binary!(FSubOp, FloatBinaryOperationV2::Subtract);
    float_binary!(FMulOp, FloatBinaryOperationV2::Multiply);
    float_binary!(FDivOp, FloatBinaryOperationV2::Divide);

    if let Some(compare) = Operation::get_op::<ICmpOp>(actual, context) {
        let predicate = match compare.predicate(context) {
            ICmpPredicateAttr::EQ => ComparePredicateV2::IntegerEqual,
            ICmpPredicateAttr::NE => ComparePredicateV2::IntegerNotEqual,
            ICmpPredicateAttr::ULT => ComparePredicateV2::UnsignedLessThan,
            ICmpPredicateAttr::ULE => ComparePredicateV2::UnsignedLessOrEqual,
            ICmpPredicateAttr::SLT => ComparePredicateV2::SignedLessThan,
            ICmpPredicateAttr::SLE => ComparePredicateV2::SignedLessOrEqual,
            _ => return Err(InspectionErrorV1::UnexpectedGraph),
        };
        return Ok(InstructionKindV2::Compare {
            predicate,
            left: operand(0)?.id(),
            right: operand(1)?.id(),
        });
    }
    if let Some(compare) = Operation::get_op::<FCmpOp>(actual, context) {
        if compare.fast_math_flags(context) != FastmathFlagsAttr::default() {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        let predicate = match compare.predicate(context) {
            FCmpPredicateAttr::OEQ => ComparePredicateV2::OrderedEqual,
            FCmpPredicateAttr::ONE => ComparePredicateV2::OrderedNotEqual,
            FCmpPredicateAttr::OLT => ComparePredicateV2::OrderedLessThan,
            FCmpPredicateAttr::OLE => ComparePredicateV2::OrderedLessOrEqual,
            _ => return Err(InspectionErrorV1::UnexpectedGraph),
        };
        return Ok(InstructionKindV2::Compare {
            predicate,
            left: operand(0)?.id(),
            right: operand(1)?.id(),
        });
    }

    let cast_result = binding.result.map(TypedValueV2::value_type);
    macro_rules! cast {
        ($op:ty, $kind:expr) => {
            if Operation::get_op::<$op>(actual, context).is_some() {
                return Ok(InstructionKindV2::Cast {
                    operation: $kind,
                    value: operand(0)?.id(),
                    to: cast_result.ok_or(InspectionErrorV1::UnexpectedGraph)?,
                });
            }
        };
    }
    if let Some(cast) = Operation::get_op::<ZExtOp>(actual, context) {
        if cast.nneg(context) {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        return Ok(InstructionKindV2::Cast {
            operation: CastOperationV2::ZeroExtend,
            value: operand(0)?.id(),
            to: cast_result.ok_or(InspectionErrorV1::UnexpectedGraph)?,
        });
    }
    cast!(SExtOp, CastOperationV2::SignExtend);
    cast!(TruncOp, CastOperationV2::Truncate);
    cast!(FPExtOp, CastOperationV2::FloatExtend);
    cast!(FPTruncOp, CastOperationV2::FloatTruncate);
    if let Some(cast) = Operation::get_op::<UIToFPOp>(actual, context) {
        if cast.nneg(context) {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        return Ok(InstructionKindV2::Cast {
            operation: CastOperationV2::UnsignedIntToFloat,
            value: operand(0)?.id(),
            to: cast_result.ok_or(InspectionErrorV1::UnexpectedGraph)?,
        });
    }
    cast!(SIToFPOp, CastOperationV2::SignedIntToFloat);
    cast!(FPToUIOp, CastOperationV2::FloatToUnsignedInt);
    cast!(FPToSIOp, CastOperationV2::FloatToSignedInt);
    cast!(PtrToIntOp, CastOperationV2::PointerToInt);

    if let Some(gep) = Operation::get_op::<GetElementPtrOp>(actual, context) {
        let base = operand(0)?;
        if gep.src_elem_type(context) != source_element_type(context, base.value_type())? {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        if gep
            .get_attr_gep_indices(context)
            .is_none_or(|indices| indices.0.len() > fe2o3_llvm_handoff::MAX_GEP_INDICES_V2)
        {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        let mut indices = Vec::new();
        for index in gep.indices(context) {
            if indices.len() == fe2o3_llvm_handoff::MAX_GEP_INDICES_V2 {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            let GepIndex::Value(value) = index else {
                return Err(InspectionErrorV1::UnexpectedGraph);
            };
            indices.push(
                values
                    .get(&value)
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?
                    .id(),
            );
        }
        return Ok(InstructionKindV2::GetElementPtr {
            base: base.id(),
            indices,
        });
    }
    if let Some(load) = Operation::get_op::<LoadOp>(actual, context) {
        let alignment = decode_alignment(load.alignment(context))?;
        return match binding.result.unwrap().value_type() {
            ValueTypeV2::Scalar(value_type) => Ok(InstructionKindV2::Load {
                pointer: operand(0)?.id(),
                value_type,
                alignment,
            }),
            ValueTypeV2::Vector { element, lanes: 4 } => Ok(InstructionKindV2::VectorLoad4 {
                pointer: operand(0)?.id(),
                element_type: element,
                alignment,
            }),
            _ => Err(InspectionErrorV1::UnexpectedGraph),
        };
    }
    if let Some(store) = Operation::get_op::<StoreOp>(actual, context) {
        let stored = operand(0)?;
        let ValueTypeV2::Scalar(value_type) = stored.value_type() else {
            return Err(InspectionErrorV1::UnexpectedGraph);
        };
        return Ok(InstructionKindV2::Store {
            pointer: operand(1)?.id(),
            value: stored.id(),
            value_type,
            alignment: decode_alignment(store.alignment(context))?,
        });
    }
    if let Some(call) = Operation::get_op::<CallOp>(actual, context) {
        if operation.get_num_operands() > fe2o3_llvm_handoff::MAX_FUNCTION_PARAMETERS_V2 {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        if !call
            .get_attr_llvm_call_fastmath_flags(context)
            .is_none_or(|flags| *flags == FastmathFlagsAttr::default())
        {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        let CallOpCallable::Direct(symbol) = call.callee(context) else {
            return Err(InspectionErrorV1::UnexpectedGraph);
        };
        let symbol = symbol.as_ref();
        let target = if let Some(intrinsic) = intrinsic_symbols.get(symbol) {
            CallTargetV2::Intrinsic(*intrinsic)
        } else if let Some(function) = function_symbols.get(symbol) {
            CallTargetV2::Function(*function)
        } else {
            return Err(InspectionErrorV1::UnexpectedGraph);
        };
        let mut arguments = Vec::new();
        for argument in call.args(context) {
            if arguments.len() == fe2o3_llvm_handoff::MAX_FUNCTION_PARAMETERS_V2 {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            arguments.push(
                values
                    .get(&argument)
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?
                    .id(),
            );
        }
        return Ok(InstructionKindV2::Call { target, arguments });
    }
    if Operation::get_op::<InsertElementOp>(actual, context).is_some() {
        return Ok(InstructionKindV2::InsertElement {
            vector: operand(0)?.id(),
            element: operand(1)?.id(),
            index: operand(2)?.id(),
        });
    }
    if Operation::get_op::<ExtractElementOp>(actual, context).is_some() {
        return Ok(InstructionKindV2::ExtractElement {
            vector: operand(0)?.id(),
            index: operand(1)?.id(),
        });
    }
    Err(InspectionErrorV1::UnexpectedGraph)
}

fn derive_terminator(
    context: &Context,
    actual: Ptr<Operation>,
    block_ids: &HashMap<Ptr<BasicBlock>, BlockIdV2>,
    values: &HashMap<Value, TypedValueV2>,
) -> Result<TerminatorV2, InspectionErrorV1> {
    let operation = actual.deref(context);
    if operation.get_num_results() != 0 || operation.num_regions() != 0 {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    if Operation::get_op::<ReturnOp>(actual, context).is_some() {
        return match operation.get_num_operands() {
            0 if operation.get_num_successors() == 0 => Ok(TerminatorV2::Return(None)),
            1 if operation.get_num_successors() == 0 => Ok(TerminatorV2::Return(Some(
                values
                    .get(&operation.get_operand(0))
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?
                    .id(),
            ))),
            _ => Err(InspectionErrorV1::UnexpectedGraph),
        };
    }
    if Operation::get_op::<BrOp>(actual, context).is_some() {
        if operation.get_num_successors() != 1 {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        return Ok(TerminatorV2::Branch(
            *block_ids
                .get(&operation.get_successor(0))
                .ok_or(InspectionErrorV1::UnexpectedGraph)?,
        ));
    }
    if Operation::get_op::<CondBrOp>(actual, context).is_some() {
        if operation.get_num_successors() != 2 || operation.get_num_operands() == 0 {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        return Ok(TerminatorV2::ConditionalBranch {
            condition: values
                .get(&operation.get_operand(0))
                .ok_or(InspectionErrorV1::UnexpectedGraph)?
                .id(),
            then_block: *block_ids
                .get(&operation.get_successor(0))
                .ok_or(InspectionErrorV1::UnexpectedGraph)?,
            else_block: *block_ids
                .get(&operation.get_successor(1))
                .ok_or(InspectionErrorV1::UnexpectedGraph)?,
        });
    }
    if Operation::get_op::<UnreachableOp>(actual, context).is_some()
        && operation.get_num_operands() == 0
        && operation.get_num_successors() == 0
    {
        return Ok(TerminatorV2::Unreachable);
    }
    Err(InspectionErrorV1::UnexpectedGraph)
}

fn decode_constant(
    context: &Context,
    attribute: pliron::attribute::AttrObj,
) -> Result<ScalarConstantV2, InspectionErrorV1> {
    if let Some(integer) = attribute.downcast_ref::<IntegerAttr>() {
        let scalar = match integer.get_type().deref(context).width() {
            1 => ScalarTypeV1::I1,
            8 => ScalarTypeV1::I8,
            16 => ScalarTypeV1::I16,
            32 => ScalarTypeV1::I32,
            64 => ScalarTypeV1::I64,
            _ => return Err(InspectionErrorV1::UnexpectedGraph),
        };
        return ScalarConstantV2::new(scalar, integer.value().to_u64())
            .map_err(|_| InspectionErrorV1::UnexpectedGraph);
    }
    if let Some(single) = attribute.downcast_ref::<FPSingleAttr>() {
        return ScalarConstantV2::new(
            ScalarTypeV1::F32,
            u64::from(f32::from(single.clone()).to_bits()),
        )
        .map_err(|_| InspectionErrorV1::UnexpectedGraph);
    }
    Err(InspectionErrorV1::UnexpectedGraph)
}

fn decode_global_type(
    context: &Context,
    value: TypeHandle,
) -> Result<(ScalarTypeV1, Option<u16>), InspectionErrorV1> {
    if let Some(array) = value.deref(context).downcast_ref::<ArrayType>() {
        let elements =
            u16::try_from(array.size()).map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
        return Ok((
            decode_scalar_type(context, array.elem_type())?,
            Some(elements),
        ));
    }
    Ok((decode_scalar_type(context, value)?, None))
}

fn decode_return_type(
    context: &Context,
    value: TypeHandle,
) -> Result<ReturnTypeV2, InspectionErrorV1> {
    if value.deref(context).is::<VoidType>() {
        Ok(ReturnTypeV2::Void)
    } else {
        Ok(ReturnTypeV2::Value(decode_value_type(context, value)?))
    }
}

fn decode_value_type(
    context: &Context,
    value: TypeHandle,
) -> Result<ValueTypeV2, InspectionErrorV1> {
    let value_ref = value.deref(context);
    if let Some(pointer) = value_ref.downcast_ref::<PointerType>() {
        return Ok(ValueTypeV2::Pointer {
            pointee: ScalarTypeV1::I8,
            address_space: decode_address_space(pointer.address_space())?,
        });
    }
    if let Some(vector) = value_ref.downcast_ref::<VectorType>() {
        if vector.kind() != VectorTypeKind::Fixed {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        return Ok(ValueTypeV2::Vector {
            element: decode_scalar_type(context, vector.elem_type())?,
            lanes: u8::try_from(vector.num_elements())
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?,
        });
    }
    drop(value_ref);
    Ok(ValueTypeV2::Scalar(decode_scalar_type(context, value)?))
}

fn decode_scalar_type(
    context: &Context,
    value: TypeHandle,
) -> Result<ScalarTypeV1, InspectionErrorV1> {
    let value = value.deref(context);
    if let Some(integer) = value.downcast_ref::<IntegerType>() {
        return Ok(match integer.width() {
            1 => ScalarTypeV1::I1,
            8 => ScalarTypeV1::I8,
            16 => ScalarTypeV1::I16,
            32 => ScalarTypeV1::I32,
            64 => ScalarTypeV1::I64,
            _ => return Err(InspectionErrorV1::UnexpectedGraph),
        });
    }
    if value.is::<FP32Type>() {
        return Ok(ScalarTypeV1::F32);
    }
    Err(InspectionErrorV1::UnexpectedGraph)
}

fn source_element_type(
    context: &Context,
    value: ValueTypeV2,
) -> Result<TypeHandle, InspectionErrorV1> {
    match value {
        ValueTypeV2::Pointer { pointee, .. } => scalar_type(context, pointee),
        ValueTypeV2::ArrayPointer {
            element, elements, ..
        } => {
            Ok(ArrayType::get(context, scalar_type(context, element)?, u64::from(elements)).into())
        }
        _ => Err(InspectionErrorV1::UnexpectedGraph),
    }
}

fn scalar_type(context: &Context, scalar: ScalarTypeV1) -> Result<TypeHandle, InspectionErrorV1> {
    match scalar {
        ScalarTypeV1::I1 => {
            Ok(IntegerType::get(context, 1, pliron::builtin::types::Signedness::Signless).into())
        }
        ScalarTypeV1::I8 => {
            Ok(IntegerType::get(context, 8, pliron::builtin::types::Signedness::Signless).into())
        }
        ScalarTypeV1::I16 => {
            Ok(IntegerType::get(context, 16, pliron::builtin::types::Signedness::Signless).into())
        }
        ScalarTypeV1::I32 => {
            Ok(IntegerType::get(context, 32, pliron::builtin::types::Signedness::Signless).into())
        }
        ScalarTypeV1::I64 => {
            Ok(IntegerType::get(context, 64, pliron::builtin::types::Signedness::Signless).into())
        }
        ScalarTypeV1::F32 => Ok(FP32Type::get(context).into()),
        _ => Err(InspectionErrorV1::UnexpectedGraph),
    }
}

fn decode_address_space(value: u32) -> Result<AddressSpaceV1, InspectionErrorV1> {
    Ok(match value {
        0 => AddressSpaceV1::Flat,
        1 => AddressSpaceV1::Global,
        2 => AddressSpaceV1::Region,
        3 => AddressSpaceV1::Local,
        4 => AddressSpaceV1::Constant,
        5 => AddressSpaceV1::Private,
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    })
}

fn decode_alignment(value: Option<u32>) -> Result<u16, InspectionErrorV1> {
    value
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| value.is_power_of_two() && *value <= 256)
        .ok_or(InspectionErrorV1::UnexpectedGraph)
}

fn bounded_collect<T>(
    iterator: impl IntoIterator<Item = T>,
    maximum: usize,
) -> Result<Vec<T>, InspectionErrorV1> {
    let mut result = Vec::new();
    for value in iterator {
        if result.len() == maximum {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        result.push(value);
    }
    Ok(result)
}
