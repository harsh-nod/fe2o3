use fe2o3_core::Stream;
use fe2o3_host::{KernelParams, LoadedArgumentAdmittedLaunch};

struct Kernel;

fn launch(
    admitted: LoadedArgumentAdmittedLaunch<'_, '_, Kernel>,
    stream: &Stream,
    params: &mut KernelParams,
) {
    admitted.launch_raw(stream, params).unwrap();
}

fn main() {}
