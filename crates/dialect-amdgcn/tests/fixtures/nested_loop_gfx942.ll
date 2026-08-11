target triple = "amdgcn-amd-amdhsa"

define i32 @nested_loop(i32 %arg0) nounwind "target-features"="-wavefrontsize32,+wavefrontsize64" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" {
bb0:
  br label %bb1
bb1:
  %v3 = phi i32 [ 0, %bb0 ], [ %v17, %bb7 ]
  %v4 = phi i32 [ 0, %bb0 ], [ %v8, %bb7 ]
  %v5 = icmp ult i32 %v3, %arg0
  br i1 %v5, label %bb2, label %bb8
bb2:
  br label %bb3
bb3:
  %v7 = phi i32 [ 0, %bb2 ], [ %v15, %bb6 ]
  %v8 = phi i32 [ %v4, %bb2 ], [ %v13, %bb6 ]
  %v9 = icmp ult i32 %v7, %arg0
  br i1 %v9, label %bb4, label %bb7
bb4:
  %v11 = icmp eq i32 %v7, 2
  br i1 %v11, label %edge_bb4_0_bb6, label %bb5
edge_bb4_0_bb6:
  br label %bb6
bb5:
  %v12 = add i32 %v8, %v7
  br label %bb6
bb6:
  %v13 = phi i32 [ %v8, %edge_bb4_0_bb6 ], [ %v12, %bb5 ]
  %v15 = add i32 %v7, 1
  br label %bb3
bb7:
  %v17 = add i32 %v3, 1
  br label %bb1
bb8:
  %v18 = phi i32 [ %v4, %bb1 ]
  ret i32 %v18
}
