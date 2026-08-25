use fe2o3_kernel_ir::{
    LDS_REDUCTION_V1_DESCRIPTOR_SYMBOL, LDS_REDUCTION_V1_KERNEL_ID, LDS_REDUCTION_V1_NAMESPACE,
    LDS_REDUCTION_V1_SOURCE_SHA256,
};

use crate::workgroup_sync_v1_worker::{ExactWorkgroupSyncProfileV1, WorkgroupSyncProfileKindV1};

pub(crate) const PROFILE: ExactWorkgroupSyncProfileV1 = ExactWorkgroupSyncProfileV1 {
    kind: WorkgroupSyncProfileKindV1::LdsReduction,
    kernel: LDS_REDUCTION_V1_KERNEL_ID,
    descriptor: LDS_REDUCTION_V1_DESCRIPTOR_SYMBOL,
    source_sha256: LDS_REDUCTION_V1_SOURCE_SHA256,
    namespace: LDS_REDUCTION_V1_NAMESPACE,
    source_authority: [
        0xe8, 0x20, 0xad, 0xdd, 0x6c, 0xca, 0x8e, 0xb2, 0x4a, 0x14, 0x9e, 0x69, 0xec, 0x95,
        0x30, 0x83, 0x64, 0x4e, 0x99, 0x5a, 0x56, 0x9e, 0xf5, 0x05, 0xe7, 0x60, 0x5b, 0x03,
        0x65, 0xe5, 0x49, 0x3f,
    ],
    portable_mir: [
        0xe7, 0x43, 0x90, 0xb3, 0x0c, 0x64, 0xc9, 0x78, 0xe0, 0x4d, 0x33, 0xd3, 0x34, 0x3a,
        0x79, 0xc8, 0x84, 0x34, 0xb0, 0xff, 0x05, 0x8d, 0x45, 0xfc, 0xe4, 0xcb, 0xc7, 0xa9,
        0x68, 0x5c, 0xdc, 0xcb,
    ],
    fn_abi: [
        0x9c, 0xdb, 0xeb, 0x1e, 0xfd, 0xe9, 0xf0, 0x35, 0x90, 0x6c, 0x9d, 0x57, 0x51, 0xda,
        0xf0, 0x8f, 0xff, 0x95, 0xfb, 0xb0, 0xbf, 0x41, 0x2b, 0x4e, 0xb0, 0xa1, 0xc4, 0x34,
        0x12, 0xbb, 0xc0, 0xe6,
    ],
    compiler_semantics: [
        0x1c, 0x9f, 0xfd, 0xb9, 0x49, 0xc2, 0x18, 0xc2, 0xca, 0xd9, 0x87, 0x55, 0x89, 0x57,
        0xa2, 0x71, 0x9a, 0xf4, 0x92, 0x34, 0x91, 0x98, 0xbe, 0x95, 0xa9, 0x43, 0xf8, 0x46,
        0x91, 0x05, 0xfd, 0xf2,
    ],
    trusted_terminals: [
        0x65, 0xb8, 0xff, 0x15, 0xd8, 0x60, 0xed, 0x7e, 0x30, 0xbe, 0x75, 0xe3, 0x24, 0xd1,
        0x0e, 0x91, 0x27, 0xeb, 0x1d, 0x6c, 0x64, 0xe8, 0x1d, 0x11, 0x10, 0xbc, 0xa5, 0xe0,
        0xca, 0xcd, 0x3f, 0xb0,
    ],
    compiler_crate_binding:
        "fd63fb50f774e07f310d4b967e6fefbccf4a33d7abcf7096924037702cd8d0da",
    abi_binding: b"ptr64;size=32;align=8;values@0:16:8:slice-i32:shared-readonly;output@16:16:8:slice-i32:unique-readwrite",
    effect_binding: b"one-linear-lds-allocation:i32x64:256-bytes:align4:no-escape;all-64-threads-convergent;lane-publish;publish-read-barrier;read;read-reuse-barrier;lane0-only-output",
    resource_binding: b"target=gfx942:xnack-;cov=6;wave=64;block=64,1,1;grid=1,1,1;static-lds=0;required-dynamic-lds=256;maximum-dynamic-lds=256;cov6-hidden-dynamic-lds-size@relative120:field4:required-value256;allocation-count=1",
    canonical_ir_binding: b"fe2o3::workgroup_lds_reduction_v1;exact-i32x64-scratch;epochs=uninitialized,lane-initialized,published,read,reusable;barriers=publish-read,read-reuse;output=lane0",
    producer_version: "typed-workgroup-lds-reduction-gfx942-cov6-v1",
    llvm_body_tail: LLVM_BODY_TAIL,
};

const LLVM_BODY_TAIL: &str = r#"@__fe2o3_lds_reduction_v1_scratch = external addrspace(3) global [0 x i32], align 4

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare void @llvm.amdgcn.s.barrier() #2
declare void @llvm.trap() #3

define amdgpu_kernel void @lds_publish_read_reduce_i32_v1(ptr addrspace(1) noalias nocapture readonly align 4 %values.data, i64 %values.len, ptr addrspace(1) noalias nocapture align 4 %output.data, i64 %output.len) #0 !reqd_work_group_size !0 !kernel_arg_access_qual !1 !kernel_arg_type !2 !kernel_arg_base_type !2 !kernel_arg_type_qual !3 {
entry:
  %lane = call i32 @llvm.amdgcn.workitem.id.x()
  %lane.ok = icmp ult i32 %lane, 64
  %values.ok = icmp eq i64 %values.len, 64
  %output.ok = icmp eq i64 %output.len, 1
  %lengths.ok = and i1 %values.ok, %output.ok
  %valid = and i1 %lane.ok, %lengths.ok
  br i1 %valid, label %publish, label %trap

trap:
  call void @llvm.trap()
  unreachable

publish:
  %lane64 = zext i32 %lane to i64
  %value.ptr = getelementptr inbounds i32, ptr addrspace(1) %values.data, i64 %lane64
  %value = load i32, ptr addrspace(1) %value.ptr, align 4
  %scratch.ptr = getelementptr inbounds [0 x i32], ptr addrspace(3) @__fe2o3_lds_reduction_v1_scratch, i32 0, i32 %lane
  store i32 %value, ptr addrspace(3) %scratch.ptr, align 4
  fence syncscope("workgroup") release
  call void @llvm.amdgcn.s.barrier()
  fence syncscope("workgroup") acquire
  %is.lane.zero = icmp eq i32 %lane, 0
  br i1 %is.lane.zero, label %reduce.loop, label %reuse.barrier

reduce.loop:
  %index = phi i32 [ 0, %publish ], [ %next.index, %reduce.loop ]
  %sum = phi i32 [ 0, %publish ], [ %next.sum, %reduce.loop ]
  %read.ptr = getelementptr inbounds [0 x i32], ptr addrspace(3) @__fe2o3_lds_reduction_v1_scratch, i32 0, i32 %index
  %read.value = load i32, ptr addrspace(3) %read.ptr, align 4
  %next.sum = add i32 %sum, %read.value
  %next.index = add nuw nsw i32 %index, 1
  %done = icmp eq i32 %next.index, 64
  br i1 %done, label %reduced, label %reduce.loop

reduced:
  br label %reuse.barrier

reuse.barrier:
  %result = phi i32 [ %next.sum, %reduced ], [ 0, %publish ]
  fence syncscope("workgroup") release
  call void @llvm.amdgcn.s.barrier()
  fence syncscope("workgroup") acquire
  br i1 %is.lane.zero, label %write, label %return

write:
  store i32 %result, ptr addrspace(1) %output.data, align 4
  br label %return

return:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" "target-cpu"="gfx942" }
attributes #1 = { nounwind readnone speculatable willreturn }
attributes #2 = { convergent nounwind }
attributes #3 = { cold noreturn nounwind }

!0 = !{i32 64, i32 1, i32 1}
!1 = !{!"read_only", !"none", !"read_write", !"none"}
!2 = !{!"int*", !"ulong", !"int*", !"ulong"}
!3 = !{!"const", !"", !"restrict", !""}
"#;
