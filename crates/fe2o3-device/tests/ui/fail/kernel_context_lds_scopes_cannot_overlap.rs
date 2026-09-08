use fe2o3_device::KernelContext;

enum KernelA {}

fn overlap<'kernel>(context: &mut KernelContext<'kernel, KernelA>) {
    let first = context.workgroup_lds();
    let second = context.workgroup_lds();
    drop(first);
    drop(second);
}

fn main() {}
