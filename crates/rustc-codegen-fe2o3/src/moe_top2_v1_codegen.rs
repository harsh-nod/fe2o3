//! Closed LLVM lowering for the authenticated T8/E4/K2/C4 MoE router.

use fe2o3_kernel_ir::{MoeTop2KernelIrV1, MoeTop2ProfileV1, verify_moe_top2_v1};

pub(crate) const EXACT_MOE_TOP2_GFX942_DATA_LAYOUT_V1: &str = concat!(
    "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-",
    "p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-",
    "v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-",
    "n32:64-S32-A5-G1-ni:7:8:9",
);

pub(crate) const EMPTY_PROVIDER_CLOSURE_V1: &[u8] =
    b"provider-closure=none;imports=0;exports=0;external-declarations=llvm-intrinsics-only";

pub(crate) fn lower_exact_moe_top2_v1(
    ir: &MoeTop2KernelIrV1,
    profile: &MoeTop2ProfileV1,
) -> Result<String, &'static str> {
    verify_moe_top2_v1(ir, profile).map_err(|_| "noncanonical MoE Kernel IR or profile")?;
    let body = format!(
        "target triple = \"amdgcn-amd-amdhsa\"\n\
target datalayout = \"{EXACT_MOE_TOP2_GFX942_DATA_LAYOUT_V1}\"\n\n{LLVM_BODY_TAIL}"
    );
    audit_exact_body(&body)?;
    Ok(body)
}

fn audit_exact_body(body: &str) -> Result<(), &'static str> {
    let required = [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "define amdgpu_kernel void @moe_top2_route_f32_t8_e4_k2_c4_v1",
        "define internal i32 @__fe2o3_moe_select_expert_v1",
        "define internal i32 @__fe2o3_moe_requested_count_v1",
        "define internal i32 @__fe2o3_moe_admitted_count_v1",
        "define internal i32 @__fe2o3_moe_expert_offset_v1",
        "define internal i32 @__fe2o3_moe_route_slot_v1",
        "call i32 @llvm.amdgcn.workitem.id.x()",
        "call void @llvm.trap()",
        "icmp eq i64 %logits.len, 32",
        "and i32 %finite.bits, 2139095040",
        "fcmp ogt float",
        "store i32 %top2.value",
        "store i32 %requested.value",
        "store i32 %admitted.value",
        "store i32 %offset.value",
        "store i32 %route.slot",
        "store i32 %permutation.value",
        "!0 = !{i32 64, i32 1, i32 1}",
    ];
    if required.iter().any(|needle| !body.contains(needle))
        || body.matches("define amdgpu_kernel").count() != 1
        || body.matches("define internal").count() != 5
        || body.matches("store i32").count() != 7
        || body.contains("atomicrmw")
        || body.contains("addrspace(3)")
        || ["COMGR", "comgr", " shell ", " hip", " cuda"]
            .iter()
            .any(|needle| body.contains(needle))
    {
        return Err("exact MoE LLVM body audit failed");
    }
    Ok(())
}

const LLVM_BODY_TAIL: &str = r#"declare i32 @llvm.amdgcn.workitem.id.x() #1
declare void @llvm.trap() #2

define internal i32 @__fe2o3_moe_select_expert_v1(ptr addrspace(1) nocapture readonly align 4 %logits, i32 %token, i32 %rank) #3 {
entry:
  %base = mul nuw nsw i32 %token, 4
  %index0 = zext i32 %base to i64
  %index1.32 = add nuw nsw i32 %base, 1
  %index1 = zext i32 %index1.32 to i64
  %index2.32 = add nuw nsw i32 %base, 2
  %index2 = zext i32 %index2.32 to i64
  %index3.32 = add nuw nsw i32 %base, 3
  %index3 = zext i32 %index3.32 to i64
  %ptr0 = getelementptr inbounds float, ptr addrspace(1) %logits, i64 %index0
  %ptr1 = getelementptr inbounds float, ptr addrspace(1) %logits, i64 %index1
  %ptr2 = getelementptr inbounds float, ptr addrspace(1) %logits, i64 %index2
  %ptr3 = getelementptr inbounds float, ptr addrspace(1) %logits, i64 %index3
  %score0 = load float, ptr addrspace(1) %ptr0, align 4
  %score1 = load float, ptr addrspace(1) %ptr1, align 4
  %score2 = load float, ptr addrspace(1) %ptr2, align 4
  %score3 = load float, ptr addrspace(1) %ptr3, align 4
  %one.beats.zero = fcmp ogt float %score1, %score0
  %best01 = select i1 %one.beats.zero, i32 1, i32 0
  %best01.score = select i1 %one.beats.zero, float %score1, float %score0
  %two.beats.best = fcmp ogt float %score2, %best01.score
  %best012 = select i1 %two.beats.best, i32 2, i32 %best01
  %best012.score = select i1 %two.beats.best, float %score2, float %best01.score
  %three.beats.best = fcmp ogt float %score3, %best012.score
  %best = select i1 %three.beats.best, i32 3, i32 %best012
  %best.is0 = icmp eq i32 %best, 0
  %best.is1 = icmp eq i32 %best, 1
  %best.is2 = icmp eq i32 %best, 2
  %best.is3 = icmp eq i32 %best, 3
  %remaining0 = select i1 %best.is0, float 0xFFF0000000000000, float %score0
  %remaining1 = select i1 %best.is1, float 0xFFF0000000000000, float %score1
  %remaining2 = select i1 %best.is2, float 0xFFF0000000000000, float %score2
  %remaining3 = select i1 %best.is3, float 0xFFF0000000000000, float %score3
  %second.one.beats.zero = fcmp ogt float %remaining1, %remaining0
  %second01 = select i1 %second.one.beats.zero, i32 1, i32 0
  %second01.score = select i1 %second.one.beats.zero, float %remaining1, float %remaining0
  %second.two.beats.best = fcmp ogt float %remaining2, %second01.score
  %second012 = select i1 %second.two.beats.best, i32 2, i32 %second01
  %second012.score = select i1 %second.two.beats.best, float %remaining2, float %second01.score
  %second.three.beats.best = fcmp ogt float %remaining3, %second012.score
  %second = select i1 %second.three.beats.best, i32 3, i32 %second012
  %is.first = icmp eq i32 %rank, 0
  %selected = select i1 %is.first, i32 %best, i32 %second
  ret i32 %selected
}

define internal i32 @__fe2o3_moe_requested_count_v1(ptr addrspace(1) nocapture readonly align 4 %logits, i32 %expert) #3 {
entry:
  br label %loop
loop:
  %route = phi i32 [ 0, %entry ], [ %next.route, %loop ]
  %count = phi i32 [ 0, %entry ], [ %next.count, %loop ]
  %token = lshr i32 %route, 1
  %rank = and i32 %route, 1
  %selected = call i32 @__fe2o3_moe_select_expert_v1(ptr addrspace(1) %logits, i32 %token, i32 %rank)
  %matches = icmp eq i32 %selected, %expert
  %increment = zext i1 %matches to i32
  %next.count = add nuw nsw i32 %count, %increment
  %next.route = add nuw nsw i32 %route, 1
  %done = icmp eq i32 %next.route, 16
  br i1 %done, label %return, label %loop
return:
  ret i32 %next.count
}

define internal i32 @__fe2o3_moe_admitted_count_v1(ptr addrspace(1) nocapture readonly align 4 %logits, i32 %expert) #3 {
entry:
  %requested = call i32 @__fe2o3_moe_requested_count_v1(ptr addrspace(1) %logits, i32 %expert)
  %over.capacity = icmp ugt i32 %requested, 4
  %admitted = select i1 %over.capacity, i32 4, i32 %requested
  ret i32 %admitted
}

define internal i32 @__fe2o3_moe_expert_offset_v1(ptr addrspace(1) nocapture readonly align 4 %logits, i32 %expert) #3 {
entry:
  %is.zero = icmp eq i32 %expert, 0
  br i1 %is.zero, label %return.zero, label %loop
loop:
  %index = phi i32 [ 0, %entry ], [ %next.index, %loop ]
  %offset = phi i32 [ 0, %entry ], [ %next.offset, %loop ]
  %admitted = call i32 @__fe2o3_moe_admitted_count_v1(ptr addrspace(1) %logits, i32 %index)
  %next.offset = add nuw nsw i32 %offset, %admitted
  %next.index = add nuw nsw i32 %index, 1
  %done = icmp eq i32 %next.index, %expert
  br i1 %done, label %return.sum, label %loop
return.zero:
  ret i32 0
return.sum:
  ret i32 %next.offset
}

define internal i32 @__fe2o3_moe_route_slot_v1(ptr addrspace(1) nocapture readonly align 4 %logits, i32 %route) #3 {
entry:
  %token = lshr i32 %route, 1
  %rank.in.token = and i32 %route, 1
  %expert = call i32 @__fe2o3_moe_select_expert_v1(ptr addrspace(1) %logits, i32 %token, i32 %rank.in.token)
  %is.first.route = icmp eq i32 %route, 0
  br i1 %is.first.route, label %rank.ready.zero, label %rank.loop
rank.loop:
  %prior = phi i32 [ 0, %entry ], [ %next.prior, %rank.loop ]
  %stable.rank = phi i32 [ 0, %entry ], [ %next.rank, %rank.loop ]
  %prior.token = lshr i32 %prior, 1
  %prior.rank = and i32 %prior, 1
  %prior.expert = call i32 @__fe2o3_moe_select_expert_v1(ptr addrspace(1) %logits, i32 %prior.token, i32 %prior.rank)
  %matches = icmp eq i32 %prior.expert, %expert
  %increment = zext i1 %matches to i32
  %next.rank = add nuw nsw i32 %stable.rank, %increment
  %next.prior = add nuw nsw i32 %prior, 1
  %rank.done = icmp eq i32 %next.prior, %route
  br i1 %rank.done, label %rank.ready.sum, label %rank.loop
rank.ready.zero:
  br label %rank.ready
rank.ready.sum:
  br label %rank.ready
rank.ready:
  %final.rank = phi i32 [ 0, %rank.ready.zero ], [ %next.rank, %rank.ready.sum ]
  %accepted = icmp ult i32 %final.rank, 4
  %offset = call i32 @__fe2o3_moe_expert_offset_v1(ptr addrspace(1) %logits, i32 %expert)
  %accepted.slot = add nuw nsw i32 %offset, %final.rank
  %slot = select i1 %accepted, i32 %accepted.slot, i32 -1
  ret i32 %slot
}

define amdgpu_kernel void @moe_top2_route_f32_t8_e4_k2_c4_v1(ptr addrspace(1) noalias nocapture readonly align 4 %logits.data, i64 %logits.len, ptr addrspace(1) noalias nocapture align 4 %top2.data, i64 %top2.len, ptr addrspace(1) noalias nocapture align 4 %requested.data, i64 %requested.len, ptr addrspace(1) noalias nocapture align 4 %admitted.data, i64 %admitted.len, ptr addrspace(1) noalias nocapture align 4 %offsets.data, i64 %offsets.len, ptr addrspace(1) noalias nocapture align 4 %slots.data, i64 %slots.len, ptr addrspace(1) noalias nocapture align 4 %permutation.data, i64 %permutation.len, ptr addrspace(1) noalias nocapture align 4 %inverse.data, i64 %inverse.len) #0 !reqd_work_group_size !0 !kernel_arg_access_qual !1 !kernel_arg_type !2 !kernel_arg_base_type !2 !kernel_arg_type_qual !3 {
entry:
  %lane = call i32 @llvm.amdgcn.workitem.id.x()
  %lane.ok = icmp ult i32 %lane, 64
  br i1 %lane.ok, label %lane.valid, label %trap
lane.valid:
  %lane.zero = icmp eq i32 %lane, 0
  br i1 %lane.zero, label %shape, label %return
shape:
  %logits.ok = icmp eq i64 %logits.len, 32
  %top2.ok = icmp eq i64 %top2.len, 16
  %requested.ok = icmp eq i64 %requested.len, 4
  %admitted.ok = icmp eq i64 %admitted.len, 4
  %offsets.ok = icmp eq i64 %offsets.len, 5
  %slots.ok = icmp eq i64 %slots.len, 16
  %permutation.ok = icmp eq i64 %permutation.len, 16
  %inverse.ok = icmp eq i64 %inverse.len, 16
  %shape0 = and i1 %logits.ok, %top2.ok
  %shape1 = and i1 %requested.ok, %admitted.ok
  %shape2 = and i1 %offsets.ok, %slots.ok
  %shape3 = and i1 %permutation.ok, %inverse.ok
  %shape01 = and i1 %shape0, %shape1
  %shape23 = and i1 %shape2, %shape3
  %shape.ok = and i1 %shape01, %shape23
  br i1 %shape.ok, label %finite.loop, label %trap
finite.loop:
  %finite.index = phi i32 [ 0, %shape ], [ %finite.next, %finite.continue ]
  %finite.index64 = zext i32 %finite.index to i64
  %finite.ptr = getelementptr inbounds float, ptr addrspace(1) %logits.data, i64 %finite.index64
  %finite.value = load float, ptr addrspace(1) %finite.ptr, align 4
  %finite.bits = bitcast float %finite.value to i32
  %finite.exponent = and i32 %finite.bits, 2139095040
  %finite.ok = icmp ne i32 %finite.exponent, 2139095040
  %finite.next = add nuw nsw i32 %finite.index, 1
  %finite.done = icmp eq i32 %finite.next, 32
  br i1 %finite.ok, label %finite.continue, label %trap
finite.continue:
  br i1 %finite.done, label %top2.loop, label %finite.loop
trap:
  call void @llvm.trap()
  unreachable
top2.loop:
  %top2.route = phi i32 [ 0, %finite.continue ], [ %top2.next, %top2.loop ]
  %top2.token = lshr i32 %top2.route, 1
  %top2.rank = and i32 %top2.route, 1
  %top2.value = call i32 @__fe2o3_moe_select_expert_v1(ptr addrspace(1) %logits.data, i32 %top2.token, i32 %top2.rank)
  %top2.index64 = zext i32 %top2.route to i64
  %top2.ptr = getelementptr inbounds i32, ptr addrspace(1) %top2.data, i64 %top2.index64
  store i32 %top2.value, ptr addrspace(1) %top2.ptr, align 4
  %top2.next = add nuw nsw i32 %top2.route, 1
  %top2.done = icmp eq i32 %top2.next, 16
  br i1 %top2.done, label %counts.loop, label %top2.loop
counts.loop:
  %counts.expert = phi i32 [ 0, %top2.loop ], [ %counts.next, %counts.loop ]
  %requested.value = call i32 @__fe2o3_moe_requested_count_v1(ptr addrspace(1) %logits.data, i32 %counts.expert)
  %admitted.value = call i32 @__fe2o3_moe_admitted_count_v1(ptr addrspace(1) %logits.data, i32 %counts.expert)
  %counts.index64 = zext i32 %counts.expert to i64
  %requested.ptr = getelementptr inbounds i32, ptr addrspace(1) %requested.data, i64 %counts.index64
  %admitted.ptr = getelementptr inbounds i32, ptr addrspace(1) %admitted.data, i64 %counts.index64
  store i32 %requested.value, ptr addrspace(1) %requested.ptr, align 4
  store i32 %admitted.value, ptr addrspace(1) %admitted.ptr, align 4
  %counts.next = add nuw nsw i32 %counts.expert, 1
  %counts.done = icmp eq i32 %counts.next, 4
  br i1 %counts.done, label %offsets.loop, label %counts.loop
offsets.loop:
  %offset.index = phi i32 [ 0, %counts.loop ], [ %offset.next, %offsets.loop ]
  %offset.value = call i32 @__fe2o3_moe_expert_offset_v1(ptr addrspace(1) %logits.data, i32 %offset.index)
  %offset.index64 = zext i32 %offset.index to i64
  %offset.ptr = getelementptr inbounds i32, ptr addrspace(1) %offsets.data, i64 %offset.index64
  store i32 %offset.value, ptr addrspace(1) %offset.ptr, align 4
  %offset.next = add nuw nsw i32 %offset.index, 1
  %offset.done = icmp eq i32 %offset.next, 5
  br i1 %offset.done, label %routes.loop, label %offsets.loop
routes.loop:
  %route.index = phi i32 [ 0, %offsets.loop ], [ %route.next, %routes.loop ]
  %route.slot = call i32 @__fe2o3_moe_route_slot_v1(ptr addrspace(1) %logits.data, i32 %route.index)
  %route.index64 = zext i32 %route.index to i64
  %slot.ptr = getelementptr inbounds i32, ptr addrspace(1) %slots.data, i64 %route.index64
  %inverse.ptr = getelementptr inbounds i32, ptr addrspace(1) %inverse.data, i64 %route.index64
  store i32 %route.slot, ptr addrspace(1) %slot.ptr, align 4
  store i32 %route.slot, ptr addrspace(1) %inverse.ptr, align 4
  %route.next = add nuw nsw i32 %route.index, 1
  %route.done = icmp eq i32 %route.next, 16
  br i1 %route.done, label %permutation.outer, label %routes.loop
permutation.outer:
  %permutation.slot = phi i32 [ 0, %routes.loop ], [ %permutation.next.slot, %permutation.store ]
  br label %permutation.inner
permutation.inner:
  %permutation.route = phi i32 [ 0, %permutation.outer ], [ %permutation.next.route, %permutation.inner ]
  %permutation.found = phi i32 [ -1, %permutation.outer ], [ %permutation.next.found, %permutation.inner ]
  %candidate.slot = call i32 @__fe2o3_moe_route_slot_v1(ptr addrspace(1) %logits.data, i32 %permutation.route)
  %candidate.matches = icmp eq i32 %candidate.slot, %permutation.slot
  %permutation.next.found = select i1 %candidate.matches, i32 %permutation.route, i32 %permutation.found
  %permutation.next.route = add nuw nsw i32 %permutation.route, 1
  %permutation.inner.done = icmp eq i32 %permutation.next.route, 16
  br i1 %permutation.inner.done, label %permutation.store, label %permutation.inner
permutation.store:
  %permutation.value = phi i32 [ %permutation.next.found, %permutation.inner ]
  %permutation.index64 = zext i32 %permutation.slot to i64
  %permutation.ptr = getelementptr inbounds i32, ptr addrspace(1) %permutation.data, i64 %permutation.index64
  store i32 %permutation.value, ptr addrspace(1) %permutation.ptr, align 4
  %permutation.next.slot = add nuw nsw i32 %permutation.slot, 1
  %permutation.done = icmp eq i32 %permutation.next.slot, 16
  br i1 %permutation.done, label %return, label %permutation.outer
return:
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" "target-cpu"="gfx942" }
attributes #1 = { nounwind readnone speculatable willreturn }
attributes #2 = { cold noreturn nounwind }
attributes #3 = { alwaysinline nounwind readonly willreturn }

!0 = !{i32 64, i32 1, i32 1}
!1 = !{!"read_only", !"none", !"read_write", !"none", !"read_write", !"none", !"read_write", !"none", !"read_write", !"none", !"read_write", !"none", !"read_write", !"none", !"read_write", !"none"}
!2 = !{!"float*", !"ulong", !"uint*", !"ulong", !"uint*", !"ulong", !"uint*", !"ulong", !"uint*", !"ulong", !"uint*", !"ulong", !"uint*", !"ulong", !"uint*", !"ulong"}
!3 = !{!"const", !"", !"restrict", !"", !"restrict", !"", !"restrict", !"", !"restrict", !"", !"restrict", !"", !"restrict", !"", !"restrict", !""}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_lowering_is_closed_and_deterministic() {
        let ir = fe2o3_kernel_ir::moe_top2_v1_kernel_ir();
        let profile = MoeTop2ProfileV1::exact_gfx942_xnack_minus_cov6();
        let first = lower_exact_moe_top2_v1(&ir, &profile).unwrap();
        let second = lower_exact_moe_top2_v1(&ir, &profile).unwrap();
        assert_eq!(first, second);
        assert!(first.contains(EXACT_MOE_TOP2_GFX942_DATA_LAYOUT_V1));
    }

    #[test]
    fn profile_and_ir_substitution_fail_closed() {
        let mut ir = fe2o3_kernel_ir::moe_top2_v1_kernel_ir();
        let profile = MoeTop2ProfileV1::exact_gfx942_xnack_minus_cov6();
        ir.routing.swap(0, 1);
        assert!(lower_exact_moe_top2_v1(&ir, &profile).is_err());

        let ir = fe2o3_kernel_ir::moe_top2_v1_kernel_ir();
        let mut profile = MoeTop2ProfileV1::exact_gfx942_xnack_minus_cov6();
        profile.grid[0] = 2;
        assert!(lower_exact_moe_top2_v1(&ir, &profile).is_err());
    }
}
