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
        0x7d, 0x17, 0xda, 0x2c, 0x01, 0xea, 0xff, 0x69, 0xe2, 0xd9, 0x57, 0x0c, 0x23, 0xd7,
        0xd2, 0x74, 0xf8, 0x3a, 0xa1, 0x89, 0xf6, 0x0b, 0xbc, 0xb0, 0x54, 0x44, 0x9b, 0x62,
        0x85, 0xe6, 0xda, 0x4f,
    ],
    portable_mir: [
        0x46, 0xf7, 0x44, 0x75, 0x1a, 0x18, 0x75, 0x78, 0x9f, 0x1d, 0x32, 0x48, 0x02, 0xfc,
        0xb9, 0xf3, 0x36, 0x4c, 0x75, 0xe2, 0xfc, 0xe9, 0x3f, 0x92, 0xdd, 0x96, 0xbf, 0xe0,
        0x3b, 0x71, 0xfe, 0x0e,
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
        0x53, 0x8c, 0x3b, 0x0d, 0x67, 0x20, 0x4c, 0x78, 0x3f, 0x64, 0x1c, 0xd6, 0x0d, 0x8d,
        0x0b, 0x8f, 0xbf, 0x9a, 0x5f, 0x68, 0x35, 0xba, 0xa0, 0x8c, 0x7e, 0x47, 0xa6, 0x48,
        0xf1, 0xee, 0x39, 0x71,
    ],
    compiler_crate_binding:
        "dede4079399a3df33da7bcc9fc46bc84c3ab329642fa27241feaf10aff06388e",
    abi_binding: b"ptr64;size=40;align=8;values@0:16:8:slice-u32:shared-readonly;eligible@16:16:8:slice-u32:shared-readonly;target@32:8:8:global-mut-u32:host-unique:device-shared-atomic",
    effect_binding: b"eligible-lane-exactly-once;fetch-add-u32;ordering=relaxed;scope=system;address-space=global;one-live-aligned-atomic;mathematical-sum-fits-u32",
    resource_binding: b"target=gfx942:xnack-;cov=6;wave=64;block=64,1,1;grid=1,1,1;static-lds=0;required-dynamic-lds=0;maximum-dynamic-lds=0;cov6-hidden-dynamic-lds-size=absent;capability=atomics",
    canonical_ir_binding: b"fe2o3::scoped_atomic_add_v1;conditional-nonzero-eligibility;fetch-add-u32-relaxed-system-global;unique-host-borrow;lanes-alias-one-atomic",
    producer_version: "typed-scoped-atomic-gfx942-cov6-v1",
    llvm_body_tail: LLVM_BODY_TAIL,
};

const LLVM_BODY_TAIL: &str = r#"declare i32 @llvm.amdgcn.workitem.id.x() #1
declare void @llvm.trap() #2

define amdgpu_kernel void @scoped_atomic_add_u32_v1(ptr addrspace(1) noalias nocapture readonly align 4 %values.data, i64 %values.len, ptr addrspace(1) noalias nocapture readonly align 4 %eligible.data, i64 %eligible.len, i64 %target.address) #0 !reqd_work_group_size !0 !kernel_arg_access_qual !1 !kernel_arg_type !2 !kernel_arg_base_type !2 !kernel_arg_type_qual !3 {
entry:
  %lane = call i32 @llvm.amdgcn.workitem.id.x()
  %lane.ok = icmp ult i32 %lane, 64
  %values.ok = icmp eq i64 %values.len, 64
  %eligible.ok = icmp eq i64 %eligible.len, 64
  %lengths.ok = and i1 %values.ok, %eligible.ok
  %target.low-bits = and i64 %target.address, 3
  %target.aligned = icmp eq i64 %target.low-bits, 0
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
  %old = atomicrmw add ptr addrspace(1) %target.ptr, i32 %value monotonic, align 4
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
