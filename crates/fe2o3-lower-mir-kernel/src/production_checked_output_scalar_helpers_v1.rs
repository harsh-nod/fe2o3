use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCallEffectDecisionV1, CanonicalKirCallEffectErrorV1, CanonicalKirCallEffectsV1,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirFunctionCoordinateV1, CastKind, FunctionRole, OperationKind, ScalarType,
    Terminator, Type, UnaryOp,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticAbiPassModeV1, SemanticCanonAbiV1, SemanticExternAbiV1,
    SemanticFunctionIdV1, SemanticFunctionRoleV1, SemanticLocalRoleV1, SemanticScalarTypeV1,
    SemanticSourceArgumentOwnershipV1, SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1,
};

/// A physical scalar-helper census, not source identity, totality, or a returned
/// value summary. Calls remain ordered and cannot acquire optimizer purity.
/// The enclosing admission transaction owns new scratch reservations.
pub(super) struct RawEmptyScalarHelpers<'i, 'g> {
    inventory: &'i CanonicalKirInventoryV1<'g>,
    functions: Vec<u8>,
}

impl RawEmptyScalarHelpers<'_, '_> {
    pub(super) fn function(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        function: CanonicalKirFunctionCoordinateV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<bool> {
        charge(budget, 3)?;
        self.require_inventory(inventory)?;
        Ok(self.functions.get(function.0 as usize).copied() == Some(1))
    }

    pub(super) fn call(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<bool> {
        charge(budget, 3)?;
        self.require_inventory(inventory)?;
        let Some(operation) = inventory.operations().get(ordinal) else {
            return Ok(false);
        };
        if !matches!(operation.operation.kind, OperationKind::Call { .. }) {
            return Ok(false);
        }
        // Inventory call occurrences are in canonical coordinate order. Keep
        // hot queries independent of function-name length and module size.
        let mut first = 0;
        let mut end = inventory.calls().len();
        while first < end {
            charge(budget, 2)?;
            let middle = first + (end - first) / 2;
            let call = &inventory.calls()[middle];
            match call.coordinate.cmp(&operation.coordinate) {
                std::cmp::Ordering::Less => first = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => {
                    return Ok(call.target.is_some_and(|target| {
                        self.functions.get(target.0 as usize).copied() == Some(1)
                    }));
                }
            }
        }
        Err(refused(
            "scalar helpers",
            "complete call occurrence inventory",
        ))
    }

    fn require_inventory(&self, inventory: &CanonicalKirInventoryV1<'_>) -> R<()> {
        if !std::ptr::eq(self.inventory, inventory) {
            return Err(refused("scalar helpers", "same borrowed inventory"));
        }
        Ok(())
    }
}

fn scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Scalar(
            ScalarType::Bool
                | ScalarType::I8
                | ScalarType::U8
                | ScalarType::I16
                | ScalarType::U16
                | ScalarType::I32
                | ScalarType::U32
                | ScalarType::I64
                | ScalarType::U64
                | ScalarType::Index
                | ScalarType::F32
                | ScalarType::F64
        )
    )
}

fn effect_error(error: CanonicalKirCallEffectErrorV1) -> E {
    match error {
        CanonicalKirCallEffectErrorV1::Resource(error) => E::Resource(error),
        _ => refused("scalar helpers", "complete helper effect inventory"),
    }
}

pub(super) fn check<'i, 'g>(
    inventory: &'i CanonicalKirInventoryV1<'g>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<RawEmptyScalarHelpers<'i, 'g>> {
    charge(budget, 3)?;
    let mut helpers = 0usize;
    for function in inventory.functions() {
        charge(budget, 2)?;
        if function.function.role == FunctionRole::InternalHelper {
            helpers = helpers.checked_add(1).ok_or_else(arithmetic)?;
        }
    }
    budget
        .reserve_storage(std::mem::size_of::<&CanonicalKirInventoryV1<'_>>())
        .map_err(E::Resource)?;
    let count = if helpers == 0 {
        0
    } else {
        inventory.functions().len()
    };
    let mut functions = scratch::<u8>(count, budget)?;
    charge(budget, count)?;
    functions.resize(count, 0);
    if helpers == 0 {
        return Ok(RawEmptyScalarHelpers {
            inventory,
            functions,
        });
    }

    let (effects, storage) =
        CanonicalKirCallEffectsV1::derive(inventory, budget).map_err(effect_error)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(E::Resource)?;
    // Inspect every retained helper, including uncalled C/O functions whose
    // call disappeared only through an independently checked dead-control rule.
    for (ordinal, function) in inventory.functions().iter().enumerate() {
        charge(budget, 3)?;
        if function.function.role != FunctionRole::InternalHelper {
            continue;
        }
        if function.function.body.is_none() || function.function.signature.results.len() > 1 {
            return Err(refused(
                "scalar helpers",
                "defined direct scalar or Unit result",
            ));
        }
        for ty in function
            .function
            .signature
            .parameters
            .iter()
            .chain(&function.function.signature.results)
        {
            charge(budget, 1)?;
            if !scalar(ty) {
                return Err(refused(
                    "scalar helpers",
                    "direct ordinary scalar signature",
                ));
            }
        }
        for definition in &inventory.definitions()[function.definitions.clone()] {
            charge(budget, 2)?;
            if !scalar(definition.ty) {
                return Err(refused(
                    "scalar helpers",
                    "ordinary scalar helper definitions",
                ));
            }
        }
        for block in &inventory.blocks()[function.blocks.clone()] {
            charge(budget, 2)?;
            match block.terminator {
                Terminator::Branch { .. }
                | Terminator::ConditionalBranch { .. }
                | Terminator::Switch { .. }
                | Terminator::IntegerSwitch { .. }
                | Terminator::Unreachable => {}
                Terminator::Return { values }
                    if values.len() == function.function.signature.results.len() => {}
                _ => return Err(refused("scalar helpers", "closed scalar helper control")),
            }
        }
        for (offset, row) in inventory.operations()[function.operations.clone()]
            .iter()
            .enumerate()
        {
            charge(budget, 4)?;
            if !row.effects.is_empty() || !row.compiler_ordering().is_empty() {
                return Err(refused(
                    "scalar helpers",
                    "no helper memory or ordering effects",
                ));
            }
            match row.operation.kind {
                OperationKind::Constant(_)
                | OperationKind::Compare { .. }
                | OperationKind::Select { .. }
                | OperationKind::Unary {
                    op: UnaryOp::Not, ..
                }
                | OperationKind::Binary {
                    op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Checked(_),
                    ..
                }
                | OperationKind::Call { .. } => {}
                OperationKind::Unary {
                    op: UnaryOp::Negate,
                    ..
                } if matches!(row.operation.results.as_slice(), [result] if result.ty == Type::F32) =>
                {
                    charge(budget, 2)?;
                }
                OperationKind::Binary {
                    op: BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply,
                    ..
                } => {
                    // Preserve the existing strict native FP policy. This
                    // admits no fast-math flags, integer overflow shortcut,
                    // call purity, or returned-value range facts.
                    charge(budget, 2)?;
                    if !matches!(row.operation.results.as_slice(), [result]
                        if matches!(result.ty, Type::F32 | Type::F64))
                    {
                        return Err(refused("scalar helpers", "closed scalar helper opcode"));
                    }
                }
                OperationKind::Binary {
                    op: op @ (BinaryOp::Divide | BinaryOp::Remainder),
                    ..
                } => {
                    charge(budget, 2)?;
                    if matches!(row.operation.results.as_slice(), [result]
                        if matches!(result.ty, Type::F32 | Type::F64))
                        && !(op == BinaryOp::Divide
                            && matches!(row.operation.results.as_slice(), [result] if result.ty == Type::F32))
                    {
                        return Err(refused("scalar helpers", "closed scalar helper opcode"));
                    }
                }
                OperationKind::Cast {
                    kind: CastKind::IntegerToFloat | CastKind::FloatToInteger,
                    ..
                } if numeric_casts::native(
                    inventory,
                    function.operations.start + offset,
                    budget,
                )? => {}
                OperationKind::Cast {
                    kind:
                        CastKind::Truncate
                        | CastKind::ZeroExtend
                        | CastKind::SignExtend
                        | CastKind::Bitcast,
                    ..
                } => {
                    charge(budget, 3)?;
                    let operand = &inventory.uses()[row.operands.start];
                    let input = inventory.definitions()[operand.definition].ty;
                    if matches!(input, Type::Scalar(ScalarType::F32 | ScalarType::F64))
                        || matches!(row.operation.results.as_slice(), [result]
                            if matches!(result.ty, Type::F32 | Type::F64))
                    {
                        return Err(refused("scalar helpers", "closed scalar helper opcode"));
                    }
                }
                _ => return Err(refused("scalar helpers", "closed scalar helper opcode")),
            }
        }
        for call in &inventory.calls()[function.calls.clone()] {
            charge(budget, 3)?;
            if call.target.is_none_or(|target| {
                inventory.functions()[target.0 as usize].function.role
                    != FunctionRole::InternalHelper
            }) {
                return Err(refused(
                    "scalar helpers",
                    "internal scalar helper callees only",
                ));
            }
        }
        if effects
            .decision(function.coordinate, budget)
            .map_err(effect_error)?
            != CanonicalKirCallEffectDecisionV1::CompleteEmpty
        {
            return Err(refused("scalar helpers", "complete empty helper closure"));
        }
        functions[ordinal] = 1;
    }
    drop(effects);
    budget
        .release_storage(storage.retained_storage())
        .map_err(E::Resource)?;
    Ok(RawEmptyScalarHelpers {
        inventory,
        functions,
    })
}

fn source_scalar(semantic: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        semantic
            .types()
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(
            SemanticScalarTypeV1::Bool
                | SemanticScalarTypeV1::Float { bits: 32 | 64 }
                | SemanticScalarTypeV1::Integer {
                    bits: 8 | 16 | 32 | 64,
                    ..
                }
        ))
    )
}

/// Only an ABI predicate. Root-qualified membership and complete source/N
/// reconstruction are separate mandatory requirements of check_source/caller.
pub(super) fn source_helper_abi(
    semantic: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    charge(budget, 3)?;
    let function = semantic
        .functions()
        .get(function.index() as usize)
        .ok_or_else(|| refused("scalar helpers", "source function coordinate"))?;
    if function.role() != SemanticFunctionRoleV1::InternalHelper {
        return Ok(false);
    }
    charge(budget, 12)?;
    let abi = function.abi();
    if function.export().is_some()
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || abi.arguments().len() != abi.source_input_types().len()
        || abi.arguments().len() != abi.source_argument_ownership().len()
        || abi.fixed_count() as usize != abi.arguments().len()
    {
        return Err(refused("scalar helpers", "ordinary direct Rust helper ABI"));
    }
    for ((argument, ty), ownership) in abi
        .arguments()
        .iter()
        .zip(abi.source_input_types())
        .zip(abi.source_argument_ownership())
    {
        charge(budget, 8)?;
        if !argument.is_source()
            || argument.ty() != *ty
            || argument.value().adjusted().is_some()
            || argument.value().pointee_override().is_some()
            || !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
            || *ownership != SemanticSourceArgumentOwnershipV1::ByValue
            || !source_scalar(semantic, *ty)
        {
            return Err(refused(
                "scalar helpers",
                "direct by-value scalar helper arguments",
            ));
        }
    }
    charge(budget, 6)?;
    let returned = abi.return_value();
    let unit = matches!(
        semantic
            .types()
            .get(abi.source_output_type().index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Unit)
    );
    if returned.source_ty() != abi.source_output_type()
        || returned.adjusted().is_some()
        || returned.pointee_override().is_some()
        || !(unit && matches!(returned.mode(), SemanticAbiPassModeV1::Ignore)
            || source_scalar(semantic, abi.source_output_type())
                && matches!(returned.mode(), SemanticAbiPassModeV1::Direct(_)))
    {
        return Err(refused(
            "scalar helpers",
            "direct scalar or exact Unit source return",
        ));
    }
    let mut returns = 0usize;
    for local in function.locals() {
        charge(budget, 2)?;
        if local.role() == SemanticLocalRoleV1::Return {
            returns = returns.checked_add(1).ok_or_else(arithmetic)?;
            if local.ty() != abi.source_output_type() {
                return Err(refused("scalar helpers", "exact source return local"));
            }
        }
    }
    if returns != 1 {
        return Err(refused("scalar helpers", "one source return local"));
    }
    Ok(true)
}

#[cfg(test)]
#[path = "production_checked_output_scalar_helpers_v1_tests.rs"]
mod tests;
