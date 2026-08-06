target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1

define amdgpu_kernel void @phi_loop(ptr addrspace(1) %arg0.data, i64 %arg0.len, i64 %arg1) #0 !reqd_work_group_size !0 {
bb0:
  %v2 = getelementptr i8, ptr addrspace(1) %arg0.data, i64 0
  br label %bb1
bb1:
  %v10 = phi i64 [ %arg1, %bb0 ], [ %v15, %bb1 ]
  %v11.data = phi ptr addrspace(1) [ %arg0.data, %bb0 ], [ %v11.data, %bb1 ]
  %v11.len = phi i64 [ %arg0.len, %bb0 ], [ %v11.len, %bb1 ]
  %v12 = phi ptr addrspace(1) [ %v2, %bb0 ], [ %v12, %bb1 ]
  %v13 = add i64 %v11.len, 0
  %v15 = add i64 %v10, 1
  %v16 = icmp ult i64 %v15, %v13
  br i1 %v16, label %bb1, label %bb2
bb2:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 64, i32 1, i32 1}
