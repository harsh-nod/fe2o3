use fe2o3_bf16_gemm_v1::PreparedBf16GemmKernelV1;

fn duplicate(value: PreparedBf16GemmKernelV1) {
    let _ = value.clone();
}

fn main() {}
