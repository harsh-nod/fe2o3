//! Nontrapping value recognition, not a numerical or initial-memory theorem.
use super::*;
use crate::pliron_semantic_memory_v1::{PlironSemanticMemorySiteV1, paired_read_access_v1};
use dialect_kernel::{
    SEMANTIC_TYPED_READ_SYMBOL_BASE_V1, SemanticNumericalPolicyAttr, SemanticOverflowAttr,
    SemanticReadOrderingAttr, SemanticReadVolatilityAttr, SemanticTypedBinaryKindAttr,
    SemanticTypedBinaryOp, SemanticTypedConstantOp, SemanticTypedExpressionRootOp,
    SemanticTypedReadOp, SemanticTypedScalarV1, SemanticTypedSelectOp, SemanticTypedSymbolOp,
    SemanticTypedUnaryKindAttr, SemanticTypedUnaryOp,
};
use dialect_proof::{EvidenceRefOp, ObligationOp, RequireEffectRefinementOp, RequireRefinementOp};
use pliron::context::Ptr;
use std::collections::HashSet;

type Error = ConditionalPrefixDerivationErrorV1;

pub(super) fn is_value(op: &dyn Op) -> bool {
    op.downcast_ref::<SemanticTypedSymbolOp>().is_some()
        || op.downcast_ref::<SemanticTypedConstantOp>().is_some()
        || op.downcast_ref::<SemanticTypedUnaryOp>().is_some()
        || op.downcast_ref::<SemanticTypedBinaryOp>().is_some()
        || op.downcast_ref::<SemanticTypedExpressionRootOp>().is_some()
        || op.downcast_ref::<SemanticTypedReadOp>().is_some()
        || op.downcast_ref::<SemanticTypedSelectOp>().is_some()
}

pub(super) fn verify_metadata(
    context: &Context,
    op: &dyn Op,
    views: &HashMap<Value, usize>,
) -> Result<bool, Error> {
    if let Some(effect) = op.downcast_ref::<RequireEffectRefinementOp>() {
        // Only this closed rank-one schema has ten operands. The view must
        // already be one of the adapter's locally verified rank-one views.
        if op.get_operation().deref(context).get_num_operands() != 10
            || !views.contains_key(&effect.view(context))
        {
            return Err(Error::Malformed("effect metadata shape"));
        }
    } else if op.downcast_ref::<ObligationOp>().is_none()
        && op.downcast_ref::<EvidenceRefOp>().is_none()
        && op.downcast_ref::<RequireRefinementOp>().is_none()
    {
        return Ok(false);
    }
    op.verify(context)
        .map_err(|_| Error::Malformed("proof metadata"))?;
    // Local schema validity does not discharge any proof obligation.
    Ok(true)
}

pub(super) fn validate(
    context: &Context,
    model: &PrefixModel,
    operations: &[Vec<Ptr<Operation>>],
    work: &mut usize,
) -> Result<(), Error> {
    let order = topological_blocks(&model.blocks, work)?;
    let mut values = HashMap::<Value, SemanticTypedScalarV1>::new();
    let mut read_symbols = HashSet::new();
    for block in order {
        for (position, pointer) in operations[block].iter().copied().enumerate() {
            let op = Operation::get_op_dyn(pointer, context);
            let raw = pointer.deref(context);
            charge(work, raw.get_num_operands() + 1)?;
            if let Some(access) = op.downcast_ref::<RankedAccessOp>() {
                if let Some(value) = access.stored_value(context) {
                    let scalar = values
                        .get(&value)
                        .ok_or(Error::Unsupported("undefined stored value"))?;
                    let view = Operation::get_op_dyn(
                        access
                            .view(context)
                            .defining_op()
                            .ok_or(Error::Malformed("view"))?,
                        context,
                    );
                    let width = view
                        .downcast_ref::<RankedViewOp>()
                        .and_then(|view| view.view_type(context))
                        .map(|ty| ty.deref(context).element_width());
                    if width != Some(u32::from(scalar.bits())) {
                        return Err(Error::Malformed("stored value width"));
                    }
                }
            }
            if !is_value(&*op) {
                continue;
            }
            op.verify(context)
                .map_err(|_| Error::Malformed("typed value"))?;
            let scalar = if let Some(symbol) = op.downcast_ref::<SemanticTypedSymbolOp>() {
                if symbol
                    .symbol(context)
                    .is_none_or(|id| id >= SEMANTIC_TYPED_READ_SYMBOL_BASE_V1)
                {
                    return Err(Error::Unsupported("unbound read symbol"));
                }
                symbol.scalar(context)
            } else if let Some(constant) = op.downcast_ref::<SemanticTypedConstantOp>() {
                constant.scalar(context)
            } else if let Some(read) = op.downcast_ref::<SemanticTypedReadOp>() {
                if read.guarded(context).is_some()
                    || read.memory_space(context) != Some(MemorySpaceAttr::Global)
                    || read.ordering(context) != Some(SemanticReadOrderingAttr::Unordered)
                    || !matches!(
                        read.volatility(context),
                        Some(
                            SemanticReadVolatilityAttr::NonVolatile
                                | SemanticReadVolatilityAttr::Volatile
                        )
                    )
                    || !read_symbols.insert(
                        read.symbol(context)
                            .ok_or(Error::Malformed("read symbol"))?,
                    )
                {
                    return Err(Error::Unsupported("read semantics or identity"));
                }
                paired_read_access_v1(
                    context,
                    read,
                    position.checked_sub(1).map(|i| operations[block][i]),
                    PlironSemanticMemorySiteV1::new(block, position),
                )
                .map_err(|_| Error::Malformed("unpaired read"))?;
                // The RankedAccessOp alone supplies the memory event. Volatile
                // reads stay observable; neither mode implies initial-memory
                // equality, and read values never control this prefix CFG.
                read.scalar(context)
            } else if let Some(unary) = op.downcast_ref::<SemanticTypedUnaryOp>() {
                let scalar = unary
                    .scalar(context)
                    .ok_or(Error::Malformed("unary scalar"))?;
                let total = match unary.kind(context) {
                    Some(SemanticTypedUnaryKindAttr::Negate) => scalar.is_float(),
                    Some(SemanticTypedUnaryKindAttr::Not) => {
                        scalar.is_bool() || scalar.is_integer()
                    }
                    None => false,
                };
                if !total || values.get(&unary.operand(context)) != Some(&scalar) {
                    return Err(Error::Unsupported("unary domain or operand"));
                }
                Some(scalar)
            } else if let Some(binary) = op.downcast_ref::<SemanticTypedBinaryOp>() {
                let scalar = binary
                    .scalar(context)
                    .ok_or(Error::Malformed("binary scalar"))?;
                use SemanticTypedBinaryKindAttr as Kind;
                let total = match binary.kind(context) {
                    Some(Kind::Add | Kind::Subtract | Kind::Multiply) => !scalar.is_bool(),
                    Some(Kind::Divide | Kind::Remainder) => scalar.is_float(),
                    Some(Kind::BitAnd | Kind::BitOr | Kind::BitXor) => !scalar.is_float(),
                    _ => false,
                };
                if !total
                    || binary.overflow(context) != Some(SemanticOverflowAttr::Wrapping)
                    || values.get(&binary.lhs(context)) != Some(&scalar)
                    || values.get(&binary.rhs(context)) != Some(&scalar)
                {
                    return Err(Error::Unsupported("binary domain or operands"));
                }
                Some(scalar)
            } else if let Some(select) = op.downcast_ref::<SemanticTypedSelectOp>() {
                let scalar = select
                    .scalar(context)
                    .ok_or(Error::Malformed("select scalar"))?;
                if values
                    .get(&select.condition(context))
                    .is_none_or(|ty| !ty.is_bool())
                    || values.get(&select.when_true(context)) != Some(&scalar)
                    || values.get(&select.when_false(context)) != Some(&scalar)
                {
                    return Err(Error::Unsupported("select domain or operands"));
                }
                // All operands are already defined, nontrapping SSA values.
                // This is selection, never an assumed CFG guard or skipped read.
                Some(scalar)
            } else if let Some(root) = op.downcast_ref::<SemanticTypedExpressionRootOp>() {
                let scalar = *values
                    .get(&root.expression(context))
                    .ok_or(Error::Unsupported("undefined root operand"))?;
                let expected = if scalar.is_float() {
                    SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits
                } else {
                    SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence
                };
                if root.policy(context) != Some(expected) {
                    return Err(Error::Unsupported("value root policy"));
                }
                Some(scalar)
            } else {
                return Err(Error::Unsupported("value operation"));
            };
            let scalar = scalar.ok_or(Error::Unsupported("undefined scalar"))?;
            if values.insert(raw.get_result(0), scalar).is_some() {
                return Err(Error::Malformed("duplicate value definition"));
            }
        }
    }
    // The caller's structural-identity check invokes native SSA verification.
    // A topological visit alone is not a proof that a definition dominates use.
    Ok(())
}
