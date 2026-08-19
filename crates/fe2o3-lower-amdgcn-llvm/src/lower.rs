use std::{
    collections::BTreeMap,
    num::NonZero,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_amdgcn::{AdmittedAmdgcnPlironLlvmV1, admit_amdgcn_pliron_llvm_v1};
use fe2o3_llvm_handoff::{
    AddressSpaceV1, AxisV2, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallTargetV2,
    CastOperationV2, ComparePredicateV2, FloatBinaryOperationV2, FunctionV2, Gfx942HandoffV2,
    GlobalIdV2, GlobalLinkageV2, GlobalV2, InstructionKindV2, InstructionV2,
    IntegerBinaryOperationV2, IntrinsicReferenceV2, IntrinsicV2, ReturnTypeV2, ScalarConstantV2,
    ScalarTypeV1, TerminatorV2, ValueIdV2, ValueTypeV2,
};
use fe2o3_pliron::{ensure_context_identity, require_context_identity};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::{BytesAttr, FPSingleAttr, IntegerAttr},
        op_interfaces::{
            AtMostOneRegionInterface, BranchOpInterface, CallOpCallable, CallOpInterface,
            OneResultInterface, SingleBlockRegionInterface, SymbolOpInterface,
        },
        ops::ModuleOp,
        types::{FP32Type, IntegerType, Signedness},
    },
    context::{Context, Ptr},
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::{TypeHandle, Typed, TypedHandle},
    utils::apint::APInt,
    value::Value,
};
use pliron_llvm::{
    attributes::{
        FCmpPredicateAttr, FastmathFlagsAttr, ICmpPredicateAttr, IntegerOverflowFlagsAttr,
        LinkageAttr,
    },
    op_interfaces::{
        AlignableOpInterface, BinArithOp, CastOpInterface, CastOpWithNNegInterface, FastMathFlags,
        FloatBinArithOpWithFastMathFlags, IntBinArithOpWithOverflowFlag, IsDeclaration, NNegFlag,
    },
    ops::{
        AShrOp, AddOp, AddressOfOp, AndOp, BrOp, CallOp, CondBrOp, ConstantOp, ExtractElementOp,
        FAddOp, FCmpOp, FDivOp, FMulOp, FPExtOp, FPToSIOp, FPToUIOp, FPTruncOp, FSubOp, FuncOp,
        GepIndex, GetElementPtrOp, GlobalOp, ICmpOp, InsertElementOp, LShrOp, LoadOp, MulOp, OrOp,
        PtrToIntOp, ReturnOp, SExtOp, SIToFPOp, ShlOp, StoreOp, SubOp, TruncOp, UIToFPOp,
        UnreachableOp, XorOp, ZExtOp, ZeroOp,
    },
    types::{ArrayType, FuncType, PointerType, VectorType, VectorTypeKind, VoidType},
};
use sha2::{Digest as _, Sha256};

use crate::model::{
    CanonicalLoweringReceiptV1, CanonicalPlironLlvmGraphExportV1, ConstructionStageV1,
    GraphExportErrorV1, GraphExportIdentityV1, GraphExportRequestV1, InspectionErrorV1,
    LiveGraphInspectionV1, LoweredAmdgcnPlironLlvmV1, LoweringErrorV1, LoweringReceiptIdentityV1,
    MAX_LOWERING_RECEIPT_BYTES_V1, OwnedDialectModuleV1,
};

const MODULE_NAME_V1: &str = "fe2o3_amdgcn_pliron_llvm_v1";
const RECEIPT_MAGIC_V1: &[u8] = b"fe2o3.lower-amdgcn-llvm.receipt.v1\0";
const RECEIPT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.lower-amdgcn-llvm.identity.v1\0";
const WORKER_EXPORT_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.lower-amdgcn-llvm.worker-export.identity.v1\0";

/// Lowers one canonical gfx942 typed handoff into a private verified Pliron LLVM graph.
///
/// This bounded lane invokes no LLVM C API, COMGR API, compiler or linker
/// subprocess, object linker, loader, or runtime API.
pub fn lower_amdgcn_to_pliron_llvm_v1(
    source: &Gfx942HandoffV2,
) -> Result<LoweredAmdgcnPlironLlvmV1, LoweringErrorV1> {
    catch_unwind(AssertUnwindSafe(|| lower_inner(source)))
        .unwrap_or(Err(LoweringErrorV1::UpstreamPanicked))
}

fn lower_inner(source: &Gfx942HandoffV2) -> Result<LoweredAmdgcnPlironLlvmV1, LoweringErrorV1> {
    let admitted = admit_amdgcn_pliron_llvm_v1(source).map_err(LoweringErrorV1::Admission)?;
    let mut context = Context::new();
    let context_identity = ensure_context_identity(&mut context)
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::ContextIdentity))?;
    let module = build_module(&mut context, admitted)?;
    verify_operation(module.get_operation(), &context)
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::DialectVerification))?;
    let owned = OwnedDialectModuleV1 {
        owner: context_identity,
        module,
    };
    let inspection = inspect_module(&context, &owned, source)
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::DialectInspection))?;
    let receipt = encode_receipt(source, admitted.profile(), inspection)?;
    Ok(LoweredAmdgcnPlironLlvmV1 {
        context,
        module: owned,
        context_identity,
        source: source.clone(),
        source_identity: source.identity(),
        profile: admitted.profile(),
        inspection,
        receipt,
    })
}

pub(crate) fn inspect_lowered(
    lowered: &LoweredAmdgcnPlironLlvmV1,
) -> Result<LiveGraphInspectionV1, InspectionErrorV1> {
    catch_unwind(AssertUnwindSafe(|| {
        let current = require_context_identity(&lowered.context)
            .map_err(|_| InspectionErrorV1::ContextIdentity)?;
        if current != lowered.context_identity {
            return Err(InspectionErrorV1::ContextIdentity);
        }
        if lowered.module.owner != current {
            return Err(InspectionErrorV1::ForeignOwner);
        }
        inspect_module(&lowered.context, &lowered.module, &lowered.source)
    }))
    .unwrap_or(Err(InspectionErrorV1::UpstreamPanicked))
}

pub(crate) fn export_graph(
    lowered: &LoweredAmdgcnPlironLlvmV1,
    request: GraphExportRequestV1,
) -> Result<CanonicalPlironLlvmGraphExportV1, GraphExportErrorV1> {
    if request.source_identity != lowered.source_identity {
        return Err(GraphExportErrorV1::SourceIdentitySubstitution);
    }
    if request.receipt_identity != lowered.receipt.identity {
        return Err(GraphExportErrorV1::ReceiptIdentitySubstitution);
    }
    if lowered.source.identity() != lowered.source_identity {
        return Err(GraphExportErrorV1::SourceIdentitySubstitution);
    }

    let inspection = inspect_lowered(lowered).map_err(GraphExportErrorV1::Inspection)?;
    let receipt = encode_receipt(&lowered.source, lowered.profile, inspection)
        .map_err(|_| GraphExportErrorV1::ReceiptConstruction)?;
    if inspection != lowered.inspection || receipt != lowered.receipt {
        return Err(GraphExportErrorV1::LiveGraphSubstitution);
    }

    let identity: [u8; 32] = Sha256::new()
        .chain_update(WORKER_EXPORT_IDENTITY_DOMAIN_V1)
        .chain_update(lowered.source_identity.as_bytes())
        .chain_update(receipt.identity.as_bytes())
        .chain_update(inspection.graph_sha256)
        .finalize()
        .into();
    Ok(CanonicalPlironLlvmGraphExportV1 {
        source: lowered.source.clone(),
        receipt,
        inspection,
        identity: GraphExportIdentityV1(identity),
    })
}

fn build_module(
    context: &mut Context,
    admitted: AdmittedAmdgcnPlironLlvmV1<'_>,
) -> Result<ModuleOp, LoweringErrorV1> {
    let module_name = Identifier::try_from(MODULE_NAME_V1)
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph))?;
    let module = ModuleOp::new(context, module_name);
    let globals = admitted
        .handoff()
        .module()
        .globals()
        .iter()
        .map(|global| build_global(context, &module, global).map(|binding| (global.id(), binding)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let intrinsics = admitted
        .handoff()
        .module()
        .intrinsics()
        .iter()
        .map(|intrinsic| {
            build_intrinsic(context, &module, intrinsic)
                .map(|binding| (intrinsic.intrinsic(), binding))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    for function in admitted.handoff().module().functions() {
        build_function(context, &module, function, &globals, &intrinsics)?;
    }
    Ok(module)
}

#[derive(Clone)]
struct GlobalBinding {
    symbol: Identifier,
    address_space: u32,
}

#[derive(Clone)]
struct IntrinsicBinding {
    symbol: Identifier,
    function_type: TypedHandle<FuncType>,
}

fn build_intrinsic(
    context: &mut Context,
    module: &ModuleOp,
    source: &IntrinsicReferenceV2,
) -> Result<IntrinsicBinding, LoweringErrorV1> {
    let symbol = Identifier::try_from(intrinsic_symbol(source.intrinsic()))
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph))?;
    let function_type = intrinsic_function_type(context, source.intrinsic())?;
    let declaration = FuncOp::new(context, symbol.clone(), function_type);
    module.append_operation(context, declaration.get_operation(), 0);
    Ok(IntrinsicBinding {
        symbol,
        function_type,
    })
}

fn build_global(
    context: &mut Context,
    module: &ModuleOp,
    source: &GlobalV2,
) -> Result<GlobalBinding, LoweringErrorV1> {
    let symbol = Identifier::try_from(source.symbol())
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph))?;
    let value_type = global_value_type(context, source)?;
    let global = GlobalOp::new(context, symbol.clone(), value_type);
    global.set_attr_llvm_global_linkage(
        context,
        match source.linkage() {
            GlobalLinkageV2::Internal => LinkageAttr::InternalLinkage,
            GlobalLinkageV2::External => LinkageAttr::ExternalLinkage,
        },
    );
    let address_space = address_space_id(source.address_space());
    global.set_address_space(context, address_space);
    global.set_alignment(context, u32::from(source.alignment()));
    if let Some(bytes) = source.byte_initializer() {
        global.set_initializer_value(context, BytesAttr::new(bytes.to_vec()).into());
    }
    module.append_operation(context, global.get_operation(), 0);
    Ok(GlobalBinding {
        symbol,
        address_space,
    })
}

fn global_value_type(context: &Context, source: &GlobalV2) -> Result<TypeHandle, LoweringErrorV1> {
    let element = scalar_type(context, source.value_type())?;
    match source.array_elements() {
        Some(elements) => Ok(ArrayType::get(context, element, u64::from(elements)).into()),
        None => Ok(element),
    }
}

fn build_function(
    context: &mut Context,
    module: &ModuleOp,
    source: &FunctionV2,
    globals: &BTreeMap<GlobalIdV2, GlobalBinding>,
    intrinsics: &BTreeMap<IntrinsicV2, IntrinsicBinding>,
) -> Result<(), LoweringErrorV1> {
    let symbol = Identifier::try_from(source.symbol())
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph))?;
    let arguments = source
        .parameters()
        .iter()
        .map(|parameter| type_for(context, parameter.value().value_type()))
        .collect::<Result<Vec<_>, _>>()?;
    let function_type = FuncType::get(context, VoidType::get(context).into(), arguments, false);
    let function = FuncOp::new(context, symbol, function_type);
    module.append_operation(context, function.get_operation(), 0);

    let entry = function.get_or_create_entry_block(context);
    let region = function
        .get_region(context)
        .ok_or(LoweringErrorV1::Construction(
            ConstructionStageV1::DialectGraph,
        ))?;
    let mut blocks = BTreeMap::new();
    blocks.insert(source.entry(), entry);
    for block in source.blocks() {
        if block.id() == source.entry() {
            continue;
        }
        let phi_types = phi_result_types(block)
            .map(|value_type| type_for(context, value_type))
            .collect::<Result<Vec<_>, _>>()?;
        let label = Identifier::try_from(format!("bb_{}", block.id().get()).as_str())
            .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph))?;
        let target = BasicBlock::new(context, Some(label), phi_types);
        target.insert_at_back(region, context);
        blocks.insert(block.id(), target);
    }

    let mut values = BTreeMap::new();
    let value_types = collect_value_types(source)?;
    for (index, parameter) in source.parameters().iter().enumerate() {
        values.insert(
            parameter.value().id(),
            entry.deref(context).get_argument(index),
        );
    }
    for block in source.blocks() {
        if block.id() == source.entry() {
            if block
                .instructions()
                .iter()
                .any(|instruction| matches!(instruction.kind(), InstructionKindV2::Phi { .. }))
            {
                return Err(LoweringErrorV1::Construction(
                    ConstructionStageV1::DialectGraph,
                ));
            }
            continue;
        }
        let target = *blocks
            .get(&block.id())
            .ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))?;
        for (index, instruction) in block
            .instructions()
            .iter()
            .filter(|instruction| matches!(instruction.kind(), InstructionKindV2::Phi { .. }))
            .enumerate()
        {
            let result = instruction.result().ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))?;
            values.insert(result.id(), target.deref(context).get_argument(index));
        }
    }

    let ordered_blocks = ordered_blocks(source);
    for block in &ordered_blocks {
        let target = *blocks
            .get(&block.id())
            .ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))?;
        for instruction in block.instructions() {
            if matches!(instruction.kind(), InstructionKindV2::Phi { .. }) {
                continue;
            }
            let result = build_instruction(
                context,
                target,
                instruction.kind(),
                &values,
                &value_types,
                globals,
                intrinsics,
            )?;
            match (instruction.result(), result) {
                (Some(expected), Some(value)) => {
                    values.insert(expected.id(), value);
                }
                (None, None) => {}
                _ => {
                    return Err(LoweringErrorV1::Construction(
                        ConstructionStageV1::DialectGraph,
                    ));
                }
            }
        }
    }
    for block in ordered_blocks {
        let target = *blocks
            .get(&block.id())
            .ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))?;
        build_terminator(context, target, block, source, &blocks, &values)?;
    }
    Ok(())
}

fn collect_value_types(
    source: &FunctionV2,
) -> Result<BTreeMap<ValueIdV2, ValueTypeV2>, LoweringErrorV1> {
    let mut types = BTreeMap::new();
    for parameter in source.parameters() {
        if types
            .insert(parameter.value().id(), parameter.value().value_type())
            .is_some()
        {
            return Err(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ));
        }
    }
    for block in source.blocks() {
        for instruction in block.instructions() {
            if let Some(result) = instruction.result()
                && types.insert(result.id(), result.value_type()).is_some()
            {
                return Err(LoweringErrorV1::Construction(
                    ConstructionStageV1::DialectGraph,
                ));
            }
        }
    }
    Ok(types)
}

fn ordered_blocks(source: &FunctionV2) -> Vec<&BasicBlockV2> {
    let mut blocks = Vec::with_capacity(source.blocks().len());
    if let Some(entry) = source
        .blocks()
        .iter()
        .find(|block| block.id() == source.entry())
    {
        blocks.push(entry);
    }
    blocks.extend(
        source
            .blocks()
            .iter()
            .filter(|block| block.id() != source.entry()),
    );
    blocks
}

fn phi_result_types(block: &BasicBlockV2) -> impl Iterator<Item = ValueTypeV2> + '_ {
    block.instructions().iter().filter_map(|instruction| {
        matches!(instruction.kind(), InstructionKindV2::Phi { .. })
            .then(|| instruction.result().map(|result| result.value_type()))
            .flatten()
    })
}

fn build_instruction(
    context: &mut Context,
    block: Ptr<BasicBlock>,
    instruction: &InstructionKindV2,
    values: &BTreeMap<ValueIdV2, Value>,
    value_types: &BTreeMap<ValueIdV2, ValueTypeV2>,
    globals: &BTreeMap<GlobalIdV2, GlobalBinding>,
    intrinsics: &BTreeMap<IntrinsicV2, IntrinsicBinding>,
) -> Result<Option<Value>, LoweringErrorV1> {
    let value = |id: ValueIdV2| {
        values
            .get(&id)
            .copied()
            .ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))
    };
    let result = match instruction {
        InstructionKindV2::Constant(constant) => {
            let attribute = constant_attribute(context, *constant)?;
            let operation = ConstantOp::new(context, attribute);
            Some(append_result(context, block, operation))
        }
        InstructionKindV2::VectorZero { element_type } => {
            let vector_type = type_for(
                context,
                ValueTypeV2::Vector {
                    element: *element_type,
                    lanes: 4,
                },
            )?;
            let operation = ZeroOp::new(context, vector_type);
            Some(append_result(context, block, operation))
        }
        InstructionKindV2::GlobalAddress(global) => {
            let binding = globals.get(global).ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))?;
            let operation =
                AddressOfOp::new(context, binding.symbol.clone(), binding.address_space);
            Some(append_result(context, block, operation))
        }
        InstructionKindV2::Binary {
            operation,
            left,
            right,
        } => Some(build_binary(
            context,
            block,
            *operation,
            value(*left)?,
            value(*right)?,
        )),
        InstructionKindV2::Compare {
            predicate,
            left,
            right,
        } => Some(build_compare(
            context,
            block,
            *predicate,
            value(*left)?,
            value(*right)?,
        )),
        InstructionKindV2::Cast {
            operation,
            value: operand,
            to,
        } => {
            let target_type = type_for(context, *to)?;
            Some(build_cast(
                context,
                block,
                *operation,
                value(*operand)?,
                target_type,
            ))
        }
        InstructionKindV2::GetElementPtr { base, indices } => {
            let base_value = value(*base)?;
            let source_type = source_element_type(
                context,
                *value_types.get(base).ok_or(LoweringErrorV1::Construction(
                    ConstructionStageV1::DialectGraph,
                ))?,
            )?;
            let indices = indices
                .iter()
                .map(|index| value(*index).map(GepIndex::Value))
                .collect::<Result<Vec<_>, _>>()?;
            let operation = GetElementPtrOp::new(context, base_value, indices, source_type);
            Some(append_result(context, block, operation))
        }
        InstructionKindV2::Load {
            pointer,
            value_type,
            alignment,
        } => {
            let result_type = scalar_type(context, *value_type)?;
            let operation = LoadOp::new(context, value(*pointer)?, result_type);
            operation.set_alignment(context, u32::from(*alignment));
            Some(append_result(context, block, operation))
        }
        InstructionKindV2::VectorLoad4 {
            pointer,
            element_type,
            alignment,
        } => {
            let result_type = type_for(
                context,
                ValueTypeV2::Vector {
                    element: *element_type,
                    lanes: 4,
                },
            )?;
            let operation = LoadOp::new(context, value(*pointer)?, result_type);
            operation.set_alignment(context, u32::from(*alignment));
            Some(append_result(context, block, operation))
        }
        InstructionKindV2::Store {
            pointer,
            value: stored,
            alignment,
            ..
        } => {
            let operation = StoreOp::new(context, value(*stored)?, value(*pointer)?);
            operation.set_alignment(context, u32::from(*alignment));
            operation.get_operation().insert_at_back(block, context);
            None
        }
        InstructionKindV2::InsertElement {
            vector,
            element,
            index,
        } => {
            let operation =
                InsertElementOp::new(context, value(*vector)?, value(*element)?, value(*index)?);
            Some(append_result(context, block, operation))
        }
        InstructionKindV2::ExtractElement { vector, index } => {
            let operation = ExtractElementOp::new(context, value(*vector)?, value(*index)?);
            Some(append_result(context, block, operation))
        }
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(intrinsic),
            arguments,
        } => {
            let binding = intrinsics
                .get(intrinsic)
                .ok_or(LoweringErrorV1::Construction(
                    ConstructionStageV1::DialectGraph,
                ))?;
            let arguments = arguments
                .iter()
                .map(|argument| value(*argument))
                .collect::<Result<Vec<_>, _>>()?;
            let operation = CallOp::new(
                context,
                CallOpCallable::Direct(binding.symbol.clone()),
                binding.function_type,
                arguments,
            );
            match intrinsic.signature().0 {
                ReturnTypeV2::Value(_) => Some(append_result(context, block, operation)),
                ReturnTypeV2::Void => {
                    operation.get_operation().insert_at_back(block, context);
                    None
                }
            }
        }
        InstructionKindV2::Call {
            target: CallTargetV2::Function(_),
            ..
        }
        | InstructionKindV2::Phi { .. } => {
            return Err(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ));
        }
    };
    Ok(result)
}

fn append_result<T>(context: &mut Context, block: Ptr<BasicBlock>, operation: T) -> Value
where
    T: Op + OneResultInterface,
{
    let result = operation.get_result(context);
    operation.get_operation().insert_at_back(block, context);
    result
}

fn build_binary(
    context: &mut Context,
    block: Ptr<BasicBlock>,
    operation: BinaryOperationV2,
    left: Value,
    right: Value,
) -> Value {
    macro_rules! plain {
        ($operation:ty) => {{
            let operation = <$operation>::new(context, left, right);
            append_result(context, block, operation)
        }};
    }
    macro_rules! overflow {
        ($operation:ty) => {{
            let operation = <$operation>::new_with_overflow_flag(
                context,
                left,
                right,
                IntegerOverflowFlagsAttr::default(),
            );
            append_result(context, block, operation)
        }};
    }
    macro_rules! float {
        ($operation:ty) => {{
            let operation = <$operation>::new_with_fast_math_flags(
                context,
                left,
                right,
                FastmathFlagsAttr::default(),
            );
            append_result(context, block, operation)
        }};
    }
    match operation {
        BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add) => overflow!(AddOp),
        BinaryOperationV2::Integer(IntegerBinaryOperationV2::Subtract) => overflow!(SubOp),
        BinaryOperationV2::Integer(IntegerBinaryOperationV2::Multiply) => overflow!(MulOp),
        BinaryOperationV2::Integer(IntegerBinaryOperationV2::And) => plain!(AndOp),
        BinaryOperationV2::Integer(IntegerBinaryOperationV2::Or) => plain!(OrOp),
        BinaryOperationV2::Integer(IntegerBinaryOperationV2::Xor) => plain!(XorOp),
        BinaryOperationV2::Integer(IntegerBinaryOperationV2::ShiftLeft) => overflow!(ShlOp),
        BinaryOperationV2::Integer(IntegerBinaryOperationV2::LogicalShiftRight) => plain!(LShrOp),
        BinaryOperationV2::Integer(IntegerBinaryOperationV2::ArithmeticShiftRight) => {
            plain!(AShrOp)
        }
        BinaryOperationV2::Float(FloatBinaryOperationV2::Add) => float!(FAddOp),
        BinaryOperationV2::Float(FloatBinaryOperationV2::Subtract) => float!(FSubOp),
        BinaryOperationV2::Float(FloatBinaryOperationV2::Multiply) => float!(FMulOp),
        BinaryOperationV2::Float(FloatBinaryOperationV2::Divide) => float!(FDivOp),
    }
}

fn build_compare(
    context: &mut Context,
    block: Ptr<BasicBlock>,
    predicate: ComparePredicateV2,
    left: Value,
    right: Value,
) -> Value {
    match predicate {
        ComparePredicateV2::IntegerEqual
        | ComparePredicateV2::IntegerNotEqual
        | ComparePredicateV2::UnsignedLessThan
        | ComparePredicateV2::UnsignedLessOrEqual
        | ComparePredicateV2::SignedLessThan
        | ComparePredicateV2::SignedLessOrEqual => {
            let predicate = match predicate {
                ComparePredicateV2::IntegerEqual => ICmpPredicateAttr::EQ,
                ComparePredicateV2::IntegerNotEqual => ICmpPredicateAttr::NE,
                ComparePredicateV2::UnsignedLessThan => ICmpPredicateAttr::ULT,
                ComparePredicateV2::UnsignedLessOrEqual => ICmpPredicateAttr::ULE,
                ComparePredicateV2::SignedLessThan => ICmpPredicateAttr::SLT,
                ComparePredicateV2::SignedLessOrEqual => ICmpPredicateAttr::SLE,
                _ => unreachable!("integer predicate arm"),
            };
            let operation = ICmpOp::new(context, predicate, left, right);
            append_result(context, block, operation)
        }
        ComparePredicateV2::OrderedEqual
        | ComparePredicateV2::OrderedNotEqual
        | ComparePredicateV2::OrderedLessThan
        | ComparePredicateV2::OrderedLessOrEqual => {
            let predicate = match predicate {
                ComparePredicateV2::OrderedEqual => FCmpPredicateAttr::OEQ,
                ComparePredicateV2::OrderedNotEqual => FCmpPredicateAttr::ONE,
                ComparePredicateV2::OrderedLessThan => FCmpPredicateAttr::OLT,
                ComparePredicateV2::OrderedLessOrEqual => FCmpPredicateAttr::OLE,
                _ => unreachable!("float predicate arm"),
            };
            let operation = FCmpOp::new(context, predicate, left, right);
            append_result(context, block, operation)
        }
    }
}

fn build_cast(
    context: &mut Context,
    block: Ptr<BasicBlock>,
    operation: CastOperationV2,
    value: Value,
    target: TypeHandle,
) -> Value {
    macro_rules! cast {
        ($operation:ty) => {{
            let operation = <$operation>::new(context, value, target);
            append_result(context, block, operation)
        }};
    }
    match operation {
        CastOperationV2::ZeroExtend => {
            let operation = ZExtOp::new_with_nneg(context, value, target, false);
            append_result(context, block, operation)
        }
        CastOperationV2::SignExtend => cast!(SExtOp),
        CastOperationV2::Truncate => cast!(TruncOp),
        CastOperationV2::FloatExtend => cast!(FPExtOp),
        CastOperationV2::FloatTruncate => cast!(FPTruncOp),
        CastOperationV2::UnsignedIntToFloat => {
            let operation = UIToFPOp::new_with_nneg(context, value, target, false);
            append_result(context, block, operation)
        }
        CastOperationV2::SignedIntToFloat => cast!(SIToFPOp),
        CastOperationV2::FloatToUnsignedInt => cast!(FPToUIOp),
        CastOperationV2::FloatToSignedInt => cast!(FPToSIOp),
        CastOperationV2::PointerToInt => cast!(PtrToIntOp),
    }
}

fn build_terminator(
    context: &mut Context,
    target: Ptr<BasicBlock>,
    block: &BasicBlockV2,
    function: &FunctionV2,
    blocks: &BTreeMap<BlockIdV2, Ptr<BasicBlock>>,
    values: &BTreeMap<ValueIdV2, Value>,
) -> Result<(), LoweringErrorV1> {
    let destination = |id: BlockIdV2| {
        blocks
            .get(&id)
            .copied()
            .ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))
    };
    let operation = match block.terminator() {
        TerminatorV2::Return(None) => ReturnOp::new(context, None).get_operation(),
        TerminatorV2::Branch(next) => BrOp::new(
            context,
            destination(*next)?,
            phi_operands(function, *next, block.id(), values)?,
        )
        .get_operation(),
        TerminatorV2::ConditionalBranch {
            condition,
            then_block,
            else_block,
        } => CondBrOp::new(
            context,
            *values.get(condition).ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))?,
            destination(*then_block)?,
            phi_operands(function, *then_block, block.id(), values)?,
            destination(*else_block)?,
            phi_operands(function, *else_block, block.id(), values)?,
        )
        .get_operation(),
        TerminatorV2::Unreachable => UnreachableOp::new(context).get_operation(),
        TerminatorV2::Return(Some(_)) => {
            return Err(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ));
        }
    };
    operation.insert_at_back(target, context);
    Ok(())
}

fn phi_operands(
    function: &FunctionV2,
    target: BlockIdV2,
    predecessor: BlockIdV2,
    values: &BTreeMap<ValueIdV2, Value>,
) -> Result<Vec<Value>, LoweringErrorV1> {
    let block = function
        .blocks()
        .iter()
        .find(|block| block.id() == target)
        .ok_or(LoweringErrorV1::Construction(
            ConstructionStageV1::DialectGraph,
        ))?;
    block
        .instructions()
        .iter()
        .filter_map(|instruction| match instruction.kind() {
            InstructionKindV2::Phi { incoming } => Some(incoming),
            _ => None,
        })
        .map(|incoming| {
            let (value, _) = incoming
                .iter()
                .find(|(_, block)| *block == predecessor)
                .ok_or(LoweringErrorV1::Construction(
                    ConstructionStageV1::DialectGraph,
                ))?;
            values
                .get(value)
                .copied()
                .ok_or(LoweringErrorV1::Construction(
                    ConstructionStageV1::DialectGraph,
                ))
        })
        .collect()
}

fn constant_attribute(
    context: &mut Context,
    constant: ScalarConstantV2,
) -> Result<pliron::attribute::AttrObj, LoweringErrorV1> {
    match constant.scalar_type() {
        scalar @ (ScalarTypeV1::I1
        | ScalarTypeV1::I8
        | ScalarTypeV1::I16
        | ScalarTypeV1::I32
        | ScalarTypeV1::I64) => {
            let width = scalar_width(scalar).ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))?;
            let width = NonZero::new(width as usize).ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))?;
            let integer = IntegerType::get(context, width.get() as u32, Signedness::Signless);
            Ok(IntegerAttr::new(integer, APInt::from_u64(constant.bits(), width)).into())
        }
        ScalarTypeV1::F32 => Ok(FPSingleAttr::from(f32::from_bits(constant.bits() as u32)).into()),
        ScalarTypeV1::F16 | ScalarTypeV1::Bf16 | ScalarTypeV1::F64 => Err(
            LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph),
        ),
    }
}

fn type_for(context: &Context, value_type: ValueTypeV2) -> Result<TypeHandle, LoweringErrorV1> {
    match value_type {
        ValueTypeV2::Scalar(scalar) => scalar_type(context, scalar),
        ValueTypeV2::Pointer { address_space, .. } => {
            Ok(PointerType::get(context, address_space_id(address_space)).into())
        }
        ValueTypeV2::Vector { element, lanes } => Ok(VectorType::get(
            context,
            scalar_type(context, element)?,
            u32::from(lanes),
            VectorTypeKind::Fixed,
        )
        .into()),
        ValueTypeV2::ArrayPointer { address_space, .. } => {
            Ok(PointerType::get(context, address_space_id(address_space)).into())
        }
    }
}

fn intrinsic_function_type(
    context: &Context,
    intrinsic: IntrinsicV2,
) -> Result<TypedHandle<FuncType>, LoweringErrorV1> {
    let (result, parameters) = intrinsic.signature();
    let result = match result {
        ReturnTypeV2::Void => VoidType::get(context).into(),
        ReturnTypeV2::Value(value_type) => type_for(context, value_type)?,
    };
    let parameters = parameters
        .into_iter()
        .map(|value_type| type_for(context, value_type))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FuncType::get(context, result, parameters, false))
}

const fn intrinsic_symbol(intrinsic: IntrinsicV2) -> &'static str {
    match intrinsic {
        IntrinsicV2::AmdGpuWorkitemId(axis) => match axis {
            AxisV2::X => "llvm_amdgcn_workitem_id_x",
            AxisV2::Y => "llvm_amdgcn_workitem_id_y",
            AxisV2::Z => "llvm_amdgcn_workitem_id_z",
        },
        IntrinsicV2::AmdGpuWorkgroupId(axis) => match axis {
            AxisV2::X => "llvm_amdgcn_workgroup_id_x",
            AxisV2::Y => "llvm_amdgcn_workgroup_id_y",
            AxisV2::Z => "llvm_amdgcn_workgroup_id_z",
        },
        IntrinsicV2::AmdGpuBarrier => "llvm_amdgcn_s_barrier",
        IntrinsicV2::FmaF32 => "llvm_fma_f32",
        IntrinsicV2::SqrtF32 => "llvm_sqrt_f32",
        IntrinsicV2::Trap => "llvm_trap",
        IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k => "llvm_amdgcn_mfma_f32_16x16x16bf16_1k",
    }
}

const fn intrinsic_tag(intrinsic: IntrinsicV2) -> u8 {
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

fn scalar_type(context: &Context, scalar: ScalarTypeV1) -> Result<TypeHandle, LoweringErrorV1> {
    match scalar {
        ScalarTypeV1::I1
        | ScalarTypeV1::I8
        | ScalarTypeV1::I16
        | ScalarTypeV1::I32
        | ScalarTypeV1::I64 => Ok(IntegerType::get(
            context,
            scalar_width(scalar).expect("integer scalar has a width"),
            Signedness::Signless,
        )
        .into()),
        ScalarTypeV1::F32 => Ok(FP32Type::get(context).into()),
        ScalarTypeV1::F16 | ScalarTypeV1::Bf16 | ScalarTypeV1::F64 => Err(
            LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph),
        ),
    }
}

const fn scalar_width(scalar: ScalarTypeV1) -> Option<u32> {
    match scalar {
        ScalarTypeV1::I1 => Some(1),
        ScalarTypeV1::I8 => Some(8),
        ScalarTypeV1::I16 => Some(16),
        ScalarTypeV1::I32 => Some(32),
        ScalarTypeV1::I64 => Some(64),
        ScalarTypeV1::F16 | ScalarTypeV1::Bf16 | ScalarTypeV1::F32 | ScalarTypeV1::F64 => None,
    }
}

const fn address_space_id(address_space: AddressSpaceV1) -> u32 {
    match address_space {
        AddressSpaceV1::Flat => 0,
        AddressSpaceV1::Global => 1,
        AddressSpaceV1::Region => 2,
        AddressSpaceV1::Local => 3,
        AddressSpaceV1::Constant => 4,
        AddressSpaceV1::Private => 5,
    }
}

fn source_element_type(
    context: &Context,
    pointer_type: ValueTypeV2,
) -> Result<TypeHandle, LoweringErrorV1> {
    match pointer_type {
        ValueTypeV2::Pointer { pointee, .. } => scalar_type(context, pointee),
        ValueTypeV2::ArrayPointer {
            element, elements, ..
        } => {
            Ok(ArrayType::get(context, scalar_type(context, element)?, u64::from(elements)).into())
        }
        ValueTypeV2::Scalar(_) | ValueTypeV2::Vector { .. } => Err(LoweringErrorV1::Construction(
            ConstructionStageV1::DialectGraph,
        )),
    }
}

fn inspect_module(
    context: &Context,
    owned: &OwnedDialectModuleV1,
    source: &Gfx942HandoffV2,
) -> Result<LiveGraphInspectionV1, InspectionErrorV1> {
    let module_pointer = owned.module.get_operation();
    module_pointer
        .try_deref(context)
        .map_err(|_| InspectionErrorV1::StaleModule)?;
    verify_operation(module_pointer, context)
        .map_err(|_| InspectionErrorV1::DialectVerification)?;
    let actual_module_operations = owned
        .module
        .get_body(context, 0)
        .deref(context)
        .iter(context)
        .collect::<Vec<_>>();
    let global_count = source.module().globals().len();
    let intrinsic_count = source.module().intrinsics().len();
    if actual_module_operations.len()
        != global_count + intrinsic_count + source.module().functions().len()
    {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let (actual_globals, remaining) = actual_module_operations.split_at(global_count);
    let (actual_intrinsics, actual_functions) = remaining.split_at(intrinsic_count);

    let mut facts = Vec::new();
    let mut global_count = 0_u32;
    let mut intrinsic_count = 0_u32;
    let mut function_count = 0_u32;
    let mut block_count = 0_u32;
    let mut block_argument_count = 0_u32;
    let mut operation_count = 0_u32;
    let mut strict_float = true;
    let mut exact_memory_alignment = true;
    for (source_global, actual) in source.module().globals().iter().zip(actual_globals) {
        inspect_global(
            context,
            *actual,
            source_global,
            &mut facts,
            &mut exact_memory_alignment,
        )?;
        global_count = global_count
            .checked_add(1)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    }
    for (source_intrinsic, actual) in source.module().intrinsics().iter().zip(actual_intrinsics) {
        inspect_intrinsic(context, *actual, source_intrinsic.intrinsic(), &mut facts)?;
        intrinsic_count = intrinsic_count
            .checked_add(1)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
    }
    for (source_function, actual) in source.module().functions().iter().zip(actual_functions) {
        let function = Operation::get_op::<FuncOp>(*actual, context)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        let source_symbol = Identifier::try_from(source_function.symbol())
            .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
        if function.get_symbol_name(context) != source_symbol {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        let expected_function_type = FuncType::get(
            context,
            VoidType::get(context).into(),
            source_function
                .parameters()
                .iter()
                .map(|parameter| {
                    type_for(context, parameter.value().value_type())
                        .map_err(|_| InspectionErrorV1::UnexpectedGraph)
                })
                .collect::<Result<Vec<_>, _>>()?,
            false,
        );
        if function.get_type(context) != expected_function_type || function.is_declaration(context)
        {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        function_count = function_count
            .checked_add(1)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        facts.push(1);
        let actual_blocks = function
            .get_region(context)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?
            .deref(context)
            .iter(context)
            .collect::<Vec<_>>();
        let source_blocks = ordered_blocks(source_function);
        if actual_blocks.len() != source_blocks.len() {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
        let block_bindings = source_blocks
            .iter()
            .zip(actual_blocks.iter())
            .map(|(source_block, actual_block)| (source_block.id(), *actual_block))
            .collect::<BTreeMap<_, _>>();
        let source_value_types =
            collect_value_types(source_function).map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
        let mut values = BTreeMap::new();
        for (source_block, actual_block) in source_blocks.iter().zip(actual_blocks.iter()) {
            if source_block.id() == source_function.entry() {
                for (index, parameter) in source_function.parameters().iter().enumerate() {
                    let actual_argument = actual_block.deref(context).get_argument(index);
                    if !value_type_matches(
                        context,
                        actual_argument.get_type(context),
                        parameter.value().value_type(),
                    ) {
                        return Err(InspectionErrorV1::UnexpectedGraph);
                    }
                    values.insert(parameter.value().id(), actual_argument);
                }
            } else {
                for (index, phi) in source_block
                    .instructions()
                    .iter()
                    .filter(|instruction| {
                        matches!(instruction.kind(), InstructionKindV2::Phi { .. })
                    })
                    .enumerate()
                {
                    let result = phi.result().ok_or(InspectionErrorV1::UnexpectedGraph)?;
                    let actual_argument = actual_block.deref(context).get_argument(index);
                    if !value_type_matches(
                        context,
                        actual_argument.get_type(context),
                        result.value_type(),
                    ) {
                        return Err(InspectionErrorV1::UnexpectedGraph);
                    }
                    values.insert(result.id(), actual_argument);
                }
            }
        }
        let mut pending_terminators = Vec::with_capacity(source_blocks.len());
        for (source_block, actual_block) in source_blocks
            .iter()
            .copied()
            .zip(actual_blocks.iter().copied())
        {
            block_count = block_count
                .checked_add(1)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let expected_arguments = if source_block.id() == source_function.entry() {
                source_function.parameters().len()
            } else {
                phi_result_types(source_block).count()
            };
            let actual_argument_count = actual_block.deref(context).arguments().count();
            if actual_argument_count != expected_arguments {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            block_argument_count = block_argument_count
                .checked_add(
                    u32::try_from(actual_argument_count)
                        .map_err(|_| InspectionErrorV1::UnexpectedGraph)?,
                )
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            facts.push(2);
            facts.extend_from_slice(&(actual_argument_count as u32).to_le_bytes());
            let actual_operations = actual_block
                .deref(context)
                .iter(context)
                .collect::<Vec<_>>();
            let expected_operations = source_block
                .instructions()
                .iter()
                .filter(|instruction| !matches!(instruction.kind(), InstructionKindV2::Phi { .. }))
                .count()
                + 1;
            if actual_operations.len() != expected_operations {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            let mut actual_operations = actual_operations.into_iter();
            for instruction in source_block.instructions() {
                if matches!(instruction.kind(), InstructionKindV2::Phi { .. }) {
                    facts.push(3);
                    let result = instruction
                        .result()
                        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
                    facts.extend_from_slice(&result.id().get().to_le_bytes());
                    continue;
                }
                let actual = actual_operations
                    .next()
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?;
                let result = inspect_instruction(
                    context,
                    actual,
                    instruction,
                    &values,
                    &source_value_types,
                    source.module().globals(),
                    &mut facts,
                    &mut strict_float,
                    &mut exact_memory_alignment,
                )?;
                match (instruction.result(), result) {
                    (Some(expected), Some(actual)) => {
                        values.insert(expected.id(), actual);
                    }
                    (None, None) => {}
                    _ => return Err(InspectionErrorV1::UnexpectedGraph),
                }
                operation_count = operation_count
                    .checked_add(1)
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            }
            let terminator = actual_operations
                .next()
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if actual_operations.next().is_some() {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            pending_terminators.push((source_block, terminator));
            operation_count = operation_count
                .checked_add(1)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        }
        for (source_block, terminator) in pending_terminators {
            inspect_terminator(
                context,
                terminator,
                source_block,
                source_function,
                &block_bindings,
                &values,
                &mut facts,
            )?;
        }
    }
    let graph_sha256: [u8; 32] = Sha256::digest(&facts).into();
    Ok(LiveGraphInspectionV1 {
        global_count,
        intrinsic_count,
        function_count,
        block_count,
        block_argument_count,
        operation_count,
        graph_sha256,
        strict_float,
        exact_memory_alignment,
    })
}

fn inspect_intrinsic(
    context: &Context,
    actual: Ptr<Operation>,
    expected: IntrinsicV2,
    facts: &mut Vec<u8>,
) -> Result<(), InspectionErrorV1> {
    let declaration =
        Operation::get_op::<FuncOp>(actual, context).ok_or(InspectionErrorV1::UnexpectedGraph)?;
    let symbol = Identifier::try_from(intrinsic_symbol(expected))
        .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
    let expected_type = intrinsic_function_type(context, expected)
        .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
    if declaration.get_symbol_name(context) != symbol
        || declaration.get_type(context) != expected_type
        || !declaration.is_declaration(context)
    {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    facts.push(6);
    facts.push(intrinsic_tag(expected));
    Ok(())
}

fn inspect_global(
    context: &Context,
    actual: Ptr<Operation>,
    expected: &GlobalV2,
    facts: &mut Vec<u8>,
    exact_memory_alignment: &mut bool,
) -> Result<(), InspectionErrorV1> {
    let global =
        Operation::get_op::<GlobalOp>(actual, context).ok_or(InspectionErrorV1::UnexpectedGraph)?;
    let symbol =
        Identifier::try_from(expected.symbol()).map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
    let linkage = match expected.linkage() {
        GlobalLinkageV2::Internal => LinkageAttr::InternalLinkage,
        GlobalLinkageV2::External => LinkageAttr::ExternalLinkage,
    };
    if global.get_symbol_name(context) != symbol
        || global.address_space(context) != address_space_id(expected.address_space())
        || global.get_attr_llvm_global_linkage(context).as_deref() != Some(&linkage)
    {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    if global.alignment(context) != Some(u32::from(expected.alignment())) {
        *exact_memory_alignment = false;
        return Err(InspectionErrorV1::UnexpectedGraph);
    }

    let global_type = global.get_type(context);
    let global_type = global_type.deref(context);
    match expected.array_elements() {
        Some(elements) => {
            let array = global_type
                .downcast_ref::<ArrayType>()
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if array.size() != u64::from(elements)
                || !scalar_type_matches(context, array.elem_type(), expected.value_type())
            {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
        }
        None => return Err(InspectionErrorV1::UnexpectedGraph),
    }
    drop(global_type);

    match (
        expected.byte_initializer(),
        global.get_initializer_value(context),
    ) {
        (Some(expected), Some(actual)) => {
            let actual = actual
                .downcast_ref::<BytesAttr>()
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if actual.as_ref().as_slice() != expected {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push(5);
            facts.extend_from_slice(&(expected.len() as u32).to_le_bytes());
            facts.extend_from_slice(&Sha256::digest(expected));
        }
        (None, None) => facts.push(4),
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    }
    facts.extend_from_slice(&expected.alignment().to_le_bytes());
    facts.push(expected.address_space() as u8);
    Ok(())
}

fn scalar_type_matches(context: &Context, actual: TypeHandle, expected: ScalarTypeV1) -> bool {
    let actual = actual.deref(context);
    match expected {
        ScalarTypeV1::I1
        | ScalarTypeV1::I8
        | ScalarTypeV1::I16
        | ScalarTypeV1::I32
        | ScalarTypeV1::I64 => actual
            .downcast_ref::<IntegerType>()
            .is_some_and(|integer| integer.width() == scalar_width(expected).unwrap()),
        ScalarTypeV1::F32 => actual.is::<FP32Type>(),
        ScalarTypeV1::F16 | ScalarTypeV1::Bf16 | ScalarTypeV1::F64 => false,
    }
}

fn value_type_matches(context: &Context, actual: TypeHandle, expected: ValueTypeV2) -> bool {
    match expected {
        ValueTypeV2::Scalar(scalar) => scalar_type_matches(context, actual, scalar),
        ValueTypeV2::Pointer { address_space, .. }
        | ValueTypeV2::ArrayPointer { address_space, .. } => actual
            .deref(context)
            .downcast_ref::<PointerType>()
            .is_some_and(|pointer| pointer.address_space() == address_space_id(address_space)),
        ValueTypeV2::Vector { element, lanes } => actual
            .deref(context)
            .downcast_ref::<VectorType>()
            .is_some_and(|vector| {
                vector.kind() == VectorTypeKind::Fixed
                    && vector.num_elements() == u32::from(lanes)
                    && scalar_type_matches(context, vector.elem_type(), element)
            }),
    }
}

fn instruction_operands(instruction: &InstructionKindV2) -> Vec<ValueIdV2> {
    match instruction {
        InstructionKindV2::Constant(_)
        | InstructionKindV2::VectorZero { .. }
        | InstructionKindV2::GlobalAddress(_) => Vec::new(),
        InstructionKindV2::Binary { left, right, .. }
        | InstructionKindV2::Compare { left, right, .. } => vec![*left, *right],
        InstructionKindV2::Cast { value, .. } => vec![*value],
        InstructionKindV2::GetElementPtr { base, indices } => {
            let mut operands = Vec::with_capacity(indices.len() + 1);
            operands.push(*base);
            operands.extend(indices.iter().copied());
            operands
        }
        InstructionKindV2::Load { pointer, .. }
        | InstructionKindV2::VectorLoad4 { pointer, .. } => vec![*pointer],
        InstructionKindV2::Store { pointer, value, .. } => vec![*value, *pointer],
        InstructionKindV2::Call { arguments, .. } => arguments.clone(),
        InstructionKindV2::InsertElement {
            vector,
            element,
            index,
        } => vec![*vector, *element, *index],
        InstructionKindV2::ExtractElement { vector, index } => vec![*vector, *index],
        InstructionKindV2::Phi { .. } => Vec::new(),
    }
}

fn constant_matches(actual: pliron::attribute::AttrObj, expected: ScalarConstantV2) -> bool {
    match expected.scalar_type() {
        ScalarTypeV1::I1
        | ScalarTypeV1::I8
        | ScalarTypeV1::I16
        | ScalarTypeV1::I32
        | ScalarTypeV1::I64 => actual
            .downcast_ref::<IntegerAttr>()
            .is_some_and(|integer| integer.value().to_u64() == expected.bits()),
        ScalarTypeV1::F32 => actual
            .downcast_ref::<FPSingleAttr>()
            .is_some_and(|single| f32::from(single.clone()).to_bits() == expected.bits() as u32),
        ScalarTypeV1::F16 | ScalarTypeV1::Bf16 | ScalarTypeV1::F64 => false,
    }
}

fn integer_predicate(predicate: ComparePredicateV2) -> ICmpPredicateAttr {
    match predicate {
        ComparePredicateV2::IntegerEqual => ICmpPredicateAttr::EQ,
        ComparePredicateV2::IntegerNotEqual => ICmpPredicateAttr::NE,
        ComparePredicateV2::UnsignedLessThan => ICmpPredicateAttr::ULT,
        ComparePredicateV2::UnsignedLessOrEqual => ICmpPredicateAttr::ULE,
        ComparePredicateV2::SignedLessThan => ICmpPredicateAttr::SLT,
        ComparePredicateV2::SignedLessOrEqual => ICmpPredicateAttr::SLE,
        ComparePredicateV2::OrderedEqual
        | ComparePredicateV2::OrderedNotEqual
        | ComparePredicateV2::OrderedLessThan
        | ComparePredicateV2::OrderedLessOrEqual => unreachable!("float predicate"),
    }
}

fn float_predicate(predicate: ComparePredicateV2) -> FCmpPredicateAttr {
    match predicate {
        ComparePredicateV2::OrderedEqual => FCmpPredicateAttr::OEQ,
        ComparePredicateV2::OrderedNotEqual => FCmpPredicateAttr::ONE,
        ComparePredicateV2::OrderedLessThan => FCmpPredicateAttr::OLT,
        ComparePredicateV2::OrderedLessOrEqual => FCmpPredicateAttr::OLE,
        ComparePredicateV2::IntegerEqual
        | ComparePredicateV2::IntegerNotEqual
        | ComparePredicateV2::UnsignedLessThan
        | ComparePredicateV2::UnsignedLessOrEqual
        | ComparePredicateV2::SignedLessThan
        | ComparePredicateV2::SignedLessOrEqual => unreachable!("integer predicate"),
    }
}

const fn compare_tag(predicate: ComparePredicateV2) -> u8 {
    match predicate {
        ComparePredicateV2::IntegerEqual => 1,
        ComparePredicateV2::IntegerNotEqual => 2,
        ComparePredicateV2::UnsignedLessThan => 3,
        ComparePredicateV2::UnsignedLessOrEqual => 4,
        ComparePredicateV2::SignedLessThan => 5,
        ComparePredicateV2::SignedLessOrEqual => 6,
        ComparePredicateV2::OrderedEqual => 7,
        ComparePredicateV2::OrderedNotEqual => 8,
        ComparePredicateV2::OrderedLessThan => 9,
        ComparePredicateV2::OrderedLessOrEqual => 10,
    }
}

const fn scalar_tag(scalar: ScalarTypeV1) -> u8 {
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

const fn value_type_tag(value_type: ValueTypeV2) -> u8 {
    match value_type {
        ValueTypeV2::Scalar(_) => 1,
        ValueTypeV2::Pointer { .. } => 2,
        ValueTypeV2::Vector { .. } => 3,
        ValueTypeV2::ArrayPointer { .. } => 4,
    }
}

fn inspect_instruction(
    context: &Context,
    actual: Ptr<Operation>,
    expected: &InstructionV2,
    values: &BTreeMap<ValueIdV2, Value>,
    source_value_types: &BTreeMap<ValueIdV2, ValueTypeV2>,
    globals: &[GlobalV2],
    facts: &mut Vec<u8>,
    strict_float: &mut bool,
    exact_memory_alignment: &mut bool,
) -> Result<Option<Value>, InspectionErrorV1> {
    let operation = actual.deref(context);
    if operation.num_regions() != 0 || operation.get_num_successors() != 0 {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let expected_operands = instruction_operands(expected.kind())
        .into_iter()
        .map(|id| {
            values
                .get(&id)
                .copied()
                .ok_or(InspectionErrorV1::UnexpectedGraph)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if operation.get_num_operands() != expected_operands.len()
        || expected_operands
            .iter()
            .enumerate()
            .any(|(index, expected)| operation.get_operand(index) != *expected)
    {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    drop(operation);

    macro_rules! typed {
        ($operation:ty, $tag:expr) => {{
            Operation::get_op::<$operation>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            facts.push($tag);
        }};
    }
    macro_rules! strict {
        ($operation:ty, $tag:expr) => {{
            let operation = Operation::get_op::<$operation>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if operation.fast_math_flags(context) != FastmathFlagsAttr::default() {
                *strict_float = false;
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push($tag);
        }};
    }
    macro_rules! overflow {
        ($operation:ty, $tag:expr) => {{
            let operation = Operation::get_op::<$operation>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if operation.integer_overflow_flag(context) != IntegerOverflowFlagsAttr::default() {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push($tag);
        }};
    }
    match expected.kind() {
        InstructionKindV2::Constant(constant) => {
            let operation = Operation::get_op::<ConstantOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if !constant_matches(operation.get_value(context), *constant) {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push(10);
            facts.push(scalar_tag(constant.scalar_type()));
            facts.extend_from_slice(&constant.bits().to_le_bytes());
        }
        InstructionKindV2::VectorZero { element_type } => {
            typed!(ZeroOp, 39);
            facts.push(scalar_tag(*element_type));
        }
        InstructionKindV2::GlobalAddress(global) => {
            let operation = Operation::get_op::<AddressOfOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let expected_global = globals
                .iter()
                .find(|candidate| candidate.id() == *global)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let symbol = Identifier::try_from(expected_global.symbol())
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
            if operation.get_global_name(context) != symbol {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push(40);
            facts.extend_from_slice(&global.get().to_le_bytes());
        }
        InstructionKindV2::Binary { operation, .. } => match operation {
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add) => overflow!(AddOp, 11),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Subtract) => {
                overflow!(SubOp, 12)
            }
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Multiply) => {
                overflow!(MulOp, 13)
            }
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::And) => typed!(AndOp, 14),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Or) => typed!(OrOp, 15),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Xor) => typed!(XorOp, 16),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::ShiftLeft) => {
                overflow!(ShlOp, 17)
            }
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::LogicalShiftRight) => {
                typed!(LShrOp, 18)
            }
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::ArithmeticShiftRight) => {
                typed!(AShrOp, 19)
            }
            BinaryOperationV2::Float(FloatBinaryOperationV2::Add) => strict!(FAddOp, 20),
            BinaryOperationV2::Float(FloatBinaryOperationV2::Subtract) => strict!(FSubOp, 21),
            BinaryOperationV2::Float(FloatBinaryOperationV2::Multiply) => strict!(FMulOp, 22),
            BinaryOperationV2::Float(FloatBinaryOperationV2::Divide) => strict!(FDivOp, 23),
        },
        InstructionKindV2::Compare { predicate, .. } => match predicate {
            ComparePredicateV2::IntegerEqual
            | ComparePredicateV2::IntegerNotEqual
            | ComparePredicateV2::UnsignedLessThan
            | ComparePredicateV2::UnsignedLessOrEqual
            | ComparePredicateV2::SignedLessThan
            | ComparePredicateV2::SignedLessOrEqual => {
                let operation = Operation::get_op::<ICmpOp>(actual, context)
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?;
                if operation.predicate(context) != integer_predicate(*predicate) {
                    return Err(InspectionErrorV1::UnexpectedGraph);
                }
                facts.push(24);
                facts.push(compare_tag(*predicate));
            }
            ComparePredicateV2::OrderedEqual
            | ComparePredicateV2::OrderedNotEqual
            | ComparePredicateV2::OrderedLessThan
            | ComparePredicateV2::OrderedLessOrEqual => {
                let operation = Operation::get_op::<FCmpOp>(actual, context)
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?;
                if operation.predicate(context) != float_predicate(*predicate)
                    || operation.fast_math_flags(context) != FastmathFlagsAttr::default()
                {
                    return Err(InspectionErrorV1::UnexpectedGraph);
                }
                facts.push(25);
                facts.push(compare_tag(*predicate));
            }
        },
        InstructionKindV2::Cast { operation, to, .. } => {
            facts.push(value_type_tag(*to));
            match operation {
                CastOperationV2::ZeroExtend => {
                    let operation = Operation::get_op::<ZExtOp>(actual, context)
                        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
                    if operation.nneg(context) {
                        return Err(InspectionErrorV1::UnexpectedGraph);
                    }
                    facts.push(26);
                }
                CastOperationV2::SignExtend => typed!(SExtOp, 27),
                CastOperationV2::Truncate => typed!(TruncOp, 28),
                CastOperationV2::FloatExtend => typed!(FPExtOp, 29),
                CastOperationV2::FloatTruncate => typed!(FPTruncOp, 30),
                CastOperationV2::UnsignedIntToFloat => {
                    let operation = Operation::get_op::<UIToFPOp>(actual, context)
                        .ok_or(InspectionErrorV1::UnexpectedGraph)?;
                    if operation.nneg(context) {
                        return Err(InspectionErrorV1::UnexpectedGraph);
                    }
                    facts.push(31);
                }
                CastOperationV2::SignedIntToFloat => typed!(SIToFPOp, 32),
                CastOperationV2::FloatToUnsignedInt => typed!(FPToUIOp, 33),
                CastOperationV2::FloatToSignedInt => typed!(FPToSIOp, 34),
                CastOperationV2::PointerToInt => typed!(PtrToIntOp, 35),
            }
        }
        InstructionKindV2::GetElementPtr { base, indices } => {
            let operation = Operation::get_op::<GetElementPtrOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let expected_source_type = source_element_type(
                context,
                *source_value_types
                    .get(base)
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?,
            )
            .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
            if operation.src_elem_type(context) != expected_source_type {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            let actual_indices = operation.indices(context);
            if actual_indices.len() != indices.len()
                || actual_indices.iter().zip(indices).any(|(actual, expected)| {
                    !matches!(actual, GepIndex::Value(value) if values.get(expected) == Some(value))
                })
            {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push(36);
        }
        InstructionKindV2::Load { alignment, .. } => {
            let operation = Operation::get_op::<LoadOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if operation.alignment(context) != Some(u32::from(*alignment)) {
                *exact_memory_alignment = false;
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push(37);
            facts.extend_from_slice(&alignment.to_le_bytes());
        }
        InstructionKindV2::VectorLoad4 { alignment, .. } => {
            let operation = Operation::get_op::<LoadOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if operation.alignment(context) != Some(u32::from(*alignment)) {
                *exact_memory_alignment = false;
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push(41);
            facts.extend_from_slice(&alignment.to_le_bytes());
        }
        InstructionKindV2::Store { alignment, .. } => {
            let operation = Operation::get_op::<StoreOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            if operation.alignment(context) != Some(u32::from(*alignment)) {
                *exact_memory_alignment = false;
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push(38);
            facts.extend_from_slice(&alignment.to_le_bytes());
        }
        InstructionKindV2::InsertElement { .. } => typed!(InsertElementOp, 42),
        InstructionKindV2::ExtractElement { .. } => typed!(ExtractElementOp, 43),
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(intrinsic),
            ..
        } => {
            let operation = Operation::get_op::<CallOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let symbol = Identifier::try_from(intrinsic_symbol(*intrinsic))
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
            if !matches!(operation.callee(context), CallOpCallable::Direct(actual) if actual == symbol)
                || operation.callee_type(context)
                    != intrinsic_function_type(context, *intrinsic)
                        .map_err(|_| InspectionErrorV1::UnexpectedGraph)?
                        .into()
            {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            if !operation
                .get_attr_llvm_call_fastmath_flags(context)
                .is_none_or(|flags| *flags == FastmathFlagsAttr::default())
            {
                *strict_float = false;
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.push(44);
            facts.push(intrinsic_tag(*intrinsic));
        }
        InstructionKindV2::Call {
            target: CallTargetV2::Function(_),
            ..
        }
        | InstructionKindV2::Phi { .. } => {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
    }

    let operation = actual.deref(context);
    let result = match expected.result() {
        Some(expected_result) => {
            if operation.get_num_results() != 1
                || !value_type_matches(context, operation.get_type(0), expected_result.value_type())
            {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.extend_from_slice(&expected_result.id().get().to_le_bytes());
            facts.push(value_type_tag(expected_result.value_type()));
            Some(operation.get_result(0))
        }
        None if matches!(expected.kind(), InstructionKindV2::Call { .. }) => {
            if operation.get_num_results() != 1
                || !operation.get_type(0).deref(context).is::<VoidType>()
            {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            None
        }
        None => {
            if operation.get_num_results() != 0 {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            None
        }
    };
    Ok(result)
}

fn inspect_terminator(
    context: &Context,
    actual: Ptr<Operation>,
    source_block: &BasicBlockV2,
    function: &FunctionV2,
    blocks: &BTreeMap<BlockIdV2, Ptr<BasicBlock>>,
    values: &BTreeMap<ValueIdV2, Value>,
    facts: &mut Vec<u8>,
) -> Result<(), InspectionErrorV1> {
    let operation = actual.deref(context);
    if operation.get_num_results() != 0 || operation.num_regions() != 0 {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }
    let tag = match source_block.terminator() {
        TerminatorV2::Return(None)
            if Operation::get_op::<ReturnOp>(actual, context).is_some()
                && operation.get_num_operands() == 0
                && operation.get_num_successors() == 0 =>
        {
            50
        }
        TerminatorV2::Branch(target) => {
            let branch = Operation::get_op::<BrOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let expected_target = *blocks
                .get(target)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let expected_operands = phi_operands(function, *target, source_block.id(), values)
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
            if operation.get_num_successors() != 1
                || operation.get_successor(0) != expected_target
                || branch.successor_operands(context, 0) != expected_operands
            {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.extend_from_slice(&target.get().to_le_bytes());
            51
        }
        TerminatorV2::ConditionalBranch { .. } => {
            let TerminatorV2::ConditionalBranch {
                condition,
                then_block,
                else_block,
            } = source_block.terminator()
            else {
                unreachable!()
            };
            let branch = Operation::get_op::<CondBrOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let expected_condition = *values
                .get(condition)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let then_target = *blocks
                .get(then_block)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let else_target = *blocks
                .get(else_block)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            let then_operands = phi_operands(function, *then_block, source_block.id(), values)
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
            let else_operands = phi_operands(function, *else_block, source_block.id(), values)
                .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
            if operation.get_num_operands() == 0
                || operation.get_operand(0) != expected_condition
                || operation.get_num_successors() != 2
                || operation.get_successor(0) != then_target
                || operation.get_successor(1) != else_target
                || branch.successor_operands(context, 0) != then_operands
                || branch.successor_operands(context, 1) != else_operands
            {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            facts.extend_from_slice(&condition.get().to_le_bytes());
            facts.extend_from_slice(&then_block.get().to_le_bytes());
            facts.extend_from_slice(&else_block.get().to_le_bytes());
            52
        }
        TerminatorV2::Unreachable
            if Operation::get_op::<UnreachableOp>(actual, context).is_some()
                && operation.get_num_operands() == 0
                && operation.get_num_successors() == 0 =>
        {
            53
        }
        _ => return Err(InspectionErrorV1::UnexpectedGraph),
    };
    facts.push(tag);
    Ok(())
}

fn encode_receipt(
    source: &Gfx942HandoffV2,
    profile: dialect_amdgcn::AmdgcnPlironLlvmProfileV1,
    inspection: LiveGraphInspectionV1,
) -> Result<CanonicalLoweringReceiptV1, LoweringErrorV1> {
    let source_bytes = source.encode_canonical();
    let source_len = u32::try_from(source_bytes.as_bytes().len())
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::Receipt))?;
    let mut bytes = Vec::with_capacity(RECEIPT_MAGIC_V1.len() + source_bytes.as_bytes().len() + 96);
    bytes.extend_from_slice(RECEIPT_MAGIC_V1);
    bytes.push(match profile {
        dialect_amdgcn::AmdgcnPlironLlvmProfileV1::ScalarMemoryArithmetic => 1,
        dialect_amdgcn::AmdgcnPlironLlvmProfileV1::ScalarControlFlowGemm => 2,
        dialect_amdgcn::AmdgcnPlironLlvmProfileV1::TiledDataRepresentationGemm => 3,
    });
    bytes.extend_from_slice(&source_len.to_le_bytes());
    bytes.extend_from_slice(source_bytes.as_bytes());
    bytes.extend_from_slice(&inspection.global_count.to_le_bytes());
    bytes.extend_from_slice(&inspection.intrinsic_count.to_le_bytes());
    bytes.extend_from_slice(&inspection.function_count.to_le_bytes());
    bytes.extend_from_slice(&inspection.block_count.to_le_bytes());
    bytes.extend_from_slice(&inspection.block_argument_count.to_le_bytes());
    bytes.extend_from_slice(&inspection.operation_count.to_le_bytes());
    bytes.extend_from_slice(&inspection.graph_sha256);
    bytes.push(u8::from(inspection.strict_float));
    bytes.push(u8::from(inspection.exact_memory_alignment));
    if bytes.len() > MAX_LOWERING_RECEIPT_BYTES_V1 {
        return Err(LoweringErrorV1::Construction(ConstructionStageV1::Receipt));
    }
    let identity: [u8; 32] = Sha256::new()
        .chain_update(RECEIPT_IDENTITY_DOMAIN_V1)
        .chain_update(&bytes)
        .finalize()
        .into();
    Ok(CanonicalLoweringReceiptV1 {
        bytes,
        identity: LoweringReceiptIdentityV1(identity),
    })
}

#[cfg(test)]
mod tests {
    use fe2o3_amdgcn_pliron_llvm::{ScalarKernelModuleV1, lower_scalar_kernel_v2};
    use fe2o3_llvm_handoff::{IdentityV1, StageIdentitiesV1};
    use pliron::{
        basic_block::BasicBlock, context::Ptr, linked_list::ContainsLinkedList,
        operation::Operation,
    };
    use pliron_llvm::{
        op_interfaces::AlignableOpInterface,
        ops::{FuncOp, LoadOp, StoreOp},
    };

    use super::*;
    use crate::{GraphExportErrorV1, GraphExportRequestV1};

    fn scalar_source() -> Gfx942HandoffV2 {
        lower_scalar_kernel_v2(&ScalarKernelModuleV1::canonical(
            "graph_export_mutation_module",
            "graph_export_mutation_kernel",
            IdentityV1::new([0x31; 32]).unwrap(),
            StageIdentitiesV1::new([0x41; 32], [0x42; 32], [0x43; 32]).unwrap(),
        ))
        .unwrap()
    }

    fn first_function(lowered: &LoweredAmdgcnPlironLlvmV1) -> FuncOp {
        lowered
            .module
            .module
            .get_body(&lowered.context, 0)
            .deref(&lowered.context)
            .iter(&lowered.context)
            .find_map(|operation| Operation::get_op::<FuncOp>(operation, &lowered.context))
            .unwrap()
    }

    fn request(lowered: &LoweredAmdgcnPlironLlvmV1) -> GraphExportRequestV1 {
        GraphExportRequestV1::new(lowered.source_identity(), lowered.receipt().identity())
    }

    fn entry_block(lowered: &LoweredAmdgcnPlironLlvmV1, function: FuncOp) -> Ptr<BasicBlock> {
        function
            .get_region(&lowered.context)
            .unwrap()
            .deref(&lowered.context)
            .iter(&lowered.context)
            .next()
            .unwrap()
    }

    #[test]
    fn live_operand_substitution_fails_closed() {
        let source = scalar_source();
        let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).unwrap();
        let function = first_function(&lowered);
        let entry = entry_block(&lowered, function);
        let replacement = entry.deref(&lowered.context).get_argument(0);
        let store = entry
            .deref(&lowered.context)
            .iter(&lowered.context)
            .find(|operation| Operation::get_op::<StoreOp>(*operation, &lowered.context).is_some())
            .unwrap();
        Operation::replace_operand(store, &lowered.context, 1, replacement);

        assert!(matches!(
            lowered.export_graph_v1(request(&lowered)),
            Err(GraphExportErrorV1::Inspection(
                InspectionErrorV1::UnexpectedGraph
            ))
        ));
    }

    #[test]
    fn live_alignment_substitution_fails_closed() {
        let source = scalar_source();
        let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).unwrap();
        let function = first_function(&lowered);
        let load = entry_block(&lowered, function)
            .deref(&lowered.context)
            .iter(&lowered.context)
            .find_map(|operation| Operation::get_op::<LoadOp>(operation, &lowered.context))
            .unwrap();
        load.set_alignment(&lowered.context, 8);

        assert!(matches!(
            lowered.export_graph_v1(request(&lowered)),
            Err(GraphExportErrorV1::Inspection(
                InspectionErrorV1::UnexpectedGraph
            ))
        ));
    }
}
