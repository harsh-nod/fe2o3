use fe2o3_host::{LoadedKernel, LoadedPreparedLaunch, PreparedLaunch};

struct Kernel;

fn detach<'a>(
    loaded: &'a LoadedKernel<Kernel>,
    prepared: PreparedLaunch<Kernel>,
) -> LoadedPreparedLaunch<'static, Kernel> {
    loaded.bind(prepared).unwrap()
}

fn main() {}
