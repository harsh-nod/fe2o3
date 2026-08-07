use fe2o3_host::{
    HsaKernelLaunchAuthorizationV1, HsaLaunchGeometryV1, LoadedHsaExecutableV1,
    ReviewedHsaExecutableLifecycleAdapterV1,
};

fn escape<K, A: ReviewedHsaExecutableLifecycleAdapterV1>(
    loaded: &mut LoadedHsaExecutableV1<K, A>,
) -> HsaKernelLaunchAuthorizationV1<'static, K, A> {
    loaded
        .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
        .unwrap()
}

fn main() {}
