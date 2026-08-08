//! Exact semantic helpers for the bounded General V3 alpha/zeta profile.

use super::{AuthenticatedSemanticCall, FunctionLowerer, TranslationDiagnostic};
use crate::kernel_ir_lowering::{LocalBinding, TranslationDiagnosticCode, diagnostic};
use crate::mir_import::{MirBinaryOp, MirRvalueKind, MirTypeShape};
use crate::semantic_features::AuthenticatedSemanticItem;
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, ComparePredicate, IntrinsicOperation, OperationKind, Terminator, Type,
};

pub(super) fn try_lower_call(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: AuthenticatedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Option<Result<Terminator, TranslationDiagnostic>> {
    if !lowerer.is_exact_general_v3_alpha_zeta_context() {
        return None;
    }

    match call.item {
        AuthenticatedSemanticItem::TrustedDevice(TrustedDeviceItem::ThreadIndex1d) => {
            Some(lower_thread_index_1d(lowerer, call, block))
        }
        AuthenticatedSemanticItem::TrustedDevice(TrustedDeviceItem::ThreadIndexGet) => {
            Some(lower_thread_index_get(lowerer, call, block))
        }
        AuthenticatedSemanticItem::TrustedDevice(TrustedDeviceItem::DisjointSliceGetMut) => {
            Some(lower_disjoint_slice_get_mut(lowerer, call, block))
        }
        _ => None,
    }
}

pub(super) fn try_lower_assignment(
    lowerer: &mut FunctionLowerer<'_, '_>,
    assignment: super::SemanticAssignment<'_>,
    block: &mut BasicBlock,
) -> Option<Result<(), TranslationDiagnostic>> {
    if assignment.rvalue != MirRvalueKind::Binary(MirBinaryOp::Mul) {
        return None;
    }

    Some(lower_f32_multiply_assignment(lowerer, assignment, block))
}

fn lower_f32_multiply_assignment(
    lowerer: &mut FunctionLowerer<'_, '_>,
    assignment: super::SemanticAssignment<'_>,
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
    call: AuthenticatedSemanticCall<'_>,
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
        arguments: Vec::new(),
    })
}

fn lower_thread_index_get(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: AuthenticatedSemanticCall<'_>,
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
        arguments: Vec::new(),
    })
}

fn lower_disjoint_slice_get_mut(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: AuthenticatedSemanticCall<'_>,
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
        arguments: Vec::new(),
    })
}
