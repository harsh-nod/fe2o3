target triple = "amdgcn-amd-amdhsa"

define i32 @branching_fill(i32 %arg0) nounwind "target-features"="-wavefrontsize32,+wavefrontsize64" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" {
bb0:
  %v2 = icmp ult i32 %arg0, 10
  br i1 %v2, label %bb1, label %bb2
bb1:
  br label %bb3
bb2:
  br label %bb3
bb3:
  %v5 = phi i32 [ 7, %bb1 ], [ 0, %bb2 ]
  ret i32 %v5
}
