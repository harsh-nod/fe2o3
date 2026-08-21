use fe2o3_bf16_gemm_v1::PreparedBf16GemmKernelV1;

fn extract(value: PreparedBf16GemmKernelV1) {
    let _ = value.compiler_handoff;
}

fn main() {}
