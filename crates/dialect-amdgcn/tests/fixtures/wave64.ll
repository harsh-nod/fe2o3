target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1
declare i32 @llvm.amdgcn.mbcnt.lo(i32, i32) #1
declare i32 @llvm.amdgcn.mbcnt.hi(i32, i32) #1
declare i64 @llvm.amdgcn.ballot.i64(i1) #2
declare i32 @llvm.amdgcn.ds.bpermute(i32, i32) #2

define amdgpu_kernel void @wave_kernel(i1 %arg0, i32 %arg1, i32 %arg2) #0 !reqd_work_group_size !0 {
bb0:
  %v3.lo = call i32 @llvm.amdgcn.mbcnt.lo(i32 -1, i32 0)
  %v3 = call i32 @llvm.amdgcn.mbcnt.hi(i32 -1, i32 %v3.lo)
  %v4 = call i64 @llvm.amdgcn.ballot.i64(i1 %arg0)
  %v5.mask = call i64 @llvm.amdgcn.ballot.i64(i1 %arg0)
  %v5 = icmp ne i64 %v5.mask, 0
  %v6.mask = call i64 @llvm.amdgcn.ballot.i64(i1 %arg0)
  %v6 = icmp eq i64 %v6.mask, -1
  %v7.lane.lo = call i32 @llvm.amdgcn.mbcnt.lo(i32 -1, i32 0)
  %v7.lane = call i32 @llvm.amdgcn.mbcnt.hi(i32 -1, i32 %v7.lane.lo)
  %v7.tile.base = and i32 %v7.lane, -32
  %v7.tile.relative = and i32 %arg2, 31
  %v7.source = or i32 %v7.tile.base, %v7.tile.relative
  %v7.source.byte = shl i32 %v7.source, 2
  %v7 = call i32 @llvm.amdgcn.ds.bpermute(i32 %v7.source.byte, i32 %arg1)
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-features"="-wavefrontsize32,+wavefrontsize64" }
attributes #1 = { nounwind readnone speculatable willreturn }
attributes #2 = { convergent nounwind }

!0 = !{i32 64, i32 1, i32 1}
