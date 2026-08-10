//! Exact memory-operation bridge for the bounded gfx942 General V3 profile.

use super::{FunctionLowerer, HandlerClaim, SessionRecognizedSemanticCall, TranslationDiagnostic};
use crate::kernel_ir_lowering::{LocalBinding, TranslationDiagnosticCode, diagnostic};
use crate::semantic_features::SessionRecognizedSemanticItem;
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, CopyNonOverlappingContract, MemoryElementType,
    MemoryIntrinsicOperation, Operation, OperationKind, PointerDistanceContract,
    PointerDistanceKind, PointerDistanceUnit, ScalarType, Terminator, Type, ValueId,
    VolatileAccessContract,
};

pub(super) fn claim_call(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> HandlerClaim {
    if !is_memory_item(call.item) {
        return HandlerClaim::NotOwned;
    }
    if !lowerer.is_gfx942_memory_v1_context() {
        return HandlerClaim::Reject(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            format!(
                "session-recognized memory call `{}` requires the gfx942 General V3 memory-v1 profile",
                call.callee.identity()
            ),
        ));
    }
    HandlerClaim::Owned
}

pub(super) fn lower_call(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    match call.item {
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::MemoryOffsetFrom) => {
            lower_offset_from(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::MemoryVolatileLoad) => {
            lower_volatile_load(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::MemoryVolatileStore) => {
            lower_volatile_store(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::MemoryCopyNonOverlapping,
        ) => lower_copy_nonoverlapping(lowerer, call, block),
        _ => unreachable!("only claimed memory calls may be lowered"),
    }
}

fn is_memory_item(item: SessionRecognizedSemanticItem) -> bool {
    matches!(
        item,
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::MemoryOffsetFrom
                | TrustedDeviceItem::MemoryVolatileLoad
                | TrustedDeviceItem::MemoryVolatileStore
                | TrustedDeviceItem::MemoryCopyNonOverlapping
        )
    )
}

fn lower_offset_from(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [allocation, pointer_index, origin_index] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 3, call.operands.len(), call.location.clone()));
    };
    let (slice, element, pointer) =
        lower_indexed_slice_pointer(lowerer, allocation, pointer_index, false, call, block)?;
    let (origin_slice, origin_element, origin) =
        lower_indexed_slice_pointer(lowerer, allocation, origin_index, false, call, block)?;
    if slice != origin_slice || element != origin_element {
        return Err(diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            call.location.clone(),
            "offset_from pointers must originate from one identical slice allocation",
        ));
    }
    lowerer.require_destination_type(
        call.destination,
        &Type::Scalar(ScalarType::I64),
        call.location,
    )?;
    let result = lowerer.emit_result(
        block,
        Type::Scalar(ScalarType::I64),
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::PointerDistance {
            pointer,
            origin,
            kind: PointerDistanceKind::Signed,
            unit: PointerDistanceUnit::Elements,
            element,
            address_space: AddressSpace::Global,
            layout: element.expected_layout(),
            contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
        }),
        call.location,
    )?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(result),
        call.location.clone(),
    )?;
    branch_to_target(lowerer, call)
}

fn lower_volatile_load(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [allocation, index] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 2, call.operands.len(), call.location.clone()));
    };
    let (_, element, pointer) =
        lower_indexed_slice_pointer(lowerer, allocation, index, false, call, block)?;
    let result_type = element.ir_type();
    lowerer.require_destination_type(call.destination, &result_type, call.location)?;
    let result = lowerer.emit_result(
        block,
        result_type,
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
            pointer,
            element,
            address_space: AddressSpace::Global,
            layout: element.expected_layout(),
            contract: VolatileAccessContract::rust_allocation_load(),
        }),
        call.location,
    )?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(result),
        call.location.clone(),
    )?;
    branch_to_target(lowerer, call)
}

fn lower_volatile_store(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [allocation, index, value] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 3, call.operands.len(), call.location.clone()));
    };
    let (_, element, pointer) =
        lower_indexed_slice_pointer(lowerer, allocation, index, true, call, block)?;
    let value = lowerer.lower_operand(value, block, call.location)?;
    if lowerer.value_type(value, call.location)? != &element.ir_type() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "volatile_store value type must exactly match the destination element",
        ));
    }
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
            pointer,
            value,
            element,
            address_space: AddressSpace::Global,
            layout: element.expected_layout(),
            contract: VolatileAccessContract::rust_allocation_store(),
        }),
    ));
    branch_to_target(lowerer, call)
}

fn lower_copy_nonoverlapping(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [source, source_index, destination, destination_index, count] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 5, call.operands.len(), call.location.clone()));
    };
    let (source_slice, source_element, source_pointer) =
        lower_indexed_slice_pointer(lowerer, source, source_index, false, call, block)?;
    let (destination_slice, destination_element, destination_pointer) =
        lower_indexed_slice_pointer(lowerer, destination, destination_index, true, call, block)?;
    if source_element != destination_element {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "copy_nonoverlapping source and destination elements must match exactly",
        ));
    }
    if source_slice == destination_slice {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            "copy_nonoverlapping rejects source and destination from the same slice because this profile cannot prove dynamic region disjointness",
        ));
    }
    let count = lowerer.lower_operand(count, block, call.location)?;
    if lowerer.value_type(count, call.location)? != &Type::INDEX {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "copy_nonoverlapping count must lower to target usize",
        ));
    }
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
            source: source_pointer,
            destination: destination_pointer,
            count,
            element: source_element,
            source_address_space: AddressSpace::Global,
            destination_address_space: AddressSpace::Global,
            layout: source_element.expected_layout(),
            contract: CopyNonOverlappingContract::supported_rust(),
        }),
    ));
    branch_to_target(lowerer, call)
}

fn lower_indexed_slice_pointer(
    lowerer: &mut FunctionLowerer<'_, '_>,
    allocation: &crate::mir_import::MirOperandRef,
    index: &crate::mir_import::MirOperandRef,
    require_writable: bool,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<(ValueId, MemoryElementType, ValueId), TranslationDiagnostic> {
    let allocation = lowerer.lower_operand(allocation, block, call.location)?;
    let Type::Slice(slice) = lowerer.value_type(allocation, call.location)?.clone() else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "memory-v1 allocation operand must be a translated slice",
        ));
    };
    if slice.address_space != AddressSpace::Global {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "memory-v1 supports only the global address space",
        ));
    }
    if require_writable && slice.access != AccessMode::ReadWrite {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "memory-v1 destination must be a writable DisjointSlice",
        ));
    }
    let Type::Scalar(scalar @ (ScalarType::F32 | ScalarType::F64)) = *slice.element else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "memory-v1 supports only f32 and f64 slice elements",
        ));
    };
    let element = MemoryElementType::Scalar(scalar);
    let index = lowerer.lower_operand(index, block, call.location)?;
    if lowerer.value_type(index, call.location)? != &Type::INDEX {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "memory-v1 element index must lower to target usize",
        ));
    }
    let pointer_type = Type::pointer(element.ir_type(), AddressSpace::Global, slice.access);
    let data = lowerer.emit_result(
        block,
        pointer_type.clone(),
        OperationKind::SliceData { slice: allocation },
        call.location,
    )?;
    let pointer = lowerer.emit_result(
        block,
        pointer_type,
        OperationKind::GetElementPointer {
            base: data,
            offset: index,
        },
        call.location,
    )?;
    Ok((allocation, element, pointer))
}

fn branch_to_target(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<Terminator, TranslationDiagnostic> {
    Ok(Terminator::Branch {
        target: lowerer.block_id(call.target, call.location.clone())?,
        arguments: Vec::new(),
    })
}
