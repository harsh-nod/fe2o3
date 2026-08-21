use fe2o3_llm_kernels::gemm::InspectedQwen3GemmKernelSetV1;

fn inspect(value: InspectedQwen3GemmKernelSetV1) {
    let InspectedQwen3GemmKernelSetV1 { catalog, .. } = value;
    let _ = catalog;
}

fn main() {}
