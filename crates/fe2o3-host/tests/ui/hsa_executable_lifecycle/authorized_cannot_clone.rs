use fe2o3_host::{AuthorizedHsaLoadV1, ReviewedHsaExecutableLifecycleAdapterV1};

fn duplicate<K, A: ReviewedHsaExecutableLifecycleAdapterV1>(
    authorized: AuthorizedHsaLoadV1<K, A>,
) {
    let _duplicate = authorized.clone();
}

fn main() {}
