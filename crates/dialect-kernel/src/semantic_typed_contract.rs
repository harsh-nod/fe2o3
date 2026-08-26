//! Closed, workload-neutral typed scalar-expression operations.
//!
//! These operations describe source/MIR operator congruence. They do not
//! claim target-instruction IEEE-754 conformance or source-to-ISA refinement.

use std::collections::BTreeSet;

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
    DimensionAttr, SemanticConstantAttr, SemanticContractError, SemanticExpressionCommitmentAttr,
    SemanticNumericalPolicyAttr, SemanticScalarType, SemanticSymbolAttr,
};

#[pliron_attr(name = "kernel.semantic_scalar_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SemanticScalarKindAttr {
    Bool,
    UnsignedInteger,
    SignedInteger,
    Float,
}

#[pliron_attr(name = "kernel.semantic_unary_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SemanticTypedUnaryKindAttr {
    Not,
    Negate,
}

#[pliron_attr(name = "kernel.semantic_typed_binary_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SemanticTypedBinaryKindAttr {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    BitXor,
    BitAnd,
    BitOr,
    ShiftLeft,
    ShiftRight,
}

#[pliron_attr(name = "kernel.semantic_overflow", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SemanticOverflowAttr {
    Wrapping,
    Checked,
}

#[pliron_attr(name = "kernel.semantic_compare_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SemanticTypedCompareKindAttr {
    Equal,
    LessThan,
    LessOrEqual,
    NotEqual,
    GreaterOrEqual,
    GreaterThan,
}

#[pliron_attr(name = "kernel.semantic_cast_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SemanticTypedCastKindAttr {
    Integer,
    IntegerToFloat,
    FloatToFloat,
    FloatToIntegerSaturating,
}

#[pliron_attr(name = "kernel.semantic_ieee_rounding", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SemanticIeeeRoundingAttr {
    NearestTiesToEven,
    TowardZero,
    TowardPositive,
    TowardNegative,
}

#[pliron_attr(name = "kernel.semantic_exceptional_value", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SemanticExceptionalValueAttr {
    PreserveExactBits,
    CanonicalNan,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticTypedScalarV1 {
    kind: SemanticScalarKindAttr,
    bits: u16,
}

impl SemanticTypedScalarV1 {
    pub const fn new(kind: SemanticScalarKindAttr, bits: u16) -> Option<Self> {
        let supported = match kind {
            SemanticScalarKindAttr::Bool => bits == 1,
            SemanticScalarKindAttr::UnsignedInteger | SemanticScalarKindAttr::SignedInteger => {
                matches!(bits, 8 | 16 | 32 | 64)
            }
            SemanticScalarKindAttr::Float => matches!(bits, 32 | 64),
        };
        if supported {
            Some(Self { kind, bits })
        } else {
            None
        }
    }

    pub const fn kind(self) -> SemanticScalarKindAttr {
        self.kind
    }

    pub const fn bits(self) -> u16 {
        self.bits
    }

    pub const fn is_bool(self) -> bool {
        matches!(self.kind, SemanticScalarKindAttr::Bool)
    }

    pub const fn is_integer(self) -> bool {
        matches!(
            self.kind,
            SemanticScalarKindAttr::UnsignedInteger | SemanticScalarKindAttr::SignedInteger
        )
    }

    pub const fn is_float(self) -> bool {
        matches!(self.kind, SemanticScalarKindAttr::Float)
    }
}

fn semantic_operation<O: Op>(
    context: &mut Context,
    operands: Vec<Value>,
) -> pliron::context::Ptr<Operation> {
    Operation::new(
        context,
        O::get_concrete_op_info(),
        vec![SemanticScalarType::get(context).into()],
        operands,
        vec![],
        0,
    )
}

fn typed_scalar(
    kind: Option<SemanticScalarKindAttr>,
    bits: Option<u32>,
) -> Option<SemanticTypedScalarV1> {
    SemanticTypedScalarV1::new(kind?, u16::try_from(bits?).ok()?)
}

fn is_semantic_scalar(value: Value, context: &Context) -> bool {
    value
        .get_type(context)
        .deref(context)
        .is::<SemanticScalarType>()
}

fn verify_shape(operation: &dyn Op, context: &Context, operands: usize) -> PlironResult<()> {
    let raw = operation.get_operation().deref(context);
    if raw.get_num_operands() != operands
        || raw.get_num_results() != 1
        || raw.get_num_successors() != 0
        || raw.num_regions() != 0
        || !is_semantic_scalar(raw.get_result(0), context)
        || (0..operands).any(|index| !is_semantic_scalar(raw.get_operand(index), context))
    {
        return verify_err!(
            operation.loc(context),
            SemanticContractError::MalformedOperation
        );
    }
    Ok(())
}

fn verify_keys(operation: &dyn Op, context: &Context, expected: &[&str]) -> PlironResult<()> {
    let raw = operation.get_operation().deref(context);
    let actual = raw
        .attributes
        .0
        .keys()
        .filter(|key| *key != &*ATTR_KEY_DEBUG_INFO)
        .map(AsRef::as_ref)
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return verify_err!(
            operation.loc(context),
            SemanticContractError::MalformedOperation
        );
    }
    Ok(())
}

trait ScalarAttributeAccess {
    fn set_scalar_kind(&self, context: &mut Context, kind: SemanticScalarKindAttr);
    fn set_bit_width(&self, context: &mut Context, bits: u32);
    fn scalar_kind(&self, context: &Context) -> Option<SemanticScalarKindAttr>;
    fn bit_width(&self, context: &Context) -> Option<u32>;
}

fn set_scalar<O: ScalarAttributeAccess>(
    operation: &O,
    context: &mut Context,
    scalar: SemanticTypedScalarV1,
) {
    operation.set_scalar_kind(context, scalar.kind());
    operation.set_bit_width(context, scalar.bits().into());
}

fn get_scalar<O: ScalarAttributeAccess>(
    operation: &O,
    context: &Context,
) -> Option<SemanticTypedScalarV1> {
    typed_scalar(operation.scalar_kind(context), operation.bit_width(context))
}

macro_rules! impl_scalar_access {
    ($op:ty, $set_kind:ident, $set_bits:ident, $get_kind:ident, $get_bits:ident) => {
        impl ScalarAttributeAccess for $op {
            fn set_scalar_kind(&self, context: &mut Context, kind: SemanticScalarKindAttr) {
                self.$set_kind(context, kind);
            }
            fn set_bit_width(&self, context: &mut Context, bits: u32) {
                self.$set_bits(context, DimensionAttr(bits));
            }
            fn scalar_kind(&self, context: &Context) -> Option<SemanticScalarKindAttr> {
                self.$get_kind(context).map(|attr| *attr)
            }
            fn bit_width(&self, context: &Context) -> Option<u32> {
                self.$get_bits(context).map(|attr| attr.0)
            }
        }
    };
}

#[pliron_op(
    name = "kernel.semantic_typed_symbol",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_semantic_typed_symbol_id: SemanticSymbolAttr,
        kernel_semantic_typed_symbol_scalar_kind: SemanticScalarKindAttr,
        kernel_semantic_typed_symbol_bit_width: DimensionAttr
    )
)]
pub struct SemanticTypedSymbolOp;

impl SemanticTypedSymbolOp {
    pub fn new(context: &mut Context, symbol: u32, scalar: SemanticTypedScalarV1) -> Self {
        let op = Self::from_operation(semantic_operation::<Self>(context, vec![]));
        op.set_attr_kernel_semantic_typed_symbol_id(context, SemanticSymbolAttr(symbol));
        set_scalar(&op, context, scalar);
        op
    }

    pub fn symbol(&self, context: &Context) -> Option<u32> {
        self.get_attr_kernel_semantic_typed_symbol_id(context)
            .map(|attr| attr.0)
    }

    pub fn scalar(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        get_scalar(self, context)
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl_scalar_access!(
    SemanticTypedSymbolOp,
    set_attr_kernel_semantic_typed_symbol_scalar_kind,
    set_attr_kernel_semantic_typed_symbol_bit_width,
    get_attr_kernel_semantic_typed_symbol_scalar_kind,
    get_attr_kernel_semantic_typed_symbol_bit_width
);

impl Verify for SemanticTypedSymbolOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_shape(self, context, 0)?;
        verify_keys(
            self,
            context,
            &[
                "kernel_semantic_typed_symbol_id",
                "kernel_semantic_typed_symbol_scalar_kind",
                "kernel_semantic_typed_symbol_bit_width",
            ],
        )?;
        if self.symbol(context).is_none() || self.scalar(context).is_none() {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

/// Typed SSA extraction of one ordered component from an authenticated
/// cooperative-tensor result root.
///
/// The operation is target- and workload-neutral. Its result-root attribute is
/// reconciled against the live tensor instruction by the production tensor
/// refinement join; the local verifier only checks closed typed shape.
pub const MAX_TENSOR_RESULT_COMPONENTS_V1: usize = 64;

#[pliron_op(
    name = "kernel.tensor_result_component",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_tensor_result_component_result_root: SemanticExpressionCommitmentAttr,
        kernel_tensor_result_component_ordinal: SemanticSymbolAttr,
        kernel_tensor_result_component_scalar_kind: SemanticScalarKindAttr,
        kernel_tensor_result_component_bit_width: DimensionAttr
    )
)]
pub struct TensorResultComponentOp;

impl TensorResultComponentOp {
    pub fn new(
        context: &mut Context,
        result_root: SemanticExpressionCommitmentAttr,
        component: u32,
        scalar: SemanticTypedScalarV1,
    ) -> Self {
        let op = Self::from_operation(semantic_operation::<Self>(context, vec![]));
        op.set_attr_kernel_tensor_result_component_result_root(context, result_root);
        op.set_attr_kernel_tensor_result_component_ordinal(context, SemanticSymbolAttr(component));
        op.set_attr_kernel_tensor_result_component_scalar_kind(context, scalar.kind());
        op.set_attr_kernel_tensor_result_component_bit_width(
            context,
            DimensionAttr(u32::from(scalar.bits())),
        );
        op
    }

    pub fn result_root(&self, context: &Context) -> Option<[u64; 4]> {
        self.get_attr_kernel_tensor_result_component_result_root(context)
            .map(|attr| attr.words())
    }

    pub fn component(&self, context: &Context) -> Option<u32> {
        self.get_attr_kernel_tensor_result_component_ordinal(context)
            .map(|attr| attr.0)
    }

    pub fn scalar(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        let kind = self
            .get_attr_kernel_tensor_result_component_scalar_kind(context)
            .map(|attr| *attr)?;
        let bits = self
            .get_attr_kernel_tensor_result_component_bit_width(context)
            .map(|attr| attr.0)?;
        SemanticTypedScalarV1::new(kind, u16::try_from(bits).ok()?)
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl Verify for TensorResultComponentOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_shape(self, context, 0)?;
        verify_keys(
            self,
            context,
            &[
                "kernel_tensor_result_component_result_root",
                "kernel_tensor_result_component_ordinal",
                "kernel_tensor_result_component_scalar_kind",
                "kernel_tensor_result_component_bit_width",
            ],
        )?;
        let Some(result_root) = self.get_attr_kernel_tensor_result_component_result_root(context)
        else {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        };
        result_root.verify(context)?;
        if self
            .component(context)
            .is_none_or(|component| component as usize >= MAX_TENSOR_RESULT_COMPONENTS_V1)
            || self.scalar(context).is_none()
        {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_typed_constant",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_semantic_typed_constant_bits: SemanticConstantAttr,
        kernel_semantic_typed_constant_scalar_kind: SemanticScalarKindAttr,
        kernel_semantic_typed_constant_bit_width: DimensionAttr
    )
)]
pub struct SemanticTypedConstantOp;

impl SemanticTypedConstantOp {
    pub fn new(context: &mut Context, bits: u64, scalar: SemanticTypedScalarV1) -> Self {
        let op = Self::from_operation(semantic_operation::<Self>(context, vec![]));
        op.set_attr_kernel_semantic_typed_constant_bits(context, SemanticConstantAttr(bits));
        set_scalar(&op, context, scalar);
        op
    }

    pub fn bits(&self, context: &Context) -> Option<u64> {
        self.get_attr_kernel_semantic_typed_constant_bits(context)
            .map(|attr| attr.0)
    }

    pub fn scalar(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        get_scalar(self, context)
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl_scalar_access!(
    SemanticTypedConstantOp,
    set_attr_kernel_semantic_typed_constant_scalar_kind,
    set_attr_kernel_semantic_typed_constant_bit_width,
    get_attr_kernel_semantic_typed_constant_scalar_kind,
    get_attr_kernel_semantic_typed_constant_bit_width
);

impl Verify for SemanticTypedConstantOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_shape(self, context, 0)?;
        verify_keys(
            self,
            context,
            &[
                "kernel_semantic_typed_constant_bits",
                "kernel_semantic_typed_constant_scalar_kind",
                "kernel_semantic_typed_constant_bit_width",
            ],
        )?;
        let (Some(bits), Some(scalar)) = (self.bits(context), self.scalar(context)) else {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        };
        if scalar.bits() < 64 && bits >= (1_u64 << scalar.bits()) {
            return verify_err!(
                self.loc(context),
                "typed semantic constant exceeds its scalar width"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_typed_unary",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_semantic_unary_kind: SemanticTypedUnaryKindAttr,
        kernel_semantic_typed_unary_scalar_kind: SemanticScalarKindAttr,
        kernel_semantic_typed_unary_bit_width: DimensionAttr
    )
)]
pub struct SemanticTypedUnaryOp;

impl SemanticTypedUnaryOp {
    pub fn new(
        context: &mut Context,
        kind: SemanticTypedUnaryKindAttr,
        scalar: SemanticTypedScalarV1,
        operand: Value,
    ) -> Self {
        let op = Self::from_operation(semantic_operation::<Self>(context, vec![operand]));
        op.set_attr_kernel_semantic_unary_kind(context, kind);
        set_scalar(&op, context, scalar);
        op
    }

    pub fn kind(&self, context: &Context) -> Option<SemanticTypedUnaryKindAttr> {
        self.get_attr_kernel_semantic_unary_kind(context)
            .map(|attr| *attr)
    }

    pub fn scalar(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        get_scalar(self, context)
    }

    pub fn operand(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl_scalar_access!(
    SemanticTypedUnaryOp,
    set_attr_kernel_semantic_typed_unary_scalar_kind,
    set_attr_kernel_semantic_typed_unary_bit_width,
    get_attr_kernel_semantic_typed_unary_scalar_kind,
    get_attr_kernel_semantic_typed_unary_bit_width
);

impl Verify for SemanticTypedUnaryOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_shape(self, context, 1)?;
        verify_keys(
            self,
            context,
            &[
                "kernel_semantic_unary_kind",
                "kernel_semantic_typed_unary_scalar_kind",
                "kernel_semantic_typed_unary_bit_width",
            ],
        )?;
        if self.kind(context).is_none() || self.scalar(context).is_none() {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_typed_binary",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_semantic_typed_binary_kind: SemanticTypedBinaryKindAttr,
        kernel_semantic_overflow: SemanticOverflowAttr,
        kernel_semantic_typed_binary_scalar_kind: SemanticScalarKindAttr,
        kernel_semantic_typed_binary_bit_width: DimensionAttr
    )
)]
pub struct SemanticTypedBinaryOp;

impl SemanticTypedBinaryOp {
    pub fn new(
        context: &mut Context,
        kind: SemanticTypedBinaryKindAttr,
        overflow: SemanticOverflowAttr,
        scalar: SemanticTypedScalarV1,
        lhs: Value,
        rhs: Value,
    ) -> Self {
        let op = Self::from_operation(semantic_operation::<Self>(context, vec![lhs, rhs]));
        op.set_attr_kernel_semantic_typed_binary_kind(context, kind);
        op.set_attr_kernel_semantic_overflow(context, overflow);
        set_scalar(&op, context, scalar);
        op
    }

    pub fn kind(&self, context: &Context) -> Option<SemanticTypedBinaryKindAttr> {
        self.get_attr_kernel_semantic_typed_binary_kind(context)
            .map(|attr| *attr)
    }

    pub fn overflow(&self, context: &Context) -> Option<SemanticOverflowAttr> {
        self.get_attr_kernel_semantic_overflow(context)
            .map(|attr| *attr)
    }

    pub fn scalar(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        get_scalar(self, context)
    }

    pub fn lhs(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }

    pub fn rhs(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(1)
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl_scalar_access!(
    SemanticTypedBinaryOp,
    set_attr_kernel_semantic_typed_binary_scalar_kind,
    set_attr_kernel_semantic_typed_binary_bit_width,
    get_attr_kernel_semantic_typed_binary_scalar_kind,
    get_attr_kernel_semantic_typed_binary_bit_width
);

impl Verify for SemanticTypedBinaryOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_shape(self, context, 2)?;
        verify_keys(
            self,
            context,
            &[
                "kernel_semantic_typed_binary_kind",
                "kernel_semantic_overflow",
                "kernel_semantic_typed_binary_scalar_kind",
                "kernel_semantic_typed_binary_bit_width",
            ],
        )?;
        if self.kind(context).is_none()
            || self.overflow(context).is_none()
            || self.scalar(context).is_none()
        {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_typed_compare",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_semantic_compare_kind: SemanticTypedCompareKindAttr,
        kernel_semantic_typed_compare_scalar_kind: SemanticScalarKindAttr,
        kernel_semantic_typed_compare_bit_width: DimensionAttr
    )
)]
pub struct SemanticTypedCompareOp;

impl SemanticTypedCompareOp {
    pub fn new(
        context: &mut Context,
        kind: SemanticTypedCompareKindAttr,
        operand_scalar: SemanticTypedScalarV1,
        lhs: Value,
        rhs: Value,
    ) -> Self {
        let op = Self::from_operation(semantic_operation::<Self>(context, vec![lhs, rhs]));
        op.set_attr_kernel_semantic_compare_kind(context, kind);
        set_scalar(&op, context, operand_scalar);
        op
    }

    pub fn kind(&self, context: &Context) -> Option<SemanticTypedCompareKindAttr> {
        self.get_attr_kernel_semantic_compare_kind(context)
            .map(|attr| *attr)
    }

    pub fn operand_scalar(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        get_scalar(self, context)
    }

    pub fn lhs(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }
    pub fn rhs(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(1)
    }
    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl_scalar_access!(
    SemanticTypedCompareOp,
    set_attr_kernel_semantic_typed_compare_scalar_kind,
    set_attr_kernel_semantic_typed_compare_bit_width,
    get_attr_kernel_semantic_typed_compare_scalar_kind,
    get_attr_kernel_semantic_typed_compare_bit_width
);

impl Verify for SemanticTypedCompareOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_shape(self, context, 2)?;
        verify_keys(
            self,
            context,
            &[
                "kernel_semantic_compare_kind",
                "kernel_semantic_typed_compare_scalar_kind",
                "kernel_semantic_typed_compare_bit_width",
            ],
        )?;
        if self.kind(context).is_none() || self.operand_scalar(context).is_none() {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_typed_select",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_semantic_typed_select_scalar_kind: SemanticScalarKindAttr,
        kernel_semantic_typed_select_bit_width: DimensionAttr
    )
)]
pub struct SemanticTypedSelectOp;

impl SemanticTypedSelectOp {
    pub fn new(
        context: &mut Context,
        scalar: SemanticTypedScalarV1,
        condition: Value,
        when_true: Value,
        when_false: Value,
    ) -> Self {
        let op = Self::from_operation(semantic_operation::<Self>(
            context,
            vec![condition, when_true, when_false],
        ));
        set_scalar(&op, context, scalar);
        op
    }

    pub fn scalar(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        get_scalar(self, context)
    }
    pub fn condition(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }
    pub fn when_true(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(1)
    }
    pub fn when_false(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(2)
    }
    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl_scalar_access!(
    SemanticTypedSelectOp,
    set_attr_kernel_semantic_typed_select_scalar_kind,
    set_attr_kernel_semantic_typed_select_bit_width,
    get_attr_kernel_semantic_typed_select_scalar_kind,
    get_attr_kernel_semantic_typed_select_bit_width
);

impl Verify for SemanticTypedSelectOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_shape(self, context, 3)?;
        verify_keys(
            self,
            context,
            &[
                "kernel_semantic_typed_select_scalar_kind",
                "kernel_semantic_typed_select_bit_width",
            ],
        )?;
        if self.scalar(context).is_none() {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_typed_cast",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_semantic_cast_kind: SemanticTypedCastKindAttr,
        kernel_semantic_source_scalar_kind: SemanticScalarKindAttr,
        kernel_semantic_source_bit_width: DimensionAttr,
        kernel_semantic_target_scalar_kind: SemanticScalarKindAttr,
        kernel_semantic_target_bit_width: DimensionAttr
    )
)]
pub struct SemanticTypedCastOp;

impl SemanticTypedCastOp {
    pub fn new(
        context: &mut Context,
        kind: SemanticTypedCastKindAttr,
        source: SemanticTypedScalarV1,
        target: SemanticTypedScalarV1,
        operand: Value,
    ) -> Self {
        let op = Self::from_operation(semantic_operation::<Self>(context, vec![operand]));
        op.set_attr_kernel_semantic_cast_kind(context, kind);
        op.set_attr_kernel_semantic_source_scalar_kind(context, source.kind());
        op.set_attr_kernel_semantic_source_bit_width(context, DimensionAttr(source.bits().into()));
        op.set_attr_kernel_semantic_target_scalar_kind(context, target.kind());
        op.set_attr_kernel_semantic_target_bit_width(context, DimensionAttr(target.bits().into()));
        op
    }

    pub fn kind(&self, context: &Context) -> Option<SemanticTypedCastKindAttr> {
        self.get_attr_kernel_semantic_cast_kind(context)
            .map(|attr| *attr)
    }
    pub fn source(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        typed_scalar(
            self.get_attr_kernel_semantic_source_scalar_kind(context)
                .map(|attr| *attr),
            self.get_attr_kernel_semantic_source_bit_width(context)
                .map(|attr| attr.0),
        )
    }
    pub fn target(&self, context: &Context) -> Option<SemanticTypedScalarV1> {
        typed_scalar(
            self.get_attr_kernel_semantic_target_scalar_kind(context)
                .map(|attr| *attr),
            self.get_attr_kernel_semantic_target_bit_width(context)
                .map(|attr| attr.0),
        )
    }
    pub fn operand(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }
    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl Verify for SemanticTypedCastOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_shape(self, context, 1)?;
        verify_keys(
            self,
            context,
            &[
                "kernel_semantic_cast_kind",
                "kernel_semantic_source_scalar_kind",
                "kernel_semantic_source_bit_width",
                "kernel_semantic_target_scalar_kind",
                "kernel_semantic_target_bit_width",
            ],
        )?;
        if self.kind(context).is_none()
            || self.source(context).is_none()
            || self.target(context).is_none()
        {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_typed_root",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (
        kernel_semantic_numerical_policy: SemanticNumericalPolicyAttr,
        kernel_semantic_ieee_rounding: SemanticIeeeRoundingAttr,
        kernel_semantic_exceptional_value: SemanticExceptionalValueAttr,
        kernel_semantic_typed_root_commitment: SemanticExpressionCommitmentAttr
    )
)]
pub struct SemanticTypedExpressionRootOp;

impl SemanticTypedExpressionRootOp {
    pub fn new(
        context: &mut Context,
        expression: Value,
        policy: SemanticNumericalPolicyAttr,
        rounding: SemanticIeeeRoundingAttr,
        exceptional_values: SemanticExceptionalValueAttr,
        commitment: [u64; 4],
    ) -> Self {
        let op = Self::from_operation(semantic_operation::<Self>(context, vec![expression]));
        op.set_attr_kernel_semantic_numerical_policy(context, policy);
        op.set_attr_kernel_semantic_ieee_rounding(context, rounding);
        op.set_attr_kernel_semantic_exceptional_value(context, exceptional_values);
        op.set_attr_kernel_semantic_typed_root_commitment(
            context,
            SemanticExpressionCommitmentAttr::new(commitment),
        );
        op
    }

    pub fn expression(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }
    pub fn policy(&self, context: &Context) -> Option<SemanticNumericalPolicyAttr> {
        self.get_attr_kernel_semantic_numerical_policy(context)
            .map(|attr| *attr)
    }
    pub fn rounding(&self, context: &Context) -> Option<SemanticIeeeRoundingAttr> {
        self.get_attr_kernel_semantic_ieee_rounding(context)
            .map(|attr| *attr)
    }
    pub fn exceptional_values(&self, context: &Context) -> Option<SemanticExceptionalValueAttr> {
        self.get_attr_kernel_semantic_exceptional_value(context)
            .map(|attr| *attr)
    }
    pub fn commitment(&self, context: &Context) -> Option<[u64; 4]> {
        self.get_attr_kernel_semantic_typed_root_commitment(context)
            .map(|attr| attr.words())
    }
    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl Verify for SemanticTypedExpressionRootOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_shape(self, context, 1)?;
        verify_keys(
            self,
            context,
            &[
                "kernel_semantic_numerical_policy",
                "kernel_semantic_ieee_rounding",
                "kernel_semantic_exceptional_value",
                "kernel_semantic_typed_root_commitment",
            ],
        )?;
        let (Some(_policy), Some(rounding), Some(exceptional_values), Some(_)) = (
            self.policy(context),
            self.rounding(context),
            self.exceptional_values(context),
            self.commitment(context),
        ) else {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        };
        if rounding != SemanticIeeeRoundingAttr::NearestTiesToEven
            || exceptional_values != SemanticExceptionalValueAttr::PreserveExactBits
        {
            return verify_err!(
                self.loc(context),
                "typed semantic root requires the canonical exact numerical-policy fields"
            );
        }
        Ok(())
    }
}
