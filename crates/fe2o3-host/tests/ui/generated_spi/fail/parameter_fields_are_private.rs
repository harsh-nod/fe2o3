use fe2o3_host::{GeneratedAdmittedLaunch, KernelParams, LoadedArgumentAdmittedLaunch};

struct Kernel;

fn forge<'loaded, 'allocation>(
    admitted: LoadedArgumentAdmittedLaunch<'loaded, 'allocation, Kernel>,
    params: KernelParams,
) -> GeneratedAdmittedLaunch<'loaded, 'allocation, Kernel, ()> {
    GeneratedAdmittedLaunch {
        admitted,
        params,
        resources: (),
    }
}

fn main() {}
