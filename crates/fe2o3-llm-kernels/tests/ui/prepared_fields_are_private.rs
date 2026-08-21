use fe2o3_llm_kernels::gemm::{
    PreparedQwen3GemmKernelSetV1, Qwen3GemmProfileCatalogV1,
};

fn forge(catalog: Qwen3GemmProfileCatalogV1) -> PreparedQwen3GemmKernelSetV1 {
    PreparedQwen3GemmKernelSetV1 {
        catalog,
        reference: todo!(),
        vectorized: todo!(),
    }
}

fn main() {}
