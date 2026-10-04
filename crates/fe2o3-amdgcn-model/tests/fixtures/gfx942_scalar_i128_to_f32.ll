target triple = "amdgcn-amd-amdhsa"

declare i64 @llvm.ctlz.i64(i64, i1 immarg)

define float @__fe2o3_ir_scalar_v2_4645324f5356320003000c0000000000080501000305010004020000(i128 %arg0) #0 {
entry:
  %negative = icmp slt i128 %arg0, 0
  %negated = sub i128 0, %arg0
  %magnitude = select i1 %negative, i128 %negated, i128 %arg0
  %zero = icmp eq i128 %magnitude, 0
  %high.wide = lshr i128 %magnitude, 64
  %high = trunc i128 %high.wide to i64
  %low = trunc i128 %magnitude to i64
  %has.high = icmp ne i64 %high, 0
  %clz.high = call i64 @llvm.ctlz.i64(i64 %high, i1 false)
  %clz.low = call i64 @llvm.ctlz.i64(i64 %low, i1 false)
  %clz.low.plus = add i64 %clz.low, 64
  %leading = select i1 %has.high, i64 %clz.high, i64 %clz.low.plus
  %bit.length = sub i64 128, %leading
  %needs.right = icmp ugt i64 %bit.length, 24
  %right.raw = sub i64 %bit.length, 24
  %right.safe = select i1 %needs.right, i64 %right.raw, i64 1
  %right = zext i64 %right.safe to i128
  %truncated = lshr i128 %magnitude, %right
  %one.shifted = shl i128 1, %right
  %remainder.mask = sub i128 %one.shifted, 1
  %remainder = and i128 %magnitude, %remainder.mask
  %half.shift = sub i128 %right, 1
  %half = shl i128 1, %half.shift
  %above.half = icmp ugt i128 %remainder, %half
  %at.half = icmp eq i128 %remainder, %half
  %truncated.odd.bits = and i128 %truncated, 1
  %truncated.odd = icmp ne i128 %truncated.odd.bits, 0
  %tie.up = and i1 %at.half, %truncated.odd
  %round.up.raw = or i1 %above.half, %tie.up
  %round.up = and i1 %needs.right, %round.up.raw
  %round.bit = zext i1 %round.up to i128
  %rounded.right = add i128 %truncated, %round.bit
  %left.raw = sub i64 24, %bit.length
  %left.safe = select i1 %needs.right, i64 0, i64 %left.raw
  %left = zext i64 %left.safe to i128
  %shifted.left = shl i128 %magnitude, %left
  %rounded = select i1 %needs.right, i128 %rounded.right, i128 %shifted.left
  %carry.bits = lshr i128 %rounded, 24
  %carry = icmp ne i128 %carry.bits, 0
  %carried = lshr i128 %rounded, 1
  %significand = select i1 %carry, i128 %carried, i128 %rounded
  %exponent.base = add i64 %bit.length, 126
  %carry.i64 = zext i1 %carry to i64
  %exponent = add i64 %exponent.base, %carry.i64
  %fraction.i128 = and i128 %significand, 8388607
  %fraction = trunc i128 %fraction.i128 to i32
  %exponent.storage = trunc i64 %exponent to i32
  %exponent.bits = shl i32 %exponent.storage, 23
  %positive.bits = or i32 %exponent.bits, %fraction
  %sign.storage = zext i1 %negative to i32
  %sign.bits = shl i32 %sign.storage, 31
  %signed.bits = or i32 %positive.bits, %sign.bits
  %bits = select i1 %zero, i32 0, i32 %signed.bits
  %result = bitcast i32 %bits to float
  ret float %result
}

attributes #0 = { nounwind "target-cpu"="gfx942" "denormal-fp-math"="ieee,ieee" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" }
