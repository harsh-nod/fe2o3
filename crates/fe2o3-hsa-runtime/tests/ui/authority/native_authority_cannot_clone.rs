use fe2o3_hsa_runtime::{
    ReviewedHsaExecutableV1, ReviewedHsaKernelV1, ReviewedHsaRuntimeAdapterV1,
};

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<ReviewedHsaExecutableV1>();
    require_clone::<ReviewedHsaKernelV1>();
    require_clone::<ReviewedHsaRuntimeAdapterV1>();
}
