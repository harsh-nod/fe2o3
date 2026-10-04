; Repository-owned qualification kernel for the bounded gfx942 runtime lanes.
; The checked artifact is built by build-and-verify.sh from this exact module.
target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1

define amdgpu_kernel void @vecadd(ptr addrspace(1) noalias readonly align 4 %arg0.data, i64 %arg0.len, ptr addrspace(1) noalias readonly align 4 %arg1.data, i64 %arg1.len, ptr addrspace(1) noalias writeonly align 4 %arg2.data, i64 %arg2.len) #0 !reqd_work_group_size !0 {
entry:
  %local.i32 = call i32 @llvm.amdgcn.workitem.id.x()
  %group.i32 = call i32 @llvm.amdgcn.workgroup.id.x()
  %local = zext i32 %local.i32 to i64
  %group = zext i32 %group.i32 to i64
  %base = mul i64 %group, 256
  %index = add i64 %base, %local
  %in.output = icmp ult i64 %index, %arg2.len
  br i1 %in.output, label %check.inputs, label %exit

check.inputs:
  %in.left = icmp ult i64 %index, %arg0.len
  %in.right = icmp ult i64 %index, %arg1.len
  %in.inputs = and i1 %in.left, %in.right
  br i1 %in.inputs, label %add, label %exit

add:
  %left.ptr = getelementptr float, ptr addrspace(1) %arg0.data, i64 %index
  %right.ptr = getelementptr float, ptr addrspace(1) %arg1.data, i64 %index
  %output.ptr = getelementptr float, ptr addrspace(1) %arg2.data, i64 %index
  %left = load float, ptr addrspace(1) %left.ptr, align 4
  %right = load float, ptr addrspace(1) %right.ptr, align 4
  %sum = fadd float %left, %right
  store float %sum, ptr addrspace(1) %output.ptr, align 4
  br label %exit

exit:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="256,256" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 256, i32 1, i32 1}
