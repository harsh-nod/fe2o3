use std::cmp::Ordering;

#[cfg(test)]
use fe2o3_kernel_ir::FloatOperation;
use fe2o3_kernel_ir::{
    BinaryOp, CastKind, ComparePredicate, F32MathFunction, FloatConversionKind, FunctionId,
    NarrowFloatFormat, ScalarType, UnaryOp, ValueId, WidenedFloatBinaryOp,
};
use rustc_apfloat::ieee::{BFloat, Double, Half, Single};
use rustc_apfloat::{Float, FloatConvert, Round, Status};

use crate::{ScalarBitsV1, SimulationTargetV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SoftFloatErrorV1 {
    InvalidIntegerConversion,
    InternalInvariant(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SoftFloatOperationV1 {
    Convert(FloatConversionKind),
    WidenedBinary(NarrowFloatFormat, WidenedFloatBinaryOp),
    F32Math(F32MathFunction),
    Bf16x2FusedMultiplyAdd,
}

#[cfg(test)]
impl SoftFloatOperationV1 {
    fn from_operation(operation: &FloatOperation) -> Self {
        match operation {
            FloatOperation::Convert { kind, .. } => Self::Convert(*kind),
            FloatOperation::WidenedBinary { format, op, .. } => Self::WidenedBinary(*format, *op),
            FloatOperation::F32Math { function, .. } => Self::F32Math(*function),
            FloatOperation::Bf16x2FusedMultiplyAdd { .. } => Self::Bf16x2FusedMultiplyAdd,
        }
    }
}

/// Decodes only exact canonical FloatOperation V1 identities. The admitted module has already
/// passed KIR semantic verification; this compact form prevents runtime heap allocation while
/// preserving the structured operation kind used by the evaluator.
pub(crate) fn operation_for_call_v1(
    callee: &FunctionId,
    arguments: &[ValueId],
) -> Option<SoftFloatOperationV1> {
    let (operation, arity) = match callee.as_str() {
        "__fe2o3_ir_float_v1_f16_to_f32" => (
            SoftFloatOperationV1::Convert(FloatConversionKind::F16ToF32),
            1,
        ),
        "__fe2o3_ir_float_v1_f32_to_f16_rne" => (
            SoftFloatOperationV1::Convert(FloatConversionKind::F32ToF16RoundTiesEven),
            1,
        ),
        "__fe2o3_ir_float_v1_bf16_to_f32" => (
            SoftFloatOperationV1::Convert(FloatConversionKind::Bf16ToF32),
            1,
        ),
        "__fe2o3_ir_float_v1_f32_to_bf16_rne" => (
            SoftFloatOperationV1::Convert(FloatConversionKind::F32ToBf16RoundTiesEven),
            1,
        ),
        "__fe2o3_ir_float_v1_f16_add_widened_rne" => (
            SoftFloatOperationV1::WidenedBinary(NarrowFloatFormat::F16, WidenedFloatBinaryOp::Add),
            2,
        ),
        "__fe2o3_ir_float_v1_f16_sub_widened_rne" => (
            SoftFloatOperationV1::WidenedBinary(
                NarrowFloatFormat::F16,
                WidenedFloatBinaryOp::Subtract,
            ),
            2,
        ),
        "__fe2o3_ir_float_v1_f16_mul_widened_rne" => (
            SoftFloatOperationV1::WidenedBinary(
                NarrowFloatFormat::F16,
                WidenedFloatBinaryOp::Multiply,
            ),
            2,
        ),
        "__fe2o3_ir_float_v1_f16_div_widened_rne" => (
            SoftFloatOperationV1::WidenedBinary(
                NarrowFloatFormat::F16,
                WidenedFloatBinaryOp::Divide,
            ),
            2,
        ),
        "__fe2o3_ir_float_v1_bf16_add_widened_rne" => (
            SoftFloatOperationV1::WidenedBinary(NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Add),
            2,
        ),
        "__fe2o3_ir_float_v1_bf16_sub_widened_rne" => (
            SoftFloatOperationV1::WidenedBinary(
                NarrowFloatFormat::Bf16,
                WidenedFloatBinaryOp::Subtract,
            ),
            2,
        ),
        "__fe2o3_ir_float_v1_bf16_mul_widened_rne" => (
            SoftFloatOperationV1::WidenedBinary(
                NarrowFloatFormat::Bf16,
                WidenedFloatBinaryOp::Multiply,
            ),
            2,
        ),
        "__fe2o3_ir_float_v1_bf16_div_widened_rne" => (
            SoftFloatOperationV1::WidenedBinary(
                NarrowFloatFormat::Bf16,
                WidenedFloatBinaryOp::Divide,
            ),
            2,
        ),
        "__fe2o3_ir_float_v1_sqrt_f32" => (SoftFloatOperationV1::F32Math(F32MathFunction::Sqrt), 1),
        "__fe2o3_ir_float_v1_fma_f32" => (
            SoftFloatOperationV1::F32Math(F32MathFunction::FusedMultiplyAdd),
            3,
        ),
        "__fe2o3_ir_float_v1_floor_f32" => {
            (SoftFloatOperationV1::F32Math(F32MathFunction::Floor), 1)
        }
        "__fe2o3_ir_float_v1_ceil_f32" => (SoftFloatOperationV1::F32Math(F32MathFunction::Ceil), 1),
        "__fe2o3_ir_float_v1_trunc_f32" => {
            (SoftFloatOperationV1::F32Math(F32MathFunction::Truncate), 1)
        }
        "__fe2o3_ir_float_v1_roundeven_f32" => (
            SoftFloatOperationV1::F32Math(F32MathFunction::RoundTiesEven),
            1,
        ),
        "__fe2o3_ir_float_v1_sin_f32" => (SoftFloatOperationV1::F32Math(F32MathFunction::Sin), 1),
        "__fe2o3_ir_float_v1_cos_f32" => (SoftFloatOperationV1::F32Math(F32MathFunction::Cos), 1),
        "__fe2o3_ir_float_v1_exp_f32" => (SoftFloatOperationV1::F32Math(F32MathFunction::Exp), 1),
        "__fe2o3_ir_float_v1_exp2_f32" => (SoftFloatOperationV1::F32Math(F32MathFunction::Exp2), 1),
        "__fe2o3_ir_float_v1_log_f32" => (SoftFloatOperationV1::F32Math(F32MathFunction::Ln), 1),
        "__fe2o3_ir_float_v1_log2_f32" => (SoftFloatOperationV1::F32Math(F32MathFunction::Log2), 1),
        "__fe2o3_ir_float_v1_log10_f32" => {
            (SoftFloatOperationV1::F32Math(F32MathFunction::Log10), 1)
        }
        "__fe2o3_ir_float_v1_fabs_f32" => {
            (SoftFloatOperationV1::F32Math(F32MathFunction::Abs), 1)
        }
        "__fe2o3_ir_float_v1_fma_bf16x2" => (SoftFloatOperationV1::Bf16x2FusedMultiplyAdd, 3),
        _ => return None,
    };
    (arguments.len() == arity).then_some(operation)
}

pub(crate) fn execute_unary_v1(
    op: UnaryOp,
    value: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SoftFloatErrorV1> {
    if op != UnaryOp::Negate || !value.ty().is_float() {
        return Err(SoftFloatErrorV1::InternalInvariant(
            "unsupported software-float unary operation",
        ));
    }
    let width = float_width(value.ty())?;
    scalar(value.ty(), value.bits() ^ (1_u128 << (width - 1)), target)
}

pub(crate) fn execute_binary_v1(
    op: BinaryOp,
    lhs: ScalarBitsV1,
    rhs: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SoftFloatErrorV1> {
    if lhs.ty() != rhs.ty() || !lhs.ty().is_float() {
        return Err(SoftFloatErrorV1::InternalInvariant(
            "mismatched software-float binary operands",
        ));
    }
    let bits = match lhs.ty() {
        ScalarType::F16 => binary_bits::<Half>(op, lhs.bits(), rhs.bits())?,
        ScalarType::Bf16 => binary_bits::<BFloat>(op, lhs.bits(), rhs.bits())?,
        ScalarType::F32 => binary_bits::<Single>(op, lhs.bits(), rhs.bits())?,
        ScalarType::F64 => binary_bits::<Double>(op, lhs.bits(), rhs.bits())?,
        _ => {
            return Err(SoftFloatErrorV1::InternalInvariant(
                "non-float software-float binary operand",
            ));
        }
    };
    scalar(lhs.ty(), bits, target)
}

pub(crate) fn execute_compare_v1(
    predicate: ComparePredicate,
    lhs: ScalarBitsV1,
    rhs: ScalarBitsV1,
) -> Result<bool, SoftFloatErrorV1> {
    if lhs.ty() != rhs.ty() || !lhs.ty().is_float() {
        return Err(SoftFloatErrorV1::InternalInvariant(
            "mismatched software-float compare operands",
        ));
    }
    match lhs.ty() {
        ScalarType::F16 => compare::<Half>(predicate, lhs.bits(), rhs.bits()),
        ScalarType::Bf16 => compare::<BFloat>(predicate, lhs.bits(), rhs.bits()),
        ScalarType::F32 => compare::<Single>(predicate, lhs.bits(), rhs.bits()),
        ScalarType::F64 => compare::<Double>(predicate, lhs.bits(), rhs.bits()),
        _ => Err(SoftFloatErrorV1::InternalInvariant(
            "non-float software-float compare operand",
        )),
    }
}

pub(crate) fn execute_cast_v1(
    kind: CastKind,
    value: ScalarBitsV1,
    to: ScalarType,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SoftFloatErrorV1> {
    let bits = match kind {
        CastKind::FloatExtend | CastKind::FloatTruncate => {
            convert_float_bits(value.ty(), value.bits(), to)?
        }
        CastKind::IntegerToFloat => integer_to_float_bits(value, to)?,
        CastKind::FloatToInteger => {
            return float_to_integer(value, to, target);
        }
        _ => {
            return Err(SoftFloatErrorV1::InternalInvariant(
                "non-float cast reached software-float evaluator",
            ));
        }
    };
    scalar(to, bits, target)
}

#[cfg(test)]
pub(crate) fn execute_operation_v1(
    operation: &FloatOperation,
    operands: &[ScalarBitsV1],
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SoftFloatErrorV1> {
    execute_compact_operation_v1(
        SoftFloatOperationV1::from_operation(operation),
        operands,
        target,
    )
}

pub(crate) fn execute_compact_operation_v1(
    operation: SoftFloatOperationV1,
    operands: &[ScalarBitsV1],
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SoftFloatErrorV1> {
    match operation {
        SoftFloatOperationV1::Convert(kind) => {
            require_arity(operands, 1)?;
            let to = match kind {
                FloatConversionKind::F16ToF32 | FloatConversionKind::Bf16ToF32 => ScalarType::F32,
                FloatConversionKind::F32ToF16RoundTiesEven => ScalarType::F16,
                FloatConversionKind::F32ToBf16RoundTiesEven => ScalarType::Bf16,
            };
            scalar(
                to,
                convert_float_bits(operands[0].ty(), operands[0].bits(), to)?,
                target,
            )
        }
        SoftFloatOperationV1::WidenedBinary(format, op) => {
            require_arity(operands, 2)?;
            let narrow = match format {
                NarrowFloatFormat::F16 => ScalarType::F16,
                NarrowFloatFormat::Bf16 => ScalarType::Bf16,
            };
            if operands.iter().any(|value| value.ty() != narrow) {
                return Err(SoftFloatErrorV1::InternalInvariant(
                    "widened float operands do not match their format",
                ));
            }
            let lhs = convert_float_bits(narrow, operands[0].bits(), ScalarType::F32)?;
            let rhs = convert_float_bits(narrow, operands[1].bits(), ScalarType::F32)?;
            let core = match op {
                WidenedFloatBinaryOp::Add => BinaryOp::Add,
                WidenedFloatBinaryOp::Subtract => BinaryOp::Subtract,
                WidenedFloatBinaryOp::Multiply => BinaryOp::Multiply,
                WidenedFloatBinaryOp::Divide => BinaryOp::Divide,
            };
            let widened = binary_bits::<Single>(core, lhs, rhs)?;
            scalar(
                narrow,
                convert_float_bits(ScalarType::F32, widened, narrow)?,
                target,
            )
        }
        SoftFloatOperationV1::F32Math(function) => execute_f32_math(function, operands, target),
        SoftFloatOperationV1::Bf16x2FusedMultiplyAdd => {
            require_arity(operands, 3)?;
            if operands.iter().any(|value| value.ty() != ScalarType::U32) {
                return Err(SoftFloatErrorV1::InternalInvariant(
                    "packed BF16 FMA operands are not u32",
                ));
            }
            let mut packed = 0_u128;
            for lane in 0..2 {
                let shift = lane * 16;
                let value = (operands[0].bits() >> shift) & 0xffff;
                let multiplier = (operands[1].bits() >> shift) & 0xffff;
                let addend = (operands[2].bits() >> shift) & 0xffff;
                let value = Single::from_bits(convert_bits::<BFloat, Single>(value));
                let multiplier = Single::from_bits(convert_bits::<BFloat, Single>(multiplier));
                let addend = Single::from_bits(convert_bits::<BFloat, Single>(addend));
                let result = value
                    .mul_add_r(multiplier, addend, Round::NearestTiesToEven)
                    .value;
                packed |= convert_bits::<Single, BFloat>(result.to_bits()) << shift;
            }
            scalar(ScalarType::U32, packed, target)
        }
    }
}

fn execute_f32_math(
    function: F32MathFunction,
    operands: &[ScalarBitsV1],
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SoftFloatErrorV1> {
    if operands.iter().any(|value| value.ty() != ScalarType::F32) {
        return Err(SoftFloatErrorV1::InternalInvariant(
            "f32 math operand has the wrong type",
        ));
    }
    let result = match function {
        F32MathFunction::FusedMultiplyAdd => {
            require_arity(operands, 3)?;
            Single::from_bits(operands[0].bits())
                .mul_add_r(
                    Single::from_bits(operands[1].bits()),
                    Single::from_bits(operands[2].bits()),
                    Round::NearestTiesToEven,
                )
                .value
        }
        F32MathFunction::Abs => {
            require_arity(operands, 1)?;
            Single::from_bits(operands[0].bits() & 0x7fff_ffff)
        }
        F32MathFunction::Floor
        | F32MathFunction::Ceil
        | F32MathFunction::Truncate
        | F32MathFunction::RoundTiesEven => {
            require_arity(operands, 1)?;
            let round = match function {
                F32MathFunction::Floor => Round::TowardNegative,
                F32MathFunction::Ceil => Round::TowardPositive,
                F32MathFunction::Truncate => Round::TowardZero,
                F32MathFunction::RoundTiesEven => Round::NearestTiesToEven,
                _ => unreachable!("matched integral rounding function"),
            };
            Single::from_bits(operands[0].bits())
                .round_to_integral(round)
                .value
        }
        F32MathFunction::Sqrt
        | F32MathFunction::Sin
        | F32MathFunction::Cos
        | F32MathFunction::Exp
        | F32MathFunction::Exp2
        | F32MathFunction::Ln
        | F32MathFunction::Log2
        | F32MathFunction::Log10 => {
            return Err(SoftFloatErrorV1::InternalInvariant(
                "unsupported float function passed preflight",
            ));
        }
    };
    scalar(ScalarType::F32, result.to_bits(), target)
}

fn binary_bits<F: Float>(op: BinaryOp, lhs: u128, rhs: u128) -> Result<u128, SoftFloatErrorV1> {
    let lhs = F::from_bits(lhs);
    let rhs = F::from_bits(rhs);
    let result = match op {
        BinaryOp::Add => lhs.add_r(rhs, Round::NearestTiesToEven),
        BinaryOp::Subtract => lhs.sub_r(rhs, Round::NearestTiesToEven),
        BinaryOp::Multiply => lhs.mul_r(rhs, Round::NearestTiesToEven),
        BinaryOp::Divide => lhs.div_r(rhs, Round::NearestTiesToEven),
        BinaryOp::Remainder => lhs.c_fmod(rhs),
        _ => {
            return Err(SoftFloatErrorV1::InternalInvariant(
                "unsupported software-float binary operation",
            ));
        }
    };
    Ok(result.value.to_bits())
}

fn compare<F: Float>(
    predicate: ComparePredicate,
    lhs: u128,
    rhs: u128,
) -> Result<bool, SoftFloatErrorV1> {
    let lhs = F::from_bits(lhs);
    let rhs = F::from_bits(rhs);
    let ordering = lhs.partial_cmp(&rhs);
    Ok(match predicate {
        ComparePredicate::Equal => ordering == Some(Ordering::Equal),
        ComparePredicate::NotEqual => ordering != Some(Ordering::Equal),
        ComparePredicate::LessThan => ordering == Some(Ordering::Less),
        ComparePredicate::LessThanOrEqual => {
            matches!(ordering, Some(Ordering::Less | Ordering::Equal))
        }
        ComparePredicate::GreaterThan => ordering == Some(Ordering::Greater),
        ComparePredicate::GreaterThanOrEqual => {
            matches!(ordering, Some(Ordering::Greater | Ordering::Equal))
        }
    })
}

fn convert_float_bits(
    from: ScalarType,
    bits: u128,
    to: ScalarType,
) -> Result<u128, SoftFloatErrorV1> {
    Ok(match (from, to) {
        (ScalarType::F16, ScalarType::F32) => convert_bits::<Half, Single>(bits),
        (ScalarType::F16, ScalarType::F64) => convert_bits::<Half, Double>(bits),
        (ScalarType::Bf16, ScalarType::F32) => convert_bits::<BFloat, Single>(bits),
        (ScalarType::Bf16, ScalarType::F64) => convert_bits::<BFloat, Double>(bits),
        (ScalarType::F32, ScalarType::F16) => convert_bits::<Single, Half>(bits),
        (ScalarType::F32, ScalarType::Bf16) => convert_bits::<Single, BFloat>(bits),
        (ScalarType::F32, ScalarType::F64) => convert_bits::<Single, Double>(bits),
        (ScalarType::F64, ScalarType::F16) => convert_bits::<Double, Half>(bits),
        (ScalarType::F64, ScalarType::Bf16) => convert_bits::<Double, BFloat>(bits),
        (ScalarType::F64, ScalarType::F32) => convert_bits::<Double, Single>(bits),
        _ => {
            return Err(SoftFloatErrorV1::InternalInvariant(
                "invalid software-float format conversion",
            ));
        }
    })
}

fn convert_bits<F, T>(bits: u128) -> u128
where
    F: Float + FloatConvert<T>,
    T: Float,
{
    let mut loses_info = false;
    F::from_bits(bits)
        .convert_r(Round::NearestTiesToEven, &mut loses_info)
        .value
        .to_bits()
}

fn integer_to_float_bits(value: ScalarBitsV1, to: ScalarType) -> Result<u128, SoftFloatErrorV1> {
    if !value.ty().is_integer() || value.ty() == ScalarType::Index || !to.is_float() {
        return Err(SoftFloatErrorV1::InternalInvariant(
            "invalid integer-to-float cast",
        ));
    }
    macro_rules! convert {
        ($float:ty) => {{
            if value.ty().is_signed_integer() {
                <$float>::from_i128(signed_integer(value)?).value.to_bits()
            } else {
                <$float>::from_u128(value.bits()).value.to_bits()
            }
        }};
    }
    Ok(match to {
        ScalarType::F16 => convert!(Half),
        ScalarType::Bf16 => convert!(BFloat),
        ScalarType::F32 => convert!(Single),
        ScalarType::F64 => convert!(Double),
        _ => {
            return Err(SoftFloatErrorV1::InternalInvariant(
                "integer-to-float target is not a float",
            ));
        }
    })
}

fn float_to_integer(
    value: ScalarBitsV1,
    to: ScalarType,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SoftFloatErrorV1> {
    if !value.ty().is_float() || !to.is_integer() || to == ScalarType::Index {
        return Err(SoftFloatErrorV1::InternalInvariant(
            "invalid float-to-integer cast",
        ));
    }
    let width = to.bit_width().ok_or(SoftFloatErrorV1::InternalInvariant(
        "fixed-width integer cast target",
    ))?;
    macro_rules! convert {
        ($float:ty) => {{
            let input = <$float>::from_bits(value.bits());
            let mut exact = false;
            if to.is_signed_integer() {
                let converted = input.to_i128_r(usize::from(width), Round::TowardZero, &mut exact);
                if converted.status.contains(Status::INVALID_OP) {
                    return Err(SoftFloatErrorV1::InvalidIntegerConversion);
                }
                converted.value as u128 & bit_mask(width)
            } else {
                let converted = input.to_u128_r(usize::from(width), Round::TowardZero, &mut exact);
                if converted.status.contains(Status::INVALID_OP) {
                    return Err(SoftFloatErrorV1::InvalidIntegerConversion);
                }
                converted.value & bit_mask(width)
            }
        }};
    }
    let bits = match value.ty() {
        ScalarType::F16 => convert!(Half),
        ScalarType::Bf16 => convert!(BFloat),
        ScalarType::F32 => convert!(Single),
        ScalarType::F64 => convert!(Double),
        _ => {
            return Err(SoftFloatErrorV1::InternalInvariant(
                "float-to-integer source is not a float",
            ));
        }
    };
    scalar(to, bits, target)
}

fn signed_integer(value: ScalarBitsV1) -> Result<i128, SoftFloatErrorV1> {
    let width = value
        .ty()
        .bit_width()
        .ok_or(SoftFloatErrorV1::InternalInvariant(
            "fixed-width signed integer",
        ))?;
    if width == 128 {
        return Ok(value.bits() as i128);
    }
    let shift = 128 - width;
    Ok(((value.bits() << shift) as i128) >> shift)
}

fn scalar(
    ty: ScalarType,
    bits: u128,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SoftFloatErrorV1> {
    ScalarBitsV1::new(ty, bits, target)
        .map_err(|_| SoftFloatErrorV1::InternalInvariant("software-float result bits"))
}

fn float_width(ty: ScalarType) -> Result<u16, SoftFloatErrorV1> {
    ty.bit_width()
        .filter(|_| ty.is_float())
        .ok_or(SoftFloatErrorV1::InternalInvariant(
            "software-float type width",
        ))
}

const fn bit_mask(width: u16) -> u128 {
    if width == 128 {
        u128::MAX
    } else {
        (1_u128 << width) - 1
    }
}

fn require_arity(values: &[ScalarBitsV1], expected: usize) -> Result<(), SoftFloatErrorV1> {
    if values.len() == expected {
        Ok(())
    } else {
        Err(SoftFloatErrorV1::InternalInvariant(
            "software-float operation arity",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::FunctionId;

    const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

    fn value(ty: ScalarType, bits: u128) -> ScalarBitsV1 {
        ScalarBitsV1::new(ty, bits, TARGET).unwrap()
    }

    fn operation(name: &str) -> FloatOperation {
        FloatOperation::from_intrinsic_id(&FunctionId::new(name)).unwrap()
    }

    #[test]
    fn compact_call_index_matches_every_canonical_float_operation_and_arity() {
        for name in [
            "__fe2o3_ir_float_v1_f16_to_f32",
            "__fe2o3_ir_float_v1_f32_to_f16_rne",
            "__fe2o3_ir_float_v1_bf16_to_f32",
            "__fe2o3_ir_float_v1_f32_to_bf16_rne",
            "__fe2o3_ir_float_v1_f16_add_widened_rne",
            "__fe2o3_ir_float_v1_f16_sub_widened_rne",
            "__fe2o3_ir_float_v1_f16_mul_widened_rne",
            "__fe2o3_ir_float_v1_f16_div_widened_rne",
            "__fe2o3_ir_float_v1_bf16_add_widened_rne",
            "__fe2o3_ir_float_v1_bf16_sub_widened_rne",
            "__fe2o3_ir_float_v1_bf16_mul_widened_rne",
            "__fe2o3_ir_float_v1_bf16_div_widened_rne",
            "__fe2o3_ir_float_v1_sqrt_f32",
            "__fe2o3_ir_float_v1_fma_f32",
            "__fe2o3_ir_float_v1_floor_f32",
            "__fe2o3_ir_float_v1_ceil_f32",
            "__fe2o3_ir_float_v1_trunc_f32",
            "__fe2o3_ir_float_v1_roundeven_f32",
            "__fe2o3_ir_float_v1_sin_f32",
            "__fe2o3_ir_float_v1_cos_f32",
            "__fe2o3_ir_float_v1_exp_f32",
            "__fe2o3_ir_float_v1_exp2_f32",
            "__fe2o3_ir_float_v1_log_f32",
            "__fe2o3_ir_float_v1_log2_f32",
            "__fe2o3_ir_float_v1_log10_f32",
            "__fe2o3_ir_float_v1_fma_bf16x2",
        ] {
            let operation = operation(name);
            let arguments = (0..operation.parameter_types().len())
                .map(|index| ValueId(u32::try_from(index).unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(
                operation_for_call_v1(&FunctionId::new(name), &arguments),
                Some(SoftFloatOperationV1::from_operation(&operation)),
                "{name}"
            );
            assert_eq!(
                operation_for_call_v1(&FunctionId::new(name), &[]),
                None,
                "{name} rejects wrong arity"
            );
        }
        assert_eq!(
            operation_for_call_v1(&FunctionId::new("__fe2o3_ir_float_v1_unknown"), &[]),
            None
        );
    }

    #[test]
    fn every_format_preserves_edges_and_uses_ieee_rne_arithmetic() {
        let formats = [
            (
                ScalarType::F16,
                0x3c00,
                0x4000,
                0x4200,
                0x4400,
                0x4580,
                0x3e00,
                0x7c00,
                0xfc00,
                0x7c01,
                0x7e01,
                0x8000,
                0x03ff,
                0x7bff,
                0x1000,
            ),
            (
                ScalarType::Bf16,
                0x3f80,
                0x4000,
                0x4040,
                0x4080,
                0x40b0,
                0x3fc0,
                0x7f80,
                0xff80,
                0x7f81,
                0x7fc1,
                0x8000,
                0x007f,
                0x7f7f,
                0x3b80,
            ),
            (
                ScalarType::F32,
                0x3f80_0000,
                0x4000_0000,
                0x4040_0000,
                0x4080_0000,
                0x40b0_0000,
                0x3fc0_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7f80_0001,
                0x7fc0_0001,
                0x8000_0000,
                0x007f_ffff,
                0x7f7f_ffff,
                0x3380_0000,
            ),
            (
                ScalarType::F64,
                0x3ff0_0000_0000_0000,
                0x4000_0000_0000_0000,
                0x4008_0000_0000_0000,
                0x4010_0000_0000_0000,
                0x4016_0000_0000_0000,
                0x3ff8_0000_0000_0000,
                0x7ff0_0000_0000_0000,
                0xfff0_0000_0000_0000,
                0x7ff0_0000_0000_0001,
                0x7ff8_0000_0000_0001,
                0x8000_0000_0000_0000,
                0x000f_ffff_ffff_ffff,
                0x7fef_ffff_ffff_ffff,
                0x3ca0_0000_0000_0000,
            ),
        ];
        for (
            ty,
            one,
            two,
            three,
            four,
            five_half,
            one_half,
            infinity,
            negative_infinity,
            signaling_nan,
            quiet_nan,
            sign,
            max_subnormal,
            max_finite,
            half_ulp_at_one,
        ) in formats
        {
            assert_eq!(
                execute_binary_v1(BinaryOp::Add, value(ty, one), value(ty, one), TARGET)
                    .unwrap()
                    .bits(),
                two,
                "{ty:?} add"
            );
            for (op, lhs, rhs, expected) in [
                (BinaryOp::Subtract, two, one, one),
                (BinaryOp::Multiply, two, two, four),
                (BinaryOp::Divide, two, two, one),
                (BinaryOp::Remainder, five_half, two, one_half),
            ] {
                assert_eq!(
                    execute_binary_v1(op, value(ty, lhs), value(ty, rhs), TARGET)
                        .unwrap()
                        .bits(),
                    expected,
                    "{ty:?} {op:?}"
                );
            }
            assert_eq!(
                execute_binary_v1(BinaryOp::Add, value(ty, one), value(ty, two), TARGET)
                    .unwrap()
                    .bits(),
                three,
                "{ty:?} three"
            );
            assert_eq!(
                execute_binary_v1(BinaryOp::Add, value(ty, 1), value(ty, 1), TARGET)
                    .unwrap()
                    .bits(),
                2,
                "{ty:?} subnormal"
            );
            assert_eq!(
                execute_binary_v1(
                    BinaryOp::Add,
                    value(ty, max_subnormal),
                    value(ty, 0),
                    TARGET
                )
                .unwrap()
                .bits(),
                max_subnormal,
                "{ty:?} maximum subnormal"
            );
            assert_eq!(
                execute_binary_v1(
                    BinaryOp::Add,
                    value(ty, one),
                    value(ty, half_ulp_at_one),
                    TARGET
                )
                .unwrap()
                .bits(),
                one,
                "{ty:?} RNE halfway value"
            );
            assert_eq!(
                execute_binary_v1(
                    BinaryOp::Multiply,
                    value(ty, max_finite),
                    value(ty, two),
                    TARGET
                )
                .unwrap()
                .bits(),
                infinity,
                "{ty:?} overflow"
            );
            assert_eq!(
                execute_binary_v1(BinaryOp::Divide, value(ty, 1), value(ty, two), TARGET)
                    .unwrap()
                    .bits(),
                0,
                "{ty:?} underflow tie rounds to even zero"
            );
            assert_eq!(
                execute_binary_v1(BinaryOp::Divide, value(ty, one), value(ty, 0), TARGET,)
                    .unwrap()
                    .bits(),
                infinity,
                "{ty:?} division by zero"
            );
            let invalid = execute_binary_v1(
                BinaryOp::Add,
                value(ty, infinity),
                value(ty, negative_infinity),
                TARGET,
            )
            .unwrap();
            assert_eq!(invalid.bits(), quiet_nan & !1, "{ty:?} invalid quiet NaN");
            assert_eq!(
                execute_binary_v1(
                    BinaryOp::Add,
                    value(ty, signaling_nan),
                    value(ty, one),
                    TARGET,
                )
                .unwrap()
                .bits(),
                quiet_nan,
                "{ty:?} signaling NaN quieting"
            );
            assert_eq!(
                execute_binary_v1(BinaryOp::Add, value(ty, quiet_nan), value(ty, one), TARGET)
                    .unwrap()
                    .bits(),
                quiet_nan,
                "{ty:?} quiet NaN payload"
            );
            assert_eq!(
                execute_unary_v1(UnaryOp::Negate, value(ty, quiet_nan), TARGET)
                    .unwrap()
                    .bits(),
                quiet_nan ^ sign,
                "{ty:?} NaN sign/payload"
            );
            assert_eq!(
                execute_unary_v1(UnaryOp::Negate, value(ty, 0), TARGET)
                    .unwrap()
                    .bits(),
                sign,
                "{ty:?} signed zero"
            );
            assert_eq!(
                execute_binary_v1(BinaryOp::Add, value(ty, sign), value(ty, sign), TARGET)
                    .unwrap()
                    .bits(),
                sign,
                "{ty:?} negative zero addition"
            );
            assert_eq!(
                execute_binary_v1(BinaryOp::Add, value(ty, 0), value(ty, sign), TARGET)
                    .unwrap()
                    .bits(),
                0,
                "{ty:?} opposite signed zero addition"
            );
        }
    }

    #[test]
    fn comparisons_are_ordered_except_not_equal_is_unordered() {
        for (ty, sign, one, quiet_nan, signaling_nan) in [
            (ScalarType::F16, 0x8000, 0x3c00, 0x7e42, 0x7c42),
            (ScalarType::Bf16, 0x8000, 0x3f80, 0x7fc2, 0x7f82),
            (
                ScalarType::F32,
                0x8000_0000,
                0x3f80_0000,
                0x7fc0_0042,
                0x7f80_0042,
            ),
            (
                ScalarType::F64,
                0x8000_0000_0000_0000,
                0x3ff0_0000_0000_0000,
                0x7ff8_0000_0000_0042,
                0x7ff0_0000_0000_0042,
            ),
        ] {
            let zero = value(ty, 0);
            let negative_zero = value(ty, sign);
            let one = value(ty, one);
            for nan in [quiet_nan, signaling_nan] {
                for predicate in [
                    ComparePredicate::Equal,
                    ComparePredicate::NotEqual,
                    ComparePredicate::LessThan,
                    ComparePredicate::LessThanOrEqual,
                    ComparePredicate::GreaterThan,
                    ComparePredicate::GreaterThanOrEqual,
                ] {
                    let expected_nan = predicate == ComparePredicate::NotEqual;
                    assert_eq!(
                        execute_compare_v1(predicate, value(ty, nan), one).unwrap(),
                        expected_nan,
                        "{ty:?} {predicate:?} NaN"
                    );
                }
            }
            assert!(execute_compare_v1(ComparePredicate::Equal, zero, negative_zero).unwrap());
            assert!(execute_compare_v1(ComparePredicate::LessThan, zero, one).unwrap());
            assert!(execute_compare_v1(ComparePredicate::GreaterThanOrEqual, one, one).unwrap());
        }
    }

    #[test]
    fn format_and_integer_casts_have_exact_ties_and_fail_closed_ranges() {
        for (to, input, expected) in [
            (ScalarType::F16, 0x3f80_1000, 0x3c00),
            (ScalarType::F16, 0x3f80_3000, 0x3c02),
            (ScalarType::Bf16, 0x3f80_8000, 0x3f80),
            (ScalarType::Bf16, 0x3f81_8000, 0x3f82),
        ] {
            assert_eq!(
                execute_cast_v1(
                    CastKind::FloatTruncate,
                    value(ScalarType::F32, input),
                    to,
                    TARGET,
                )
                .unwrap()
                .bits(),
                expected
            );
        }
        assert_eq!(
            execute_cast_v1(
                CastKind::FloatTruncate,
                value(ScalarType::F64, 0x3ff0_0000_1000_0000),
                ScalarType::F32,
                TARGET,
            )
            .unwrap()
            .bits(),
            0x3f80_0000
        );
        for (from, input, to, expected) in [
            (ScalarType::F16, 0x7e42, ScalarType::F32, 0x7fc8_4000),
            (ScalarType::F16, 0x7c42, ScalarType::F32, 0x7fc8_4000),
            (ScalarType::Bf16, 0x7fc2, ScalarType::F32, 0x7fc2_0000),
            (ScalarType::Bf16, 0x7f82, ScalarType::F32, 0x7fc2_0000),
            (ScalarType::F32, 0x7fc8_4000, ScalarType::F16, 0x7e42),
            (ScalarType::F32, 0x7fc2_0000, ScalarType::Bf16, 0x7fc2),
            (
                ScalarType::F32,
                0x7fc0_0042,
                ScalarType::F64,
                0x7ff8_0008_4000_0000,
            ),
            (
                ScalarType::F32,
                0x7f80_0042,
                ScalarType::F64,
                0x7ff8_0008_4000_0000,
            ),
            (
                ScalarType::F64,
                0x7ff8_0008_4000_0000,
                ScalarType::F32,
                0x7fc0_0042,
            ),
            (
                ScalarType::F64,
                0x7ff0_0008_4000_0000,
                ScalarType::F32,
                0x7fc0_0042,
            ),
        ] {
            let kind = if from.bit_width().unwrap() < to.bit_width().unwrap() {
                CastKind::FloatExtend
            } else {
                CastKind::FloatTruncate
            };
            assert_eq!(
                execute_cast_v1(kind, value(from, input), to, TARGET)
                    .unwrap()
                    .bits(),
                expected,
                "{from:?} NaN payload to {to:?}"
            );
        }
        assert_eq!(
            execute_cast_v1(
                CastKind::IntegerToFloat,
                value(ScalarType::U32, 16_777_217),
                ScalarType::F32,
                TARGET,
            )
            .unwrap()
            .bits(),
            0x4b80_0000
        );
        assert_eq!(
            execute_cast_v1(
                CastKind::IntegerToFloat,
                value(ScalarType::I32, (-3_i32) as u32 as u128),
                ScalarType::F32,
                TARGET,
            )
            .unwrap()
            .bits(),
            0xc040_0000
        );
        assert_eq!(
            execute_cast_v1(
                CastKind::FloatToInteger,
                value(ScalarType::F32, 0xbff3_3333),
                ScalarType::I32,
                TARGET,
            )
            .unwrap()
            .bits(),
            u128::from(u32::MAX)
        );
        for bits in [0x7fc0_0000, 0x7f80_0000, 0x4f00_0000] {
            assert_eq!(
                execute_cast_v1(
                    CastKind::FloatToInteger,
                    value(ScalarType::F32, bits),
                    ScalarType::I32,
                    TARGET,
                ),
                Err(SoftFloatErrorV1::InvalidIntegerConversion)
            );
        }
        for (bits, to) in [(0xc301_0000, ScalarType::I8), (0x4380_0000, ScalarType::U8)] {
            assert_eq!(
                execute_cast_v1(
                    CastKind::FloatToInteger,
                    value(ScalarType::F32, bits),
                    to,
                    TARGET,
                ),
                Err(SoftFloatErrorV1::InvalidIntegerConversion),
                "finite F32 outside {to:?} range"
            );
        }

        let float_formats = [
            (ScalarType::F16, 0x3c00, 0xbc00),
            (ScalarType::Bf16, 0x3f80, 0xbf80),
            (ScalarType::F32, 0x3f80_0000, 0xbf80_0000),
            (
                ScalarType::F64,
                0x3ff0_0000_0000_0000,
                0xbff0_0000_0000_0000,
            ),
        ];
        for (from, input, _) in float_formats {
            for (to, expected, _) in float_formats {
                let from_width = from.bit_width().unwrap();
                let to_width = to.bit_width().unwrap();
                if from_width == to_width {
                    continue;
                }
                let kind = if from_width < to_width {
                    CastKind::FloatExtend
                } else {
                    CastKind::FloatTruncate
                };
                assert_eq!(
                    execute_cast_v1(kind, value(from, input), to, TARGET)
                        .unwrap()
                        .bits(),
                    expected,
                    "{from:?} to {to:?}"
                );
                let sign = 1_u128 << (to_width - 1);
                assert_eq!(
                    execute_cast_v1(kind, value(from, 1_u128 << (from_width - 1)), to, TARGET,)
                        .unwrap()
                        .bits(),
                    sign,
                    "{from:?} negative zero to {to:?}"
                );
            }
        }

        for integer in [
            ScalarType::I8,
            ScalarType::I16,
            ScalarType::I32,
            ScalarType::I64,
            ScalarType::I128,
            ScalarType::U8,
            ScalarType::U16,
            ScalarType::U32,
            ScalarType::U64,
            ScalarType::U128,
        ] {
            let integer_bits = if integer.is_signed_integer() {
                bit_mask(integer.bit_width().unwrap())
            } else {
                1
            };
            for (float, positive_one, negative_one) in float_formats {
                assert_eq!(
                    execute_cast_v1(
                        CastKind::IntegerToFloat,
                        value(integer, integer_bits),
                        float,
                        TARGET,
                    )
                    .unwrap()
                    .bits(),
                    if integer.is_signed_integer() {
                        negative_one
                    } else {
                        positive_one
                    },
                    "{integer:?} to {float:?}"
                );
                assert_eq!(
                    execute_cast_v1(
                        CastKind::FloatToInteger,
                        value(float, positive_one),
                        integer,
                        TARGET,
                    )
                    .unwrap()
                    .bits(),
                    1,
                    "{float:?} to {integer:?}"
                );
                if !integer.is_signed_integer() {
                    assert_eq!(
                        execute_cast_v1(
                            CastKind::FloatToInteger,
                            value(float, negative_one),
                            integer,
                            TARGET,
                        ),
                        Err(SoftFloatErrorV1::InvalidIntegerConversion),
                        "negative {float:?} to {integer:?}"
                    );
                }
            }
        }

        for (float, infinity) in [
            (ScalarType::F16, 0x7c00),
            (ScalarType::Bf16, 0x7f80),
            (ScalarType::F32, 0x7f80_0000),
            (ScalarType::F64, 0x7ff0_0000_0000_0000),
        ] {
            for integer in [ScalarType::I8, ScalarType::U8] {
                assert_eq!(
                    execute_cast_v1(
                        CastKind::FloatToInteger,
                        value(float, infinity),
                        integer,
                        TARGET,
                    ),
                    Err(SoftFloatErrorV1::InvalidIntegerConversion),
                    "{float:?} infinity to {integer:?}"
                );
            }
        }
    }

    #[test]
    fn canonical_float_operations_widen_fuse_round_and_pack_exactly() {
        let f16_to_f32 = operation("__fe2o3_ir_float_v1_f16_to_f32");
        assert_eq!(
            execute_operation_v1(&f16_to_f32, &[value(ScalarType::F16, 0x3c01)], TARGET,)
                .unwrap()
                .bits(),
            0x3f80_2000
        );
        let widened = operation("__fe2o3_ir_float_v1_bf16_add_widened_rne");
        assert_eq!(
            execute_operation_v1(
                &widened,
                &[
                    value(ScalarType::Bf16, 0x3f80),
                    value(ScalarType::Bf16, 0x3f80),
                ],
                TARGET,
            )
            .unwrap()
            .bits(),
            0x4000
        );
        let fma = operation("__fe2o3_ir_float_v1_fma_f32");
        assert_eq!(
            execute_operation_v1(
                &fma,
                &[
                    value(ScalarType::F32, 0x4168_0000),
                    value(ScalarType::F32, 0xc168_0000),
                    value(ScalarType::F32, 0x4361_0000),
                ],
                TARGET,
            )
            .unwrap()
            .bits(),
            0x416c_0000
        );
        for (name, input, expected) in [
            ("__fe2o3_ir_float_v1_floor_f32", 0xbfc0_0000, 0xc000_0000),
            ("__fe2o3_ir_float_v1_ceil_f32", 0xbfc0_0000, 0xbf80_0000),
            ("__fe2o3_ir_float_v1_trunc_f32", 0xbfc0_0000, 0xbf80_0000),
            (
                "__fe2o3_ir_float_v1_roundeven_f32",
                0x4020_0000,
                0x4000_0000,
            ),
        ] {
            assert_eq!(
                execute_operation_v1(&operation(name), &[value(ScalarType::F32, input)], TARGET,)
                    .unwrap()
                    .bits(),
                expected,
                "{name}"
            );
        }
        let packed = operation("__fe2o3_ir_float_v1_fma_bf16x2");
        assert_eq!(
            execute_operation_v1(
                &packed,
                &[
                    value(ScalarType::U32, 0x3f80_3f80),
                    value(ScalarType::U32, 0x4000_4000),
                    value(ScalarType::U32, 0x3f80_3f80),
                ],
                TARGET,
            )
            .unwrap()
            .bits(),
            0x4040_4040
        );
    }
}
