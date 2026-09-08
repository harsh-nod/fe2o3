use fe2o3_device::KernelContext;

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<KernelContext<'static>>();
}
