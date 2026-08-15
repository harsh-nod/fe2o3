use fe2o3_kernel_ir::{
    SCOPED_ATOMIC_V1_DESCRIPTOR_SYMBOL, SCOPED_ATOMIC_V1_KERNEL_ID, SCOPED_ATOMIC_V1_NAMESPACE,
    SCOPED_ATOMIC_V1_SOURCE_SHA256,
};

use crate::workgroup_sync_v1_worker::{ExactWorkgroupSyncProfileV1, WorkgroupSyncProfileKindV1};

pub(crate) const PROFILE: ExactWorkgroupSyncProfileV1 = ExactWorkgroupSyncProfileV1 {
    kind: WorkgroupSyncProfileKindV1::ScopedAtomic,
    kernel: SCOPED_ATOMIC_V1_KERNEL_ID,
    descriptor: SCOPED_ATOMIC_V1_DESCRIPTOR_SYMBOL,
    source_sha256: SCOPED_ATOMIC_V1_SOURCE_SHA256,
    namespace: SCOPED_ATOMIC_V1_NAMESPACE,
    source_authority: [
        0xcb, 0xe0, 0x12, 0xbd, 0xe7, 0x63, 0x22, 0x7f, 0xcf, 0x4e, 0x22, 0x35, 0x75, 0xf8,
        0x93, 0x5f, 0xd8, 0x6b, 0xfa, 0xf2, 0xc9, 0x81, 0x72, 0xb0, 0x9d, 0xe1, 0x1f, 0xf0,
        0x79, 0xb1, 0x34, 0x86,
    ],
    portable_mir: [
        0x52, 0x1d, 0xec, 0x6e, 0x8e, 0x00, 0xb3, 0x8a, 0x4c, 0x47, 0x9c, 0xf3, 0xb9, 0x3d,
        0x51, 0x54, 0x43, 0x18, 0xcd, 0x2b, 0xac, 0xe9, 0xb0, 0x8c, 0x56, 0xe2, 0xd6, 0xaf,
        0x57, 0xab, 0x37, 0xf5,
    ],
    fn_abi: [
        0xfa, 0xd7, 0x32, 0x25, 0x2d, 0xa6, 0x44, 0xac, 0xb7, 0xa3, 0x8f, 0x09, 0x13, 0xe0,
        0x62, 0x46, 0x12, 0x09, 0x3a, 0x7d, 0x98, 0x29, 0x42, 0x49, 0x7c, 0x3d, 0xe4, 0xda,
        0x4f, 0x4b, 0xc8, 0x2f,
    ],
    compiler_semantics: [
        0xbc, 0xf7, 0xe8, 0x74, 0xdb, 0x23, 0x61, 0x57, 0xdd, 0x6a, 0x8d, 0x8d, 0x76, 0xc6,
        0x9b, 0x69, 0x04, 0x17, 0x3e, 0xfe, 0xb5, 0x4f, 0x89, 0x05, 0xb9, 0xae, 0x1d, 0x48,
        0x10, 0xca, 0x7b, 0x76,
    ],
    trusted_terminals: [
        0x20, 0xa0, 0x07, 0x6e, 0x0e, 0xe9, 0xeb, 0x4e, 0x8d, 0xd9, 0x0e, 0x60, 0x1b, 0x36,
        0x8f, 0xf3, 0x95, 0x78, 0x5d, 0xfe, 0xf1, 0xfd, 0x5c, 0x80, 0x6d, 0x13, 0x18, 0x74,
        0x16, 0x75, 0xe8, 0x14,
    ],
    compiler_crate_binding:
        "dede4079399a3df33da7bcc9fc46bc84c3ab329642fa27241feaf10aff06388e",
    abi_binding: b"ptr64;size=40;align=8;values@0:16:8:slice-u32:shared-readonly;eligible@16:16:8:slice-u32:shared-readonly;target@32:8:8:global-mut-u32:host-unique:device-shared-atomic",
    effect_binding: b"eligible-lane-exactly-once;fetch-add-u32;ordering=relaxed;scope=system;address-space=global;one-live-aligned-atomic;mathematical-sum-fits-u32",
    resource_binding: b"target=gfx942:xnack-;cov=6;wave=64;block=64,1,1;grid=1,1,1;static-lds=0;required-dynamic-lds=0;maximum-dynamic-lds=0;cov6-hidden-dynamic-lds-size=absent;capability=atomics",
    canonical_ir_binding: b"fe2o3::scoped_atomic_add_v1;conditional-nonzero-eligibility;fetch-add-u32-relaxed-system-global;unique-host-borrow;lanes-alias-one-atomic",
    producer_version: "typed-scoped-atomic-gfx942-cov6-v1",
    llvm_body: LLVM_BODY,
};

const LLVM_BODY: &str = r#"target triple = "amdgcn-amd-amdhsa"
target datalayout = "e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare void @llvm.trap() #2

define amdgpu_kernel void @scoped_atomic_add_u32_v1(ptr addrspace(1) noalias nocapture readonly align 4 %values.data, i64 %values.len, ptr addrspace(1) noalias nocapture readonly align 4 %eligible.data, i64 %eligible.len, i64 %target.address) #0 !reqd_work_group_size !0 !kernel_arg_access_qual !1 !kernel_arg_type !2 !kernel_arg_base_type !2 !kernel_arg_type_qual !3 {
entry:
  %lane = call i32 @llvm.amdgcn.workitem.id.x()
  %lane.ok = icmp ult i32 %lane, 64
  %values.ok = icmp eq i64 %values.len, 64
  %eligible.ok = icmp eq i64 %eligible.len, 64
  %lengths.ok = and i1 %values.ok, %eligible.ok
  %target.aligned = icmp eq i64 (and i64 %target.address, 3), 0
  %target.nonnull = icmp ne i64 %target.address, 0
  %target.ok = and i1 %target.aligned, %target.nonnull
  %shape.ok = and i1 %lane.ok, %lengths.ok
  %valid = and i1 %shape.ok, %target.ok
  br i1 %valid, label %inspect, label %trap

trap:
  call void @llvm.trap()
  unreachable

inspect:
  %lane64 = zext i32 %lane to i64
  %eligible.ptr = getelementptr inbounds i32, ptr addrspace(1) %eligible.data, i64 %lane64
  %eligible.value = load i32, ptr addrspace(1) %eligible.ptr, align 4
  %participates = icmp ne i32 %eligible.value, 0
  br i1 %participates, label %atomic, label %return

atomic:
  %value.ptr = getelementptr inbounds i32, ptr addrspace(1) %values.data, i64 %lane64
  %value = load i32, ptr addrspace(1) %value.ptr, align 4
  %target.ptr = inttoptr i64 %target.address to ptr addrspace(1)
  %old = atomicrmw add ptr addrspace(1) %target.ptr, i32 %value syncscope("system") monotonic, align 4
  br label %return

return:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" "target-cpu"="gfx942" }
attributes #1 = { nounwind readnone speculatable willreturn }
attributes #2 = { cold noreturn nounwind }

!0 = !{i32 64, i32 1, i32 1}
!1 = !{!"read_only", !"none", !"read_only", !"none", !"none"}
!2 = !{!"uint*", !"ulong", !"uint*", !"ulong", !"ulong"}
!3 = !{!"const", !"", !"const", !"", !""}
"#;
