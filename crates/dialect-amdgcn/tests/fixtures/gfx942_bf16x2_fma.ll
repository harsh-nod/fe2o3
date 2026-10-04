target triple = "amdgcn-amd-amdhsa"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1
declare float @llvm.experimental.constrained.fma.f32(float, float, float, metadata, metadata)

define internal float @__fe2o3_bf16_to_f32_v1(i16 %bits) alwaysinline nounwind "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" {
entry:
  %wide = zext i16 %bits to i32
  %shifted = shl i32 %wide, 16
  %result = bitcast i32 %shifted to float
  ret float %result
}

define internal i16 @__fe2o3_f32_to_bf16_rne_v1(float %value) alwaysinline nounwind "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" {
entry:
  %bits = bitcast float %value to i32
  %exponent = and i32 %bits, 2139095040
  %fraction = and i32 %bits, 8388607
  %special = icmp eq i32 %exponent, 2139095040
  %payload = icmp ne i32 %fraction, 0
  %is.nan = and i1 %special, %payload
  %upper = lshr i32 %bits, 16
  %nan = or i32 %upper, 64
  %lsb = and i32 %upper, 1
  %bias = add i32 32767, %lsb
  %biased = add i32 %bits, %bias
  %rounded = lshr i32 %biased, 16
  %selected = select i1 %is.nan, i32 %nan, i32 %rounded
  %result = trunc i32 %selected to i16
  ret i16 %result
}

define amdgpu_kernel void @math_kernel(i32 %arg0, i32 %arg1, i32 %arg2) #0 !reqd_work_group_size !0 {
bb0:
  %v3.value.lo = trunc i32 %arg0 to i16
  %v3.value.shift = lshr i32 %arg0, 16
  %v3.value.hi = trunc i32 %v3.value.shift to i16
  %v3.value.0 = call float @__fe2o3_bf16_to_f32_v1(i16 %v3.value.lo)
  %v3.value.1 = call float @__fe2o3_bf16_to_f32_v1(i16 %v3.value.hi)
  %v3.multiplier.lo = trunc i32 %arg1 to i16
  %v3.multiplier.shift = lshr i32 %arg1, 16
  %v3.multiplier.hi = trunc i32 %v3.multiplier.shift to i16
  %v3.multiplier.0 = call float @__fe2o3_bf16_to_f32_v1(i16 %v3.multiplier.lo)
  %v3.multiplier.1 = call float @__fe2o3_bf16_to_f32_v1(i16 %v3.multiplier.hi)
  %v3.addend.lo = trunc i32 %arg2 to i16
  %v3.addend.shift = lshr i32 %arg2, 16
  %v3.addend.hi = trunc i32 %v3.addend.shift to i16
  %v3.addend.0 = call float @__fe2o3_bf16_to_f32_v1(i16 %v3.addend.lo)
  %v3.addend.1 = call float @__fe2o3_bf16_to_f32_v1(i16 %v3.addend.hi)
  %v3.fma.0 = call float @llvm.experimental.constrained.fma.f32(float %v3.value.0, float %v3.multiplier.0, float %v3.addend.0, metadata !"round.tonearest", metadata !"fpexcept.ignore")
  %v3.bf16.0 = call i16 @__fe2o3_f32_to_bf16_rne_v1(float %v3.fma.0)
  %v3.wide.0 = zext i16 %v3.bf16.0 to i32
  %v3.fma.1 = call float @llvm.experimental.constrained.fma.f32(float %v3.value.1, float %v3.multiplier.1, float %v3.addend.1, metadata !"round.tonearest", metadata !"fpexcept.ignore")
  %v3.bf16.1 = call i16 @__fe2o3_f32_to_bf16_rne_v1(float %v3.fma.1)
  %v3.wide.1 = zext i16 %v3.bf16.1 to i32
  %v3.high = shl i32 %v3.wide.1, 16
  %v3 = or i32 %v3.wide.0, %v3.high
  ret void
}

attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-cpu"="gfx942" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" }
attributes #1 = { nounwind readnone speculatable willreturn }

!0 = !{i32 64, i32 1, i32 1}
