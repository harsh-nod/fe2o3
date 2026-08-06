use fe2o3_host::{GeneratedAdmittedLaunch, KernelParams, LoadedArgumentAdmittedLaunch};

struct Kernel;

fn pair(
    admitted: LoadedArgumentAdmittedLaunch<'_, '_, Kernel>,
    params: KernelParams,
) {
    let _ = GeneratedAdmittedLaunch::from_generated_unchecked(admitted, params, ());
}

fn main() {}
