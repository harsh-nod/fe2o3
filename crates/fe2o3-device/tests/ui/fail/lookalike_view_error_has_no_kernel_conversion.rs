use fe2o3_device::KernelResult;

struct LookalikeViewError;

fn lookalike_constructor() -> Result<(), LookalikeViewError> {
    Err(LookalikeViewError)
}

fn kernel_path() -> KernelResult {
    lookalike_constructor()?;
    Ok(())
}

fn main() {
    let _ = kernel_path();
}
