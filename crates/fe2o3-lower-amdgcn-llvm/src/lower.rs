use std::{
    collections::BTreeMap,
    num::NonZero,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_amdgcn::{AdmittedAmdgcnPlironLlvmV1, admit_amdgcn_pliron_llvm_v1};
use fe2o3_llvm_handoff::{
    AddressSpaceV1, BasicBlockV2, BinaryOperationV2, BlockIdV2, CastOperationV2,
    ComparePredicateV2, FloatBinaryOperationV2, FunctionV2, Gfx942HandoffV2, InstructionKindV2,
    IntegerBinaryOperationV2, ScalarConstantV2, ScalarTypeV1, TerminatorV2, ValueIdV2, ValueTypeV2,
};
use fe2o3_pliron::{ensure_context_identity, require_context_identity};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::{FPSingleAttr, IntegerAttr},
        op_interfaces::{
            AtMostOneRegionInterface, OneResultInterface, SingleBlockRegionInterface,
            SymbolOpInterface,
        },
        ops::ModuleOp,
        types::{FP32Type, IntegerType, Signedness},
    },
    context::{Context, Ptr},
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::{TypeHandle, Typed},
    utils::apint::APInt,
    value::Value,
};
use pliron_llvm::{
    attributes::{
        FCmpPredicateAttr, FastmathFlagsAttr, ICmpPredicateAttr, IntegerOverflowFlagsAttr,
    },
    op_interfaces::{
        AlignableOpInterface, BinArithOp, CastOpInterface, FastMathFlags,
        FloatBinArithOpWithFastMathFlags, IntBinArithOpWithOverflowFlag,
    },
    ops::{
        AShrOp, AddOp, AndOp, BrOp, CondBrOp, ConstantOp, FAddOp, FCmpOp, FDivOp, FMulOp, FPExtOp,
        FPToSIOp, FPToUIOp, FPTruncOp, FSubOp, FuncOp, GepIndex, GetElementPtrOp, ICmpOp, LShrOp,
        LoadOp, MulOp, OrOp, PtrToIntOp, ReturnOp, SExtOp, SIToFPOp, ShlOp, StoreOp, SubOp,
        TruncOp, UIToFPOp, UnreachableOp, XorOp, ZExtOp,
    },
    types::{FuncType, PointerType, VoidType},
};
use sha2::{Digest as _, Sha256};

use crate::model::{
    CanonicalLoweringReceiptV1, ConstructionStageV1, InspectionErrorV1, LiveGraphInspectionV1,
    LoweredAmdgcnPlironLlvmV1, LoweringErrorV1, LoweringReceiptIdentityV1,
    MAX_LOWERING_RECEIPT_BYTES_V1, OwnedDialectModuleV1,
};

const MODULE_NAME_V1: &str = "fe2o3_amdgcn_pliron_llvm_v1";
const RECEIPT_MAGIC_V1: &[u8] = b"fe2o3.lower-amdgcn-llvm.receipt.v1\0";
const RECEIPT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.lower-amdgcn-llvm.identity.v1\0";

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

fn build_module(
    context: &mut Context,
    admitted: AdmittedAmdgcnPlironLlvmV1<'_>,
) -> Result<ModuleOp, LoweringErrorV1> {
    let module_name = Identifier::try_from(MODULE_NAME_V1)
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph))?;
    let module = ModuleOp::new(context, module_name);
    for function in admitted.handoff().module().functions() {
        build_function(context, &module, function)?;
    }
    Ok(module)
}

fn build_function(
    context: &mut Context,
    module: &ModuleOp,
    source: &FunctionV2,
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

    for block in ordered_blocks(source) {
        let target = *blocks
            .get(&block.id())
            .ok_or(LoweringErrorV1::Construction(
                ConstructionStageV1::DialectGraph,
            ))?;
        for instruction in block.instructions() {
            if matches!(instruction.kind(), InstructionKindV2::Phi { .. }) {
                continue;
            }
            let result = build_instruction(context, target, instruction.kind(), &values)?;
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
        build_terminator(context, target, block, source, &blocks, &values)?;
    }
    Ok(())
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
            let source_type = source_element_type(context, base_value)?;
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
        InstructionKindV2::Phi { .. }
        | InstructionKindV2::GlobalAddress(_)
        | InstructionKindV2::VectorZero { .. }
        | InstructionKindV2::VectorLoad4 { .. }
        | InstructionKindV2::Call { .. }
        | InstructionKindV2::InsertElement { .. }
        | InstructionKindV2::ExtractElement { .. } => {
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
        CastOperationV2::ZeroExtend => cast!(ZExtOp),
        CastOperationV2::SignExtend => cast!(SExtOp),
        CastOperationV2::Truncate => cast!(TruncOp),
        CastOperationV2::FloatExtend => cast!(FPExtOp),
        CastOperationV2::FloatTruncate => cast!(FPTruncOp),
        CastOperationV2::UnsignedIntToFloat => cast!(UIToFPOp),
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

fn type_for(context: &mut Context, value_type: ValueTypeV2) -> Result<TypeHandle, LoweringErrorV1> {
    match value_type {
        ValueTypeV2::Scalar(scalar) => scalar_type(context, scalar),
        ValueTypeV2::Pointer { address_space, .. } => {
            Ok(PointerType::get(context, address_space_id(address_space)).into())
        }
        ValueTypeV2::Vector { .. } | ValueTypeV2::ArrayPointer { .. } => Err(
            LoweringErrorV1::Construction(ConstructionStageV1::DialectGraph),
        ),
    }
}

fn scalar_type(context: &mut Context, scalar: ScalarTypeV1) -> Result<TypeHandle, LoweringErrorV1> {
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
    context: &mut Context,
    pointer: Value,
) -> Result<TypeHandle, LoweringErrorV1> {
    if pointer
        .get_type(context)
        .deref(context)
        .downcast_ref::<PointerType>()
        .is_none()
    {
        return Err(LoweringErrorV1::Construction(
            ConstructionStageV1::DialectGraph,
        ));
    }
    Ok(FP32Type::get(context).into())
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
    let actual_functions = owned
        .module
        .get_body(context, 0)
        .deref(context)
        .iter(context)
        .collect::<Vec<_>>();
    if actual_functions.len() != source.module().functions().len() {
        return Err(InspectionErrorV1::UnexpectedGraph);
    }

    let mut facts = Vec::new();
    let mut function_count = 0_u32;
    let mut block_count = 0_u32;
    let mut block_argument_count = 0_u32;
    let mut operation_count = 0_u32;
    let mut strict_float = true;
    let mut exact_memory_alignment = true;
    for (source_function, actual) in source.module().functions().iter().zip(actual_functions) {
        let function = Operation::get_op::<FuncOp>(actual, context)
            .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        let source_symbol = Identifier::try_from(source_function.symbol())
            .map_err(|_| InspectionErrorV1::UnexpectedGraph)?;
        if function.get_symbol_name(context) != source_symbol {
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
        for (source_block, actual_block) in source_blocks.into_iter().zip(actual_blocks) {
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
                    continue;
                }
                let actual = actual_operations
                    .next()
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?;
                inspect_instruction(
                    context,
                    actual,
                    instruction.kind(),
                    &mut facts,
                    &mut strict_float,
                    &mut exact_memory_alignment,
                )?;
                operation_count = operation_count
                    .checked_add(1)
                    .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            }
            let terminator = actual_operations
                .next()
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            inspect_terminator(context, terminator, source_block.terminator(), &mut facts)?;
            if actual_operations.next().is_some() {
                return Err(InspectionErrorV1::UnexpectedGraph);
            }
            operation_count = operation_count
                .checked_add(1)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
        }
    }
    let graph_sha256: [u8; 32] = Sha256::digest(&facts).into();
    Ok(LiveGraphInspectionV1 {
        function_count,
        block_count,
        block_argument_count,
        operation_count,
        graph_sha256,
        strict_float,
        exact_memory_alignment,
    })
}

fn inspect_instruction(
    context: &Context,
    actual: Ptr<Operation>,
    expected: &InstructionKindV2,
    facts: &mut Vec<u8>,
    strict_float: &mut bool,
    exact_memory_alignment: &mut bool,
) -> Result<(), InspectionErrorV1> {
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
            *strict_float &= operation.fast_math_flags(context) == FastmathFlagsAttr::default();
            facts.push($tag);
        }};
    }
    match expected {
        InstructionKindV2::Constant(_) => typed!(ConstantOp, 10),
        InstructionKindV2::Binary { operation, .. } => match operation {
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add) => typed!(AddOp, 11),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Subtract) => typed!(SubOp, 12),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Multiply) => typed!(MulOp, 13),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::And) => typed!(AndOp, 14),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Or) => typed!(OrOp, 15),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::Xor) => typed!(XorOp, 16),
            BinaryOperationV2::Integer(IntegerBinaryOperationV2::ShiftLeft) => typed!(ShlOp, 17),
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
            | ComparePredicateV2::SignedLessOrEqual => typed!(ICmpOp, 24),
            ComparePredicateV2::OrderedEqual
            | ComparePredicateV2::OrderedNotEqual
            | ComparePredicateV2::OrderedLessThan
            | ComparePredicateV2::OrderedLessOrEqual => typed!(FCmpOp, 25),
        },
        InstructionKindV2::Cast { operation, .. } => match operation {
            CastOperationV2::ZeroExtend => typed!(ZExtOp, 26),
            CastOperationV2::SignExtend => typed!(SExtOp, 27),
            CastOperationV2::Truncate => typed!(TruncOp, 28),
            CastOperationV2::FloatExtend => typed!(FPExtOp, 29),
            CastOperationV2::FloatTruncate => typed!(FPTruncOp, 30),
            CastOperationV2::UnsignedIntToFloat => typed!(UIToFPOp, 31),
            CastOperationV2::SignedIntToFloat => typed!(SIToFPOp, 32),
            CastOperationV2::FloatToUnsignedInt => typed!(FPToUIOp, 33),
            CastOperationV2::FloatToSignedInt => typed!(FPToSIOp, 34),
            CastOperationV2::PointerToInt => typed!(PtrToIntOp, 35),
        },
        InstructionKindV2::GetElementPtr { .. } => typed!(GetElementPtrOp, 36),
        InstructionKindV2::Load { alignment, .. } => {
            let operation = Operation::get_op::<LoadOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            *exact_memory_alignment &= operation.alignment(context) == Some(u32::from(*alignment));
            facts.push(37);
            facts.extend_from_slice(&alignment.to_le_bytes());
        }
        InstructionKindV2::Store { alignment, .. } => {
            let operation = Operation::get_op::<StoreOp>(actual, context)
                .ok_or(InspectionErrorV1::UnexpectedGraph)?;
            *exact_memory_alignment &= operation.alignment(context) == Some(u32::from(*alignment));
            facts.push(38);
            facts.extend_from_slice(&alignment.to_le_bytes());
        }
        InstructionKindV2::Phi { .. }
        | InstructionKindV2::GlobalAddress(_)
        | InstructionKindV2::VectorZero { .. }
        | InstructionKindV2::VectorLoad4 { .. }
        | InstructionKindV2::Call { .. }
        | InstructionKindV2::InsertElement { .. }
        | InstructionKindV2::ExtractElement { .. } => {
            return Err(InspectionErrorV1::UnexpectedGraph);
        }
    }
    Ok(())
}

fn inspect_terminator(
    context: &Context,
    actual: Ptr<Operation>,
    expected: &TerminatorV2,
    facts: &mut Vec<u8>,
) -> Result<(), InspectionErrorV1> {
    let tag = match expected {
        TerminatorV2::Return(None) if Operation::get_op::<ReturnOp>(actual, context).is_some() => {
            50
        }
        TerminatorV2::Branch(_) if Operation::get_op::<BrOp>(actual, context).is_some() => 51,
        TerminatorV2::ConditionalBranch { .. }
            if Operation::get_op::<CondBrOp>(actual, context).is_some() =>
        {
            52
        }
        TerminatorV2::Unreachable
            if Operation::get_op::<UnreachableOp>(actual, context).is_some() =>
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
    });
    bytes.extend_from_slice(&source_len.to_le_bytes());
    bytes.extend_from_slice(source_bytes.as_bytes());
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
