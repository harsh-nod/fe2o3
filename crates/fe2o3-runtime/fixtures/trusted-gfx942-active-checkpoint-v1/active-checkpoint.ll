; Repository-owned finite-liveness kernel for direct-KFD stopped-checkpoint qualification.
; The checked artifact is built by build-and-verify.sh from this exact module.
target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare ptr addrspace(4) @llvm.amdgcn.implicitarg.ptr() #1

define amdgpu_kernel void @active_checkpoint_liveness(ptr addrspace(1) noalias writeonly align 4 %output) #0 !reqd_work_group_size !0 {
entry:
  %lane = call i32 @llvm.amdgcn.workitem.id.x()
  %implicit = call ptr addrspace(4) @llvm.amdgcn.implicitarg.ptr()
  %implicit.probe = load volatile i8, ptr addrspace(4) %implicit, align 1
  br label %spin

spin:
  %iteration = phi i32 [ 0, %entry ], [ %next, %spin ]
  call void asm sideeffect "", ""()
  %next = add nuw i32 %iteration, 1
  %pending = icmp ult i32 %next, 1000000000
  br i1 %pending, label %spin, label %complete, !llvm.loop !1

complete:
  %slot = getelementptr i32, ptr addrspace(1) %output, i32 %lane
  store i32 %next, ptr addrspace(1) %slot, align 4
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "amdgpu-implicitarg-num-bytes"="256" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 64, i32 1, i32 1}
!1 = distinct !{!1, !2}
!2 = !{!"llvm.loop.unroll.disable"}
!llvm.module.flags = !{!3}
!3 = !{i32 1, !"amdhsa_code_object_version", i32 600}
