use fe2o3_host::GeneratedAdmittedLaunch;

struct Kernel;

fn swap<'loaded, 'allocation, 'params>(
    first: GeneratedAdmittedLaunch<'loaded, 'allocation, 'params, Kernel, ()>,
    second: GeneratedAdmittedLaunch<'loaded, 'allocation, 'params, Kernel, ()>,
) -> GeneratedAdmittedLaunch<'loaded, 'allocation, 'params, Kernel, ()> {
    GeneratedAdmittedLaunch {
        admitted: first.admitted,
        params: second.params,
        resources: second.resources,
    }
}

fn main() {}
