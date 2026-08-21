use fe2o3_llm_kernels::rmsnorm::{
    PreparedQwen3RmsNormKernelV1, Qwen3RmsNormProfileCatalogV1,
};

fn forge(catalog: Qwen3RmsNormProfileCatalogV1) -> PreparedQwen3RmsNormKernelV1 {
    PreparedQwen3RmsNormKernelV1 {
        catalog,
        source_identity: todo!(),
        worker_admission_identity: todo!(),
        assembly: todo!(),
        compiler_handoff_identity: todo!(),
        manifest_identity: todo!(),
        compiler_handoff: todo!(),
    }
}

fn main() {}
