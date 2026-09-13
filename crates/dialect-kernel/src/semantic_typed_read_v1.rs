//! Scalar reads with real SSA memory operands, not free expression symbols.
//!
//! Local verification checks representation only. Bounds, provenance, memory
//! stability, and equality with another read require independent live analysis.

use std::{collections::BTreeSet, error::Error, fmt};

use pliron::{
    builtin::{
        ATTR_KEY_DEBUG_INFO,
        op_interfaces::{NRegionsInterface, NResultsInterface},
    },
    common_traits::Verify,
    context::Context,
    derive::{pliron_attr, pliron_op},
    op::Op,
    operation::Operation,
    result::Result as PlironResult,
    r#type::Typed,
    value::Value,
    verify_err,
};

use crate::{
    DimensionAttr, MemorySpaceAttr, RankedViewOp, SemanticScalarKindAttr, SemanticScalarType,
    SemanticSymbolAttr, SemanticTypedBinaryOp, SemanticTypedCastOp, SemanticTypedCompareOp,
    SemanticTypedConstantOp, SemanticTypedScalarV1, SemanticTypedSelectOp, SemanticTypedSymbolOp,
    SemanticTypedUnaryOp, is_index_type, ranked_view_type,
};

/// Reserved load identities. An identity is a label, never a scalar variable.
pub const SEMANTIC_TYPED_READ_SYMBOL_BASE_V1: u32 = 0xc000_0000;

#[pliron_attr(name = "kernel.semantic_read_volatility", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SemanticReadVolatilityAttr {
    NonVolatile,
    Volatile,
}

#[pliron_attr(name = "kernel.semantic_read_ordering", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SemanticReadOrderingAttr {
    Unordered,
    Acquire,
    SequentiallyConsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticTypedReadErrorV1 {
    MalformedShape,
    InvalidSymbol,
    InvalidView,
    InvalidIndex,
    ScalarMismatch,
    AddressSpaceMismatch,
    InvalidGuardOrFallback,
    UnsupportedOrdering,
}

impl fmt::Display for SemanticTypedReadErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid typed semantic read: {self:?}")
    }
}

impl Error for SemanticTypedReadErrorV1 {}

/// A read result has one defining memory operation. Optional trailing operands
/// are a boolean guard and exact-typed fallback, in that order. A false guard
/// performs no read and yields the fallback. Volatility is retained even though
/// the initial live binding analysis rejects volatile observations.
#[pliron_op(
    name = "kernel.semantic_typed_read",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_semantic_read_id: SemanticSymbolAttr,
        kernel_semantic_read_scalar_kind: SemanticScalarKindAttr,
        kernel_semantic_read_bit_width: DimensionAttr,
        kernel_semantic_read_space: MemorySpaceAttr,
        kernel_semantic_read_volatility: SemanticReadVolatilityAttr,
        kernel_semantic_read_ordering: SemanticReadOrderingAttr
    )
)]
pub struct SemanticTypedReadOp;

impl SemanticTypedReadOp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context: &mut Context,
        symbol: u32,
        scalar: SemanticTypedScalarV1,
        space: MemorySpaceAttr,
        volatility: SemanticReadVolatilityAttr,
        ordering: SemanticReadOrderingAttr,
        view: Value,
        indices: Vec<Value>,
        guarded: Option<(Value, Value)>,
    ) -> Result<Self, SemanticTypedReadErrorV1> {
        validate_inputs(
            context, symbol, scalar, space, ordering, view, &indices, guarded,
        )?;
        let mut operands =
            Vec::with_capacity(1 + indices.len() + 2 * usize::from(guarded.is_some()));
        operands.push(view);
        operands.extend(indices);
        if let Some((guard, fallback)) = guarded {
            operands.extend([guard, fallback]);
        }
        let result_type = SemanticScalarType::get(context).into();
        let raw = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![result_type],
            operands,
            vec![],
            0,
        );
        let op = Self::from_operation(raw);
        op.set_attr_kernel_semantic_read_id(context, SemanticSymbolAttr(symbol));
        op.set_attr_kernel_semantic_read_scalar_kind(context, scalar.kind());
        op.set_attr_kernel_semantic_read_bit_width(context, DimensionAttr(scalar.bits().into()));
        op.set_attr_kernel_semantic_read_space(context, space);
        op.set_attr_kernel_semantic_read_volatility(context, volatility);
        op.set_attr_kernel_semantic_read_ordering(context, ordering);
        Ok(op)
    }

    pub fn symbol(&self, context: &Context) -> Option<u32> {
        self.get_attr_kernel_semantic_read_id(context).map(|a| a.0)
    }

    pub fn scalar(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        SemanticTypedScalarV1::new(
            *self.get_attr_kernel_semantic_read_scalar_kind(context)?,
            u16::try_from(self.get_attr_kernel_semantic_read_bit_width(context)?.0).ok()?,
        )
    }

    pub fn memory_space(&self, context: &Context) -> Option<MemorySpaceAttr> {
        self.get_attr_kernel_semantic_read_space(context)
            .map(|a| *a)
    }

    pub fn volatility(&self, context: &Context) -> Option<SemanticReadVolatilityAttr> {
        self.get_attr_kernel_semantic_read_volatility(context)
            .map(|a| *a)
    }

    pub fn ordering(&self, context: &Context) -> Option<SemanticReadOrderingAttr> {
        self.get_attr_kernel_semantic_read_ordering(context)
            .map(|a| *a)
    }

    pub fn view(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }

    pub fn indices(&self, context: &Context) -> Option<Vec<Value>> {
        let rank = ranked_view_type(self.view(context), context)?
            .deref(context)
            .rank();
        let raw = self.get_operation().deref(context);
        (raw.get_num_operands() >= 1 + rank)
            .then(|| (1..=rank).map(|i| raw.get_operand(i)).collect())
    }

    pub fn guarded(&self, context: &Context) -> Option<(Value, Value)> {
        let rank = ranked_view_type(self.view(context), context)?
            .deref(context)
            .rank();
        let raw = self.get_operation().deref(context);
        (raw.get_num_operands() == 3 + rank)
            .then(|| (raw.get_operand(1 + rank), raw.get_operand(2 + rank)))
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }

    fn validate(&self, context: &Context) -> Result<(), SemanticTypedReadErrorV1> {
        let raw = self.get_operation().deref(context);
        let keys = raw
            .attributes
            .0
            .keys()
            .filter(|key| *key != &*ATTR_KEY_DEBUG_INFO)
            .map(AsRef::as_ref)
            .collect::<BTreeSet<_>>();
        if keys
            != BTreeSet::from([
                "kernel_semantic_read_id",
                "kernel_semantic_read_scalar_kind",
                "kernel_semantic_read_bit_width",
                "kernel_semantic_read_space",
                "kernel_semantic_read_volatility",
                "kernel_semantic_read_ordering",
            ])
            || raw.get_num_results() != 1
            || raw.get_num_operands() == 0
            || raw.get_num_successors() != 0
            || raw.num_regions() != 0
            || !raw
                .get_result(0)
                .get_type(context)
                .deref(context)
                .is::<SemanticScalarType>()
            || self.volatility(context).is_none()
        {
            return Err(SemanticTypedReadErrorV1::MalformedShape);
        }
        let indices = self
            .indices(context)
            .ok_or(SemanticTypedReadErrorV1::InvalidView)?;
        let guarded = self.guarded(context);
        if raw.get_num_operands() != 1 + indices.len() + 2 * usize::from(guarded.is_some()) {
            return Err(SemanticTypedReadErrorV1::MalformedShape);
        }
        validate_inputs(
            context,
            self.symbol(context)
                .ok_or(SemanticTypedReadErrorV1::InvalidSymbol)?,
            self.scalar(context)
                .ok_or(SemanticTypedReadErrorV1::ScalarMismatch)?,
            self.memory_space(context)
                .ok_or(SemanticTypedReadErrorV1::AddressSpaceMismatch)?,
            self.ordering(context)
                .ok_or(SemanticTypedReadErrorV1::UnsupportedOrdering)?,
            self.view(context),
            &indices,
            guarded,
        )
    }
}

impl Verify for SemanticTypedReadOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        if let Err(error) = self.validate(context) {
            return verify_err!(self.loc(context), error);
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_inputs(
    context: &Context,
    symbol: u32,
    scalar: SemanticTypedScalarV1,
    space: MemorySpaceAttr,
    ordering: SemanticReadOrderingAttr,
    view: Value,
    indices: &[Value],
    guarded: Option<(Value, Value)>,
) -> Result<(), SemanticTypedReadErrorV1> {
    if symbol < SEMANTIC_TYPED_READ_SYMBOL_BASE_V1 {
        return Err(SemanticTypedReadErrorV1::InvalidSymbol);
    }
    if ordering != SemanticReadOrderingAttr::Unordered {
        return Err(SemanticTypedReadErrorV1::UnsupportedOrdering);
    }
    let view_type = ranked_view_type(view, context).ok_or(SemanticTypedReadErrorV1::InvalidView)?;
    let view_type = view_type.deref(context);
    if view_type.rank() != indices.len() {
        return Err(SemanticTypedReadErrorV1::InvalidIndex);
    }
    if view_type.element_width() != u32::from(scalar.bits()) {
        return Err(SemanticTypedReadErrorV1::ScalarMismatch);
    }
    if indices.iter().any(|index| !is_index_type(*index, context)) {
        return Err(SemanticTypedReadErrorV1::InvalidIndex);
    }
    let definition = view
        .defining_op()
        .ok_or(SemanticTypedReadErrorV1::InvalidView)?;
    let definition = Operation::get_op_dyn(definition, context);
    let view_op = definition
        .downcast_ref::<RankedViewOp>()
        .ok_or(SemanticTypedReadErrorV1::InvalidView)?;
    if view_op.result(context) != view || view_op.memory_space(context) != Some(space) {
        return Err(SemanticTypedReadErrorV1::AddressSpaceMismatch);
    }
    if let Some((guard, fallback)) = guarded {
        if operand_scalar(context, guard).is_none_or(|ty| !ty.is_bool())
            || operand_scalar(context, fallback) != Some(scalar)
        {
            return Err(SemanticTypedReadErrorV1::InvalidGuardOrFallback);
        }
    }
    Ok(())
}

// Scalar kinds are carried on typed operations, not on the common PLIRON
// semantic marker type. Do not accept that marker alone as an exact type.
fn operand_scalar(context: &Context, value: Value) -> Option<SemanticTypedScalarV1> {
    if !value
        .get_type(context)
        .deref(context)
        .is::<SemanticScalarType>()
    {
        return None;
    }
    let operation = Operation::get_op_dyn(value.defining_op()?, context);
    if operation.get_operation().deref(context).get_num_results() != 1
        || operation.get_operation().deref(context).get_result(0) != value
    {
        return None;
    }
    if let Some(op) = operation.downcast_ref::<SemanticTypedConstantOp>() {
        op.scalar(context)
    } else if let Some(op) = operation.downcast_ref::<SemanticTypedSymbolOp>() {
        op.scalar(context)
    } else if let Some(op) = operation.downcast_ref::<SemanticTypedUnaryOp>() {
        op.scalar(context)
    } else if let Some(op) = operation.downcast_ref::<SemanticTypedBinaryOp>() {
        op.scalar(context)
    } else if let Some(op) = operation.downcast_ref::<SemanticTypedSelectOp>() {
        op.scalar(context)
    } else if let Some(op) = operation.downcast_ref::<SemanticTypedCastOp>() {
        op.target(context)
    } else if operation.downcast_ref::<SemanticTypedCompareOp>().is_some() {
        SemanticTypedScalarV1::new(SemanticScalarKindAttr::Bool, 1)
    } else if let Some(op) = operation.downcast_ref::<SemanticTypedReadOp>() {
        op.scalar(context)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
