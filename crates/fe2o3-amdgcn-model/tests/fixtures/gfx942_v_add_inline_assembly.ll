target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1

define amdgpu_kernel void @assembly_kernel(i32 %arg0, i32 %arg1) #0 !reqd_work_group_size !0 {
bb0:
  %v2 = call i32 asm "v_add_u32 $0, $1, $2", "=v,v,v"(i32 %arg0, i32 %arg1)
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 64, i32 1, i32 1}
