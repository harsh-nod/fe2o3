//! Workload-neutral primitives shared by authenticated typed kernels.

use super::{FunctionLowerer, HandlerClaim, SessionRecognizedSemanticCall, TranslationDiagnostic};
use crate::kernel_ir_lowering::{LocalBinding, TranslationDiagnosticCode, diagnostic};
use crate::mir_import::MirTypeShape;
use crate::semantic_features::SessionRecognizedSemanticItem;
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, ComparePredicate, IntrinsicOperation, OperationKind, Terminator, Type,
};

pub(super) fn claim_call(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> HandlerClaim {
    if !matches!(
        call.item,
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::ThreadIndex1d
                | TrustedDeviceItem::ThreadIndexGet
                | TrustedDeviceItem::DisjointSliceGetMut
                | TrustedDeviceItem::DisjointSliceLen
        )
    ) {
        return HandlerClaim::NotOwned;
    }
    if lowerer.is_general_v3_profile_context() {
        HandlerClaim::Owned
    } else {
        HandlerClaim::NotOwned
    }
}

pub(super) fn lower_call(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    match call.item {
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::ThreadIndex1d) => {
            lower_thread_index_1d(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::ThreadIndexGet) => {
            lower_thread_index_get(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::DisjointSliceGetMut) => {
            lower_disjoint_slice_get_mut(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::DisjointSliceLen) => {
            lower_disjoint_slice_len(lowerer, call, block)
        }
        _ => unreachable!("only claimed typed-kernel primitive calls may be lowered"),
    }
}

fn lower_disjoint_slice_len(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [receiver] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 1, call.operands.len(), call.location.clone()));
    };
    let receiver = lowerer.lower_operand(receiver, block, call.location)?;
    if !matches!(lowerer.value_type(receiver, call.location)?, Type::Slice(_)) {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "trusted DisjointSlice::len receiver is not a translated slice",
        ));
    }
    lowerer.require_destination_type(call.destination, &Type::INDEX, call.location)?;
    let length = lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::SliceLength { slice: receiver },
        call.location,
    )?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(length),
        call.location.clone(),
    )?;
    Ok(Terminator::Branch {
        target: lowerer.block_id(call.target, call.location.clone())?,
        arguments: lowerer.edge_arguments(call.target, call.location)?,
    })
}

fn lower_thread_index_1d(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    if !call.operands.is_empty() {
        return Err(lowerer.call_arity(call.callee, 0, call.operands.len(), call.location.clone()));
    }
    let MirTypeShape::Adt { identity } =
        lowerer.local_shape(call.destination.local, call.location)?
    else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "thread::index_1d destination is not the trusted ThreadIndex type",
        ));
    };
    if identity != TrustedDeviceItem::ThreadIndex.canonical_path() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "thread::index_1d destination is not the trusted ThreadIndex type",
        ));
    }

    let index = lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        call.location,
    )?;
    lowerer.trusted_thread_indices.insert(index);
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(index),
        call.location.clone(),
    )?;
    Ok(Terminator::Branch {
        target: lowerer.block_id(call.target, call.location.clone())?,
        arguments: lowerer.edge_arguments(call.target, call.location)?,
    })
}

fn lower_thread_index_get(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [receiver] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 1, call.operands.len(), call.location.clone()));
    };
    let receiver = lowerer.lower_operand(receiver, block, call.location)?;
    if !lowerer.trusted_thread_indices.contains(&receiver) {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            "ThreadIndex::get receiver did not originate from trusted thread::index_1d",
        ));
    }
    lowerer.require_destination_type(call.destination, &Type::INDEX, call.location)?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(receiver),
        call.location.clone(),
    )?;
    Ok(Terminator::Branch {
        target: lowerer.block_id(call.target, call.location.clone())?,
        arguments: lowerer.edge_arguments(call.target, call.location)?,
    })
}

fn lower_disjoint_slice_get_mut(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [receiver, index] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 2, call.operands.len(), call.location.clone()));
    };
    let receiver = lowerer.lower_operand(receiver, block, call.location)?;
    let index = lowerer.lower_operand(index, block, call.location)?;
    let receiver_ty = lowerer.value_type(receiver, call.location)?.clone();
    let Type::Slice(slice) = receiver_ty else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "trusted DisjointSlice::get_mut receiver is not a translated slice",
        ));
    };
    if slice.access != AccessMode::ReadWrite {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "trusted DisjointSlice::get_mut receiver must be writable",
        ));
    }
    if lowerer.value_type(index, call.location)? != &Type::INDEX
        || !lowerer.trusted_thread_indices.contains(&index)
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            "DisjointSlice::get_mut index did not originate from trusted thread::index_1d",
        ));
    }

    let length = lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::SliceLength { slice: receiver },
        call.location,
    )?;
    let in_bounds = lowerer.emit_result(
        block,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: index,
            rhs: length,
        },
        call.location,
    )?;
    let pointer_ty = Type::pointer((*slice.element).clone(), slice.address_space, slice.access);
    let data = lowerer.emit_result(
        block,
        pointer_ty.clone(),
        OperationKind::SliceData { slice: receiver },
        call.location,
    )?;
    let payload = lowerer.emit_result(
        block,
        pointer_ty,
        OperationKind::GetElementPointer {
            base: data,
            offset: index,
        },
        call.location,
    )?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::OptionPointer {
            discriminant: in_bounds,
            payload,
            some_entry: None,
        },
        call.location.clone(),
    )?;
    Ok(Terminator::Branch {
        target: lowerer.block_id(call.target, call.location.clone())?,
        arguments: lowerer.edge_arguments(call.target, call.location)?,
    })
}
