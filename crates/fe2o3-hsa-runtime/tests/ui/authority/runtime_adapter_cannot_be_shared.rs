use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;

fn require_sync<T: Sync>() {}

fn main() {
    require_sync::<ReviewedHsaRuntimeAdapterV1>();
}
