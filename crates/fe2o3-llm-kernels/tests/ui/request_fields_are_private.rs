use fe2o3_llm_kernels::gemm::InertQwen3GemmKernelWorkerRequestV1;

fn inspect(value: InertQwen3GemmKernelWorkerRequestV1) {
    let InertQwen3GemmKernelWorkerRequestV1 { catalog, .. } = value;
    let _ = catalog;
}

fn main() {}
