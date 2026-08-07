use fe2o3_host::{AuthorizedHsaLoadV1, ReviewedHsaExecutableLifecycleAdapterV1};

fn extract_bytes<K, A: ReviewedHsaExecutableLifecycleAdapterV1>(
    authorized: &AuthorizedHsaLoadV1<K, A>,
) {
    let _bytes = authorized.exact_bytes();
}

fn main() {}
