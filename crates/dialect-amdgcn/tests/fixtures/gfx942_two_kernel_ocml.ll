target triple = "amdgcn-amd-amdhsa"

declare float @__ocml_sin_f32(float)

define amdgpu_kernel void @math_kernel(float %arg0) #0 !reqd_work_group_size !0 {
bb0:
  %v1 = call float @shared_math_helper(float %arg0)
  %v2 = call float @__ocml_sin_f32(float %v1)
  ret void
}

define amdgpu_kernel void @plain_kernel(float %arg0) #1 !reqd_work_group_size !1 {
bb0:
  %v1 = call float @shared_math_helper(float %arg0)
  ret void
}

define internal float @shared_math_helper(float %arg0) nounwind "target-features"="-wavefrontsize32,+wavefrontsize64" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" {
bb0:
  ret float %arg0
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-features"="-wavefrontsize32,+wavefrontsize64" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" }
attributes #1 = { nounwind "amdgpu-flat-work-group-size"="128,128" "target-features"="-wavefrontsize32,+wavefrontsize64" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" }

!0 = !{i32 64, i32 1, i32 1}
!1 = !{i32 128, i32 1, i32 1}
