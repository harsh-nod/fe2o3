//! Exact, bounded scalar-operation lowering for the gfx942 profile.

use std::fmt::{self, Write as _};

use fe2o3_kernel_ir::Type;
use fe2o3_kernel_ir::scalar_ops_v2::{
    Cast, FloatBinary, FloatComparisonPolicy, FloatWidth, GFX942_FLOAT_CAPABILITIES, IntBinary,
    IntMode, IntUnary, IntWidth, Operation, Predicate, ScalarOperationV2, ScalarType,
    ShiftDirection, ShiftPolicy, verify,
};

pub const MAX_GFX942_SCALAR_LLVM_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarV2LoweringError {
    InvalidContract(String),
    Unsupported(String),
    ResourceLimit { limit: usize, actual: usize },
}

impl fmt::Display for ScalarV2LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract(message) | Self::Unsupported(message) => {
                formatter.write_str(message)
            }
            Self::ResourceLimit { limit, actual } => write!(
                formatter,
                "gfx942 scalar LLVM exceeds {limit} bytes (would emit {actual})"
            ),
        }
    }
}

impl std::error::Error for ScalarV2LoweringError {}

/// Lowers one admitted scalar carrier to a complete deterministic LLVM module.
///
/// This function intentionally has no target parameter: it denotes only the
/// exact gfx942 strict floating-point profile. Selection of any other target
/// must fail before this lowering entry point is called.
pub fn lower_scalar_v2_to_gfx942_llvm(
    scalar: &ScalarOperationV2,
) -> Result<String, ScalarV2LoweringError> {
    let operation = scalar.operation();
    verify(operation, GFX942_FLOAT_CAPABILITIES).map_err(|diagnostics| {
        ScalarV2LoweringError::InvalidContract(format!(
            "invalid scalar V2 contract: {diagnostics:?}"
        ))
    })?;
    reject_unsupported(operation)?;

    let operands = scalar.operand_types();
    let results = scalar.result_types();
    let mut output = String::with_capacity(4096);
    writeln!(output, "target triple = \"amdgcn-amd-amdhsa\"\n").unwrap();
    emit_declarations(&mut output, operation);
    if needs_declaration(operation) {
        writeln!(output).unwrap();
    }
    write!(
        output,
        "define {} @{}(",
        llvm_results(&results),
        scalar.intrinsic_function_id()
    )
    .unwrap();
    for (index, ty) in operands.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        write!(output, "{} %arg{index}", llvm_ir_type(ty)?).unwrap();
    }
    writeln!(output, ") #0 {{\nentry:").unwrap();
    emit_operation(&mut output, operation, &results)?;
    writeln!(output, "}}\n").unwrap();
    writeln!(
        output,
        "attributes #0 = {{ nounwind \"target-cpu\"=\"gfx942\" \"denormal-fp-math\"=\"ieee,ieee\" \"denormal-fp-math-f32\"=\"ieee,ieee\" \"unsafe-fp-math\"=\"false\" \"no-infs-fp-math\"=\"false\" \"no-nans-fp-math\"=\"false\" \"no-signed-zeros-fp-math\"=\"false\" \"approx-func-fp-math\"=\"false\" }}"
    )
    .unwrap();
    if output.len() > MAX_GFX942_SCALAR_LLVM_BYTES {
        return Err(ScalarV2LoweringError::ResourceLimit {
            limit: MAX_GFX942_SCALAR_LLVM_BYTES,
            actual: output.len(),
        });
    }
    Ok(output)
}

fn reject_unsupported(operation: Operation) -> Result<(), ScalarV2LoweringError> {
    match operation {
        Operation::Cast {
            cast: Cast::PointerAddressSpace | Cast::PointerToInt { .. } | Cast::IntToPointer { .. },
            ..
        } => Err(ScalarV2LoweringError::Unsupported(
            "gfx942 scalar V2 rejects pointer and provenance casts".to_owned(),
        )),
        Operation::Cast {
            cast: Cast::Bitcast,
            from: ScalarType::Float(_),
            to: ScalarType::Float(_),
        } => Err(ScalarV2LoweringError::Unsupported(
            "float-to-float bitcast is not an intentional scalar V2 surface".to_owned(),
        )),
        _ => Ok(()),
    }
}

fn needs_declaration(operation: Operation) -> bool {
    match operation {
        Operation::IntegerBinary { op, mode, .. } => {
            matches!(op, IntBinary::Add | IntBinary::Sub | IntBinary::Mul)
                && mode != IntMode::Wrapping
                || matches!(op, IntBinary::Div | IntBinary::Rem) && mode != IntMode::Checked
        }
        Operation::IntegerUnary {
            op: IntUnary::Neg,
            mode,
            ..
        } => mode != IntMode::Wrapping,
        Operation::Shift {
            policy: ShiftPolicy::RustOperator {
                overflow_checks: true,
            },
            ..
        } => true,
        Operation::FloatBinary { op, .. } => op != FloatBinary::Rem,
        Operation::Cast {
            cast: Cast::FloatToInt { .. },
            to: ScalarType::Int { width, .. },
            ..
        } => width != IntWidth::W128,
        Operation::Cast {
            cast: Cast::IntToFloat { .. },
            from:
                ScalarType::Int {
                    width: IntWidth::W128,
                    ..
                },
            ..
        } => true,
        _ => false,
    }
}

fn emit_declarations(output: &mut String, operation: Operation) {
    match operation {
        Operation::IntegerBinary { ty, op, mode }
            if matches!(op, IntBinary::Add | IntBinary::Sub | IntBinary::Mul) =>
        {
            let (width, signed) = integer_parts(ty);
            let stem = integer_stem(op);
            match mode {
                IntMode::Checked | IntMode::Overflowing => writeln!(
                    output,
                    "declare {{ i{width}, i1 }} @llvm.{}{}.with.overflow.i{width}(i{width}, i{width})",
                    signed_prefix(signed), stem
                )
                .unwrap(),
                IntMode::Saturating => writeln!(
                    output,
                    "declare i{width} @llvm.{}{}.sat.i{width}(i{width}, i{width})",
                    signed_prefix(signed), stem
                )
                .unwrap(),
                IntMode::Wrapping => {}
            }
        }
        Operation::IntegerBinary { op, mode, .. }
            if matches!(op, IntBinary::Div | IntBinary::Rem) && mode != IntMode::Checked =>
        {
            writeln!(output, "declare void @llvm.trap()").unwrap();
        }
        Operation::IntegerUnary {
            ty,
            op: IntUnary::Neg,
            mode,
        } => {
            let (width, _) = integer_parts(ty);
            match mode {
                IntMode::Checked | IntMode::Overflowing => writeln!(
                    output,
                    "declare {{ i{width}, i1 }} @llvm.ssub.with.overflow.i{width}(i{width}, i{width})"
                )
                .unwrap(),
                IntMode::Saturating => writeln!(
                    output,
                    "declare i{width} @llvm.ssub.sat.i{width}(i{width}, i{width})"
                )
                .unwrap(),
                IntMode::Wrapping => {}
            }
        }
        Operation::Shift {
            policy: ShiftPolicy::RustOperator {
                overflow_checks: true,
            },
            ..
        } => writeln!(output, "declare void @llvm.trap()").unwrap(),
        Operation::FloatBinary { ty, op, .. } if op != FloatBinary::Rem => {
            let llvm = float_llvm_type(ty);
            writeln!(
                output,
                "declare {llvm} @llvm.experimental.constrained.{}.{}({llvm}, {llvm}, metadata, metadata)",
                float_stem(op),
                float_suffix(ty)
            )
            .unwrap();
        }
        Operation::Cast {
            from: ScalarType::Float(float),
            to: ScalarType::Int { width, signed },
            cast: Cast::FloatToInt { .. },
        } if width != IntWidth::W128 => writeln!(
            output,
            "declare i{} @llvm.fp{}i.sat.i{}.f{}({})",
            width.bits(),
            if signed { "tos" } else { "tou" },
            width.bits(),
            float.bits(),
            float_llvm_width(float)
        )
        .unwrap(),
        Operation::Cast {
            from:
                ScalarType::Int {
                    width: IntWidth::W128,
                    ..
                },
            cast: Cast::IntToFloat { .. },
            ..
        } => writeln!(output, "declare i64 @llvm.ctlz.i64(i64, i1 immarg)").unwrap(),
        _ => {}
    }
}

fn emit_operation(
    output: &mut String,
    operation: Operation,
    results: &[Type],
) -> Result<(), ScalarV2LoweringError> {
    match operation {
        Operation::IntegerBinary { ty, op, mode } => {
            emit_integer_binary(output, ty, op, mode, results)
        }
        Operation::IntegerUnary { ty, op, mode } => {
            emit_integer_unary(output, ty, op, mode, results)
        }
        Operation::Shift {
            ty,
            rhs_ty,
            direction,
            policy,
        } => emit_shift(output, ty, rhs_ty, direction, policy, results),
        Operation::IntegerCompare { ty, predicate } => {
            let (width, signed) = integer_parts(ty);
            writeln!(
                output,
                "  %result = icmp {} i{width} %arg0, %arg1",
                integer_predicate(predicate, signed)
            )
            .unwrap();
            writeln!(output, "  ret i1 %result").unwrap();
            Ok(())
        }
        Operation::FloatBinary { ty, op, .. } => {
            let llvm = float_llvm_type(ty);
            if op == FloatBinary::Rem {
                writeln!(output, "  %result = frem {llvm} %arg0, %arg1").unwrap();
            } else {
                writeln!(
                    output,
                    "  %result = call {llvm} @llvm.experimental.constrained.{}.{}({llvm} %arg0, {llvm} %arg1, metadata !\"round.tonearest\", metadata !\"fpexcept.ignore\")",
                    float_stem(op),
                    float_suffix(ty)
                )
                .unwrap();
            }
            writeln!(output, "  ret {llvm} %result").unwrap();
            Ok(())
        }
        Operation::FloatNeg { ty, .. } => emit_float_neg(output, ty),
        Operation::FloatCompare {
            ty,
            predicate,
            policy,
        } => emit_float_compare(output, ty, predicate, policy),
        Operation::FloatTotalCompare { ty } => emit_total_cmp(output, ty),
        Operation::Cast { from, to, cast } => emit_cast(output, from, to, cast, results),
    }
}

fn emit_integer_binary(
    output: &mut String,
    ty: ScalarType,
    op: IntBinary,
    mode: IntMode,
    results: &[Type],
) -> Result<(), ScalarV2LoweringError> {
    let (width, signed) = integer_parts(ty);
    if matches!(op, IntBinary::And | IntBinary::Or | IntBinary::Xor) {
        writeln!(
            output,
            "  %result = {} i{width} %arg0, %arg1",
            integer_stem(op)
        )
        .unwrap();
        writeln!(output, "  ret i{width} %result").unwrap();
        return Ok(());
    }
    if matches!(op, IntBinary::Div | IntBinary::Rem) {
        return emit_div_rem(output, width, signed, op, mode, results);
    }
    let stem = integer_stem(op);
    match mode {
        IntMode::Wrapping => {
            writeln!(output, "  %result = {stem} i{width} %arg0, %arg1").unwrap();
            writeln!(output, "  ret i{width} %result").unwrap();
        }
        IntMode::Checked | IntMode::Overflowing => {
            writeln!(
                output,
                "  %pair = call {{ i{width}, i1 }} @llvm.{}{}.with.overflow.i{width}(i{width} %arg0, i{width} %arg1)",
                signed_prefix(signed), stem
            )
            .unwrap();
            writeln!(
                output,
                "  %value = extractvalue {{ i{width}, i1 }} %pair, 0"
            )
            .unwrap();
            writeln!(
                output,
                "  %overflow = extractvalue {{ i{width}, i1 }} %pair, 1"
            )
            .unwrap();
            let flag = if mode == IntMode::Checked {
                writeln!(output, "  %valid = xor i1 %overflow, true").unwrap();
                "%valid"
            } else {
                "%overflow"
            };
            emit_pair_return(output, results, "%value", flag)?;
        }
        IntMode::Saturating => {
            writeln!(
                output,
                "  %result = call i{width} @llvm.{}{}.sat.i{width}(i{width} %arg0, i{width} %arg1)",
                signed_prefix(signed),
                stem
            )
            .unwrap();
            writeln!(output, "  ret i{width} %result").unwrap();
        }
    }
    Ok(())
}

fn emit_div_rem(
    output: &mut String,
    width: u16,
    signed: bool,
    op: IntBinary,
    mode: IntMode,
    results: &[Type],
) -> Result<(), ScalarV2LoweringError> {
    if width == 128 {
        return emit_i128_div_rem(output, signed, op, mode, results);
    }
    let instruction = match (op, signed) {
        (IntBinary::Div, true) => "sdiv",
        (IntBinary::Div, false) => "udiv",
        (IntBinary::Rem, true) => "srem",
        (IntBinary::Rem, false) => "urem",
        _ => unreachable!(),
    };
    writeln!(output, "  %zero = icmp eq i{width} %arg1, 0").unwrap();
    if signed {
        writeln!(
            output,
            "  %is.min = icmp eq i{width} %arg0, {}",
            signed_min(width)
        )
        .unwrap();
        writeln!(output, "  %is.neg.one = icmp eq i{width} %arg1, -1").unwrap();
        writeln!(output, "  %range = and i1 %is.min, %is.neg.one").unwrap();
        writeln!(output, "  %invalid = or i1 %zero, %range").unwrap();
    } else {
        writeln!(output, "  %range = icmp eq i1 false, true").unwrap();
        writeln!(output, "  %invalid = or i1 %zero, false").unwrap();
    }
    if mode != IntMode::Checked {
        writeln!(output, "  br i1 %zero, label %trap, label %compute\ntrap:").unwrap();
        writeln!(output, "  call void @llvm.trap()\n  unreachable\ncompute:").unwrap();
    }
    writeln!(
        output,
        "  %safe.zero = select i1 %zero, i{width} 1, i{width} %arg1"
    )
    .unwrap();
    writeln!(
        output,
        "  %safe.rhs = select i1 %range, i{width} 1, i{width} %safe.zero"
    )
    .unwrap();
    writeln!(
        output,
        "  %computed = {instruction} i{width} %arg0, %safe.rhs"
    )
    .unwrap();
    let boundary = if op == IntBinary::Div {
        if mode == IntMode::Saturating {
            signed_max(width)
        } else {
            signed_min(width)
        }
    } else {
        "0".to_owned()
    };
    writeln!(
        output,
        "  %ranged = select i1 %range, i{width} {boundary}, i{width} %computed"
    )
    .unwrap();
    match mode {
        IntMode::Checked => {
            writeln!(
                output,
                "  %value = select i1 %invalid, i{width} 0, i{width} %ranged"
            )
            .unwrap();
            writeln!(output, "  %valid = xor i1 %invalid, true").unwrap();
            emit_pair_return(output, results, "%value", "%valid")?;
        }
        IntMode::Overflowing => emit_pair_return(output, results, "%ranged", "%range")?,
        IntMode::Wrapping | IntMode::Saturating => {
            writeln!(output, "  ret i{width} %ranged").unwrap();
        }
    }
    Ok(())
}

fn emit_i128_div_rem(
    output: &mut String,
    signed: bool,
    op: IntBinary,
    mode: IntMode,
    results: &[Type],
) -> Result<(), ScalarV2LoweringError> {
    writeln!(output, "  %zero = icmp eq i128 %arg1, 0").unwrap();
    if signed {
        writeln!(output, "  %lhs.negative = icmp slt i128 %arg0, 0").unwrap();
        writeln!(output, "  %rhs.negative = icmp slt i128 %arg1, 0").unwrap();
        writeln!(output, "  %lhs.negated = sub i128 0, %arg0").unwrap();
        writeln!(output, "  %rhs.negated = sub i128 0, %arg1").unwrap();
        writeln!(
            output,
            "  %lhs.magnitude = select i1 %lhs.negative, i128 %lhs.negated, i128 %arg0"
        )
        .unwrap();
        writeln!(
            output,
            "  %rhs.magnitude = select i1 %rhs.negative, i128 %rhs.negated, i128 %arg1"
        )
        .unwrap();
        writeln!(
            output,
            "  %is.min = icmp eq i128 %arg0, {}",
            signed_min(128)
        )
        .unwrap();
        writeln!(output, "  %is.neg.one = icmp eq i128 %arg1, -1").unwrap();
        writeln!(output, "  %range = and i1 %is.min, %is.neg.one").unwrap();
        writeln!(output, "  %invalid = or i1 %zero, %range").unwrap();
    } else {
        writeln!(output, "  %lhs.magnitude = add i128 %arg0, 0").unwrap();
        writeln!(output, "  %rhs.magnitude = add i128 %arg1, 0").unwrap();
        writeln!(output, "  %range = icmp eq i1 false, true").unwrap();
        writeln!(output, "  %invalid = or i1 %zero, false").unwrap();
    }
    let predecessor = if mode == IntMode::Checked {
        "entry"
    } else {
        writeln!(
            output,
            "  br i1 %zero, label %trap, label %divide.setup\ntrap:"
        )
        .unwrap();
        writeln!(
            output,
            "  call void @llvm.trap()\n  unreachable\ndivide.setup:"
        )
        .unwrap();
        "divide.setup"
    };
    writeln!(output, "  br label %divide.loop\ndivide.loop:").unwrap();
    writeln!(
        output,
        "  %divide.index = phi i8 [ 127, %{predecessor} ], [ %divide.next, %divide.loop ]"
    )
    .unwrap();
    writeln!(output, "  %divide.remainder = phi i128 [ 0, %{predecessor} ], [ %divide.remainder.next, %divide.loop ]").unwrap();
    writeln!(output, "  %divide.quotient = phi i128 [ 0, %{predecessor} ], [ %divide.quotient.next, %divide.loop ]").unwrap();
    writeln!(output, "  %divide.shift = zext i8 %divide.index to i128").unwrap();
    writeln!(
        output,
        "  %divide.source.shifted = lshr i128 %lhs.magnitude, %divide.shift"
    )
    .unwrap();
    writeln!(
        output,
        "  %divide.source.bit = and i128 %divide.source.shifted, 1"
    )
    .unwrap();
    writeln!(
        output,
        "  %divide.remainder.shifted = shl i128 %divide.remainder, 1"
    )
    .unwrap();
    writeln!(
        output,
        "  %divide.remainder.with.bit = or i128 %divide.remainder.shifted, %divide.source.bit"
    )
    .unwrap();
    writeln!(
        output,
        "  %divide.ge = icmp uge i128 %divide.remainder.with.bit, %rhs.magnitude"
    )
    .unwrap();
    writeln!(
        output,
        "  %divide.remainder.sub = sub i128 %divide.remainder.with.bit, %rhs.magnitude"
    )
    .unwrap();
    writeln!(output, "  %divide.remainder.next = select i1 %divide.ge, i128 %divide.remainder.sub, i128 %divide.remainder.with.bit").unwrap();
    writeln!(output, "  %divide.bit = shl i128 1, %divide.shift").unwrap();
    writeln!(
        output,
        "  %divide.quotient.with.bit = or i128 %divide.quotient, %divide.bit"
    )
    .unwrap();
    writeln!(output, "  %divide.quotient.next = select i1 %divide.ge, i128 %divide.quotient.with.bit, i128 %divide.quotient").unwrap();
    writeln!(output, "  %divide.done = icmp eq i8 %divide.index, 0").unwrap();
    writeln!(output, "  %divide.next = sub i8 %divide.index, 1").unwrap();
    writeln!(
        output,
        "  br i1 %divide.done, label %divide.exit, label %divide.loop\ndivide.exit:"
    )
    .unwrap();
    if signed {
        writeln!(
            output,
            "  %quotient.negative = xor i1 %lhs.negative, %rhs.negative"
        )
        .unwrap();
        writeln!(
            output,
            "  %quotient.negated = sub i128 0, %divide.quotient.next"
        )
        .unwrap();
        writeln!(output, "  %quotient = select i1 %quotient.negative, i128 %quotient.negated, i128 %divide.quotient.next").unwrap();
        writeln!(
            output,
            "  %remainder.negated = sub i128 0, %divide.remainder.next"
        )
        .unwrap();
        writeln!(output, "  %remainder = select i1 %lhs.negative, i128 %remainder.negated, i128 %divide.remainder.next").unwrap();
    } else {
        writeln!(output, "  %quotient = add i128 %divide.quotient.next, 0").unwrap();
        writeln!(output, "  %remainder = add i128 %divide.remainder.next, 0").unwrap();
    }
    let computed = if op == IntBinary::Div {
        "%quotient"
    } else {
        "%remainder"
    };
    let boundary = if op == IntBinary::Div {
        if mode == IntMode::Saturating {
            signed_max(128)
        } else {
            signed_min(128)
        }
    } else {
        "0".to_owned()
    };
    writeln!(
        output,
        "  %ranged = select i1 %range, i128 {boundary}, i128 {computed}"
    )
    .unwrap();
    match mode {
        IntMode::Checked => {
            writeln!(
                output,
                "  %value = select i1 %invalid, i128 0, i128 %ranged"
            )
            .unwrap();
            writeln!(output, "  %valid = xor i1 %invalid, true").unwrap();
            emit_pair_return(output, results, "%value", "%valid")?;
        }
        IntMode::Overflowing => emit_pair_return(output, results, "%ranged", "%range")?,
        IntMode::Wrapping | IntMode::Saturating => writeln!(output, "  ret i128 %ranged").unwrap(),
    }
    Ok(())
}

fn emit_integer_unary(
    output: &mut String,
    ty: ScalarType,
    op: IntUnary,
    mode: IntMode,
    results: &[Type],
) -> Result<(), ScalarV2LoweringError> {
    let (width, _) = integer_parts(ty);
    if op == IntUnary::Not {
        writeln!(
            output,
            "  %result = xor i{width} %arg0, -1\n  ret i{width} %result"
        )
        .unwrap();
        return Ok(());
    }
    match mode {
        IntMode::Wrapping => {
            writeln!(
                output,
                "  %result = sub i{width} 0, %arg0\n  ret i{width} %result"
            )
            .unwrap();
        }
        IntMode::Checked | IntMode::Overflowing => {
            writeln!(output, "  %pair = call {{ i{width}, i1 }} @llvm.ssub.with.overflow.i{width}(i{width} 0, i{width} %arg0)").unwrap();
            writeln!(
                output,
                "  %value = extractvalue {{ i{width}, i1 }} %pair, 0"
            )
            .unwrap();
            writeln!(
                output,
                "  %overflow = extractvalue {{ i{width}, i1 }} %pair, 1"
            )
            .unwrap();
            let flag = if mode == IntMode::Checked {
                writeln!(output, "  %valid = xor i1 %overflow, true").unwrap();
                "%valid"
            } else {
                "%overflow"
            };
            emit_pair_return(output, results, "%value", flag)?;
        }
        IntMode::Saturating => {
            writeln!(output, "  %result = call i{width} @llvm.ssub.sat.i{width}(i{width} 0, i{width} %arg0)\n  ret i{width} %result").unwrap();
        }
    }
    Ok(())
}

fn emit_shift(
    output: &mut String,
    ty: ScalarType,
    rhs_ty: ScalarType,
    direction: ShiftDirection,
    policy: ShiftPolicy,
    results: &[Type],
) -> Result<(), ScalarV2LoweringError> {
    let (width, signed) = integer_parts(ty);
    let (rhs_width, rhs_signed) = integer_parts(rhs_ty);
    emit_extend_i128(output, "%arg1", rhs_width, rhs_signed, "%rhs.signed");
    emit_extend_i128(output, "%arg1", rhs_width, false, "%rhs.raw");
    if rhs_signed {
        writeln!(output, "  %rhs.negative = icmp slt i128 %rhs.signed, 0").unwrap();
    } else {
        writeln!(output, "  %rhs.negative = icmp eq i1 false, true").unwrap();
    }
    writeln!(output, "  %rhs.high = icmp uge i128 %rhs.raw, {width}").unwrap();
    writeln!(output, "  %invalid = or i1 %rhs.negative, %rhs.high").unwrap();
    writeln!(output, "  %rhs.masked = and i128 %rhs.raw, {}", width - 1).unwrap();
    if width == 128 {
        writeln!(output, "  %amount = add i128 %rhs.masked, 0").unwrap();
    } else {
        writeln!(output, "  %amount = trunc i128 %rhs.masked to i{width}").unwrap();
    }
    if matches!(
        policy,
        ShiftPolicy::RustOperator {
            overflow_checks: true
        }
    ) {
        writeln!(
            output,
            "  br i1 %invalid, label %trap, label %compute\ntrap:"
        )
        .unwrap();
        writeln!(output, "  call void @llvm.trap()\n  unreachable\ncompute:").unwrap();
    }
    let opcode = match (direction, signed) {
        (ShiftDirection::Left, _) => "shl",
        (ShiftDirection::Right, true) => "ashr",
        (ShiftDirection::Right, false) => "lshr",
    };
    writeln!(output, "  %shifted = {opcode} i{width} %arg0, %amount").unwrap();
    match policy {
        ShiftPolicy::Checked => {
            writeln!(
                output,
                "  %value = select i1 %invalid, i{width} 0, i{width} %shifted"
            )
            .unwrap();
            writeln!(output, "  %valid = xor i1 %invalid, true").unwrap();
            emit_pair_return(output, results, "%value", "%valid")?;
        }
        ShiftPolicy::Overflowing => emit_pair_return(output, results, "%shifted", "%invalid")?,
        ShiftPolicy::Wrapping | ShiftPolicy::RustOperator { .. } => {
            writeln!(output, "  ret i{width} %shifted").unwrap();
        }
    }
    Ok(())
}

fn emit_float_neg(output: &mut String, ty: ScalarType) -> Result<(), ScalarV2LoweringError> {
    let (float, int, mask) = match ty {
        ScalarType::Float(FloatWidth::F32) => ("float", "i32", "2147483648"),
        ScalarType::Float(FloatWidth::F64) => ("double", "i64", "9223372036854775808"),
        _ => return Err(unsupported("float negation requires f32 or f64")),
    };
    writeln!(output, "  %bits = bitcast {float} %arg0 to {int}").unwrap();
    writeln!(output, "  %neg.bits = xor {int} %bits, {mask}").unwrap();
    writeln!(
        output,
        "  %result = bitcast {int} %neg.bits to {float}\n  ret {float} %result"
    )
    .unwrap();
    Ok(())
}

fn emit_float_compare(
    output: &mut String,
    ty: ScalarType,
    predicate: Predicate,
    policy: FloatComparisonPolicy,
) -> Result<(), ScalarV2LoweringError> {
    let code = match policy {
        FloatComparisonPolicy::RustPartialEq => match predicate {
            Predicate::Eq => "oeq",
            Predicate::Ne => "une",
            _ => return Err(unsupported("invalid Rust PartialEq predicate")),
        },
        FloatComparisonPolicy::RustPartialOrd | FloatComparisonPolicy::IeeeOrdered => {
            ordered_float_predicate(predicate)
        }
        FloatComparisonPolicy::IeeeUnordered => unordered_float_predicate(predicate),
    };
    let llvm = float_llvm_type(ty);
    writeln!(
        output,
        "  %result = fcmp {code} {llvm} %arg0, %arg1\n  ret i1 %result"
    )
    .unwrap();
    Ok(())
}

fn emit_total_cmp(output: &mut String, ty: ScalarType) -> Result<(), ScalarV2LoweringError> {
    let (float, width) = match ty {
        ScalarType::Float(FloatWidth::F32) => ("float", 32),
        ScalarType::Float(FloatWidth::F64) => ("double", 64),
        _ => return Err(unsupported("total_cmp requires f32 or f64")),
    };
    for index in 0..2 {
        writeln!(
            output,
            "  %bits{index} = bitcast {float} %arg{index} to i{width}"
        )
        .unwrap();
        writeln!(
            output,
            "  %sign{index} = ashr i{width} %bits{index}, {}",
            width - 1
        )
        .unwrap();
        writeln!(output, "  %mask{index} = lshr i{width} %sign{index}, 1").unwrap();
        writeln!(
            output,
            "  %key{index} = xor i{width} %bits{index}, %mask{index}"
        )
        .unwrap();
    }
    writeln!(output, "  %less = icmp slt i{width} %key0, %key1").unwrap();
    writeln!(output, "  %greater = icmp sgt i{width} %key0, %key1").unwrap();
    writeln!(output, "  %positive = select i1 %greater, i8 1, i8 0").unwrap();
    writeln!(
        output,
        "  %result = select i1 %less, i8 -1, i8 %positive\n  ret i8 %result"
    )
    .unwrap();
    Ok(())
}

fn emit_cast(
    output: &mut String,
    from: ScalarType,
    to: ScalarType,
    cast: Cast,
    results: &[Type],
) -> Result<(), ScalarV2LoweringError> {
    let from_ty = scalar_llvm_type(from)?;
    let to_ty = scalar_llvm_type(to)?;
    match cast {
        Cast::IntExtend { signed } => writeln!(
            output,
            "  %result = {} {from_ty} %arg0 to {to_ty}",
            if signed { "sext" } else { "zext" }
        )
        .unwrap(),
        Cast::IntNarrow => {
            writeln!(output, "  %result = trunc {from_ty} %arg0 to {to_ty}").unwrap()
        }
        Cast::FloatExtend => {
            writeln!(output, "  %result = fpext {from_ty} %arg0 to {to_ty}").unwrap()
        }
        Cast::FloatNarrow => {
            writeln!(output, "  %result = fptrunc {from_ty} %arg0 to {to_ty}").unwrap()
        }
        Cast::IntToFloat { .. } => {
            let ScalarType::Int { width, signed } = from else {
                unreachable!()
            };
            if width == IntWidth::W128 {
                emit_i128_to_float(output, signed, to)?;
                return Ok(());
            }
            writeln!(
                output,
                "  %result = {} {from_ty} %arg0 to {to_ty}",
                if signed { "sitofp" } else { "uitofp" }
            )
            .unwrap();
        }
        Cast::FloatToInt { .. } => {
            let ScalarType::Float(float) = from else {
                unreachable!()
            };
            let ScalarType::Int { width, signed } = to else {
                unreachable!()
            };
            if width == IntWidth::W128 {
                emit_float_to_i128(output, float, signed)?;
                return Ok(());
            }
            writeln!(
                output,
                "  %result = call {to_ty} @llvm.fp{}i.sat.i{}.f{}({from_ty} %arg0)",
                if signed { "tos" } else { "tou" },
                width.bits(),
                float.bits()
            )
            .unwrap();
        }
        Cast::BoolToInt | Cast::CharToInt => {
            let opcode = if from.bit_width() < to.bit_width() {
                "zext"
            } else {
                "trunc"
            };
            writeln!(output, "  %result = {opcode} {from_ty} %arg0 to {to_ty}").unwrap();
        }
        Cast::IntToBoolChecked => {
            writeln!(output, "  %valid = icmp ule {from_ty} %arg0, 1").unwrap();
            writeln!(output, "  %value.raw = trunc {from_ty} %arg0 to i1").unwrap();
            writeln!(
                output,
                "  %value = select i1 %valid, i1 %value.raw, i1 false"
            )
            .unwrap();
            emit_pair_return(output, results, "%value", "%valid")?;
            return Ok(());
        }
        Cast::IntToCharChecked => {
            writeln!(output, "  %wide = trunc {from_ty} %arg0 to i32").unwrap();
            writeln!(output, "  %unicode = icmp ule {from_ty} %arg0, 1114111").unwrap();
            writeln!(output, "  %surrogate.low = icmp uge i32 %wide, 55296").unwrap();
            writeln!(output, "  %surrogate.high = icmp ule i32 %wide, 57343").unwrap();
            writeln!(
                output,
                "  %surrogate = and i1 %surrogate.low, %surrogate.high"
            )
            .unwrap();
            writeln!(output, "  %not.surrogate = xor i1 %surrogate, true").unwrap();
            writeln!(output, "  %valid = and i1 %unicode, %not.surrogate").unwrap();
            writeln!(output, "  %value = select i1 %valid, i32 %wide, i32 0").unwrap();
            emit_pair_return(output, results, "%value", "%valid")?;
            return Ok(());
        }
        Cast::Bitcast => {
            writeln!(output, "  %result = bitcast {from_ty} %arg0 to {to_ty}").unwrap()
        }
        Cast::PointerAddressSpace | Cast::PointerToInt { .. } | Cast::IntToPointer { .. } => {
            return Err(unsupported("pointer casts are not admitted"));
        }
    }
    writeln!(output, "  ret {to_ty} %result").unwrap();
    Ok(())
}

fn emit_float_to_i128(
    output: &mut String,
    float: FloatWidth,
    signed: bool,
) -> Result<(), ScalarV2LoweringError> {
    let (float_ty, storage, exponent_bits, fraction_bits, bias) = match float {
        FloatWidth::F32 => ("float", "i32", 8_u16, 23_u16, 127_i32),
        FloatWidth::F64 => ("double", "i64", 11_u16, 52_u16, 1023_i32),
        _ => return Err(unsupported("float-to-i128 requires f32 or f64")),
    };
    let storage_bits = exponent_bits + fraction_bits + 1;
    let exponent_mask = (1_u128 << exponent_bits) - 1;
    let fraction_mask = (1_u128 << fraction_bits) - 1;
    writeln!(output, "  %bits = bitcast {float_ty} %arg0 to {storage}").unwrap();
    writeln!(
        output,
        "  %sign.shift = lshr {storage} %bits, {}",
        storage_bits - 1
    )
    .unwrap();
    writeln!(output, "  %negative = trunc {storage} %sign.shift to i1").unwrap();
    writeln!(
        output,
        "  %exponent.shift = lshr {storage} %bits, {fraction_bits}"
    )
    .unwrap();
    writeln!(
        output,
        "  %exponent.raw = and {storage} %exponent.shift, {exponent_mask}"
    )
    .unwrap();
    writeln!(output, "  %fraction = and {storage} %bits, {fraction_mask}").unwrap();
    writeln!(
        output,
        "  %is.special = icmp eq {storage} %exponent.raw, {exponent_mask}"
    )
    .unwrap();
    writeln!(output, "  %fraction.zero = icmp eq {storage} %fraction, 0").unwrap();
    writeln!(
        output,
        "  %is.infinity = and i1 %is.special, %fraction.zero"
    )
    .unwrap();
    writeln!(output, "  %fraction.nonzero = xor i1 %fraction.zero, true").unwrap();
    writeln!(output, "  %is.nan = and i1 %is.special, %fraction.nonzero").unwrap();
    writeln!(
        output,
        "  %is.subnormal = icmp eq {storage} %exponent.raw, 0"
    )
    .unwrap();
    if storage == "i32" {
        writeln!(output, "  %exponent.i32 = add i32 %exponent.raw, 0").unwrap();
    } else {
        writeln!(output, "  %exponent.i32 = trunc i64 %exponent.raw to i32").unwrap();
    }
    writeln!(output, "  %exponent = sub i32 %exponent.i32, {bias}").unwrap();
    writeln!(
        output,
        "  %implicit = or {storage} %fraction, {}",
        1_u128 << fraction_bits
    )
    .unwrap();
    writeln!(output, "  %significand = zext {storage} %implicit to i128").unwrap();
    writeln!(
        output,
        "  %shift.left.raw = sub i32 %exponent, {fraction_bits}"
    )
    .unwrap();
    writeln!(
        output,
        "  %shift.left.negative = icmp slt i32 %shift.left.raw, 0"
    )
    .unwrap();
    writeln!(
        output,
        "  %shift.left.large = icmp uge i32 %shift.left.raw, 128"
    )
    .unwrap();
    writeln!(
        output,
        "  %shift.left.bounded = select i1 %shift.left.large, i32 127, i32 %shift.left.raw"
    )
    .unwrap();
    writeln!(
        output,
        "  %shift.left.safe = select i1 %shift.left.negative, i32 0, i32 %shift.left.bounded"
    )
    .unwrap();
    writeln!(output, "  %shift.left = zext i32 %shift.left.safe to i128").unwrap();
    writeln!(output, "  %left.value = shl i128 %significand, %shift.left").unwrap();
    writeln!(
        output,
        "  %shift.right.raw = sub i32 {fraction_bits}, %exponent"
    )
    .unwrap();
    writeln!(
        output,
        "  %shift.right.negative = icmp slt i32 %shift.right.raw, 0"
    )
    .unwrap();
    writeln!(
        output,
        "  %shift.right.safe = select i1 %shift.right.negative, i32 0, i32 %shift.right.raw"
    )
    .unwrap();
    writeln!(
        output,
        "  %shift.right.large = icmp uge i32 %shift.right.safe, 128"
    )
    .unwrap();
    writeln!(
        output,
        "  %shift.right.bounded = select i1 %shift.right.large, i32 127, i32 %shift.right.safe"
    )
    .unwrap();
    writeln!(
        output,
        "  %shift.right = zext i32 %shift.right.bounded to i128"
    )
    .unwrap();
    writeln!(
        output,
        "  %right.shifted = lshr i128 %significand, %shift.right"
    )
    .unwrap();
    writeln!(
        output,
        "  %right.value = select i1 %shift.right.large, i128 0, i128 %right.shifted"
    )
    .unwrap();
    writeln!(
        output,
        "  %use.left = icmp sge i32 %exponent, {fraction_bits}"
    )
    .unwrap();
    writeln!(
        output,
        "  %magnitude.raw = select i1 %use.left, i128 %left.value, i128 %right.value"
    )
    .unwrap();
    writeln!(output, "  %below.one = icmp slt i32 %exponent, 0").unwrap();
    writeln!(
        output,
        "  %zero.magnitude = or i1 %below.one, %is.subnormal"
    )
    .unwrap();
    writeln!(
        output,
        "  %magnitude = select i1 %zero.magnitude, i128 0, i128 %magnitude.raw"
    )
    .unwrap();
    let limit_exponent = if signed { 127 } else { 128 };
    writeln!(
        output,
        "  %too.large = icmp sge i32 %exponent, {limit_exponent}"
    )
    .unwrap();
    if signed {
        writeln!(output, "  %negative.value = sub i128 0, %magnitude").unwrap();
        writeln!(
            output,
            "  %signed.value = select i1 %negative, i128 %negative.value, i128 %magnitude"
        )
        .unwrap();
        writeln!(
            output,
            "  %positive.saturated = select i1 %too.large, i128 {}, i128 %signed.value",
            signed_max(128)
        )
        .unwrap();
        writeln!(
            output,
            "  %negative.saturated = select i1 %too.large, i128 {}, i128 %signed.value",
            signed_min(128)
        )
        .unwrap();
        writeln!(output, "  %finite.value = select i1 %negative, i128 %negative.saturated, i128 %positive.saturated").unwrap();
    } else {
        writeln!(
            output,
            "  %negative.or.zero = or i1 %negative, %zero.magnitude"
        )
        .unwrap();
        writeln!(
            output,
            "  %bounded = select i1 %too.large, i128 -1, i128 %magnitude"
        )
        .unwrap();
        writeln!(
            output,
            "  %finite.value = select i1 %negative.or.zero, i128 0, i128 %bounded"
        )
        .unwrap();
    }
    writeln!(
        output,
        "  %infinite.saturation = select i1 %negative, i128 {}, i128 {}",
        if signed {
            signed_min(128)
        } else {
            "0".to_owned()
        },
        if signed {
            signed_max(128)
        } else {
            "-1".to_owned()
        }
    )
    .unwrap();
    writeln!(
        output,
        "  %with.infinity = select i1 %is.infinity, i128 %infinite.saturation, i128 %finite.value"
    )
    .unwrap();
    writeln!(
        output,
        "  %result = select i1 %is.nan, i128 0, i128 %with.infinity"
    )
    .unwrap();
    writeln!(output, "  ret i128 %result").unwrap();
    Ok(())
}

fn emit_i128_to_float(
    output: &mut String,
    signed: bool,
    to: ScalarType,
) -> Result<(), ScalarV2LoweringError> {
    let (float_ty, storage, precision, fraction_bits, bias) = match to {
        ScalarType::Float(FloatWidth::F32) => ("float", "i32", 24_u16, 23_u16, 127_u16),
        ScalarType::Float(FloatWidth::F64) => ("double", "i64", 53_u16, 52_u16, 1023_u16),
        _ => return Err(unsupported("i128 conversion requires f32 or f64")),
    };
    if signed {
        writeln!(output, "  %negative = icmp slt i128 %arg0, 0").unwrap();
        writeln!(output, "  %negated = sub i128 0, %arg0").unwrap();
        writeln!(
            output,
            "  %magnitude = select i1 %negative, i128 %negated, i128 %arg0"
        )
        .unwrap();
    } else {
        writeln!(output, "  %negative = icmp eq i1 false, true").unwrap();
        writeln!(output, "  %magnitude = add i128 %arg0, 0").unwrap();
    }
    writeln!(output, "  %zero = icmp eq i128 %magnitude, 0").unwrap();
    writeln!(output, "  %high.wide = lshr i128 %magnitude, 64").unwrap();
    writeln!(output, "  %high = trunc i128 %high.wide to i64").unwrap();
    writeln!(output, "  %low = trunc i128 %magnitude to i64").unwrap();
    writeln!(output, "  %has.high = icmp ne i64 %high, 0").unwrap();
    writeln!(
        output,
        "  %clz.high = call i64 @llvm.ctlz.i64(i64 %high, i1 false)"
    )
    .unwrap();
    writeln!(
        output,
        "  %clz.low = call i64 @llvm.ctlz.i64(i64 %low, i1 false)"
    )
    .unwrap();
    writeln!(output, "  %clz.low.plus = add i64 %clz.low, 64").unwrap();
    writeln!(
        output,
        "  %leading = select i1 %has.high, i64 %clz.high, i64 %clz.low.plus"
    )
    .unwrap();
    writeln!(output, "  %bit.length = sub i64 128, %leading").unwrap();
    writeln!(
        output,
        "  %needs.right = icmp ugt i64 %bit.length, {precision}"
    )
    .unwrap();
    writeln!(output, "  %right.raw = sub i64 %bit.length, {precision}").unwrap();
    writeln!(
        output,
        "  %right.safe = select i1 %needs.right, i64 %right.raw, i64 1"
    )
    .unwrap();
    writeln!(output, "  %right = zext i64 %right.safe to i128").unwrap();
    writeln!(output, "  %truncated = lshr i128 %magnitude, %right").unwrap();
    writeln!(output, "  %one.shifted = shl i128 1, %right").unwrap();
    writeln!(output, "  %remainder.mask = sub i128 %one.shifted, 1").unwrap();
    writeln!(
        output,
        "  %remainder = and i128 %magnitude, %remainder.mask"
    )
    .unwrap();
    writeln!(output, "  %half.shift = sub i128 %right, 1").unwrap();
    writeln!(output, "  %half = shl i128 1, %half.shift").unwrap();
    writeln!(output, "  %above.half = icmp ugt i128 %remainder, %half").unwrap();
    writeln!(output, "  %at.half = icmp eq i128 %remainder, %half").unwrap();
    writeln!(output, "  %truncated.odd.bits = and i128 %truncated, 1").unwrap();
    writeln!(
        output,
        "  %truncated.odd = icmp ne i128 %truncated.odd.bits, 0"
    )
    .unwrap();
    writeln!(output, "  %tie.up = and i1 %at.half, %truncated.odd").unwrap();
    writeln!(output, "  %round.up.raw = or i1 %above.half, %tie.up").unwrap();
    writeln!(output, "  %round.up = and i1 %needs.right, %round.up.raw").unwrap();
    writeln!(output, "  %round.bit = zext i1 %round.up to i128").unwrap();
    writeln!(output, "  %rounded.right = add i128 %truncated, %round.bit").unwrap();
    writeln!(output, "  %left.raw = sub i64 {precision}, %bit.length").unwrap();
    writeln!(
        output,
        "  %left.safe = select i1 %needs.right, i64 0, i64 %left.raw"
    )
    .unwrap();
    writeln!(output, "  %left = zext i64 %left.safe to i128").unwrap();
    writeln!(output, "  %shifted.left = shl i128 %magnitude, %left").unwrap();
    writeln!(
        output,
        "  %rounded = select i1 %needs.right, i128 %rounded.right, i128 %shifted.left"
    )
    .unwrap();
    writeln!(output, "  %carry.bits = lshr i128 %rounded, {precision}").unwrap();
    writeln!(output, "  %carry = icmp ne i128 %carry.bits, 0").unwrap();
    writeln!(output, "  %carried = lshr i128 %rounded, 1").unwrap();
    writeln!(
        output,
        "  %significand = select i1 %carry, i128 %carried, i128 %rounded"
    )
    .unwrap();
    writeln!(
        output,
        "  %exponent.base = add i64 %bit.length, {}",
        bias - 1
    )
    .unwrap();
    writeln!(output, "  %carry.i64 = zext i1 %carry to i64").unwrap();
    writeln!(output, "  %exponent = add i64 %exponent.base, %carry.i64").unwrap();
    writeln!(
        output,
        "  %fraction.i128 = and i128 %significand, {}",
        (1_u128 << fraction_bits) - 1
    )
    .unwrap();
    writeln!(
        output,
        "  %fraction = trunc i128 %fraction.i128 to {storage}"
    )
    .unwrap();
    writeln!(
        output,
        "  %exponent.storage = trunc i64 %exponent to {storage}"
    )
    .unwrap();
    writeln!(
        output,
        "  %exponent.bits = shl {storage} %exponent.storage, {fraction_bits}"
    )
    .unwrap();
    writeln!(
        output,
        "  %positive.bits = or {storage} %exponent.bits, %fraction"
    )
    .unwrap();
    writeln!(output, "  %sign.storage = zext i1 %negative to {storage}").unwrap();
    writeln!(
        output,
        "  %sign.bits = shl {storage} %sign.storage, {}",
        fraction_bits + if float_ty == "float" { 8 } else { 11 }
    )
    .unwrap();
    writeln!(
        output,
        "  %signed.bits = or {storage} %positive.bits, %sign.bits"
    )
    .unwrap();
    writeln!(
        output,
        "  %bits = select i1 %zero, {storage} 0, {storage} %signed.bits"
    )
    .unwrap();
    writeln!(output, "  %result = bitcast {storage} %bits to {float_ty}").unwrap();
    writeln!(output, "  ret {float_ty} %result").unwrap();
    Ok(())
}

fn emit_pair_return(
    output: &mut String,
    results: &[Type],
    value: &str,
    flag: &str,
) -> Result<(), ScalarV2LoweringError> {
    let [value_ty, Type::Scalar(fe2o3_kernel_ir::ScalarType::Bool)] = results else {
        return Err(unsupported("scalar pair result is not value/bool"));
    };
    let value_ty = llvm_ir_type(value_ty)?;
    writeln!(
        output,
        "  %result.0 = insertvalue {{ {value_ty}, i1 }} poison, {value_ty} {value}, 0"
    )
    .unwrap();
    writeln!(
        output,
        "  %result.1 = insertvalue {{ {value_ty}, i1 }} %result.0, i1 {flag}, 1"
    )
    .unwrap();
    writeln!(output, "  ret {{ {value_ty}, i1 }} %result.1").unwrap();
    Ok(())
}

fn emit_extend_i128(output: &mut String, value: &str, width: u16, signed: bool, result: &str) {
    if width == 128 {
        writeln!(output, "  {result} = add i128 {value}, 0").unwrap();
    } else {
        writeln!(
            output,
            "  {result} = {} i{width} {value} to i128",
            if signed { "sext" } else { "zext" }
        )
        .unwrap();
    }
}

fn llvm_results(results: &[Type]) -> String {
    match results {
        [result] => llvm_ir_type(result).expect("validated result").to_owned(),
        [first, second] => format!(
            "{{ {}, {} }}",
            llvm_ir_type(first).expect("validated result"),
            llvm_ir_type(second).expect("validated result")
        ),
        _ => unreachable!("scalar V2 has one or two results"),
    }
}

fn llvm_ir_type(ty: &Type) -> Result<&'static str, ScalarV2LoweringError> {
    let Type::Scalar(scalar) = ty else {
        return Err(unsupported("non-scalar carrier type"));
    };
    Ok(match scalar {
        fe2o3_kernel_ir::ScalarType::Bool => "i1",
        fe2o3_kernel_ir::ScalarType::I8 | fe2o3_kernel_ir::ScalarType::U8 => "i8",
        fe2o3_kernel_ir::ScalarType::I16 | fe2o3_kernel_ir::ScalarType::U16 => "i16",
        fe2o3_kernel_ir::ScalarType::I32 | fe2o3_kernel_ir::ScalarType::U32 => "i32",
        fe2o3_kernel_ir::ScalarType::I64 | fe2o3_kernel_ir::ScalarType::U64 => "i64",
        fe2o3_kernel_ir::ScalarType::I128 | fe2o3_kernel_ir::ScalarType::U128 => "i128",
        fe2o3_kernel_ir::ScalarType::F32 => "float",
        fe2o3_kernel_ir::ScalarType::F64 => "double",
        unsupported_ty => {
            return Err(unsupported(&format!(
                "unsupported carrier type {unsupported_ty:?}"
            )));
        }
    })
}

fn scalar_llvm_type(ty: ScalarType) -> Result<&'static str, ScalarV2LoweringError> {
    Ok(match ty {
        ScalarType::Bool => "i1",
        ScalarType::Char => "i32",
        ScalarType::Int { width, .. } => integer_llvm_width(width),
        ScalarType::Float(FloatWidth::F32) => "float",
        ScalarType::Float(FloatWidth::F64) => "double",
        unsupported_ty => {
            return Err(unsupported(&format!(
                "unsupported scalar type {unsupported_ty:?}"
            )));
        }
    })
}

const fn integer_llvm_width(width: IntWidth) -> &'static str {
    match width {
        IntWidth::W8 => "i8",
        IntWidth::W16 => "i16",
        IntWidth::W32 => "i32",
        IntWidth::W64 => "i64",
        IntWidth::W128 => "i128",
    }
}
const fn integer_parts(ty: ScalarType) -> (u16, bool) {
    match ty {
        ScalarType::Int { width, signed } => (width.bits(), signed),
        _ => unreachable!(),
    }
}
const fn signed_prefix(signed: bool) -> &'static str {
    if signed { "s" } else { "u" }
}
const fn integer_stem(op: IntBinary) -> &'static str {
    match op {
        IntBinary::Add => "add",
        IntBinary::Sub => "sub",
        IntBinary::Mul => "mul",
        IntBinary::Div => "div",
        IntBinary::Rem => "rem",
        IntBinary::And => "and",
        IntBinary::Or => "or",
        IntBinary::Xor => "xor",
    }
}
fn signed_min(width: u16) -> String {
    format!("-{}", 1_u128 << (width - 1))
}
fn signed_max(width: u16) -> String {
    ((1_u128 << (width - 1)) - 1).to_string()
}
const fn float_llvm_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Float(FloatWidth::F32) => "float",
        ScalarType::Float(FloatWidth::F64) => "double",
        _ => unreachable!(),
    }
}
const fn float_suffix(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Float(FloatWidth::F32) => "f32",
        ScalarType::Float(FloatWidth::F64) => "f64",
        _ => unreachable!(),
    }
}
const fn float_llvm_width(width: FloatWidth) -> &'static str {
    match width {
        FloatWidth::F32 => "float",
        FloatWidth::F64 => "double",
        _ => unreachable!(),
    }
}
const fn float_stem(op: FloatBinary) -> &'static str {
    match op {
        FloatBinary::Add => "fadd",
        FloatBinary::Sub => "fsub",
        FloatBinary::Mul => "fmul",
        FloatBinary::Div => "fdiv",
        FloatBinary::Rem => "frem",
    }
}
const fn integer_predicate(predicate: Predicate, signed: bool) -> &'static str {
    match predicate {
        Predicate::Eq => "eq",
        Predicate::Ne => "ne",
        Predicate::Lt if signed => "slt",
        Predicate::Le if signed => "sle",
        Predicate::Gt if signed => "sgt",
        Predicate::Ge if signed => "sge",
        Predicate::Lt => "ult",
        Predicate::Le => "ule",
        Predicate::Gt => "ugt",
        Predicate::Ge => "uge",
    }
}
const fn ordered_float_predicate(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::Eq => "oeq",
        Predicate::Ne => "one",
        Predicate::Lt => "olt",
        Predicate::Le => "ole",
        Predicate::Gt => "ogt",
        Predicate::Ge => "oge",
    }
}
const fn unordered_float_predicate(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::Eq => "ueq",
        Predicate::Ne => "une",
        Predicate::Lt => "ult",
        Predicate::Le => "ule",
        Predicate::Gt => "ugt",
        Predicate::Ge => "uge",
    }
}
fn unsupported(message: &str) -> ScalarV2LoweringError {
    ScalarV2LoweringError::Unsupported(message.to_owned())
}
