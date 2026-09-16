//! Scalar transfer only. No target width is inferred for Index, and no
//! floating-point, memory, call, trap-freedom or provenance claim is made.
use crate::canonical_kir_sparse_v1::{
    CanonicalKirSparseConstantV1 as ConstantValue, CanonicalKirSparseValueV1 as Value,
};
use fe2o3_kernel_ir::{
    BinaryOp, CastKind, CheckedBinaryOperator, ComparePredicate, Constant, OperationKind,
    ScalarType, UnaryOp, scalar_ops_v2 as scalar,
};

#[derive(Clone, Copy)]
pub(super) struct Input {
    pub value: Value,
    pub ty: Option<ScalarType>,
}
pub(super) struct Transfer {
    pub values: [Value; 2],
    pub exceptional: bool,
}
impl Transfer {
    fn one(value: Value) -> Self {
        Self {
            values: [value, Value::Dynamic],
            exceptional: false,
        }
    }
    fn exceptional() -> Self {
        Self {
            values: [Value::Dynamic; 2],
            exceptional: true,
        }
    }
}
pub(super) fn literal(value: &Constant) -> Option<ConstantValue> {
    let (ty, bits) = match *value {
        Constant::Bool(value) => (ScalarType::Bool, u128::from(value)),
        Constant::I8(value) => (ScalarType::I8, u128::from(value as u8)),
        Constant::I16(value) => (ScalarType::I16, u128::from(value as u16)),
        Constant::I32(value) => (ScalarType::I32, u128::from(value as u32)),
        Constant::I64(value) => (ScalarType::I64, u128::from(value as u64)),
        Constant::U8(value) => (ScalarType::U8, u128::from(value)),
        Constant::U16(value) => (ScalarType::U16, u128::from(value)),
        Constant::U32(value) => (ScalarType::U32, u128::from(value)),
        Constant::U64(value) => (ScalarType::U64, u128::from(value)),
        Constant::Index(value) => (ScalarType::Index, u128::from(value)),
        Constant::F16Bits(_)
        | Constant::Bf16Bits(_)
        | Constant::F32Bits(_)
        | Constant::F64Bits(_) => return None,
    };
    Some(ConstantValue { ty, bits })
}
pub(super) fn fixed(ty: ScalarType) -> Option<scalar::ScalarType> {
    use scalar::IntWidth as W;
    let width = match ty {
        ScalarType::I8 | ScalarType::U8 => W::W8,
        ScalarType::I16 | ScalarType::U16 => W::W16,
        ScalarType::I32 | ScalarType::U32 => W::W32,
        ScalarType::I64 | ScalarType::U64 => W::W64,
        ScalarType::I128 | ScalarType::U128 => W::W128,
        ScalarType::Bool
        | ScalarType::Index
        | ScalarType::F16
        | ScalarType::Bf16
        | ScalarType::F32
        | ScalarType::F64 => return None,
    };
    Some(scalar::ScalarType::Int {
        width,
        signed: ty.is_signed_integer(),
    })
}
fn pending(inputs: &[Input]) -> Value {
    if inputs.iter().any(|input| input.value == Value::Dynamic) {
        Value::Dynamic
    } else {
        Value::Unknown
    }
}
fn constant(ty: ScalarType, bits: u128) -> Value {
    Value::Constant(ConstantValue { ty, bits })
}
fn outcome(ty: ScalarType, outcome: Option<scalar::IntOutcome>, checked: bool) -> Transfer {
    match outcome {
        Some(scalar::IntOutcome::Value(bits)) => Transfer::one(constant(ty, bits)),
        Some(scalar::IntOutcome::Overflowing { value, overflowed }) if checked => Transfer {
            values: [
                constant(ty, value),
                constant(ScalarType::Bool, u128::from(overflowed)),
            ],
            exceptional: false,
        },
        Some(scalar::IntOutcome::Overflowing {
            value,
            overflowed: false,
        }) => Transfer::one(constant(ty, value)),
        Some(scalar::IntOutcome::Overflowing {
            overflowed: true, ..
        })
        | Some(scalar::IntOutcome::CheckedNone | scalar::IntOutcome::Trap) => {
            Transfer::exceptional()
        }
        None => Transfer::one(Value::Dynamic),
    }
}
pub(super) fn transfer(
    kind: &OperationKind,
    result_type: Option<ScalarType>,
    inputs: [Input; 3],
) -> Transfer {
    let [a, b, c] = inputs;
    match kind {
        OperationKind::Constant(value) => {
            Transfer::one(literal(value).map_or(Value::Dynamic, Value::Constant))
        }
        OperationKind::Select { .. } => {
            if let Value::Constant(condition) = a.value
                && condition.ty == ScalarType::Bool
                && condition.bits <= 1
            {
                return Transfer::one(if condition.bits == 1 {
                    b.value
                } else {
                    c.value
                });
            }
            if b.value == c.value && matches!(b.value, Value::Constant(_)) {
                return Transfer::one(b.value);
            }
            Transfer::one(if a.value == Value::Dynamic {
                b.value.join(c.value)
            } else {
                Value::Unknown
            })
        }
        OperationKind::Unary { op, .. } => {
            if a.ty == Some(ScalarType::Bool) && *op == UnaryOp::Not {
                return Transfer::one(match a.value {
                    Value::Constant(value) => {
                        constant(ScalarType::Bool, u128::from(value.bits == 0))
                    }
                    _ => pending(&[a]),
                });
            }
            let Some(ty) = a.ty.and_then(fixed) else {
                return Transfer::one(Value::Dynamic);
            };
            let Value::Constant(value) = a.value else {
                return Transfer::one(pending(&[a]));
            };
            let op = match op {
                UnaryOp::Negate => scalar::IntUnary::Neg,
                UnaryOp::Not => scalar::IntUnary::Not,
            };
            outcome(
                value.ty,
                scalar::evaluate_integer_unary(ty, op, scalar::IntMode::Wrapping, value.bits),
                false,
            )
        }
        OperationKind::Binary { op, .. } => {
            let Some(ty) = a.ty.and_then(fixed) else {
                return Transfer::one(Value::Dynamic);
            };
            let (Value::Constant(left), Value::Constant(right)) = (a.value, b.value) else {
                let value = pending(&[a, b]);
                return Transfer {
                    values: [value; 2],
                    exceptional: false,
                };
            };
            if matches!(op, BinaryOp::ShiftLeft | BinaryOp::ShiftRight) {
                let Some(rhs_ty) = b.ty.and_then(fixed) else {
                    return Transfer::one(Value::Dynamic);
                };
                let direction = if *op == BinaryOp::ShiftLeft {
                    scalar::ShiftDirection::Left
                } else {
                    scalar::ShiftDirection::Right
                };
                return outcome(
                    left.ty,
                    scalar::evaluate_shift(
                        ty,
                        rhs_ty,
                        direction,
                        scalar::ShiftPolicy::Checked,
                        left.bits,
                        right.bits,
                    ),
                    false,
                );
            }
            let (operator, mode, checked) = match op {
                BinaryOp::Add => (scalar::IntBinary::Add, scalar::IntMode::Wrapping, false),
                BinaryOp::Subtract => (scalar::IntBinary::Sub, scalar::IntMode::Wrapping, false),
                BinaryOp::Multiply => (scalar::IntBinary::Mul, scalar::IntMode::Wrapping, false),
                BinaryOp::Divide => (scalar::IntBinary::Div, scalar::IntMode::Overflowing, false),
                BinaryOp::Remainder => {
                    (scalar::IntBinary::Rem, scalar::IntMode::Overflowing, false)
                }
                BinaryOp::BitAnd => (scalar::IntBinary::And, scalar::IntMode::Wrapping, false),
                BinaryOp::BitOr => (scalar::IntBinary::Or, scalar::IntMode::Wrapping, false),
                BinaryOp::BitXor => (scalar::IntBinary::Xor, scalar::IntMode::Wrapping, false),
                BinaryOp::Checked(operator) => (
                    match operator {
                        CheckedBinaryOperator::Add => scalar::IntBinary::Add,
                        CheckedBinaryOperator::Subtract => scalar::IntBinary::Sub,
                        CheckedBinaryOperator::Multiply => scalar::IntBinary::Mul,
                    },
                    scalar::IntMode::Overflowing,
                    true,
                ),
                BinaryOp::ShiftLeft | BinaryOp::ShiftRight => unreachable!("handled above"),
            };
            outcome(
                left.ty,
                scalar::evaluate_integer_binary(ty, operator, mode, left.bits, right.bits),
                checked,
            )
        }
        OperationKind::Compare { predicate, .. } => {
            let Some(ty) = a.ty else {
                return Transfer::one(Value::Dynamic);
            };
            if fixed(ty).is_none() && ty != ScalarType::Bool {
                return Transfer::one(Value::Dynamic);
            }
            let (Value::Constant(left), Value::Constant(right)) = (a.value, b.value) else {
                return Transfer::one(pending(&[a, b]));
            };
            let predicate = match predicate {
                ComparePredicate::Equal => scalar::Predicate::Eq,
                ComparePredicate::NotEqual => scalar::Predicate::Ne,
                ComparePredicate::LessThan => scalar::Predicate::Lt,
                ComparePredicate::LessThanOrEqual => scalar::Predicate::Le,
                ComparePredicate::GreaterThan => scalar::Predicate::Gt,
                ComparePredicate::GreaterThanOrEqual => scalar::Predicate::Ge,
            };
            let result = if ty == ScalarType::Bool {
                match predicate {
                    scalar::Predicate::Eq => Some(left.bits == right.bits),
                    scalar::Predicate::Ne => Some(left.bits != right.bits),
                    _ => None,
                }
            } else {
                scalar::evaluate_integer_compare(
                    fixed(ty).expect("checked above"),
                    predicate,
                    left.bits,
                    right.bits,
                )
            };
            Transfer::one(result.map_or(Value::Dynamic, |value| {
                constant(ScalarType::Bool, u128::from(value))
            }))
        }
        OperationKind::Cast { kind, .. } => {
            let (Some(from), Some(to)) = (a.ty, result_type) else {
                return Transfer::one(Value::Dynamic);
            };
            let Some(target) = fixed(to) else {
                return Transfer::one(Value::Dynamic);
            };
            let source = if from == ScalarType::Bool {
                Some(scalar::ScalarType::Bool)
            } else {
                fixed(from)
            };
            let Some(source) = source else {
                return Transfer::one(Value::Dynamic);
            };
            let cast = match kind {
                CastKind::Truncate => scalar::Cast::IntNarrow,
                CastKind::ZeroExtend if from == ScalarType::Bool => scalar::Cast::BoolToInt,
                CastKind::ZeroExtend => scalar::Cast::IntExtend { signed: false },
                CastKind::SignExtend => scalar::Cast::IntExtend { signed: true },
                CastKind::Bitcast => scalar::Cast::Bitcast,
                CastKind::RestrictPointerAccess
                | CastKind::FloatExtend
                | CastKind::FloatTruncate
                | CastKind::IntegerToFloat
                | CastKind::FloatToInteger => return Transfer::one(Value::Dynamic),
            };
            let Value::Constant(value) = a.value else {
                return Transfer::one(pending(&[a]));
            };
            Transfer::one(
                scalar::evaluate_integer_cast(source, target, cast, value.bits)
                    .map_or(Value::Dynamic, |bits| constant(to, bits)),
            )
        }
        OperationKind::Execution(_)
        | OperationKind::VerificationContract(_)
        | OperationKind::VectorLoad(_)
        | OperationKind::VectorStore(_)
        | OperationKind::VectorLayoutConvert(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::MemoryIntrinsic(_)
        | OperationKind::Call { .. }
        | OperationKind::Alloca { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Load { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::Store { .. }
        | OperationKind::Barrier(_)
        | OperationKind::Atomic(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::Matrix(_)
        | OperationKind::Gfx950LdsTranspose(_)
        | OperationKind::Wave(_)
        | OperationKind::InlineAssembly(_) => Transfer::one(Value::Dynamic),
    }
}
