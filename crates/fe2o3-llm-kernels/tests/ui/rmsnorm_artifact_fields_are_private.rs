use fe2o3_llm_kernels::rmsnorm::InspectedQwen3RmsNormKernelV1;

fn inspect(value: InspectedQwen3RmsNormKernelV1) {
    let InspectedQwen3RmsNormKernelV1 { catalog, .. } = value;
    let _ = catalog;
}

fn main() {}
