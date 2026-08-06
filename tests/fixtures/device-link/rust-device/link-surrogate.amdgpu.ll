; Link-test surrogate for the compiler output represented by src/lib.rs.
; This is deliberately inert test input, not compiler-derived evidence.

target triple = "amdgcn-amd-amdhsa"

declare i32 @external_scale_bias_v1(i32, i32) #0
declare i32 @llvm.amdgcn.workitem.id.x() #1

define protected i32 @rust_accumulate_v1(i32 %value, i32 %lane) #0 {
entry:
  %result = add i32 %value, %lane
  ret i32 %result
}

define protected amdgpu_kernel void @rust_calls_hip_kernel_v1(
    ptr addrspace(1) %input, ptr addrspace(1) %output, i64 %count) #2 {
entry:
  %lane32 = call i32 @llvm.amdgcn.workitem.id.x()
  %lane = zext i32 %lane32 to i64
  %in.bounds = icmp ult i64 %lane, %count
  br i1 %in.bounds, label %body, label %exit

body:
  %input.ptr = getelementptr i32, ptr addrspace(1) %input, i64 %lane
  %value = load i32, ptr addrspace(1) %input.ptr, align 4
  %result = call i32 @external_scale_bias_v1(i32 %value, i32 %lane32)
  %output.ptr = getelementptr i32, ptr addrspace(1) %output, i64 %lane
  store i32 %result, ptr addrspace(1) %output.ptr, align 4
  br label %exit

exit:
  ret void
}

attributes #0 = { nounwind memory(none) "target-cpu"="gfx942" "target-features"="+sramecc,-xnack" }
attributes #1 = { nounwind memory(none) }
attributes #2 = { nounwind "target-cpu"="gfx942" "target-features"="+sramecc,-xnack" }

!llvm.module.flags = !{!0, !1, !2}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
!1 = !{i32 1, !"amdgpu.sramecc", i32 1}
!2 = !{i32 1, !"amdgpu.xnack", i32 0}
