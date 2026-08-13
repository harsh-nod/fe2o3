//! Authenticated bounded gfx942 collective and deferred-barrier lowering.

use super::{FunctionLowerer, HandlerClaim, SessionRecognizedSemanticCall, TranslationDiagnostic};
use crate::kernel_ir_lowering::{LocalBinding, TranslationDiagnosticCode, diagnostic};
use crate::mir_import::MirOperandRef;
use crate::semantic_features::SessionRecognizedSemanticItem;
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BarrierSemantics, BasicBlock, BinaryOp, CastKind,
    ComparePredicate, Constant, Convergence, Fence, IndexKind, IntrinsicKind, IntrinsicOperation,
    MemoryAccess, MemoryOrdering, Operation, OperationKind, ScalarType, SynchronizationScope,
    Terminator, Type, ValueId, WaveOperation, WaveOperationKind, WaveWidth, WorkgroupBarrier,
    WorkgroupMemory, WorkgroupMemoryExtent, gfx942_xnack_minus_target_capability,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectiveScope {
    Wave64,
    Workgroup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectiveKind {
    Reduce,
    InclusiveScan,
    ExclusiveScan,
}

pub(super) fn claim_call(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> HandlerClaim {
    if is_wave_lds_v1(call.item) {
        let supported = match call.item {
            SessionRecognizedSemanticItem::TrustedDevice(
                TrustedDeviceItem::Gfx942Wave64ReduceActiveU32,
            ) => lowerer.is_gfx942_wave64_collective_context(),
            SessionRecognizedSemanticItem::TrustedDevice(
                TrustedDeviceItem::Gfx942StaticLdsU32x256
                | TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32,
            ) => lowerer.gfx942_collective_workgroup_size() == Some(256),
            _ => unreachable!("wave/LDS V1 classifier is exact"),
        };
        return if supported {
            HandlerClaim::Owned
        } else {
            reject_context(call)
        };
    }

    let Some((scope, _)) = collective(call.item) else {
        if !is_owned_non_collective(call.item) {
            return HandlerClaim::NotOwned;
        }
        if !lowerer.is_gfx942_collective_v1_context() {
            return reject_context(call);
        }
        return HandlerClaim::Owned;
    };

    let supported = match scope {
        CollectiveScope::Wave64 => lowerer.is_gfx942_wave64_collective_context(),
        CollectiveScope::Workgroup => lowerer.is_gfx942_collective_v1_context(),
    };
    if !supported {
        return reject_context(call);
    }
    HandlerClaim::Owned
}

pub(super) fn lower_call(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    if let Some((scope, kind)) = collective(call.item) {
        return lower_collective(lowerer, call, block, scope, kind);
    }

    match call.item {
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::Gfx942CollectivesFromCompiler,
        ) => lower_context_constructor(lowerer, call),
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::Gfx942BarrierArrive) => {
            lower_arrive(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::Gfx942BarrierWait) => {
            lower_wait(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(TrustedDeviceItem::Gfx942StaticLdsU32x256) => {
            lower_static_lds_u32x256(lowerer, call, block)
        }
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::Gfx942Wave64ReduceActiveU32,
        ) => lower_wave64_reduce_active_u32(lowerer, call, block),
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32,
        ) => lower_workgroup256_reduce_active_u32(lowerer, call, block),
        _ => unreachable!("only claimed collective-v1 calls reach lowering"),
    }
}

fn is_wave_lds_v1(item: SessionRecognizedSemanticItem) -> bool {
    matches!(
        item,
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::Gfx942StaticLdsU32x256
                | TrustedDeviceItem::Gfx942Wave64ReduceActiveU32
                | TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32
        )
    )
}

fn is_owned_non_collective(item: SessionRecognizedSemanticItem) -> bool {
    matches!(
        item,
        SessionRecognizedSemanticItem::TrustedDevice(
            TrustedDeviceItem::Gfx942CollectivesFromCompiler
                | TrustedDeviceItem::Gfx942BarrierArrive
                | TrustedDeviceItem::Gfx942BarrierWait
        )
    )
}

fn lower_static_lds_u32x256(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [context] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 1, call.operands.len(), call.location.clone()));
    };
    require_context(lowerer, context, call)?;
    if !call.destination.projection.is_empty() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedProjection,
            call.location.clone(),
            "gfx942 static LDS requires an unprojected destination",
        ));
    }
    let pointer = lowerer.emit_result(
        block,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        ),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: Type::Scalar(ScalarType::U32),
            extent: WorkgroupMemoryExtent::Static(256),
            alignment: 4,
        }),
        call.location,
    )?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Gfx942StaticLdsU32x256(pointer),
        call.location.clone(),
    )?;
    branch_to_target(lowerer, call)
}

fn lower_wave64_reduce_active_u32(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [context, active, value] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 3, call.operands.len(), call.location.clone()));
    };
    require_context(lowerer, context, call)?;
    let masked = lower_active_u32(lowerer, block, active, value, true, call)?;
    let result = lower_wave64(
        lowerer,
        block,
        masked,
        &Type::Scalar(ScalarType::U32),
        CollectiveKind::Reduce,
        call.location,
    )?;
    lowerer.require_destination_type(
        call.destination,
        &Type::Scalar(ScalarType::U32),
        call.location,
    )?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(result),
        call.location.clone(),
    )?;
    branch_to_target(lowerer, call)
}

fn lower_workgroup256_reduce_active_u32(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let [context, scratch, active, value] = call.operands else {
        return Err(lowerer.call_arity(call.callee, 4, call.operands.len(), call.location.clone()));
    };
    require_context(lowerer, context, call)?;
    let scratch = require_static_lds_u32x256(lowerer, scratch, call)?;
    let masked = lower_active_u32(lowerer, block, active, value, false, call)?;
    let result = lower_workgroup_with_scratch(
        lowerer,
        block,
        scratch,
        masked,
        &Type::Scalar(ScalarType::U32),
        CollectiveKind::Reduce,
        256,
        call.location,
    )?;
    lowerer.require_destination_type(
        call.destination,
        &Type::Scalar(ScalarType::U32),
        call.location,
    )?;
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(result),
        call.location.clone(),
    )?;
    branch_to_target(lowerer, call)
}

fn require_static_lds_u32x256(
    lowerer: &FunctionLowerer<'_, '_>,
    operand: &MirOperandRef,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<ValueId, TranslationDiagnostic> {
    let MirOperandRef::Place(place) = operand else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "static LDS must be an authenticated local capability",
        ));
    };
    if !place.projection.is_empty() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedProjection,
            call.location.clone(),
            "static LDS does not support projected authority",
        ));
    }
    match lowerer.locals.get(&place.local) {
        Some(LocalBinding::Gfx942StaticLdsU32x256(pointer)) => Ok(*pointer),
        _ => Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "static LDS did not originate from the authenticated compiler constructor",
        )),
    }
}

fn lower_active_u32(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    active: &MirOperandRef,
    value: &MirOperandRef,
    record_wave_mask: bool,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<ValueId, TranslationDiagnostic> {
    let active = lowerer.lower_operand(active, block, call.location)?;
    let value = lowerer.lower_operand(value, block, call.location)?;
    let u32_type = Type::Scalar(ScalarType::U32);
    if lowerer.value_type(active, call.location)? != &u32_type
        || lowerer.value_type(value, call.location)? != &u32_type
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "gfx942 active-u32 reduction requires u32 activity and value operands",
        ));
    }
    let zero = constant_u32(lowerer, block, 0, call.location)?;
    let predicate = compare(
        lowerer,
        block,
        ComparePredicate::NotEqual,
        active,
        zero,
        call.location,
    )?;
    if record_wave_mask {
        let _mask = lowerer.emit_result(
            block,
            Type::Scalar(ScalarType::U64),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::Ballot { predicate },
                WaveWidth::Wave64,
            )),
            call.location,
        )?;
    }
    select(
        lowerer,
        block,
        predicate,
        value,
        zero,
        u32_type,
        call.location,
    )
}

fn collective(item: SessionRecognizedSemanticItem) -> Option<(CollectiveScope, CollectiveKind)> {
    use CollectiveKind::{ExclusiveScan, InclusiveScan, Reduce};
    use CollectiveScope::{Wave64, Workgroup};
    use TrustedDeviceItem::{
        Gfx942Wave64ExclusiveScanSum, Gfx942Wave64InclusiveScanSum, Gfx942Wave64ReduceSum,
        Gfx942WorkgroupExclusiveScanSum, Gfx942WorkgroupInclusiveScanSum, Gfx942WorkgroupReduceSum,
    };
    match item {
        SessionRecognizedSemanticItem::TrustedDevice(Gfx942Wave64ReduceSum) => {
            Some((Wave64, Reduce))
        }
        SessionRecognizedSemanticItem::TrustedDevice(Gfx942Wave64InclusiveScanSum) => {
            Some((Wave64, InclusiveScan))
        }
        SessionRecognizedSemanticItem::TrustedDevice(Gfx942Wave64ExclusiveScanSum) => {
            Some((Wave64, ExclusiveScan))
        }
        SessionRecognizedSemanticItem::TrustedDevice(Gfx942WorkgroupReduceSum) => {
            Some((Workgroup, Reduce))
        }
        SessionRecognizedSemanticItem::TrustedDevice(Gfx942WorkgroupInclusiveScanSum) => {
            Some((Workgroup, InclusiveScan))
        }
        SessionRecognizedSemanticItem::TrustedDevice(Gfx942WorkgroupExclusiveScanSum) => {
            Some((Workgroup, ExclusiveScan))
        }
        _ => None,
    }
}

fn reject_context(call: SessionRecognizedSemanticCall<'_>) -> HandlerClaim {
    HandlerClaim::Reject(diagnostic(
        TranslationDiagnosticCode::UnsupportedCall,
        call.location.clone(),
        format!(
            "session-recognized collective call `{}` requires exact gfx942:xnack- General V3, a one-dimensional power-of-two workgroup no larger than 256, and wave64-compatible width for wave operations",
            call.callee.identity()
        ),
    ))
}

fn lower_context_constructor(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<Terminator, TranslationDiagnostic> {
    if !call.operands.is_empty() {
        return Err(lowerer.call_arity(call.callee, 0, call.operands.len(), call.location.clone()));
    }
    if !call.destination.projection.is_empty() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedProjection,
            call.location.clone(),
            "gfx942 collective context requires an unprojected destination",
        ));
    }
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Gfx942CollectiveCapability,
        call.location.clone(),
    )?;
    lowerer
        .required_capabilities
        .insert(gfx942_xnack_minus_target_capability());
    branch_to_target(lowerer, call)
}

fn lower_arrive(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    require_no_operands(lowerer, call)?;
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::Fence(Fence {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::Release,
                [AddressSpace::Global, AddressSpace::Workgroup],
            ),
        }),
    ));
    branch_to_target(lowerer, call)
}

fn lower_wait(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    require_no_operands(lowerer, call)?;
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::Acquire,
                [AddressSpace::Global, AddressSpace::Workgroup],
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    ));
    branch_to_target(lowerer, call)
}

fn require_no_operands(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<(), TranslationDiagnostic> {
    if call.operands.is_empty() {
        Ok(())
    } else {
        Err(lowerer.call_arity(call.callee, 0, call.operands.len(), call.location.clone()))
    }
}

fn lower_collective(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
    scope: CollectiveScope,
    kind: CollectiveKind,
) -> Result<Terminator, TranslationDiagnostic> {
    let (context, value) = match (scope, call.operands) {
        (CollectiveScope::Wave64, [_, context, value]) => (context, value),
        (CollectiveScope::Workgroup, [_, context, _, value]) => (context, value),
        (CollectiveScope::Wave64, operands) => {
            return Err(lowerer.call_arity(call.callee, 3, operands.len(), call.location.clone()));
        }
        (CollectiveScope::Workgroup, operands) => {
            return Err(lowerer.call_arity(call.callee, 4, operands.len(), call.location.clone()));
        }
    };
    require_context(lowerer, context, call)?;
    let value = lowerer.lower_operand(value, block, call.location)?;
    let ty = lowerer.value_type(value, call.location)?.clone();
    if !matches!(
        ty,
        Type::Scalar(ScalarType::U32 | ScalarType::I32 | ScalarType::F32)
    ) {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "gfx942 sum collectives support exactly u32, i32, and f32",
        ));
    }
    lowerer.require_destination_type(call.destination, &ty, call.location)?;

    let result = match scope {
        CollectiveScope::Wave64 => lower_wave64(lowerer, block, value, &ty, kind, call.location)?,
        CollectiveScope::Workgroup => {
            let size = lowerer
                .gfx942_collective_workgroup_size()
                .expect("claim authenticated the workgroup");
            lower_workgroup(lowerer, block, value, &ty, kind, size, call.location)?
        }
    };
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(result),
        call.location.clone(),
    )?;
    branch_to_target(lowerer, call)
}

fn require_context(
    lowerer: &FunctionLowerer<'_, '_>,
    operand: &MirOperandRef,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<(), TranslationDiagnostic> {
    let MirOperandRef::Place(place) = operand else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "collective context must be an authenticated local capability",
        ));
    };
    if !place.projection.is_empty()
        || !matches!(
            lowerer.locals.get(&place.local),
            Some(LocalBinding::Gfx942CollectiveCapability)
        )
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            call.location.clone(),
            "collective context did not originate from the authenticated compiler constructor",
        ));
    }
    Ok(())
}

fn lower_wave64(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    value: ValueId,
    ty: &Type,
    kind: CollectiveKind,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    let rank = lowerer.emit_result(
        block,
        Type::Scalar(ScalarType::U32),
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::LaneId,
            WaveWidth::Wave64,
        )),
        location,
    )?;
    match kind {
        CollectiveKind::Reduce => {
            let mut result = value;
            for offset in [32, 16, 8, 4, 2, 1] {
                let offset = constant_u32(lowerer, block, offset, location)?;
                let source = binary(
                    lowerer,
                    block,
                    BinaryOp::BitXor,
                    rank,
                    offset,
                    Type::Scalar(ScalarType::U32),
                    location,
                )?;
                let peer = shuffle(lowerer, block, result, source, ty, location)?;
                result = binary(
                    lowerer,
                    block,
                    BinaryOp::Add,
                    result,
                    peer,
                    ty.clone(),
                    location,
                )?;
            }
            Ok(result)
        }
        CollectiveKind::InclusiveScan | CollectiveKind::ExclusiveScan => {
            let mut result = value;
            for offset in [1, 2, 4, 8, 16, 32] {
                let offset = constant_u32(lowerer, block, offset, location)?;
                let active = compare(
                    lowerer,
                    block,
                    ComparePredicate::GreaterThanOrEqual,
                    rank,
                    offset,
                    location,
                )?;
                let sub = binary(
                    lowerer,
                    block,
                    BinaryOp::Subtract,
                    rank,
                    offset,
                    Type::Scalar(ScalarType::U32),
                    location,
                )?;
                let zero = constant_u32(lowerer, block, 0, location)?;
                let source = select(
                    lowerer,
                    block,
                    active,
                    sub,
                    zero,
                    Type::Scalar(ScalarType::U32),
                    location,
                )?;
                let peer = shuffle(lowerer, block, result, source, ty, location)?;
                let sum = binary(
                    lowerer,
                    block,
                    BinaryOp::Add,
                    peer,
                    result,
                    ty.clone(),
                    location,
                )?;
                result = select(lowerer, block, active, sum, result, ty.clone(), location)?;
            }
            if kind == CollectiveKind::InclusiveScan {
                return Ok(result);
            }
            let one = constant_u32(lowerer, block, 1, location)?;
            let nonzero = compare(
                lowerer,
                block,
                ComparePredicate::GreaterThanOrEqual,
                rank,
                one,
                location,
            )?;
            let source = binary(
                lowerer,
                block,
                BinaryOp::Subtract,
                rank,
                one,
                Type::Scalar(ScalarType::U32),
                location,
            )?;
            let previous = shuffle(lowerer, block, result, source, ty, location)?;
            let zero = zero_for(lowerer, block, ty, location)?;
            select(
                lowerer,
                block,
                nonzero,
                previous,
                zero,
                ty.clone(),
                location,
            )
        }
    }
}

fn lower_workgroup(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    value: ValueId,
    ty: &Type,
    kind: CollectiveKind,
    size: u32,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    let scratch = lowerer.emit_result(
        block,
        Type::pointer(ty.clone(), AddressSpace::Workgroup, AccessMode::ReadWrite),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: ty.clone(),
            extent: WorkgroupMemoryExtent::Static(size),
            alignment: 4,
        }),
        location,
    )?;
    lower_workgroup_with_scratch(lowerer, block, scratch, value, ty, kind, size, location)
}

#[allow(clippy::too_many_arguments)]
fn lower_workgroup_with_scratch(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    scratch: ValueId,
    value: ValueId,
    ty: &Type,
    kind: CollectiveKind,
    size: u32,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    let rank = lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
        location,
    )?;
    store_at(lowerer, block, scratch, rank, value, location)?;
    barrier(block, MemoryOrdering::AcquireRelease);

    match kind {
        CollectiveKind::Reduce => {
            let mut offset = size >> 1;
            while offset != 0 {
                let offset_value = constant_index(lowerer, block, offset, location)?;
                let active = compare(
                    lowerer,
                    block,
                    ComparePredicate::LessThan,
                    rank,
                    offset_value,
                    location,
                )?;
                let pair = binary(
                    lowerer,
                    block,
                    BinaryOp::Add,
                    rank,
                    offset_value,
                    Type::INDEX,
                    location,
                )?;
                let zero_index = constant_index(lowerer, block, 0, location)?;
                let safe_pair = select(
                    lowerer,
                    block,
                    active,
                    pair,
                    zero_index,
                    Type::INDEX,
                    location,
                )?;
                let lhs = load_at(lowerer, block, scratch, rank, ty, location)?;
                let rhs = load_at(lowerer, block, scratch, safe_pair, ty, location)?;
                let sum = binary(
                    lowerer,
                    block,
                    BinaryOp::Add,
                    lhs,
                    rhs,
                    ty.clone(),
                    location,
                )?;
                let next = select(lowerer, block, active, sum, lhs, ty.clone(), location)?;
                barrier(block, MemoryOrdering::AcquireRelease);
                store_at(lowerer, block, scratch, rank, next, location)?;
                barrier(block, MemoryOrdering::AcquireRelease);
                offset >>= 1;
            }
            let zero = constant_index(lowerer, block, 0, location)?;
            let result = load_at(lowerer, block, scratch, zero, ty, location)?;
            barrier(block, MemoryOrdering::AcquireRelease);
            Ok(result)
        }
        CollectiveKind::InclusiveScan | CollectiveKind::ExclusiveScan => {
            let mut offset = 1;
            while offset < size {
                let offset_value = constant_index(lowerer, block, offset, location)?;
                let active = compare(
                    lowerer,
                    block,
                    ComparePredicate::GreaterThanOrEqual,
                    rank,
                    offset_value,
                    location,
                )?;
                let predecessor = binary(
                    lowerer,
                    block,
                    BinaryOp::Subtract,
                    rank,
                    offset_value,
                    Type::INDEX,
                    location,
                )?;
                let zero_index = constant_index(lowerer, block, 0, location)?;
                let safe_predecessor = select(
                    lowerer,
                    block,
                    active,
                    predecessor,
                    zero_index,
                    Type::INDEX,
                    location,
                )?;
                let current = load_at(lowerer, block, scratch, rank, ty, location)?;
                let prefix = load_at(lowerer, block, scratch, safe_predecessor, ty, location)?;
                barrier(block, MemoryOrdering::AcquireRelease);
                let sum = binary(
                    lowerer,
                    block,
                    BinaryOp::Add,
                    prefix,
                    current,
                    ty.clone(),
                    location,
                )?;
                let next = select(lowerer, block, active, sum, current, ty.clone(), location)?;
                store_at(lowerer, block, scratch, rank, next, location)?;
                barrier(block, MemoryOrdering::AcquireRelease);
                offset <<= 1;
            }
            let inclusive = load_at(lowerer, block, scratch, rank, ty, location)?;
            if kind == CollectiveKind::InclusiveScan {
                barrier(block, MemoryOrdering::AcquireRelease);
                return Ok(inclusive);
            }
            let one = constant_index(lowerer, block, 1, location)?;
            let nonzero = compare(
                lowerer,
                block,
                ComparePredicate::GreaterThanOrEqual,
                rank,
                one,
                location,
            )?;
            let predecessor = binary(
                lowerer,
                block,
                BinaryOp::Subtract,
                rank,
                one,
                Type::INDEX,
                location,
            )?;
            let zero_index = constant_index(lowerer, block, 0, location)?;
            let safe_predecessor = select(
                lowerer,
                block,
                nonzero,
                predecessor,
                zero_index,
                Type::INDEX,
                location,
            )?;
            let previous = load_at(lowerer, block, scratch, safe_predecessor, ty, location)?;
            let zero = zero_for(lowerer, block, ty, location)?;
            let result = select(
                lowerer,
                block,
                nonzero,
                previous,
                zero,
                ty.clone(),
                location,
            )?;
            barrier(block, MemoryOrdering::AcquireRelease);
            Ok(result)
        }
    }
}

fn shuffle(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    value: ValueId,
    source: ValueId,
    ty: &Type,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    let (bits, bits_ty) = if ty == &Type::Scalar(ScalarType::F32) {
        (
            lowerer.emit_result(
                block,
                Type::Scalar(ScalarType::U32),
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value,
                    to: Type::Scalar(ScalarType::U32),
                },
                location,
            )?,
            Type::Scalar(ScalarType::U32),
        )
    } else {
        (value, ty.clone())
    };
    let shuffled = lowerer.emit_result(
        block,
        bits_ty,
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::ShuffleIndex {
                value: bits,
                source_lane: source,
                tile_width: 64,
            },
            WaveWidth::Wave64,
        )),
        location,
    )?;
    if ty == &Type::Scalar(ScalarType::F32) {
        lowerer.emit_result(
            block,
            ty.clone(),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: shuffled,
                to: ty.clone(),
            },
            location,
        )
    } else {
        Ok(shuffled)
    }
}

fn load_at(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    scratch: ValueId,
    index: ValueId,
    ty: &Type,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    let pointer = lowerer.emit_result(
        block,
        Type::pointer(ty.clone(), AddressSpace::Workgroup, AccessMode::ReadWrite),
        OperationKind::GetElementPointer {
            base: scratch,
            offset: index,
        },
        location,
    )?;
    lowerer.emit_result(
        block,
        ty.clone(),
        OperationKind::Load {
            pointer,
            access: MemoryAccess::new(AddressSpace::Workgroup, 4),
        },
        location,
    )
}

fn store_at(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    scratch: ValueId,
    index: ValueId,
    value: ValueId,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<(), TranslationDiagnostic> {
    let ty = lowerer.value_type(value, location)?.clone();
    let pointer = lowerer.emit_result(
        block,
        Type::pointer(ty, AddressSpace::Workgroup, AccessMode::ReadWrite),
        OperationKind::GetElementPointer {
            base: scratch,
            offset: index,
        },
        location,
    )?;
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::Store {
            pointer,
            value,
            access: MemoryAccess::new(AddressSpace::Workgroup, 4),
        },
    ));
    Ok(())
}

fn barrier(block: &mut BasicBlock, ordering: MemoryOrdering) {
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(ordering, [AddressSpace::Workgroup]),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    ));
}

fn binary(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    op: BinaryOp,
    lhs: ValueId,
    rhs: ValueId,
    ty: Type,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    lowerer.emit_result(block, ty, OperationKind::Binary { op, lhs, rhs }, location)
}

fn compare(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    predicate: ComparePredicate,
    lhs: ValueId,
    rhs: ValueId,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
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

fn select(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    condition: ValueId,
    true_value: ValueId,
    false_value: ValueId,
    ty: Type,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    lowerer.emit_result(
        block,
        ty,
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        },
        location,
    )
}

fn constant_u32(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    value: u32,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    lowerer.emit_result(
        block,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(value)),
        location,
    )
}

fn constant_index(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    value: u32,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    lowerer.emit_result(
        block,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(u64::from(value))),
        location,
    )
}

fn zero_for(
    lowerer: &mut FunctionLowerer<'_, '_>,
    block: &mut BasicBlock,
    ty: &Type,
    location: &crate::kernel_ir_lowering::TranslationLocation,
) -> Result<ValueId, TranslationDiagnostic> {
    let constant = match ty {
        Type::Scalar(ScalarType::U32) => Constant::U32(0),
        Type::Scalar(ScalarType::I32) => Constant::I32(0),
        Type::Scalar(ScalarType::F32) => Constant::F32Bits(0),
        _ => unreachable!("collective type was validated"),
    };
    lowerer.emit_result(
        block,
        ty.clone(),
        OperationKind::Constant(constant),
        location,
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

#[cfg(test)]
mod tests;
