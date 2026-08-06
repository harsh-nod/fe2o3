use fe2o3_host::GeneratedAdmittedLaunch;

struct Kernel;

fn swap<'loaded, 'allocation>(
    first: GeneratedAdmittedLaunch<'loaded, 'allocation, Kernel, ()>,
    second: GeneratedAdmittedLaunch<'loaded, 'allocation, Kernel, ()>,
) -> GeneratedAdmittedLaunch<'loaded, 'allocation, Kernel, ()> {
    GeneratedAdmittedLaunch {
        admitted: first.admitted,
        params: second.params,
        resources: second.resources,
    }
}

fn main() {}
