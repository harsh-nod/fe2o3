target triple = "amdgcn-amd-amdhsa"

declare void @llvm.trap()

define i64 @__fe2o3_ir_scalar_v2_4645324f5356320003000800000000000104020003040100(i64 %arg0, i64 %arg1) #0 {
entry:
  %zero = icmp eq i64 %arg1, 0
  %is.min = icmp eq i64 %arg0, -9223372036854775808
  %is.neg.one = icmp eq i64 %arg1, -1
  %range = and i1 %is.min, %is.neg.one
  %invalid = or i1 %zero, %range
  br i1 %zero, label %trap, label %compute
trap:
  call void @llvm.trap()
  unreachable
compute:
  %safe.zero = select i1 %zero, i64 1, i64 %arg1
  %safe.rhs = select i1 %range, i64 1, i64 %safe.zero
  %computed = sdiv i64 %arg0, %safe.rhs
  %ranged = select i1 %range, i64 -9223372036854775808, i64 %computed
  ret i64 %ranged
}

attributes #0 = { nounwind "target-cpu"="gfx942" "denormal-fp-math"="ieee,ieee" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" }
