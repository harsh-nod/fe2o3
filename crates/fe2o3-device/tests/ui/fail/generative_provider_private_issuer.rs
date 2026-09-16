use fe2o3_device::KernelContext;

fn bypass(context: &mut KernelContext<'_>) {
    let _ = context.__compiler_workgroup_capability_current();
}

fn main() {}
