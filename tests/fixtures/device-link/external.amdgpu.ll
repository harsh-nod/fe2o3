; External AMDGPU side of the bidirectional G7 fixture.
; The Rust kernel imports external_scale_bias_v1. This definition then imports
; rust_accumulate_v1 from the Rust device object, closing both directions.

target triple = "amdgcn-amd-amdhsa"

declare i32 @rust_accumulate_v1(i32, i32)

define protected i32 @external_scale_bias_v1(i32 %value, i32 %lane) #0 {
entry:
  %scaled = mul i32 %value, 3
  %biased = add i32 %scaled, 5
  %result = call i32 @rust_accumulate_v1(i32 %biased, i32 %lane)
  ret i32 %result
}

attributes #0 = { nounwind "target-cpu"="gfx942" "target-features"="+sramecc,-xnack" }

!llvm.module.flags = !{!0, !1, !2}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
!1 = !{i32 1, !"amdgpu.sramecc", i32 1}
!2 = !{i32 1, !"amdgpu.xnack", i32 0}
