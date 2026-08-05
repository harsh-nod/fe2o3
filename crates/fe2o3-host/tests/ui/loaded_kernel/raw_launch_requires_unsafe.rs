use fe2o3_core::Stream;
use fe2o3_host::{KernelParams, LoadedPreparedLaunch};

struct Kernel;

fn launch(
    prepared: LoadedPreparedLaunch<'_, Kernel>,
    stream: &Stream,
    params: &mut KernelParams,
) {
    prepared.launch_raw(stream, params).unwrap();
}

fn main() {}
