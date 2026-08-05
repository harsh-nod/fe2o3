use fe2o3_host::{ArgumentAdmittedLaunch, ArgumentAliasAdmission, PreparedLaunch};

struct Kernel;

fn impossible<T>() -> T {
    panic!("cannot create the sealed registration")
}

fn forge<'allocation>(
    prepared: PreparedLaunch<Kernel>,
    admission: ArgumentAliasAdmission<'allocation>,
) -> ArgumentAdmittedLaunch<'allocation, Kernel> {
    ArgumentAdmittedLaunch {
        prepared,
        admission,
        _registration: impossible(),
    }
}

fn main() {}
