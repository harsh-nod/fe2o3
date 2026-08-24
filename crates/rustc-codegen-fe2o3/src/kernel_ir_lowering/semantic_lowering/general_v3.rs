//! Exact semantic helpers for the bounded General V3 alpha/zeta profile.

use super::{
    FunctionLowerer, HandlerClaim, SemanticAssignment, SessionRecognizedSemanticCall,
    TranslationDiagnostic,
};
use crate::kernel_ir_lowering::{LocalBinding, TranslationDiagnosticCode, diagnostic};
use crate::mir_import::{MirBinaryOp, MirOperandRef, MirPlaceRef, MirRvalueKind, MirTypeShape};
use crate::semantic_features::SessionRecognizedSemanticItem;
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{BasicBlock, MatrixOperation, Operation, OperationKind, Terminator, Type};

pub(super) fn claim_call(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> HandlerClaim {
    if matches!(
        call.item,
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::DeviceMatrixCurrent
                | TrustedDeviceItem::DeviceMatrixMultiplyAccumulate
        )
    ) {
        return if lowerer.is_exact_gfx942_wave64_matrix_context() {
            HandlerClaim::Owned
        } else {
            HandlerClaim::Reject(diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                call.location.clone(),
                format!(
                    "session-recognized matrix call `{}` requires the exact gfx942:xnack- one-wave 64x1x1 kernel context",
                    call.callee.identity()
                ),
            ))
        };
    }

    HandlerClaim::NotOwned
}

pub(super) fn lower_call(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    match call.item {
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::DeviceMatrixCurrent) => {
            lower_device_matrix_current(lowerer, call)
        }
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
        ) => lower_device_matrix_multiply_accumulate(lowerer, call, block),
        _ => unreachable!("only claimed General V3 calls may be lowered"),
    }
}

fn lower_device_matrix_current(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<Terminator, TranslationDiagnostic> {
    if !call.operands.is_empty() {
        return Err(lowerer.call_arity(call.callee, 0, call.operands.len(), call.location.clone()));
    }
    lowerer.require_strict_float_policy(call.location)?;
    let _binding = lowerer.require_matrix_frontend_abi(call.location)?;
    if lowerer
        .locals
        .values()
        .any(|binding| matches!(binding, LocalBinding::DeviceMatrixValueCapability))
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            "DeviceMatrix::current may be acquired only once per kernel invocation",
        ));
    }
    require_shape(
        lowerer,
        call.destination,
        TrustedDeviceItem::DeviceMatrix,
        "DeviceMatrix::current destination",
        call.location,
    )?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::DeviceMatrixValueCapability,
        call.location.clone(),
    )?;
    branch_to_call_target(lowerer, call)
}

fn lower_device_matrix_multiply_accumulate(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [receiver, lhs, rhs, accumulator] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 4, call.operands.len(), call.location.clone()));
    };
    lowerer.require_strict_float_policy(call.location)?;
    let binding = lowerer.require_matrix_frontend_abi(call.location)?;
    require_matrix_receiver(lowerer, receiver, call.location)?;
    let lhs = require_bf16_fragment(lowerer, lhs, "lhs", call.location)?;
    let rhs = require_bf16_fragment(lowerer, rhs, "rhs", call.location)?;
    let accumulator = require_f32_fragment(lowerer, accumulator, call.location)?;
    require_shape(
        lowerer,
        call.destination,
        TrustedDeviceItem::F32AccumulatorFragment,
        "matrix multiply-accumulate destination",
        call.location,
    )?;

    let matrix =
        MatrixOperation::multiply_accumulate(lhs, rhs, accumulator).with_frontend_binding(binding);
    let results = matrix
        .result_types()
        .into_iter()
        .map(|ty| lowerer.fresh_value(ty, call.location))
        .collect::<Result<Vec<_>, _>>()?;
    let [r0, r1, r2, r3] = results.as_slice() else {
        unreachable!("the kernel IR V1 matrix contract has four accumulator results")
    };
    block.operations.push(Operation::new(
        results.clone(),
        OperationKind::Matrix(matrix),
    ));
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::F32AccumulatorFragment([r0.id, r1.id, r2.id, r3.id]),
        call.location.clone(),
    )?;
    branch_to_call_target(lowerer, call)
}

fn require_matrix_receiver(
    lowerer: &FunctionLowerer<'_, '_>,
    operand: &crate::mir_import::MirOperandRef,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<(), TranslationDiagnostic> {
    let crate::mir_import::MirOperandRef::Place(place) = operand else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            location.clone(),
            "DeviceMatrix receiver must be an authenticated local capability",
        ));
    };
    let exact_shape = matches!(
        lowerer.imported_local_shape(place.local),
        Some(MirTypeShape::Reference { pointee, mutable: false })
            if matches!(
                pointee.as_ref(),
                MirTypeShape::Adt { identity }
                    if identity == TrustedDeviceItem::DeviceMatrix.canonical_path()
            )
    );
    if !place.projection.is_empty()
        || !exact_shape
        || !matches!(
            lowerer.locals.get(&place.local),
            Some(LocalBinding::DeviceMatrixReferenceCapability)
        )
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            location.clone(),
            "DeviceMatrix receiver must be an exact unprojected &DeviceMatrix originating from the authenticated compiler constructor",
        ));
    }
    Ok(())
}

fn require_bf16_fragment(
    lowerer: &FunctionLowerer<'_, '_>,
    operand: &crate::mir_import::MirOperandRef,
    role: &str,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<[fe2o3_kernel_ir::ValueId; 4], TranslationDiagnostic> {
    require_fragment_binding(lowerer, operand, role, location, |binding| match binding {
        LocalBinding::Bf16MfmaFragment(values) => Some(values),
        _ => None,
    })
}

fn require_f32_fragment(
    lowerer: &FunctionLowerer<'_, '_>,
    operand: &crate::mir_import::MirOperandRef,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<[fe2o3_kernel_ir::ValueId; 4], TranslationDiagnostic> {
    require_fragment_binding(
        lowerer,
        operand,
        "accumulator",
        location,
        |binding| match binding {
            LocalBinding::F32AccumulatorFragment(values) => Some(values),
            _ => None,
        },
    )
}

fn require_fragment_binding(
    lowerer: &FunctionLowerer<'_, '_>,
    operand: &crate::mir_import::MirOperandRef,
    role: &str,
    location: &crate::kernel_ir_lowering::TranslationLocation,
    select: impl FnOnce(LocalBinding) -> Option<[fe2o3_kernel_ir::ValueId; 4]>,
) -> Result<[fe2o3_kernel_ir::ValueId; 4], TranslationDiagnostic> {
    let crate::mir_import::MirOperandRef::Place(place) = operand else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            location.clone(),
            format!("matrix {role} must be an unprojected authenticated fragment local"),
        ));
    };
    let values = place
        .projection
        .is_empty()
        .then(|| lowerer.locals.get(&place.local).copied())
        .flatten()
        .and_then(select)
        .ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!("matrix {role} must be an unprojected authenticated fragment local"),
            )
        })?;
    Ok(values)
}

fn require_shape(
    lowerer: &FunctionLowerer<'_, '_>,
    place: &MirPlaceRef,
    item: TrustedDeviceItem,
    role: &str,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<(), TranslationDiagnostic> {
    if !place.projection.is_empty()
        || !matches!(
            lowerer.local_shape(place.local, location)?,
            MirTypeShape::Adt { identity } if identity == item.canonical_path()
        )
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            location.clone(),
            format!("{role} must have exact type `{}`", item.canonical_path()),
        ));
    }
    Ok(())
}

fn branch_to_call_target(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<Terminator, TranslationDiagnostic> {
    Ok(Terminator::Branch {
        target: lowerer.block_id(call.target, call.location.clone())?,
        arguments: lowerer.edge_arguments(call.target, call.location)?,
    })
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
