//! Exact semantic helpers for authenticated scalar kernel profiles.

use super::{
    FunctionLowerer, HandlerClaim, SemanticAssignment, SessionRecognizedSemanticCall,
    TranslationDiagnostic,
};
use crate::kernel_ir_lowering::{LocalBinding, TranslationDiagnosticCode, diagnostic};
use crate::mir_import::{
    MirBinaryOp, MirConstant, MirOperandRef, MirPlaceRef, MirRvalueKind, MirTypeShape,
};
use crate::semantic_features::SessionRecognizedSemanticItem;
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BinaryOp, CheckedBinaryOperator, ComparePredicate, Constant,
    MatrixOperation, Operation, OperationKind, TensorLayoutContractV1, Terminator, Type,
    WorkgroupSize,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticDisjointIndexSpaceV1;

pub(super) fn claim_call(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> HandlerClaim {
    if matches!(
        call.item,
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::DeviceMatrixCurrent
                | TrustedDeviceItem::DeviceMatrixMultiplyAccumulate
                | TrustedDeviceItem::F32AccumulatorFragmentIntoValues
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

    let owned = matches!(
        call.item,
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::ThreadIndexCheckedTiled2D
                | TrustedDeviceItem::DisjointSliceGetTiled2DMut
        )
    );
    if !owned {
        return HandlerClaim::NotOwned;
    }
    if !lowerer.is_general_v3_profile_context() {
        return HandlerClaim::NotOwned;
    }
    if !lowerer.is_authenticated_general_v3_scalar_context() {
        return HandlerClaim::Reject(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            format!(
                "session-recognized semantic call `{}` requires an exact alpha/zeta contract or a compiler-sealed generated General V3 kernel context",
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
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::ThreadIndexCheckedTiled2D,
        ) => lower_thread_index_checked_tiled_2d(lowerer, call, block),
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::DisjointSliceGetTiled2DMut,
        ) => lower_disjoint_slice_get_tiled_2d_mut(lowerer, call, block),
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::DeviceMatrixCurrent) => {
            lower_device_matrix_current(lowerer, call)
        }
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
        ) => lower_device_matrix_multiply_accumulate(lowerer, call, block),
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::F32AccumulatorFragmentIntoValues,
        ) => lower_f32_fragment_into_values(lowerer, call),
        _ => unreachable!("only claimed General V3 calls may be lowered"),
    }
}

fn lower_f32_fragment_into_values(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<Terminator, TranslationDiagnostic> {
    lowerer.require_strict_float_policy(call.location)?;
    let [fragment] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 1, call.operands.len(), call.location.clone()));
    };
    let values = require_f32_fragment(lowerer, fragment, call.location)?;
    require_array_shape(
        lowerer,
        call.destination,
        &MirTypeShape::F32,
        "F32AccumulatorFragment::into_values destination",
        call.location,
    )?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::FixedArray4(values),
        call.location.clone(),
    )?;
    branch_to_call_target(lowerer, call)
}

fn require_array_shape(
    lowerer: &FunctionLowerer<'_, '_>,
    place: &MirPlaceRef,
    element: &MirTypeShape,
    role: &str,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<(), TranslationDiagnostic> {
    if !place.projection.is_empty()
        || !matches!(
            lowerer.local_shape(place.local, location)?,
            MirTypeShape::Array { element: actual, length: Some(4) }
                if actual.as_ref() == element
        )
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            location.clone(),
            format!("{role} must be an exact four-element scalar array"),
        ));
    }
    Ok(())
}

fn lower_thread_index_checked_tiled_2d(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [MirOperandRef::Place(receiver)] = call.operands else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "checked_tiled_2d receiver must be one exact ThreadIndex local",
        ));
    };
    if !receiver.projection.is_empty()
        || !matches!(
            lowerer.imported_local_shape(receiver.local),
            Some(MirTypeShape::Adt { identity })
                if identity == TrustedDeviceItem::ThreadIndex.canonical_path()
        )
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "checked_tiled_2d receiver must have exact trusted ThreadIndex<Index1D> type",
        ));
    }
    let raw = match lowerer.locals.get(&receiver.local).copied() {
        Some(LocalBinding::Value(raw)) if lowerer.trusted_thread_indices.contains(&raw) => raw,
        _ => {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                call.location.clone(),
                "checked_tiled_2d receiver did not originate from trusted thread::index_1d",
            ));
        }
    };
    let evidence = call.callee.checked_tiled_2d_evidence_v1().ok_or_else(|| {
        diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            format!(
                "checked_tiled_2d lacks compiler-authenticated const-generic evidence{}",
                call.callee
                    .semantic_call_evidence_rejection_v1()
                    .map_or_else(String::new, |detail| format!(": {detail}"),)
            ),
        )
    })?;
    let (lanes_per_tile, tile_rows, tile_columns, elements_per_lane) = evidence.geometry();
    let geometry_is_valid = lanes_per_tile != 0
        && tile_rows != 0
        && tile_columns != 0
        && elements_per_lane != 0
        && lanes_per_tile.is_multiple_of(tile_columns)
        && lanes_per_tile.checked_mul(elements_per_lane) == tile_rows.checked_mul(tile_columns)
        && (lanes_per_tile / tile_columns).checked_mul(elements_per_lane) == Some(tile_rows);
    let expected_space = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
        lanes_per_tile,
        tile_rows,
        tile_columns,
        elements_per_lane,
    };
    if evidence.input_space() != SemanticDisjointIndexSpaceV1::Index1d
        || evidence.output_space() != expected_space
        || !geometry_is_valid
        || lowerer.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
        || (lanes_per_tile, tile_rows, tile_columns, elements_per_lane) != (64, 16, 16, 4)
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "checked_tiled_2d requires exact authenticated Index1D -> Tiled2D<Index1D, 64, 16, 16, 4> geometry in a 64x1x1 kernel",
        ));
    }
    if !call.destination.projection.is_empty()
        || !matches!(
            lowerer.local_shape(call.destination.local, call.location)?,
            MirTypeShape::Adt { identity }
                if matches!(identity.as_str(), "core::option::Option" | "std::option::Option")
        )
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "checked_tiled_2d destination must be its exact unprojected Option<DisjointTile2D<...>> result",
        ));
    }
    let present = lowerer.emit_result(
        block,
        Type::BOOL,
        OperationKind::Constant(Constant::Bool(true)),
        call.location,
    )?;
    lowerer.trusted_thread_indices.remove(&raw);
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::OptionTiled2dWitness {
            discriminant: present,
            raw,
            evidence,
            some_entry: None,
        },
        call.location.clone(),
    )?;
    branch_to_call_target(lowerer, call)
}

fn lower_device_matrix_current(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<Terminator, TranslationDiagnostic> {
    if !call.operands.is_empty() {
        return Err(lowerer.call_arity(call.callee, 0, call.operands.len(), call.location.clone()));
    }
    lowerer.require_strict_float_policy(call.location)?;
    lowerer.require_matrix_frontend_abi(call.location)?;
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

    let mut matrix = MatrixOperation::multiply_accumulate(lhs, rhs, accumulator)
        .with_declared_tensor_layout(
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
        );
    if let Some(binding) = binding {
        matrix = matrix.with_frontend_binding(binding);
    }
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
    if is_exact_f32_binary_domain(lowerer, assignment) {
        HandlerClaim::Owned
    } else {
        HandlerClaim::NotOwned
    }
}

fn is_exact_f32_binary_domain(
    lowerer: &FunctionLowerer<'_, '_>,
    assignment: SemanticAssignment<'_>,
) -> bool {
    matches!(
        assignment.rvalue,
        MirRvalueKind::Binary(MirBinaryOp::Add | MirBinaryOp::Mul)
    ) && lowerer.is_authenticated_f32_scalar_context()
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
    lower_f32_binary_assignment(lowerer, assignment, block)
}

fn lower_f32_binary_assignment(
    lowerer: &mut FunctionLowerer<'_, '_>,
    assignment: SemanticAssignment<'_>,
    block: &mut BasicBlock,
) -> Result<(), TranslationDiagnostic> {
    let operation = match assignment.rvalue {
        MirRvalueKind::Binary(MirBinaryOp::Add) => fe2o3_kernel_ir::BinaryOp::Add,
        MirRvalueKind::Binary(MirBinaryOp::Mul) => fe2o3_kernel_ir::BinaryOp::Multiply,
        _ => unreachable!("General V3 claims only f32 add and multiply assignments"),
    };
    let operation_name = match operation {
        fe2o3_kernel_ir::BinaryOp::Add => "addition",
        fe2o3_kernel_ir::BinaryOp::Multiply => "multiply",
        _ => unreachable!("matched General V3 f32 operation"),
    };
    let [lhs, rhs] = assignment.operands else {
        return Err(diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            assignment.location.clone(),
            format!("{operation_name} must have two operands"),
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
            format!("{operation_name} requires two f32 operands; found {ty:?} and {rhs_ty:?}"),
        ));
    }
    if !lowerer.is_authenticated_f32_scalar_context() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedRvalue,
            assignment.location.clone(),
            format!("f32 {operation_name} requires an authenticated scalar kernel context"),
        ));
    }
    if lowerer.float_target.is_none() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedRvalue,
            assignment.location.clone(),
            format!("f32 {operation_name} requires the exact gfx942 floating-point profile"),
        ));
    }
    lowerer.require_strict_float_policy(assignment.location)?;
    let result = lowerer.emit_result(
        block,
        Type::F32,
        OperationKind::Binary {
            op: operation,
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

fn lower_disjoint_slice_get_tiled_2d_mut(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [receiver, witness, component, rows, columns, row_stride] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 6, call.operands.len(), call.location.clone()));
    };
    let MirOperandRef::Constant {
        literal: MirConstant::USize(0..=3),
        ..
    } = component
    else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            "DisjointSlice::get_tiled_2d_mut component must be an exact constant in 0..4",
        ));
    };
    let MirOperandRef::Place(witness_place) = witness else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "DisjointSlice::get_tiled_2d_mut witness must be an exact borrowed DisjointTile2D local",
        ));
    };
    let witness_shape_is_exact = matches!(
        lowerer.imported_local_shape(witness_place.local),
        Some(MirTypeShape::Reference { pointee, mutable: false })
            if matches!(
                pointee.as_ref(),
                MirTypeShape::Adt { identity }
                    if identity == TrustedDeviceItem::DisjointTile2D.canonical_path()
            )
    );
    let Some(LocalBinding::Tiled2dWitness {
        raw,
        evidence,
        some_entry,
    }) = witness_place
        .projection
        .is_empty()
        .then(|| lowerer.locals.get(&witness_place.local).copied())
        .flatten()
    else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "DisjointSlice::get_tiled_2d_mut witness did not preserve compiler-authenticated tiled output authority",
        ));
    };
    let exact_space = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
        lanes_per_tile: 64,
        tile_rows: 16,
        tile_columns: 16,
        elements_per_lane: 4,
    };
    let Some(use_block) = call.location.block else {
        return Err(diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            call.location.clone(),
            "DisjointSlice::get_tiled_2d_mut call has no MIR block identity",
        ));
    };
    if !witness_shape_is_exact
        || evidence.input_space() != SemanticDisjointIndexSpaceV1::Index1d
        || evidence.output_space() != exact_space
        || evidence.geometry() != (64, 16, 16, 4)
        || !crate::kernel_ir_lowering::mir_block_dominates(lowerer.function, some_entry, use_block)
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "DisjointSlice::get_tiled_2d_mut requires an exact guarded Tiled2D<Index1D, 64, 16, 16, 4> witness",
        ));
    }

    for (operand, role) in [
        (rows, "rows"),
        (columns, "columns"),
        (row_stride, "row stride"),
    ] {
        let MirOperandRef::Place(place) = operand else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                call.location.clone(),
                format!("DisjointSlice::get_tiled_2d_mut {role} must be an exact usize local"),
            ));
        };
        if !place.projection.is_empty()
            || lowerer.imported_local_shape(place.local) != Some(&MirTypeShape::USize)
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                call.location.clone(),
                format!("DisjointSlice::get_tiled_2d_mut {role} must be an exact usize local"),
            ));
        }
    }

    let receiver = lowerer.lower_operand(receiver, block, call.location)?;
    let component = lowerer.lower_operand(component, block, call.location)?;
    let rows = lowerer.lower_operand(rows, block, call.location)?;
    let columns = lowerer.lower_operand(columns, block, call.location)?;
    let row_stride = lowerer.lower_operand(row_stride, block, call.location)?;
    let receiver_ty = lowerer.value_type(receiver, call.location)?.clone();
    let Type::Slice(slice) = receiver_ty else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "DisjointSlice::get_tiled_2d_mut receiver is not a translated slice",
        ));
    };
    if slice.access != AccessMode::ReadWrite
        || slice.address_space != fe2o3_kernel_ir::AddressSpace::Global
        || slice.element.as_ref() != &Type::F32
        || lowerer.value_type(raw, call.location)? != &Type::INDEX
        || [component, rows, columns, row_stride]
            .into_iter()
            .any(|value| lowerer.value_type(value, call.location).ok() != Some(&Type::INDEX))
        || !call.destination.projection.is_empty()
        || !matches!(
            lowerer.local_shape(call.destination.local, call.location)?,
            MirTypeShape::Adt { identity }
                if matches!(identity.as_str(), "core::option::Option" | "std::option::Option")
        )
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "DisjointSlice::get_tiled_2d_mut does not match the exact writable f32 tiled-output signature",
        ));
    }
    let zero = emit_index_constant(lowerer, block, 0, call.location)?;
    let one = emit_index_constant(lowerer, block, 1, call.location)?;
    let four = emit_index_constant(lowerer, block, 4, call.location)?;
    let fifteen = emit_index_constant(lowerer, block, 15, call.location)?;
    let sixteen = emit_index_constant(lowerer, block, 16, call.location)?;
    let sixty_four = emit_index_constant(lowerer, block, 64, call.location)?;

    let (columns_rounded, columns_overflow) = emit_checked_index_binary(
        lowerer,
        block,
        CheckedBinaryOperator::Add,
        columns,
        fifteen,
        call.location,
    )?;
    let tiles_per_row = emit_index_binary(
        lowerer,
        block,
        BinaryOp::Divide,
        columns_rounded,
        sixteen,
        call.location,
    )?;
    let tiles_per_row_zero = emit_compare(
        lowerer,
        block,
        ComparePredicate::Equal,
        tiles_per_row,
        zero,
        call.location,
    )?;
    let safe_tiles_per_row = lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::Select {
            condition: tiles_per_row_zero,
            true_value: one,
            false_value: tiles_per_row,
        },
        call.location,
    )?;

    let tile = emit_index_binary(
        lowerer,
        block,
        BinaryOp::Divide,
        raw,
        sixty_four,
        call.location,
    )?;
    let lane = emit_index_binary(
        lowerer,
        block,
        BinaryOp::Remainder,
        raw,
        sixty_four,
        call.location,
    )?;
    let tile_row = emit_index_binary(
        lowerer,
        block,
        BinaryOp::Divide,
        tile,
        safe_tiles_per_row,
        call.location,
    )?;
    let tile_column = emit_index_binary(
        lowerer,
        block,
        BinaryOp::Remainder,
        tile,
        safe_tiles_per_row,
        call.location,
    )?;
    let lane_row = emit_index_binary(
        lowerer,
        block,
        BinaryOp::Divide,
        lane,
        sixteen,
        call.location,
    )?;
    let local_column = emit_index_binary(
        lowerer,
        block,
        BinaryOp::Remainder,
        lane,
        sixteen,
        call.location,
    )?;

    let (local_row_base, local_row_mul_overflow) = emit_checked_index_binary(
        lowerer,
        block,
        CheckedBinaryOperator::Multiply,
        lane_row,
        four,
        call.location,
    )?;
    let (local_row, local_row_add_overflow) = emit_checked_index_binary(
        lowerer,
        block,
        CheckedBinaryOperator::Add,
        local_row_base,
        component,
        call.location,
    )?;
    let (tile_row_base, tile_row_mul_overflow) = emit_checked_index_binary(
        lowerer,
        block,
        CheckedBinaryOperator::Multiply,
        tile_row,
        sixteen,
        call.location,
    )?;
    let (row, row_add_overflow) = emit_checked_index_binary(
        lowerer,
        block,
        CheckedBinaryOperator::Add,
        tile_row_base,
        local_row,
        call.location,
    )?;
    let (tile_column_base, tile_column_mul_overflow) = emit_checked_index_binary(
        lowerer,
        block,
        CheckedBinaryOperator::Multiply,
        tile_column,
        sixteen,
        call.location,
    )?;
    let (column, column_add_overflow) = emit_checked_index_binary(
        lowerer,
        block,
        CheckedBinaryOperator::Add,
        tile_column_base,
        local_column,
        call.location,
    )?;
    let (row_base, row_mul_overflow) = emit_checked_index_binary(
        lowerer,
        block,
        CheckedBinaryOperator::Multiply,
        row,
        row_stride,
        call.location,
    )?;
    let (index, index_add_overflow) = emit_checked_index_binary(
        lowerer,
        block,
        CheckedBinaryOperator::Add,
        row_base,
        column,
        call.location,
    )?;

    let component_valid = emit_compare(
        lowerer,
        block,
        ComparePredicate::LessThan,
        component,
        four,
        call.location,
    )?;
    let stride_valid = emit_compare(
        lowerer,
        block,
        ComparePredicate::GreaterThanOrEqual,
        row_stride,
        columns,
        call.location,
    )?;
    let tiles_per_row_valid = emit_bool_not(lowerer, block, tiles_per_row_zero, call.location)?;
    let row_valid = emit_compare(
        lowerer,
        block,
        ComparePredicate::LessThan,
        row,
        rows,
        call.location,
    )?;
    let column_valid = emit_compare(
        lowerer,
        block,
        ComparePredicate::LessThan,
        column,
        columns,
        call.location,
    )?;
    let length = lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::SliceLength { slice: receiver },
        call.location,
    )?;
    let index_valid = emit_compare(
        lowerer,
        block,
        ComparePredicate::LessThan,
        index,
        length,
        call.location,
    )?;
    let overflow = emit_bool_fold(
        lowerer,
        block,
        BinaryOp::BitOr,
        &[
            columns_overflow,
            local_row_mul_overflow,
            local_row_add_overflow,
            tile_row_mul_overflow,
            row_add_overflow,
            tile_column_mul_overflow,
            column_add_overflow,
            row_mul_overflow,
            index_add_overflow,
        ],
        call.location,
    )?;
    let no_overflow = emit_bool_not(lowerer, block, overflow, call.location)?;
    let present = emit_bool_fold(
        lowerer,
        block,
        BinaryOp::BitAnd,
        &[
            component_valid,
            stride_valid,
            tiles_per_row_valid,
            row_valid,
            column_valid,
            index_valid,
            no_overflow,
        ],
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
            discriminant: present,
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

fn emit_index_constant(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    value: u64,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<fe2o3_kernel_ir::ValueId, TranslationDiagnostic> {
    lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(value)),
        location,
    )
}

fn emit_index_binary(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    op: BinaryOp,
    lhs: fe2o3_kernel_ir::ValueId,
    rhs: fe2o3_kernel_ir::ValueId,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<fe2o3_kernel_ir::ValueId, TranslationDiagnostic> {
    lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::Binary { op, lhs, rhs },
        location,
    )
}

fn emit_checked_index_binary(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    operator: CheckedBinaryOperator,
    lhs: fe2o3_kernel_ir::ValueId,
    rhs: fe2o3_kernel_ir::ValueId,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<(fe2o3_kernel_ir::ValueId, fe2o3_kernel_ir::ValueId), TranslationDiagnostic> {
    let value = lowerer.fresh_value(Type::INDEX, location)?;
    let overflow = lowerer.fresh_value(Type::BOOL, location)?;
    let result = (value.id, overflow.id);
    block.operations.push(Operation::checked_binary(
        value, overflow, operator, lhs, rhs,
    ));
    Ok(result)
}

fn emit_compare(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    predicate: ComparePredicate,
    lhs: fe2o3_kernel_ir::ValueId,
    rhs: fe2o3_kernel_ir::ValueId,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<fe2o3_kernel_ir::ValueId, TranslationDiagnostic> {
    lowerer.emit_result(
        block,
        Type::BOOL,
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        },
        location,
    )
}

fn emit_bool_not(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    operand: fe2o3_kernel_ir::ValueId,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<fe2o3_kernel_ir::ValueId, TranslationDiagnostic> {
    lowerer.emit_result(
        block,
        Type::BOOL,
        OperationKind::Unary {
            op: fe2o3_kernel_ir::UnaryOp::Not,
            operand,
        },
        location,
    )
}

fn emit_bool_fold(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    op: BinaryOp,
    values: &[fe2o3_kernel_ir::ValueId],
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<fe2o3_kernel_ir::ValueId, TranslationDiagnostic> {
    let Some((&first, rest)) = values.split_first() else {
        return Err(diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            location.clone(),
            "boolean fold requires at least one value",
        ));
    };
    rest.iter().try_fold(first, |lhs, &rhs| {
        lowerer.emit_result(
            block,
            Type::BOOL,
            OperationKind::Binary { op, lhs, rhs },
            location,
        )
    })
}
