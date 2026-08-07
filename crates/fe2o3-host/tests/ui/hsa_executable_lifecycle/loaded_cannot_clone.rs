use fe2o3_host::{LoadedHsaExecutableV1, ReviewedHsaExecutableLifecycleAdapterV1};

fn duplicate<K, A: ReviewedHsaExecutableLifecycleAdapterV1>(
    loaded: LoadedHsaExecutableV1<K, A>,
) {
    let _duplicate = loaded.clone();
}

fn main() {}
