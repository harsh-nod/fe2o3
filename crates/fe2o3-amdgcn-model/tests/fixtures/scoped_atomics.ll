target triple = "amdgcn-amd-amdhsa"

@__fe2o3_lds_scoped_atomics_4 = internal addrspace(3) global [1 x i32] undef, align 4

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1

define amdgpu_kernel void @scoped_atomics(ptr addrspace(1) %arg0.data, i64 %arg0.len, i32 %arg1, i32 %arg2) #0 !reqd_work_group_size !0 {
bb0:
  %v3 = getelementptr i8, ptr addrspace(1) %arg0.data, i64 0
  %v4 = getelementptr [1 x i32], ptr addrspace(3) @__fe2o3_lds_scoped_atomics_4, i32 0, i32 0
  %v5 = load atomic i32, ptr addrspace(1) %v3 syncscope("workgroup") monotonic, align 4
  store atomic i32 %arg1, ptr addrspace(1) %v3 syncscope("agent") release, align 4
  %v6 = atomicrmw xchg ptr addrspace(1) %v3, i32 %arg1 seq_cst, align 4
  %v7.cmpxchg = cmpxchg ptr addrspace(1) %v3, i32 %arg2, i32 %arg1 syncscope("agent") acq_rel acquire, align 4
  %v7 = extractvalue { i32, i1 } %v7.cmpxchg, 0
  %v8 = extractvalue { i32, i1 } %v7.cmpxchg, 1
  %v9 = atomicrmw add ptr addrspace(3) %v4, i32 %arg1 syncscope("workgroup") monotonic, align 4
  %v10 = atomicrmw sub ptr addrspace(1) %v3, i32 %arg1 syncscope("workgroup") monotonic, align 4
  %v11 = atomicrmw umin ptr addrspace(1) %v3, i32 %arg1 syncscope("agent") acquire, align 4
  %v12 = atomicrmw umax ptr addrspace(1) %v3, i32 %arg1 syncscope("agent") release, align 4
  %v13 = atomicrmw and ptr addrspace(1) %v3, i32 %arg1 acq_rel, align 4
  %v14 = atomicrmw or ptr addrspace(1) %v3, i32 %arg1 seq_cst, align 4
  %v15 = atomicrmw xor ptr addrspace(1) %v3, i32 %arg1 syncscope("workgroup") monotonic, align 4
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 64, i32 1, i32 1}
