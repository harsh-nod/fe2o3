use std::collections::{BTreeMap, BTreeSet};

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME, AccessMode, Atomic, Axis, BarrierSemantics,
    BasicBlock, BinaryOp, CastKind, ComparePredicate, Constant, Convergence, ExecutionAtomicKindV1,
    ExecutionCapabilityOpV1, ExecutionCapabilityOperationV1, ExecutionCapabilityRoleV1,
    ExecutionCollectiveKindV1, ExecutionElementLayoutV1, ExecutionMemoryAddressSpaceV1,
    ExecutionMemorySemanticsV1, ExecutionTypeIdentityV1, Function, FunctionId, IndexKind,
    IntrinsicKind, IntrinsicOperation, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, SynchronizationScope, TargetCapability, Terminator, Type, ValueDef, ValueId,
    WaveOperation, WaveOperationKind, WaveWidth, WorkgroupBarrier, WorkgroupMemory,
    WorkgroupMemoryExtent, WorkgroupSize,
};

use super::{
    LoweringDiagnosticCode, LoweringErrors, LoweringLocation, V13LoweringAuthorityV1,
    target_requirements_for_execution_operation_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhysicalExtentV1 {
    Static(u64),
    Dynamic(ValueId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PhysicalValueV1 {
    value: ValueId,
    extent: Option<PhysicalExtentV1>,
}

impl PhysicalValueV1 {
    const fn scalar(value: ValueId) -> Self {
        Self {
            value,
            extent: None,
        }
    }

    const fn view(value: ValueId, extent: PhysicalExtentV1) -> Self {
        Self {
            value,
            extent: Some(extent),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_subgroup_collective(
    module: &Module,
    operation: &Operation,
    contract: &ExecutionCapabilityOpV1,
    kind: ExecutionCollectiveKindV1,
    scalar: ScalarType,
    width: WaveWidth,
    original_types: &BTreeMap<ValueId, Type>,
    lowered_types: &mut BTreeMap<ValueId, Type>,
    aliases: &BTreeMap<ValueId, ExecutionAliasV1>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<(), LoweringErrors> {
    if !matches!(scalar, ScalarType::U32 | ScalarType::I32 | ScalarType::F32) {
        return Err(incomplete(
            module,
            format!("V13 subgroup {kind:?} has no reviewed {scalar:?} arithmetic lowering"),
        ));
    }
    let physical = physical_operands(contract, original_types, lowered_types, aliases);
    let value = require_scalar_type(module, &physical, scalar)?;
    let result = sole_ordinary_scalar_result(module, operation, scalar)?;
    let lane = emit_wave_lane(module, width, lowered_types, next_value, output)?;

    let inclusive = match kind {
        ExecutionCollectiveKindV1::ReduceSum => emit_subgroup_reduce_sum(
            module,
            value,
            scalar,
            lane,
            width,
            result.id,
            lowered_types,
            next_value,
            output,
        )?,
        ExecutionCollectiveKindV1::InclusiveScanSum
        | ExecutionCollectiveKindV1::ExclusiveScanSum => {
            let destination =
                (kind == ExecutionCollectiveKindV1::InclusiveScanSum).then_some(result.id);
            emit_subgroup_inclusive_scan(
                module,
                value,
                scalar,
                lane,
                width,
                destination,
                lowered_types,
                next_value,
                output,
            )?
        }
    };
    if kind == ExecutionCollectiveKindV1::ExclusiveScanSum {
        emit_subgroup_exclusive_result(
            module,
            inclusive,
            scalar,
            lane,
            width,
            result.id,
            lowered_types,
            next_value,
            output,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_subgroup_reduce_sum(
    module: &Module,
    value: ValueId,
    scalar: ScalarType,
    lane: ValueId,
    width: WaveWidth,
    result: ValueId,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let mut reduced = value;
    let stages = width.lanes().trailing_zeros();
    for stage in 0..stages {
        let distance = emit_constant_u32(module, 1 << stage, types, next_value, output)?;
        let source = emit_binary(
            module,
            BinaryOp::BitXor,
            lane,
            distance,
            Type::Scalar(ScalarType::U32),
            None,
            types,
            next_value,
            output,
        )?;
        let peer = emit_wave_shuffle(
            module, reduced, scalar, source, width, types, next_value, output,
        )?;
        let destination = (stage + 1 == stages).then_some(result);
        reduced = emit_binary(
            module,
            BinaryOp::Add,
            reduced,
            peer,
            Type::Scalar(scalar),
            destination,
            types,
            next_value,
            output,
        )?;
    }
    Ok(reduced)
}

#[allow(clippy::too_many_arguments)]
fn emit_subgroup_inclusive_scan(
    module: &Module,
    value: ValueId,
    scalar: ScalarType,
    lane: ValueId,
    width: WaveWidth,
    result: Option<ValueId>,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let mut prefix = value;
    let stages = width.lanes().trailing_zeros();
    for stage in 0..stages {
        let distance = emit_constant_u32(module, 1 << stage, types, next_value, output)?;
        let active = emit_compare(
            module,
            ComparePredicate::GreaterThanOrEqual,
            lane,
            distance,
            types,
            next_value,
            output,
        )?;
        let source = emit_binary(
            module,
            BinaryOp::Subtract,
            lane,
            distance,
            Type::Scalar(ScalarType::U32),
            None,
            types,
            next_value,
            output,
        )?;
        let peer = emit_wave_shuffle(
            module, prefix, scalar, source, width, types, next_value, output,
        )?;
        let sum = emit_binary(
            module,
            BinaryOp::Add,
            peer,
            prefix,
            Type::Scalar(scalar),
            None,
            types,
            next_value,
            output,
        )?;
        let destination = (stage + 1 == stages).then_some(result).flatten();
        prefix = emit_select(
            module,
            active,
            sum,
            prefix,
            Type::Scalar(scalar),
            destination,
            types,
            next_value,
            output,
        )?;
    }
    Ok(prefix)
}

#[allow(clippy::too_many_arguments)]
fn emit_subgroup_exclusive_result(
    module: &Module,
    inclusive: ValueId,
    scalar: ScalarType,
    lane: ValueId,
    width: WaveWidth,
    result: ValueId,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<(), LoweringErrors> {
    let one = emit_constant_u32(module, 1, types, next_value, output)?;
    let has_predecessor = emit_compare(
        module,
        ComparePredicate::GreaterThanOrEqual,
        lane,
        one,
        types,
        next_value,
        output,
    )?;
    let predecessor = emit_binary(
        module,
        BinaryOp::Subtract,
        lane,
        one,
        Type::Scalar(ScalarType::U32),
        None,
        types,
        next_value,
        output,
    )?;
    let prior = emit_wave_shuffle(
        module,
        inclusive,
        scalar,
        predecessor,
        width,
        types,
        next_value,
        output,
    )?;
    let zero = emit_scalar_zero(module, scalar, types, next_value, output)?;
    emit_select(
        module,
        has_predecessor,
        prior,
        zero,
        Type::Scalar(scalar),
        Some(result),
        types,
        next_value,
        output,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_wave_shuffle(
    module: &Module,
    value: ValueId,
    scalar: ScalarType,
    source: ValueId,
    width: WaveWidth,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let shuffled_type = if scalar == ScalarType::F32 {
        ScalarType::U32
    } else {
        scalar
    };
    let shuffled_value = if scalar == ScalarType::F32 {
        emit_cast(
            module,
            CastKind::Bitcast,
            value,
            Type::Scalar(ScalarType::U32),
            types,
            next_value,
            output,
        )?
    } else {
        value
    };
    let shuffled = emit_value(
        module,
        Type::Scalar(shuffled_type),
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::ShuffleIndex {
                value: shuffled_value,
                source_lane: source,
                tile_width: width.lanes(),
            },
            width,
        )),
        types,
        next_value,
        output,
    )?;
    if scalar == ScalarType::F32 {
        emit_cast(
            module,
            CastKind::Bitcast,
            shuffled,
            Type::F32,
            types,
            next_value,
            output,
        )
    } else {
        Ok(shuffled)
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_workgroup_collective(
    module: &Module,
    function: &FunctionId,
    operation: &Operation,
    contract: &ExecutionCapabilityOpV1,
    kind: ExecutionCollectiveKindV1,
    scalar: ScalarType,
    layout: ExecutionElementLayoutV1,
    elements: u64,
    original_types: &BTreeMap<ValueId, Type>,
    types: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<(), LoweringErrors> {
    if !matches!(scalar, ScalarType::U32 | ScalarType::I32 | ScalarType::F32) {
        return Err(incomplete(
            module,
            format!("V13 workgroup {kind:?} has no reviewed {scalar:?} arithmetic lowering"),
        ));
    }
    let size = u32::try_from(elements)
        .ok()
        .filter(|size| *size != 0 && *size <= 1024)
        .ok_or_else(|| incomplete(module, "V13 workgroup collective extent exceeds 1024"))?;
    let workgroup_size = require_exact_workgroup(module, function, size)?;
    if kind == ExecutionCollectiveKindV1::ReduceSum && !size.is_power_of_two() {
        return Err(incomplete(
            module,
            "V13 workgroup reduction requires the reviewed power-of-two tree geometry",
        ));
    }
    let physical = physical_operands(contract, original_types, types, aliases);
    let scratch = require_pointer(module, &physical)?;
    let value = require_scalar_type(module, &physical, scalar)?;
    let result = sole_ordinary_scalar_result(module, operation, scalar)?;
    let rank = emit_flat_local_index(module, workgroup_size, types, next_value, output)?;
    emit_memory_store(
        module,
        scratch,
        rank,
        value,
        ExecutionMemoryAddressSpaceV1::Workgroup,
        layout.byte_alignment,
        types,
        next_value,
        output,
    )?;
    output.push(default_workgroup_barrier());

    let collective_result = match kind {
        ExecutionCollectiveKindV1::ReduceSum => emit_workgroup_reduce_sum(
            module,
            scratch,
            scalar,
            rank,
            size,
            layout.byte_alignment,
            result.id,
            types,
            next_value,
            output,
        )?,
        ExecutionCollectiveKindV1::InclusiveScanSum
        | ExecutionCollectiveKindV1::ExclusiveScanSum => emit_workgroup_scan_sum(
            module,
            scratch,
            scalar,
            rank,
            size,
            layout.byte_alignment,
            kind,
            result.id,
            types,
            next_value,
            output,
        )?,
    };
    debug_assert_eq!(collective_result, result.id);
    output.push(default_workgroup_barrier());
    alias_capability_results(
        module,
        operation,
        PhysicalValueV1::view(scratch, PhysicalExtentV1::Static(elements)),
        aliases,
    )
}

fn require_exact_workgroup(
    module: &Module,
    function: &FunctionId,
    elements: u32,
) -> Result<WorkgroupSize, LoweringErrors> {
    let size = static_workgroup_size(module, function)?;
    let flat = size
        .x
        .checked_mul(size.y)
        .and_then(|xy| xy.checked_mul(size.z));
    if flat == Some(elements) {
        Ok(size)
    } else {
        Err(incomplete(
            module,
            format!(
                "V13 workgroup collective extent {elements} does not match its static launch geometry"
            ),
        ))
    }
}

fn require_scalar_type(
    module: &Module,
    physical: &[(ValueId, Type)],
    scalar: ScalarType,
) -> Result<ValueId, LoweringErrors> {
    physical
        .iter()
        .find_map(|(value, ty)| (ty == &Type::Scalar(scalar)).then_some(*value))
        .ok_or_else(|| incomplete(module, format!("V13 operation has no {scalar:?} carrier")))
}

fn sole_ordinary_scalar_result<'a>(
    module: &Module,
    operation: &'a Operation,
    scalar: ScalarType,
) -> Result<&'a ValueDef, LoweringErrors> {
    let results = ordinary_results(operation);
    match results.as_slice() {
        [result] if result.ty == Type::Scalar(scalar) => operation
            .results
            .iter()
            .find(|candidate| candidate.id == result.id)
            .ok_or_else(|| incomplete(module, "V13 physical result disappeared")),
        _ => Err(incomplete(
            module,
            format!("V13 operation requires exactly one ordinary {scalar:?} result"),
        )),
    }
}

fn sole_ordinary_bool_result<'a>(
    module: &Module,
    operation: &'a Operation,
) -> Result<&'a ValueDef, LoweringErrors> {
    let results = ordinary_results(operation);
    match results.as_slice() {
        [result] if result.ty == Type::BOOL => operation
            .results
            .iter()
            .find(|candidate| candidate.id == result.id)
            .ok_or_else(|| incomplete(module, "V13 boolean result disappeared")),
        _ => Err(incomplete(
            module,
            "V13 bounds-checked store requires exactly one boolean result",
        )),
    }
}

fn reject_capability_results(module: &Module, operation: &Operation) -> Result<(), LoweringErrors> {
    if operation
        .results
        .iter()
        .any(|result| matches!(result.ty, Type::ExecutionCapability(_)))
    {
        Err(incomplete(
            module,
            "V13 physical result operation unexpectedly returns logical authority",
        ))
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_value(
    _module: &Module,
    ty: Type,
    kind: OperationKind,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let result = fresh_value(next_value)?;
    output.push(Operation::effect_free(
        ValueDef::new(result, ty.clone()),
        kind,
    ));
    types.insert(result, ty);
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn emit_value_at(
    _module: &Module,
    result: ValueId,
    ty: Type,
    kind: OperationKind,
    types: &mut BTreeMap<ValueId, Type>,
    output: &mut Vec<Operation>,
) -> ValueId {
    output.push(Operation::effect_free(
        ValueDef::new(result, ty.clone()),
        kind,
    ));
    types.insert(result, ty);
    result
}

fn emit_constant_u32(
    module: &Module,
    value: u32,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    emit_value(
        module,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(value)),
        types,
        next_value,
        output,
    )
}

fn emit_constant_index(
    module: &Module,
    value: u64,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    emit_value(
        module,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(value)),
        types,
        next_value,
        output,
    )
}

fn emit_scalar_zero(
    module: &Module,
    scalar: ScalarType,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let constant = scalar_zero_constant(scalar)
        .ok_or_else(|| incomplete(module, "V13 128-bit scalar zero is unsupported"))?;
    emit_value(
        module,
        Type::Scalar(scalar),
        OperationKind::Constant(constant),
        types,
        next_value,
        output,
    )
}

const fn scalar_zero_constant(scalar: ScalarType) -> Option<Constant> {
    Some(match scalar {
        ScalarType::Bool => Constant::Bool(false),
        ScalarType::I8 => Constant::I8(0),
        ScalarType::I16 => Constant::I16(0),
        ScalarType::I32 => Constant::I32(0),
        ScalarType::I64 => Constant::I64(0),
        ScalarType::U8 => Constant::U8(0),
        ScalarType::U16 => Constant::U16(0),
        ScalarType::U32 => Constant::U32(0),
        ScalarType::U64 => Constant::U64(0),
        ScalarType::Index => Constant::Index(0),
        ScalarType::F16 => Constant::F16Bits(0),
        ScalarType::Bf16 => Constant::Bf16Bits(0),
        ScalarType::F32 => Constant::F32Bits(0),
        ScalarType::F64 => Constant::F64Bits(0),
        ScalarType::I128 | ScalarType::U128 => return None,
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_in_bounds(
    module: &Module,
    index: ValueId,
    extent: Option<PhysicalExtentV1>,
    destination: Option<ValueId>,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let extent = materialize_extent(module, extent, types, next_value, output)?;
    let kind = OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs: index,
        rhs: extent,
    };
    Ok(match destination {
        Some(result) => emit_value_at(module, result, Type::BOOL, kind, types, output),
        None => emit_value(module, Type::BOOL, kind, types, next_value, output)?,
    })
}

fn materialize_extent(
    module: &Module,
    extent: Option<PhysicalExtentV1>,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    match extent {
        Some(PhysicalExtentV1::Static(elements)) => {
            emit_constant_index(module, elements, types, next_value, output)
        }
        Some(PhysicalExtentV1::Dynamic(value)) if types.get(&value) == Some(&Type::INDEX) => {
            Ok(value)
        }
        Some(PhysicalExtentV1::Dynamic(_)) => {
            Err(incomplete(module, "V13 dynamic extent is not an index"))
        }
        None => Err(incomplete(module, "V13 memory view has no retained extent")),
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_optional_load(
    module: &Module,
    operation: &Operation,
    pointer: ValueId,
    index: ValueId,
    extent: Option<PhysicalExtentV1>,
    space: ExecutionMemoryAddressSpaceV1,
    alignment: u16,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<(), LoweringErrors> {
    let Type::Pointer(pointer_type) = types
        .get(&pointer)
        .ok_or_else(|| incomplete(module, "V13 load pointer type is absent"))?
    else {
        return Err(incomplete(module, "V13 load carrier is not a pointer"));
    };
    let Type::Scalar(scalar) = pointer_type.pointee.as_ref() else {
        return Err(incomplete(
            module,
            "V13 optional load supports scalar elements",
        ));
    };
    let scalar = *scalar;
    let results = ordinary_results(operation);
    let [value_result, present_result] = results.as_slice() else {
        return Err(incomplete(
            module,
            "V13 Option<T> load requires payload and presence results",
        ));
    };
    if value_result.ty != Type::Scalar(scalar) || present_result.ty != Type::BOOL {
        return Err(incomplete(
            module,
            "V13 Option<T> load result ABI must be (T, bool)",
        ));
    }
    let predicate = emit_in_bounds(
        module,
        index,
        extent,
        Some(present_result.id),
        types,
        next_value,
        output,
    )?;
    let zero_index = emit_constant_index(module, 0, types, next_value, output)?;
    let safe_index = emit_select(
        module,
        predicate,
        index,
        zero_index,
        Type::INDEX,
        None,
        types,
        next_value,
        output,
    )?;
    let address = emit_gep(module, pointer, safe_index, types, next_value, output)?;
    let fallback = emit_scalar_zero(module, scalar, types, next_value, output)?;
    emit_value_at(
        module,
        value_result.id,
        value_result.ty.clone(),
        OperationKind::GuardedLoad {
            pointer: address,
            predicate,
            fallback,
            access: memory_access(space, alignment),
        },
        types,
        output,
    );
    reject_capability_results(module, operation)
}

#[allow(clippy::too_many_arguments)]
fn emit_binary(
    module: &Module,
    op: BinaryOp,
    lhs: ValueId,
    rhs: ValueId,
    ty: Type,
    destination: Option<ValueId>,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let kind = OperationKind::Binary { op, lhs, rhs };
    Ok(match destination {
        Some(result) => emit_value_at(module, result, ty, kind, types, output),
        None => emit_value(module, ty, kind, types, next_value, output)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_compare(
    module: &Module,
    predicate: ComparePredicate,
    lhs: ValueId,
    rhs: ValueId,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    emit_value(
        module,
        Type::BOOL,
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        },
        types,
        next_value,
        output,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_select(
    module: &Module,
    condition: ValueId,
    true_value: ValueId,
    false_value: ValueId,
    ty: Type,
    destination: Option<ValueId>,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let kind = OperationKind::Select {
        condition,
        true_value,
        false_value,
    };
    Ok(match destination {
        Some(result) => emit_value_at(module, result, ty, kind, types, output),
        None => emit_value(module, ty, kind, types, next_value, output)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_cast(
    module: &Module,
    kind: CastKind,
    value: ValueId,
    to: Type,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    emit_value(
        module,
        to.clone(),
        OperationKind::Cast { kind, value, to },
        types,
        next_value,
        output,
    )
}

fn emit_wave_lane(
    module: &Module,
    width: WaveWidth,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    emit_value(
        module,
        Type::Scalar(ScalarType::U32),
        OperationKind::Wave(WaveOperation::full(WaveOperationKind::LaneId, width)),
        types,
        next_value,
        output,
    )
}

fn emit_flat_local_index(
    module: &Module,
    size: WorkgroupSize,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let x = emit_local_index(module, Axis::X, types, next_value, output)?;
    if size.y == 1 && size.z == 1 {
        return Ok(x);
    }
    let y = emit_local_index(module, Axis::Y, types, next_value, output)?;
    let size_x = emit_constant_index(module, u64::from(size.x), types, next_value, output)?;
    let y_row = emit_binary(
        module,
        BinaryOp::Multiply,
        y,
        size_x,
        Type::INDEX,
        None,
        types,
        next_value,
        output,
    )?;
    let xy = emit_binary(
        module,
        BinaryOp::Add,
        y_row,
        x,
        Type::INDEX,
        None,
        types,
        next_value,
        output,
    )?;
    if size.z == 1 {
        return Ok(xy);
    }
    let z = emit_local_index(module, Axis::Z, types, next_value, output)?;
    let plane = emit_constant_index(
        module,
        u64::from(size.x) * u64::from(size.y),
        types,
        next_value,
        output,
    )?;
    let z_plane = emit_binary(
        module,
        BinaryOp::Multiply,
        z,
        plane,
        Type::INDEX,
        None,
        types,
        next_value,
        output,
    )?;
    emit_binary(
        module,
        BinaryOp::Add,
        z_plane,
        xy,
        Type::INDEX,
        None,
        types,
        next_value,
        output,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_memory_load(
    module: &Module,
    base: ValueId,
    index: ValueId,
    scalar: ScalarType,
    space: ExecutionMemoryAddressSpaceV1,
    alignment: u16,
    destination: Option<ValueId>,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let pointer = emit_gep(module, base, index, types, next_value, output)?;
    let kind = OperationKind::Load {
        pointer,
        access: memory_access(space, alignment),
    };
    let ty = Type::Scalar(scalar);
    Ok(match destination {
        Some(result) => emit_value_at(module, result, ty, kind, types, output),
        None => emit_value(module, ty, kind, types, next_value, output)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_memory_store(
    module: &Module,
    base: ValueId,
    index: ValueId,
    value: ValueId,
    space: ExecutionMemoryAddressSpaceV1,
    alignment: u16,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<(), LoweringErrors> {
    let pointer = emit_gep(module, base, index, types, next_value, output)?;
    output.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer,
            value,
            access: memory_access(space, alignment),
        },
    ));
    Ok(())
}

fn default_workgroup_barrier() -> Operation {
    workgroup_barrier(ExecutionMemorySemanticsV1 {
        scope: fe2o3_kernel_ir::ExecutionMemoryScopeV1::Workgroup,
        ordering: fe2o3_kernel_ir::ExecutionMemoryOrderingV1::AcquireRelease,
        spaces: fe2o3_kernel_ir::ExecutionMemorySpacesV1::Workgroup,
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_workgroup_reduce_sum(
    module: &Module,
    scratch: ValueId,
    scalar: ScalarType,
    rank: ValueId,
    size: u32,
    alignment: u16,
    result: ValueId,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let mut offset = size >> 1;
    while offset != 0 {
        let offset_value =
            emit_constant_index(module, u64::from(offset), types, next_value, output)?;
        let active = emit_compare(
            module,
            ComparePredicate::LessThan,
            rank,
            offset_value,
            types,
            next_value,
            output,
        )?;
        let pair = emit_binary(
            module,
            BinaryOp::Add,
            rank,
            offset_value,
            Type::INDEX,
            None,
            types,
            next_value,
            output,
        )?;
        let zero = emit_constant_index(module, 0, types, next_value, output)?;
        let safe_pair = emit_select(
            module,
            active,
            pair,
            zero,
            Type::INDEX,
            None,
            types,
            next_value,
            output,
        )?;
        let lhs = emit_memory_load(
            module,
            scratch,
            rank,
            scalar,
            ExecutionMemoryAddressSpaceV1::Workgroup,
            alignment,
            None,
            types,
            next_value,
            output,
        )?;
        let rhs = emit_memory_load(
            module,
            scratch,
            safe_pair,
            scalar,
            ExecutionMemoryAddressSpaceV1::Workgroup,
            alignment,
            None,
            types,
            next_value,
            output,
        )?;
        let sum = emit_binary(
            module,
            BinaryOp::Add,
            lhs,
            rhs,
            Type::Scalar(scalar),
            None,
            types,
            next_value,
            output,
        )?;
        let selected = emit_select(
            module,
            active,
            sum,
            lhs,
            Type::Scalar(scalar),
            None,
            types,
            next_value,
            output,
        )?;
        output.push(default_workgroup_barrier());
        emit_memory_store(
            module,
            scratch,
            rank,
            selected,
            ExecutionMemoryAddressSpaceV1::Workgroup,
            alignment,
            types,
            next_value,
            output,
        )?;
        output.push(default_workgroup_barrier());
        offset >>= 1;
    }
    let zero = emit_constant_index(module, 0, types, next_value, output)?;
    emit_memory_load(
        module,
        scratch,
        zero,
        scalar,
        ExecutionMemoryAddressSpaceV1::Workgroup,
        alignment,
        Some(result),
        types,
        next_value,
        output,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_workgroup_scan_sum(
    module: &Module,
    scratch: ValueId,
    scalar: ScalarType,
    rank: ValueId,
    size: u32,
    alignment: u16,
    kind: ExecutionCollectiveKindV1,
    result: ValueId,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let mut offset = 1;
    while offset < size {
        let offset_value =
            emit_constant_index(module, u64::from(offset), types, next_value, output)?;
        let active = emit_compare(
            module,
            ComparePredicate::GreaterThanOrEqual,
            rank,
            offset_value,
            types,
            next_value,
            output,
        )?;
        let safe_rank = emit_select(
            module,
            active,
            rank,
            offset_value,
            Type::INDEX,
            None,
            types,
            next_value,
            output,
        )?;
        let source = emit_binary(
            module,
            BinaryOp::Subtract,
            safe_rank,
            offset_value,
            Type::INDEX,
            None,
            types,
            next_value,
            output,
        )?;
        let current = emit_memory_load(
            module,
            scratch,
            rank,
            scalar,
            ExecutionMemoryAddressSpaceV1::Workgroup,
            alignment,
            None,
            types,
            next_value,
            output,
        )?;
        let prefix = emit_memory_load(
            module,
            scratch,
            source,
            scalar,
            ExecutionMemoryAddressSpaceV1::Workgroup,
            alignment,
            None,
            types,
            next_value,
            output,
        )?;
        let sum = emit_binary(
            module,
            BinaryOp::Add,
            prefix,
            current,
            Type::Scalar(scalar),
            None,
            types,
            next_value,
            output,
        )?;
        let selected = emit_select(
            module,
            active,
            sum,
            current,
            Type::Scalar(scalar),
            None,
            types,
            next_value,
            output,
        )?;
        output.push(default_workgroup_barrier());
        emit_memory_store(
            module,
            scratch,
            rank,
            selected,
            ExecutionMemoryAddressSpaceV1::Workgroup,
            alignment,
            types,
            next_value,
            output,
        )?;
        output.push(default_workgroup_barrier());
        offset <<= 1;
    }

    match kind {
        ExecutionCollectiveKindV1::InclusiveScanSum => emit_memory_load(
            module,
            scratch,
            rank,
            scalar,
            ExecutionMemoryAddressSpaceV1::Workgroup,
            alignment,
            Some(result),
            types,
            next_value,
            output,
        ),
        ExecutionCollectiveKindV1::ExclusiveScanSum => {
            let one = emit_constant_index(module, 1, types, next_value, output)?;
            let active = emit_compare(
                module,
                ComparePredicate::GreaterThanOrEqual,
                rank,
                one,
                types,
                next_value,
                output,
            )?;
            let safe_rank = emit_select(
                module,
                active,
                rank,
                one,
                Type::INDEX,
                None,
                types,
                next_value,
                output,
            )?;
            let source = emit_binary(
                module,
                BinaryOp::Subtract,
                safe_rank,
                one,
                Type::INDEX,
                None,
                types,
                next_value,
                output,
            )?;
            let prefix = emit_memory_load(
                module,
                scratch,
                source,
                scalar,
                ExecutionMemoryAddressSpaceV1::Workgroup,
                alignment,
                None,
                types,
                next_value,
                output,
            )?;
            let zero = emit_scalar_zero(module, scalar, types, next_value, output)?;
            emit_select(
                module,
                active,
                prefix,
                zero,
                Type::Scalar(scalar),
                Some(result),
                types,
                next_value,
                output,
            )
        }
        ExecutionCollectiveKindV1::ReduceSum => unreachable!("reduction uses its own recipe"),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionAliasV1 {
    Physical(PhysicalValueV1),
    Erased,
}

pub(super) fn lower_execution_capabilities_v1(
    module: &Module,
    profile: ProductionAmdTargetProfileV1,
    authority: &V13LoweringAuthorityV1,
) -> Result<Module, LoweringErrors> {
    let mut lowered = module.clone();
    strip_exact_execution_requirements(&mut lowered);
    bind_exact_amd_target(&mut lowered, profile);
    let original_function_count = lowered.functions.len();
    let mut generated_helpers = Vec::new();

    for function_index in 0..original_function_count {
        let function = &mut lowered.functions[function_index];
        if function.body.is_none() {
            continue;
        }
        reject_execution_capability_parameters(module, function)?;
        let types = value_types(function);
        let element_types = infer_element_types(module, function, &types)?;
        let mut aliases = BTreeMap::new();
        let mut lowered_types = types.clone();
        let mut next_value = next_value_id(module, function)?;
        let mut wave_width = None;
        let body = function.body.as_mut().expect("definition checked above");

        for block in &mut body.blocks {
            let mut operations = Vec::with_capacity(block.operations.len());
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                    reject_unlowered_execution_operand(module, operation, &aliases)?;
                    operations.push(operation.clone());
                    continue;
                };
                require_operation_closure(module, authority, contract)?;
                lower_operation(
                    module,
                    operation,
                    contract,
                    &types,
                    &element_types,
                    &mut lowered_types,
                    &mut aliases,
                    &mut next_value,
                    &mut wave_width,
                    &mut operations,
                    &mut generated_helpers,
                    function_index,
                    block.id,
                    operation_index,
                )?;
            }
            block.operations = operations;
        }

        if let Some(width) = wave_width {
            bind_wave(&mut function.required_capabilities, width);
        }
        function.required_capabilities.extend(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .flat_map(Operation::required_capabilities),
        );
    }
    lowered.functions.extend(generated_helpers);

    let physical_capabilities = lowered
        .functions
        .iter()
        .flat_map(|function| &function.required_capabilities)
        .cloned()
        .collect::<BTreeSet<_>>();
    lowered
        .required_capabilities
        .extend(physical_capabilities.iter().cloned());
    for kernel in &mut lowered.kernels {
        if let Some(entry) = lowered
            .functions
            .iter()
            .find(|function| function.id == kernel.entry)
        {
            kernel
                .required_capabilities
                .extend(entry.required_capabilities.iter().cloned());
        }
    }

    let wave_functions = lowered
        .functions
        .iter()
        .filter_map(|function| {
            function
                .required_capabilities
                .iter()
                .find_map(|capability| match capability {
                    TargetCapability::WaveWidth(width) => Some((function.id.clone(), *width)),
                    _ => None,
                })
        })
        .collect::<BTreeMap<_, _>>();
    if !wave_functions.is_empty() {
        let widths = wave_functions.values().copied().collect::<BTreeSet<_>>();
        if widths.len() != 1 {
            return Err(incomplete(
                module,
                "one AMD LLVM module cannot bind both Wave32 and Wave64 execution contracts",
            ));
        }
        bind_wave(
            &mut lowered.required_capabilities,
            *widths.first().expect("nonempty wave-width set"),
        );
        for kernel in &mut lowered.kernels {
            if let Some(width) = wave_functions.get(&kernel.entry) {
                bind_wave(&mut kernel.required_capabilities, *width);
            }
        }
    }

    debug_assert_ne!(authority.identity(), [0; 32]);
    Ok(lowered)
}

fn strip_exact_execution_requirements(module: &mut Module) {
    let retain =
        |capability: &TargetCapability| !matches!(capability, TargetCapability::Execution(_));
    module.required_capabilities.retain(retain);
    for kernel in &mut module.kernels {
        kernel.required_capabilities.retain(retain);
    }
    for function in &mut module.functions {
        function.required_capabilities.retain(retain);
    }
}

fn bind_exact_amd_target(module: &mut Module, profile: ProductionAmdTargetProfileV1) {
    let name = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        ProductionAmdTargetProfileV1::Gfx950 => AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    };
    let capability = TargetCapability::Extension {
        namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
        name: name.to_owned(),
    };
    module.required_capabilities.insert(capability.clone());
    for kernel in &mut module.kernels {
        kernel.required_capabilities.insert(capability.clone());
        if let Some(entry) = module
            .functions
            .iter_mut()
            .find(|function| function.id == kernel.entry)
        {
            entry.required_capabilities.insert(capability.clone());
        }
    }
}

fn bind_wave(capabilities: &mut BTreeSet<TargetCapability>, width: WaveWidth) {
    capabilities.insert(TargetCapability::Subgroups);
    capabilities.insert(TargetCapability::SubgroupSize(width.lanes()));
    capabilities.insert(TargetCapability::WaveWidth(width));
}

fn require_operation_closure(
    module: &Module,
    authority: &V13LoweringAuthorityV1,
    contract: &ExecutionCapabilityOpV1,
) -> Result<(), LoweringErrors> {
    let location = LoweringLocation::module(module);
    let requirements = target_requirements_for_execution_operation_v1(&contract.operation)
        .map_err(|error| {
            incomplete(
                module,
                format!("could not derive V13 lowering requirements: {error}"),
            )
        })?;
    for requirement in requirements {
        authority.require(requirement, &location)?;
    }
    Ok(())
}

fn reject_execution_capability_parameters(
    module: &Module,
    function: &Function,
) -> Result<(), LoweringErrors> {
    let body = function
        .body
        .as_ref()
        .expect("caller selected a definition");
    if function
        .signature
        .parameters
        .iter()
        .chain(
            body.blocks
                .iter()
                .flat_map(|block| block.parameters.iter().map(|value| &value.ty)),
        )
        .any(|ty| matches!(ty, Type::ExecutionCapability(_)))
    {
        return Err(incomplete(
            module,
            "V13 execution capabilities crossing function or block boundaries lack a physical ABI/phi lowering",
        ));
    }
    Ok(())
}

fn reject_unlowered_execution_operand(
    module: &Module,
    operation: &Operation,
    aliases: &BTreeMap<ValueId, ExecutionAliasV1>,
) -> Result<(), LoweringErrors> {
    if operation
        .kind
        .operands()
        .into_iter()
        .any(|operand| match aliases.get(&operand) {
            Some(ExecutionAliasV1::Erased) => true,
            Some(ExecutionAliasV1::Physical(physical)) => physical.value != operand,
            None => false,
        })
    {
        return Err(incomplete(
            module,
            "a non-V13 operation consumes a logical execution token without an explicit physical adapter",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_operation(
    module: &Module,
    operation: &Operation,
    contract: &ExecutionCapabilityOpV1,
    original_types: &BTreeMap<ValueId, Type>,
    element_types: &BTreeMap<ExecutionTypeIdentityV1, Type>,
    lowered_types: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
    next_value: &mut u32,
    wave_width: &mut Option<WaveWidth>,
    output: &mut Vec<Operation>,
    generated_helpers: &mut Vec<Function>,
    function_index: usize,
    block_id: fe2o3_kernel_ir::BlockId,
    operation_index: usize,
) -> Result<(), LoweringErrors> {
    use ExecutionCapabilityOperationV1 as Op;

    match &contract.operation {
        Op::WorkgroupDerive { .. } => erase_capability_results(module, operation, aliases),
        Op::SubgroupDerive { width, .. } => {
            require_wave(module, *width, wave_width)?;
            erase_capability_results(module, operation, aliases)
        }
        Op::MatrixAccess { width, .. } => {
            require_wave(module, *width, wave_width)?;
            erase_capability_results(module, operation, aliases)
        }
        Op::LdsAllocate {
            element,
            layout,
            elements,
            ..
        }
        | Op::WorkgroupMemoryAllocate {
            element,
            layout,
            elements,
            ..
        } => {
            let result = sole_capability_result(module, operation)?;
            let element = element_types.get(element).cloned().ok_or_else(|| {
                incomplete(
                    module,
                    "V13 LDS element identity has no structural physical KIR type witness",
                )
            })?;
            let logical_elements = *elements;
            let physical_elements = u32::try_from(logical_elements).map_err(|_| {
                incomplete(
                    module,
                    "V13 LDS element count exceeds the legacy lowering width",
                )
            })?;
            let pointer = Type::pointer(
                element.clone(),
                fe2o3_kernel_ir::AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            );
            output.push(Operation::effect_free(
                ValueDef::new(result.id, pointer.clone()),
                OperationKind::WorkgroupMemory(WorkgroupMemory {
                    element,
                    extent: WorkgroupMemoryExtent::Static(physical_elements),
                    alignment: u32::from(layout.byte_alignment),
                }),
            ));
            lowered_types.insert(result.id, pointer);
            aliases.insert(
                result.id,
                ExecutionAliasV1::Physical(PhysicalValueV1::view(
                    result.id,
                    PhysicalExtentV1::Static(logical_elements),
                )),
            );
            Ok(())
        }
        Op::LdsInitializeByInvocation {
            layout, elements, ..
        } => {
            let physical = physical_operands(contract, original_types, lowered_types, aliases);
            let pointer = require_pointer(module, &physical)?;
            let value = require_scalar_other_than(module, &physical, pointer)?;
            let index = fresh_value(next_value)?;
            output.push(Operation::effect_free(
                ValueDef::new(index, Type::INDEX),
                OperationKind::Intrinsic(IntrinsicOperation::new(
                    IntrinsicKind::InvocationIndex {
                        kind: fe2o3_kernel_ir::IndexKind::Local,
                        axis: fe2o3_kernel_ir::Axis::X,
                    },
                    Type::INDEX,
                )),
            ));
            lowered_types.insert(index, Type::INDEX);
            let address = emit_gep(module, pointer, index, lowered_types, next_value, output)?;
            output.push(Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: address,
                    value,
                    access: memory_access(
                        ExecutionMemoryAddressSpaceV1::Workgroup,
                        layout.byte_alignment,
                    ),
                },
            ));
            alias_capability_results(
                module,
                operation,
                PhysicalValueV1::view(pointer, PhysicalExtentV1::Static(*elements)),
                aliases,
            )
        }
        Op::LdsPublish { .. } => {
            let physical = physical_operands(contract, original_types, lowered_types, aliases);
            let pointer = require_pointer(module, &physical)?;
            output.push(workgroup_barrier(ExecutionMemorySemanticsV1 {
                scope: fe2o3_kernel_ir::ExecutionMemoryScopeV1::Workgroup,
                ordering: fe2o3_kernel_ir::ExecutionMemoryOrderingV1::AcquireRelease,
                spaces: fe2o3_kernel_ir::ExecutionMemorySpacesV1::Workgroup,
            }));
            alias_capability_results(module, operation, physical_value(aliases, pointer), aliases)
        }
        Op::LdsReadPublished { layout, .. } => {
            let physical = physical_operands(contract, original_types, lowered_types, aliases);
            let pointer = require_pointer(module, &physical)?;
            let index = require_index(module, &physical)?;
            lower_optional_load(
                module,
                operation,
                pointer,
                index,
                physical_value(aliases, pointer).extent,
                ExecutionMemoryAddressSpaceV1::Workgroup,
                layout.byte_alignment,
                lowered_types,
                next_value,
                output,
            )
        }
        Op::WorkgroupBarrier { semantics, .. } => {
            output.push(workgroup_barrier(*semantics));
            erase_capability_results(module, operation, aliases)
        }
        Op::SubgroupBarrier {
            semantics, width, ..
        } => {
            require_wave(module, *width, wave_width)?;
            // Full-wave execution is lockstep on the admitted AMD profiles. The
            // wavefront fence carries the exact memory scope/order; no separate
            // execution instruction is required for a uniform subgroup barrier.
            output.push(fence(*semantics));
            erase_capability_results(module, operation, aliases)
        }
        Op::WorkgroupFence { semantics, .. } => {
            output.push(fence(*semantics));
            erase_capability_results(module, operation, aliases)
        }
        Op::SubgroupFence {
            semantics, width, ..
        } => {
            require_wave(module, *width, wave_width)?;
            output.push(fence(*semantics));
            erase_capability_results(module, operation, aliases)
        }
        Op::Atomic {
            kind,
            value_type,
            address_space,
            scope,
            success,
            failure,
            ..
        } => lower_atomic(
            module,
            operation,
            contract,
            *kind,
            *value_type,
            *address_space,
            *scope,
            *success,
            *failure,
            original_types,
            lowered_types,
            aliases,
            next_value,
            output,
        ),
        Op::SubgroupCollective {
            kind,
            value_type,
            width,
            ..
        } => lower_subgroup_collective(
            module,
            operation,
            contract,
            *kind,
            *value_type,
            require_wave(module, *width, wave_width)?,
            original_types,
            lowered_types,
            aliases,
            next_value,
            output,
        ),
        Op::RawMemoryBind { space, .. } => {
            let physical = physical_operands(contract, original_types, lowered_types, aliases);
            let pointer =
                materialize_pointer(module, &physical, *space, lowered_types, next_value, output)?;
            let extent = raw_memory_extent(contract).ok_or_else(|| {
                incomplete(module, "V13 raw-memory view has no dynamic extent carrier")
            })?;
            alias_capability_results(
                module,
                operation,
                PhysicalValueV1::view(pointer.value, extent),
                aliases,
            )
        }
        Op::WorkgroupMemoryIndex { .. } => {
            let result = sole_capability_result(module, operation)?;
            output.push(Operation::effect_free(
                ValueDef::new(result.id, Type::INDEX),
                OperationKind::Intrinsic(IntrinsicOperation::new(
                    IntrinsicKind::InvocationIndex {
                        kind: fe2o3_kernel_ir::IndexKind::Local,
                        axis: fe2o3_kernel_ir::Axis::X,
                    },
                    Type::INDEX,
                )),
            ));
            lowered_types.insert(result.id, Type::INDEX);
            aliases.insert(
                result.id,
                ExecutionAliasV1::Physical(PhysicalValueV1::scalar(result.id)),
            );
            Ok(())
        }
        Op::WorkgroupMemoryPublish { .. } => {
            let physical = physical_operands(contract, original_types, lowered_types, aliases);
            let pointer = require_pointer(module, &physical)?;
            output.push(workgroup_barrier(ExecutionMemorySemanticsV1 {
                scope: fe2o3_kernel_ir::ExecutionMemoryScopeV1::Workgroup,
                ordering: fe2o3_kernel_ir::ExecutionMemoryOrderingV1::AcquireRelease,
                spaces: fe2o3_kernel_ir::ExecutionMemorySpacesV1::Workgroup,
            }));
            alias_capability_results(module, operation, physical_value(aliases, pointer), aliases)
        }
        Op::MemoryLoad { layout, space, .. } => {
            let physical = physical_operands(contract, original_types, lowered_types, aliases);
            let pointer = require_pointer(module, &physical)?;
            let index = require_index(module, &physical)?;
            lower_optional_load(
                module,
                operation,
                pointer,
                index,
                physical_value(aliases, pointer).extent,
                *space,
                layout.byte_alignment,
                lowered_types,
                next_value,
                output,
            )
        }
        Op::MemoryStore { layout, space, .. } => {
            let physical = physical_operands(contract, original_types, lowered_types, aliases);
            let pointer = require_pointer(module, &physical)?;
            let index = require_index(module, &physical)?;
            let value = physical
                .iter()
                .rev()
                .find_map(|(value, ty)| {
                    (ty.as_scalar().is_some() && *value != index).then_some(*value)
                })
                .ok_or_else(|| incomplete(module, "V13 memory store has no scalar value"))?;
            let result = sole_ordinary_bool_result(module, operation)?;
            let predicate = emit_in_bounds(
                module,
                index,
                physical_value(aliases, pointer).extent,
                Some(result.id),
                lowered_types,
                next_value,
                output,
            )?;
            let zero = emit_constant_index(module, 0, lowered_types, next_value, output)?;
            let safe_index = emit_select(
                module,
                predicate,
                index,
                zero,
                Type::INDEX,
                None,
                lowered_types,
                next_value,
                output,
            )?;
            let address = emit_gep(
                module,
                pointer,
                safe_index,
                lowered_types,
                next_value,
                output,
            )?;
            output.push(Operation::new(
                vec![],
                OperationKind::GuardedStore {
                    pointer: address,
                    predicate,
                    value,
                    access: memory_access(*space, layout.byte_alignment),
                },
            ));
            reject_capability_results(module, operation)
        }
        Op::WorkgroupCollective {
            kind,
            value_type,
            layout,
            elements,
            ..
        } => lower_workgroup_collective(
            module,
            &module.functions[function_index].id,
            operation,
            contract,
            *kind,
            *value_type,
            *layout,
            *elements,
            original_types,
            lowered_types,
            aliases,
            next_value,
            output,
        ),
        Op::AsyncCopy {
            layout, elements, ..
        } => lower_async_copy(
            module,
            operation,
            contract,
            *layout,
            *elements,
            original_types,
            lowered_types,
            aliases,
            next_value,
            output,
            generated_helpers,
            function_index,
            block_id,
            operation_index,
        ),
        Op::AsyncWait { .. } => {
            let physical = physical_operands(contract, original_types, lowered_types, aliases);
            let pending = require_pointer(module, &physical)?;
            output.push(workgroup_barrier(ExecutionMemorySemanticsV1 {
                scope: fe2o3_kernel_ir::ExecutionMemoryScopeV1::Workgroup,
                ordering: fe2o3_kernel_ir::ExecutionMemoryOrderingV1::AcquireRelease,
                spaces: fe2o3_kernel_ir::ExecutionMemorySpacesV1::Workgroup,
            }));
            alias_capability_results(module, operation, physical_value(aliases, pending), aliases)
        }
        Op::PrivateMemoryAllocate {
            element,
            layout,
            elements,
            ..
        } => lower_private_allocation(
            module,
            operation,
            *element,
            *layout,
            *elements,
            element_types,
            lowered_types,
            aliases,
            next_value,
            output,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_async_copy(
    module: &Module,
    operation: &Operation,
    contract: &ExecutionCapabilityOpV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
    original_types: &BTreeMap<ValueId, Type>,
    lowered_types: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
    generated_helpers: &mut Vec<Function>,
    function_index: usize,
    block_id: fe2o3_kernel_ir::BlockId,
    operation_index: usize,
) -> Result<(), LoweringErrors> {
    let physical = physical_operands(contract, original_types, lowered_types, aliases);
    let source =
        require_pointer_in_space(module, &physical, ExecutionMemoryAddressSpaceV1::Global)?;
    let destination =
        require_pointer_in_space(module, &physical, ExecutionMemoryAddressSpaceV1::Workgroup)?;
    let source_offset = require_index(module, &physical)?;
    let source_extent = physical_value(aliases, source).extent;
    let source_type = lowered_types
        .get(&source)
        .cloned()
        .ok_or_else(|| incomplete(module, "V13 async source pointer type is missing"))?;
    let destination_type = lowered_types
        .get(&destination)
        .cloned()
        .ok_or_else(|| incomplete(module, "V13 async destination pointer type is missing"))?;
    let (Type::Pointer(source_pointer), Type::Pointer(destination_pointer)) =
        (&source_type, &destination_type)
    else {
        return Err(incomplete(
            module,
            "V13 async copy requires physical pointers",
        ));
    };
    if source_pointer.pointee != destination_pointer.pointee {
        return Err(incomplete(
            module,
            "V13 async copy source and destination element types differ",
        ));
    }
    validate_physical_layout(module, &source_pointer.pointee, layout)?;
    let element_type = (*source_pointer.pointee).clone();
    let elements = u32::try_from(elements)
        .ok()
        .filter(|elements| *elements != 0)
        .ok_or_else(|| incomplete(module, "V13 async copy extent exceeds KIR index lowering"))?;
    let function = &module.functions[function_index].id;
    let size = static_workgroup_size(module, function)?;
    let flat_size = size
        .x
        .checked_mul(size.y)
        .and_then(|xy| xy.checked_mul(size.z))
        .filter(|size| *size != 0)
        .ok_or_else(|| incomplete(module, "V13 async copy workgroup size overflows"))?;
    let launch_rank = module
        .kernels
        .iter()
        .find(|kernel| &kernel.entry == function)
        .map(|kernel| kernel.domain.rank())
        .ok_or_else(|| incomplete(module, "V13 async copy has no kernel launch domain"))?;
    let local_x = emit_local_index(module, Axis::X, lowered_types, next_value, output)?;
    let rank = match launch_rank {
        1 => local_x,
        2 | 3 => {
            let local_y = emit_local_index(module, Axis::Y, lowered_types, next_value, output)?;
            let size_x =
                emit_constant_index(module, u64::from(size.x), lowered_types, next_value, output)?;
            let row = if launch_rank == 2 {
                local_y
            } else {
                let local_z = emit_local_index(module, Axis::Z, lowered_types, next_value, output)?;
                let size_y = emit_constant_index(
                    module,
                    u64::from(size.y),
                    lowered_types,
                    next_value,
                    output,
                )?;
                let row = emit_binary(
                    module,
                    BinaryOp::Multiply,
                    local_z,
                    size_y,
                    Type::INDEX,
                    None,
                    lowered_types,
                    next_value,
                    output,
                )?;
                emit_binary(
                    module,
                    BinaryOp::Add,
                    row,
                    local_y,
                    Type::INDEX,
                    None,
                    lowered_types,
                    next_value,
                    output,
                )?
            };
            let row = emit_binary(
                module,
                BinaryOp::Multiply,
                row,
                size_x,
                Type::INDEX,
                None,
                lowered_types,
                next_value,
                output,
            )?;
            emit_binary(
                module,
                BinaryOp::Add,
                row,
                local_x,
                Type::INDEX,
                None,
                lowered_types,
                next_value,
                output,
            )?
        }
        _ => return Err(incomplete(module, "V13 async copy launch rank is invalid")),
    };
    let stride = emit_constant_index(
        module,
        u64::from(flat_size),
        lowered_types,
        next_value,
        output,
    )?;
    let source_length =
        materialize_extent(module, source_extent, lowered_types, next_value, output)?;

    if elements <= flat_size {
        let copy_extent = emit_constant_index(
            module,
            u64::from(elements),
            lowered_types,
            next_value,
            output,
        )?;
        let destination_in_bounds = emit_compare(
            module,
            ComparePredicate::LessThan,
            rank,
            copy_extent,
            lowered_types,
            next_value,
            output,
        )?;
        let source_index = emit_binary(
            module,
            BinaryOp::Add,
            source_offset,
            rank,
            Type::INDEX,
            None,
            lowered_types,
            next_value,
            output,
        )?;
        let source_in_bounds = emit_compare(
            module,
            ComparePredicate::LessThan,
            source_index,
            source_length,
            lowered_types,
            next_value,
            output,
        )?;
        let false_value =
            emit_scalar_zero(module, ScalarType::Bool, lowered_types, next_value, output)?;
        let copy = emit_select(
            module,
            destination_in_bounds,
            source_in_bounds,
            false_value,
            Type::BOOL,
            None,
            lowered_types,
            next_value,
            output,
        )?;
        let zero_index = emit_constant_index(module, 0, lowered_types, next_value, output)?;
        let safe_source_index = emit_select(
            module,
            copy,
            source_index,
            zero_index,
            Type::INDEX,
            None,
            lowered_types,
            next_value,
            output,
        )?;
        let source_address = emit_gep(
            module,
            source,
            safe_source_index,
            lowered_types,
            next_value,
            output,
        )?;
        let scalar = element_type
            .as_scalar()
            .ok_or_else(|| incomplete(module, "V13 async copy requires scalar elements"))?;
        let fallback = emit_scalar_zero(module, scalar, lowered_types, next_value, output)?;
        let value = emit_value(
            module,
            element_type,
            OperationKind::GuardedLoad {
                pointer: source_address,
                predicate: copy,
                fallback,
                access: memory_access(ExecutionMemoryAddressSpaceV1::Global, layout.byte_alignment),
            },
            lowered_types,
            next_value,
            output,
        )?;
        let safe_destination_index = emit_select(
            module,
            destination_in_bounds,
            rank,
            zero_index,
            Type::INDEX,
            None,
            lowered_types,
            next_value,
            output,
        )?;
        let destination_address = emit_gep(
            module,
            destination,
            safe_destination_index,
            lowered_types,
            next_value,
            output,
        )?;
        output.push(Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: destination_address,
                predicate: destination_in_bounds,
                value,
                access: memory_access(
                    ExecutionMemoryAddressSpaceV1::Workgroup,
                    layout.byte_alignment,
                ),
            },
        ));
        return alias_capability_results(
            module,
            operation,
            PhysicalValueV1::view(destination, PhysicalExtentV1::Static(u64::from(elements))),
            aliases,
        );
    }

    let helper_id = FunctionId::new(format!(
        "__fe2o3_v13_async_copy_{function_index}_{}_{}",
        block_id.0, operation_index
    ));
    if module
        .functions
        .iter()
        .chain(generated_helpers.iter())
        .any(|candidate| candidate.id == helper_id)
    {
        return Err(incomplete(module, "V13 async helper identity collision"));
    }
    generated_helpers.push(async_copy_helper(
        helper_id.clone(),
        source_type,
        destination_type,
        element_type,
        elements,
        layout,
    ));
    output.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: helper_id,
            arguments: vec![
                source,
                destination,
                source_offset,
                rank,
                stride,
                source_length,
            ],
        },
    ));
    alias_capability_results(
        module,
        operation,
        PhysicalValueV1::view(destination, PhysicalExtentV1::Static(u64::from(elements))),
        aliases,
    )
}

fn async_copy_helper(
    id: FunctionId,
    source_pointer: Type,
    destination_pointer: Type,
    element: Type,
    elements: u32,
    layout: ExecutionElementLayoutV1,
) -> Function {
    let parameters = vec![
        ValueId(0),
        ValueId(1),
        ValueId(2),
        ValueId(3),
        ValueId(4),
        ValueId(5),
    ];
    let mut entry = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::Constant(Constant::Index(u64::from(elements))),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(3),
                rhs: ValueId(6),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: fe2o3_kernel_ir::BlockId(1),
        then_arguments: vec![ValueId(3)],
        else_target: fe2o3_kernel_ir::BlockId(2),
        else_arguments: vec![],
    });

    let mut copy = BasicBlock::new(fe2o3_kernel_ir::BlockId(1));
    copy.parameters = vec![ValueDef::new(ValueId(8), Type::INDEX)];
    let zero = scalar_zero_constant(element.as_scalar().expect("validated async scalar"))
        .expect("128-bit async elements are not admitted");
    copy.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(8),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(9),
                rhs: ValueId(5),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), Type::INDEX),
            OperationKind::Select {
                condition: ValueId(10),
                true_value: ValueId(9),
                false_value: ValueId(11),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(13), source_pointer.clone()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(12),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(14), element.clone()),
            OperationKind::Constant(zero),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(15), element),
            OperationKind::GuardedLoad {
                pointer: ValueId(13),
                predicate: ValueId(10),
                fallback: ValueId(14),
                access: memory_access(ExecutionMemoryAddressSpaceV1::Global, layout.byte_alignment),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(16), destination_pointer.clone()),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(8),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(16),
                value: ValueId(15),
                access: memory_access(
                    ExecutionMemoryAddressSpaceV1::Workgroup,
                    layout.byte_alignment,
                ),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(17), Type::INDEX),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(8),
                rhs: ValueId(4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(18), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(17),
                rhs: ValueId(6),
            },
        ),
    ];
    copy.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(18),
        then_target: fe2o3_kernel_ir::BlockId(1),
        then_arguments: vec![ValueId(17)],
        else_target: fe2o3_kernel_ir::BlockId(2),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(fe2o3_kernel_ir::BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    Function::internal_helper(
        id,
        Signature::new(
            vec![
                source_pointer,
                destination_pointer,
                Type::INDEX,
                Type::INDEX,
                Type::INDEX,
                Type::INDEX,
            ],
            vec![],
        ),
        parameters,
        vec![entry, copy, exit],
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_private_allocation(
    module: &Module,
    operation: &Operation,
    element_identity: ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
    element_types: &BTreeMap<ExecutionTypeIdentityV1, Type>,
    lowered_types: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<(), LoweringErrors> {
    let logical_result = sole_capability_result(module, operation)?;
    let element = element_types
        .get(&element_identity)
        .cloned()
        .ok_or_else(|| {
            incomplete(
                module,
                "V13 private element identity has no structural physical KIR type witness",
            )
        })?;
    validate_physical_layout(module, &element, layout)?;
    let count = match elements {
        0 => return Err(incomplete(module, "V13 private allocation cannot be empty")),
        1 => None,
        elements => Some(emit_constant_index(
            module,
            elements,
            lowered_types,
            next_value,
            output,
        )?),
    };
    let pointer = Type::pointer(
        element.clone(),
        fe2o3_kernel_ir::AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    output.push(Operation::effect_free(
        ValueDef::new(logical_result.id, pointer.clone()),
        OperationKind::Alloca {
            element,
            count,
            address_space: fe2o3_kernel_ir::AddressSpace::Private,
            alignment: u32::from(layout.byte_alignment),
        },
    ));
    lowered_types.insert(logical_result.id, pointer);
    aliases.insert(
        logical_result.id,
        ExecutionAliasV1::Physical(PhysicalValueV1::view(
            logical_result.id,
            PhysicalExtentV1::Static(elements),
        )),
    );
    Ok(())
}

fn validate_physical_layout(
    module: &Module,
    element: &Type,
    layout: ExecutionElementLayoutV1,
) -> Result<(), LoweringErrors> {
    let byte_size = match element {
        Type::Scalar(scalar) => scalar.bit_width().map(|bits| u32::from(bits).div_ceil(8)),
        Type::Pointer(_) => Some(8),
        _ => None,
    };
    if byte_size != Some(layout.byte_size) {
        return Err(incomplete(
            module,
            format!(
                "V13 layout size {} does not match physical element {element:?}",
                layout.byte_size
            ),
        ));
    }
    if u32::from(layout.byte_alignment) < layout.byte_size.next_power_of_two() {
        return Err(incomplete(
            module,
            "V13 element alignment is below the physical natural alignment",
        ));
    }
    Ok(())
}

fn static_workgroup_size(
    module: &Module,
    function: &FunctionId,
) -> Result<WorkgroupSize, LoweringErrors> {
    module
        .kernels
        .iter()
        .find(|kernel| &kernel.entry == function)
        .and_then(|kernel| kernel.workgroup_size)
        .ok_or_else(|| {
            incomplete(
                module,
                "V13 cooperative operation requires static workgroup size",
            )
        })
}

fn emit_local_index(
    module: &Module,
    axis: Axis,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    emit_value(
        module,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis,
            },
            Type::INDEX,
        )),
        types,
        next_value,
        output,
    )
}

fn require_pointer_in_space(
    module: &Module,
    physical: &[(ValueId, Type)],
    space: ExecutionMemoryAddressSpaceV1,
) -> Result<ValueId, LoweringErrors> {
    physical
        .iter()
        .find_map(|(value, ty)| {
            matches!(ty, Type::Pointer(pointer) if pointer.address_space == space.address_space())
                .then_some(*value)
        })
        .ok_or_else(|| incomplete(module, format!("V13 operation has no {space:?} pointer")))
}

#[allow(clippy::too_many_arguments)]
fn lower_atomic(
    module: &Module,
    operation: &Operation,
    contract: &ExecutionCapabilityOpV1,
    kind: ExecutionAtomicKindV1,
    value_type: ScalarType,
    address_space: ExecutionMemoryAddressSpaceV1,
    scope: fe2o3_kernel_ir::ExecutionMemoryScopeV1,
    success: Option<fe2o3_kernel_ir::ExecutionMemoryOrderingV1>,
    failure: Option<fe2o3_kernel_ir::ExecutionMemoryOrderingV1>,
    original_types: &BTreeMap<ValueId, Type>,
    lowered_types: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<(), LoweringErrors> {
    let physical = physical_operands(contract, original_types, lowered_types, aliases);
    if kind == ExecutionAtomicKindV1::BindGlobalView {
        let pointer = materialize_pointer(
            module,
            &physical,
            address_space,
            lowered_types,
            next_value,
            output,
        )?;
        return alias_capability_results(module, operation, pointer, aliases);
    }
    if kind == ExecutionAtomicKindV1::BindGlobalLocation {
        let pointer = require_pointer(module, &physical)?;
        let index = require_index(module, &physical)?;
        let extent = physical_value(aliases, pointer).extent;
        let result = sole_ordinary_bool_result(module, operation)?;
        let predicate = emit_in_bounds(
            module,
            index,
            extent,
            Some(result.id),
            lowered_types,
            next_value,
            output,
        )?;
        let zero = emit_constant_index(module, 0, lowered_types, next_value, output)?;
        let safe_index = emit_select(
            module,
            predicate,
            index,
            zero,
            Type::INDEX,
            None,
            lowered_types,
            next_value,
            output,
        )?;
        let location = emit_gep(
            module,
            pointer,
            safe_index,
            lowered_types,
            next_value,
            output,
        )?;
        return alias_capability_results(
            module,
            operation,
            PhysicalValueV1 {
                value: location,
                extent,
            },
            aliases,
        );
    }

    let pointer = require_pointer(module, &physical)?;
    let values = physical
        .iter()
        .filter_map(|(value, ty)| {
            (*value != pointer && ty == &Type::Scalar(value_type)).then_some(*value)
        })
        .collect::<Vec<_>>();
    let (atomic_kind, value, compare) = match kind {
        ExecutionAtomicKindV1::Load => (fe2o3_kernel_ir::AtomicKind::Load, None, None),
        ExecutionAtomicKindV1::Store => (
            fe2o3_kernel_ir::AtomicKind::Store,
            values.first().copied(),
            None,
        ),
        ExecutionAtomicKindV1::FetchAdd => (
            fe2o3_kernel_ir::AtomicKind::Add,
            values.first().copied(),
            None,
        ),
        ExecutionAtomicKindV1::CompareExchange => (
            fe2o3_kernel_ir::AtomicKind::CompareExchange,
            values.get(1).copied(),
            values.first().copied(),
        ),
        ExecutionAtomicKindV1::BindGlobalLocation | ExecutionAtomicKindV1::BindGlobalView => {
            unreachable!("binding atomics returned above")
        }
    };
    let expected_values = match kind {
        ExecutionAtomicKindV1::Load => 0,
        ExecutionAtomicKindV1::Store | ExecutionAtomicKindV1::FetchAdd => 1,
        ExecutionAtomicKindV1::CompareExchange => 2,
        _ => unreachable!(),
    };
    if values.len() != expected_values {
        return Err(incomplete(
            module,
            format!(
                "V13 {kind:?} atomic has {} physical values, expected {expected_values}",
                values.len()
            ),
        ));
    }
    let results = ordinary_results(operation);
    let expected_results = match kind {
        ExecutionAtomicKindV1::Store => 0,
        ExecutionAtomicKindV1::CompareExchange => 2,
        _ => 1,
    };
    if results.len() != expected_results {
        return Err(incomplete(
            module,
            format!(
                "V13 {kind:?} atomic has {} physical results, expected {expected_results}",
                results.len()
            ),
        ));
    }
    output.push(Operation::new(
        results,
        OperationKind::Atomic(Atomic {
            kind: atomic_kind,
            pointer,
            value,
            compare,
            access: memory_access(
                address_space,
                value_type.bit_width().unwrap_or(8).div_ceil(8),
            ),
            scope: scope.synchronization_scope(),
            ordering: success
                .unwrap_or(fe2o3_kernel_ir::ExecutionMemoryOrderingV1::Relaxed)
                .memory_ordering(),
            failure_ordering: failure.map(|ordering| ordering.memory_ordering()),
        }),
    ));
    for result in operation
        .results
        .iter()
        .filter(|result| matches!(result.ty, Type::ExecutionCapability(_)))
    {
        aliases.insert(
            result.id,
            ExecutionAliasV1::Physical(physical_value(aliases, pointer)),
        );
    }
    Ok(())
}

fn workgroup_barrier(semantics: ExecutionMemorySemanticsV1) -> Operation {
    Operation::new(
        vec![],
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: semantics.scope.synchronization_scope(),
            semantics: barrier_semantics(semantics),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    )
}

fn fence(semantics: ExecutionMemorySemanticsV1) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Fence(fe2o3_kernel_ir::Fence {
            memory_scope: semantics.scope.synchronization_scope(),
            semantics: barrier_semantics(semantics),
        }),
    )
}

fn barrier_semantics(semantics: ExecutionMemorySemanticsV1) -> BarrierSemantics {
    BarrierSemantics::new(
        semantics.ordering.memory_ordering(),
        semantics.spaces.address_spaces(),
    )
}

fn memory_access(space: ExecutionMemoryAddressSpaceV1, alignment: u16) -> MemoryAccess {
    MemoryAccess::new(space.address_space(), u32::from(alignment))
}

fn emit_gep(
    module: &Module,
    base: ValueId,
    offset: ValueId,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<ValueId, LoweringErrors> {
    let pointer_type = types
        .get(&base)
        .filter(|ty| matches!(ty, Type::Pointer(_)))
        .cloned()
        .ok_or_else(|| {
            incomplete(
                module,
                "V13 memory view does not resolve to a physical pointer",
            )
        })?;
    let result = fresh_value(next_value)?;
    output.push(Operation::effect_free(
        ValueDef::new(result, pointer_type.clone()),
        OperationKind::GetElementPointer { base, offset },
    ));
    types.insert(result, pointer_type);
    Ok(result)
}

fn materialize_pointer(
    module: &Module,
    physical: &[(ValueId, Type)],
    space: ExecutionMemoryAddressSpaceV1,
    types: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<PhysicalValueV1, LoweringErrors> {
    for (value, ty) in physical {
        match ty {
            Type::Pointer(pointer) if pointer.address_space == space.address_space() => {
                return Ok(PhysicalValueV1::scalar(*value));
            }
            Type::Slice(slice) if slice.address_space == space.address_space() => {
                let extent = fresh_value(next_value)?;
                output.push(Operation::effect_free(
                    ValueDef::new(extent, Type::INDEX),
                    OperationKind::SliceLength { slice: *value },
                ));
                types.insert(extent, Type::INDEX);
                let result = fresh_value(next_value)?;
                let pointer =
                    Type::pointer((*slice.element).clone(), slice.address_space, slice.access);
                output.push(Operation::effect_free(
                    ValueDef::new(result, pointer.clone()),
                    OperationKind::SliceData { slice: *value },
                ));
                types.insert(result, pointer);
                return Ok(PhysicalValueV1::view(
                    result,
                    PhysicalExtentV1::Dynamic(extent),
                ));
            }
            Type::GlobalCapability(capability)
                if space == ExecutionMemoryAddressSpaceV1::Global =>
            {
                let extent = fresh_value(next_value)?;
                output.push(Operation::effect_free(
                    ValueDef::new(extent, Type::INDEX),
                    OperationKind::SliceLength { slice: *value },
                ));
                types.insert(extent, Type::INDEX);
                let result = fresh_value(next_value)?;
                let pointer = capability.physical_pointer_type();
                output.push(Operation::effect_free(
                    ValueDef::new(result, pointer.clone()),
                    OperationKind::SliceData { slice: *value },
                ));
                types.insert(result, pointer);
                return Ok(PhysicalValueV1::view(
                    result,
                    PhysicalExtentV1::Dynamic(extent),
                ));
            }
            _ => {}
        }
    }
    Err(incomplete(
        module,
        format!("V13 {space:?} memory binding has no physical pointer/slice carrier"),
    ))
}

fn physical_operands(
    contract: &ExecutionCapabilityOpV1,
    original_types: &BTreeMap<ValueId, Type>,
    lowered_types: &BTreeMap<ValueId, Type>,
    aliases: &BTreeMap<ValueId, ExecutionAliasV1>,
) -> Vec<(ValueId, Type)> {
    contract
        .operands
        .iter()
        .filter_map(|operand| match aliases.get(operand) {
            Some(ExecutionAliasV1::Physical(physical)) => lowered_types
                .get(&physical.value)
                .cloned()
                .map(|ty| (physical.value, ty)),
            Some(ExecutionAliasV1::Erased) => None,
            None => original_types.get(operand).and_then(|ty| {
                (!matches!(ty, Type::ExecutionCapability(_) | Type::KernelContext(_)))
                    .then(|| (*operand, ty.clone()))
            }),
        })
        .collect()
}

fn require_pointer(
    module: &Module,
    physical: &[(ValueId, Type)],
) -> Result<ValueId, LoweringErrors> {
    physical
        .iter()
        .find_map(|(value, ty)| matches!(ty, Type::Pointer(_)).then_some(*value))
        .ok_or_else(|| incomplete(module, "V13 operation has no physical pointer carrier"))
}

fn require_index(module: &Module, physical: &[(ValueId, Type)]) -> Result<ValueId, LoweringErrors> {
    physical
        .iter()
        .find_map(|(value, ty)| (ty == &Type::INDEX).then_some(*value))
        .ok_or_else(|| incomplete(module, "V13 operation has no physical index carrier"))
}

fn require_scalar_other_than(
    module: &Module,
    physical: &[(ValueId, Type)],
    excluded: ValueId,
) -> Result<ValueId, LoweringErrors> {
    physical
        .iter()
        .rev()
        .find_map(|(value, ty)| (*value != excluded && ty.as_scalar().is_some()).then_some(*value))
        .ok_or_else(|| incomplete(module, "V13 operation has no physical scalar carrier"))
}

fn ordinary_results(operation: &Operation) -> Vec<ValueDef> {
    operation
        .results
        .iter()
        .filter(|result| !matches!(result.ty, Type::ExecutionCapability(_)))
        .cloned()
        .collect()
}

fn sole_capability_result<'a>(
    module: &Module,
    operation: &'a Operation,
) -> Result<&'a ValueDef, LoweringErrors> {
    let results = operation
        .results
        .iter()
        .filter(|result| matches!(result.ty, Type::ExecutionCapability(_)))
        .collect::<Vec<_>>();
    match results.as_slice() {
        [result] if ordinary_results(operation).is_empty() => Ok(result),
        _ => Err(incomplete(
            module,
            "V13 logical allocation/index operation requires exactly one capability result",
        )),
    }
}

fn erase_capability_results(
    module: &Module,
    operation: &Operation,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
) -> Result<(), LoweringErrors> {
    if !ordinary_results(operation).is_empty() {
        return Err(incomplete(
            module,
            "logical V13 operation unexpectedly carries a physical result",
        ));
    }
    for result in &operation.results {
        aliases.insert(result.id, ExecutionAliasV1::Erased);
    }
    Ok(())
}

fn alias_capability_results(
    module: &Module,
    operation: &Operation,
    physical: PhysicalValueV1,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
) -> Result<(), LoweringErrors> {
    let mut count = 0;
    for result in &operation.results {
        let Type::ExecutionCapability(capability) = &result.ty else {
            continue;
        };
        let alias = match capability.role {
            ExecutionCapabilityRoleV1::Lds { .. }
            | ExecutionCapabilityRoleV1::ScopedAtomic { .. }
            | ExecutionCapabilityRoleV1::PendingAsyncCopy { .. }
            | ExecutionCapabilityRoleV1::MemoryView { .. } => ExecutionAliasV1::Physical(physical),
            ExecutionCapabilityRoleV1::KernelAuthority
            | ExecutionCapabilityRoleV1::Workgroup
            | ExecutionCapabilityRoleV1::Subgroup { .. }
            | ExecutionCapabilityRoleV1::Matrix { .. }
            | ExecutionCapabilityRoleV1::WorkgroupMemoryIndex
            | ExecutionCapabilityRoleV1::EpochTransition
            | ExecutionCapabilityRoleV1::UnsafeRawMemoryObligation => ExecutionAliasV1::Erased,
        };
        aliases.insert(result.id, alias);
        count += 1;
    }
    if count == 0 {
        return Err(incomplete(
            module,
            "V13 typestate transition has no capability result",
        ));
    }
    Ok(())
}

fn physical_value(
    aliases: &BTreeMap<ValueId, ExecutionAliasV1>,
    value: ValueId,
) -> PhysicalValueV1 {
    aliases
        .values()
        .find_map(|alias| match alias {
            ExecutionAliasV1::Physical(physical) if physical.value == value => Some(*physical),
            ExecutionAliasV1::Physical(_) | ExecutionAliasV1::Erased => None,
        })
        .unwrap_or_else(|| PhysicalValueV1::scalar(value))
}

fn raw_memory_extent(contract: &ExecutionCapabilityOpV1) -> Option<PhysicalExtentV1> {
    let ExecutionCapabilityOperationV1::RawMemoryBind { extent, .. } = &contract.operation else {
        return None;
    };
    contract
        .operands
        .get(usize::from(extent.operand))
        .copied()
        .map(PhysicalExtentV1::Dynamic)
}

fn require_wave(
    module: &Module,
    width: u32,
    selected: &mut Option<WaveWidth>,
) -> Result<WaveWidth, LoweringErrors> {
    let width = match width {
        32 => WaveWidth::Wave32,
        64 => WaveWidth::Wave64,
        _ => {
            return Err(incomplete(
                module,
                format!("AMD production V13 lowering has no physical subgroup width {width}"),
            ));
        }
    };
    if selected.is_some_and(|selected| selected != width) {
        return Err(incomplete(
            module,
            "one physical function cannot mix Wave32 and Wave64 execution contracts",
        ));
    }
    *selected = Some(width);
    Ok(width)
}

fn value_types(function: &Function) -> BTreeMap<ValueId, Type> {
    let mut types = BTreeMap::new();
    let body = function.body.as_ref().expect("definition required");
    types.extend(
        body.parameters
            .iter()
            .copied()
            .zip(function.signature.parameters.iter().cloned()),
    );
    for block in &body.blocks {
        types.extend(
            block
                .parameters
                .iter()
                .map(|value| (value.id, value.ty.clone())),
        );
        types.extend(
            block
                .operations
                .iter()
                .flat_map(|operation| &operation.results)
                .map(|value| (value.id, value.ty.clone())),
        );
    }
    types
}

fn infer_element_types(
    module: &Module,
    function: &Function,
    types: &BTreeMap<ValueId, Type>,
) -> Result<BTreeMap<ExecutionTypeIdentityV1, Type>, LoweringErrors> {
    let mut inferred = BTreeMap::new();
    let body = function.body.as_ref().expect("definition required");
    for operation in body.blocks.iter().flat_map(|block| &block.operations) {
        let OperationKind::ExecutionCapability(contract) = &operation.kind else {
            continue;
        };
        let (identity, ty) = match &contract.operation {
            ExecutionCapabilityOperationV1::Atomic {
                element,
                value_type,
                ..
            }
            | ExecutionCapabilityOperationV1::WorkgroupCollective {
                element,
                value_type,
                ..
            }
            | ExecutionCapabilityOperationV1::SubgroupCollective {
                element,
                value_type,
                ..
            } => (*element, Some(Type::Scalar(*value_type))),
            ExecutionCapabilityOperationV1::LdsReadPublished { element, .. }
            | ExecutionCapabilityOperationV1::MemoryLoad { element, .. } => (
                *element,
                ordinary_results(operation)
                    .first()
                    .map(|result| result.ty.clone()),
            ),
            ExecutionCapabilityOperationV1::LdsInitializeByInvocation { element, .. }
            | ExecutionCapabilityOperationV1::MemoryStore { element, .. } => (
                *element,
                contract
                    .operands
                    .iter()
                    .rev()
                    .filter_map(|value| types.get(value))
                    .find(|ty| ty.as_scalar().is_some())
                    .cloned(),
            ),
            ExecutionCapabilityOperationV1::RawMemoryBind { element, .. } => (
                *element,
                contract
                    .operands
                    .iter()
                    .filter_map(|value| types.get(value))
                    .find_map(|ty| match ty {
                        Type::Pointer(pointer) => Some((*pointer.pointee).clone()),
                        Type::Slice(slice) => Some((*slice.element).clone()),
                        Type::GlobalCapability(capability) => Some(capability.element().clone()),
                        _ => None,
                    }),
            ),
            ExecutionCapabilityOperationV1::AsyncCopy { element, .. } => (
                *element,
                contract
                    .operands
                    .iter()
                    .filter_map(|value| types.get(value))
                    .find_map(|ty| match ty {
                        Type::Pointer(pointer) => Some((*pointer.pointee).clone()),
                        Type::Slice(slice) => Some((*slice.element).clone()),
                        Type::GlobalCapability(capability) => Some(capability.element().clone()),
                        _ => None,
                    }),
            ),
            _ => continue,
        };
        let Some(ty) = ty else {
            continue;
        };
        if inferred
            .insert(identity, ty.clone())
            .is_some_and(|old| old != ty)
        {
            return Err(LoweringErrors::one(
                LoweringLocation::module(module),
                LoweringDiagnosticCode::IncompleteOperation,
                "one V13 element identity maps to conflicting physical KIR types",
            ));
        }
    }
    Ok(inferred)
}

fn next_value_id(module: &Module, function: &Function) -> Result<u32, LoweringErrors> {
    let maximum = value_types(function)
        .keys()
        .map(|value| value.0)
        .max()
        .unwrap_or(0);
    maximum.checked_add(1).ok_or_else(|| {
        LoweringErrors::one(
            LoweringLocation::device_function(module, function),
            LoweringDiagnosticCode::ResourceLimit,
            "V13 lowering exhausted the ValueId namespace",
        )
    })
}

fn fresh_value(next: &mut u32) -> Result<ValueId, LoweringErrors> {
    let value = ValueId(*next);
    *next = next.checked_add(1).ok_or_else(|| {
        LoweringErrors::one(
            LoweringLocation::module(&Module::new("v13-value-id")),
            LoweringDiagnosticCode::ResourceLimit,
            "V13 lowering exhausted the ValueId namespace",
        )
    })?;
    Ok(value)
}

fn incomplete(module: &Module, message: impl Into<String>) -> LoweringErrors {
    LoweringErrors::one(
        LoweringLocation::module(module),
        LoweringDiagnosticCode::IncompleteOperation,
        message,
    )
}
