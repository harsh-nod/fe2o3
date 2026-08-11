target triple = "amdgcn-amd-amdhsa"

define i128 @__fe2o3_ir_scalar_v2_4645324f5356320003000c0000000000080601000403000003050100(double %arg0) #0 {
entry:
  %bits = bitcast double %arg0 to i64
  %sign.shift = lshr i64 %bits, 63
  %negative = trunc i64 %sign.shift to i1
  %exponent.shift = lshr i64 %bits, 52
  %exponent.raw = and i64 %exponent.shift, 2047
  %fraction = and i64 %bits, 4503599627370495
  %is.special = icmp eq i64 %exponent.raw, 2047
  %fraction.zero = icmp eq i64 %fraction, 0
  %is.infinity = and i1 %is.special, %fraction.zero
  %fraction.nonzero = xor i1 %fraction.zero, true
  %is.nan = and i1 %is.special, %fraction.nonzero
  %is.subnormal = icmp eq i64 %exponent.raw, 0
  %exponent.i32 = trunc i64 %exponent.raw to i32
  %exponent = sub i32 %exponent.i32, 1023
  %implicit = or i64 %fraction, 4503599627370496
  %significand = zext i64 %implicit to i128
  %shift.left.raw = sub i32 %exponent, 52
  %shift.left.negative = icmp slt i32 %shift.left.raw, 0
  %shift.left.large = icmp uge i32 %shift.left.raw, 128
  %shift.left.bounded = select i1 %shift.left.large, i32 127, i32 %shift.left.raw
  %shift.left.safe = select i1 %shift.left.negative, i32 0, i32 %shift.left.bounded
  %shift.left = zext i32 %shift.left.safe to i128
  %left.value = shl i128 %significand, %shift.left
  %shift.right.raw = sub i32 52, %exponent
  %shift.right.negative = icmp slt i32 %shift.right.raw, 0
  %shift.right.safe = select i1 %shift.right.negative, i32 0, i32 %shift.right.raw
  %shift.right.large = icmp uge i32 %shift.right.safe, 128
  %shift.right.bounded = select i1 %shift.right.large, i32 127, i32 %shift.right.safe
  %shift.right = zext i32 %shift.right.bounded to i128
  %right.shifted = lshr i128 %significand, %shift.right
  %right.value = select i1 %shift.right.large, i128 0, i128 %right.shifted
  %use.left = icmp sge i32 %exponent, 52
  %magnitude.raw = select i1 %use.left, i128 %left.value, i128 %right.value
  %below.one = icmp slt i32 %exponent, 0
  %zero.magnitude = or i1 %below.one, %is.subnormal
  %magnitude = select i1 %zero.magnitude, i128 0, i128 %magnitude.raw
  %too.large = icmp sge i32 %exponent, 127
  %negative.value = sub i128 0, %magnitude
  %signed.value = select i1 %negative, i128 %negative.value, i128 %magnitude
  %positive.saturated = select i1 %too.large, i128 170141183460469231731687303715884105727, i128 %signed.value
  %negative.saturated = select i1 %too.large, i128 -170141183460469231731687303715884105728, i128 %signed.value
  %finite.value = select i1 %negative, i128 %negative.saturated, i128 %positive.saturated
  %infinite.saturation = select i1 %negative, i128 -170141183460469231731687303715884105728, i128 170141183460469231731687303715884105727
  %with.infinity = select i1 %is.infinity, i128 %infinite.saturation, i128 %finite.value
  %result = select i1 %is.nan, i128 0, i128 %with.infinity
  ret i128 %result
}

attributes #0 = { nounwind "target-cpu"="gfx942" "denormal-fp-math"="ieee,ieee" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" }
