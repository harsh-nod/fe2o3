//! Allocation-free exact fixed-width values. Unknown runtime values never fold.
use fe2o3_kernel_ir::scalar_ops_v2::{
    self as ops, Cast, IntMode, IntOutcome, IntUnary, IntWidth, ScalarType,
};
use fe2o3_rustc_front::{
    CONSTANT_FOLD_POLICY_VERSION_V1, ConstantFoldBinaryOpV1 as Op, FixedWidthIntegerV1,
    fold_binary_v1,
};
use rustc_middle::{
    mir::{BinOp, UnOp},
    ty::{IntTy, Ty, TyKind, UintTy},
};

pub(super) fn ty(value: Ty<'_>) -> Option<ScalarType> {
    let (width, signed) = match value.kind() {
        TyKind::Bool => return Some(ScalarType::Bool),
        TyKind::Int(IntTy::I8) => (IntWidth::W8, true),
        TyKind::Int(IntTy::I16) => (IntWidth::W16, true),
        TyKind::Int(IntTy::I32) => (IntWidth::W32, true),
        TyKind::Int(IntTy::I64) => (IntWidth::W64, true),
        TyKind::Int(IntTy::I128) => (IntWidth::W128, true),
        TyKind::Uint(UintTy::U8) => (IntWidth::W8, false),
        TyKind::Uint(UintTy::U16) => (IntWidth::W16, false),
        TyKind::Uint(UintTy::U32) => (IntWidth::W32, false),
        TyKind::Uint(UintTy::U64) => (IntWidth::W64, false),
        TyKind::Uint(UintTy::U128) => (IntWidth::W128, false),
        _ => return None,
    };
    Some(ScalarType::Int { width, signed })
}

pub(super) fn lossless(from: ScalarType, to: ScalarType) -> bool {
    matches!((from, to), (ScalarType::Int { width: a, signed: sa }, ScalarType::Int { width: b, signed: sb })
        if b.bits() > a.bits() && (!sa || sb))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Scalar {
    pub(super) ty: ScalarType,
    pub(super) bits: u128,
}
impl Scalar {
    pub(super) fn new(ty: ScalarType, bits: u128) -> Option<Self> {
        let signed = match ty {
            ScalarType::Bool => false,
            ScalarType::Int { signed, .. } => signed,
            _ => return None,
        };
        FixedWidthIntegerV1::new(ty.bit_width(), signed, bits).ok()?;
        Some(Self { ty, bits })
    }
    fn fixed(self) -> Option<FixedWidthIntegerV1> {
        let signed = match self.ty {
            ScalarType::Bool => false,
            ScalarType::Int { signed, .. } => signed,
            _ => return None,
        };
        FixedWidthIntegerV1::new(self.ty.bit_width(), signed, self.bits).ok()
    }
    pub(super) fn cast(self, to: ScalarType) -> Option<Self> {
        let ScalarType::Int {
            width: from,
            signed,
        } = self.ty
        else {
            return None;
        };
        let ScalarType::Int { width: target, .. } = to else {
            return None;
        };
        let cast = match from.bits().cmp(&target.bits()) {
            std::cmp::Ordering::Less => Cast::IntExtend { signed },
            std::cmp::Ordering::Equal => Cast::Bitcast,
            std::cmp::Ordering::Greater => Cast::IntNarrow,
        };
        Self::new(
            to,
            ops::evaluate_integer_cast(self.ty, to, cast, self.bits)?,
        )
    }
    pub(super) fn unary(self, op: UnOp) -> Option<Self> {
        if self.ty == ScalarType::Bool {
            return (op == UnOp::Not)
                .then(|| Self::new(self.ty, self.bits ^ 1))
                .flatten();
        }
        let (operation, mode) = match op {
            UnOp::Not => (IntUnary::Not, IntMode::Wrapping),
            UnOp::Neg => (IntUnary::Neg, IntMode::Checked),
            _ => return None,
        };
        match ops::evaluate_integer_unary(self.ty, operation, mode, self.bits)? {
            IntOutcome::Value(bits) => Self::new(self.ty, bits),
            IntOutcome::CheckedNone | IntOutcome::Overflowing { .. } | IntOutcome::Trap => None,
        }
    }
    pub(super) fn binary(self, op: BinOp, other: Self) -> Option<Self> {
        if self.ty != other.ty {
            return None;
        }
        let operation = match op {
            BinOp::Add => Op::Add,
            BinOp::Sub => Op::Subtract,
            BinOp::Mul => Op::Multiply,
            BinOp::Div => Op::Divide,
            BinOp::Rem => Op::Remainder,
            BinOp::BitAnd => Op::BitAnd,
            BinOp::BitOr => Op::BitOr,
            BinOp::BitXor => Op::BitXor,
            BinOp::Shl => Op::ShiftLeft,
            BinOp::Shr => Op::ShiftRight,
            BinOp::Eq => Op::Equal,
            BinOp::Ne => Op::NotEqual,
            BinOp::Lt => Op::LessThan,
            BinOp::Le => Op::LessThanOrEqual,
            BinOp::Gt => Op::GreaterThan,
            BinOp::Ge => Op::GreaterThanOrEqual,
            _ => return None,
        };
        let result = fold_binary_v1(
            CONSTANT_FOLD_POLICY_VERSION_V1,
            operation,
            self.fixed()?.into(),
            other.fixed()?.into(),
        )
        .ok()?;
        Self::new(
            if result.width() == 1 {
                ScalarType::Bool
            } else {
                self.ty
            },
            result.bits(),
        )
    }
}
