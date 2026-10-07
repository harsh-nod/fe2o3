target triple = "amdgcn-amd-amdhsa"
target datalayout = "e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9"

declare void @llvm.pseudoprobe(i64, i64, i32, i64)
declare i32 @llvm.amdgcn.workgroup.id.x() #1
declare i32 @llvm.amdgcn.workitem.id.x() #1

define amdgpu_kernel void @fill_write_only(ptr addrspace(1) %arg0.data, i64 %arg0.len) #0 !reqd_work_group_size !0 {
bb2:
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 1, i32 0, i64 -1)
  %v8.local.i32 = call i32 @llvm.amdgcn.workitem.id.x()
  %v8.group.i32 = call i32 @llvm.amdgcn.workgroup.id.x()
  %v8.local = zext i32 %v8.local.i32 to i64
  %v8.group = zext i32 %v8.group.i32 to i64
  %v8.base = mul i64 %v8.group, 64
  %v8 = add i64 %v8.base, %v8.local
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 2, i32 0, i64 -1)
  %v9 = add i64 %v8, 0
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 3, i32 0, i64 -1)
  %v10 = trunc i64 %v9 to i32
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 4, i32 0, i64 -1)
  %v11 = add i64 %arg0.len, 0
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 5, i32 0, i64 -1)
  %v12 = icmp ult i64 %v8, %v11
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 6, i32 0, i64 -1)
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 7, i32 0, i64 -1)
  %v14 = select i1 %v12, i64 %v8, i64 0
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 8, i32 0, i64 -1)
  %v15 = getelementptr i8, ptr addrspace(1) %arg0.data, i64 0
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 9, i32 0, i64 -1)
  %v16 = getelementptr i32, ptr addrspace(1) %v15, i64 %v14
  call void @llvm.pseudoprobe(i64 1117723068154451651, i64 10, i32 0, i64 -1)
  br i1 %v12, label %guarded_store_bb2_op9_true, label %guarded_store_bb2_op9_merge
guarded_store_bb2_op9_true:
  store i32 %v10, ptr addrspace(1) %v16, align 4
  br label %guarded_store_bb2_op9_merge
guarded_store_bb2_op9_merge:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" "fp-contract"="off" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 64, i32 1, i32 1}
!llvm.pseudo_probe_desc = !{!1}
!1 = !{i64 1117723068154451651, i64 4205197201937875056, !"fill_write_only"}
!fe2o3.semantic_anchor.v1 = !{!2, !3, !4, !5, !6, !7, !8, !9, !10, !11, !12}
!2 = !{!"sha256:8953d03f8c954a4d11aacc156e4d1cf3cbf9422d663509a0a8114110fde2f582", !"kir-version:9", i64 563, !"target:gfx942:xnack-", i64 1117723068154451651, i64 4205197201937875056, i64 1, i64 10}
!3 = !{i64 1, i64 0, i64 0, i64 0}
!4 = !{i64 2, i64 0, i64 0, i64 1}
!5 = !{i64 3, i64 0, i64 0, i64 2}
!6 = !{i64 4, i64 0, i64 0, i64 3}
!7 = !{i64 5, i64 0, i64 0, i64 4}
!8 = !{i64 6, i64 0, i64 0, i64 5}
!9 = !{i64 7, i64 0, i64 0, i64 6}
!10 = !{i64 8, i64 0, i64 0, i64 7}
!11 = !{i64 9, i64 0, i64 0, i64 8}
!12 = !{i64 10, i64 0, i64 0, i64 9}
