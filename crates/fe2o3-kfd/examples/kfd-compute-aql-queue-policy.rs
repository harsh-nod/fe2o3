//! Default-feature link target for the pure-Rust compute-AQL queue policy.

fn main() {
    println!(
        "{} {}",
        fe2o3_kfd::GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1,
        fe2o3_kfd::NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_SHA256_V1,
    );
}
