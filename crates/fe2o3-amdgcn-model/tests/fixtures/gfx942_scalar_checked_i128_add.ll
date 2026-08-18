target triple = "amdgcn-amd-amdhsa"

declare { i128, i1 } @llvm.sadd.with.overflow.i128(i128, i128)

define { i128, i1 } @__fe2o3_ir_scalar_v2_4645324f5356320003000800000000000101010003050100(i128 %arg0, i128 %arg1) #0 {
entry:
  %pair = call { i128, i1 } @llvm.sadd.with.overflow.i128(i128 %arg0, i128 %arg1)
  %value = extractvalue { i128, i1 } %pair, 0
  %overflow = extractvalue { i128, i1 } %pair, 1
  %valid = xor i1 %overflow, true
  %result.0 = insertvalue { i128, i1 } poison, i128 %value, 0
  %result.1 = insertvalue { i128, i1 } %result.0, i1 %valid, 1
  ret { i128, i1 } %result.1
}

attributes #0 = { nounwind "target-cpu"="gfx942" "denormal-fp-math"="ieee,ieee" "denormal-fp-math-f32"="ieee,ieee" "unsafe-fp-math"="false" "no-infs-fp-math"="false" "no-nans-fp-math"="false" "no-signed-zeros-fp-math"="false" "approx-func-fp-math"="false" }
