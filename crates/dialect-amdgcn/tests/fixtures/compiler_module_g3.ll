target triple = "amdgcn-amd-amdhsa"

declare i32 @external_bias(i32)

define amdgpu_kernel void @alpha_kernel(i32 %arg0) #0 !reqd_work_group_size !0 {
bb0:
  %v1 = call i32 @scale(i32 %arg0)
  ret void
}

define amdgpu_kernel void @zeta_kernel(i32 %arg0, i32 %arg1) #1 !reqd_work_group_size !1 {
bb0:
  %v2 = call i32 @scale(i32 %arg0)
  %v3 = call i32 @public_adjust(i32 %arg1)
  ret void
}

define i32 @public_adjust(i32 %arg0) nounwind {
bb0:
  %v1 = call i32 @external_bias(i32 %arg0)
  ret i32 %v1
}

define i32 @scale(i32 %arg0) nounwind {
bb0:
  %v2 = mul i32 %arg0, 2
  ret i32 %v2
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" }
attributes #1 = { nounwind "amdgpu-flat-work-group-size"="128,128" }

!0 = !{i32 64, i32 1, i32 1}
!1 = !{i32 128, i32 1, i32 1}
