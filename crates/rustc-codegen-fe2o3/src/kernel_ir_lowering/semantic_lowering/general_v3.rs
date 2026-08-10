//! Exact semantic helpers for the bounded General V3 alpha/zeta profile.

use super::{
    FunctionLowerer, HandlerClaim, SemanticAssignment, SessionRecognizedSemanticCall,
    TranslationDiagnostic,
};
use crate::kernel_ir_lowering::{LocalBinding, TranslationDiagnosticCode, diagnostic};
use crate::mir_import::{MirBinaryOp, MirOperandRef, MirPlaceRef, MirRvalueKind, MirTypeShape};
use crate::semantic_features::SessionRecognizedSemanticItem;
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, ComparePredicate, IntrinsicOperation, OperationKind, Terminator, Type,
};

pub(super) fn claim_call(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> HandlerClaim {
    let owned = matches!(
        call.item,
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::ThreadIndex1d
                | TrustedDeviceItem::ThreadIndexGet
                | TrustedDeviceItem::DisjointSliceGetMut
        )
    );
    if !owned {
        return HandlerClaim::NotOwned;
    }
    if !lowerer.is_general_v3_profile_context() {
        return HandlerClaim::NotOwned;
    }
    if !lowerer.is_exact_general_v3_alpha_zeta_context() {
        return HandlerClaim::Reject(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            format!(
                "session-recognized semantic call `{}` requires an exact General V3 alpha/zeta kernel context",
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
            lower_thread_index_1d(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::ThreadIndexGet) => {
            lower_thread_index_get(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::DisjointSliceGetMut) => {
            lower_disjoint_slice_get_mut(lowerer, call, block)
        }
        _ => unreachable!("only claimed General V3 calls may be lowered"),
    }
}

pub(super) fn claim_assignment(
    lowerer: &FunctionLowerer<'_, '_>,
    assignment: SemanticAssignment<'_>,
) -> HandlerClaim {
    if is_exact_f32_multiply_domain(lowerer, assignment) {
        HandlerClaim::Owned
    } else {
        HandlerClaim::NotOwned
    }
}

fn is_exact_f32_multiply_domain(
    lowerer: &FunctionLowerer<'_, '_>,
    assignment: SemanticAssignment<'_>,
) -> bool {
    assignment.rvalue == MirRvalueKind::Binary(MirBinaryOp::Mul)
        && lowerer.is_exact_general_v3_alpha_zeta_context()
        && is_f32_destination(lowerer, assignment.destination)
        && matches!(
            assignment.operands,
            [lhs, rhs]
                if is_f32_operand(lowerer, lhs) && is_f32_operand(lowerer, rhs)
        )
}

fn is_f32_destination(lowerer: &FunctionLowerer<'_, '_>, place: &MirPlaceRef) -> bool {
    if is_unprojected_f32_place(lowerer, place) {
        return true;
    }
    matches!(
        place.projection.as_slice(),
        [crate::mir_import::MirProjectionElem::Deref]
    ) && matches!(
        lowerer.imported_local_shape(place.local),
        Some(MirTypeShape::Reference {
            pointee,
            mutable: true,
        }) if pointee.as_ref() == &MirTypeShape::F32
    )
}

fn is_unprojected_f32_place(lowerer: &FunctionLowerer<'_, '_>, place: &MirPlaceRef) -> bool {
    place.projection.is_empty()
        && lowerer.imported_local_shape(place.local) == Some(&MirTypeShape::F32)
}

fn is_f32_operand(lowerer: &FunctionLowerer<'_, '_>, operand: &MirOperandRef) -> bool {
    match operand {
        MirOperandRef::Place(place) => is_unprojected_f32_place(lowerer, place),
        MirOperandRef::Constant { ty, .. } => ty.shape == MirTypeShape::F32,
    }
}

pub(super) fn lower_assignment(
    lowerer: &mut FunctionLowerer<'_, '_>,
    assignment: SemanticAssignment<'_>,
    block: &mut BasicBlock,
) -> Result<(), TranslationDiagnostic> {
    lower_f32_multiply_assignment(lowerer, assignment, block)
}

fn lower_f32_multiply_assignment(
    lowerer: &mut FunctionLowerer<'_, '_>,
    assignment: SemanticAssignment<'_>,
    block: &mut BasicBlock,
) -> Result<(), TranslationDiagnostic> {
    let [lhs, rhs] = assignment.operands else {
        return Err(diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            assignment.location.clone(),
            "multiply must have two operands",
        ));
    };
    let lhs = lowerer.lower_operand(lhs, block, assignment.location)?;
    let rhs = lowerer.lower_operand(rhs, block, assignment.location)?;
    let ty = lowerer.value_type(lhs, assignment.location)?.clone();
    let rhs_ty = lowerer.value_type(rhs, assignment.location)?;
    if ty != Type::F32 || rhs_ty != &Type::F32 {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            assignment.location.clone(),
            format!("multiply requires two f32 operands; found {ty:?} and {rhs_ty:?}"),
        ));
    }
    if !lowerer.is_exact_general_v3_alpha_zeta_context() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedRvalue,
            assignment.location.clone(),
            "f32 multiply requires an exact General V3 alpha/zeta kernel context",
        ));
    }
    if lowerer.float_target.is_none() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedRvalue,
            assignment.location.clone(),
            "f32 multiply requires the exact gfx942 floating-point profile",
        ));
    }
    lowerer.require_strict_float_policy(assignment.location)?;
    let result = lowerer.emit_result(
        block,
        Type::F32,
        OperationKind::Binary {
            op: fe2o3_kernel_ir::BinaryOp::Multiply,
            lhs,
            rhs,
        },
        assignment.location,
    )?;
    lowerer.assign_value(
        assignment.destination,
        result,
        block,
        assignment.location.clone(),
    )
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
