target triple = "amdgcn-amd-amdhsa"

declare void @llvm.trap()

define i128 @__fe2o3_ir_scalar_v2_4645324f5356320003000800000000000104020003050100(i128 %arg0, i128 %arg1) #0 {
entry:
  %zero = icmp eq i128 %arg1, 0
  %lhs.negative = icmp slt i128 %arg0, 0
  %rhs.negative = icmp slt i128 %arg1, 0
  %lhs.negated = sub i128 0, %arg0
  %rhs.negated = sub i128 0, %arg1
  %lhs.magnitude = select i1 %lhs.negative, i128 %lhs.negated, i128 %arg0
  %rhs.magnitude = select i1 %rhs.negative, i128 %rhs.negated, i128 %arg1
  %is.min = icmp eq i128 %arg0, -170141183460469231731687303715884105728
  %is.neg.one = icmp eq i128 %arg1, -1
  %range = and i1 %is.min, %is.neg.one
  %invalid = or i1 %zero, %range
  br i1 %zero, label %trap, label %divide.setup
trap:
  call void @llvm.trap()
  unreachable
divide.setup:
  br label %divide.loop
divide.loop:
  %divide.index = phi i8 [ 127, %divide.setup ], [ %divide.next, %divide.loop ]
  %divide.remainder = phi i128 [ 0, %divide.setup ], [ %divide.remainder.next, %divide.loop ]
  %divide.quotient = phi i128 [ 0, %divide.setup ], [ %divide.quotient.next, %divide.loop ]
  %divide.shift = zext i8 %divide.index to i128
  %divide.source.shifted = lshr i128 %lhs.magnitude, %divide.shift
  %divide.source.bit = and i128 %divide.source.shifted, 1
  %divide.remainder.shifted = shl i128 %divide.remainder, 1
  %divide.remainder.with.bit = or i128 %divide.remainder.shifted, %divide.source.bit
  %divide.ge = icmp uge i128 %divide.remainder.with.bit, %rhs.magnitude
  %divide.remainder.sub = sub i128 %divide.remainder.with.bit, %rhs.magnitude
  %divide.remainder.next = select i1 %divide.ge, i128 %divide.remainder.sub, i128 %divide.remainder.with.bit
  %divide.bit = shl i128 1, %divide.shift
  %divide.quotient.with.bit = or i128 %divide.quotient, %divide.bit
  %divide.quotient.next = select i1 %divide.ge, i128 %divide.quotient.with.bit, i128 %divide.quotient
  %divide.done = icmp eq i8 %divide.index, 0
  %divide.next = sub i8 %divide.index, 1
  br i1 %divide.done, label %divide.exit, label %divide.loop
divide.exit:
  %quotient.negative = xor i1 %lhs.negative, %rhs.negative
  %quotient.negated = sub i128 0, %divide.quotient.next
  %quotient = select i1 %quotient.negative, i128 %quotient.negated, i128 %divide.quotient.next
  %remainder.negated = sub i128 0, %divide.remainder.next
  %remainder = select i1 %lhs.negative, i128 %remainder.negated, i128 %divide.remainder.next
  %ranged = select i1 %range, i128 -170141183460469231731687303715884105728, i128 %quotient
  ret i128 %ranged
}

attributes #0 = { nounwind "target-cpu"="gfx942" "denormal-fp-math"="ieee,ieee" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" }
