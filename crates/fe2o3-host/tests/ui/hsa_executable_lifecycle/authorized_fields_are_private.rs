use fe2o3_host::{AuthorizedHsaLoadV1, ReviewedHsaExecutableLifecycleAdapterV1};

fn extract_adapter<K, A: ReviewedHsaExecutableLifecycleAdapterV1>(
    authorized: AuthorizedHsaLoadV1<K, A>,
) {
    let _adapter = authorized.adapter;
}

fn main() {}
