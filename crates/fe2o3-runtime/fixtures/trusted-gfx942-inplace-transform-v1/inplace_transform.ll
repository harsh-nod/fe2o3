; Repository-owned in-place u32 transform for the bounded gfx942 runtime lane.
; The checked artifact is built by build-and-verify.sh from this exact module.
target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1
declare i32 @llvm.fshl.i32(i32, i32, i32) #1

define amdgpu_kernel void @inplace_transform(ptr addrspace(1) noalias align 4 %data, i64 %data.len) #0 !reqd_work_group_size !0 {
entry:
  %local.i32 = call i32 @llvm.amdgcn.workitem.id.x()
  %group.i32 = call i32 @llvm.amdgcn.workgroup.id.x()
  %local = zext i32 %local.i32 to i64
  %group = zext i32 %group.i32 to i64
  %base = mul i64 %group, 256
  %index = add i64 %base, %local
  %in.range = icmp ult i64 %index, %data.len
  br i1 %in.range, label %transform, label %exit

transform:
  %element.ptr = getelementptr i32, ptr addrspace(1) %data, i64 %index
  %value = load i32, ptr addrspace(1) %element.ptr, align 4
  %rotated = call i32 @llvm.fshl.i32(i32 %value, i32 %value, i32 13)
  %mixed = xor i32 %rotated, -1640531527
  %index.i32 = trunc i64 %index to i32
  %result = add i32 %mixed, %index.i32
  store i32 %result, ptr addrspace(1) %element.ptr, align 4
  br label %exit

exit:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="256,256" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 256, i32 1, i32 1}
