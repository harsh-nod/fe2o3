use super::*;
use fe2o3_kernel_ir::{
    Axis, CastKind, FormalMemoryAccessKind, FunctionRole, IndexKind, IntrinsicKind,
    KirLocalMemoryEffectRefV1,
};

include!("production_checked_output_masked_assert_success_v1.rs");

pub(super) fn source(
    owner: &ProductionSemanticKirOwnerV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    source_parts(owner.semantic().semantic(), &owner.correspondence, budget)
}

pub(super) fn source_parts(
    source: &AdmittedInertSemanticMirV1,
    correspondence: &SemanticKirCorrespondenceV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    charge(budget, 4)?;
    if source.roots().is_empty() || !source.statics().is_empty() || !source.allocations().is_empty()
    {
        return Err(refused(
            "source",
            "nonempty roots without source statics or allocations",
        ));
    }
    let roles = source_roles::check_source_parts(source, correspondence, budget)?;
    for (ordinal, function) in source.functions().iter().enumerate() {
        charge(budget, 7)?;
        let helper = roles.retained_helper(ordinal)?;
        charge(budget, function.blocks().len())?;
        let has_shift_assertion = function.blocks().iter().any(|block| {
            matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Assert {
                    message: SemanticAssertMessageV1::Overflow {
                        operation: SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight,
                        ..
                    },
                    ..
                }
            )
        });
        if has_shift_assertion {
            let function =
                SemanticFunctionIdV1::from_index(u32::try_from(ordinal).map_err(|_| arithmetic())?);
            // The callback returns the census result, not a query owner. Source
            // errors survive intact after the query's exact-floor cleanup.
            crate::with_production_semantic_masked_shift_query_v1(
                source,
                function,
                fe2o3_mir_model::SemanticMaskedShiftLimitsV1::default(),
                budget,
                |query, budget| {
                    Ok(source_function(
                        source,
                        ordinal,
                        helper,
                        Some(query),
                        budget,
                    ))
                },
            )
            .map_err(masked_assertion_query_error_v1)??;
        } else {
            source_function(source, ordinal, helper, None, budget)?;
        }
    }
    Ok(())
}

fn source_function(
    source: &AdmittedInertSemanticMirV1,
    ordinal: usize,
    helper: bool,
    mut masked_assertions: Option<&mut crate::ProductionSemanticMaskedShiftQueryV1<'_, '_>>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    let function = source.functions().get(ordinal).ok_or(E::Source(
        ProductionSemanticKirErrorV1::CorrespondenceMismatch,
    ))?;
    let abi = function.abi();
    if function.role() == SemanticFunctionRoleV1::KernelRoot
        && (abi.can_unwind()
            || abi.c_variadic()
            || !abi.hidden_arguments().is_empty()
            || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
            || !matches!(
                source.types()[abi.return_type().index() as usize].shape(),
                SemanticTypeShapeV1::Unit
            ))
    {
        return Err(refused("source", "Unit root ABI"));
    }
    // Retained fixed arrays require the exact source/N/ranked relation and
    // the independent actual-B/O private-memory census below. A source
    // array type alone is neither an admission nor a refusal certificate.
    for (block_ordinal, block) in function.blocks().iter().enumerate() {
        charge(budget, 1)?;
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::SwitchInt { .. }
            | SemanticTerminatorKindV1::Assert { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::Unreachable => {}
            SemanticTerminatorKindV1::Call(call) => {
                charge(budget, 1)?;
                match source.callables().get(call.callee().index() as usize) {
                    Some(SemanticCallableDeclV1::CompilerIntrinsic { .. }) if !helper => {}
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::SaturatingInteger(_),
                        ..
                    }) => {}
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation:
                            SemanticCompilerIntrinsicOperationV1::MathContextCurrent { .. }
                            | SemanticCompilerIntrinsicOperationV1::MathF32 {
                                function:
                                    fe2o3_mir_model::semantic_mir_v1::SemanticF32MathFunctionV1::Exp,
                                ..
                            },
                        ..
                    }) => {}
                    Some(SemanticCallableDeclV1::Defined { function: callee })
                        if source.functions().get(callee.index() as usize).is_some_and(
                            |callee| callee.role() == SemanticFunctionRoleV1::InternalHelper,
                        ) => {}
                    _ => return Err(refused("source", "closed direct scalar helper calls")),
                }
            }
            _ => return Err(refused("source", "closed control and assertion grammar")),
        }
        for (statement_ordinal, statement) in block.statements().iter().enumerate() {
            charge(budget, 1)?;
            let value = match statement.kind() {
                SemanticStatementKindV1::Nop
                | SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Deinitialize(_)
                | SemanticStatementKindV1::SetDiscriminant { .. } => continue,
                SemanticStatementKindV1::Store(store)
                    if store.volatility() == SemanticVolatilityV1::NonVolatile =>
                {
                    continue;
                }
                SemanticStatementKindV1::Assign(assignment) => assignment.value(),
                _ => return Err(refused("source", "no atomic, volatile or assumed effects")),
            };
            charge(budget, 2)?;
            match value.kind() {
                SemanticRvalueKindV1::Use(_)
                | SemanticRvalueKindV1::Length(_)
                | SemanticRvalueKindV1::Discriminant(_)
                | SemanticRvalueKindV1::Aggregate(_)
                | SemanticRvalueKindV1::Borrow { .. }
                | SemanticRvalueKindV1::AddressOf { .. }
                | SemanticRvalueKindV1::CheckedBinary(_)
                | SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::Not | SemanticUnaryOpV1::PointerMetadata,
                    ..
                }
                | SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer | SemanticCastKindV1::Pointer,
                    ..
                }
                | SemanticRvalueKindV1::Binary {
                    operation:
                        SemanticBinaryOpV1::BitAnd
                        | SemanticBinaryOpV1::BitOr
                        | SemanticBinaryOpV1::BitXor
                        | SemanticBinaryOpV1::Equal
                        | SemanticBinaryOpV1::NotEqual
                        | SemanticBinaryOpV1::LessThan
                        | SemanticBinaryOpV1::LessOrEqual
                        | SemanticBinaryOpV1::GreaterThan
                        | SemanticBinaryOpV1::GreaterOrEqual,
                    ..
                } => {}
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Float,
                    operand,
                } if numeric_casts::source_integer_to_f32(
                    source,
                    operand.ty(),
                    value.result_type(),
                    budget,
                )? => {}
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight,
                    ..
                } if constant_shifts::source(source, value, budget)?
                    || masked_shifts::source(
                        source,
                        ordinal,
                        block_ordinal,
                        statement_ordinal,
                        budget,
                    )?
                    || masked_assertion_success_shift_v1(
                        source,
                        ordinal,
                        block_ordinal,
                        statement_ordinal,
                        masked_assertions.as_deref_mut(),
                        budget,
                    )? => {}
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::Negate,
                    ..
                }
                | SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Divide,
                    ..
                } if matches!(
                    source.types()[value.result_type().index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 })
                ) => {}
                SemanticRvalueKindV1::Binary {
                    operation:
                        SemanticBinaryOpV1::Add
                        | SemanticBinaryOpV1::Subtract
                        | SemanticBinaryOpV1::Multiply,
                    ..
                } if matches!(
                    source.types()[value.result_type().index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(
                        SemanticScalarTypeV1::Float { bits: 32 | 64 }
                            | SemanticScalarTypeV1::Integer {
                                bits: 8 | 16 | 32 | 64,
                                ..
                            }
                    )
                ) =>
                {
                    // Integer wrapping results materialize as result zero
                    // of Checked; exact source/N/B joins retain that pair.
                    // Plain native integer arithmetic remains inadmissible.
                }
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Divide | SemanticBinaryOpV1::Remainder,
                    ..
                } if matches!(
                    source.types()[value.result_type().index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32 | 64,
                    })
                ) =>
                {
                    // The native census must independently prove a nonzero
                    // divisor at each actual B/C/O operation, not just N.
                }
                SemanticRvalueKindV1::Load(load)
                    if load.volatility() == SemanticVolatilityV1::NonVolatile => {}
                _ => {
                    return Err(refused(
                        "source",
                        "total scalar/global recipe; no unchecked arithmetic",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn scalar(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(_))
}

pub(super) fn ranked(
    lowering: &ProductionRankedKernelLoweringInputV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    for block in lowering.kernel().blocks() {
        charge(budget, 1)?;
        for operation in block.operations() {
            charge(budget, 1)?;
            match operation {
                ProductionRankedOperationV1::ExecutionLayout { .. }
                | ProductionRankedOperationV1::View { .. }
                | ProductionRankedOperationV1::IndexConstant { .. }
                | ProductionRankedOperationV1::IndexUnsignedCast { .. }
                | ProductionRankedOperationV1::IndexUnknown { .. }
                | ProductionRankedOperationV1::InvocationIndex { .. }
                | ProductionRankedOperationV1::IndexBinary { .. }
                | ProductionRankedOperationV1::DeterministicJoin { .. }
                | ProductionRankedOperationV1::Dimension { .. }
                | ProductionRankedOperationV1::Access { .. }
                | ProductionRankedOperationV1::ValueAccess { .. }
                | ProductionRankedOperationV1::OwnershipContract { .. }
                | ProductionRankedOperationV1::SemanticExpression { .. }
                | ProductionRankedOperationV1::RequireEquivalent { .. } => {}
                ProductionRankedOperationV1::ViewInSpace { memory_space, .. }
                    if matches!(
                        *memory_space,
                        dialect_kernel::MemorySpaceAttr::Global
                            | dialect_kernel::MemorySpaceAttr::Private
                    ) => {}
                _ => {
                    return Err(refused(
                        "ranked",
                        "closed global effects and source scalar recipes",
                    ));
                }
            }
        }
        // All ranked control forms carry either ordinary scalar dependencies,
        // an edge payload, Return or Trap. Their exact source reconciliation is
        // independently replayed by the consumed source/ranked owner.
    }
    Ok(())
}
fn ty(ty: &Type) -> bool {
    scalar(ty)
        || match ty {
            Type::Pointer(pointer) => {
                pointer.address_space == AddressSpace::Global && scalar(&pointer.pointee)
            }
            Type::Slice(slice) => {
                slice.address_space == AddressSpace::Global && scalar(&slice.element)
            }
            _ => false,
        }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn native(
    inventory: &CanonicalKirInventoryV1<'_>,
    private: &private_memory::PrivateMemory<'_, '_>,
    division: &unsigned_division::UnsignedDivision<'_, '_>,
    helpers: &scalar_helpers::RawEmptyScalarHelpers<'_, '_>,
    phase: &'static str,
    mut authorized_trap: impl FnMut(usize, CanonicalKirOperationCoordinateV1) -> R<bool>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    charge(budget, 2)?;
    if !private.is_for(inventory) {
        return Err(refused(phase, "same-inventory private-memory census"));
    }
    if inventory.kernels().is_empty() {
        return Err(refused(phase, "complete root-only module"));
    }
    let mut roots = 0usize;
    for function in inventory.functions() {
        charge(budget, 3)?;
        let helper = helpers.function(inventory, function.coordinate, budget)?;
        if function.function.role == FunctionRole::ExternalImport {
            charge(
                budget,
                function
                    .function
                    .id
                    .as_str()
                    .len()
                    .checked_add(2)
                    .and_then(|n| n.checked_mul(8))
                    .ok_or_else(arithmetic)?,
            )?;
            // V12 admission already checks the exact reserved declaration,
            // including its signature and capabilities. Each use below must
            // additionally come from a transported source assertion trap.
            if matches!(
                fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(
                    &function.function.id,
                    &[],
                ),
                Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap)
            ) {
                continue;
            }
            if exp::declaration(function.function, budget)? {
                continue;
            }
            return Err(refused(
                phase,
                "only the canonical assertion trap declaration",
            ));
        }
        if helper {
            continue;
        }
        if function.function.role != FunctionRole::KernelEntry
            || function.function.body.is_none()
            || !function.function.signature.results.is_empty()
        {
            return Err(refused(phase, "defined Unit roots only"));
        }
        roots += 1;
    }
    if roots != inventory.kernels().len() {
        return Err(refused(phase, "complete root-only module"));
    }
    for (index, definition) in inventory.definitions().iter().enumerate() {
        charge(budget, 1)?;
        if !ty(definition.ty) && !private.definition(index) {
            if matches!(definition.ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Private)
            {
                return Err(E::PrivateAddressR2);
            }
            return Err(refused(phase, "scalar/global pointer or slice type"));
        }
    }
    for block in inventory.blocks() {
        charge(budget, 1)?;
        match block.terminator {
            Terminator::Branch { .. }
            | Terminator::ConditionalBranch { .. }
            | Terminator::Switch { .. }
            | Terminator::IntegerSwitch { .. }
            | Terminator::Unreachable => {}
            Terminator::Return { values }
                if values.is_empty()
                    || helpers.function(inventory, block.coordinate.function, budget)? => {}
            _ => return Err(refused(phase, "closed native control")),
        }
    }
    for (ordinal, row) in inventory.operations().iter().enumerate() {
        charge(budget, 5)?;
        if !row.compiler_ordering().is_empty() {
            return Err(refused(phase, "no ordered effects"));
        }
        let memory = match &row.operation.kind {
            OperationKind::Load { access, .. } | OperationKind::GuardedLoad { access, .. } => {
                Some((*access, false))
            }
            OperationKind::Store { access, .. } | OperationKind::GuardedStore { access, .. } => {
                Some((*access, true))
            }
            OperationKind::Alloca {
                address_space: AddressSpace::Private,
                ..
            } if private.operation(ordinal) => continue,
            OperationKind::Alloca { .. } => return Err(E::PrivateAddressR2),
            OperationKind::Constant(_)
            | OperationKind::Compare { .. }
            | OperationKind::Select { .. }
            | OperationKind::SliceData { .. }
            | OperationKind::SliceLength { .. }
            | OperationKind::GetElementPointer { .. }
            | OperationKind::Unary {
                op: UnaryOp::Not, ..
            }
            | OperationKind::Binary {
                op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Checked(_),
                ..
            } => None,
            OperationKind::Binary {
                op: BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply,
                ..
            } if matches!(row.operation.results.as_slice(), [value] if matches!(value.ty, Type::F32 | Type::F64)) => {
                None
            }
            OperationKind::Binary {
                op: BinaryOp::ShiftLeft | BinaryOp::ShiftRight,
                ..
            } if constant_shifts::native(inventory, ordinal, budget)?
                || masked_shifts::native(inventory, ordinal, budget)? =>
            {
                None
            }
            OperationKind::Unary {
                op: UnaryOp::Negate,
                ..
            }
            | OperationKind::Binary {
                op: BinaryOp::Divide,
                ..
            } if matches!(row.operation.results.as_slice(), [value] if value.ty == Type::F32) => {
                // Exact IEEE F32 recipes, not the integer nonzero/overflow
                // rule below. No reciprocal rewrite or fast-math permission.
                None
            }
            OperationKind::Cast {
                kind: CastKind::IntegerToFloat | CastKind::FloatToInteger,
                ..
            } if numeric_casts::native(inventory, ordinal, budget)? => None,
            OperationKind::Cast {
                kind:
                    CastKind::Truncate
                    | CastKind::ZeroExtend
                    | CastKind::SignExtend
                    | CastKind::Bitcast
                    | CastKind::RestrictPointerAccess,
                ..
            } => None,
            OperationKind::Binary {
                op: BinaryOp::Divide | BinaryOp::Remainder,
                ..
            } => {
                if !division.operation(inventory, ordinal, budget)? {
                    return Err(refused(
                        phase,
                        "unsigned division requires a nonzero divisor on every incoming path",
                    ));
                }
                None
            }
            OperationKind::Intrinsic(intrinsic)
                if matches!(
                    intrinsic.kind,
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Global,
                        axis: Axis::X
                    } | IntrinsicKind::LaunchExtent { axis: Axis::X }
                ) =>
            {
                None
            }
            OperationKind::Call { callee, arguments } => {
                if !helpers.call(inventory, ordinal, budget)? && !exp::call(row.operation, budget)?
                {
                    charge(
                        budget,
                        callee
                            .as_str()
                            .len()
                            .checked_add(2)
                            .and_then(|n| n.checked_mul(8))
                            .ok_or_else(arithmetic)?,
                    )?;
                    if !arguments.is_empty()
                        || !row.operation.results.is_empty()
                        || !authorized_trap(ordinal, row.coordinate)?
                    {
                        return Err(refused(phase, "source-authorized trap only"));
                    }
                    if !matches!(
                        fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(
                            callee, arguments
                        ),
                        Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap)
                    ) {
                        return Err(refused(phase, "exact reserved trap descriptor"));
                    }
                    let function = &inventory.functions()[row.coordinate.block.function.0 as usize];
                    let block = &inventory.blocks()
                        [function.blocks.start + row.coordinate.block.block as usize];
                    if ordinal + 1 != block.operations.end
                        || !matches!(block.terminator, Terminator::Unreachable)
                    {
                        return Err(refused(phase, "terminating trap control"));
                    }
                }
                None
            }
            _ => return Err(refused(phase, "closed opcode census")),
        };
        match memory {
            Some((access, write)) => {
                if access.address_space == AddressSpace::Private {
                    if private.operation(ordinal) {
                        continue;
                    }
                    return Err(E::PrivateAddressR2);
                }
                if access.address_space != AddressSpace::Global
                    || access.volatile
                    || row.effects.len() != 1
                {
                    return Err(refused(phase, "one ordinary global effect"));
                }
                charge(budget, 1)?;
                let effect = &inventory.effects()[row.effects.start].effect;
                if !matches!(
                    (write, effect),
                    (false, KirLocalMemoryEffectRefV1::Read(AddressSpace::Global))
                        | (true, KirLocalMemoryEffectRefV1::Write(AddressSpace::Global))
                ) {
                    return Err(refused(phase, "exact physical memory effect"));
                }
            }
            None if !row.effects.is_empty() => {
                return Err(refused(phase, "unexpected physical effect"));
            }
            None => {}
        }
    }
    Ok(())
}

pub(super) fn formal(
    inventory: &CanonicalKirInventoryV1<'_>,
    private: &private_memory::PrivateMemory<'_, '_>,
    reports: &[FormalMemoryObligations],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    charge(budget, 2)?;
    if !private.is_for(inventory) {
        return Err(refused("formal O", "same-inventory private-memory census"));
    }
    if reports.len() != inventory.kernels().len() || reports.is_empty() {
        return Err(refused("formal O", "exact kernel report roster"));
    }
    for (kernel, report) in inventory.kernels().iter().zip(reports) {
        let work = kernel
            .kernel
            .id
            .as_str()
            .len()
            .checked_add(kernel.kernel.entry.as_str().len())
            .and_then(|n| n.checked_add(4))
            .ok_or_else(arithmetic)?;
        charge(budget, work)?;
        if report.kernel() != &kernel.kernel.id
            || report.entry() != &kernel.kernel.entry
            || !report.inter_invocation_conflicts().is_empty()
        {
            return Err(refused("formal O", "exact conflict-free root report"));
        }
        let function = &inventory.functions()[kernel.entry.0 as usize];
        let mut next = 0usize;
        for block in &inventory.blocks()[function.blocks.clone()] {
            charge(budget, 1)?;
            for ordinal in block.operations.clone() {
                let operation = &inventory.operations()[ordinal];
                charge(budget, 1)?;
                let (kind, access) = match &operation.operation.kind {
                    OperationKind::Load { access, .. }
                    | OperationKind::GuardedLoad { access, .. } => {
                        (FormalMemoryAccessKind::Read, access)
                    }
                    OperationKind::Store { access, .. }
                    | OperationKind::GuardedStore { access, .. } => {
                        (FormalMemoryAccessKind::Write, access)
                    }
                    _ => continue,
                };
                if access.address_space == AddressSpace::Private {
                    if !private.operation(ordinal) {
                        return Err(refused("formal O", "unproved private access"));
                    }
                    continue;
                }
                charge(budget, 6)?;
                let fact = report.accesses().get(next).ok_or_else(|| {
                    refused("formal O", "every physical access has an actual-O fact")
                })?;
                if fact.location().block != block.block.id
                    || fact.location().operation_index != operation.coordinate.operation as usize
                    || fact.kind() != kind
                    || fact.address_space() != access.address_space
                    || fact.alignment() != u64::from(access.alignment)
                {
                    return Err(refused(
                        "formal O",
                        "exact physical access/fact correspondence",
                    ));
                }
                next += 1;
            }
        }
        charge(budget, 1)?;
        if next != report.accesses().len() {
            return Err(refused("formal O", "no extra access facts"));
        }
    }
    Ok(())
}
