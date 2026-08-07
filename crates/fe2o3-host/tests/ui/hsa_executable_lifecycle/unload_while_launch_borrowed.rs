use fe2o3_host::{
    HsaLaunchGeometryV1, LoadedHsaExecutableV1, ReviewedHsaExecutableLifecycleAdapterV1,
};

fn unload_early<K, A: ReviewedHsaExecutableLifecycleAdapterV1>(
    mut loaded: LoadedHsaExecutableV1<K, A>,
) {
    let launch = loaded
        .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
        .unwrap();
    let _unloaded = loaded.unload();
    drop(launch);
}

fn main() {}
