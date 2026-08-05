use fe2o3_host::{GeneratedAdmittedLaunch, KernelParams, LoadedArgumentAdmittedLaunch};

struct Kernel;

fn forge<'loaded, 'allocation, 'params>(
    admitted: LoadedArgumentAdmittedLaunch<'loaded, 'allocation, Kernel>,
    params: &'params mut KernelParams,
) -> GeneratedAdmittedLaunch<'loaded, 'allocation, 'params, Kernel, ()> {
    GeneratedAdmittedLaunch {
        admitted,
        params,
        resources: (),
    }
}

fn main() {}
