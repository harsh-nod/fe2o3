use fe2o3_core::{CooperativeLaunchCapability, Stream};
use fe2o3_host::{CooperativeLaunchAdmission, LoadedPreparedLaunch};

fn escape<'loaded, K>(
    launch: LoadedPreparedLaunch<'loaded, K>,
    capability: CooperativeLaunchCapability,
    stream: &Stream,
) -> CooperativeLaunchAdmission<'loaded, 'static, K> {
    launch.admit_cooperative(capability, stream).unwrap()
}

fn main() {}
