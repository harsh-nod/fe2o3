//! Executable scalar, control-flow, and memory operations for the target-neutral GPU dialect.
//!
//! The interfaces in this module are deliberately conservative. An operation is
//! declared side-effect-free only when it cannot trap, synchronize, call unknown
//! code, or access memory. Missing optimization information therefore retains the
//! operation.

use std::{boxed::Box, num::NonZero};

use pliron::{
    attribute::{AttrObj, attr_cast},
    basic_block::BasicBlock,
    builtin::{
        attr_interfaces::{MaterializableAttr, TypedAttrInterface},
        attributes::{IntegerAttr, OperandSegmentSizesAttr, StringAttr, TypeAttr},
        op_interfaces::{
            ATTR_KEY_OPERAND_SEGMENT_SIZES, BranchOpInterface, IsTerminatorInterface,
            NOpdsInterface, NRegionsInterface, NResultsInterface, NSuccsInterface, OneOpdInterface,
            OneResultInterface, OneSuccInterface, OperandSegmentInterface,
        },
        ops::FuncOp,
        type_interfaces::FunctionTypeInterface,
        types::{FP16Type, FP32Type, FP64Type, FunctionType, IntegerType, Signedness},
    },
    common_traits::Verify,
    context::{Context, Ptr},
    derive::{attr_interface_impl, op_interface_impl, pliron_attr, pliron_op, pliron_type},
    irbuild::{
        IRStatus,
        match_rewrite::{MatchRewrite, MatchRewriter},
        rewriter::Rewriter,
    },
    op::Op,
    operation::Operation,
    opts::{
        constants::{BranchOpFoldInterface, ConstFoldInterface},
        dce::SideEffects,
    },
    result::Result,
    r#type::{TypeHandle, Typed},
    utils::apint::{APInt, bw},
    value::Value,
    verify_err,
};

use crate::{AddressSpaceAttr, TargetNeutralGpuOpInterface};

/// Access permitted through a pointer or slice.
#[pliron_attr(name = "gpu.access_mode", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AccessModeAttr {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

/// Target-neutral unary scalar operation.
#[pliron_attr(name = "gpu.unary_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnaryKindAttr {
    Negate,
    Not,
}

/// Target-neutral binary scalar operation.
#[pliron_attr(name = "gpu.binary_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BinaryKindAttr {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    CheckedAdd,
    CheckedSubtract,
    CheckedMultiply,
}

impl BinaryKindAttr {
    fn is_checked(self) -> bool {
        matches!(
            self,
            Self::CheckedAdd | Self::CheckedSubtract | Self::CheckedMultiply
        )
    }
}

/// Comparison predicate. Signedness is taken from the operand type.
#[pliron_attr(name = "gpu.compare_predicate", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ComparePredicateAttr {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// Target-neutral scalar cast semantics.
#[pliron_attr(name = "gpu.cast_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CastKindAttr {
    RestrictPointerAccess,
    Truncate,
    ZeroExtend,
    SignExtend,
    FloatExtend,
    FloatTruncate,
    IntegerToFloat,
    FloatToInteger,
    Bitcast,
}

/// Canonical Kernel IR operation families preserved opaquely by the generic
/// optimizing middle end.
///
/// The operation's SSA operands and result types remain first-class Pliron
/// values. The family tag prevents one preserved semantic operation from being
/// mistaken for another, while the bridge-owned origin table retains the exact
/// versioned payload. These operations are intentionally effectful until a
/// dedicated executable dialect operation is implemented for the family.
#[pliron_attr(name = "gpu.preserved_operation_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PreservedOperationKindAttr {
    Intrinsic,
    MemoryIntrinsic,
    Alloca,
    GuardedLoad,
    GuardedStore,
    Barrier,
    Atomic,
    Fence,
    WorkgroupBarrier,
    WorkgroupMemory,
    Matrix,
    Gfx950LdsTranspose,
    Wave,
    InlineAssembly,
}

/// Canonical Kernel IR terminators whose payload is retained by the bridge.
#[pliron_attr(name = "gpu.preserved_terminator_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PreservedTerminatorKindAttr {
    Switch,
    IntegerSwitch,
    Unreachable,
}

/// Required alignment for a memory operation.
#[pliron_attr(name = "gpu.memory_alignment", format = "$0", verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MemoryAlignmentAttr(pub u32);

/// Whether a memory operation is volatile.
#[pliron_attr(name = "gpu.volatile", format = "$0", verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VolatileAttr(pub bool);

/// Pointer-sized integer. Its concrete width is intentionally target-dependent.
#[pliron_type(name = "gpu.index", format, generate_get = true, verifier = "succ")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct IndexType;

/// IEEE bfloat16 scalar type.
#[pliron_type(name = "gpu.bf16", format, generate_get = true, verifier = "succ")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BFloat16Type;

/// Typed pointer retaining pointee, address-space, and access semantics.
#[pliron_type(
    name = "gpu.pointer",
    format = "`<` $pointee `,` $address_space `,` $access `>`",
    generate_get = true,
    verifier = "succ"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PointerType {
    pointee: TypeHandle,
    address_space: AddressSpaceAttr,
    access: AccessModeAttr,
}

impl PointerType {
    pub fn pointee(&self) -> TypeHandle {
        self.pointee
    }

    pub const fn address_space(&self) -> AddressSpaceAttr {
        self.address_space
    }

    pub const fn access(&self) -> AccessModeAttr {
        self.access
    }
}

/// Fat slice retaining element, address-space, and access semantics.
#[pliron_type(
    name = "gpu.slice",
    format = "`<` $element `,` $address_space `,` $access `>`",
    generate_get = true,
    verifier = "succ"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SliceType {
    element: TypeHandle,
    address_space: AddressSpaceAttr,
    access: AccessModeAttr,
}

impl SliceType {
    pub fn element(&self) -> TypeHandle {
        self.element
    }

    pub const fn address_space(&self) -> AddressSpaceAttr {
        self.address_space
    }

    pub const fn access(&self) -> AccessModeAttr {
        self.access
    }
}

/// Exact constant for a target-sized index value.
#[pliron_attr(name = "gpu.index_value", format = "$0", verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IndexAttr(pub u64);

#[attr_interface_impl]
impl TypedAttrInterface for IndexAttr {
    fn get_type(&self, ctx: &Context) -> TypeHandle {
        IndexType::get(ctx).into()
    }
}

/// Exact bfloat16 bit pattern. No host floating-point conversion is performed.
#[pliron_attr(name = "gpu.bf16_value", format = "$0", verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BFloat16Attr(pub u16);

#[attr_interface_impl]
impl TypedAttrInterface for BFloat16Attr {
    fn get_type(&self, ctx: &Context) -> TypeHandle {
        BFloat16Type::get(ctx).into()
    }
}

fn verify_shape(op: &dyn Op, ctx: &Context, operands: usize, results: usize) -> Result<()> {
    let operation = op.get_operation().deref(ctx);
    if operation.get_num_operands() != operands
        || operation.get_num_results() != results
        || operation.get_num_successors() != 0
        || operation.num_regions() != 0
    {
        return verify_err!(
            op.loc(ctx),
            "{} requires {operands} operands, {results} results, no successors, and no regions",
            op.get_opid()
        );
    }
    Ok(())
}

fn is_integer(ctx: &Context, ty: TypeHandle) -> bool {
    ty.deref(ctx).is::<IndexType>()
        || ty
            .deref(ctx)
            .downcast_ref::<IntegerType>()
            .is_some_and(|integer| integer.width() != 1)
}

fn is_float(ctx: &Context, ty: TypeHandle) -> bool {
    let ty = ty.deref(ctx);
    ty.is::<FP16Type>() || ty.is::<FP32Type>() || ty.is::<FP64Type>() || ty.is::<BFloat16Type>()
}

fn is_scalar(ctx: &Context, ty: TypeHandle) -> bool {
    is_bool(ctx, ty) || is_integer(ctx, ty) || is_float(ctx, ty)
}

fn is_numeric(ctx: &Context, ty: TypeHandle) -> bool {
    is_integer(ctx, ty) || is_float(ctx, ty)
}

fn is_bool(ctx: &Context, ty: TypeHandle) -> bool {
    ty.deref(ctx)
        .downcast_ref::<IntegerType>()
        .is_some_and(|integer| integer.width() == 1 && integer.signedness() == Signedness::Signless)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarCastTypeV1 {
    Bool,
    SignedInteger(u32),
    UnsignedInteger(u32),
    Index,
    Float(u32),
}

impl ScalarCastTypeV1 {
    const fn is_integer(self) -> bool {
        matches!(
            self,
            Self::SignedInteger(_) | Self::UnsignedInteger(_) | Self::Index
        )
    }

    const fn is_signed_integer(self) -> bool {
        matches!(self, Self::SignedInteger(_))
    }

    const fn is_float(self) -> bool {
        matches!(self, Self::Float(_))
    }

    const fn is_numeric(self) -> bool {
        self.is_integer() || self.is_float()
    }

    const fn bit_width(self) -> Option<u32> {
        match self {
            Self::Bool => Some(1),
            Self::SignedInteger(width) | Self::UnsignedInteger(width) | Self::Float(width) => {
                Some(width)
            }
            Self::Index => None,
        }
    }
}

fn scalar_cast_type_v1(ctx: &Context, ty: TypeHandle) -> Option<ScalarCastTypeV1> {
    let raw = ty.deref(ctx);
    if let Some(integer) = raw.downcast_ref::<IntegerType>() {
        return match (integer.width(), integer.signedness()) {
            (1, Signedness::Signless) => Some(ScalarCastTypeV1::Bool),
            (width @ (8 | 16 | 32 | 64 | 128), Signedness::Signed) => {
                Some(ScalarCastTypeV1::SignedInteger(width))
            }
            (width @ (8 | 16 | 32 | 64 | 128), Signedness::Unsigned) => {
                Some(ScalarCastTypeV1::UnsignedInteger(width))
            }
            _ => None,
        };
    }
    if raw.is::<IndexType>() {
        return Some(ScalarCastTypeV1::Index);
    }
    if raw.is::<FP16Type>() || raw.is::<BFloat16Type>() {
        return Some(ScalarCastTypeV1::Float(16));
    }
    if raw.is::<FP32Type>() {
        return Some(ScalarCastTypeV1::Float(32));
    }
    raw.is::<FP64Type>().then_some(ScalarCastTypeV1::Float(64))
}

fn valid_integer_cast_v1(kind: CastKindAttr, from: ScalarCastTypeV1, to: ScalarCastTypeV1) -> bool {
    if from == to {
        return false;
    }
    match (from, to) {
        (ScalarCastTypeV1::UnsignedInteger(32), ScalarCastTypeV1::Index) => {
            return kind == CastKindAttr::ZeroExtend;
        }
        (ScalarCastTypeV1::UnsignedInteger(64), ScalarCastTypeV1::Index)
        | (ScalarCastTypeV1::Index, ScalarCastTypeV1::UnsignedInteger(64)) => {
            return kind == CastKindAttr::Bitcast;
        }
        (ScalarCastTypeV1::Index, _) | (_, ScalarCastTypeV1::Index) => return false,
        _ => {}
    }

    let (Some(from_width), Some(to_width)) = (from.bit_width(), to.bit_width()) else {
        return false;
    };
    let expected = match from_width.cmp(&to_width) {
        std::cmp::Ordering::Greater => CastKindAttr::Truncate,
        std::cmp::Ordering::Less if from.is_signed_integer() => CastKindAttr::SignExtend,
        std::cmp::Ordering::Less => CastKindAttr::ZeroExtend,
        std::cmp::Ordering::Equal => CastKindAttr::Bitcast,
    };
    kind == expected
}

fn valid_scalar_cast_v1(
    ctx: &Context,
    kind: CastKindAttr,
    from_type: TypeHandle,
    to_type: TypeHandle,
) -> bool {
    let (Some(from), Some(to)) = (
        scalar_cast_type_v1(ctx, from_type),
        scalar_cast_type_v1(ctx, to_type),
    ) else {
        return false;
    };
    if (from.is_integer() || from == ScalarCastTypeV1::Bool) && to.is_integer() {
        return valid_integer_cast_v1(kind, from, to);
    }
    if from == ScalarCastTypeV1::Index || to == ScalarCastTypeV1::Index {
        return false;
    }

    let (Some(from_width), Some(to_width)) = (from.bit_width(), to.bit_width()) else {
        return false;
    };
    match kind {
        CastKindAttr::RestrictPointerAccess => false,
        CastKindAttr::Truncate => from.is_integer() && to.is_integer() && from_width > to_width,
        CastKindAttr::ZeroExtend => {
            (from == ScalarCastTypeV1::Bool || (from.is_integer() && !from.is_signed_integer()))
                && to.is_integer()
                && from_width < to_width
        }
        CastKindAttr::SignExtend => {
            from.is_signed_integer() && to.is_integer() && from_width < to_width
        }
        CastKindAttr::FloatExtend => from.is_float() && to.is_float() && from_width < to_width,
        CastKindAttr::FloatTruncate => from.is_float() && to.is_float() && from_width > to_width,
        CastKindAttr::IntegerToFloat => from.is_integer() && to.is_float(),
        CastKindAttr::FloatToInteger => from.is_float() && to.is_integer(),
        CastKindAttr::Bitcast => {
            from.is_numeric() && to.is_numeric() && from_type != to_type && from_width == to_width
        }
    }
}

fn same_operand_types(op: &dyn Op, ctx: &Context) -> bool {
    let operation = op.get_operation().deref(ctx);
    operation.get_num_operands() < 2
        || operation
            .operands()
            .map(|value| value.get_type(ctx))
            .all(|ty| ty == operation.get_operand(0).get_type(ctx))
}

/// A typed target-neutral constant.
#[pliron_op(
    name = "gpu.constant",
    format = "`<` $gpu_constant_value `>` ` : ` type($0)",
    interfaces = [TargetNeutralGpuOpInterface, NOpdsInterface<0>, OneResultInterface, NRegionsInterface<0>],
    attributes = (gpu_constant_value)
)]
pub struct ConstantOp;

impl ConstantOp {
    pub fn new(ctx: &mut Context, value: AttrObj) -> Self {
        let result_type = attr_cast::<dyn TypedAttrInterface>(&*value)
            .expect("gpu.constant value must implement TypedAttrInterface")
            .get_type(ctx);
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_type],
            vec![],
            vec![],
            0,
        );
        let constant = Self { op };
        constant.set_attr_gpu_constant_value(ctx, value);
        constant
    }

    pub fn value(&self, ctx: &Context) -> AttrObj {
        self.get_attr_gpu_constant_value(ctx)
            .expect("verified gpu.constant has a value")
            .clone()
    }

    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

impl Verify for ConstantOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        verify_shape(self, ctx, 0, 1)?;
        let Some(value) = self.get_attr_gpu_constant_value(ctx) else {
            return verify_err!(self.loc(ctx), "gpu.constant requires a value attribute");
        };
        let Some(typed) = attr_cast::<dyn TypedAttrInterface>(&**value) else {
            return verify_err!(self.loc(ctx), "gpu.constant value must be typed");
        };
        if typed.get_type(ctx) != self.result(ctx).get_type(ctx)
            || !is_scalar(ctx, typed.get_type(ctx))
        {
            return verify_err!(self.loc(ctx), "gpu.constant value and result types differ");
        }
        Ok(())
    }
}

#[attr_interface_impl]
impl MaterializableAttr for IndexAttr {
    fn materialize(&self, ctx: &mut Context) -> Ptr<Operation> {
        ConstantOp::new(ctx, Box::new(*self)).get_operation()
    }
}

#[attr_interface_impl]
impl MaterializableAttr for BFloat16Attr {
    fn materialize(&self, ctx: &mut Context) -> Ptr<Operation> {
        ConstantOp::new(ctx, Box::new(*self)).get_operation()
    }
}

#[pliron_op(
    name = "gpu.unary",
    format,
    interfaces = [TargetNeutralGpuOpInterface, OneOpdInterface, OneResultInterface, NRegionsInterface<0>],
    operands = (operand),
    attributes = (gpu_unary_kind: UnaryKindAttr)
)]
pub struct UnaryOp;

impl UnaryOp {
    pub fn new(ctx: &mut Context, kind: UnaryKindAttr, operand: Value) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![operand.get_type(ctx)],
            vec![operand],
            vec![],
            0,
        );
        let unary = Self { op };
        unary.set_attr_gpu_unary_kind(ctx, kind);
        unary
    }

    pub fn kind(&self, ctx: &Context) -> Option<UnaryKindAttr> {
        self.get_attr_gpu_unary_kind(ctx).map(|kind| *kind)
    }

    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

impl Verify for UnaryOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        verify_shape(self, ctx, 1, 1)?;
        let Some(kind) = self.kind(ctx) else {
            return verify_err!(self.loc(ctx), "gpu.unary requires a kind");
        };
        let operand_ty = self.get_operand_operand(ctx).get_type(ctx);
        if operand_ty != self.result(ctx).get_type(ctx) {
            return verify_err!(self.loc(ctx), "gpu.unary operand and result types differ");
        }
        let valid = match kind {
            UnaryKindAttr::Not => is_bool(ctx, operand_ty) || is_integer(ctx, operand_ty),
            UnaryKindAttr::Negate => {
                is_float(ctx, operand_ty)
                    || operand_ty
                        .deref(ctx)
                        .downcast_ref::<IntegerType>()
                        .is_some_and(IntegerType::is_signed)
            }
        };
        if !valid {
            return verify_err!(
                self.loc(ctx),
                "gpu.unary kind does not accept its operand type"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "gpu.binary",
    format,
    interfaces = [TargetNeutralGpuOpInterface, NOpdsInterface<2>, NRegionsInterface<0>],
    operands = (lhs, rhs),
    attributes = (gpu_binary_kind: BinaryKindAttr)
)]
pub struct BinaryOp;

impl BinaryOp {
    pub fn new(ctx: &mut Context, kind: BinaryKindAttr, lhs: Value, rhs: Value) -> Self {
        let mut result_types = vec![lhs.get_type(ctx)];
        if kind.is_checked() {
            result_types.push(IntegerType::get(ctx, 1, Signedness::Signless).into());
        }
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            result_types,
            vec![lhs, rhs],
            vec![],
            0,
        );
        let binary = Self { op };
        binary.set_attr_gpu_binary_kind(ctx, kind);
        binary
    }

    pub fn kind(&self, ctx: &Context) -> Option<BinaryKindAttr> {
        self.get_attr_gpu_binary_kind(ctx).map(|kind| *kind)
    }

    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    pub fn overflow(&self, ctx: &Context) -> Option<Value> {
        (self.get_operation().deref(ctx).get_num_results() == 2)
            .then(|| self.get_operation().deref(ctx).get_result(1))
    }
}

impl Verify for BinaryOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let Some(kind) = self.kind(ctx) else {
            return verify_err!(self.loc(ctx), "gpu.binary requires a kind");
        };
        let expected_results = if kind.is_checked() { 2 } else { 1 };
        verify_shape(self, ctx, 2, expected_results)?;
        if !same_operand_types(self, ctx)
            && !matches!(kind, BinaryKindAttr::ShiftLeft | BinaryKindAttr::ShiftRight)
        {
            return verify_err!(self.loc(ctx), "gpu.binary operand types differ");
        }
        let lhs_ty = self.get_operand_lhs(ctx).get_type(ctx);
        if lhs_ty != self.result(ctx).get_type(ctx) {
            return verify_err!(self.loc(ctx), "gpu.binary lhs and result types differ");
        }
        if !is_scalar(ctx, lhs_ty) {
            return verify_err!(self.loc(ctx), "gpu.binary requires scalar operands");
        }
        let integer_only = matches!(
            kind,
            BinaryKindAttr::BitAnd
                | BinaryKindAttr::BitOr
                | BinaryKindAttr::BitXor
                | BinaryKindAttr::ShiftLeft
                | BinaryKindAttr::ShiftRight
                | BinaryKindAttr::CheckedAdd
                | BinaryKindAttr::CheckedSubtract
                | BinaryKindAttr::CheckedMultiply
        );
        let arithmetic = matches!(
            kind,
            BinaryKindAttr::Add
                | BinaryKindAttr::Subtract
                | BinaryKindAttr::Multiply
                | BinaryKindAttr::Divide
                | BinaryKindAttr::Remainder
        );
        let shift = matches!(kind, BinaryKindAttr::ShiftLeft | BinaryKindAttr::ShiftRight);
        if (integer_only && !(is_integer(ctx, lhs_ty) || is_bool(ctx, lhs_ty)))
            || (arithmetic && !is_numeric(ctx, lhs_ty))
            || ((kind.is_checked() || shift) && !is_integer(ctx, lhs_ty))
        {
            return verify_err!(self.loc(ctx), "gpu.binary kind requires integer operands");
        }
        if matches!(kind, BinaryKindAttr::ShiftLeft | BinaryKindAttr::ShiftRight)
            && !is_integer(ctx, self.get_operand_rhs(ctx).get_type(ctx))
        {
            return verify_err!(self.loc(ctx), "gpu.binary shift amount must be integer");
        }
        if kind.is_checked()
            && (!is_integer(ctx, lhs_ty)
                || !is_bool(ctx, self.overflow(ctx).unwrap().get_type(ctx)))
        {
            return verify_err!(
                self.loc(ctx),
                "checked gpu.binary requires integer value and i1 overflow"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "gpu.compare",
    format,
    interfaces = [TargetNeutralGpuOpInterface, NOpdsInterface<2>, OneResultInterface, NRegionsInterface<0>],
    operands = (lhs, rhs),
    attributes = (gpu_compare_predicate: ComparePredicateAttr)
)]
pub struct CompareOp;

impl CompareOp {
    pub fn new(ctx: &mut Context, predicate: ComparePredicateAttr, lhs: Value, rhs: Value) -> Self {
        let bool_ty = IntegerType::get(ctx, 1, Signedness::Signless).into();
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![bool_ty],
            vec![lhs, rhs],
            vec![],
            0,
        );
        let compare = Self { op };
        compare.set_attr_gpu_compare_predicate(ctx, predicate);
        compare
    }

    pub fn predicate(&self, ctx: &Context) -> Option<ComparePredicateAttr> {
        self.get_attr_gpu_compare_predicate(ctx)
            .map(|predicate| *predicate)
    }

    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

impl Verify for CompareOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        verify_shape(self, ctx, 2, 1)?;
        if self.predicate(ctx).is_none() {
            return verify_err!(self.loc(ctx), "gpu.compare requires a predicate");
        }
        let operand_type = self.get_operand_lhs(ctx).get_type(ctx);
        let bool_comparison = is_bool(ctx, operand_type)
            && matches!(
                self.predicate(ctx),
                Some(ComparePredicateAttr::Equal | ComparePredicateAttr::NotEqual)
            );
        if !same_operand_types(self, ctx)
            || (!is_scalar(ctx, operand_type) && !bool_comparison)
            || (is_bool(ctx, operand_type) && !bool_comparison)
            || !is_bool(ctx, self.result(ctx).get_type(ctx))
        {
            return verify_err!(
                self.loc(ctx),
                "gpu.compare requires equal operands and an i1 result"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "gpu.cast",
    format,
    interfaces = [TargetNeutralGpuOpInterface, OneOpdInterface, OneResultInterface, NRegionsInterface<0>],
    operands = (value),
    attributes = (gpu_cast_kind: CastKindAttr)
)]
pub struct CastOp;

impl CastOp {
    pub fn new(ctx: &mut Context, kind: CastKindAttr, value: Value, to: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![to],
            vec![value],
            vec![],
            0,
        );
        let cast = Self { op };
        cast.set_attr_gpu_cast_kind(ctx, kind);
        cast
    }

    pub fn kind(&self, ctx: &Context) -> Option<CastKindAttr> {
        self.get_attr_gpu_cast_kind(ctx).map(|kind| *kind)
    }

    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

impl Verify for CastOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        verify_shape(self, ctx, 1, 1)?;
        let Some(kind) = self.kind(ctx) else {
            return verify_err!(self.loc(ctx), "gpu.cast requires a kind");
        };
        let from = self.get_operand_value(ctx).get_type(ctx);
        let to = self.result(ctx).get_type(ctx);
        let valid_pointer_restriction = if kind == CastKindAttr::RestrictPointerAccess {
            match (
                from.deref(ctx).downcast_ref::<PointerType>(),
                to.deref(ctx).downcast_ref::<PointerType>(),
            ) {
                (Some(from), Some(to)) => {
                    from.pointee() == to.pointee()
                        && from.address_space() == to.address_space()
                        && from.access() == AccessModeAttr::ReadWrite
                        && to.access() == AccessModeAttr::ReadOnly
                }
                _ => false,
            }
        } else {
            false
        };
        if valid_pointer_restriction || valid_scalar_cast_v1(ctx, kind, from, to) {
            Ok(())
        } else {
            verify_err!(
                self.loc(ctx),
                "gpu.cast kind is not legal for its operand and result types"
            )
        }
    }
}

#[pliron_op(
    name = "gpu.select",
    format,
    interfaces = [TargetNeutralGpuOpInterface, NOpdsInterface<3>, OneResultInterface, NRegionsInterface<0>],
    operands = (condition, true_value, false_value)
)]
pub struct SelectOp;

impl SelectOp {
    pub fn new(ctx: &mut Context, condition: Value, true_value: Value, false_value: Value) -> Self {
        Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![true_value.get_type(ctx)],
                vec![condition, true_value, false_value],
                vec![],
                0,
            ),
        }
    }

    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

impl Verify for SelectOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        verify_shape(self, ctx, 3, 1)?;
        if !is_bool(ctx, self.get_operand_condition(ctx).get_type(ctx))
            || self.get_operand_true_value(ctx).get_type(ctx)
                != self.get_operand_false_value(ctx).get_type(ctx)
            || self.get_operand_true_value(ctx).get_type(ctx) != self.result(ctx).get_type(ctx)
        {
            return verify_err!(self.loc(ctx), "gpu.select has incompatible types");
        }
        Ok(())
    }
}

/// Direct call with an explicit operand/result type contract.
///
/// Calls are conservatively effectful and are never DCE candidates. The local
/// verifier checks the stored function type but does not resolve the textual
/// callee; canonical Kernel IR export performs module-level symbol resolution.
#[pliron_op(
    name = "gpu.call",
    format,
    interfaces = [TargetNeutralGpuOpInterface, NRegionsInterface<0>],
    attributes = (gpu_call_callee: StringAttr, gpu_call_signature: TypeAttr)
)]
pub struct CallOp;

impl CallOp {
    pub fn new(
        ctx: &mut Context,
        callee: impl Into<String>,
        arguments: Vec<Value>,
        result_types: Vec<TypeHandle>,
    ) -> Self {
        let argument_types = arguments
            .iter()
            .map(|argument| argument.get_type(ctx))
            .collect();
        let signature = FunctionType::get(ctx, argument_types, result_types.clone());
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            result_types,
            arguments,
            vec![],
            0,
        );
        let call = Self { op };
        call.set_attr_gpu_call_callee(ctx, StringAttr::new(callee.into()));
        call.set_attr_gpu_call_signature(ctx, TypeAttr::new(signature.into()));
        call
    }

    pub fn callee(&self, ctx: &Context) -> Option<String> {
        self.get_attr_gpu_call_callee(ctx)
            .map(|callee| callee.as_str().to_owned())
    }

    pub fn arguments(&self, ctx: &Context) -> Vec<Value> {
        self.get_operation().deref(ctx).operands().collect()
    }

    pub fn results(&self, ctx: &Context) -> Vec<Value> {
        self.get_operation().deref(ctx).results().collect()
    }

    /// The immutable argument/result contract captured when this call was built.
    ///
    /// Resolution of the textual callee against a module symbol is deliberately
    /// deferred to the canonical Kernel IR export verifier.
    pub fn signature(&self, ctx: &Context) -> Option<TypeHandle> {
        self.get_attr_gpu_call_signature(ctx)
            .map(|signature| Typed::get_type(&*signature, ctx))
    }
}

impl Verify for CallOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let raw = self.get_operation().deref(ctx);
        if self.callee(ctx).is_none_or(|callee| callee.is_empty())
            || raw.get_num_successors() != 0
            || raw.num_regions() != 0
        {
            return verify_err!(
                self.loc(ctx),
                "gpu.call requires a nonempty callee and no CFG structure"
            );
        }
        let Some(signature) = self.signature(ctx) else {
            return verify_err!(self.loc(ctx), "gpu.call requires a function signature");
        };
        let signature = signature.deref(ctx);
        let Some(signature) = signature.downcast_ref::<FunctionType>() else {
            return verify_err!(self.loc(ctx), "gpu.call signature must be a function type");
        };
        if raw
            .operands()
            .map(|value| value.get_type(ctx))
            .collect::<Vec<_>>()
            != signature.arg_types()
            || raw.result_types().collect::<Vec<_>>() != signature.res_types()
        {
            return verify_err!(
                self.loc(ctx),
                "gpu.call operands and results must match its function signature"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "gpu.return",
    format,
    interfaces = [TargetNeutralGpuOpInterface, IsTerminatorInterface, NResultsInterface<0>, NRegionsInterface<0>],
)]
pub struct ReturnOp;

impl ReturnOp {
    pub fn new(ctx: &mut Context, values: Vec<Value>) -> Self {
        Self {
            op: Operation::new(ctx, Self::get_concrete_op_info(), vec![], values, vec![], 0),
        }
    }

    pub fn values(&self, ctx: &Context) -> Vec<Value> {
        self.get_operation().deref(ctx).operands().collect()
    }
}

impl Verify for ReturnOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let raw = self.get_operation().deref(ctx);
        if raw.get_num_results() != 0 || raw.get_num_successors() != 0 || raw.num_regions() != 0 {
            return verify_err!(
                self.loc(ctx),
                "gpu.return requires no results, successors, or regions"
            );
        }
        let Some(parent) = raw
            .get_parent_block()
            .and_then(|block| block.deref(ctx).get_parent_op(ctx))
        else {
            return verify_err!(self.loc(ctx), "gpu.return must be nested in builtin.func");
        };
        let Some(function) = Operation::get_op::<FuncOp>(parent, ctx) else {
            return verify_err!(self.loc(ctx), "gpu.return must be nested in builtin.func");
        };
        let Some(signature) = function.get_attr_func_type(ctx) else {
            return verify_err!(
                self.loc(ctx),
                "enclosing builtin.func requires a function type"
            );
        };
        let signature = Typed::get_type(&*signature, ctx);
        let signature = signature.deref(ctx);
        let Some(signature) = signature.downcast_ref::<FunctionType>() else {
            return verify_err!(
                self.loc(ctx),
                "enclosing builtin.func has a non-function type"
            );
        };
        if raw
            .operands()
            .map(|value| value.get_type(ctx))
            .collect::<Vec<_>>()
            != signature.res_types()
        {
            return verify_err!(
                self.loc(ctx),
                "gpu.return operands must match enclosing function results"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "gpu.branch",
    format,
    interfaces = [TargetNeutralGpuOpInterface, IsTerminatorInterface, NResultsInterface<0>, NSuccsInterface<1>, OneSuccInterface, NRegionsInterface<0>],
    verifier = "succ"
)]
pub struct BranchOp;

impl BranchOp {
    pub fn new(ctx: &mut Context, destination: Ptr<BasicBlock>, arguments: Vec<Value>) -> Self {
        Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![],
                arguments,
                vec![destination],
                0,
            ),
        }
    }
}

#[op_interface_impl]
impl BranchOpInterface for BranchOp {
    fn successor_operands(&self, ctx: &Context, succ_idx: usize) -> Vec<Value> {
        assert_eq!(succ_idx, 0, "gpu.branch has one successor");
        self.get_operation().deref(ctx).operands().collect()
    }

    fn add_successor_operand(&self, ctx: &mut Context, succ_idx: usize, operand: Value) -> usize {
        assert_eq!(succ_idx, 0, "gpu.branch has one successor");
        Operation::push_operand(self.get_operation(), ctx, operand)
    }

    fn remove_successor_operand(
        &self,
        ctx: &mut Context,
        succ_idx: usize,
        operand_idx: usize,
    ) -> Value {
        assert_eq!(succ_idx, 0, "gpu.branch has one successor");
        Operation::remove_operand(self.get_operation(), ctx, operand_idx)
    }
}

#[pliron_op(
    name = "gpu.cond_branch",
    format,
    interfaces = [TargetNeutralGpuOpInterface, IsTerminatorInterface, NResultsInterface<0>, NSuccsInterface<2>, NRegionsInterface<0>],
    operands = (condition, then_arguments, else_arguments)
)]
pub struct CondBranchOp;

impl CondBranchOp {
    pub fn new(
        ctx: &mut Context,
        condition: Value,
        then_destination: Ptr<BasicBlock>,
        then_arguments: Vec<Value>,
        else_destination: Ptr<BasicBlock>,
        else_arguments: Vec<Value>,
    ) -> Self {
        let (operands, segments) =
            Self::compute_segment_sizes(vec![vec![condition], then_arguments, else_arguments]);
        let branch = Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![],
                operands,
                vec![then_destination, else_destination],
                0,
            ),
        };
        branch.set_operand_segment_sizes(ctx, segments);
        branch
    }

    pub fn condition(&self, ctx: &Context) -> Value {
        self.get_operand_condition(ctx)
    }
}

#[op_interface_impl]
impl OperandSegmentInterface for CondBranchOp {}

impl Verify for CondBranchOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        if !is_bool(ctx, self.condition(ctx).get_type(ctx)) {
            return verify_err!(self.loc(ctx), "gpu.cond_branch condition must be i1");
        }
        Ok(())
    }
}

#[op_interface_impl]
impl BranchOpInterface for CondBranchOp {
    fn successor_operands(&self, ctx: &Context, succ_idx: usize) -> Vec<Value> {
        assert!(succ_idx < 2, "gpu.cond_branch has two successors");
        self.get_segment(ctx, succ_idx + 1)
    }

    fn add_successor_operand(&self, ctx: &mut Context, succ_idx: usize, operand: Value) -> usize {
        assert!(succ_idx < 2, "gpu.cond_branch has two successors");
        self.push_to_segment(ctx, succ_idx + 1, operand)
    }

    fn remove_successor_operand(
        &self,
        ctx: &mut Context,
        succ_idx: usize,
        operand_idx: usize,
    ) -> Value {
        assert!(succ_idx < 2, "gpu.cond_branch has two successors");
        self.remove_from_segment(ctx, succ_idx + 1, operand_idx)
    }
}

#[pliron_op(
    name = "gpu.slice_length",
    format,
    interfaces = [TargetNeutralGpuOpInterface, OneOpdInterface, OneResultInterface, NRegionsInterface<0>],
    operands = (slice: SliceType),
)]
pub struct SliceLengthOp;

impl SliceLengthOp {
    pub fn new(ctx: &mut Context, slice: Value) -> Self {
        Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![IndexType::get(ctx).into()],
                vec![slice],
                vec![],
                0,
            ),
        }
    }
}

impl Verify for SliceLengthOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        verify_shape(self, ctx, 1, 1)?;
        if self.get_operation().deref(ctx).get_type(0) != IndexType::get(ctx).into() {
            return verify_err!(self.loc(ctx), "gpu.slice_length result must be gpu.index");
        }
        Ok(())
    }
}

#[pliron_op(
    name = "gpu.slice_data",
    format,
    interfaces = [TargetNeutralGpuOpInterface, OneOpdInterface, OneResultInterface, NRegionsInterface<0>],
    operands = (slice: SliceType),
)]
pub struct SliceDataOp;

impl SliceDataOp {
    pub fn new(ctx: &mut Context, slice: Value) -> Option<Self> {
        let slice_ty = slice.get_type(ctx);
        let (element, address_space, access) = {
            let slice_ty_ref = slice_ty.deref(ctx);
            let slice_ty = slice_ty_ref.downcast_ref::<SliceType>()?;
            (
                slice_ty.element(),
                slice_ty.address_space(),
                slice_ty.access(),
            )
        };
        let result_ty = PointerType::get(ctx, element, address_space, access).into();
        Some(Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![result_ty],
                vec![slice],
                vec![],
                0,
            ),
        })
    }
}

impl Verify for SliceDataOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        verify_shape(self, ctx, 1, 1)?;
        let slice_type = self.get_operand_slice(ctx).get_type(ctx);
        let slice_type = slice_type.deref(ctx);
        let Some(slice_type) = slice_type.downcast_ref::<SliceType>() else {
            return verify_err!(self.loc(ctx), "gpu.slice_data requires a slice operand");
        };
        let result_type = self.get_operation().deref(ctx).get_type(0);
        let result_type = result_type.deref(ctx);
        let Some(result_type) = result_type.downcast_ref::<PointerType>() else {
            return verify_err!(self.loc(ctx), "gpu.slice_data result must be a pointer");
        };
        if result_type.pointee() != slice_type.element()
            || result_type.address_space() != slice_type.address_space()
            || result_type.access() != slice_type.access()
        {
            return verify_err!(
                self.loc(ctx),
                "gpu.slice_data result must match its slice element, address space, and access"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "gpu.get_element_pointer",
    format,
    interfaces = [TargetNeutralGpuOpInterface, NOpdsInterface<2>, OneResultInterface, NRegionsInterface<0>],
    operands = (base: PointerType, offset)
)]
pub struct GetElementPointerOp;

impl GetElementPointerOp {
    pub fn new(ctx: &mut Context, base: Value, offset: Value) -> Self {
        Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![base.get_type(ctx)],
                vec![base, offset],
                vec![],
                0,
            ),
        }
    }
}

impl Verify for GetElementPointerOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        if !is_integer(ctx, self.get_operand_offset(ctx).get_type(ctx))
            || self.get_operand_base(ctx).get_type(ctx)
                != self.get_operation().deref(ctx).get_result(0).get_type(ctx)
        {
            return verify_err!(
                self.loc(ctx),
                "gpu.get_element_pointer has incompatible types"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "gpu.load",
    format,
    interfaces = [TargetNeutralGpuOpInterface, OneOpdInterface, OneResultInterface, NRegionsInterface<0>],
    operands = (pointer: PointerType),
    attributes = (
        gpu_load_address_space: AddressSpaceAttr,
        gpu_load_alignment: MemoryAlignmentAttr,
        gpu_load_volatile: VolatileAttr
    )
)]
pub struct LoadOp;

impl LoadOp {
    pub fn new(ctx: &mut Context, pointer: Value, alignment: u32, volatile: bool) -> Option<Self> {
        if alignment == 0 || !alignment.is_power_of_two() {
            return None;
        }
        let pointer_ty = pointer.get_type(ctx);
        let (pointee, address_space, access) = {
            let pointer_ty_ref = pointer_ty.deref(ctx);
            let pointer_ty = pointer_ty_ref.downcast_ref::<PointerType>()?;
            (
                pointer_ty.pointee(),
                pointer_ty.address_space(),
                pointer_ty.access(),
            )
        };
        if access == AccessModeAttr::WriteOnly {
            return None;
        }
        let load = Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![pointee],
                vec![pointer],
                vec![],
                0,
            ),
        };
        load.set_attr_gpu_load_address_space(ctx, address_space);
        load.set_attr_gpu_load_alignment(ctx, MemoryAlignmentAttr(alignment));
        load.set_attr_gpu_load_volatile(ctx, VolatileAttr(volatile));
        Some(load)
    }

    pub fn address_space(&self, ctx: &Context) -> Option<AddressSpaceAttr> {
        self.get_attr_gpu_load_address_space(ctx)
            .map(|space| *space)
    }

    pub fn alignment(&self, ctx: &Context) -> Option<u32> {
        self.get_attr_gpu_load_alignment(ctx)
            .map(|alignment| alignment.0)
    }

    pub fn is_volatile(&self, ctx: &Context) -> Option<bool> {
        self.get_attr_gpu_load_volatile(ctx)
            .map(|volatile| volatile.0)
    }
}

impl Verify for LoadOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let pointer_ty = self.get_operand_pointer(ctx).get_type(ctx);
        let pointer_ty_ref = pointer_ty.deref(ctx);
        let Some(pointer_ty) = pointer_ty_ref.downcast_ref::<PointerType>() else {
            return verify_err!(self.loc(ctx), "gpu.load requires a pointer");
        };
        let (Some(address_space), Some(alignment), Some(_volatile)) = (
            self.address_space(ctx),
            self.alignment(ctx),
            self.is_volatile(ctx),
        ) else {
            return verify_err!(
                self.loc(ctx),
                "gpu.load requires complete memory access attributes"
            );
        };
        if pointer_ty.access() == AccessModeAttr::WriteOnly
            || pointer_ty.address_space() != address_space
            || alignment == 0
            || !alignment.is_power_of_two()
            || pointer_ty.pointee() != self.get_operation().deref(ctx).get_result(0).get_type(ctx)
        {
            return verify_err!(
                self.loc(ctx),
                "gpu.load violates pointer access or pointee type"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "gpu.store",
    format,
    interfaces = [TargetNeutralGpuOpInterface, NOpdsInterface<2>, NResultsInterface<0>, NRegionsInterface<0>],
    operands = (pointer: PointerType, value),
    attributes = (
        gpu_store_address_space: AddressSpaceAttr,
        gpu_store_alignment: MemoryAlignmentAttr,
        gpu_store_volatile: VolatileAttr
    )
)]
pub struct StoreOp;

impl StoreOp {
    pub fn new(
        ctx: &mut Context,
        pointer: Value,
        value: Value,
        alignment: u32,
        volatile: bool,
    ) -> Option<Self> {
        if alignment == 0 || !alignment.is_power_of_two() {
            return None;
        }
        let pointer_type = pointer.get_type(ctx);
        let (address_space, access) = {
            let pointer_type_ref = pointer_type.deref(ctx);
            let pointer_type = pointer_type_ref.downcast_ref::<PointerType>()?;
            (pointer_type.address_space(), pointer_type.access())
        };
        if access == AccessModeAttr::ReadOnly {
            return None;
        }
        let store = Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![],
                vec![pointer, value],
                vec![],
                0,
            ),
        };
        store.set_attr_gpu_store_address_space(ctx, address_space);
        store.set_attr_gpu_store_alignment(ctx, MemoryAlignmentAttr(alignment));
        store.set_attr_gpu_store_volatile(ctx, VolatileAttr(volatile));
        Some(store)
    }

    pub fn address_space(&self, ctx: &Context) -> Option<AddressSpaceAttr> {
        self.get_attr_gpu_store_address_space(ctx)
            .map(|space| *space)
    }

    pub fn alignment(&self, ctx: &Context) -> Option<u32> {
        self.get_attr_gpu_store_alignment(ctx)
            .map(|alignment| alignment.0)
    }

    pub fn is_volatile(&self, ctx: &Context) -> Option<bool> {
        self.get_attr_gpu_store_volatile(ctx)
            .map(|volatile| volatile.0)
    }
}

impl Verify for StoreOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let pointer_ty = self.get_operand_pointer(ctx).get_type(ctx);
        let pointer_ty_ref = pointer_ty.deref(ctx);
        let Some(pointer_ty) = pointer_ty_ref.downcast_ref::<PointerType>() else {
            return verify_err!(self.loc(ctx), "gpu.store requires a pointer");
        };
        let (Some(address_space), Some(alignment), Some(_volatile)) = (
            self.address_space(ctx),
            self.alignment(ctx),
            self.is_volatile(ctx),
        ) else {
            return verify_err!(
                self.loc(ctx),
                "gpu.store requires complete memory access attributes"
            );
        };
        if pointer_ty.access() == AccessModeAttr::ReadOnly
            || pointer_ty.address_space() != address_space
            || alignment == 0
            || !alignment.is_power_of_two()
            || pointer_ty.pointee() != self.get_operand_value(ctx).get_type(ctx)
        {
            return verify_err!(
                self.loc(ctx),
                "gpu.store violates pointer access or pointee type"
            );
        }
        Ok(())
    }
}

/// A typed, fail-closed carrier for canonical Kernel IR semantics that do not
/// yet have a dedicated optimizing Pliron operation.
///
/// Operand identities and result types are represented directly in the graph.
/// Exact versioned semantic fields are retained in the importing bridge's
/// private origin metadata. This operation is always effectful: generic passes
/// may propagate values through its operands but may not fold, CSE, or erase
/// the operation based on an incomplete semantic model.
#[pliron_op(
    name = "gpu.preserved_operation",
    format,
    interfaces = [TargetNeutralGpuOpInterface, NRegionsInterface<0>],
    attributes = (gpu_preserved_operation_kind: PreservedOperationKindAttr)
)]
pub struct PreservedOperationOp;

impl PreservedOperationOp {
    pub fn new(
        ctx: &mut Context,
        kind: PreservedOperationKindAttr,
        operands: Vec<Value>,
        result_types: Vec<TypeHandle>,
    ) -> Self {
        let operation = Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                result_types,
                operands,
                vec![],
                0,
            ),
        };
        operation.set_attr_gpu_preserved_operation_kind(ctx, kind);
        operation
    }

    pub fn kind(&self, ctx: &Context) -> Option<PreservedOperationKindAttr> {
        self.get_attr_gpu_preserved_operation_kind(ctx)
            .map(|kind| *kind)
    }

    pub fn operands(&self, ctx: &Context) -> Vec<Value> {
        self.get_operation().deref(ctx).operands().collect()
    }
}

impl Verify for PreservedOperationOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let raw = self.get_operation().deref(ctx);
        if self.kind(ctx).is_none() || raw.get_num_successors() != 0 || raw.num_regions() != 0 {
            return verify_err!(
                self.loc(ctx),
                "gpu.preserved_operation requires a semantic family and no CFG structure"
            );
        }
        Ok(())
    }
}

#[op_interface_impl]
impl SideEffects for PreservedOperationOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        true
    }
}

/// A CFG-aware carrier for switch-like and unreachable Kernel IR terminators.
///
/// Operand segment zero contains the selector for switch variants. Each
/// remaining segment corresponds positionally to one real Pliron successor.
/// The exact case constants and variant payload remain bridge-owned metadata.
#[pliron_op(
    name = "gpu.preserved_terminator",
    format,
    interfaces = [TargetNeutralGpuOpInterface, IsTerminatorInterface, NResultsInterface<0>, NRegionsInterface<0>],
    attributes = (gpu_preserved_terminator_kind: PreservedTerminatorKindAttr)
)]
pub struct PreservedTerminatorOp;

impl PreservedTerminatorOp {
    pub fn new_switch(
        ctx: &mut Context,
        kind: PreservedTerminatorKindAttr,
        selector: Value,
        successors: Vec<Ptr<BasicBlock>>,
        successor_arguments: Vec<Vec<Value>>,
    ) -> Option<Self> {
        if kind == PreservedTerminatorKindAttr::Unreachable
            || successors.is_empty()
            || successors.len() != successor_arguments.len()
        {
            return None;
        }
        let mut segments = Vec::with_capacity(successor_arguments.len() + 1);
        segments.push(vec![selector]);
        segments.extend(successor_arguments);
        let (operands, sizes) = Self::compute_segment_sizes(segments);
        let operation = Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![],
                operands,
                successors,
                0,
            ),
        };
        operation.set_operand_segment_sizes(ctx, sizes);
        operation.set_attr_gpu_preserved_terminator_kind(ctx, kind);
        Some(operation)
    }

    pub fn new_unreachable(ctx: &mut Context) -> Self {
        let operation = Self {
            op: Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 0),
        };
        operation.set_operand_segment_sizes(ctx, OperandSegmentSizesAttr(vec![]));
        operation
            .set_attr_gpu_preserved_terminator_kind(ctx, PreservedTerminatorKindAttr::Unreachable);
        operation
    }

    pub fn kind(&self, ctx: &Context) -> Option<PreservedTerminatorKindAttr> {
        self.get_attr_gpu_preserved_terminator_kind(ctx)
            .map(|kind| *kind)
    }

    pub fn selector(&self, ctx: &Context) -> Option<Value> {
        self.get_segment(ctx, 0).first().copied()
    }
}

#[op_interface_impl]
impl OperandSegmentInterface for PreservedTerminatorOp {}

#[op_interface_impl]
impl BranchOpInterface for PreservedTerminatorOp {
    fn successor_operands(&self, ctx: &Context, succ_idx: usize) -> Vec<Value> {
        assert!(
            succ_idx < self.get_operation().deref(ctx).get_num_successors(),
            "gpu.preserved_terminator successor index out of range"
        );
        self.get_segment(ctx, succ_idx + 1)
    }

    fn add_successor_operand(&self, ctx: &mut Context, succ_idx: usize, operand: Value) -> usize {
        assert!(
            succ_idx < self.get_operation().deref(ctx).get_num_successors(),
            "gpu.preserved_terminator successor index out of range"
        );
        self.push_to_segment(ctx, succ_idx + 1, operand)
    }

    fn remove_successor_operand(
        &self,
        ctx: &mut Context,
        succ_idx: usize,
        operand_idx: usize,
    ) -> Value {
        assert!(
            succ_idx < self.get_operation().deref(ctx).get_num_successors(),
            "gpu.preserved_terminator successor index out of range"
        );
        self.remove_from_segment(ctx, succ_idx + 1, operand_idx)
    }
}

impl Verify for PreservedTerminatorOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let raw = self.get_operation().deref(ctx);
        if raw.get_num_results() != 0 || raw.num_regions() != 0 {
            return verify_err!(
                self.loc(ctx),
                "gpu.preserved_terminator requires no results or regions"
            );
        }
        let Some(kind) = self.kind(ctx) else {
            return verify_err!(
                self.loc(ctx),
                "gpu.preserved_terminator requires a semantic kind"
            );
        };
        let Some(segments) = raw
            .attributes
            .get::<OperandSegmentSizesAttr>(&ATTR_KEY_OPERAND_SEGMENT_SIZES)
            .map(|segments| &segments.0)
        else {
            return verify_err!(
                self.loc(ctx),
                "gpu.preserved_terminator requires operand segment metadata"
            );
        };
        match kind {
            PreservedTerminatorKindAttr::Switch | PreservedTerminatorKindAttr::IntegerSwitch => {
                if raw.get_num_successors() == 0
                    || segments.len() != raw.get_num_successors() + 1
                    || segments.first() != Some(&1)
                    || raw.get_num_operands() == 0
                    || !is_integer(ctx, raw.get_operand(0).get_type(ctx))
                {
                    return verify_err!(
                        self.loc(ctx),
                        "preserved switch requires one integer selector and one operand segment per successor"
                    );
                }
            }
            PreservedTerminatorKindAttr::Unreachable => {
                if raw.get_num_operands() != 0
                    || raw.get_num_successors() != 0
                    || !segments.is_empty()
                {
                    return verify_err!(
                        self.loc(ctx),
                        "preserved unreachable cannot have operands or successors"
                    );
                }
            }
        }
        Ok(())
    }
}

macro_rules! pure_ops {
    ($($op:ty),+ $(,)?) => {
        $(
            #[op_interface_impl]
            impl SideEffects for $op {
                fn has_side_effects(&self, _ctx: &Context) -> bool {
                    false
                }
            }
        )+
    };
}

pure_ops!(ConstantOp, CompareOp, SelectOp, SliceLengthOp, SliceDataOp);

#[op_interface_impl]
impl SideEffects for UnaryOp {
    fn has_side_effects(&self, ctx: &Context) -> bool {
        self.kind(ctx).is_none_or(|kind| kind != UnaryKindAttr::Not)
    }
}

#[op_interface_impl]
impl SideEffects for BinaryOp {
    fn has_side_effects(&self, ctx: &Context) -> bool {
        self.kind(ctx).is_none_or(|kind| {
            !matches!(
                kind,
                BinaryKindAttr::BitAnd
                    | BinaryKindAttr::BitOr
                    | BinaryKindAttr::BitXor
                    | BinaryKindAttr::CheckedAdd
                    | BinaryKindAttr::CheckedSubtract
                    | BinaryKindAttr::CheckedMultiply
            )
        })
    }
}

#[op_interface_impl]
impl SideEffects for CastOp {
    fn has_side_effects(&self, ctx: &Context) -> bool {
        self.kind(ctx).is_none_or(|kind| {
            !matches!(
                kind,
                CastKindAttr::RestrictPointerAccess
                    | CastKindAttr::Truncate
                    | CastKindAttr::ZeroExtend
                    | CastKindAttr::SignExtend
                    | CastKindAttr::Bitcast
            )
        })
    }
}

#[op_interface_impl]
impl ConstFoldInterface for ConstantOp {
    fn check_fold(&self, ctx: &Context, operands: &[Option<AttrObj>]) -> Vec<Option<AttrObj>> {
        if operands.is_empty() {
            vec![Some(self.value(ctx))]
        } else {
            vec![None]
        }
    }

    fn fold_in_place(
        &self,
        _ctx: &mut Context,
        _operands: &[Option<AttrObj>],
        _rewriter: &mut dyn Rewriter,
    ) -> IRStatus {
        IRStatus::Unchanged
    }
}

fn integer_operands(operands: &[Option<AttrObj>]) -> Option<(IntegerAttr, IntegerAttr)> {
    let [Some(lhs), Some(rhs)] = operands else {
        return None;
    };
    let lhs = lhs.downcast_ref::<IntegerAttr>()?.clone();
    let rhs = rhs.downcast_ref::<IntegerAttr>()?.clone();
    (lhs.value().bw() == rhs.value().bw()).then_some((lhs, rhs))
}

fn integer_result(attr: &IntegerAttr, value: APInt) -> AttrObj {
    Box::new(IntegerAttr::new(attr.get_type(), value))
}

fn signed_division_traps(lhs: &APInt, rhs: &APInt) -> bool {
    let Some(width) = NonZero::new(lhs.bw()) else {
        return true;
    };
    rhs.is_zero() || (*lhs == APInt::imin(width) && *rhs == APInt::umax(width))
}

#[op_interface_impl]
impl ConstFoldInterface for UnaryOp {
    fn check_fold(&self, ctx: &Context, operands: &[Option<AttrObj>]) -> Vec<Option<AttrObj>> {
        let [Some(operand)] = operands else {
            return vec![None];
        };
        let Some(operand) = operand.downcast_ref::<IntegerAttr>() else {
            return vec![None];
        };
        if self.kind(ctx) != Some(UnaryKindAttr::Not) {
            return vec![None];
        }
        let value = operand.value();
        let Some(width) = NonZero::new(value.bw()) else {
            return vec![None];
        };
        vec![Some(integer_result(
            operand,
            value.xor(&APInt::umax(width)),
        ))]
    }

    fn fold_in_place(
        &self,
        ctx: &mut Context,
        operands: &[Option<AttrObj>],
        rewriter: &mut dyn Rewriter,
    ) -> IRStatus {
        self.fold_with_materialization(ctx, operands, rewriter)
    }
}

#[op_interface_impl]
impl ConstFoldInterface for BinaryOp {
    fn check_fold(&self, ctx: &Context, operands: &[Option<AttrObj>]) -> Vec<Option<AttrObj>> {
        let result_count = self.get_operation().deref(ctx).get_num_results();
        let none = || vec![None; result_count];
        let Some(kind) = self.kind(ctx) else {
            return none();
        };
        let Some((lhs, rhs)) = integer_operands(operands) else {
            return none();
        };
        let lhs_value = lhs.value();
        let rhs_value = rhs.value();
        let signed = lhs.get_type().deref(ctx).is_signed();
        let value = match kind {
            BinaryKindAttr::Add => {
                let (value, unsigned_overflow, signed_overflow) =
                    lhs_value.add_overflow(&rhs_value);
                if if signed {
                    signed_overflow
                } else {
                    unsigned_overflow
                } {
                    return none();
                }
                value
            }
            BinaryKindAttr::Subtract => {
                let (value, unsigned_overflow, signed_overflow) =
                    lhs_value.sub_overflow(&rhs_value);
                if if signed {
                    signed_overflow
                } else {
                    unsigned_overflow
                } {
                    return none();
                }
                value
            }
            BinaryKindAttr::Multiply => {
                let (value, unsigned_overflow, signed_overflow) =
                    lhs_value.mul_overflow(&rhs_value);
                if if signed {
                    signed_overflow
                } else {
                    unsigned_overflow
                } {
                    return none();
                }
                value
            }
            BinaryKindAttr::Divide if rhs_value.is_zero() => return none(),
            BinaryKindAttr::Divide if signed && signed_division_traps(&lhs_value, &rhs_value) => {
                return none();
            }
            BinaryKindAttr::Divide if signed => lhs_value.sdiv(&rhs_value),
            BinaryKindAttr::Divide => lhs_value.udiv(&rhs_value),
            BinaryKindAttr::Remainder if rhs_value.is_zero() => return none(),
            BinaryKindAttr::Remainder
                if signed && signed_division_traps(&lhs_value, &rhs_value) =>
            {
                return none();
            }
            BinaryKindAttr::Remainder if signed => lhs_value.srem(&rhs_value),
            BinaryKindAttr::Remainder => lhs_value.urem(&rhs_value),
            BinaryKindAttr::BitAnd => lhs_value.and(&rhs_value),
            BinaryKindAttr::BitOr => lhs_value.or(&rhs_value),
            BinaryKindAttr::BitXor => lhs_value.xor(&rhs_value),
            BinaryKindAttr::ShiftLeft | BinaryKindAttr::ShiftRight
                if rhs_value.to_usize() >= lhs_value.bw() =>
            {
                return none();
            }
            BinaryKindAttr::ShiftLeft => lhs_value.shl(&rhs_value),
            BinaryKindAttr::ShiftRight if signed => lhs_value.ashr(&rhs_value),
            BinaryKindAttr::ShiftRight => lhs_value.lshr(&rhs_value),
            BinaryKindAttr::CheckedAdd
            | BinaryKindAttr::CheckedSubtract
            | BinaryKindAttr::CheckedMultiply => {
                let (value, unsigned_overflow, signed_overflow) = match kind {
                    BinaryKindAttr::CheckedAdd => lhs_value.add_overflow(&rhs_value),
                    BinaryKindAttr::CheckedSubtract => lhs_value.sub_overflow(&rhs_value),
                    BinaryKindAttr::CheckedMultiply => lhs_value.mul_overflow(&rhs_value),
                    _ => unreachable!(),
                };
                let overflow = if signed {
                    signed_overflow
                } else {
                    unsigned_overflow
                };
                let bool_ty = IntegerType::get(ctx, 1, Signedness::Signless);
                return vec![
                    Some(integer_result(&lhs, value)),
                    Some(Box::new(IntegerAttr::new(
                        bool_ty,
                        APInt::from_u8(u8::from(overflow), bw(1)),
                    ))),
                ];
            }
        };
        vec![Some(integer_result(&lhs, value))]
    }

    fn fold_in_place(
        &self,
        ctx: &mut Context,
        operands: &[Option<AttrObj>],
        rewriter: &mut dyn Rewriter,
    ) -> IRStatus {
        self.fold_with_materialization(ctx, operands, rewriter)
    }
}

#[op_interface_impl]
impl ConstFoldInterface for CastOp {
    fn check_fold(&self, ctx: &Context, operands: &[Option<AttrObj>]) -> Vec<Option<AttrObj>> {
        let [Some(operand)] = operands else {
            return vec![None];
        };
        let Some(operand) = operand.downcast_ref::<IntegerAttr>() else {
            return vec![None];
        };
        let Some(kind) = self.kind(ctx) else {
            return vec![None];
        };
        if !valid_scalar_cast_v1(
            ctx,
            kind,
            self.get_operand_value(ctx).get_type(ctx),
            self.result(ctx).get_type(ctx),
        ) {
            return vec![None];
        }
        let result_type = self.result(ctx).get_type(ctx);
        let result_type_ref = result_type.deref(ctx);
        let Some(result_integer) = result_type_ref.downcast_ref::<IntegerType>() else {
            return vec![None];
        };
        let target_width = result_integer.width() as usize;
        let Some(width) = NonZero::new(target_width) else {
            return vec![None];
        };
        let value = match kind {
            CastKindAttr::SignExtend => operand.value().sext(width),
            CastKindAttr::Truncate | CastKindAttr::ZeroExtend | CastKindAttr::Bitcast => {
                operand.value().zext(width)
            }
            CastKindAttr::RestrictPointerAccess => return vec![None],
            _ => return vec![None],
        };
        let target_type =
            IntegerType::get(ctx, result_integer.width(), result_integer.signedness());
        vec![Some(Box::new(IntegerAttr::new(target_type, value)))]
    }

    fn fold_in_place(
        &self,
        ctx: &mut Context,
        operands: &[Option<AttrObj>],
        rewriter: &mut dyn Rewriter,
    ) -> IRStatus {
        self.fold_with_materialization(ctx, operands, rewriter)
    }
}

#[op_interface_impl]
impl ConstFoldInterface for CompareOp {
    fn check_fold(&self, ctx: &Context, operands: &[Option<AttrObj>]) -> Vec<Option<AttrObj>> {
        let Some((lhs, rhs)) = integer_operands(operands) else {
            return vec![None];
        };
        let Some(predicate) = self.predicate(ctx) else {
            return vec![None];
        };
        let lhs_value = lhs.value();
        let rhs_value = rhs.value();
        let signed = lhs.get_type().deref(ctx).is_signed();
        let result = match predicate {
            ComparePredicateAttr::Equal => lhs_value == rhs_value,
            ComparePredicateAttr::NotEqual => lhs_value != rhs_value,
            ComparePredicateAttr::LessThan if signed => lhs_value.slt(&rhs_value),
            ComparePredicateAttr::LessThan => lhs_value.ult(&rhs_value),
            ComparePredicateAttr::LessThanOrEqual if signed => lhs_value.sle(&rhs_value),
            ComparePredicateAttr::LessThanOrEqual => lhs_value.ule(&rhs_value),
            ComparePredicateAttr::GreaterThan if signed => lhs_value.sgt(&rhs_value),
            ComparePredicateAttr::GreaterThan => lhs_value.ugt(&rhs_value),
            ComparePredicateAttr::GreaterThanOrEqual if signed => lhs_value.sge(&rhs_value),
            ComparePredicateAttr::GreaterThanOrEqual => lhs_value.uge(&rhs_value),
        };
        let bool_ty = IntegerType::get(ctx, 1, Signedness::Signless);
        vec![Some(Box::new(IntegerAttr::new(
            bool_ty,
            APInt::from_u8(u8::from(result), bw(1)),
        )))]
    }

    fn fold_in_place(
        &self,
        ctx: &mut Context,
        operands: &[Option<AttrObj>],
        rewriter: &mut dyn Rewriter,
    ) -> IRStatus {
        self.fold_with_materialization(ctx, operands, rewriter)
    }
}

#[op_interface_impl]
impl ConstFoldInterface for SelectOp {
    fn check_fold(&self, _ctx: &Context, operands: &[Option<AttrObj>]) -> Vec<Option<AttrObj>> {
        let [Some(condition), true_value, false_value] = operands else {
            return vec![None];
        };
        let Some(condition) = condition.downcast_ref::<IntegerAttr>() else {
            return vec![None];
        };
        let selected = if condition.value().is_zero() {
            false_value
        } else {
            true_value
        };
        vec![selected.clone()]
    }

    fn fold_in_place(
        &self,
        ctx: &mut Context,
        operands: &[Option<AttrObj>],
        rewriter: &mut dyn Rewriter,
    ) -> IRStatus {
        self.fold_with_materialization(ctx, operands, rewriter)
    }
}

#[op_interface_impl]
impl BranchOpFoldInterface for BranchOp {
    fn check_fold(&self, ctx: &Context, _operands: &[Option<AttrObj>]) -> Vec<Ptr<BasicBlock>> {
        self.get_operation().deref(ctx).successors().collect()
    }

    fn fold_in_place(
        &self,
        _ctx: &mut Context,
        _operands: &[Option<AttrObj>],
        _rewriter: &mut dyn Rewriter,
    ) -> IRStatus {
        IRStatus::Unchanged
    }
}

#[op_interface_impl]
impl BranchOpFoldInterface for CondBranchOp {
    fn check_fold(&self, ctx: &Context, operands: &[Option<AttrObj>]) -> Vec<Ptr<BasicBlock>> {
        let successors: Vec<_> = self.get_operation().deref(ctx).successors().collect();
        let Some(condition) = operands
            .first()
            .and_then(Option::as_ref)
            .and_then(|attr| attr.downcast_ref::<IntegerAttr>())
        else {
            return successors;
        };
        vec![successors[usize::from(condition.value().is_zero())]]
    }

    fn fold_in_place(
        &self,
        ctx: &mut Context,
        operands: &[Option<AttrObj>],
        rewriter: &mut dyn Rewriter,
    ) -> IRStatus {
        let Some(condition) = operands
            .first()
            .and_then(Option::as_ref)
            .and_then(|attr| attr.downcast_ref::<IntegerAttr>())
        else {
            return IRStatus::Unchanged;
        };
        let successor_index = usize::from(condition.value().is_zero());
        let destination = self
            .get_operation()
            .deref(ctx)
            .get_successor(successor_index);
        let replacement = BranchOp::new(
            ctx,
            destination,
            self.successor_operands(ctx, successor_index),
        )
        .get_operation();
        rewriter.insert_operation(ctx, replacement);
        rewriter.replace_operation(ctx, self.get_operation(), replacement);
        IRStatus::Changed
    }
}

/// Canonicalizes `select %cond, %value, %value` to `%value`.
#[derive(Default)]
pub struct SelectSameValuePattern;

impl MatchRewrite for SelectSameValuePattern {
    fn r#match(&mut self, ctx: &Context, op: Ptr<Operation>) -> bool {
        Operation::get_op::<SelectOp>(op, ctx).is_some_and(|select| {
            select.get_operand_true_value(ctx) == select.get_operand_false_value(ctx)
        })
    }

    fn rewrite(
        &mut self,
        ctx: &mut Context,
        rewriter: &mut MatchRewriter,
        op: Ptr<Operation>,
    ) -> Result<()> {
        let Some(select) = Operation::get_op::<SelectOp>(op, ctx) else {
            return Ok(());
        };
        rewriter.replace_operation_with_values(ctx, op, vec![select.get_operand_true_value(ctx)]);
        Ok(())
    }
}
