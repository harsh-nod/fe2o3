target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1

define amdgpu_kernel void @vecadd(ptr addrspace(1) %arg0.data, i64 %arg0.len, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len) #0 !reqd_work_group_size !0 {
bb0:
  %v3.local.i32 = call i32 @llvm.amdgcn.workitem.id.x()
  %v3.group.i32 = call i32 @llvm.amdgcn.workgroup.id.x()
  %v3.local = zext i32 %v3.local.i32 to i64
  %v3.group = zext i32 %v3.group.i32 to i64
  %v3.base = mul i64 %v3.group, 256
  %v3 = add i64 %v3.base, %v3.local
  %v5 = add i64 %v3, 0
  %v6 = add i64 %arg2.len, 0
  %v7 = icmp ult i64 %v3, %v6
  %v8 = getelementptr i8, ptr addrspace(1) %arg2.data, i64 0
  %v9 = getelementptr float, ptr addrspace(1) %v8, i64 %v3
  br i1 %v7, label %bb1, label %bb4
bb1:
  %v10 = add i64 %arg0.len, 0
  %v11 = icmp ult i64 %v5, %v10
  br i1 %v11, label %bb2, label %bb5
bb2:
  %v12 = getelementptr i8, ptr addrspace(1) %arg0.data, i64 0
  %v13 = getelementptr float, ptr addrspace(1) %v12, i64 %v5
  %v14 = load float, ptr addrspace(1) %v13, align 4
  %v15 = add i64 %arg1.len, 0
  %v16 = icmp ult i64 %v5, %v15
  br i1 %v16, label %bb3, label %bb5
bb3:
  %v17 = getelementptr i8, ptr addrspace(1) %arg1.data, i64 0
  %v18 = getelementptr float, ptr addrspace(1) %v17, i64 %v5
  %v19 = load float, ptr addrspace(1) %v18, align 4
  %v20 = fadd float %v14, %v19
  store float %v20, ptr addrspace(1) %v9, align 4
  br label %bb4
bb4:
  ret void
bb5:
  unreachable
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="256,256" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 256, i32 1, i32 1}
