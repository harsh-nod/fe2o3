; Fixed-work qualification fixture; no caller-supplied iteration count.
target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1

define amdgpu_kernel void @mixed_long(ptr addrspace(1) noalias align 4 %data, i64 %data.len) #0 !reqd_work_group_size !0 {
entry:
  %lane.i32 = call i32 @llvm.amdgcn.workitem.id.x()
  %lane = zext i32 %lane.i32 to i64
  %in.range = icmp ult i64 %lane, %data.len
  br i1 %in.range, label %load, label %exit

load:
  %index = add i64 %lane, 16
  %element = getelementptr i32, ptr addrspace(1) %data, i64 %index
  %initial = load i32, ptr addrspace(1) %element, align 4
  br label %loop

loop:
  %step = phi i32 [ 0, %load ], [ %next.step, %loop ]
  %value = phi i32 [ %initial, %load ], [ %next.value, %loop ]
  %product = mul i32 %value, 1664525
  %next.value = add i32 %product, 1013904223
  %next.step = add nuw i32 %step, 1
  %finished = icmp eq i32 %next.step, 33554433
  br i1 %finished, label %store, label %loop, !llvm.loop !1

store:
  store i32 %next.value, ptr addrspace(1) %element, align 4
  br label %exit

exit:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 64, i32 1, i32 1}
!1 = distinct !{!1, !2}
!2 = !{!"llvm.loop.unroll.disable"}
