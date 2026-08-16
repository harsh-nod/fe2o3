target triple = "amdgcn-amd-amdhsa"
target datalayout = "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9"

declare i32 @llvm.amdgcn.workgroup.id.x() #1
declare i32 @llvm.amdgcn.workitem.id.x() #1
declare float @__ocml_exp_f32(float)

define amdgpu_kernel void @row_softmax_v1(ptr addrspace(1) %arg0.data, i64 %arg0.len, ptr addrspace(1) %arg1.data, i64 %arg1.len) #0 !reqd_work_group_size !0 {
bb0:
  %v2 = getelementptr i8, ptr addrspace(1) %arg0.data, i64 0
  %v3 = getelementptr i8, ptr addrspace(1) %arg1.data, i64 0
  %v4.local.i32 = call i32 @llvm.amdgcn.workitem.id.x()
  %v4.local = zext i32 %v4.local.i32 to i64
  %v4 = add i64 %v4.local, 0
  %v6 = icmp eq i64 %v4, 0
  br i1 %v6, label %bb1, label %bb10
bb1:
  br label %bb2
bb2:
  %v10 = phi i64 [ 0, %bb1 ], [ %v17, %bb3 ]
  %v11 = phi float [ 0xFFF0000000000000, %bb1 ], [ %v16, %bb3 ]
  %v12 = icmp ult i64 %v10, 64
  br i1 %v12, label %bb3, label %bb4
bb3:
  %v13 = getelementptr float, ptr addrspace(1) %v2, i64 %v10
  %v14 = load float, ptr addrspace(1) %v13, align 4
  %v15 = fcmp ogt float %v14, %v11
  %v16 = select i1 %v15, float %v14, float %v11
  %v17 = add i64 %v10, 1
  br label %bb2
bb4:
  %v18 = phi float [ %v11, %bb2 ]
  br label %bb5
bb5:
  %v20 = phi i64 [ 0, %bb4 ], [ %v29, %bb6 ]
  %v21 = phi float [ 0x0000000000000000, %bb4 ], [ %v28, %bb6 ]
  %v22 = phi float [ %v18, %bb4 ], [ %v22, %bb6 ]
  %v23 = icmp ult i64 %v20, 64
  br i1 %v23, label %bb6, label %bb7
bb6:
  %v24 = getelementptr float, ptr addrspace(1) %v2, i64 %v20
  %v25 = load float, ptr addrspace(1) %v24, align 4
  %v26 = fsub float %v25, %v22
  %v27 = call float @__ocml_exp_f32(float %v26)
  %v28 = fadd float %v21, %v27
  %v29 = add i64 %v20, 1
  br label %bb5
bb7:
  %v30 = phi float [ %v22, %bb5 ]
  %v31 = phi float [ %v21, %bb5 ]
  br label %bb8
bb8:
  %v32 = phi i64 [ 0, %bb7 ], [ %v42, %bb9 ]
  %v33 = phi float [ %v30, %bb7 ], [ %v33, %bb9 ]
  %v34 = phi float [ %v31, %bb7 ], [ %v34, %bb9 ]
  %v35 = icmp ult i64 %v32, 64
  br i1 %v35, label %bb9, label %bb11
bb9:
  %v36 = getelementptr float, ptr addrspace(1) %v2, i64 %v32
  %v37 = load float, ptr addrspace(1) %v36, align 4
  %v38 = fsub float %v37, %v33
  %v39 = call float @__ocml_exp_f32(float %v38)
  %v40 = fdiv float %v39, %v34
  %v41 = getelementptr float, ptr addrspace(1) %v3, i64 %v32
  store float %v40, ptr addrspace(1) %v41, align 4
  %v42 = add i64 %v32, 1
  br label %bb8
bb10:
  ret void
bb11:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" "fp-contract"="off" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 64, i32 1, i32 1}
