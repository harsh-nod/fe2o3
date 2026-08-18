target triple = "amdgcn-amd-amdhsa"

define i32 @integer_match(i32 %arg0) nounwind "target-features"="-wavefrontsize32,+wavefrontsize64" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" {
bb0:
  switch i32 %arg0, label %bb4 [
    i32 0, label %bb1
    i32 7, label %bb2
    i32 42, label %bb3
  ]
bb1:
  br label %bb5
bb2:
  br label %bb5
bb3:
  br label %bb5
bb4:
  br label %bb5
bb5:
  %v5 = phi i32 [ 10, %bb1 ], [ 20, %bb2 ], [ 30, %bb3 ], [ 99, %bb4 ]
  ret i32 %v5
}
