target triple = "amdgcn-amd-amdhsa"
target datalayout = "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9"

@factor = internal addrspace(4) constant float bitcast (i32 1082130432 to float)
@counter = external addrspace(1) global i64

declare i32 @llvm.amdgcn.workitem.id.x()
declare void @llvm.amdgcn.s.barrier()
declare float @llvm.sqrt.f32(float)

define internal ccc float @scale(float %value) #0 {
bb0:
  %v3 = fmul float %value, bitcast (i32 1056964608 to float)
  %v4 = call float @llvm.sqrt.f32(float %v3)
  ret float %v4
}

define amdgpu_kernel void @write_scaled(ptr addrspace(1) noalias captures(none) nonnull writeonly align 4 dereferenceable(4096) %output, i64 %length) #1 {
bb0:
  %v9 = load float, ptr addrspace(4) @factor, align 4
  %v10 = call i32 @llvm.amdgcn.workitem.id.x()
  %v11 = zext i32 %v10 to i64
  %v12 = icmp ult i64 %v11, %length
  br i1 %v12, label %bb1, label %bb2
bb1:
  %v13 = getelementptr float, ptr addrspace(1) %output, i64 %v11
  %v14 = call ccc float @scale(float %v9)
  store float %v14, ptr addrspace(1) %v13, align 4
  call void @llvm.amdgcn.s.barrier()
  br label %bb2
bb2:
  ret void
}

attributes #0 = { nounwind alwaysinline memory(none) willreturn "target-cpu"="gfx942" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" }
attributes #1 = { nounwind "amdgpu-flat-work-group-size"="64,256" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" "fp-contract"="off" "target-cpu"="gfx942" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" }

!llvm.module.flags = !{!0, !1, !2}
!opencl.ocl.version = !{!3}
!opencl.spir.version = !{!4}
!llvm.ident = !{!5}
!fe2o3.handoff.identity = !{!6}

!0 = !{i32 1, !"amdhsa_code_object_version", i32 600}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 1, !"wchar_size", i32 4}
!3 = !{i32 2, i32 0}
!4 = !{i32 2, i32 0}
!5 = !{!"sha256:5151515151515151515151515151515151515151515151515151515151515151"}
!6 = !{!"sha256:ed9e5d893717bce0e07d15ec55d49e45026a8e387e9548d552481b3ff67acbaf"}
