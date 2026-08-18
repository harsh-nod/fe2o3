target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1

define amdgpu_kernel void @fill(ptr addrspace(1) %arg0.data, i64 %arg0.len, float %arg1) #0 !reqd_work_group_size !0 {
bb0:
  %v2.local.i32 = call i32 @llvm.amdgcn.workitem.id.x()
  %v2.group.i32 = call i32 @llvm.amdgcn.workgroup.id.x()
  %v2.local = zext i32 %v2.local.i32 to i64
  %v2.group = zext i32 %v2.group.i32 to i64
  %v2.base = mul i64 %v2.group, 64
  %v2 = add i64 %v2.base, %v2.local
  %v3 = add i64 %arg0.len, 0
  %v4 = icmp ult i64 %v2, %v3
  br i1 %v4, label %bb1, label %bb2
bb1:
  %v5 = getelementptr i8, ptr addrspace(1) %arg0.data, i64 0
  %v6 = getelementptr float, ptr addrspace(1) %v5, i64 %v2
  store float %arg1, ptr addrspace(1) %v6, align 4
  br label %bb2
bb2:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 64, i32 1, i32 1}
