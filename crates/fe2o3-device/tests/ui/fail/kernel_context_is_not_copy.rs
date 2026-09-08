use fe2o3_device::KernelContext;

fn require_copy<T: Copy>() {}

fn main() {
    require_copy::<KernelContext<'static>>();
}
