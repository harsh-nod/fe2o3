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
        0xbb, 0x05, 0xd2, 0xed, 0xce, 0x90, 0x93, 0xf5, 0x3e, 0x68, 0xb6, 0x37, 0xe8, 0xa4,
        0x6f, 0x70, 0x9a, 0x34, 0x1c, 0x29, 0x5a, 0x14, 0xfd, 0xee, 0xd7, 0x44, 0xfd, 0xa4,
        0x7c, 0x3f, 0xdf, 0x3a,
    ],
    portable_mir: [
        0x20, 0xd5, 0x49, 0x5b, 0x23, 0x66, 0x24, 0xc5, 0x1a, 0x67, 0x87, 0xd9, 0x95, 0x56,
        0x94, 0x56, 0xb1, 0xa6, 0xbb, 0xfc, 0x7c, 0x70, 0xe5, 0x43, 0x69, 0x4d, 0xeb, 0xc6,
        0x2b, 0xeb, 0x46, 0xb1,
    ],
    fn_abi: [
        0xb3, 0x84, 0x04, 0x57, 0xdb, 0x66, 0x5f, 0x11, 0x4c, 0xae, 0xff, 0x92, 0xa4, 0xc7,
        0xdd, 0xbe, 0x63, 0x88, 0xac, 0x14, 0xbe, 0xc4, 0x8c, 0x29, 0x77, 0xc9, 0xa6, 0x21,
        0x16, 0x81, 0x40, 0xc6,
    ],
    compiler_semantics: [
        0x1c, 0x9f, 0xfd, 0xb9, 0x49, 0xc2, 0x18, 0xc2, 0xca, 0xd9, 0x87, 0x55, 0x89, 0x57,
        0xa2, 0x71, 0x9a, 0xf4, 0x92, 0x34, 0x91, 0x98, 0xbe, 0x95, 0xa9, 0x43, 0xf8, 0x46,
        0x91, 0x05, 0xfd, 0xf2,
    ],
    trusted_terminals: [
        0x50, 0x97, 0xff, 0x92, 0xf4, 0x88, 0x1d, 0x71, 0x17, 0x18, 0x29, 0x30, 0x84, 0x8d,
        0x55, 0xab, 0x78, 0x1e, 0xe6, 0x82, 0x24, 0xe1, 0xac, 0x78, 0x9e, 0xbf, 0x85, 0xf8,
        0xbd, 0x41, 0x98, 0xcf,
    ],
    compiler_crate_binding:
        "fd63fb50f774e07f310d4b967e6fefbccf4a33d7abcf7096924037702cd8d0da",
    abi_binding: b"ptr64;size=40;align=8;values@0:16:8:slice-i32:shared-readonly;epoch@16:4:4:u32:value;output@24:16:8:slice-i32:unique-readwrite",
    effect_binding: b"one-linear-lds-allocation:i32x64:256-bytes:align4:no-escape;all-64-threads-convergent;lane-publish;publish-read-barrier;read;read-reuse-barrier;lane0-only-output",
    resource_binding: b"target=gfx942:xnack-;cov=6;wave=64;block=64,1,1;grid=1,1,1;static-lds=0;required-dynamic-lds=256;maximum-dynamic-lds=256;cov6-hidden-dynamic-lds-size@relative120:field4:required-value256;allocation-count=1",
    canonical_ir_binding: b"fe2o3::workgroup_lds_reduction_v1;exact-i32x64-scratch;epochs=uninitialized,lane-initialized,published,read,reusable;barriers=publish-read,read-reuse;output=lane0",
    producer_version: "typed-workgroup-lds-reduction-gfx942-cov6-v1",
    llvm_body: LLVM_BODY,
};

const LLVM_BODY: &str = r#"target triple = "amdgcn-amd-amdhsa"
target datalayout = "e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9"

@__fe2o3_lds_reduction_v1_scratch = external addrspace(3) global [0 x i32], align 4

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare void @llvm.amdgcn.s.barrier() #2
declare void @llvm.trap() #3

define amdgpu_kernel void @lds_publish_read_reduce_i32_v1(ptr addrspace(1) noalias nocapture readonly align 4 %values.data, i64 %values.len, i32 %epoch, ptr addrspace(1) noalias nocapture align 4 %output.data, i64 %output.len) #0 !reqd_work_group_size !0 !kernel_arg_access_qual !1 !kernel_arg_type !2 !kernel_arg_base_type !2 !kernel_arg_type_qual !3 {
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
!1 = !{!"read_only", !"none", !"none", !"read_write", !"none"}
!2 = !{!"int*", !"ulong", !"uint", !"int*", !"ulong"}
!3 = !{!"const", !"", !"", !"restrict", !""}
"#;
