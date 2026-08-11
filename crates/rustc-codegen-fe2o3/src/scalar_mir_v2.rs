//! Target-bound admission of normalized rustc scalar MIR into scalar Kernel IR V2.

use std::fmt;

use dialect_amdgcn::{ScalarV2LoweringError, lower_scalar_v2_to_gfx942_llvm};
use fe2o3_kernel_ir::ValueId;
use fe2o3_kernel_ir::scalar_ops_v2::{
    Cast, Diagnostic, FloatArithmeticSemantics, FloatBinary, FloatComparisonPolicy,
    FloatToIntSemantics, FloatWidth, IntBinary, IntMode, IntToFloatSemantics, IntUnary, Operation,
    Predicate, ScalarOperationV2, ScalarType, ShiftDirection, ShiftPolicy,
};

use crate::AmdGpuTarget;

pub const EXACT_SCALAR_V2_TARGET: &str = "gfx942:xnack-";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustcMirBinaryV2 {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    AddWithOverflow,
    SubWithOverflow,
    MulWithOverflow,
    AddUnchecked,
    SubUnchecked,
    MulUnchecked,
    ShlUnchecked,
    ShrUnchecked,
    Cmp,
    Offset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustcMirUnaryV2 {
    Neg,
    Not,
    PtrMetadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustcScalarIntrinsicV2 {
    IntegerBinary {
        op: IntBinary,
        mode: IntMode,
        ty: ScalarType,
    },
    Shift {
        direction: ShiftDirection,
        policy: ShiftPolicy,
        ty: ScalarType,
        rhs_ty: ScalarType,
    },
    IntToBoolChecked {
        from: ScalarType,
    },
    IntToCharChecked {
        from: ScalarType,
    },
    FloatCompare {
        ty: ScalarType,
        predicate: Predicate,
        policy: FloatComparisonPolicy,
    },
    FloatTotalCmp {
        ty: ScalarType,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustcScalarExpressionV2 {
    Binary {
        op: RustcMirBinaryV2,
        lhs: ScalarType,
        rhs: ScalarType,
        /// Exact rustc overflow policy used only by source `<<` and `>>`.
        overflow_checks: bool,
    },
    Unary {
        op: RustcMirUnaryV2,
        ty: ScalarType,
    },
    NumericCast {
        from: ScalarType,
        to: ScalarType,
    },
    Intrinsic(RustcScalarIntrinsicV2),
}

#[derive(Clone, Copy, Debug)]
pub struct RustcScalarRequestV2<'a> {
    pub target: &'a AmdGpuTarget,
    pub custom_llvm_pipeline: bool,
    pub expression: RustcScalarExpressionV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcScalarArtifactV2 {
    pub kernel_ir: ScalarOperationV2,
    pub gfx942_llvm: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustcScalarAdmissionErrorV2 {
    WrongTarget {
        expected: &'static str,
        actual: String,
    },
    CustomLlvmPipeline,
    UnsupportedMir(String),
    InvalidKernelIr(Vec<Diagnostic>),
    Backend(ScalarV2LoweringError),
}

impl fmt::Display for RustcScalarAdmissionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTarget { expected, actual } => {
                write!(
                    formatter,
                    "scalar V2 requires target {expected}, found {actual}"
                )
            }
            Self::CustomLlvmPipeline => formatter
                .write_str("scalar V2 rejects custom LLVM arguments, passes, and fast-math policy"),
            Self::UnsupportedMir(message) => formatter.write_str(message),
            Self::InvalidKernelIr(diagnostics) => {
                write!(formatter, "invalid scalar Kernel IR V2: {diagnostics:?}")
            }
            Self::Backend(error) => write!(formatter, "gfx942 scalar lowering failed: {error}"),
        }
    }
}

impl std::error::Error for RustcScalarAdmissionErrorV2 {}

/// Admits one exact normalized rustc scalar expression and lowers it atomically.
///
/// An error contains diagnostics only. LLVM text is returned only after target,
/// source-semantics, Kernel IR, and backend validation have all succeeded.
pub fn lower_rustc_scalar_v2(
    request: RustcScalarRequestV2<'_>,
) -> Result<RustcScalarArtifactV2, RustcScalarAdmissionErrorV2> {
    if request.target.as_str() != EXACT_SCALAR_V2_TARGET {
        return Err(RustcScalarAdmissionErrorV2::WrongTarget {
            expected: EXACT_SCALAR_V2_TARGET,
            actual: request.target.as_str().to_owned(),
        });
    }
    if request.custom_llvm_pipeline {
        return Err(RustcScalarAdmissionErrorV2::CustomLlvmPipeline);
    }
    let operation = normalize_expression(request.expression)?;
    let arity = operation_arity(operation);
    let kernel_ir = ScalarOperationV2::new(
        operation,
        (0..arity).map(|value| ValueId(value as u32)).collect(),
    )
    .map_err(RustcScalarAdmissionErrorV2::InvalidKernelIr)?;
    let gfx942_llvm =
        lower_scalar_v2_to_gfx942_llvm(&kernel_ir).map_err(RustcScalarAdmissionErrorV2::Backend)?;
    Ok(RustcScalarArtifactV2 {
        kernel_ir,
        gfx942_llvm,
    })
}

fn normalize_expression(
    expression: RustcScalarExpressionV2,
) -> Result<Operation, RustcScalarAdmissionErrorV2> {
    match expression {
        RustcScalarExpressionV2::Binary {
            op,
            lhs,
            rhs,
            overflow_checks,
        } => normalize_binary(op, lhs, rhs, overflow_checks),
        RustcScalarExpressionV2::Unary { op, ty } => normalize_unary(op, ty),
        RustcScalarExpressionV2::NumericCast { from, to } => normalize_cast(from, to),
        RustcScalarExpressionV2::Intrinsic(intrinsic) => Ok(match intrinsic {
            RustcScalarIntrinsicV2::IntegerBinary { op, mode, ty } => {
                Operation::IntegerBinary { ty, op, mode }
            }
            RustcScalarIntrinsicV2::Shift {
                direction,
                policy,
                ty,
                rhs_ty,
            } => Operation::Shift {
                ty,
                rhs_ty,
                direction,
                policy,
            },
            RustcScalarIntrinsicV2::IntToBoolChecked { from } => Operation::Cast {
                from,
                to: ScalarType::Bool,
                cast: Cast::IntToBoolChecked,
            },
            RustcScalarIntrinsicV2::IntToCharChecked { from } => Operation::Cast {
                from,
                to: ScalarType::Char,
                cast: Cast::IntToCharChecked,
            },
            RustcScalarIntrinsicV2::FloatCompare {
                ty,
                predicate,
                policy,
            } => Operation::FloatCompare {
                ty,
                predicate,
                policy,
            },
            RustcScalarIntrinsicV2::FloatTotalCmp { ty } => Operation::FloatTotalCompare { ty },
        }),
    }
}

fn normalize_binary(
    op: RustcMirBinaryV2,
    lhs: ScalarType,
    rhs: ScalarType,
    overflow_checks: bool,
) -> Result<Operation, RustcScalarAdmissionErrorV2> {
    if lhs != rhs && !matches!(op, RustcMirBinaryV2::Shl | RustcMirBinaryV2::Shr) {
        return unsupported(format!(
            "rustc MIR binary operands must have one exact type; found {lhs:?} and {rhs:?}"
        ));
    }
    match lhs {
        ScalarType::Int { .. } => normalize_integer_binary(op, lhs, rhs, overflow_checks),
        ScalarType::Float(FloatWidth::F32 | FloatWidth::F64) => normalize_float_binary(op, lhs),
        ScalarType::Bool | ScalarType::Char | ScalarType::Pointer { .. } | ScalarType::Float(_) => {
            unsupported(format!(
                "rustc MIR scalar binary {op:?} rejects type {lhs:?}"
            ))
        }
    }
}

fn normalize_integer_binary(
    op: RustcMirBinaryV2,
    ty: ScalarType,
    rhs_ty: ScalarType,
    overflow_checks: bool,
) -> Result<Operation, RustcScalarAdmissionErrorV2> {
    let operation = match op {
        RustcMirBinaryV2::Add | RustcMirBinaryV2::Sub | RustcMirBinaryV2::Mul => {
            Operation::IntegerBinary {
                ty,
                op: mir_integer_op(op),
                mode: IntMode::Wrapping,
            }
        }
        RustcMirBinaryV2::AddWithOverflow
        | RustcMirBinaryV2::SubWithOverflow
        | RustcMirBinaryV2::MulWithOverflow => Operation::IntegerBinary {
            ty,
            op: mir_integer_op(op),
            mode: IntMode::Overflowing,
        },
        RustcMirBinaryV2::BitAnd | RustcMirBinaryV2::BitOr | RustcMirBinaryV2::BitXor => {
            Operation::IntegerBinary {
                ty,
                op: mir_integer_op(op),
                mode: IntMode::Wrapping,
            }
        }
        RustcMirBinaryV2::Shl | RustcMirBinaryV2::Shr => Operation::Shift {
            ty,
            rhs_ty,
            direction: if op == RustcMirBinaryV2::Shl {
                ShiftDirection::Left
            } else {
                ShiftDirection::Right
            },
            policy: ShiftPolicy::RustOperator { overflow_checks },
        },
        RustcMirBinaryV2::Eq
        | RustcMirBinaryV2::Ne
        | RustcMirBinaryV2::Lt
        | RustcMirBinaryV2::Le
        | RustcMirBinaryV2::Gt
        | RustcMirBinaryV2::Ge => Operation::IntegerCompare {
            ty,
            predicate: mir_predicate(op),
        },
        RustcMirBinaryV2::Div | RustcMirBinaryV2::Rem => {
            return unsupported(
                "raw rustc Div/Rem requires composition with its exact assertion terminator; use an authenticated checked/wrapping/overflowing/saturating intrinsic until that CFG composition is present",
            );
        }
        RustcMirBinaryV2::AddUnchecked
        | RustcMirBinaryV2::SubUnchecked
        | RustcMirBinaryV2::MulUnchecked
        | RustcMirBinaryV2::ShlUnchecked
        | RustcMirBinaryV2::ShrUnchecked => {
            return unsupported(
                "unchecked rustc scalar MIR is outside the safe scalar V2 contract",
            );
        }
        RustcMirBinaryV2::Cmp => {
            return unsupported(
                "rustc Cmp is not inferred as float total_cmp; only the explicit authenticated total_cmp intrinsic is admitted",
            );
        }
        RustcMirBinaryV2::Offset => {
            return unsupported("pointer offset/provenance is outside scalar rows 24 and 25");
        }
    };
    Ok(operation)
}

fn normalize_float_binary(
    op: RustcMirBinaryV2,
    ty: ScalarType,
) -> Result<Operation, RustcScalarAdmissionErrorV2> {
    Ok(match op {
        RustcMirBinaryV2::Div => {
            return unsupported(
                "gfx942 floating division is rejected before LLVM because constrained fdiv is not reviewed on LLVM 18",
            );
        }
        RustcMirBinaryV2::Add
        | RustcMirBinaryV2::Sub
        | RustcMirBinaryV2::Mul
        | RustcMirBinaryV2::Rem => Operation::FloatBinary {
            ty,
            op: match op {
                RustcMirBinaryV2::Add => FloatBinary::Add,
                RustcMirBinaryV2::Sub => FloatBinary::Sub,
                RustcMirBinaryV2::Mul => FloatBinary::Mul,
                RustcMirBinaryV2::Rem => FloatBinary::Rem,
                _ => unreachable!(),
            },
            semantics: FloatArithmeticSemantics::RustIeee754,
        },
        RustcMirBinaryV2::Eq | RustcMirBinaryV2::Ne => Operation::FloatCompare {
            ty,
            predicate: mir_predicate(op),
            policy: FloatComparisonPolicy::RustPartialEq,
        },
        RustcMirBinaryV2::Lt
        | RustcMirBinaryV2::Le
        | RustcMirBinaryV2::Gt
        | RustcMirBinaryV2::Ge => Operation::FloatCompare {
            ty,
            predicate: mir_predicate(op),
            policy: FloatComparisonPolicy::RustPartialOrd,
        },
        _ => return unsupported(format!("unsupported rustc floating MIR operation {op:?}")),
    })
}

fn normalize_unary(
    op: RustcMirUnaryV2,
    ty: ScalarType,
) -> Result<Operation, RustcScalarAdmissionErrorV2> {
    match (op, ty) {
        (RustcMirUnaryV2::Neg, ScalarType::Int { signed: true, .. }) => {
            Ok(Operation::IntegerUnary {
                ty,
                op: IntUnary::Neg,
                mode: IntMode::Wrapping,
            })
        }
        (RustcMirUnaryV2::Not, ScalarType::Int { .. }) => Ok(Operation::IntegerUnary {
            ty,
            op: IntUnary::Not,
            mode: IntMode::Wrapping,
        }),
        (RustcMirUnaryV2::Neg, ScalarType::Float(FloatWidth::F32 | FloatWidth::F64)) => {
            Ok(Operation::FloatNeg {
                ty,
                semantics: FloatArithmeticSemantics::RustIeee754,
            })
        }
        (RustcMirUnaryV2::PtrMetadata, _) => {
            unsupported("pointer metadata is outside scalar rows 24 and 25")
        }
        _ => unsupported(format!(
            "unsupported rustc unary operation {op:?} for {ty:?}"
        )),
    }
}

fn normalize_cast(
    from: ScalarType,
    to: ScalarType,
) -> Result<Operation, RustcScalarAdmissionErrorV2> {
    let cast = match (from, to) {
        (
            ScalarType::Int {
                width: from_width,
                signed,
            },
            ScalarType::Int {
                width: to_width, ..
            },
        ) if from_width.bits() < to_width.bits() => Cast::IntExtend { signed },
        (ScalarType::Int { width: from, .. }, ScalarType::Int { width: to, .. })
            if from.bits() > to.bits() =>
        {
            Cast::IntNarrow
        }
        (ScalarType::Int { .. }, ScalarType::Int { .. }) => Cast::Bitcast,
        (ScalarType::Float(from), ScalarType::Float(to)) if from.bits() < to.bits() => {
            Cast::FloatExtend
        }
        (ScalarType::Float(from), ScalarType::Float(to)) if from.bits() > to.bits() => {
            Cast::FloatNarrow
        }
        (ScalarType::Int { .. }, ScalarType::Float(_)) => Cast::IntToFloat {
            semantics: IntToFloatSemantics::RustAs,
        },
        (ScalarType::Float(_), ScalarType::Int { .. }) => Cast::FloatToInt {
            semantics: FloatToIntSemantics::RustSaturatingAs,
        },
        (ScalarType::Bool, ScalarType::Int { .. }) => Cast::BoolToInt,
        (ScalarType::Char, ScalarType::Int { .. }) => Cast::CharToInt,
        (ScalarType::Pointer { .. }, _) | (_, ScalarType::Pointer { .. }) => {
            return unsupported("numeric cast admission rejects pointer/provenance operations");
        }
        (ScalarType::Int { .. }, ScalarType::Bool | ScalarType::Char) => {
            return unsupported(
                "Rust numeric `as` cannot create bool or char; use an authenticated checked conversion",
            );
        }
        _ => {
            return unsupported(format!(
                "unsupported or no-op Rust numeric cast {from:?} to {to:?}"
            ));
        }
    };
    Ok(Operation::Cast { from, to, cast })
}

const fn operation_arity(operation: Operation) -> usize {
    match operation {
        Operation::IntegerUnary { .. } | Operation::FloatNeg { .. } | Operation::Cast { .. } => 1,
        Operation::IntegerBinary { .. }
        | Operation::Shift { .. }
        | Operation::IntegerCompare { .. }
        | Operation::FloatBinary { .. }
        | Operation::FloatCompare { .. }
        | Operation::FloatTotalCompare { .. } => 2,
    }
}

const fn mir_integer_op(op: RustcMirBinaryV2) -> IntBinary {
    match op {
        RustcMirBinaryV2::Add | RustcMirBinaryV2::AddWithOverflow => IntBinary::Add,
        RustcMirBinaryV2::Sub | RustcMirBinaryV2::SubWithOverflow => IntBinary::Sub,
        RustcMirBinaryV2::Mul | RustcMirBinaryV2::MulWithOverflow => IntBinary::Mul,
        RustcMirBinaryV2::BitAnd => IntBinary::And,
        RustcMirBinaryV2::BitOr => IntBinary::Or,
        RustcMirBinaryV2::BitXor => IntBinary::Xor,
        _ => unreachable!(),
    }
}

const fn mir_predicate(op: RustcMirBinaryV2) -> Predicate {
    match op {
        RustcMirBinaryV2::Eq => Predicate::Eq,
        RustcMirBinaryV2::Ne => Predicate::Ne,
        RustcMirBinaryV2::Lt => Predicate::Lt,
        RustcMirBinaryV2::Le => Predicate::Le,
        RustcMirBinaryV2::Gt => Predicate::Gt,
        RustcMirBinaryV2::Ge => Predicate::Ge,
        _ => unreachable!(),
    }
}

fn unsupported<T>(message: impl Into<String>) -> Result<T, RustcScalarAdmissionErrorV2> {
    Err(RustcScalarAdmissionErrorV2::UnsupportedMir(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::scalar_ops_v2::IntWidth;

    fn i(width: IntWidth, signed: bool) -> ScalarType {
        ScalarType::Int { width, signed }
    }

    fn lower(expression: RustcScalarExpressionV2) -> RustcScalarArtifactV2 {
        lower_rustc_scalar_v2(RustcScalarRequestV2 {
            target: &AmdGpuTarget::new(EXACT_SCALAR_V2_TARGET),
            custom_llvm_pipeline: false,
            expression,
        })
        .unwrap()
    }

    #[test]
    fn exact_mir_modes_and_targets_are_admitted_or_rejected_before_llvm() {
        let overflow = lower(RustcScalarExpressionV2::Binary {
            op: RustcMirBinaryV2::AddWithOverflow,
            lhs: i(IntWidth::W128, true),
            rhs: i(IntWidth::W128, true),
            overflow_checks: true,
        });
        assert!(matches!(
            overflow.kernel_ir.operation(),
            Operation::IntegerBinary {
                mode: IntMode::Overflowing,
                ..
            }
        ));
        assert!(overflow.gfx942_llvm.contains("sadd.with.overflow.i128"));

        for target in ["gfx942", "gfx942:xnack+", "gfx1100"] {
            assert!(matches!(
                lower_rustc_scalar_v2(RustcScalarRequestV2 {
                    target: &AmdGpuTarget::new(target),
                    custom_llvm_pipeline: false,
                    expression: RustcScalarExpressionV2::Binary {
                        op: RustcMirBinaryV2::Add,
                        lhs: i(IntWidth::W32, false),
                        rhs: i(IntWidth::W32, false),
                        overflow_checks: false,
                    },
                }),
                Err(RustcScalarAdmissionErrorV2::WrongTarget { .. })
            ));
        }
        assert!(matches!(
            lower_rustc_scalar_v2(RustcScalarRequestV2 {
                target: &AmdGpuTarget::new(EXACT_SCALAR_V2_TARGET),
                custom_llvm_pipeline: true,
                expression: RustcScalarExpressionV2::Unary {
                    op: RustcMirUnaryV2::Neg,
                    ty: ScalarType::Float(FloatWidth::F32),
                },
            }),
            Err(RustcScalarAdmissionErrorV2::CustomLlvmPipeline)
        ));
    }

    #[test]
    fn unsupported_mir_provenance_and_modes_fail_closed() {
        for op in [
            RustcMirBinaryV2::Div,
            RustcMirBinaryV2::Rem,
            RustcMirBinaryV2::AddUnchecked,
            RustcMirBinaryV2::ShlUnchecked,
            RustcMirBinaryV2::Cmp,
            RustcMirBinaryV2::Offset,
        ] {
            assert!(matches!(
                lower_rustc_scalar_v2(RustcScalarRequestV2 {
                    target: &AmdGpuTarget::new(EXACT_SCALAR_V2_TARGET),
                    custom_llvm_pipeline: false,
                    expression: RustcScalarExpressionV2::Binary {
                        op,
                        lhs: i(IntWidth::W64, true),
                        rhs: i(IntWidth::W64, true),
                        overflow_checks: true,
                    },
                }),
                Err(RustcScalarAdmissionErrorV2::UnsupportedMir(_))
            ));
        }
        assert!(matches!(
            lower_rustc_scalar_v2(RustcScalarRequestV2 {
                target: &AmdGpuTarget::new(EXACT_SCALAR_V2_TARGET),
                custom_llvm_pipeline: false,
                expression: RustcScalarExpressionV2::NumericCast {
                    from: i(IntWidth::W32, false),
                    to: ScalarType::Char,
                },
            }),
            Err(RustcScalarAdmissionErrorV2::UnsupportedMir(_))
        ));
        assert!(matches!(
            lower_rustc_scalar_v2(RustcScalarRequestV2 {
                target: &AmdGpuTarget::new(EXACT_SCALAR_V2_TARGET),
                custom_llvm_pipeline: false,
                expression: RustcScalarExpressionV2::Binary {
                    op: RustcMirBinaryV2::Div,
                    lhs: ScalarType::Float(FloatWidth::F64),
                    rhs: ScalarType::Float(FloatWidth::F64),
                    overflow_checks: false,
                },
            }),
            Err(RustcScalarAdmissionErrorV2::UnsupportedMir(message))
                if message.contains("rejected before LLVM")
        ));
    }

    #[test]
    fn intrinsic_div_shift_cast_compare_and_total_cmp_are_explicit() {
        let div = lower(RustcScalarExpressionV2::Intrinsic(
            RustcScalarIntrinsicV2::IntegerBinary {
                op: IntBinary::Div,
                mode: IntMode::Checked,
                ty: i(IntWidth::W128, true),
            },
        ));
        assert!(div.gfx942_llvm.contains("%divide.loop"));
        assert!(!div.gfx942_llvm.contains("sdiv i128"));

        let shift = lower(RustcScalarExpressionV2::Binary {
            op: RustcMirBinaryV2::Shr,
            lhs: i(IntWidth::W8, true),
            rhs: i(IntWidth::W128, false),
            overflow_checks: true,
        });
        assert!(shift.gfx942_llvm.contains("%rhs.raw"));
        assert!(shift.gfx942_llvm.contains("call void @llvm.trap"));

        let cast = lower(RustcScalarExpressionV2::NumericCast {
            from: ScalarType::Float(FloatWidth::F64),
            to: i(IntWidth::W128, false),
        });
        assert!(cast.gfx942_llvm.contains("%exponent.raw"));

        let compare = lower(RustcScalarExpressionV2::Binary {
            op: RustcMirBinaryV2::Ne,
            lhs: ScalarType::Float(FloatWidth::F32),
            rhs: ScalarType::Float(FloatWidth::F32),
            overflow_checks: false,
        });
        assert!(compare.gfx942_llvm.contains("fcmp une float"));

        let total = lower(RustcScalarExpressionV2::Intrinsic(
            RustcScalarIntrinsicV2::FloatTotalCmp {
                ty: ScalarType::Float(FloatWidth::F64),
            },
        ));
        assert!(total.gfx942_llvm.contains("ret i8 %result"));
    }

    #[test]
    fn admission_matrix_covers_all_integer_widths_modes_and_numeric_casts() {
        let widths = [
            IntWidth::W8,
            IntWidth::W16,
            IntWidth::W32,
            IntWidth::W64,
            IntWidth::W128,
        ];
        for width in widths {
            for signed in [false, true] {
                let ty = i(width, signed);
                for op in [
                    IntBinary::Add,
                    IntBinary::Sub,
                    IntBinary::Mul,
                    IntBinary::Div,
                    IntBinary::Rem,
                    IntBinary::And,
                    IntBinary::Or,
                    IntBinary::Xor,
                ] {
                    for mode in [
                        IntMode::Checked,
                        IntMode::Wrapping,
                        IntMode::Overflowing,
                        IntMode::Saturating,
                    ] {
                        let structurally_valid = !matches!(
                            (op, mode),
                            (IntBinary::Rem, IntMode::Saturating)
                                | (
                                    IntBinary::And | IntBinary::Or | IntBinary::Xor,
                                    IntMode::Checked | IntMode::Overflowing | IntMode::Saturating
                                )
                        );
                        let result = lower_rustc_scalar_v2(RustcScalarRequestV2 {
                            target: &AmdGpuTarget::new(EXACT_SCALAR_V2_TARGET),
                            custom_llvm_pipeline: false,
                            expression: RustcScalarExpressionV2::Intrinsic(
                                RustcScalarIntrinsicV2::IntegerBinary { op, mode, ty },
                            ),
                        });
                        assert_eq!(result.is_ok(), structurally_valid, "{ty:?} {op:?} {mode:?}");
                    }
                }
                for rhs_width in widths {
                    for rhs_signed in [false, true] {
                        for policy in [
                            ShiftPolicy::Checked,
                            ShiftPolicy::Wrapping,
                            ShiftPolicy::Overflowing,
                            ShiftPolicy::RustOperator {
                                overflow_checks: false,
                            },
                            ShiftPolicy::RustOperator {
                                overflow_checks: true,
                            },
                        ] {
                            lower(RustcScalarExpressionV2::Intrinsic(
                                RustcScalarIntrinsicV2::Shift {
                                    direction: ShiftDirection::Left,
                                    policy,
                                    ty,
                                    rhs_ty: i(rhs_width, rhs_signed),
                                },
                            ));
                        }
                    }
                }
            }
        }

        for from_width in widths {
            for to_width in widths {
                for from_signed in [false, true] {
                    for to_signed in [false, true] {
                        let artifact = lower(RustcScalarExpressionV2::NumericCast {
                            from: i(from_width, from_signed),
                            to: i(to_width, to_signed),
                        });
                        if from_width == to_width {
                            assert!(matches!(
                                artifact.kernel_ir.operation(),
                                Operation::Cast {
                                    cast: Cast::Bitcast,
                                    ..
                                }
                            ));
                            assert!(artifact.gfx942_llvm.contains(&format!(
                                "%result = add i{} %arg0, 0",
                                from_width.bits()
                            )));
                        }
                    }
                }
            }
        }
        for float in [FloatWidth::F32, FloatWidth::F64] {
            for width in widths {
                for signed in [false, true] {
                    lower(RustcScalarExpressionV2::NumericCast {
                        from: ScalarType::Float(float),
                        to: i(width, signed),
                    });
                    lower(RustcScalarExpressionV2::NumericCast {
                        from: i(width, signed),
                        to: ScalarType::Float(float),
                    });
                }
            }
        }
    }

    #[test]
    fn normalized_mir_source_fixture_covers_the_vertical_slice() {
        let fixture = include_str!("../tests/fixtures/scalar-v2.mir.json");
        let value: serde_json::Value = serde_json::from_str(fixture).unwrap();
        assert_eq!(value["target"], EXACT_SCALAR_V2_TARGET);
        let operations = value["operations"].as_array().unwrap();
        assert!(operations.len() >= 16);
        for spelling in [
            "AddWithOverflow",
            "checked_div",
            "wrapping_rem",
            "Shr",
            "FloatToInt",
            "IntToFloat",
            "RustPartialEq",
            "RustPartialOrd",
            "total_cmp",
            "bool_to_int",
            "char_to_int",
        ] {
            assert!(
                fixture.contains(spelling),
                "missing fixture spelling {spelling}"
            );
        }
    }
}
