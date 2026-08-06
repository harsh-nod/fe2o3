use fe2o3_core::{BorrowedDeviceOperation, Stream};
use fe2o3_host::{GeneratedAdmittedLaunch, LoadedLaunchError};

struct Kernel;

fn escape<'stream, 'loaded, 'allocation, R>(
    paired: GeneratedAdmittedLaunch<'loaded, 'allocation, Kernel, R>,
    stream: &'stream Stream,
) -> Result<&'stream BorrowedDeviceOperation<'stream, 'allocation>, LoadedLaunchError> {
    paired.launch_generated_scoped(stream, |operation| operation)
}

fn main() {}
