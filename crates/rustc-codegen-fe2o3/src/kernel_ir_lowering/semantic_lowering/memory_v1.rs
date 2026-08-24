//! Exact memory-operation bridge for the bounded gfx942 General V3 profile.

use super::{FunctionLowerer, HandlerClaim, SessionRecognizedSemanticCall, TranslationDiagnostic};
use crate::kernel_ir_lowering::{LocalBinding, TranslationDiagnosticCode, diagnostic};
use crate::mir_import::{MirOperandRef, MirTypeShape};
use crate::semantic_features::SessionRecognizedSemanticItem;
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, Constant, CopyNonOverlappingContract, MemoryElementType,
    MemoryIntrinsicOperation, Operation, OperationKind, PointerDistanceContract,
    PointerDistanceKind, PointerDistanceUnit, ScalarType, Terminator, Type, ValueId,
    VolatileAccessContract,
};

pub(super) fn claim_call(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> HandlerClaim {
    if !is_memory_profile_item(call.item) || !lowerer.is_memory_v1_source_context() {
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
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::ThreadIndex1d) => {
            super::general_v3::lower_call(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::ThreadIndexIntoDisjoint,
        ) => lower_thread_index_into_disjoint(lowerer, call, block),
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
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::MemoryCopyOneNonOverlapping,
        ) => lower_copy_one_nonoverlapping(lowerer, call, block),
        _ => unreachable!("only claimed memory calls may be lowered"),
    }
}

fn is_memory_profile_item(item: SessionRecognizedSemanticItem) -> bool {
    matches!(
        item,
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::ThreadIndex1d
                | TrustedDeviceItem::ThreadIndexIntoDisjoint
                | TrustedDeviceItem::MemoryOffsetFrom
                | TrustedDeviceItem::MemoryVolatileLoad
                | TrustedDeviceItem::MemoryVolatileStore
                | TrustedDeviceItem::MemoryCopyNonOverlapping
                | TrustedDeviceItem::MemoryCopyOneNonOverlapping
        )
    )
}

fn lower_thread_index_into_disjoint(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [MirOperandRef::Place(source)] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 1, call.operands.len(), call.location.clone()));
    };
    if !source.projection.is_empty()
        || !is_trusted_adt_shape(
            lowerer.local_shape(source.local, call.location)?,
            TrustedDeviceItem::ThreadIndex,
        )
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "ThreadIndex::into_disjoint source must be the unprojected trusted ThreadIndex type",
        ));
    }
    if !call.destination.projection.is_empty()
        || !is_trusted_adt_shape(
            lowerer.local_shape(call.destination.local, call.location)?,
            TrustedDeviceItem::DisjointIndex,
        )
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "ThreadIndex::into_disjoint destination must be the trusted DisjointIndex type",
        ));
    }

    let index = lowerer.lower_operand(&call.operands[0], block, call.location)?;
    if lowerer.value_type(index, call.location)? != &Type::INDEX
        || !lowerer.trusted_thread_indices.contains(&index)
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            "ThreadIndex::into_disjoint source did not originate from trusted thread::index_1d",
        ));
    }
    lowerer.trusted_disjoint_indices.insert(index);
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(index),
        call.location.clone(),
    )?;
    branch_to_target(lowerer, call)
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
    let [allocation, witness, value] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 3, call.operands.len(), call.location.clone()));
    };
    let index = lower_disjoint_index_witness(lowerer, witness, call, block)?;
    let (_, element, pointer) =
        lower_slice_pointer_at(lowerer, allocation, index, true, call, block)?;
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
    require_copy_compatible(
        source_slice,
        source_element,
        destination_slice,
        destination_element,
        call,
    )?;
    let count = lowerer.lower_operand(count, block, call.location)?;
    if lowerer.value_type(count, call.location)? != &Type::INDEX {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "copy_nonoverlapping_unchecked count must lower to target usize",
        ));
    }
    emit_copy(
        block,
        source_pointer,
        destination_pointer,
        count,
        source_element,
    );
    branch_to_target(lowerer, call)
}

fn lower_copy_one_nonoverlapping(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [source, source_index, destination, destination_witness] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 4, call.operands.len(), call.location.clone()));
    };
    let (source_slice, source_element, source_pointer) =
        lower_indexed_slice_pointer(lowerer, source, source_index, false, call, block)?;
    let destination_index =
        lower_disjoint_index_witness(lowerer, destination_witness, call, block)?;
    let (destination_slice, destination_element, destination_pointer) =
        lower_slice_pointer_at(lowerer, destination, destination_index, true, call, block)?;
    require_copy_compatible(
        source_slice,
        source_element,
        destination_slice,
        destination_element,
        call,
    )?;
    let count = lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(1)),
        call.location,
    )?;
    emit_copy(
        block,
        source_pointer,
        destination_pointer,
        count,
        source_element,
    );
    branch_to_target(lowerer, call)
}

fn require_copy_compatible(
    source_slice: ValueId,
    source_element: MemoryElementType,
    destination_slice: ValueId,
    destination_element: MemoryElementType,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<(), TranslationDiagnostic> {
    if source_element != destination_element {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "copy source and destination elements must match exactly",
        ));
    }
    if source_slice == destination_slice {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            "copy rejects source and destination from the same slice because this profile cannot prove dynamic region disjointness",
        ));
    }
    Ok(())
}

fn emit_copy(
    block: &mut BasicBlock,
    source: ValueId,
    destination: ValueId,
    count: ValueId,
    element: MemoryElementType,
) {
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
            source,
            destination,
            count,
            element,
            source_address_space: AddressSpace::Global,
            destination_address_space: AddressSpace::Global,
            layout: element.expected_layout(),
            contract: CopyNonOverlappingContract::supported_rust(),
        }),
    ));
}

fn lower_indexed_slice_pointer(
    lowerer: &mut FunctionLowerer<'_, '_>,
    allocation: &crate::mir_import::MirOperandRef,
    index: &crate::mir_import::MirOperandRef,
    require_writable: bool,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<(ValueId, MemoryElementType, ValueId), TranslationDiagnostic> {
    let index = lowerer.lower_operand(index, block, call.location)?;
    if lowerer.value_type(index, call.location)? != &Type::INDEX {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "memory-v1 element index must lower to target usize",
        ));
    }
    lower_slice_pointer_at(lowerer, allocation, index, require_writable, call, block)
}

fn lower_slice_pointer_at(
    lowerer: &mut FunctionLowerer<'_, '_>,
    allocation: &MirOperandRef,
    index: ValueId,
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

fn lower_disjoint_index_witness(
    lowerer: &mut FunctionLowerer<'_, '_>,
    witness: &MirOperandRef,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<ValueId, TranslationDiagnostic> {
    let MirOperandRef::Place(place) = witness else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "memory-v1 write authority must be a borrowed DisjointIndex",
        ));
    };
    let valid_shape = place.projection.is_empty()
        && matches!(
            lowerer.local_shape(place.local, call.location)?,
            MirTypeShape::Reference {
                pointee,
                mutable: false,
            } if is_trusted_adt_shape(pointee, TrustedDeviceItem::DisjointIndex)
        );
    if !valid_shape {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "memory-v1 write authority must be an unprojected shared reference to the trusted DisjointIndex type",
        ));
    }

    let index = lowerer.lower_operand(witness, block, call.location)?;
    if lowerer.value_type(index, call.location)? != &Type::INDEX
        || !lowerer.trusted_disjoint_indices.contains(&index)
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            "memory-v1 write authority did not originate from trusted thread::index_1d().into_disjoint()",
        ));
    }
    Ok(index)
}

fn is_trusted_adt_shape(shape: &MirTypeShape, item: TrustedDeviceItem) -> bool {
    matches!(
        shape,
        MirTypeShape::Adt { identity } if identity == item.canonical_path()
    )
}

fn branch_to_target(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<Terminator, TranslationDiagnostic> {
    Ok(Terminator::Branch {
        target: lowerer.block_id(call.target, call.location.clone())?,
        arguments: lowerer.edge_arguments(call.target, call.location)?,
    })
}
