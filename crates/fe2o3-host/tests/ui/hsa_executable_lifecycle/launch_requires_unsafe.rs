use fe2o3_host::{HsaKernelLaunchAuthorizationV1, ReviewedHsaExecutableLifecycleAdapterV1};

fn dispatch<K, A: ReviewedHsaExecutableLifecycleAdapterV1>(
    authorization: HsaKernelLaunchAuthorizationV1<'_, K, A>,
    kernarg: &mut [u8],
) {
    let _completed = authorization.launch_and_wait(kernarg);
}

fn main() {}
